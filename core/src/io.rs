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

  /// Read a fixed amount of bytes from the container into a slice.
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
    for _ in 0 .. bytes {
      self.read_byte()?;
    }
    Ok(())
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
  fn read_into_slice(&mut self, slice: &mut [u8]) -> Result<(), Self::Error> {
    if self.len() < slice.len() {
      Err(SliceError::Short(slice.len()))?;
    }
    slice.copy_from_slice(&self[.. slice.len()]);
    *self = &self[slice.len() ..];
    Ok(())
  }
}
