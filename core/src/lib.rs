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

pub use io::BytesLike;
pub use stack::*;
use string::*;
pub use number::{NumberSink, Number};

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

/// Advance the bytes until there's a non-whitespace character.
#[inline(always)]
fn advance_whitespace<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &mut B,
) -> Result<(), JsonError<'bytes, B, S>> {
  loop {
    let next = bytes.peek(0).map_err(JsonError::BytesError)?;
    // https://datatracker.ietf.org/doc/html/rfc8259#section-2 defines whitespace as follows
    if !matches!(next, b'\x20' | b'\x09' | b'\x0A' | b'\x0D') {
      break;
    }
    bytes.advance(1).map_err(JsonError::BytesError)?;
  }
  Ok(())
}

/// Advance past a comma, or to the close of the structure.
fn advance_past_comma_or_to_close<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &mut B,
) -> Result<(), JsonError<'bytes, B, S>> {
  advance_whitespace(bytes)?;
  match bytes.peek(0).map_err(JsonError::BytesError)? {
    b',' => {
      bytes.advance(1).map_err(JsonError::BytesError)?;
      advance_whitespace(bytes)?;
      if matches!(bytes.peek(0).map_err(JsonError::BytesError)?, b']' | b'}') {
        Err(JsonError::TrailingComma)?;
      }
    }
    b']' | b'}' => {}
    _ => Err(JsonError::InvalidValue)?,
  }
  Ok(())
}

/// The result from a single step of the deserialized, if within an object.
enum SingleStepObjectResult<'bytes, B: BytesLike<'bytes>, S: Stack> {
  /// A field within the object was advanced to.
  Field {
    /// The key for this field.
    key: String<'bytes, B, S>,
  },
  /// The object was closed.
  Closed,
}

/// The result from a single step of the deserialized, if within an array.
enum SingleStepArrayResult {
  /// A value within the array was advanced to.
  Value,
  /// The array was closed.
  Closed,
}

/// The result from a single step of the deserializer, if handling an unknown value.
enum SingleStepUnknownResult {
  /// An object was opened.
  ObjectOpened,
  /// An array was opened.
  ArrayOpened,
  /// A string was read.
  String,
  /// A number was read.
  Number(Number),
  /// A boolean value was advanced past.
  Bool(bool),
  /// Null was advanced past.
  Null,
}

