#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![cfg_attr(not(test), no_std)]

use core::fmt;
use core_json::Read as CjRead;
use embedded_io::{ReadExactError, Read};

/// An adapter from [`embedded_io::Read`] to [`core_json::Read`].
pub struct ReadAdapter<R: Read<Error: Copy>> {
  reader: R,
}
impl<R: Read<Error: Copy>> fmt::Debug for ReadAdapter<R> {
  fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
    fmt.debug_struct("ReadAdapter").finish_non_exhaustive()
  }
}

impl<R: Read<Error: Copy>> From<R> for ReadAdapter<R> {
  fn from(reader: R) -> Self {
    Self { reader }
  }
}

impl<R: Read<Error: Copy>> CjRead<'static> for ReadAdapter<R> {
  type Error = ReadExactError<R::Error>;

  fn read_into_slice(&mut self, slice: &mut [u8]) -> Result<(), Self::Error> {
    self.reader.read_exact(slice)
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
