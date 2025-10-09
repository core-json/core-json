#[cfg(feature = "alloc")]
use alloc::{vec, vec::Vec};

use crate::{BytesLike, Stack, JsonError, Value, JsonDeserialize, JsonStructure, JsonSerialize};

impl<T: 'static + Default + JsonDeserialize, const N: usize> JsonDeserialize for [T; N] {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
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
impl<T: 'static + Default + JsonDeserialize, const N: usize> JsonStructure for [T; N] {}

#[cfg(feature = "alloc")]
impl<T: 'static + JsonDeserialize> JsonDeserialize for Vec<T> {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    let mut res = vec![];
    let mut iter = value.iterate()?;
    while let Some(item) = iter.next() {
      res.push(T::deserialize(item?)?);
    }
    Ok(res)
  }
}
#[cfg(feature = "alloc")]
impl<T: 'static + JsonDeserialize> JsonStructure for Vec<T> {}

impl<T: JsonSerialize> JsonSerialize for [T] {
  fn serialize(&self) -> impl Iterator<Item = char> {
    core::iter::once('[')
      .chain(self.iter().enumerate().flat_map(|(i, elem)| {
        let last_item = (i + 1) == self.len();
        elem.serialize().chain((!last_item).then(|| core::iter::once(',')).into_iter().flatten())
      }))
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
