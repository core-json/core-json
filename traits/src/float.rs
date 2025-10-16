use core::num::FpCategory;
use crate::{Read, Stack, JsonError, Value, JsonDeserialize, JsonSerialize};

impl JsonDeserialize for f64 {
  fn deserialize<'read, 'parent, B: Read<'read>, S: Stack>(
    value: Value<'read, 'parent, B, S>,
  ) -> Result<Self, JsonError<'read, B, S>> {
    value.to_number()?.f64().ok_or(JsonError::TypeError)
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
  fn deserialize<'read, 'parent, B: Read<'read>, S: Stack>(
    value: Value<'read, 'parent, B, S>,
  ) -> Result<Self, JsonError<'read, B, S>> {
    JsonF64::try_from(f64::deserialize(value)?).map_err(|_| JsonError::TypeError)
  }
}

#[cfg(not(feature = "ryu"))]
mod serialize {
  use core::fmt::Write;
  use crate::NumberSink;
  use super::*;

  impl JsonSerialize for JsonF64 {
    /// This will only serialize the `f64::DIGITS` most significant digits.
    fn serialize(&self) -> impl Iterator<Item = char> {
      let mut sink = NumberSink::new();
      write!(&mut sink, "{}", self.0).expect("infallible `NumberSink` raised an error");
      let (buf, len) = sink.imprecise_str().expect("`NumberSink` couldn't sink a `f64` from Rust");
      // Safe as all of the written-to values will be written-to with ASCII characters
      buf.into_iter().take(len).map(|b| b as char)
    }
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
