use core::{marker::PhantomData, fmt::Debug};

/// A no-`std` `io::Read` alternative.
///
/// While plenty of crates define their own, we avoid external dependencies by once again defining
/// our own. For those who wish to use [`embedded-io`](https://docs.rs/embedded-io), please see
/// [`core-json-embedded-io`](https://docs.rs/core-json-embedded-io).
#[allow(clippy::len_without_is_empty)]
pub trait Read<'read>: Sized + Debug {
  /// The type for errors when interacting with this reader.
  type Error: Sized + Copy + Debug;

  /// Read a single byte from the reader.
  fn read_byte(&mut self) -> Result<u8, Self::Error>;
}

/// A wrapper for an `impl Read` with a one-byte buffer, enabling peeking.
///
/// This will always read at least one byte from the underlying reader.
pub(crate) struct PeekableRead<'read, R: Read<'read>> {
  buffer: u8,
  reader: R,
  _read: PhantomData<&'read ()>,
}

impl<'read, R: Read<'read>> PeekableRead<'read, R> {
  pub(crate) fn try_from(mut reader: R) -> Result<Self, R::Error> {
    Ok(Self { buffer: reader.read_byte()?, reader, _read: PhantomData })
  }
}

impl<'read, R: Read<'read>> PeekableRead<'read, R> {
  #[must_use]
  #[inline(always)]
  pub(crate) fn peek(&self) -> u8 {
    self.buffer
  }

  #[inline(always)]
  pub(crate) fn read_byte(&mut self) -> Result<u8, R::Error> {
    let res = self.buffer;
    self.buffer = self.reader.read_byte()?;
    Ok(res)
  }
}

/// An error when working with `&[u8]`.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum SliceError {
  /// The slice was empty.
  Empty,
}

impl<'read> Read<'read> for &'read [u8] {
  type Error = SliceError;

  #[inline(always)]
  fn read_byte(&mut self) -> Result<u8, Self::Error> {
    let res = *self.first().ok_or(SliceError::Empty)?;
    *self = &self[1 ..];
    Ok(res)
  }
}

impl<'read, R: Read<'read>> Read<'read> for &mut R {
  type Error = R::Error;

  #[inline(always)]
  fn read_byte(&mut self) -> Result<u8, Self::Error> {
    R::read_byte(self)
  }
}
