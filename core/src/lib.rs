#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]
#![deny(missing_docs)]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

mod io;
mod stack;
mod string;
mod number;

pub(crate) use io::*;
pub use io::BytesLike;
pub use stack::*;
use string::*;

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

/// Interpret the immediate value within the bytes as a `bool`.
#[inline(always)]
pub fn as_bool<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &B,
) -> Result<bool, JsonError<'bytes, B, S>> {
  let first = bytes.peek(0).map_err(JsonError::BytesError)?;
  // Return early if this definitely isn't a valid `bool`
  if !matches!(first, b't' | b'f') {
    Err(JsonError::TypeError)?;
  }
  let second = bytes.peek(1).map_err(JsonError::BytesError)?;
  let third = bytes.peek(2).map_err(JsonError::BytesError)?;
  let fourth = bytes.peek(3).map_err(JsonError::BytesError)?;
  let fifth = bytes.peek(4).map_err(JsonError::BytesError)?;

  let is_true = (first, second, third, fourth) == (b't', b'r', b'u', b'e');
  let is_false = (first, second, third, fourth, fifth) == (b'f', b'a', b'l', b's', b'e');

  if !(is_true || is_false) {
    Err(JsonError::TypeError)?;
  }

  Ok(is_true)
}

/// Check if the immediate value within the bytes is `null`.
#[inline(always)]
pub fn is_null<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &B,
) -> Result<bool, JsonError<'bytes, B, S>> {
  let first = bytes.peek(0).map_err(JsonError::BytesError)?;
  if first != b'n' {
    return Ok(false);
  }
  let second = bytes.peek(1).map_err(JsonError::BytesError)?;
  let third = bytes.peek(2).map_err(JsonError::BytesError)?;
  let fourth = bytes.peek(3).map_err(JsonError::BytesError)?;

  if (second, third, fourth) != (b'u', b'l', b'l') {
    Err(JsonError::InvalidValue)?;
  }
  Ok(true)
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
enum SingleStepObjectResult<'bytes, B: BytesLike<'bytes>> {
  /// A field within the object was advanced to.
  Field {
    /// The key for this field.
    key: String<'bytes, B>,
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
enum SingleStepUnknownResult<'bytes, B: BytesLike<'bytes>> {
  /// An object was opened.
  ObjectOpened,
  /// An array was opened.
  ArrayOpened,
  /// A string was read.
  String(String<'bytes, B>),
  /// A unit value was advanced past.
  Advanced,
}

/// The result from a single step of the deserializer.
enum SingleStepResult<'bytes, B: BytesLike<'bytes>> {
  /// The result if within an object.
  Object(SingleStepObjectResult<'bytes, B>),
  /// The result if within an array.
  Array(SingleStepArrayResult),
  /// The result if handling an unknown value.
  Unknown(SingleStepUnknownResult<'bytes, B>),
}

/// Step the deserializer forwards.
///
/// This assumes there is no leading whitespace present in `bytes` and will advance past any
/// whitespace present before the next logical unit.
fn single_step<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
  bytes: &'parent mut B,
  stack: &'parent mut S,
) -> Result<SingleStepResult<'bytes, B>, JsonError<'bytes, B, S>> {
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

      let mut result = SingleStepResult::Unknown(SingleStepUnknownResult::Advanced);
      match bytes.peek(0).map_err(JsonError::BytesError)? {
        // Handle if this opens an object
        b'{' => {
          bytes.advance(1).map_err(JsonError::BytesError)?;
          advance_whitespace(bytes)?;
          stack.push(State::Object).map_err(JsonError::StackError)?;
          return Ok(SingleStepResult::Unknown(SingleStepUnknownResult::ObjectOpened));
        }
        // Handle if this opens an array
        b'[' => {
          bytes.advance(1).map_err(JsonError::BytesError)?;
          advance_whitespace(bytes)?;
          stack.push(State::Array).map_err(JsonError::StackError)?;
          return Ok(SingleStepResult::Unknown(SingleStepUnknownResult::ArrayOpened));
        }
        // Handle if this opens an string
        b'"' => {
          bytes.advance(1).map_err(JsonError::BytesError)?;
          // Read past the string
          result = SingleStepResult::Unknown(SingleStepUnknownResult::String(read_string(bytes)?));
        }
        // This is a distinct unit value
        _ => {
          // https://datatracker.ietf.org/doc/html/rfc8259#section-3 defines all possible values
          let is_number = match number::is_number_str(bytes) {
            Ok(len) => Some(len),
            Err(JsonError::TypeError) => None,
            Err(e) => Err(e)?,
          };
          let is_bool = match as_bool(bytes) {
            Ok(value) => Some(if value { 4 } else { 5 }),
            Err(JsonError::TypeError) => None,
            Err(e) => Err(e)?,
          };
          let is_null = match is_null(bytes) {
            Ok(is_null) => {
              if is_null {
                Some(4)
              } else {
                None
              }
            }
            Err(e) => Err(e)?,
          };

          if let Some(len) = is_number.or(is_bool).or(is_null) {
            bytes.advance(len).map_err(JsonError::BytesError)?;
          } else {
            Err(JsonError::InvalidValue)?;
          }
        }
      }

      // We now have to read past the next comma, or to the next closing of a structure
      advance_past_comma_or_to_close(bytes)?;

      Ok(result)
    }
  }
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
            SingleStepUnknownResult::String(_) | SingleStepUnknownResult::Advanced => return,
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
    if !(result.is_object()? || result.is_array()?) {
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
          break Some(Ok((
            UnescapeString::from(key),
            Value { deserializer: Some(self.deserializer) },
          )))
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
  /// Check if the current item is an object.
  #[inline(always)]
  pub fn is_object(&self) -> Result<bool, JsonError<'bytes, B, S>> {
    Ok(
      self
        .deserializer
        .as_ref()
        .ok_or(JsonError::InternalError)?
        .bytes
        .peek(0)
        .map_err(JsonError::BytesError)? ==
        b'{',
    )
  }

  /// Iterate over the fields within this object.
  ///
  /// If a field is present multiple times, this will yield each instance.
  pub fn fields(mut self) -> Result<FieldIterator<'bytes, 'parent, B, S>, JsonError<'bytes, B, S>> {
    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    if let Some(err) = deserializer.error {
      Err(err)?;
    }

    match single_step(&mut deserializer.bytes, &mut deserializer.stack)? {
      SingleStepResult::Unknown(SingleStepUnknownResult::ObjectOpened) => {
        Ok(FieldIterator { deserializer, done: false })
      }
      _ => Err(JsonError::TypeError),
    }
  }

  /// Check if the current item is an array.
  #[inline(always)]
  pub fn is_array(&self) -> Result<bool, JsonError<'bytes, B, S>> {
    Ok(
      self
        .deserializer
        .as_ref()
        .ok_or(JsonError::InternalError)?
        .bytes
        .peek(0)
        .map_err(JsonError::BytesError)? ==
        b'[',
    )
  }

