use crate::{Read, Stack, JsonError, Type, Value, JsonDeserialize, JsonSerialize};

/// A three-state variable comparable to a flattened `Option<Option<T>>`.
///
/// When deserializing, this preserves the distinction between present and present as `null`, when
/// `Option` will deserialize both cases into the singular `None`.
///
/// When serializing an object, `core-json-derive` will always wrap the field with `Tri::from`.
/// This allows `core-json-derive` to omit fields which shouldn't be serialized. `Tri` does not
/// itself implement `JsonSerialize` as the caller _must_ first decide whether or not to serialize
/// it at all by its pattern.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Debug)]
pub enum Tri<T> {
  /// A value present.
  Some(T),
  /// A value present as `null`.
  ///
  /// `Option as JsonDeserialize` would consider this as `None` but here it's comparable to
  /// `Some(None)`.
  Null,
  /// A value not present.
  ///
  /// `Option as JsonDeserialize` would ambiguously also consider this as `None`, hence the need
  /// for this `enum` in the first place.
  #[default]
  None,
}

impl<T: JsonDeserialize> JsonDeserialize for Tri<T> {
  fn deserialize<'read, 'parent, B: Read<'read>, S: Stack>(
    mut value: Value<'read, 'parent, B, S>,
  ) -> Result<Self, JsonError<'read, B, S>> {
    if matches!(value.kind()?, Type::Null) {
      let () = value.to_null()?;
      return Ok(Tri::Null);
    }
    T::deserialize(value).map(Tri::Some)
  }
}

impl<'value, T> From<&'value Tri<T>> for Tri<&'value T> {
  fn from(value: &'value Tri<T>) -> Self {
    match value {
      Tri::Some(value) => Tri::Some(value),
      Tri::Null => Tri::Null,
      Tri::None => Tri::None,
    }
  }
}

impl<'value, T: JsonSerialize> From<&'value T> for Tri<&'value T> {
  /// Wrap a value serializable as JSON with `Tri::Some`.
  fn from(value: &'value T) -> Self {
    Self::Some(value)
  }
}
