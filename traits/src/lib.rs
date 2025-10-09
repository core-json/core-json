#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub use core_json::*;

mod primitives;
mod float;
mod option;
mod sequences;
mod string;

#[cfg(feature = "alloc")]
mod boxed;

pub use float::JsonF64;

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

/// A structure which can deserialized from a JSON serialization.
pub trait JsonStructure: JsonDeserialize {
  /// Deserialize this structure from an JSON-serialized blob.
  ///
  /// This will deserialize the structure present without limitation. If a bound is desired, bound
  /// the length of input or deserialize into types which define bounds.
  ///
  /// This method SHOULD NOT be overriden.
  fn deserialize_structure<'bytes, B: BytesLike<'bytes>, S: Stack>(
    json: B,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    let mut json = Deserializer::new(json)?;
    let value = json.value()?;
    Self::deserialize(value)
  }
}

/// An item which can be serialized as JSON.
pub trait JsonSerialize {
  /// Serialize this item as JSON.
  ///
  /// This returns an `impl Iterator<Item = char>` to maintain support for serializing without
  /// requiring an allocator.
  fn serialize(&self) -> impl Iterator<Item = char>;
}
