//! IO primitives around bytes.

use core::{marker::PhantomData, fmt::Debug};

/// An item which is like a `&[u8]`.
///
/// This should be a reference to a buffer for which references are cheap to work with.
#[allow(clippy::len_without_is_empty)]
pub trait BytesLike<'bytes>: Sized + Debug {
  /// The type for errors when interacting with these bytes.
  type Error: Sized + Copy + Debug;

  /// The type representing the length of a _read_ `BytesLike`, if a `BytesLike` does not
  /// inherently know its length.
  ///
  /// This should be `usize` or `()`.
  type ExternallyTrackedLength: Sized + Copy + Debug;

  /// The length of these bytes.
  fn len(&self, len: Self::ExternallyTrackedLength) -> usize;

  /// Peak at a byte.
  fn peek(&self, i: usize) -> Result<u8, Self::Error>;

  /// Read a fixed amount of bytes from the container.
  ///
  /// This MUST return `Ok((len, slice))` where `slice` is the expected length or `Err(_)`.
  fn read_bytes(
    &mut self,
    bytes: usize,
  ) -> Result<(Self::ExternallyTrackedLength, Self), Self::Error>;

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

  type ExternallyTrackedLength = ();

  #[inline(always)]
  fn len(&self, (): ()) -> usize {
    <[u8]>::len(self)
  }

  #[inline(always)]
  fn peek(&self, i: usize) -> Result<u8, Self::Error> {
    self.get(i).ok_or_else(|| SliceError::Short((i - <[u8]>::len(self)).saturating_add(1))).copied()
  }

  #[inline(always)]
  fn read_bytes(
    &mut self,
    bytes: usize,
  ) -> Result<(Self::ExternallyTrackedLength, Self), Self::Error> {
    if <[u8]>::len(self) < bytes {
      Err(SliceError::Short(bytes))?;
    }
    let res = &self[.. bytes];
    *self = &self[bytes ..];
    Ok(((), res))
  }

  #[inline(always)]
  fn read_into_slice(&mut self, slice: &mut [u8]) -> Result<(), Self::Error> {
    slice.copy_from_slice(self.read_bytes(slice.len())?.1);
    Ok(())
  }
}

/// A collection of bytes with an associated length.
///
/// This avoids defining `BytesLike::len` which lets us relax the requirement `BytesLike` knows its
/// length before it has reached its end.
#[derive(Debug)]
pub(crate) struct String<'bytes, B: BytesLike<'bytes>> {
  pub(crate) len: B::ExternallyTrackedLength,
  pub(crate) bytes: B,
  pub(crate) _encoding: PhantomData<&'bytes ()>,
}

impl<'bytes, B: BytesLike<'bytes>> String<'bytes, B> {
  /// The length of this string.
  #[inline(always)]
  pub(crate) fn len(&self) -> usize {
    self.bytes.len(self.len)
  }

  /// Consume this into its underlying bytes.
  #[inline(always)]
  pub(crate) fn consume(self) -> B {
    self.bytes
  }
}
