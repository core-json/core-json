use core::num::FpCategory;
use crate::{BytesLike, Stack, JsonError, Value, JsonDeserialize, JsonSerialize};

impl JsonDeserialize for f64 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.as_f64()
  }
}

/// A JSON-compatible `f64`.
///
/// JSON does not support representing `NaN`, `inf`, but rather only well-defined values. This
/// ensures the `f64` is representable within JSON. We additionally limit to normal `f64`s to
/// achieve certain bounds.
#[derive(Clone, Copy, Default, Debug)]
pub struct JsonF64(f64);

impl TryFrom<f64> for JsonF64 {
  type Error = FpCategory;
  fn try_from(value: f64) -> Result<Self, Self::Error> {
    let class = value.classify();
    match class {
      FpCategory::Nan | FpCategory::Infinite => Err(class)?,
      FpCategory::Zero | FpCategory::Normal | FpCategory::Subnormal => {}
    }
    Ok(Self(value))
  }
}

impl From<JsonF64> for f64 {
  fn from(value: JsonF64) -> f64 {
    value.0
  }
}

impl JsonDeserialize for JsonF64 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    JsonF64::try_from(f64::deserialize(value)?).map_err(|_| JsonError::TypeError)
  }
}

#[cfg(not(feature = "ryu"))]
mod serialize {
  use core::fmt::Write;
  use super::*;

  // A non-allocating buffer for `core::fmt::Write` which truncates after `f64::DIGITS` digits
  struct Buffer {
    // sign, significant digits, decimal point
    // For more information, please see `core_json::Value::as_f64`
    bytes: [u8; 1 + (f64::DIGITS as usize) + 1],
    i: usize,
    digits: usize,
    before_decimal: bool,
    omitted_decimal: bool,
    before_exponent: bool,
    negative_exponent: bool,
    exponent: i16,
    exponent_correction: i16,
  }
  impl Write for Buffer {
    fn write_str(&mut self, value: &str) -> core::fmt::Result {
      for char in value.chars() {
        if self.before_exponent {
          if char.is_ascii_digit() {
            // Stop writing digits after we have all the significant digits which can be definitely
            // handled without deviations when converting to/from a decimal string
            if self.digits == (f64::DIGITS as usize) {
              // If we're truncating prior to the decimal, mark a correction to the exponent
              if self.before_decimal {
                self.exponent_correction += 1;
              }
              continue;
            }

            // If this is a digit when we've omitted the decimal, correct the exponent
            if self.omitted_decimal {
              self.exponent_correction -= 1;
            }

            // Skip leading zeroes
            if (self.digits == 0) && (char == '0') {
              continue;
            }

            // Include the digit
            self.digits += 1;
          }

          if char == '.' {
            self.before_decimal = false;
            // Don't include a decimal if we have yet to include the value itself
            if self.digits == 0 {
              self.omitted_decimal = true;
              continue;
            }
            // Don't include a decimal point if we won't include the following digits
            if self.digits == (f64::DIGITS as usize) {
              continue;
            }
          }
        }

        if matches!(char, 'e' | 'E') {
          self.before_exponent = false;
          continue;
        }

        if self.before_exponent {
          // Safe as `f64`'s display will only contain ASCII characters
          self.bytes[self.i] = char as u8;
          self.i += 1;
        } else {
          // Assumes exponents will be represented as `[ plus / minus ] *DIGIT` and fit within an
          // `i16` (which they should as `f64::MAX_10_EXP = 308`)
          if char == '+' {
            continue;
          }
          if char == '-' {
            self.negative_exponent = true;
            continue;
          }
          self.exponent *= 10;
          self.exponent += i16::from((char as u8) - b'0');
        }
      }
      Ok(())
    }
  }

  impl JsonSerialize for JsonF64 {
    /// This will only serialize the `f64::DIGITS` most significant digits.
    fn serialize(&self) -> impl Iterator<Item = char> {
      let mut buffer = Buffer {
        bytes: [b'0'; _],
        i: 0,
        digits: 0,
        before_decimal: true,
        omitted_decimal: false,
        before_exponent: true,
        negative_exponent: false,
        exponent: 0,
        exponent_correction: 0,
      };
      write!(&mut buffer, "{:?}", self.0).expect("infallible buffer raised an error");

      // If Rust gave us `1.e` (invalid), decrement the buffer index to remove the '.'
      if buffer.i.checked_sub(1).map(|i| buffer.bytes[i]) == Some(b'.') {
        buffer.i -= 1;
      }

      let exponent = {
        let exponent = (if buffer.negative_exponent { -buffer.exponent } else { buffer.exponent }) +
          buffer.exponent_correction;

        ((buffer.i != 0) && (exponent != 0))
          .then(|| core::iter::once('e').chain(crate::primitives::i64_to_str(exponent)))
          .into_iter()
          .flatten()
      };
      // Safe as all of the written-to values will be written-to with ASCII characters
      buffer.bytes.into_iter().take(buffer.i.max(1)).map(|b| b as char).chain(exponent)
    }
  }

  #[test]
  fn f64_serialize() {
    use core::str::FromStr;
    #[allow(clippy::float_cmp)]
    let test = |value: f64, expected| {
      assert_eq!(
        f64::from_str(&JsonF64::try_from(value).unwrap().serialize().collect::<String>()).unwrap(),
        f64::from_str(expected).unwrap()
      );
    };
    test(0.0, "0");
    test(0.1, "1e-1");
    test(0.01, "1e-2");
    test(0.001, "1e-3");
    test(0.0012, "12e-4");
    test(0.12345678910111213, "123456789101112e-15");
    test(0.012345678910111213, "123456789101112e-16");
    test(12345678910111213.0, "123456789101112e2");
    test(12345678910111213.123, "123456789101112e2");
    test(123456789.101112, "123456789.101112");
    test(123456789.10111213, "123456789.101112");
    test(-1.0, "-1");
    test(f64::MIN, "-179769313486231e294");
    test(f64::MAX, "179769313486231e294");
    test(f64::EPSILON, "222044604925031e-30");
  }
}

#[cfg(feature = "ryu")]
mod serialize {
  use super::*;

  impl JsonSerialize for JsonF64 {
    fn serialize(&self) -> impl Iterator<Item = char> {
      let mut buffer = ryu::Buffer::new();
      // Safe as `JsonF64` ensures this isn't `NaN`, `inf`
      let result = buffer.format_finite(self.0).as_bytes();
      /*
        `ryu` yields us a string slice when we need an owned value to iterate, unfortunately, so
        we copy the yielded string (a reference to the Buffer) into our own buffer (of equivalent
        size)
      */
      let mut owned = [0; core::mem::size_of::<ryu::Buffer>()];
      owned[.. result.len()].copy_from_slice(result);
      // Safe to cast to char as `ryu` yields human-readable ASCII characters
      owned.into_iter().take(result.len()).map(|byte| byte as char)
    }
  }
}
