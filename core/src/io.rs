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

  /// Read a fixed amount of bytes from the reader into a slice.
  fn read_into_slice(&mut self, slice: &mut [u8]) -> Result<(), Self::Error>;
}

/// A wrapper for an `impl Read` with a one-byte buffer, enabling peeking.
pub(crate) struct PeekableRead<'read, R: Read<'read>> {
  buffer: Option<u8>,
  reader: R,
  _read: PhantomData<&'read ()>,
}

impl<'read, R: Read<'read>> From<R> for PeekableRead<'read, R> {
  fn from(reader: R) -> Self {
    Self { buffer: None, reader, _read: PhantomData }
  }
}

impl<'read, R: Read<'read>> PeekableRead<'read, R> {
  #[inline(always)]
  pub(crate) fn peek(&mut self) -> Result<u8, R::Error> {
    Ok(match self.buffer {
      Some(byte) => byte,
      None => {
        let mut buffer = [0u8; 1];
        self.reader.read_into_slice(&mut buffer)?;
        self.buffer = Some(buffer[0]);
        buffer[0]
      }
    })
  }

  #[inline(always)]
  pub(crate) fn read_into_slice(&mut self, slice: &mut [u8]) -> Result<(), R::Error> {
    if slice.is_empty() {
      return Ok(());
    }
    let i = if let Some(byte) = self.buffer.take() {
      slice[0] = byte;
      1
    } else {
      0
    };
    self.reader.read_into_slice(&mut slice[i ..])
  }

  #[inline(always)]
  pub(crate) fn read_byte(&mut self) -> Result<u8, R::Error> {
    let mut buf = [0; 1];
    self.read_into_slice(&mut buf)?;
    Ok(buf[0])
  }
}

/// An error when working with `&[u8]`.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum SliceError {
  /// The blob was short, as discovered when trying to read `{0}` bytes.
  Short(usize),
}

impl<'read> Read<'read> for &'read [u8] {
  type Error = SliceError;

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

impl<'read, R: Read<'read>> Read<'read> for &mut R {
  type Error = R::Error;

  #[inline(always)]
  fn read_into_slice(&mut self, slice: &mut [u8]) -> Result<(), Self::Error> {
    R::read_into_slice(self, slice)
  }
}
