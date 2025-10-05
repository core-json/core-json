#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

pub use core_json::*;

mod primitives;
mod sequences;
#[cfg(feature = "alloc")]
mod string;

/// An item which can be deserialized from a `Value`.
///
/// This will deserialize the object present without limitation. This should be kept in mind when
/// deserializing into types which allocate.
pub trait JsonDeserialize: Sized {
  /// Decode this item from a `Value`.
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>>;
}

/// An item which can deserialized from a JSON serialization.
pub trait JsonObject: JsonDeserialize {
  /// Deserialize this item from an JSON-serialized blob.
  ///
  /// This will deserialize the object present without limitation. If a bound is desired, bound the
  /// length of input or deserialize into types which define bounds.
  ///
  /// This method SHOULD NOT be overriden.
  fn deserialize_object<'bytes, B: BytesLike<'bytes>, S: Stack>(
    json: B,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    let mut json = Deserializer::new(json)?;
    let value = json.value()?;
    Self::deserialize(value)
  }
}

impl<T: JsonDeserialize> JsonDeserialize for Option<T> {
  /// This will accept `null` as a representation of `None`.
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    if value.is_null()? {
      return Ok(None);
    }
    T::deserialize(value).map(Some)
  }
}
