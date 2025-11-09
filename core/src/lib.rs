#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

mod io;
mod stack;
mod string;
mod number;
mod deserializer;

pub use io::{Read, AsyncRead};
#[cfg(feature = "std")]
pub use io::ReadAdapter;
use io::PeekableRead;
pub use stack::*;
use string::{String as InternalInternalString, StringKey, StringValue};
pub use number::{NumberSink, Number};
pub use deserializer::{AsyncDeserializer, AsyncValue};
use deserializer::*;

mod sync;
pub use sync::*;

/// An error incurred when deserializing.
#[derive(Debug)]
pub enum JsonError<'read, R: AsyncRead<'read>, S: Stack> {
  /// An unexpected state was reached during deserialization.
  InternalError,
  /// An error from the reader.
  ReadError(R::Error),
  /// An error from the stack.
  StackError(S::Error),
  /// The deserializer was reused.
  ReusedDeserializer,
  /// The JSON had an invalid key.
  InvalidKey,
  /// The JSON had an invalid delimiter between the key and value (`:` expected).
  InvalidKeyValueDelimiter,
  /// The JSON had an invalid value.
  InvalidValue,
  /// The string represented by the JSON serialization was valid yet not UTF-8.
  NotUtf8,
  /// The JSON had a trailing comma.
  TrailingComma,
  /// The JSON had mismatched delimiters between the open and close of the structure.
  MismatchedDelimiter,
  /// Operation could not be performed given the value's type.
  TypeError,
}
impl<'read, R: AsyncRead<'read>, S: Stack> Clone for JsonError<'read, R, S> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}
impl<'read, R: AsyncRead<'read>, S: Stack> Copy for JsonError<'read, R, S> {}

/// The type of the value.
///
/// <https://datatracker.ietf.org/doc/html/rfc8259#section-3> defines all possible values.
pub enum Type {
  /// An object.
  Object,
  /// An array.
  Array,
  /// A string.
  String,
  /// A RFC-8259 number.
  Number,
  /// A boolean.
  Bool,
  /// The `null` unit value.
  Null,
}

/// Get the type of the current item.
///
/// This does not assert it's a valid instance of this class of items. It solely asserts if this
/// is a valid item, it will be of this type.
#[inline(always)]
fn kind<'read, R: AsyncRead<'read>>(reader: &PeekableRead<'read, R>) -> Type {
  match reader.peek() {
    b'{' => Type::Object,
    b'[' => Type::Array,
    b'"' => Type::String,
    b't' | b'f' => Type::Bool,
    b'n' => Type::Null,
    _ => Type::Number,
  }
}

/// A field within an object.
pub struct AsyncField<'read, 'parent, R: AsyncRead<'read>, S: Stack> {
  key: Option<StringKey<'read, 'parent, R, S>>,
}

/// Handle a field.
///
/// This MUST be called every time a `SingleStepObjectResult::Field` is yielded, unless the
/// deserializer is immediately terminated (such as by setting its error to `Some(_)`). This
/// ideally would be yielded within `SingleStepObjectResult::Field`, yet that would cause every
/// `SingleStepObjectResult` to consume the parent's borrow for the rest of its lifetime, when we
/// only want to consume it upon `SingleStepObjectResult::Field`.
#[inline(always)]
fn handle_field<'read, 'parent, R: AsyncRead<'read>, S: Stack>(
  deserializer: &'parent mut AsyncDeserializer<'read, R, S>,
) -> AsyncField<'read, 'parent, R, S> {
  AsyncField { key: Some(StringKey(InternalInternalString::read(deserializer))) }
}

/// Handle a string value.
///
/// This MUST be called every time a `SingleStepUnknownResult::String` is yielded, unless the
/// deserializer is immediately terminated (such as by setting its error to `Some(_)`). This
/// ideally would be yielded within `SingleStepUnknownResult::String`, yet that would cause every
/// `SingleStepUnknownResult` to consume the parent's borrow for the rest of its lifetime, when we
/// only want to consume it upon `SingleStepUnknownResult::String`.
#[inline(always)]
fn handle_string_value<'read, 'parent, R: AsyncRead<'read>, S: Stack>(
  deserializer: &'parent mut AsyncDeserializer<'read, R, S>,
) -> StringValue<'read, 'parent, R, S> {
  StringValue(InternalInternalString::read(deserializer))
}

