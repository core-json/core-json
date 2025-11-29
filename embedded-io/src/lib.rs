#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![cfg_attr(not(test), no_std)]

use core_json::Read as CjRead;
use embedded_io::{ReadExactError, Read};

/// An adapter from [`embedded_io::Read`] to [`core_json::Read`].
#[derive(Debug)]
pub struct ReadAdapter<R: Read<Error: Copy>> {
  reader: R,
}

impl<R: Read<Error: Copy>> From<R> for ReadAdapter<R> {
  fn from(reader: R) -> Self {
    Self { reader }
  }
}

impl<R: Read<Error: Copy>> CjRead<'static> for ReadAdapter<R> {
  type Error = ReadExactError<R::Error>;

  #[inline(always)]
  fn read_byte(&mut self) -> Result<u8, Self::Error> {
    let mut res = [0; 1];
    self.reader.read_exact(&mut res)?;
    Ok(res[0])
  }

  #[inline(always)]
  fn read_exact(&mut self, slice: &mut [u8]) -> Result<(), Self::Error> {
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
  assert_eq!(field.key().unwrap().collect::<Result<String, _>>().unwrap(), "hello");
  assert_eq!(
    field.value().unwrap().to_str().unwrap().collect::<Result<String, _>>().unwrap(),
    "goodbye"
  );
  assert!(fields.next().is_none());
}
