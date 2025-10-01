#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![no_std]

extern crate alloc;

pub use monero_epee::*;

mod primitives;
mod sequences;

/// An item which can be decoded from an `EpeeEntry`.
///
/// This MAY perform coercion from the encoded type into the result type so long as no information
/// from the encoded type would potentially be lost. This means a `u8` MAY be decoded into a `u16`
/// but `1u16` MAY NOT be decoded into a `u8`.
///
/// This will decode the object present without limitation. This should be kept in mind when
/// decoding into types which allocate.
pub trait EpeeDecode: Sized {
  /// Decode this item from an entry.
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError>;
}

/// An item which can decoded from an EPEE encoding.
pub trait EpeeObject: EpeeDecode {
  /// Decode this item from an EPEE-encoded blob.
  ///
  /// This will decode the object present without limitation. If a bound is desired, bound the
  /// length of input or decode into types which define bounds.
  ///
  /// This method SHOULD NOT be overriden.
  fn decode_root<'encoding, B: BytesLike<'encoding>>(epee: B) -> Result<Self, EpeeError> {
    let mut epee = Epee::new(epee)?;
    let entry = epee.entry()?;
    Self::decode(entry)
  }
}

impl<T: EpeeDecode> EpeeDecode for Option<T> {
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError> {
    T::decode(entry).map(Some)
  }
}