/// A view of a string.
///
/// As we cannot perform allocations, we do not yield a [`alloc::string::String`] but rather an
/// iterator for the contents of the serialized string (with its escape sequences handled).
///
/// RFC 8259 allows strings to specify invalid UTF-8 codepoints. This library supports working
/// with such values, as required to be compliant with RFC 8259, but this function's return value
/// will error when attempting to return a non-UTF-8 value. If the underlying JSON is valid, the
/// deserializer will remain usable afterwards however, even though the rest of the non-UTF-8
/// string will be inaccesible.
pub struct AsyncString<'read, 'parent, R: AsyncRead<'read>, S: Stack>(
  StringValue<'read, 'parent, R, S>,
);
impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> AsyncString<'read, 'parent, R, S> {
  /// The next character within the string.
  #[inline(always)]
  pub async fn next(&mut self) -> Option<Result<char, JsonError<'read, R, S>>> {
    self.0.0.next().await
  }
}

impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> AsyncField<'read, 'parent, R, S> {
  /// Access the next character within the key.
  ///
  /// If the key has valid syntax under JSON, yet does not represent a valid UTF-8 sequence,
  /// `AsyncField::value` may still be called and deserialization may continue, even though this
  /// may error midway through the JSON-encoded string.
  #[allow(clippy::type_complexity)]
  #[inline(always)]
  pub async fn next_char_in_key(&mut self) -> Option<Result<char, JsonError<'read, R, S>>> {
    match self.key.as_mut().ok_or(JsonError::InternalError) {
      Ok(value) => value.0.next().await,
      Err(e) => Some(Err(e)),
    }
  }

  /// Access the field's value.
  #[inline(always)]
  pub fn value(mut self) -> Result<AsyncValue<'read, 'parent, R, S>, JsonError<'read, R, S>> {
    Ok(AsyncValue { deserializer: Some(self.key.take().ok_or(JsonError::InternalError)?.drop()) })
  }
}

impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> Drop for AsyncField<'read, 'parent, R, S> {
  #[inline(always)]
  fn drop(&mut self) {
    if let Some(key) = self.key.take() {
      drop(AsyncValue { deserializer: Some(key.drop()) });
    }
  }
}

/// An iterator over fields.
pub struct AsyncFieldIterator<'read, 'parent, R: AsyncRead<'read>, S: Stack> {
  deserializer: &'parent mut AsyncDeserializer<'read, R, S>,
  done: bool,
}

// When this object is dropped, advance the decoder past the unread items
impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> Drop
  for AsyncFieldIterator<'read, 'parent, R, S>
{
  #[inline(always)]
  fn drop(&mut self) {
    if !self.done {
      self.deserializer.drop_structure();
    }
  }
}

impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> AsyncFieldIterator<'read, 'parent, R, S> {
  /// The next field within the object.
  ///
  /// This is approximate to `Iterator::next` yet each item maintains a mutable reference to the
  /// iterator. Accordingly, we cannot use `Iterator::next` which requires items not borrow from
  /// the iterator.
  ///
  /// [polonius-the-crab](https://docs.rs/polonius-the-crab) details a frequent limitation of
  /// Rust's borrow checker which users of this function may incur. It also details potential
  /// solutions (primarily using inlined code instead of functions, callbacks) before presenting
  /// itself as a complete solution. Please refer to it if you have difficulties calling this
  /// method for context.
  #[allow(clippy::type_complexity, clippy::should_implement_trait)]
  pub async fn next(
    &mut self,
  ) -> Option<Result<AsyncField<'read, '_, R, S>, JsonError<'read, R, S>>> {
    if self.done {
      None?;
    }

    loop {
      let result = match self.deserializer.single_step().await {
        Ok(SingleStepResult::Object(result)) => result,
        Ok(_) => break Some(Err(JsonError::InternalError)),
        Err(e) => break Some(Err(e)),
      };
      match result {
        SingleStepObjectResult::Field => break Some(Ok(handle_field(self.deserializer))),
        SingleStepObjectResult::Closed => {
          self.done = true;
          None?
        }
      }
    }
  }
}

/// An iterator over an array.
pub struct AsyncArrayIterator<'read, 'parent, R: AsyncRead<'read>, S: Stack> {
  deserializer: &'parent mut AsyncDeserializer<'read, R, S>,
  done: bool,
}

// When this array is dropped, advance the decoder past the unread items
impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> Drop
  for AsyncArrayIterator<'read, 'parent, R, S>
{
  #[inline(always)]
  fn drop(&mut self) {
    if !self.done {
      self.deserializer.drop_structure();
    }
  }
}

impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> AsyncArrayIterator<'read, 'parent, R, S> {
  /// The next item within the array.
  ///
  /// This is approximate to `Iterator::next` yet each item maintains a mutable reference to the
  /// iterator. Accordingly, we cannot use `Iterator::next` which requires items not borrow from
  /// the iterator.
  ///
  /// [polonius-the-crab](https://docs.rs/polonius-the-crab) details a frequent limitation of
  /// Rust's borrow checker which users of this function may incur. It also details potential
  /// solutions (primarily using inlined code instead of functions, callbacks) before presenting
  /// itself as a complete solution. Please refer to it if you have difficulties calling this
  /// method for context.
  #[allow(clippy::should_implement_trait)]
  pub async fn next(
    &mut self,
  ) -> Option<Result<AsyncValue<'read, '_, R, S>, JsonError<'read, R, S>>> {
    if self.done {
      None?;
    }

    loop {
      let result = match self.deserializer.single_step().await {
        Ok(SingleStepResult::Array(result)) => result,
        Ok(_) => break Some(Err(JsonError::InternalError)),
        Err(e) => break Some(Err(e)),
      };
      match result {
        SingleStepArrayResult::Value => {
          break Some(Ok(AsyncValue { deserializer: Some(self.deserializer) }));
        }
        SingleStepArrayResult::Closed => {
          self.done = true;
          None?
        }
      }
    }
  }
}

impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> AsyncValue<'read, 'parent, R, S> {
  /// Get the type of the current item.
  ///
  /// This does not assert it's a valid instance of this class of items. It solely asserts if this
  /// is a valid item, it will be of this type.
  #[inline(always)]
  pub async fn kind(&mut self) -> Result<Type, JsonError<'read, R, S>> {
    let deserializer = self.deserializer.as_mut().ok_or(JsonError::InternalError)?;
    match DelayedDrop::drop(deserializer).await {
      Ok(()) => {}
      Err(e) => {
        deserializer.poison(e);
        Err(e)?
      }
    }
    Ok(kind(&deserializer.reader))
  }

  /// Iterate over the fields within this object.
  ///
  /// If a field is present multiple times, this will yield each instance.
  #[inline(always)]
  pub async fn fields(
    mut self,
  ) -> Result<AsyncFieldIterator<'read, 'parent, R, S>, JsonError<'read, R, S>> {
    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match deserializer.single_step().await? {
      SingleStepResult::Unknown(SingleStepUnknownResult::ObjectOpened) => {
        Ok(AsyncFieldIterator { deserializer, done: false })
      }
      SingleStepResult::Unknown(_) => Err(JsonError::TypeError)?,
      _ => Err(JsonError::InternalError),
    }
  }

  /// Iterate over all items within this container.
  #[inline(always)]
  pub async fn iterate(
    mut self,
  ) -> Result<AsyncArrayIterator<'read, 'parent, R, S>, JsonError<'read, R, S>> {
    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match deserializer.single_step().await? {
      SingleStepResult::Unknown(SingleStepUnknownResult::ArrayOpened) => {
        Ok(AsyncArrayIterator { deserializer, done: false })
      }
      SingleStepResult::Unknown(_) => Err(JsonError::TypeError)?,
      _ => Err(JsonError::InternalError),
    }
  }

  /// Get the current item as a string.
  #[inline(always)]
  pub async fn to_str(
    mut self,
  ) -> Result<AsyncString<'read, 'parent, R, S>, JsonError<'read, R, S>> {
    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match deserializer.single_step().await? {
      SingleStepResult::Unknown(SingleStepUnknownResult::String) => {
        Ok(AsyncString(handle_string_value(deserializer)))
      }
      SingleStepResult::Unknown(_) => Err(JsonError::TypeError)?,
      _ => Err(JsonError::InternalError),
    }
  }

  /// Get the current item as a number.
  #[inline(always)]
  pub async fn to_number(mut self) -> Result<Number, JsonError<'read, R, S>> {
    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match deserializer.single_step().await? {
      SingleStepResult::Unknown(SingleStepUnknownResult::Number(number)) => Ok(number),
      SingleStepResult::Unknown(_) => Err(JsonError::TypeError)?,
      _ => Err(JsonError::InternalError),
    }
  }

  /// Get the current item as a `bool`.
  #[inline(always)]
  pub async fn to_bool(mut self) -> Result<bool, JsonError<'read, R, S>> {
    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match deserializer.single_step().await? {
      SingleStepResult::Unknown(SingleStepUnknownResult::Bool(bool)) => Ok(bool),
      SingleStepResult::Unknown(_) => Err(JsonError::TypeError)?,
      _ => Err(JsonError::InternalError),
    }
  }

  /// Get the current item as `null`.
  ///
  /// The point of this method is to assert the value is `null` _and valid_. `kind` only tells the
  /// caller if it's a valid value, it will be `null`.
  #[inline(always)]
  pub async fn to_null(mut self) -> Result<(), JsonError<'read, R, S>> {
    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match deserializer.single_step().await? {
      SingleStepResult::Unknown(SingleStepUnknownResult::Null) => Ok(()),
      SingleStepResult::Unknown(_) => Err(JsonError::TypeError)?,
      _ => Err(JsonError::InternalError),
    }
  }
}
