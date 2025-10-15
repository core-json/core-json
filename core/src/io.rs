//! IO primitives around bytes.

use core::fmt::Debug;

/// An item which is like a `&[u8]`.
///
/// This should be a reference to a buffer for which references are cheap to work with.
#[allow(clippy::len_without_is_empty)]
pub trait BytesLike<'bytes>: Sized + Debug {
  /// The type for errors when interacting with these bytes.
  type Error: Sized + Copy + Debug;

  /// Peak at a byte.
  fn peek(&self, i: usize) -> Result<u8, Self::Error>;

  /// Read a fixed amount of bytes from the container.
  ///
  /// This MUST return `Ok(slice)` where `slice` has the expected length or `Err(_)`.
  fn read_bytes(&mut self, bytes: usize) -> Result<Self, Self::Error>;

  /// Read a fixed amount of bytes from the container into a slice.
  /*
    We _could_ provide this method around `read_bytes` but it'd be a very inefficient
    default implementation. It's best to require callers provide the implementation.
  */
  fn read_into_slice(&mut self, slice: &mut [u8]) -> Result<(), Self::Error>;

  /// Read a byte from the container.
  #[inline(always)]
  fn read_byte(&mut self) -> Result<u8, Self::Error> {
    let mut buf = [0; 1];
    self.read_into_slice(&mut buf)?;
    Ok(buf[0])
  }

  /// Advance the container by a certain amount of bytes.
  #[inline(always)]
  fn advance(&mut self, bytes: usize) -> Result<(), Self::Error> {
    self.read_bytes(bytes).map(|_| ())
  }
}

/// An error when working with `&[u8]`.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum SliceError {
  /// The blob was short, as discovered when trying to read `{0}` bytes.
  Short(usize),
}

impl<'bytes> BytesLike<'bytes> for &'bytes [u8] {
  type Error = SliceError;

  #[inline(always)]
  fn peek(&self, i: usize) -> Result<u8, Self::Error> {
    self.get(i).ok_or_else(|| SliceError::Short((i - self.len()).saturating_add(1))).copied()
  }

  #[inline(always)]
  fn read_bytes(&mut self, bytes: usize) -> Result<Self, Self::Error> {
    if self.len() < bytes {
      Err(SliceError::Short(bytes))?;
    }
    let res = &self[.. bytes];
    *self = &self[bytes ..];
    Ok(res)
  }

  #[inline(always)]
  fn read_into_slice(&mut self, slice: &mut [u8]) -> Result<(), Self::Error> {
    slice.copy_from_slice(self.read_bytes(slice.len())?);
    Ok(())
  }
}
