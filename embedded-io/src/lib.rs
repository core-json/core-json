#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![cfg_attr(not(test), no_std)]

use core::{fmt, cell::RefCell};
use core_json::BytesLike;
use embedded_io::{ErrorType, ReadExactError, Read, SeekFrom, Seek};

/// An error from an adapter.
pub enum Error<E: ErrorType<Error: Copy>> {
  /// An internal error occurred.
  InternalError,
  /// The adapter was bounded and the bound has been reached.
  Bounded,
  /// The underlying reader raised an error.
  Reader(E::Error),
  /// `read_exact` raised an error.
  ReadExact(ReadExactError<E::Error>),
}
impl<E: ErrorType<Error: Copy>> Clone for Error<E> {
  fn clone(&self) -> Self {
    *self
  }
}
impl<E: ErrorType<Error: Copy>> Copy for Error<E> {}
impl<E: ErrorType<Error: Copy>> fmt::Debug for Error<E> {
  fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
    match self {
      Self::InternalError => fmt.debug_struct("Error::InternalError").finish(),
      Self::Bounded => fmt.debug_struct("Error::Bounded").finish(),
      Self::Reader(e) => fmt.debug_struct("Error::Reader").field("0", &e).finish(),
      Self::ReadExact(e) => fmt.debug_struct("Error::ReadExact").field("0", &e).finish(),
    }
  }
}

/// An adapter from [`embedded_io::Read`] to [`core_json::BytesLike`].
///
/// [`SeekAdapter`] SHOULD be preferred as it'll call `Clone` less frequently.
pub struct ReadAdapter<R: Clone + Read<Error: Copy>> {
  reader: R,
  bound: Option<usize>,
}
impl<R: Clone + Read<Error: Copy>> fmt::Debug for ReadAdapter<R> {
  fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
    fmt.debug_struct("ReadAdapter").finish_non_exhaustive()
  }
}

impl<R: Clone + Read<Error: Copy>> From<R> for ReadAdapter<R> {
  fn from(reader: R) -> Self {
    Self { reader, bound: None }
  }
}

impl<R: Clone + Read<Error: Copy>> BytesLike<'static> for ReadAdapter<R> {
  type Error = Error<R>;

  fn peek(&self, mut i: usize) -> Result<u8, Self::Error> {
    let mut peek = self.reader.clone();

    // Seek ahead
    let mut buf = [0; 8];
    while i > 0 {
      let this_iter = i.min(8);
      peek.read_exact(&mut buf[.. this_iter]).map_err(Error::ReadExact)?;
      i -= this_iter;
    }

    // Peek
    peek.read_exact(&mut buf[.. 1]).map_err(Error::ReadExact)?;
    Ok(buf[0])
  }

  fn read_into_slice(&mut self, slice: &mut [u8]) -> Result<(), Self::Error> {
    if let Some(bound) = self.bound.as_mut() {
      *bound = bound.checked_sub(slice.len()).ok_or(Error::Bounded)?;
    }
    self.reader.read_exact(slice).map_err(Error::ReadExact)
  }
}

/// An adapter from [`embedded_io::Seek`] to [`core_json::BytesLike`].
///
/// This will `Clone` the underlying reader less frequently than `ReadAdapter` due to being able
/// to use `Seek` to implement `peek`.
pub struct SeekAdapter<R: Clone + Read<Error: Copy> + Seek> {
  // This is used for interior mutability within `peek`
  reader: RefCell<R>,
  bound: Option<usize>,
}
impl<R: Clone + Read<Error: Copy> + Seek> fmt::Debug for SeekAdapter<R> {
  fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
    fmt.debug_struct("SeekAdapter").finish_non_exhaustive()
  }
}

impl<R: Clone + Read<Error: Copy> + Seek> From<R> for SeekAdapter<R> {
  fn from(reader: R) -> Self {
    Self { reader: RefCell::new(reader), bound: None }
  }
}

impl<R: Clone + Read<Error: Copy> + Seek> BytesLike<'static> for SeekAdapter<R> {
  type Error = Error<R>;

  fn peek(&self, i: usize) -> Result<u8, Self::Error> {
    let i = i64::try_from(i).map_err(|_| Error::InternalError)?;

    let mut reader = self.reader.borrow_mut();

    // Seek ahead
    reader.seek_relative(i).map_err(Error::Reader)?;

    let mut buf = [0];
    reader.read_exact(&mut buf).map_err(Error::ReadExact)?;

    // Restore to the original position
    reader.seek_relative((-i) - 1).map_err(Error::Reader)?;

    Ok(buf[0])
  }

  fn read_into_slice(&mut self, slice: &mut [u8]) -> Result<(), Self::Error> {
    if let Some(bound) = self.bound.as_mut() {
      *bound = bound.checked_sub(slice.len()).ok_or(Error::Bounded)?;
    }
    self.reader.get_mut().read_exact(slice).map_err(Error::ReadExact)
  }
}

