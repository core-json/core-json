#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod io;
mod stack;
mod string;
mod string2;
mod number;
mod deserializer;

pub use io::BytesLike;
pub use stack::*;
use string::*;
pub use number::{NumberSink, Number};
pub use deserializer::{Deserializer, Value};
use deserializer::*;

/// An error incurred when deserializing.
#[derive(Debug)]
pub enum JsonError<'bytes, B: BytesLike<'bytes>, S: Stack> {
  /// An unexpected state was reached during deserialization.
  InternalError,
  /// An error from the bytes.
  BytesError(B::Error),
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
  /// The JSON had a trailing comma.
  TrailingComma,
  /// The JSON had mismatched delimiters between the open and close of the structure.
  MismatchedDelimiter,
  /// Operation could not be performed given the value's type.
  TypeError,
}
impl<'bytes, B: BytesLike<'bytes>, S: Stack> Clone for JsonError<'bytes, B, S> {
  #[inline(always)]
  fn clone(&self) -> Self {
    *self
  }
}
impl<'bytes, B: BytesLike<'bytes>, S: Stack> Copy for JsonError<'bytes, B, S> {}

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
fn kind<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &B,
) -> Result<Type, JsonError<'bytes, B, S>> {
  Ok(match bytes.peek(0).map_err(JsonError::BytesError)? {
    b'{' => Type::Object,
    b'[' => Type::Array,
    b'"' => Type::String,
    b't' | b'f' => Type::Bool,
    b'n' => Type::Null,
    _ => Type::Number,
  })
}

/// Handle a string value.
///
/// This MUST be called every time a `SingleStepUnknownResult::String` is yielded, unless the
/// deserializer is immediately terminated (such as by setting its error to `Some(_)`). This
/// ideally would be yielded within `SingleStepUnknownResult::String`, yet that would cause every
/// `SingleStepUnknownResult` to consume the parent's borrow for the rest of its lifetime, when we
/// only want to consume it upon `SingleStepUnknownResult::String`.
fn handle_string_value<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
  deserializer: &'parent mut Deserializer<'bytes, B, S>,
) -> string2::StringValue<'bytes, 'parent, B, S> {
  string2::StringValue(string2::String::read(deserializer))
}

/// An iterator over fields.
pub struct FieldIterator<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> {
  deserializer: &'parent mut Deserializer<'bytes, B, S>,
  done: bool,
}

// When this object is dropped, advance the decoder past the unread items
impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> Drop
  for FieldIterator<'bytes, 'parent, B, S>
{
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

impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> FieldIterator<'bytes, 'parent, B, S> {
  /// The next entry (key, value) within the object.
  ///
  /// The key is presented as an iterator over the characters within the serialized string, with
  /// the escape sequences handled. If the key specifies invalid UTF characters, the iterator will
  /// yield an error when it attempts to parse them. While it may not be possible to parse a key
  /// as UTF characters, decoding of this field's value (and the rest of the structure) is still
  /// possible (even _after_ the iterator yields its error). For more information, please refer to
  /// [`Value::to_str`].
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
  pub fn next(
    &mut self,
  ) -> Option<
    Result<
      (
        impl use<'bytes, B, S> + Iterator<Item = Result<char, JsonError<'bytes, B, S>>>,
        Value<'bytes, '_, B, S>,
      ),
      JsonError<'bytes, B, S>,
    >,
  > {
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
        SingleStepObjectResult::Field { key } => {
          break Some(Ok((key, Value { deserializer: Some(self.deserializer) })))
        }
        SingleStepObjectResult::Closed => {
          self.done = true;
          None?
        }
      }
    }
  }
}

/// An iterator over an array.
pub struct ArrayIterator<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> {
  deserializer: &'parent mut Deserializer<'bytes, B, S>,
  done: bool,
}

// When this array is dropped, advance the decoder past the unread items
impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> Drop
  for ArrayIterator<'bytes, 'parent, B, S>
{
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

impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> ArrayIterator<'bytes, 'parent, B, S> {
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
  pub fn next(&mut self) -> Option<Result<Value<'bytes, '_, B, S>, JsonError<'bytes, B, S>>> {
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
          break Some(Ok(Value { deserializer: Some(self.deserializer) }))
        }
        SingleStepArrayResult::Closed => {
          self.done = true;
          None?
        }
      }
    }
  }
}

impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> Value<'bytes, 'parent, B, S> {
  /// Get the type of the current item.
  ///
  /// This does not assert it's a valid instance of this class of items. It solely asserts if this
  /// is a valid item, it will be of this type.
  #[inline(always)]
  pub fn kind(&self) -> Result<Type, JsonError<'bytes, B, S>> {
    kind(&self.deserializer.as_ref().ok_or(JsonError::InternalError)?.bytes)
  }

  /// Iterate over the fields within this object.
  ///
  /// If a field is present multiple times, this will yield each instance.
  pub fn fields(mut self) -> Result<FieldIterator<'bytes, 'parent, B, S>, JsonError<'bytes, B, S>> {
    if !matches!(self.kind()?, Type::Object) {
      Err(JsonError::TypeError)?
    }

    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    if let Some(err) = deserializer.error {
      Err(err)?;
    }

    match deserializer.single_step()? {
      SingleStepResult::Unknown(SingleStepUnknownResult::ObjectOpened) => {
        Ok(FieldIterator { deserializer, done: false })
      }
      _ => Err(JsonError::InternalError),
    }
  }

  /// Iterate over all items within this container.
  pub fn iterate(
    mut self,
  ) -> Result<ArrayIterator<'bytes, 'parent, B, S>, JsonError<'bytes, B, S>> {
    if !matches!(self.kind()?, Type::Array) {
      Err(JsonError::TypeError)?
    }

    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    if let Some(err) = deserializer.error {
      Err(err)?;
    }

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
  /// will error when attempting to return a non-UTF-8 value. Please keep this subtlety in mind.
  #[inline(always)]
  pub fn to_str(
    mut self,
  ) -> Result<
    impl use<'bytes, 'parent, B, S> + Iterator<Item = Result<char, JsonError<'bytes, B, S>>>,
    JsonError<'bytes, B, S>,
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
  pub fn to_number(mut self) -> Result<Number, JsonError<'bytes, B, S>> {
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
  pub fn to_bool(mut self) -> Result<bool, JsonError<'bytes, B, S>> {
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
  pub fn to_null(mut self) -> Result<(), JsonError<'bytes, B, S>> {
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
