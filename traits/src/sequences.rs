use alloc::{vec, vec::Vec};

use crate::{BytesLike, EpeeError, EpeeEntry, EpeeDecode};

impl<T: 'static + EpeeDecode> EpeeDecode for Vec<T> {
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError> {
    if core::any::TypeId::of::<T>() == core::any::TypeId::of::<u8>() {
      let mut str = entry.to_str()?;
      let mut res = vec![0; str.len()];
      str.read_into_slice(&mut res)?;

      // We know these types are equivalent, making this an effective NOP and safe
      let res = unsafe { core::mem::transmute::<Vec<u8>, Vec<T>>(res) };

      return Ok(res);
    }

    let mut res = vec![];
    let mut iter = entry.iterate()?;
    while let Some(item) = iter.next() {
      res.push(T::decode(item?)?);
    }
    Ok(res)
  }
}

impl<T: 'static + EpeeDecode, const N: usize> EpeeDecode for [T; N] {
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError> {
    if core::any::TypeId::of::<T>() == core::any::TypeId::of::<u8>() {
      let mut str = entry.to_fixed_len_str(N)?;
      let mut original = [0; N];
      str.read_into_slice(&mut original)?;

      /*
        We know these types are equivalent, making this an effective NOP and safe. Unlike with
        `Vec`, we can't use `core::mem::transmute` as the size of the container is dependent on the
        size of the type, and Rust doesn't know
        `core::mem::size_of::<T>() == core::mem::size_of::<u8>()` in this branch. Accordingly, we
        manually implement a bitwise copy from a pointer to the `[u8; N]`. This is fine as
        `u8: Copy`.
      */
      let casted =
        unsafe { core::ptr::read_unaligned(core::ptr::addr_of!(original) as *const [T; N]) };

      return Ok(casted);
    }

    Vec::<T>::decode(entry)?.try_into().map_err(|_| EpeeError::TypeError)
  }
}
