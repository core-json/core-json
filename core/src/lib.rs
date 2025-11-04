#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod io;
mod stack;
mod string;
mod number;
mod deserializer;

pub use io::Read;
use io::PeekableRead;
pub use stack::*;
use string::*;
pub use number::{NumberSink, Number};
pub use deserializer::{Deserializer, Value};
use deserializer::*;

/// An error incurred when deserializing.
#[derive(Debug)]
pub enum JsonError<'read, R: Read<'read>, S: Stack> {
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
impl<'read, R: Read<'read>, S: Stack> Clone for JsonError<'read, R, S> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}
impl<'read, R: Read<'read>, S: Stack> Copy for JsonError<'read, R, S> {}

/// The type of the value.
///
/// https://datatracker.ietf.org/doc/html/rfc8259#section-3 defines all possible values.
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
fn kind<'read, R: Read<'read>>(reader: &PeekableRead<'read, R>) -> Type {
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
pub struct Field<'read, 'parent, R: Read<'read>, S: Stack> {
  key: StringKey<'read, 'parent, R, S>,
}

/// Handle a field.
///
/// This MUST be called every time a `SingleStepObjectResult::Field` is yielded, unless the
/// deserializer is immediately terminated (such as by setting its error to `Some(_)`). This
/// ideally would be yielded within `SingleStepObjectResult::Field`, yet that would cause every
/// `SingleStepObjectResult` to consume the parent's borrow for the rest of its lifetime, when we
/// only want to consume it upon `SingleStepObjectResult::Field`.
#[inline(always)]
fn handle_field<'read, 'parent, R: Read<'read>, S: Stack>(
  deserializer: &'parent mut Deserializer<'read, R, S>,
) -> Field<'read, 'parent, R, S> {
  Field { key: StringKey(Some(String::read(deserializer))) }
}

/// Handle a string value.
///
/// This MUST be called every time a `SingleStepUnknownResult::String` is yielded, unless the
/// deserializer is immediately terminated (such as by setting its error to `Some(_)`). This
/// ideally would be yielded within `SingleStepUnknownResult::String`, yet that would cause every
/// `SingleStepUnknownResult` to consume the parent's borrow for the rest of its lifetime, when we
/// only want to consume it upon `SingleStepUnknownResult::String`.
#[inline(always)]
fn handle_string_value<'read, 'parent, R: Read<'read>, S: Stack>(
  deserializer: &'parent mut Deserializer<'read, R, S>,
) -> StringValue<'read, 'parent, R, S> {
  StringValue(String::read(deserializer))
}

impl<'read, 'parent, R: Read<'read>, S: Stack> Field<'read, 'parent, R, S> {
  /// Access the iterator for the string used as the field's key.
  ///
  /// The iterator will yield the individual characters within the string represented by the JSON
  /// serialization, with all escape sequences handled.
  ///
  /// If the JSON underlying is invalid, the iterator will error, and while `Field::value` may
  /// still be called, all further attempted accesses will yield an error.
  ///
  /// If the JSON underlying is valid yet does not represent a valid UTF-8 sequence, the iterator
  /// will error, yet `Field::value` may still be called and deserialization may continue. The rest
  /// of the key will not be accessible however.
  #[inline(always)]
  pub fn key(
    &mut self,
  ) -> &mut (impl use<'read, 'parent, R, S> + Iterator<Item = Result<char, JsonError<'read, R, S>>>)
  {
    &mut self.key
  }
  /// Access the field's value.
  #[inline(always)]
  pub fn value(mut self) -> Value<'read, 'parent, R, S> {
    Value { deserializer: self.key.drop() }
  }
}

impl<'read, 'parent, R: Read<'read>, S: Stack> Drop for Field<'read, 'parent, R, S> {
  #[inline(always)]
  fn drop(&mut self) {
    drop(Value { deserializer: self.key.drop() });
  }
}

/// An iterator over fields.
pub struct FieldIterator<'read, 'parent, R: Read<'read>, S: Stack> {
  deserializer: &'parent mut Deserializer<'read, R, S>,
  done: bool,
}

// When this object is dropped, advance the decoder past the unread items
impl<'read, 'parent, R: Read<'read>, S: Stack> Drop for FieldIterator<'read, 'parent, R, S> {
  #[inline(always)]
  fn drop(&mut self) {
    if self.deserializer.error.is_some() {
      return;
    }

    loop {
      let Some(next) = self.next() else { break };
      let next = next.map(|_| ());
      match next {
        Ok(()) => {}
        Err(e) => {
          self.deserializer.error = Some(e);
          break;
        }
      }
    }
  }
}

impl<'read, 'parent, R: Read<'read>, S: Stack> FieldIterator<'read, 'parent, R, S> {
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
  pub fn next(&mut self) -> Option<Result<Field<'read, '_, R, S>, JsonError<'read, R, S>>> {
    if let Some(err) = self.deserializer.error {
      return Some(Err(err));
    }

    if self.done {
      None?;
    }

    loop {
      let result = match self.deserializer.single_step() {
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
pub struct ArrayIterator<'read, 'parent, R: Read<'read>, S: Stack> {
  deserializer: &'parent mut Deserializer<'read, R, S>,
  done: bool,
}

// When this array is dropped, advance the decoder past the unread items
impl<'read, 'parent, R: Read<'read>, S: Stack> Drop for ArrayIterator<'read, 'parent, R, S> {
  #[inline(always)]
  fn drop(&mut self) {
    if self.deserializer.error.is_some() {
      return;
    }

    loop {
      let Some(next) = self.next() else { break };
      let next = next.map(|_| ());
      match next {
        Ok(()) => {}
        Err(e) => {
          self.deserializer.error = Some(e);
          break;
        }
      }
    }
  }
}

