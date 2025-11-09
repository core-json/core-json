use alloc::{string::String, collections::BTreeMap};
#[cfg(feature = "std")]
use std::collections::HashMap;

use crate::{Read, Stack, JsonError, Value, JsonDeserialize, JsonStructure, JsonSerialize};

type YieldedField<'read, T, R, S> = Result<(String, T), JsonError<'read, R, S>>;
fn deserialize_map<'read, 'parent, T: JsonDeserialize, R: Read<'read>, S: Stack>(
  value: Value<'read, 'parent, R, S>,
) -> Result<impl Iterator<Item = YieldedField<'read, T, R, S>>, JsonError<'read, R, S>> {
  let mut iter = value.fields()?;
  Ok(core::iter::from_fn(move || {
    let mut field = match iter.next()? {
      Ok(value) => value,
      Err(e) => return Some(Err(e)),
    };
    let key = match field.key().collect::<Result<String, _>>() {
      Ok(value) => value,
      Err(e) => return Some(Err(e)),
    };
    let value = match field.value() {
      Ok(value) => value,
      Err(e) => return Some(Err(e)),
    };
    match T::deserialize(value) {
      Ok(value) => Some(Ok((key, value))),
      Err(e) => Some(Err(e)),
    }
  }))
}

fn serialize_field<'serializing>(
  (key, value): (&'serializing str, &'serializing (impl 'serializing + JsonSerialize)),
) -> impl Iterator<Item = char> {
  key.serialize().chain(core::iter::once(':')).chain(value.serialize())
}

#[rustfmt::skip]
fn serialize_map<'serializing>(
  mut iter: impl Iterator<Item = (&'serializing str, &'serializing (impl 'serializing + JsonSerialize))>,
) -> impl Iterator<Item = char> {
  let fields = iter.next().map(|first_field| {
    let first_field = serialize_field(first_field);
    let next_fields =
      iter.flat_map(|next_field| core::iter::once(',').chain(serialize_field(next_field)));
    first_field.chain(next_fields)
  });
  core::iter::once('{').chain(fields.into_iter().flatten()).chain(core::iter::once('}'))
}

impl<T: JsonDeserialize> JsonDeserialize for BTreeMap<String, T> {
  fn deserialize<'read, 'parent, R: Read<'read>, S: Stack>(
    value: Value<'read, 'parent, R, S>,
  ) -> Result<Self, JsonError<'read, R, S>> {
    deserialize_map::<T, _, _>(value)?.collect()
  }
}
impl<K: AsRef<str>, T: JsonSerialize> JsonSerialize for BTreeMap<K, T> {
  fn serialize(&self) -> impl Iterator<Item = char> {
    serialize_map(self.iter().map(|(key, value)| (key.as_ref(), value)))
  }
}
impl<T: JsonDeserialize> JsonStructure for BTreeMap<String, T> {}

#[cfg(feature = "std")]
impl<T: JsonDeserialize> JsonDeserialize for HashMap<String, T> {
  fn deserialize<'read, 'parent, R: Read<'read>, S: Stack>(
    value: Value<'read, 'parent, R, S>,
  ) -> Result<Self, JsonError<'read, R, S>> {
    deserialize_map::<T, _, _>(value)?.collect()
  }
}
#[cfg(feature = "std")]
impl<K: AsRef<str>, T: JsonSerialize> JsonSerialize for HashMap<K, T> {
  fn serialize(&self) -> impl Iterator<Item = char> {
    serialize_map(self.iter().map(|(key, value)| (key.as_ref(), value)))
  }
}
#[cfg(feature = "std")]
impl<T: JsonDeserialize> JsonStructure for HashMap<String, T> {}

#[cfg(feature = "alloc")]
#[test]
fn btree_map() {
  assert_eq!(BTreeMap::<String, u16>::new().serialize().collect::<String>().as_str(), "{}");
  let test_map = |map: BTreeMap<String, u16>| {
    assert_eq!(
      BTreeMap::<String, u16>::deserialize_structure::<_, crate::ConstStack<32>>(
        map.serialize().collect::<String>().as_bytes()
      )
      .unwrap(),
      map
    );
  };
  test_map(BTreeMap::from([("key1".to_string(), 1)]));
  test_map(BTreeMap::from([("key1".to_string(), 1), ("key2".to_string(), 2)]));
}

#[cfg(feature = "std")]
#[test]
fn hash_map() {
  assert_eq!(HashMap::<String, u16>::new().serialize().collect::<String>().as_str(), "{}");
  let test_map = |map: HashMap<String, u16>| {
    assert_eq!(
      HashMap::<String, u16>::deserialize_structure::<_, crate::ConstStack<32>>(
        map.serialize().collect::<String>().as_bytes()
      )
      .unwrap(),
      map
    );
  };
  test_map(HashMap::from([("key1".to_string(), 1)]));
  test_map(HashMap::from([("key1".to_string(), 1), ("key2".to_string(), 2)]));
}
