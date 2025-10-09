use crate::{BytesLike, Stack, JsonError, Value, JsonDeserialize, JsonSerialize};

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

impl<T: JsonSerialize> JsonSerialize for Option<T> {
  /// This will serialize `Some(value)` as `value` and `None` as `null`.
  fn serialize(&self) -> impl Iterator<Item = char> {
    self
      .as_ref()
      .map(|value| T::serialize(value))
      .into_iter()
      .flatten()
      .chain(self.is_none().then(|| "null".chars()).into_iter().flatten())
  }
}
