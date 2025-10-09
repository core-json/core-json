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
      FpCategory::Nan | FpCategory::Infinite | FpCategory::Subnormal => Err(class)?,
      FpCategory::Zero | FpCategory::Normal => {}
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

struct WholeFloatInterator {
  value: f64,
  digits: usize,
  i: usize,
}
impl WholeFloatInterator {
  fn new(value: f64) -> Self {
    let digits = {
      let mut digits = 0;
      let mut value = value;
      while value >= 1f64 {
        digits += 1;
        value /= 10f64;
      }
      digits
    };

    WholeFloatInterator { value, digits, i: 0 }
  }
}
impl Iterator for WholeFloatInterator {
  type Item = char;
  fn next(&mut self) -> Option<Self::Item> {
    if self.i == self.digits {
      None?;
    }

    let mut value = self.value;
    // There will be at least one digit, as `self.i` starts at `0`
    for _ in self.i .. (self.digits - 1) {
      value /= 10f64;
    }
    self.i += 1;

    // Safe as not `NaN`, not `inf`, and representable in `u8`
    // Rust 1.90 ships `f64::trunc` on `std` but it isn't available on `core` :/
    let char_offset = unsafe { (value % 10f64).to_int_unchecked::<u8>() };
    // Safe to cast as this will be `'0' ..= '9'`
    Some((b'0' + char_offset) as char)
  }
}

struct DecimalIteratorInner {
  value: f64,
}
impl Iterator for DecimalIteratorInner {
  type Item = char;
  fn next(&mut self) -> Option<Self::Item> {
    self.value = (self.value * 10f64) % 10f64;
    // Safe as not `NaN`, not `inf`, and representable in `u8`
    let char_offset = unsafe { self.value.to_int_unchecked::<u8>() };
    // Safe to cast as this will be `'0' ..= '9'`
    Some((b'0' + char_offset) as char)
  }
}

struct DecimalIterator {
  first: bool,
  remaining: usize,
  value: f64,
}
impl Iterator for DecimalIterator {
  type Item = char;
  fn next(&mut self) -> Option<Self::Item> {
    // Stop if there are no remaining digits
    if self.remaining == 0 {
      None?;
    }
    // Stop early if the remaining digits are zeroes
    if (DecimalIteratorInner { value: self.value }).take(self.remaining).all(|digit| digit == '0') {
      self.remaining = 0;
      None?;
    }

    // If this is the first iteration, yield the decimal point
    if self.first {
      self.first = false;
      return Some('.');
    }

    // Step forth the iterator
    let mut iter = DecimalIteratorInner { value: self.value };
    let result = iter.next();
    self.remaining -= 1;
    self.value = iter.value;
    result
  }
}

impl JsonSerialize for JsonF64 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    let sign = self.0.is_sign_negative().then(|| core::iter::once('-')).into_iter().flatten();

    let mut value = self.0.abs();
    let mut value_has_integer_component = value >= 1f64;
    let mut exponent = 0i32;

    // If this is a non-zero float without an integer component, apply a negative exponent
    if value != 0f64 {
      while (!value_has_integer_component) && (exponent > f64::MIN_EXP) {
        value *= 10f64;
        value_has_integer_component = value >= 1f64;
        exponent -= 1;
      }
    }
    let exponent = (exponent != 0)
      .then(|| core::iter::once('e').chain(crate::primitives::i64_to_str(exponent)))
      .into_iter()
      .flatten();

    sign
      .chain((!value_has_integer_component).then(|| core::iter::once('0')).into_iter().flatten())
      .chain(WholeFloatInterator::new(value))
      .chain(DecimalIterator {
        first: true,
        remaining: f64::DIGITS as usize,
        value: (value % 10f64),
      })
      .chain(exponent)
  }
}
