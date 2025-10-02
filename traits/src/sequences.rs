use crate::{BytesLike, Stack, JsonError, Value, JsonDeserialize};

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

#[cfg(feature = "alloc")]
use alloc::{vec, vec::Vec};
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
