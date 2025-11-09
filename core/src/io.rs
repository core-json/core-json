use core::{marker::PhantomData, fmt::Debug};

/// A no-`std` `io::Read` alternative.
///
/// While plenty of crates define their own, we avoid external dependencies by once again defining
/// our own. For those who wish to use [`embedded-io`](https://docs.rs/embedded-io), please see
/// [`core-json-embedded-io`](https://docs.rs/core-json-embedded-io).
pub trait Read<'read>: Sized + Send + Sync {
  /// The type for errors when interacting with this reader.
  type Error: Sized + Send + Sync + Copy + Debug;

  /// Read a single byte from the reader.
  #[inline(always)]
  fn read_byte(&mut self) -> Result<u8, Self::Error> {
    let mut byte = [0; 1];
    self.read_exact(&mut byte)?;
    Ok(byte[0])
  }

  /// Read into a slice from the reader.
  fn read_exact(&mut self, slice: &mut [u8]) -> Result<(), Self::Error>;
}

/// An asynchronous alternative to `Read`.
pub trait AsyncRead<'read>: Sized + Send + Sync {
  /// The type for errors when interacting with this reader.
  type Error: Sized + Send + Sync + Copy + Debug;

  /// Read a single byte from the reader.
  fn read_byte(&mut self) -> impl Send + Sync + Future<Output = Result<u8, Self::Error>>;

  /// Read into a slice from the reader.
  fn read_exact(
    &mut self,
    slice: &mut [u8],
  ) -> impl Send + Sync + Future<Output = Result<(), Self::Error>>;
}

impl<'read, R: Read<'read>> AsyncRead<'read> for R {
  type Error = <R as Read<'read>>::Error;

  #[inline(always)]
  fn read_byte(&mut self) -> impl Send + Sync + Future<Output = Result<u8, Self::Error>> {
    core::future::ready(Read::read_byte(self))
  }

  #[inline(always)]
  fn read_exact(
    &mut self,
    slice: &mut [u8],
  ) -> impl Send + Sync + Future<Output = Result<(), Self::Error>> {
    core::future::ready(Read::read_exact(self, slice))
  }
}

/// A wrapper for an `impl Read` with a one-byte buffer, enabling peeking.
///
/// This will always read at least one byte from the underlying reader.
pub(crate) struct PeekableRead<'read, R: AsyncRead<'read>> {
  buffer: u8,
  reader: R,
  _read: PhantomData<&'read ()>,
}

impl<'read, R: AsyncRead<'read>> PeekableRead<'read, R> {
  pub(crate) async fn try_from(mut reader: R) -> Result<Self, R::Error> {
    Ok(Self { buffer: reader.read_byte().await?, reader, _read: PhantomData })
  }
}

impl<'read, R: AsyncRead<'read>> PeekableRead<'read, R> {
  #[must_use]
  #[inline(always)]
  pub(crate) fn peek(&self) -> u8 {
    self.buffer
  }

  #[inline(always)]
  pub(crate) async fn read_byte(&mut self) -> Result<u8, R::Error> {
    let res = self.buffer;
    self.buffer = self.reader.read_byte().await?;
    Ok(res)
  }

  /// Read into a slice with a length which is non-zero.
  ///
  /// This will panic if the destination slice is empty, which is safe due to how we call it
  /// (entirely internal to this library).
  #[inline(always)]
  pub(crate) async fn read_exact_into_non_empty_slice(
    &mut self,
    slice: &mut [u8],
  ) -> Result<(), R::Error> {
    slice[0] = self.buffer;
    self.reader.read_exact(&mut slice[1 ..]).await?;
    // Since we've consumed the buffer, update it with the byte after the read slice
    self.buffer = self.reader.read_byte().await?;
    Ok(())
  }
}

/// An error when working with `&[u8]`.
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub enum SliceError {
  /// The slice was short by `{0}` bytes.
  Short(usize),
}

impl<'read> Read<'read> for &'read [u8] {
  type Error = SliceError;

  #[inline(always)]
  fn read_byte(&mut self) -> Result<u8, Self::Error> {
    let res = *self.first().ok_or(SliceError::Short(1))?;
    *self = &self[1 ..];
    Ok(res)
  }

  #[inline(always)]
  fn read_exact(&mut self, slice: &mut [u8]) -> Result<(), Self::Error> {
    if self.len() < slice.len() {
      Err(SliceError::Short(slice.len() - self.len()))?
    }
    slice.copy_from_slice(&self[.. slice.len()]);
    *self = &self[slice.len() ..];
    Ok(())
  }
}

impl<'read, R: Read<'read>> Read<'read> for &mut R {
  type Error = R::Error;

  #[inline(always)]
  fn read_byte(&mut self) -> Result<u8, Self::Error> {
    R::read_byte(self)
  }

  #[inline(always)]
  fn read_exact(&mut self, slice: &mut [u8]) -> Result<(), Self::Error> {
    R::read_exact(self, slice)
  }
}

/// An opaque error from a wrapped [`std::io::Read`] implementor.
#[derive(Clone, Copy, Debug)]
#[cfg(feature = "std")]
pub struct ReadError;
/// An adapter for [`std::io::Read`] implementors.
#[cfg(feature = "std")]
pub struct ReadAdapter<R: std::io::Read>(R);
#[cfg(feature = "std")]
impl<R: Send + Sync + std::io::Read> Read<'_> for ReadAdapter<R> {
  type Error = ReadError;

  #[inline(always)]
  fn read_exact(&mut self, slice: &mut [u8]) -> Result<(), Self::Error> {
    R::read_exact(&mut self.0, slice).map_err(|_| ReadError)
  }
}
