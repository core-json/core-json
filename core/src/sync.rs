use crate::*;

#[inline(always)]
fn block_on<F: Future>(fut: F) -> F::Output {
  use core::task::*;
  const CONTEXT: Context = Context::from_waker(Waker::noop());
  #[allow(const_item_mutation)]
  match core::pin::pin!(fut).poll(&mut CONTEXT) {
    Poll::Ready(value) => value,
    Poll::Pending => unreachable!("synchronous IO created a future which yielded pending"),
  }
}

/// A synchronous alternative to [`AsyncField`].
pub struct Field<'read, 'parent, R: Read<'read>, S: Stack>(AsyncField<'read, 'parent, R, S>);
impl<'read, 'parent, R: Read<'read>, S: Stack> Field<'read, 'parent, R, S> {
  /// A synchronous alternative to [`AsyncField::next_char_in_key`], collapsed to simply return an
  /// iterator.
  ///
  /// This method is not reusable. Successive calls' iterators will continue where the last left
  /// off.
  #[inline(always)]
  pub fn key(&mut self) -> impl Iterator<Item = Result<char, JsonError<'read, R, S>>> {
    core::iter::from_fn(move || block_on(self.0.next_char_in_key()))
  }
  /// A synchronous alternative to [`AsyncField::value`].
  #[inline(always)]
  pub fn value(self) -> Result<Value<'read, 'parent, R, S>, JsonError<'read, R, S>> {
    self.0.value().map(Value)
  }
}

/// A synchronous alternative to [`AsyncString`].
struct JsonString<'read, 'parent, R: Read<'read>, S: Stack>(AsyncString<'read, 'parent, R, S>);
impl<'read, 'parent, R: Read<'read>, S: Stack> Iterator for JsonString<'read, 'parent, R, S> {
  type Item = Result<char, JsonError<'read, R, S>>;
  #[inline(always)]
  fn next(&mut self) -> Option<Self::Item> {
    block_on(self.0.next())
  }
}

/// A synchronous alternative to [`AsyncFieldIterator`].
pub struct FieldIterator<'read, 'parent, R: Read<'read>, S: Stack>(
  AsyncFieldIterator<'read, 'parent, R, S>,
);

impl<'read, 'parent, R: Read<'read>, S: Stack> FieldIterator<'read, 'parent, R, S> {
  /// A synchronous alternative to [`AsyncFieldIterator::next`].
  #[allow(clippy::type_complexity, clippy::should_implement_trait)]
  pub fn next(&mut self) -> Option<Result<Field<'read, '_, R, S>, JsonError<'read, R, S>>> {
    block_on(self.0.next()).map(|res| res.map(Field))
  }
}

/// A synchronous alternative to [`AsyncArrayIterator`].
pub struct ArrayIterator<'read, 'parent, R: Read<'read>, S: Stack>(
  AsyncArrayIterator<'read, 'parent, R, S>,
);

impl<'read, 'parent, R: Read<'read>, S: Stack> ArrayIterator<'read, 'parent, R, S> {
  /// A synchronous alternative to [`AsyncArrayIterator::next`].
  #[allow(clippy::should_implement_trait)]
  pub fn next(&mut self) -> Option<Result<Value<'read, '_, R, S>, JsonError<'read, R, S>>> {
    block_on(self.0.next()).map(|res| res.map(Value))
  }
}

/// A synchronous alternative to [`AsyncValue`].
pub struct Value<'read, 'parent, R: Read<'read>, S: Stack>(AsyncValue<'read, 'parent, R, S>);

impl<'read, 'parent, R: Read<'read>, S: Stack> Value<'read, 'parent, R, S> {
  /// A synchronous alternative to [`AsyncValue::kind`].
  #[inline(always)]
  pub fn kind(&mut self) -> Result<Type, JsonError<'read, R, S>> {
    block_on(self.0.kind())
  }

  /// A synchronous alternative to [`AsyncValue::fields`].
  #[inline(always)]
  pub fn fields(self) -> Result<FieldIterator<'read, 'parent, R, S>, JsonError<'read, R, S>> {
    block_on(self.0.fields()).map(FieldIterator)
  }

  /// A synchronous alternative to [`AsyncValue::iterate`].
  #[inline(always)]
  pub fn iterate(self) -> Result<ArrayIterator<'read, 'parent, R, S>, JsonError<'read, R, S>> {
    block_on(self.0.iterate()).map(ArrayIterator)
  }

  /// A synchronous alternative to [`AsyncValue::to_str`].
  #[inline(always)]
  pub fn to_str(
    self,
  ) -> Result<impl Iterator<Item = Result<char, JsonError<'read, R, S>>>, JsonError<'read, R, S>>
  {
    block_on(self.0.to_str()).map(JsonString)
  }

  /// A synchronous alternative to [`AsyncValue::to_number`].
  #[inline(always)]
  pub fn to_number(self) -> Result<Number, JsonError<'read, R, S>> {
    block_on(self.0.to_number())
  }

  /// A synchronous alternative to [`AsyncValue::to_bool`].
  #[inline(always)]
  pub fn to_bool(self) -> Result<bool, JsonError<'read, R, S>> {
    block_on(self.0.to_bool())
  }

  /// A synchronous alternative to [`AsyncValue::to_null`].
  #[inline(always)]
  pub fn to_null(self) -> Result<(), JsonError<'read, R, S>> {
    block_on(self.0.to_null())
  }
}

/// A synchronous alternative to [`AsyncDeserializer`].
pub struct Deserializer<'read, R: Read<'read>, S: Stack>(AsyncDeserializer<'read, R, S>);
impl<'read, R: Read<'read>, S: Stack> Deserializer<'read, R, S> {
  /// A synchronous alternative to [`AsyncDeserializer::new`].
  #[inline(always)]
  pub fn new(reader: R) -> Result<Self, JsonError<'read, R, S>> {
    block_on(AsyncDeserializer::new(reader)).map(Self)
  }

  /// A synchronous alternative to [`AsyncDeserializer::value`].
  #[inline(always)]
  pub fn value(&mut self) -> Result<Value<'read, '_, R, S>, JsonError<'read, R, S>> {
    block_on(self.0.value()).map(Value)
  }
}