  /// Iterate over all items within this container.
  pub fn iterate(
    mut self,
  ) -> Result<ArrayIterator<'bytes, 'parent, B, S>, JsonError<'bytes, B, S>> {
    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    if let Some(err) = deserializer.error {
      Err(err)?;
    }

    match single_step(&mut deserializer.bytes, &mut deserializer.stack)? {
      SingleStepResult::Unknown(SingleStepUnknownResult::ArrayOpened) => {
        Ok(ArrayIterator { deserializer, done: false })
      }
      _ => Err(JsonError::TypeError),
    }
  }

  /// Check if the current item is a string.
  #[inline(always)]
  pub fn is_str(&self) -> Result<bool, JsonError<'bytes, B, S>> {
    Ok(
      self
        .deserializer
        .as_ref()
        .ok_or(JsonError::InternalError)?
        .bytes
        .peek(0)
        .map_err(JsonError::BytesError)? ==
        b'"',
    )
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
    impl use<'bytes, B, S> + Iterator<Item = Result<char, JsonError<'bytes, B, S>>>,
    JsonError<'bytes, B, S>,
  > {
    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match single_step(&mut deserializer.bytes, &mut deserializer.stack)? {
      SingleStepResult::Unknown(SingleStepUnknownResult::String(str)) => {
        Ok(UnescapeString::from(str))
      }
      _ => Err(JsonError::TypeError),
    }
  }

  /// Get the current item as an `i64`.
  ///
  /// This uses the definition of a number defined in RFC 8259, then constrains it to having no
  /// fractional, exponent parts. Then, it's yielded if it's representable within an `i64`.
  ///
  /// This is _exact_. It does not go through `f64` and does not experience its approximations.
  #[inline(always)]
  pub fn as_i64(&self) -> Result<i64, JsonError<'bytes, B, S>> {
    let bytes = &self.deserializer.as_ref().ok_or(JsonError::InternalError)?.bytes;

    // -9223372036854775808
    const MAX_I64_LEN: usize = 20;
    let len = number::is_number_str(bytes)?;
    if len > MAX_I64_LEN {
      Err(JsonError::TypeError)?;
    }

    let mut str = [0; MAX_I64_LEN];
    #[allow(clippy::needless_range_loop)]
    for i in 0 .. len {
      let byte = bytes.peek(i).map_err(JsonError::BytesError)?;
      if matches!(byte, b'.' | b'e' | b'E') {
        Err(JsonError::TypeError)?;
      }
      str[i] = byte;
    }
    let str = core::str::from_utf8(&str[.. len]).map_err(|_| JsonError::InternalError)?;
    <i64 as core::str::FromStr>::from_str(str).map_err(|_| JsonError::TypeError)
  }

  /// Get the current item as an `f64`.
  ///
  /// This may be lossy due to the inherent nature of floats combined with Rust's bounds on
  /// precision.
  pub fn as_f64(&self) -> Result<f64, JsonError<'bytes, B, S>> {
    let bytes = &self.deserializer.as_ref().ok_or(JsonError::InternalError)?.bytes;

    /*
      The syntax for this (expanded) is
      `[ minus ] int [ decimal-point 1*DIGIT ] [ e [ minus / plus ] 1*DIGIT ]`.

      https://datatracker.ietf.org/doc/html/rfc8259#section-6 lets us specify the range, precision
      of numbers.

      We bind `minus`, `decimal-point`, `e`, `plus` to a maximum length of 1 as they definitively
      have a length of 1. We bind the integer part to a maximum length of `f64::MAX_10_EXP + 1`,
      the maximum length of a normal integer. We bind the fractional part to `f64::DIGITS`, the
      amount of significant digits Rust can definitely convert back/forth with a base-10
      serialization without loss of information. We bind the exponent to four digits as `f64` has
      a maximum exponent `1000 < e < 2000`.
    */
    const MAX_DOUBLE_LEN: usize =
      1 + ((f64::MAX_10_EXP as usize) + 1) + 1 + (f64::DIGITS as usize) + 1 + 1 + 4;
    let len = number::is_number_str(bytes)?;

    let mut src = 0;
    let mut dst = 0;
    let mut str = [0; MAX_DOUBLE_LEN];
    let mut found_non_zero_digit = false;
    let mut found_decimal = false;
    let mut insignificant_digits = None;
    #[allow(clippy::explicit_counter_loop)]
    for i in 0 .. len {
      let byte = bytes.peek(src).map_err(JsonError::BytesError)?;
      src += 1;

      /*
        If we've found the leading digit, and this is within the decimal component, declare where
        the insignificant digits begin.
      */
      if matches!(byte, b'1' ..= b'9') {
        found_non_zero_digit = true;
        if found_decimal {
          insignificant_digits = i.checked_add(f64::DIGITS as usize - 1);
        }
      }

      // If this is the opening of the fractional part, note the index it was opened at
      if byte == b'.' {
        found_decimal = true;
        if found_non_zero_digit {
          insignificant_digits = i.checked_add(f64::DIGITS as usize);
        }
      }

      /*
        If this is a fractional part with more significant digits than we can reasonably handle,
        ignore the rest. This lets us handle serializations with greater precision, whose
        serialization length exceeds our limit, so as long as the integer, exponent terms are
        sufficiently bounded.
      */
      if let Some(insignificant_digits) = insignificant_digits {
        if (i > insignificant_digits) && byte.is_ascii_digit() {
          continue;
        }
      }

      // If this an exponent, reset `insignificant_digits` so we start handling digits again
      if matches!(byte, b'e' | b'E') {
        insignificant_digits = None;
      }

      if dst == str.len() {
        Err(JsonError::TypeError)?;
      }
      // Copy into the byte buffer for the string
      str[dst] = byte;
      dst += 1;
    }
    let str = core::str::from_utf8(&str[.. len]).map_err(|_| JsonError::InternalError)?;
    <f64 as core::str::FromStr>::from_str(str).map_err(|_| JsonError::TypeError)
  }

  /// Get the current item as a `bool`.
  #[inline(always)]
  pub fn as_bool(&self) -> Result<bool, JsonError<'bytes, B, S>> {
    as_bool(&self.deserializer.as_ref().ok_or(JsonError::InternalError)?.bytes)
  }

  /// Check if the current item is `null`.
  #[inline(always)]
  pub fn is_null(&self) -> Result<bool, JsonError<'bytes, B, S>> {
    is_null(&self.deserializer.as_ref().ok_or(JsonError::InternalError)?.bytes)
  }
}
