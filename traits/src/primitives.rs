use crate::{BytesLike, Stack, JsonError, Value, JsonDeserialize, JsonSerialize};

impl JsonDeserialize for i8 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value
      .to_number()?
      .i64()
      .ok_or(JsonError::TypeError)?
      .try_into()
      .map_err(|_| JsonError::TypeError)
  }
}
impl JsonDeserialize for i16 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value
      .to_number()?
      .i64()
      .ok_or(JsonError::TypeError)?
      .try_into()
      .map_err(|_| JsonError::TypeError)
  }
}
impl JsonDeserialize for i32 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value
      .to_number()?
      .i64()
      .ok_or(JsonError::TypeError)?
      .try_into()
      .map_err(|_| JsonError::TypeError)
  }
}
impl JsonDeserialize for i64 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.to_number()?.i64().ok_or(JsonError::TypeError)
  }
}

impl JsonDeserialize for u8 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value
      .to_number()?
      .i64()
      .ok_or(JsonError::TypeError)?
      .try_into()
      .map_err(|_| JsonError::TypeError)
  }
}
impl JsonDeserialize for u16 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value
      .to_number()?
      .i64()
      .ok_or(JsonError::TypeError)?
      .try_into()
      .map_err(|_| JsonError::TypeError)
  }
}
impl JsonDeserialize for u32 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value
      .to_number()?
      .i64()
      .ok_or(JsonError::TypeError)?
      .try_into()
      .map_err(|_| JsonError::TypeError)
  }
}
impl JsonDeserialize for u64 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value
      .to_number()?
      .i64()
      .ok_or(JsonError::TypeError)?
      .try_into()
      .map_err(|_| JsonError::TypeError)
  }
}

impl JsonDeserialize for bool {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.to_bool()
  }
}

struct IntInterator {
  buf: [u8; 20],
  i: usize,
  len: usize,
}
impl IntInterator {
  fn new(value: impl core::fmt::Display) -> Self {
    use core::fmt::Write;

    /// A `core::fmt::Write` which writes to a slice.
    ///
    /// We use this to achieve a non-allocating `core::fmt::Write` for primitives we know a bound
    /// for.
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

    let mut buf = [0; 20];
    let mut writer = SliceWrite(&mut buf, 0);
    write!(&mut writer, "{}", value).expect("integer primitive exceeded 20 base-10 digits");
    let len = writer.1;

    IntInterator { buf, i: 0, len }
  }
}
impl Iterator for IntInterator {
  type Item = char;
  fn next(&mut self) -> Option<Self::Item> {
    if self.i == self.len {
      None?;
    }
    let result = self.buf[self.i];
    self.i += 1;
    // This is a safe cast so long as Rust's display of an `u64` yields ASCII
    Some(result as char)
  }
}
impl JsonSerialize for i8 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    IntInterator::new(*self)
  }
}
impl JsonSerialize for i16 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    IntInterator::new(*self)
  }
}
impl JsonSerialize for i32 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    IntInterator::new(*self)
  }
}
impl JsonSerialize for i64 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    IntInterator::new(*self)
  }
}
impl JsonSerialize for u8 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    IntInterator::new(*self)
  }
}
impl JsonSerialize for u16 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    IntInterator::new(*self)
  }
}
impl JsonSerialize for u32 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    IntInterator::new(*self)
  }
}
impl JsonSerialize for u64 {
  fn serialize(&self) -> impl Iterator<Item = char> {
    IntInterator::new(*self)
  }
}

impl JsonSerialize for bool {
  fn serialize(&self) -> impl Iterator<Item = char> {
    (if *self { "true" } else { "false" }).chars()
  }
}

#[test]
fn test_int_iterator() {
  assert_eq!(JsonSerialize::serialize(&0u8).collect::<String>(), "0");
  assert_eq!(JsonSerialize::serialize(&1u8).collect::<String>(), "1");
  assert_eq!(JsonSerialize::serialize(&u64::MAX).collect::<String>(), format!("{}", u64::MAX));
  assert_eq!(JsonSerialize::serialize(&i64::MAX).collect::<String>(), format!("{}", i64::MAX));
  assert_eq!(JsonSerialize::serialize(&i64::MIN).collect::<String>(), format!("{}", i64::MIN));
}
