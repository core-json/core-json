use core::{str::FromStr, fmt::Write};

use crate::{Read, PeekableRead, Stack, JsonError};

/// An implementor of `core::fmt::Write` which writes to a slice.
struct SliceWrite<'a>(&'a mut [u8], usize);
impl<'a> Write for SliceWrite<'a> {
  #[inline(always)]
  fn write_str(&mut self, s: &str) -> core::fmt::Result {
    let remaining = self.0.len() - self.1;
    if remaining < s.len() {
      Err(core::fmt::Error)?;
    }
    self.0[self.1 .. (self.1 + s.len())].copy_from_slice(s.as_bytes());
    self.1 += s.len();
    Ok(())
  }
}

// `+ 1` as `ilog10` rounds down, `+ 1` as `10` has a logarithm of `1` yet requires two digits
const I64_SIGNIFICANT_DIGITS: usize = (i64::MAX.ilog10() + 1 + 1) as usize;
const F64_SIGNIFICANT_DIGITS: usize = f64::DIGITS as usize;
const SIGNIFICANT_DIGITS: usize = if I64_SIGNIFICANT_DIGITS > F64_SIGNIFICANT_DIGITS {
  I64_SIGNIFICANT_DIGITS
} else {
  F64_SIGNIFICANT_DIGITS
};

/// A sink for a number string.
///
/// This sink does two things:
///   1) Accumulates into a integer
///   2) Accumulates a fixed-length string representing a float
///
/// For the latter task, the string representing the float is approximate to the string sinked
/// (despite the bounded length) due to preserving the significant digits.
///
/// Writing into the sink is infallible. Recovering the result is possible for any float which
/// follows RFC 8259's syntax and whose exponent fits within an `i16`, or for the Rust strings for
/// any finite `f64`. The result for strings which don't follow either of these syntaxes is
/// undefined. As Rust does not stricly define the format it outputs strings with, we refer to the
/// format it'll accept strings with (which is specified), assuming the strings output are a subset
/// of the ones allowed as input.
#[doc(hidden)]
pub struct NumberSink {
  /// If a sign character is currently allowed within the sink.
  sign_character_allowed: bool,
  /// The amount of digits read for the current part.
  digits_in_current_part: usize,

  /// The sign of the number.
  negative: bool,
  /// The significant digits within the number.
  ///
  /// These will be ASCII characters in the range '0' ..= '9', and '0' if not explicitly set.
  digits: [u8; SIGNIFICANT_DIGITS],
  /// The amount of dsigits accumulated.
  i: usize,

  /// If we're before the decimal point.
  before_decimal: bool,
  /// If we're before the exponent marker.
  before_exponent: bool,

  /// If the exponent is negative.
  negative_exponent: bool,
  /// The absolute value of the exponent embedded within the string.
  ///
  /// This will always contain a positive value and is solely represented with an `i16` to bound
  /// its maximum value so it may be infallibly converted to its negative value.
  ///
  /// If this is `None`, it means the exponent overflowed the capacity.
  absolute_exponent: Option<i16>,
  /// The correction required for the xponent.
  ///
  /// This is required due to us shifting the string when we accumulate it in a bounded fashion. We
  /// represent it as an `i64`, allowing us to accumulate strings of length `2**63 - 2**15` without
  /// a failure occuring. As sequentially iterating to 2**63 would take a century, requiring a
  // 8,192 PB string, we may consider this infalliible.
  exponent_correction: i64,

  /// If we truncated a non-zero digit.
  ///
  /// Truncated zero digits will be reflected in the correction to the exponent, making them
  /// losslessly dropped.
  imprecise: bool,

  /// If this value was invalid per RFC-8259 syntax.
  ///
  /// This will only potentially be true if strict validation is used.
  invalid: bool,
}

