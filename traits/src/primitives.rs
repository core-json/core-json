use crate::{BytesLike, Stack, JsonError, Value, JsonDeserialize};

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

impl JsonDeserialize for f64 {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.as_f64()
  }
}

impl JsonDeserialize for bool {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.as_bool()
  }
}
