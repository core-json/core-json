use core::marker::PhantomData;

use crate::{
  Read, Stack, JsonError, Value, ArrayIterator, JsonDeserialize, JsonStructure, JsonSerialize,
};

struct Sequence<'read, 'parent, R: Read<'read>, S: Stack, T: JsonDeserialize> {
  iterator: ArrayIterator<'read, 'parent, R, S>,
  _phantom: PhantomData<T>,
}
impl<'read, 'parent, R: Read<'read>, S: Stack, T: JsonDeserialize> Iterator
  for Sequence<'read, 'parent, R, S, T>
{
  type Item = Result<T, JsonError<'read, R, S>>;
  fn next(&mut self) -> Option<Self::Item> {
    match self.iterator.next()? {
      Ok(value) => Some(T::deserialize(value)),
      Err(e) => Some(Err(e)),
    }
  }
}

pub(crate) fn serialize_sequence<'element, T: 'element + JsonSerialize>(
  iterator: impl Iterator<Item = &'element T>,
) -> impl Iterator<Item = char> {
  struct ConnectWithCommas<I: Iterator<Item = char>, II: Iterator<Item = I>> {
    iterator: II,
    current: I,
  }
  impl<I: Iterator<Item = char>, II: Iterator<Item = I>> Iterator for ConnectWithCommas<I, II> {
    type Item = char;
    fn next(&mut self) -> Option<char> {
      match self.current.next() {
        Some(char) => Some(char),
        None => {
          let next = self.iterator.next()?;
          self.current = next;
          Some(',')
        }
      }
    }
  }

  let mut iterator = iterator.map(JsonSerialize::serialize);
  core::iter::once('[')
    .chain(
      iterator.next().map(|current| ConnectWithCommas { iterator, current }).into_iter().flatten(),
    )
    .chain(core::iter::once(']'))
}

impl<T: Default + JsonDeserialize, const N: usize> JsonDeserialize for [T; N] {
  fn deserialize<'read, 'parent, B: Read<'read>, S: Stack>(
    value: Value<'read, 'parent, B, S>,
  ) -> Result<Self, JsonError<'read, B, S>> {
    let mut iter = Sequence { iterator: value.iterate()?, _phantom: PhantomData };
    let mut error = None;
    let res = core::array::from_fn(|_| {
      match error.is_none().then(|| iter.next().unwrap_or(Err(JsonError::TypeError))) {
        Some(Ok(value)) => return value,
        Some(Err(e)) => error = Some(e),
        None => {}
      }
      Default::default()
    });
    if let Some(error) = error.or_else(|| iter.next().is_some().then_some(JsonError::TypeError)) {
      Err(error)?;
    }
    Ok(res)
  }
}
impl<T: Default + JsonDeserialize, const N: usize> JsonStructure for [T; N] {}

impl<T: JsonSerialize, const N: usize> JsonSerialize for [T; N] {
  #[inline(always)]
  fn serialize(&self) -> impl Iterator<Item = char> {
    serialize_sequence(self.iter())
  }
}

impl<T: JsonSerialize> JsonSerialize for [T] {
  #[inline(always)]
  fn serialize(&self) -> impl Iterator<Item = char> {
    serialize_sequence(self.iter())
  }
}

#[cfg(feature = "alloc")]
impl<T: JsonDeserialize> JsonDeserialize for alloc::vec::Vec<T> {
  #[inline(always)]
  fn deserialize<'read, 'parent, R: Read<'read>, S: Stack>(
    value: Value<'read, 'parent, R, S>,
  ) -> Result<Self, JsonError<'read, R, S>> {
    (Sequence { iterator: value.iterate()?, _phantom: PhantomData }).collect()
  }
}
#[cfg(feature = "alloc")]
impl<T: JsonDeserialize> JsonStructure for alloc::vec::Vec<T> {}
#[cfg(feature = "alloc")]
impl<T: JsonSerialize> JsonSerialize for alloc::vec::Vec<T> {
  #[inline(always)]
  fn serialize(&self) -> impl Iterator<Item = char> {
    serialize_sequence(self.iter())
  }
}

#[cfg(feature = "alloc")]
macro_rules! set {
  ($($deser_bounds: path)|+, $($ser_bounds: path)|+, $kind: ty) => {
    impl<T: $($deser_bounds +)+> JsonDeserialize for $kind {
      /// This will deserialize a JSON array into a set, accepting duplicates and any ordering.
      #[inline(always)]
      fn deserialize<'read, 'parent, R: Read<'read>, S: Stack>(
        value: Value<'read, 'parent, R, S>,
      ) -> Result<Self, JsonError<'read, R, S>> {
        (Sequence { iterator: value.iterate()?, _phantom: PhantomData }).collect()
      }
    }
    impl<T: $($deser_bounds +)+> JsonStructure for $kind {}
    impl<T: $($ser_bounds +)+> JsonSerialize for $kind {
      /// This will serialize a set as its elements into a JSON array, with undefined ordering.
      #[inline(always)]
      fn serialize(&self) -> impl Iterator<Item = char> {
        serialize_sequence(self.iter())
      }
    }
  };
}
#[cfg(feature = "alloc")]
set!(Ord | JsonDeserialize, Ord | JsonSerialize, alloc::collections::BTreeSet<T>);
#[cfg(feature = "std")]
set!(
  Eq | core::hash::Hash | JsonDeserialize,
  Eq | core::hash::Hash | JsonSerialize,
  std::collections::HashSet<T>
);

#[test]
fn arr() {
  assert_eq!(
    <[u8; 0]>::deserialize_structure::<_, crate::ConstStack<128>>("[]".as_bytes()).unwrap(),
    [],
  );
  assert_eq!(
    <[u8; 1]>::deserialize_structure::<_, crate::ConstStack<128>>("[1]".as_bytes()).unwrap(),
    [1],
  );
  assert_eq!(
    <[u8; 2]>::deserialize_structure::<_, crate::ConstStack<128>>("[1, 2]".as_bytes()).unwrap(),
    [1, 2],
  );

  // Short arrays should be considered a distinct type
  assert!(matches!(
    <[u8; 1]>::deserialize_structure::<_, crate::ConstStack<128>>("[]".as_bytes()).unwrap_err(),
    JsonError::TypeError,
  ));
  // The same for long arrays
  assert!(matches!(
    <[u8; 1]>::deserialize_structure::<_, crate::ConstStack<128>>("[1, 2]".as_bytes()).unwrap_err(),
    JsonError::TypeError,
  ));
}

#[cfg(feature = "alloc")]
#[test]
fn seq() {
  assert_eq!(&[0; 0].serialize().collect::<String>(), "[]");
  assert_eq!(&[0; 1].serialize().collect::<String>(), "[0]");
  assert_eq!(&[0; 2].serialize().collect::<String>(), "[0,0]");
}