impl<'read, 'parent, R: Read<'read>, S: Stack> ArrayIterator<'read, 'parent, R, S> {
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
  pub fn next(&mut self) -> Option<Result<Value<'read, '_, R, S>, JsonError<'read, R, S>>> {
    if let Some(err) = self.deserializer.error {
      return Some(Err(err));
    }

    if self.done {
      None?;
    }

    loop {
      let result = match self.deserializer.single_step() {
        Ok(SingleStepResult::Array(result)) => result,
        Ok(_) => break Some(Err(JsonError::InternalError)),
        Err(e) => break Some(Err(e)),
      };
      match result {
        SingleStepArrayResult::Value => {
          break Some(Ok(Value { deserializer: Some(self.deserializer) }));
        }
        SingleStepArrayResult::Closed => {
          self.done = true;
          None?
        }
      }
    }
  }
}

impl<'read, 'parent, R: Read<'read>, S: Stack> Value<'read, 'parent, R, S> {
  /// Get the type of the current item.
  ///
  /// This does not assert it's a valid instance of this class of items. It solely asserts if this
  /// is a valid item, it will be of this type.
  #[inline(always)]
  pub fn kind(&mut self) -> Result<Type, JsonError<'read, R, S>> {
    Ok(kind(&self.deserializer.as_ref().ok_or(JsonError::InternalError)?.reader))
  }

  /// Iterate over the fields within this object.
  ///
  /// If a field is present multiple times, this will yield each instance.
  #[inline(always)]
  pub fn fields(mut self) -> Result<FieldIterator<'read, 'parent, R, S>, JsonError<'read, R, S>> {
    if !matches!(self.kind()?, Type::Object) {
      Err(JsonError::TypeError)?
    }

    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match deserializer.single_step()? {
      SingleStepResult::Unknown(SingleStepUnknownResult::ObjectOpened) => {
        Ok(FieldIterator { deserializer, done: false })
      }
      _ => Err(JsonError::InternalError),
    }
  }

  /// Iterate over all items within this container.
  #[inline(always)]
  pub fn iterate(mut self) -> Result<ArrayIterator<'read, 'parent, R, S>, JsonError<'read, R, S>> {
    if !matches!(self.kind()?, Type::Array) {
      Err(JsonError::TypeError)?
    }

    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match deserializer.single_step()? {
      SingleStepResult::Unknown(SingleStepUnknownResult::ArrayOpened) => {
        Ok(ArrayIterator { deserializer, done: false })
      }
      _ => Err(JsonError::InternalError),
    }
  }

  /// Get the current item as a 'string'.
  ///
  /// As we cannot perform allocations, we do not yield a [`alloc::string::String`] but rather an
  /// iterator for the contents of the serialized string (with its escape sequences handled). This
  /// may be converted to an `String` with `.collect::<Result<String, _>>()?`.
  ///
  /// RFC 8259 allows strings to specify invalid UTF-8 codepoints. This library supports working
  /// with such values, as required to be compliant with RFC 8259, but this function's return value
  /// will error when attempting to return a non-UTF-8 value. If the underlying JSON is valid, the
  /// deserializer will remain usable afterwards however, even though the rest of the non-UTF-8
  /// string will be inaccesible. Please keep this detail in mind.
  #[inline(always)]
  pub fn to_str(
    mut self,
  ) -> Result<
    impl use<'read, 'parent, R, S> + Iterator<Item = Result<char, JsonError<'read, R, S>>>,
    JsonError<'read, R, S>,
  > {
    if !matches!(self.kind()?, Type::String) {
      Err(JsonError::TypeError)?
    }

    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match deserializer.single_step()? {
      SingleStepResult::Unknown(SingleStepUnknownResult::String) => {
        Ok(handle_string_value(deserializer))
      }
      _ => Err(JsonError::InternalError),
    }
  }

  /// Get the current item as a number.
  #[inline(always)]
  pub fn to_number(mut self) -> Result<Number, JsonError<'read, R, S>> {
    if !matches!(self.kind()?, Type::Number) {
      Err(JsonError::TypeError)?
    }

    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match deserializer.single_step()? {
      SingleStepResult::Unknown(SingleStepUnknownResult::Number(number)) => Ok(number),
      _ => Err(JsonError::InternalError),
    }
  }

  /// Get the current item as a `bool`.
  #[inline(always)]
  pub fn to_bool(mut self) -> Result<bool, JsonError<'read, R, S>> {
    if !matches!(self.kind()?, Type::Bool) {
      Err(JsonError::TypeError)?
    }

    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match deserializer.single_step()? {
      SingleStepResult::Unknown(SingleStepUnknownResult::Bool(bool)) => Ok(bool),
      _ => Err(JsonError::InternalError),
    }
  }

  /// Get the current item as `null`.
  ///
  /// The point of this method is to assert the value is `null` _and valid_. `kind` only tells the
  /// caller if it's a valid value, it will be `null`.
  #[inline(always)]
  pub fn to_null(mut self) -> Result<(), JsonError<'read, R, S>> {
    if !matches!(self.kind()?, Type::Null) {
      Err(JsonError::TypeError)?
    }

    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match deserializer.single_step()? {
      SingleStepResult::Unknown(SingleStepUnknownResult::Null) => Ok(()),
      _ => Err(JsonError::InternalError),
    }
  }
}