/// The result from a single step of the deserializer.
enum SingleStepResult<'bytes, B: BytesLike<'bytes>, S: Stack> {
  /// The result if within an object.
  Object(SingleStepObjectResult<'bytes, B, S>),
  /// The result if within an array.
  Array(SingleStepArrayResult),
  /// The result if handling an unknown value.
  Unknown(SingleStepUnknownResult),
}

/// Step the deserializer forwards.
///
/// This assumes there is no leading whitespace present in `bytes` and will advance past any
/// whitespace present before the next logical unit.
fn single_step<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
  bytes: &'parent mut B,
  stack: &'parent mut S,
) -> Result<SingleStepResult<'bytes, B, S>, JsonError<'bytes, B, S>> {
  match stack.peek().ok_or(JsonError::InternalError)? {
    State::Object => {
      let next = bytes.read_byte().map_err(JsonError::BytesError)?;

      // Check if the object terminates
      if next == b'}' {
        stack.pop().ok_or(JsonError::InternalError)?;

        // If this isn't the outer object, advance past the comma after
        if stack.depth() != 0 {
          advance_past_comma_or_to_close(bytes)?;
        }

        return Ok(SingleStepResult::Object(SingleStepObjectResult::Closed));
      }

      // Read the name of this field
      if next != b'"' {
        Err(JsonError::InvalidKey)?;
      }
      let key = read_string(bytes)?;

      // Read the colon delimiter
      advance_whitespace::<_, S>(bytes)?;
      if bytes.read_byte().map_err(JsonError::BytesError)? != b':' {
        Err(JsonError::InvalidKeyValueDelimiter)?;
      }

      // Push how we're reading a value of an unknown type onto the stack
      advance_whitespace::<_, S>(bytes)?;
      stack.push(State::Unknown).map_err(JsonError::StackError)?;
      Ok(SingleStepResult::Object(SingleStepObjectResult::Field { key }))
    }
    State::Array => {
      // Check if the array terminates
      if bytes.peek(0).map_err(JsonError::BytesError)? == b']' {
        stack.pop().ok_or(JsonError::InternalError)?;
        bytes.advance(1).map_err(JsonError::BytesError)?;

        // If this isn't the outer object, advance past the comma after
        if stack.depth() != 0 {
          advance_past_comma_or_to_close(bytes)?;
        }

        return Ok(SingleStepResult::Array(SingleStepArrayResult::Closed));
      }

      // Since the array doesn't terminate, read the next value
      stack.push(State::Unknown).map_err(JsonError::StackError)?;
      Ok(SingleStepResult::Array(SingleStepArrayResult::Value))
    }
    State::Unknown => {
      stack.pop().ok_or(JsonError::InternalError)?;

      let result = match kind(bytes)? {
        // Handle if this opens an object
        Type::Object => {
          bytes.advance(1).map_err(JsonError::BytesError)?;
          advance_whitespace(bytes)?;
          stack.push(State::Object).map_err(JsonError::StackError)?;
          return Ok(SingleStepResult::Unknown(SingleStepUnknownResult::ObjectOpened));
        }
        // Handle if this opens an array
        Type::Array => {
          bytes.advance(1).map_err(JsonError::BytesError)?;
          advance_whitespace(bytes)?;
          stack.push(State::Array).map_err(JsonError::StackError)?;
          return Ok(SingleStepResult::Unknown(SingleStepUnknownResult::ArrayOpened));
        }
        // Handle if this opens an string
        Type::String => {
          bytes.advance(1).map_err(JsonError::BytesError)?;
          return Ok(SingleStepResult::Unknown(SingleStepUnknownResult::String));
        }
        Type::Number => {
          SingleStepResult::Unknown(SingleStepUnknownResult::Number(number::to_number_str(bytes)?))
        }
        Type::Bool => {
          let mut bool_string = [0; 5];
          bytes.read_into_slice(&mut bool_string[.. 4]).map_err(JsonError::BytesError)?;
          let bool = if &bool_string[.. 4] == b"true" {
            true
          } else {
            bytes.read_into_slice(&mut bool_string[4 ..]).map_err(JsonError::BytesError)?;
            if bool_string != *b"false" {
              Err(JsonError::TypeError)?;
            }
            false
          };
          SingleStepResult::Unknown(SingleStepUnknownResult::Bool(bool))
        }
        Type::Null => {
          let mut null_string = [0; 4];
          bytes.read_into_slice(&mut null_string).map_err(JsonError::BytesError)?;
          if null_string != *b"null" {
            Err(JsonError::TypeError)?;
          }
          SingleStepResult::Unknown(SingleStepUnknownResult::Null)
        }
      };

      // We now have to read past the next comma, or to the next closing of a structure
      advance_past_comma_or_to_close(bytes)?;

      Ok(result)
    }
  }
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

/// A deserializer for a JSON-encoded structure.
pub struct Deserializer<'bytes, B: BytesLike<'bytes>, S: Stack> {
  bytes: B,
  stack: S,
  /*
    We advance the deserializer within `Drop` which cannot return an error. If an error is raised
    within drop, we store it here to be consumed upon the next call to a method which can return an
    error (if one is ever called).
  */
  error: Option<JsonError<'bytes, B, S>>,
}

/// A JSON value.
// Internally, we assume whenever this is held, the top item on the stack is `State::Unknown`
pub struct Value<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> {
  deserializer: Option<&'parent mut Deserializer<'bytes, B, S>>,
}

impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> Drop for Value<'bytes, 'parent, B, S> {
  fn drop(&mut self) {
    /*
      When this value is dropped, we advance the deserializer past it if it hasn't already been
      converted into a `FieldIterator` or `ArrayIterator` (which each have their own `Drop`
      implementations).
    */
    if let Some(deserializer) = self.deserializer.take() {
      if deserializer.error.is_some() {
        return;
      }

      let Some(current) = deserializer.stack.peek() else {
        deserializer.error = Some(JsonError::InternalError);
        return;
      };

      let mut depth = match current {
        State::Object | State::Array => 1,
        State::Unknown => {
          let step = match single_step(&mut deserializer.bytes, &mut deserializer.stack) {
            Ok(SingleStepResult::Unknown(step)) => step,
            Ok(_) => {
              deserializer.error = Some(JsonError::InternalError);
              return;
            }
            Err(e) => {
              deserializer.error = Some(e);
              return;
            }
          };
          match step {
            // We successfully advanced past this item
            SingleStepUnknownResult::Number(_) |
            SingleStepUnknownResult::Bool(_) |
            SingleStepUnknownResult::Null => return,
            // We opened a string we now have to handle
            SingleStepUnknownResult::String => {
              handle_string_value(deserializer);
              return;
            }
            // We opened an object/array we now have to advance past
            SingleStepUnknownResult::ObjectOpened | SingleStepUnknownResult::ArrayOpened => 1,
          }
        }
      };

      // Since our object isn't a unit, step the deserializer until it's advanced past
      while depth != 0 {
        let step = match single_step(&mut deserializer.bytes, &mut deserializer.stack) {
          Ok(step) => step,
          Err(e) => {
            deserializer.error = Some(e);
            return;
          }
        };
        match step {
          SingleStepResult::Object(SingleStepObjectResult::Closed) |
          SingleStepResult::Array(SingleStepArrayResult::Closed) => depth -= 1,
          SingleStepResult::Unknown(SingleStepUnknownResult::String) => {
            handle_string_value(deserializer);
          }
          SingleStepResult::Unknown(
            SingleStepUnknownResult::ObjectOpened | SingleStepUnknownResult::ArrayOpened,
          ) => depth += 1,
          _ => {}
        }
      }
    }
  }
}

impl<'bytes, B: BytesLike<'bytes>, S: Stack> Deserializer<'bytes, B, S> {
  /// Create a new deserializer.
  pub fn new(mut bytes: B) -> Result<Self, JsonError<'bytes, B, S>> {
    advance_whitespace(&mut bytes)?;

    let mut stack = S::empty();
    stack.push(State::Unknown).map_err(JsonError::StackError)?;

    Ok(Deserializer { bytes, stack, error: None })
  }

  /// Obtain the `Value` representing the serialized structure.
  ///
  /// This takes a mutable reference as `Deserializer` is the owned object representing the
  /// deserializer's state. However, this is not eligible to be called more than once, even after
  /// the initial mutable borrow is dropped. Multiple calls to this function will cause an error to
  /// be returned.
  #[inline(always)]
  pub fn value(&mut self) -> Result<Value<'bytes, '_, B, S>, JsonError<'bytes, B, S>> {
    if self.stack.depth() != 1 {
      Err(JsonError::ReusedDeserializer)?;
    }
    let result = Value { deserializer: Some(self) };
    if !matches!(result.kind()?, Type::Object | Type::Array) {
      Err(JsonError::TypeError)?;
    }
    Ok(result)
  }
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
      let result = match single_step(&mut self.deserializer.bytes, &mut self.deserializer.stack) {
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
      let result = match single_step(&mut self.deserializer.bytes, &mut self.deserializer.stack) {
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

    match single_step(&mut deserializer.bytes, &mut deserializer.stack)? {
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

    match single_step(&mut deserializer.bytes, &mut deserializer.stack)? {
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
    match single_step(&mut deserializer.bytes, &mut deserializer.stack)? {
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
    match single_step(&mut deserializer.bytes, &mut deserializer.stack)? {
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
    match single_step(&mut deserializer.bytes, &mut deserializer.stack)? {
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
    match single_step(&mut deserializer.bytes, &mut deserializer.stack)? {
      SingleStepResult::Unknown(SingleStepUnknownResult::Null) => Ok(()),
      _ => Err(JsonError::InternalError),
    }
  }
}
