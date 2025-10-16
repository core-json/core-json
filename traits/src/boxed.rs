use alloc::boxed::Box;

use crate::{Read, Stack, JsonError, Value, JsonDeserialize, JsonStructure};

impl<T: JsonDeserialize> JsonDeserialize for Box<T> {
  fn deserialize<'read, 'parent, B: Read<'read>, S: Stack>(
    value: Value<'read, 'parent, B, S>,
  ) -> Result<Self, JsonError<'read, B, S>> {
    T::deserialize(value).map(Box::new)
  }
}

impl<T: JsonStructure> JsonStructure for Box<T> {}
