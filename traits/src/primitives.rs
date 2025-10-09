use crate::{BytesLike, Stack, JsonError, Value, JsonDeserialize, JsonSerialize};

impl JsonDeserialize for i8 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.as_i64()?.try_into().map_err(|_| JsonError::TypeError)
  }
}
impl JsonDeserialize for i16 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.as_i64()?.try_into().map_err(|_| JsonError::TypeError)
  }
}
impl JsonDeserialize for i32 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.as_i64()?.try_into().map_err(|_| JsonError::TypeError)
  }
}
impl JsonDeserialize for i64 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.as_i64()
  }
}

impl JsonDeserialize for u8 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.as_i64()?.try_into().map_err(|_| JsonError::TypeError)
  }
}
impl JsonDeserialize for u16 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.as_i64()?.try_into().map_err(|_| JsonError::TypeError)
  }
}
impl JsonDeserialize for u32 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.as_i64()?.try_into().map_err(|_| JsonError::TypeError)
  }
}
impl JsonDeserialize for u64 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.as_i64()?.try_into().map_err(|_| JsonError::TypeError)
  }
}

impl JsonDeserialize for bool {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.as_bool()
  }
}

struct IntInterator {
  value: u64,
  digits: usize,
  i: usize,
}
impl IntInterator {
  fn new(value: u64) -> Self {
    let digits = {
      let mut digits = 0;
      let mut value = value;
      while value > 0 {
        digits += 1;
        value /= 10;
      }
      digits
    };
    IntInterator { value, digits, i: 0 }
  }
}
impl Iterator for IntInterator {
  type Item = char;
  fn next(&mut self) -> Option<Self::Item> {
    if self.i == self.digits {
      None?;
    }

    let mut value = self.value;
    // There will be at least one digit, as `self.i` starts at `0`
    for _ in self.i .. (self.digits - 1) {
      value /= 10;
    }
    self.i += 1;

    // Safe to cast as this will be `< 10`, which fits within a `u8`
    let char_offset = (value % 10) as u8;
    // Safe to cast as this will be `'0' ..= '9'`
    Some((b'0' + char_offset) as char)
  }
}
fn u64_to_str(value: impl Into<u64>) -> impl Iterator<Item = char> {
  let value = value.into();
  let zero = value == 0;
  zero.then(|| core::iter::once('0')).into_iter().flatten().chain(IntInterator::new(value))
}
pub(crate) fn i64_to_str(value: impl Into<i64>) -> impl Iterator<Item = char> {
  let value: i64 = value.into();
  value
    .is_negative()
    .then(|| core::iter::once('-'))
    .into_iter()
    .flatten()
    .chain(u64_to_str(value.unsigned_abs()))
}
impl JsonSerialize for i8 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    i64_to_str(*self)
  }
}
impl JsonSerialize for i16 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    i64_to_str(*self)
  }
}
impl JsonSerialize for i32 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    i64_to_str(*self)
  }
}
impl JsonSerialize for i64 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    i64_to_str(*self)
  }
}
impl JsonSerialize for u8 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    u64_to_str(*self)
  }
}
impl JsonSerialize for u16 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    u64_to_str(*self)
  }
}
impl JsonSerialize for u32 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    u64_to_str(*self)
  }
}
impl JsonSerialize for u64 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    u64_to_str(*self)
  }
}

impl JsonSerialize for bool {
  fn serialize(&self) -> impl Iterator<Item = char> {
    (if *self { "true" } else { "false" }).chars()
  }
}