/// A wrapper which allows cheaply and safely cloning implementors of `Seek`.
///
/// This defers ownership of the reader for solely ownership of a reference to it. Each instance
/// individually tracks their position within the reader, resetting the seek head on each
/// invocation (as required for each instance to maintain a consistent view).
pub struct ClonableSeek<'reader, R: Read<Error: Copy> + Seek> {
  reader: &'reader RefCell<R>,
  pos: u64,
}

impl<'reader, R: Read<Error: Copy> + Seek> TryFrom<&'reader RefCell<R>>
  for ClonableSeek<'reader, R>
{
  type Error = R::Error;
  fn try_from(reader: &'reader RefCell<R>) -> Result<Self, R::Error> {
    let pos = reader.borrow_mut().stream_position()?;
    Ok(Self { reader, pos })
  }
}

impl<'reader, R: Read<Error: Copy> + Seek> Clone for ClonableSeek<'reader, R> {
  fn clone(&self) -> Self {
    Self { reader: self.reader, pos: self.pos }
  }
}

impl<'reader, R: Read<Error: Copy> + Seek> ErrorType for ClonableSeek<'reader, R> {
  type Error = R::Error;
}
impl<'reader, R: Read<Error: Copy> + Seek> Read for ClonableSeek<'reader, R> {
  fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
    let mut reader = self.reader.borrow_mut();
    reader.seek(SeekFrom::Start(self.pos))?;
    let res = reader.read(buf)?;
    self.pos = reader.stream_position()?;
    Ok(res)
  }
}
impl<'reader, R: Read<Error: Copy> + Seek> Seek for ClonableSeek<'reader, R> {
  fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
    let mut reader = self.reader.borrow_mut();
    reader.seek(SeekFrom::Start(self.pos))?;
    self.pos = reader.seek(pos)?;
    Ok(self.pos)
  }
}

#[test]
fn test_read() {
  const SERIALIZATION: &[u8] = br#"{ "hello": "goodbye" }"#;

  let reader = ReadAdapter::from(SERIALIZATION);
  let mut deserializer =
    core_json::Deserializer::<_, core_json::ConstStack<128>>::new(reader).unwrap();
  let value = deserializer.value().unwrap();
  let mut fields = value.fields().unwrap();
  let field = fields.next().unwrap();
  let mut field = field.unwrap();
  assert_eq!(field.key().collect::<Result<String, _>>().unwrap(), "hello");
  assert_eq!(field.value().to_str().unwrap().collect::<Result<String, _>>().unwrap(), "goodbye");
  assert!(fields.next().is_none());
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_possible_wrap)]
#[test]
fn test_seek() {
  const SERIALIZATION: &[u8] = br#"{ "hello": "goodbye" }"#;

  struct SerializationSeek(usize);
  impl ErrorType for SerializationSeek {
    type Error = <&'static [u8] as ErrorType>::Error;
  }
  impl Read for SerializationSeek {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
      let len = (&SERIALIZATION[self.0 ..]).read(buf)?;
      self.0 += len;
      Ok(len)
    }
  }
  impl Seek for SerializationSeek {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
      match pos {
        SeekFrom::Start(pos) => self.0 = pos as usize,
        SeekFrom::End(offset) => self.0 = ((SERIALIZATION.len() as i64) + offset) as usize,
        SeekFrom::Current(offset) => self.0 = ((self.0 as i64) + offset) as usize,
      }
      Ok(self.0 as u64)
    }
  }

  let reader = RefCell::new(SerializationSeek(0));
  let reader = ClonableSeek::<'_, SerializationSeek>::try_from(&reader).unwrap();
  let reader = SeekAdapter::from(reader);
  let mut deserializer =
    core_json::Deserializer::<_, core_json::ConstStack<128>>::new(reader).unwrap();
  let value = deserializer.value().unwrap();
  let mut fields = value.fields().unwrap();
  let field = fields.next().unwrap();
  let mut field = field.unwrap();
  assert_eq!(field.key().collect::<Result<String, _>>().unwrap(), "hello");
  assert_eq!(field.value().to_str().unwrap().collect::<Result<String, _>>().unwrap(), "goodbye");
  assert!(fields.next().is_none());
}
