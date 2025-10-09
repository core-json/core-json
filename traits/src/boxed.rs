use alloc::boxed::Box;

use crate::{BytesLike, Stack, JsonError, Value, JsonDeserialize, JsonStructure};

impl<T: JsonDeserialize> JsonDeserialize for Box<T> {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    T::deserialize(value).map(Box::new)
  }
}

impl<T: JsonStructure> JsonStructure for Box<T> {}
