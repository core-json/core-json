#[cfg(feature = "alloc")]
use alloc::{vec, vec::Vec};

use crate::{Read, Stack, JsonError, Value, JsonDeserialize, JsonStructure, JsonSerialize};

impl<T: Default + JsonDeserialize, const N: usize> JsonDeserialize for [T; N] {
  fn deserialize<'read, 'parent, B: Read<'read>, S: Stack>(
    value: Value<'read, 'parent, B, S>,
  ) -> Result<Self, JsonError<'read, B, S>> {
    let mut res: Self = core::array::from_fn(|_| Default::default());
    let mut iter = value.iterate()?;
    let mut i = 0;
    while let Some(item) = iter.next() {
      if i == N {
        Err(JsonError::TypeError)?;
      }
      res[i] = T::deserialize(item?)?;
      i += 1;
    }
    if i != N {
      Err(JsonError::TypeError)?;
    }
    Ok(res)
  }
}
impl<T: Default + JsonDeserialize, const N: usize> JsonStructure for [T; N] {}

#[cfg(feature = "alloc")]
impl<T: JsonDeserialize> JsonDeserialize for Vec<T> {
  fn deserialize<'read, 'parent, B: Read<'read>, S: Stack>(
    value: Value<'read, 'parent, B, S>,
  ) -> Result<Self, JsonError<'read, B, S>> {
    let mut res = vec![];
    let mut iter = value.iterate()?;
    while let Some(item) = iter.next() {
      res.push(T::deserialize(item?)?);
    }
    Ok(res)
  }
}
#[cfg(feature = "alloc")]
impl<T: JsonDeserialize> JsonStructure for Vec<T> {}

impl<T: JsonSerialize> JsonSerialize for [T] {
  fn serialize(&self) -> impl Iterator<Item = char> {
    core::iter::once('[')
      .chain(
        self
          .iter()
          .take(self.len().saturating_sub(1))
          .flat_map(|elem| elem.serialize().chain(core::iter::once(','))),
      )
      .chain(self.get(self.len().saturating_sub(1)).into_iter().flat_map(JsonSerialize::serialize))
      .chain(core::iter::once(']'))
  }
}
impl<T: JsonSerialize, const N: usize> JsonSerialize for [T; N] {
  fn serialize(&self) -> impl Iterator<Item = char> {
    self.as_slice().serialize()
  }
}

#[cfg(feature = "alloc")]
impl<T: JsonSerialize> JsonSerialize for Vec<T> {
  fn serialize(&self) -> impl Iterator<Item = char> {
    self.as_slice().serialize()
  }
}