impl NumberSink {
  /// Create a new number sink.
  #[doc(hidden)]
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      sign_character_allowed: true,
      digits_in_current_part: 0,
      negative: false,
      digits: [b'0'; _],
      i: 0,
      before_decimal: true,
      before_exponent: true,
      negative_exponent: false,
      absolute_exponent: Some(0),
      exponent_correction: 0,
      imprecise: false,
      invalid: false,
    }
  }

  /// Push a byte, intended to be an ASCII character, into the sink.
  ///
  /// If `strict` is true, this will apply the validation rules from RFC-8259.
  /*
    The syntax we apply when `strict = true` is (expanded)
    `[ minus ] int [ decimal-point 1*DIGIT ] [ e [ minus / plus ] 1*DIGIT ]`.

    https://datatracker.ietf.org/doc/html/rfc8259#section-6 lets us specify the range, precision
    of numbers.
  */
  #[inline(always)]
  fn push_byte(&mut self, strict: bool, c: u8) {
    if self.invalid {
      return;
    }

    let sign_character_allowed = self.sign_character_allowed;
    self.sign_character_allowed = false;

    if self.before_exponent {
      match c {
        b'-' => {
          if strict && (!sign_character_allowed) {
            self.invalid = true;
            return;
          }
          self.negative = true;
        }
        b'e' | b'E' => {
          // Only one 'e'/'E' is allowed, and this entire `match` statement is before it
          // That leaves us solely to check the preceding part wasn't empty, as never allowed
          if strict && (self.digits_in_current_part == 0) {
            self.invalid = true;
            return;
          }
          self.sign_character_allowed = true;
          self.digits_in_current_part = 0;
          self.before_exponent = false;
        }
        b'0' ..= b'9' => {
          // We do not allow leading zeroes for the integer part, unless it's solely zero
          if strict && self.before_decimal && (self.digits_in_current_part == 1) && (self.i == 0) {
            self.invalid = true;
            return;
          }
          self.digits_in_current_part += 1;

          // Drop leading zeroes
          if (self.i == 0) && (c == b'0') {
            // If we're after the decimal point, this effectively shifts the number up
            if !self.before_decimal {
              // Correct the exponent accordingly
              self.exponent_correction -= 1;
            }

            return;
          }
          // If this outside of our supported significant digits, drop it
          if self.i == self.digits.len() {
            // If we're before the decimal, dropping this effectively shifts the number down
            if self.before_decimal {
              // Correct the exponent accordingly
              self.exponent_correction += 1;
            }
            self.imprecise = true;
            return;
          }

          // Write the digit
          self.digits[self.i] = c;
          self.i += 1;

          // If we're after the decimal place, note this has to be shifted down a digit
          if !self.before_decimal {
            self.exponent_correction -= 1;
          }
        }
        b'.' => {
          // Only one '.' is allowed, and empty parts are never allowed
          // It also must be before the exponent, yet this entire `match` statement is
          if strict && ((!self.before_decimal) || (self.digits_in_current_part == 0)) {
            self.invalid = true;
            return;
          }
          self.digits_in_current_part = 0;
          self.before_decimal = false;
        }
        _ => {
          if strict {
            self.invalid = true;
            return;
          }
        }
      }
      return;
    }

    #[allow(clippy::needless_return)]
    match c {
      b'-' => {
        if strict && (!sign_character_allowed) {
          self.invalid = true;
          return;
        }
        self.negative_exponent = true;
      }
      // '+' is only allowed immediately after the exponent
      b'+' => {
        if strict && (!sign_character_allowed) {
          self.invalid = true;
          return;
        }
      }
      b'0' ..= b'9' => {
        self.digits_in_current_part += 1;
        // Accumulate into our exponent
        self.absolute_exponent = self.absolute_exponent.and_then(|absolute_exponent| {
          let absolute_exponent = absolute_exponent.checked_mul(10)?;
          absolute_exponent.checked_add(i16::from(c - b'0'))
        });
      }
      _ => {
        if strict {
          self.invalid = true;
          return;
        }
      }
    }
  }

  /// Get the significant digits, exponent for the number.
  ///
  /// If this has an unnecessarily large negative exponent, it will reduce it as possible. This
  /// allows "100e-1" to still be detected as not having a fractional part.
  #[inline(always)]
  fn significant_digits_and_exponent(&self) -> Option<(usize, i64)> {
    let absolute_exponent = self.absolute_exponent?;
    // This negation is infallible as `i16::MIN.abs() > i16::MAX` and it's currently positive
    let embedded_exponent =
      if self.negative_exponent { -absolute_exponent } else { absolute_exponent };
    let mut exponent = i64::from(embedded_exponent).checked_add(self.exponent_correction)?;

    let mut significant_digits = self.i;
    // Normalize this number's negative exponent, as possible
    while (significant_digits > 0) && (exponent < 0) {
      if self.digits[significant_digits - 1] != b'0' {
        break;
      }
      significant_digits -= 1;
      exponent += 1;
    }
    Some((significant_digits, exponent))
  }

  #[inline(always)]
  fn strictly_valid(&self) -> bool {
    // It has to not have been marked invalid and the last part must not have been empty
    !(self.invalid || (self.digits_in_current_part == 0))
  }

  /// Extract the exact number as an integer, if possible.
  #[inline(always)]
  pub(crate) fn i64(&self) -> Option<i64> {
    if !self.strictly_valid() {
      None?;
    }

    let (significant_digits, exponent) = self.significant_digits_and_exponent()?;

    // If this number had a loss of precision, we should not return it here.
    if self.imprecise {
      None?;
    }

    // If this had no significant digits, it is zero
    if significant_digits == 0 {
      return Some(0);
    }

    // If this number has a negative exponent, it has a fractional part
    if exponent < 0 {
      None?;
    }

    /*
      We do this manually, instead of using `i64::from_str`, to avoid the overhead of
      `str::from_utf8`/usage of `unsafe`. We also do the first loop, with wrapping arithmetic, when
      we know the value won't overflow, only doing the final steps with checked arithmetic, when
      the value might overflow.
    */
    let mut accum = 0i64;
    for digit in self.digits.iter().take(significant_digits.min(18)) {
      accum = accum.wrapping_mul(10);
      let digit = i64::from(digit - b'0');
      accum = accum.wrapping_add(if self.negative { -digit } else { digit });
    }
    for digit in &self.digits[18 .. significant_digits.max(18)] {
      accum = accum.checked_mul(10)?;
      let digit = i64::from(digit - b'0');
      accum = accum.checked_add(if self.negative { -digit } else { digit })?;
    }

    // Shift corresponding to the exponent
    for _ in 0 .. exponent {
      accum = accum.checked_mul(10)?;
    }

    Some(accum)
  }

  /// The imprecise string representing this number.
  ///
  /// This returns an owned `u8` array and the length of the string (in bytes) written within it.
  /// All of the bytes not declared to be written to are left in an undefined state. The string
  /// written will be RFC-8259-compliant.
  /*
    The length is determined due to
    `sign, significant digits, exponent marker, exponent sign, exponent`.

    We could achieve a tighter bound on the exponent, as we use `i64` for the exponent internally,
    but any exponent exceeding four decimal digits to encode its absolute value won't work with
    `f64` regardless.

    TODO: Introduce a heuristic for where we should insert a decimal, instead of always using an
    exponent to position the fractional part.
  */
  #[doc(hidden)]
  #[inline(always)]
  pub fn imprecise_str(
    &self,
  ) -> Option<([u8; 1 + SIGNIFICANT_DIGITS + 1 + 1 + I64_SIGNIFICANT_DIGITS], usize)> {
    let (original_significant_digits, mut exponent) = self.significant_digits_and_exponent()?;

    // If there are no digits within this number, return `0` immediately
    if original_significant_digits == 0 {
      return Some(([b'0'; _], 1));
    }

    let mut str = [0; _];
    let mut len = 0;
    if self.negative {
      str[len] = b'-';
      len += 1;
    }

    // Copy the significant digits
    /*
      While we support `SIGNIFICANT_DIGITS` as necessary for exact conversions to integers, for
      floats (as assumed by this function), we only use the amount of significant digits Rust can
      accurately round-trip: `f64::DIGITS`.
    */
    let significant_digits = original_significant_digits.min(f64::DIGITS as usize);
    {
      // If we're truncating digits from the tail, shift the number back up accordingly
      // This is a safe cast so long as `|SIGNIFICANT_DIGITS - f64::DIGITS| < i64::MAX`.
      #[allow(clippy::cast_possible_wrap)]
      let further_exponent_correction = (original_significant_digits - significant_digits) as i64;
      exponent = exponent.checked_add(further_exponent_correction)?;
    }
    str[len .. (len + significant_digits)].copy_from_slice(&self.digits[.. significant_digits]);
    len += significant_digits;

    if exponent != 0 {
      // Set the exponent marker
      str[len] = b'e';
      len += 1;

      // Set the exponent itself
      let mut writer = SliceWrite(&mut str[len ..], 0);
      // This should be unreachable if `I64_SIGNIFICANT_DIGITS` is properly defined and Rust is
      // sane
      write!(&mut writer, "{}", exponent).ok()?;
      len += writer.1;
    }

    Some((str, len))
  }

  /// Extract the number as a float.
  ///
  /// This will only return the number if it's finite, as RFC-8259 JSON is not able to represent
  /// infinite values, so deserializing into an infinite value demonstrates we weren't able to
  /// capture the range of this value.
  #[inline(always)]
  pub(crate) fn f64(&self) -> Option<f64> {
    if !self.strictly_valid() {
      None?;
    }

    let (str, len) = self.imprecise_str()?;

    /*
      These should be unreachable as if we yielded a string, it should be RFC-8259-compliant and
      Rust should be able to handle RFC-8259-compliant strings (due to its accepted grammar being a
      superset of RFC-8259 by happenstance/reasonability).
    */
    let str = core::str::from_utf8(&str[.. len]).ok()?;
    let candidate = f64::from_str(str).ok()?;

    if !candidate.is_finite() {
      None?;
    }

    Some(candidate)
  }
}

impl Write for NumberSink {
  #[inline(always)]
  fn write_str(&mut self, s: &str) -> core::fmt::Result {
    for s in s.as_bytes() {
      // Don't apply the validation rules, allowing accepting Rust's serializations
      self.push_byte(false, *s);
    }
    Ok(())
  }
}

/// Handle the immediate value within the reader as a number.
#[inline(always)]
pub(crate) fn to_number_str<'read, R: Read<'read>, S: Stack>(
  reader: &mut PeekableRead<'read, R>,
) -> Result<Number, JsonError<'read, R, S>> {
  let mut result = NumberSink::new();

  // Read until a byte which isn't part of the number, sinking along the way
  loop {
    let char = reader.peek().map_err(|e| JsonError::ReadError(e))?;
    // separator, array closure, object closure, whitespace
    // https://datatracker.ietf.org/doc/html/rfc8259#section-2
    if matches!(char, b',' | b']' | b'}' | b'\x20' | b'\x09' | b'\x0A' | b'\x0D') {
      break;
    }
    let char = reader.read_byte().map_err(|e| JsonError::ReadError(e))?;
    // Do apply the RFC-8259 validation rules
    result.push_byte(true, char);
  }

  if result.invalid || (result.digits_in_current_part == 0) {
    Err(JsonError::TypeError)?;
  }

  Ok(Number(result))
}

/// A number deserialized from JSON.
pub struct Number(NumberSink);
impl Number {
  /// Get the current number as an `i64`.
  ///
  /// This uses the definition of a number defined in RFC-8259, then constrains it to having no
  /// fractional part once normalized. It's yielded if it's representable within an `i64`. Note
  /// normalization will truncate "10.0", so this is lossy to if the original encoding had a
  /// fractional part.
  ///
  /// This is _exact_. It does not go through `f64` and does not experience its approximations.
  #[inline(always)]
  pub fn i64(&self) -> Option<i64> {
    self.0.i64()
  }

  /// Get the current item as an `f64`.
  ///
  /// This may be lossy due to:
  /// - The inherent nature of floats
  /// - Rust's bounds on precision
  /// - This library's precision bounds, truncating additional detail
  ///
  /// This returns `None` if the value's range exceed `f64`'s.
  #[inline(always)]
  pub fn f64(&self) -> Option<f64> {
    self.0.f64()
  }
}

#[test]
fn number_sink() {
  // Handle various floats
  {
    #[allow(clippy::float_cmp)]
    let test = |value: f64, expected| {
      let mut sink = NumberSink::new();
      write!(&mut sink, "{}", value).unwrap();
      assert_eq!(sink.f64().unwrap(), f64::from_str(expected).unwrap());
    };
    test(0.0, "0");
    test(0.1, "0.1");
    test(0.01, "0.01");
    test(0.001, "0.001");
    test(0.0012, "0.0012");
    test(0.12345678910111213, "0.123456789101112");
    test(0.012345678910111213, "0.0123456789101112");
    test(12345678910111213.0, "123456789101112e2");
    test(12345678910111213.123, "123456789101112e2");
    test(123456789.101112, "123456789.101112");
    test(123456789.10111213, "123456789.101112");
    test(-1.0, "-1");
    test(f64::MIN, "-179769313486231e294");
    test(f64::MAX, "179769313486231e294");
    test(f64::EPSILON, "222044604925031e-30");
  }

  // Handle various integers
  {
    #[allow(clippy::float_cmp)]
    let test = |value: &str, expected: i64| {
      let mut sink = NumberSink::new();
      write!(&mut sink, "{}", value).unwrap();
      assert_eq!(sink.i64().unwrap(), expected);
    };
    test("0", 0);
    test("10e1", 100);
    test("10.0e1", 100);
    test("10.0", 10);
    test("10e-1", 1);
    {
      let str = format!("{}", i64::MAX);
      test(&str, i64::MAX);
    }
    {
      let str = format!("{}", i64::MIN);
      test(&str, i64::MIN);
    }
  }
}
