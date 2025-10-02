#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc = include_str!("../../README.md")]
#![deny(missing_docs)]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

mod io;
mod stack;

pub(crate) use io::*;
pub use io::BytesLike;
pub use stack::*;

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
  /// The serialization was not valid UTF-8.
  ///
  /// Serializations are not checked to be valid UTF-8. Incorrect UTF-8 may be detected and raise
  /// this error however.
  NotUtf8,
  /// The JSON had an invalid key.
  InvalidKey,
  /// The JSON had an invalid delimiter between the key and value (`:` expected).
  InvalidKeyValueDelimiter,
  /// The JSON had mismatched delimiters between the open and close of the structure.
  MismatchedDelimiter,
  /// Operation could not be performed given the value's type.
  TypeError,
}
impl<'bytes, B: BytesLike<'bytes>, S: Stack> Clone for JsonError<'bytes, B, S> {
  fn clone(&self) -> Self {
    *self
  }
}
impl<'bytes, B: BytesLike<'bytes>, S: Stack> Copy for JsonError<'bytes, B, S> {}

fn advance_whitespace<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &mut B,
) -> Result<(), JsonError<'bytes, B, S>> {
  loop {
    let mut utf8_codepoint = [0; 4];
    utf8_codepoint[0] = bytes.peek(0).map_err(JsonError::BytesError)?;
    let utf8_codepoint_len = usize::from({
      let first_bit_set = (utf8_codepoint[0] & (1 << 7)) != 0;
      let third_bit_set = (utf8_codepoint[0] & (1 << 5)) != 0;
      let fourth_bit_set = (utf8_codepoint[0] & (1 << 4)) != 0;
      1u8 +
        u8::from(first_bit_set) +
        u8::from(first_bit_set & third_bit_set) +
        u8::from(first_bit_set & third_bit_set & fourth_bit_set)
    });
    let utf8_codepoint = &mut utf8_codepoint[.. utf8_codepoint_len];
    for (i, byte) in utf8_codepoint[1 ..].iter_mut().enumerate() {
      *byte = bytes.peek(i).map_err(JsonError::BytesError)?;
    }

    let str = core::str::from_utf8(utf8_codepoint).map_err(|_| JsonError::NotUtf8)?;
    if !str.chars().next().ok_or(JsonError::InternalError)?.is_whitespace() {
      break;
    }
    bytes.advance(utf8_codepoint_len).map_err(JsonError::BytesError)?;
  }

  Ok(())
}

/// The result from a single step of the deserializer.
enum SingleStepResult<'bytes, B: BytesLike<'bytes>> {
  /// A field within an object was advanced to.
  Field {
    /// The key for this field.
    key: String<'bytes, B>,
  },
  /// A value within an array was advanced to.
  ArrayValue,
  /// An object was opened.
  ObjectOpened,
  /// An array was opened.
  ArrayOpened,
  /// A string was read.
  String(String<'bytes, B>),
  /// A unit value was advanced past.
  Advanced,
  /// An open structure was closed.
  Closed,
}

/// Step the deserializer forwards.
fn single_step<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
  bytes: &'parent mut B,
  stack: &'parent mut S,
) -> Result<SingleStepResult<'bytes, B>, JsonError<'bytes, B, S>> {
  match stack.peek().ok_or(JsonError::InternalError)? {
    State::Object => {
      advance_whitespace::<_, S>(bytes)?;
      let next = bytes.read_byte().map_err(JsonError::BytesError)?;

      // Check if the object terminates
      if next == b'}' {
        stack.pop().ok_or(JsonError::InternalError)?;
        return Ok(SingleStepResult::Closed);
      }

      // Read the name of this field
      if next != b'"' {
        Err(JsonError::InvalidKey)?;
      }
      let key = read_string(bytes).map_err(JsonError::BytesError)?;

      // Read the colon delimiter
      advance_whitespace::<_, S>(bytes)?;
      if bytes.read_byte().map_err(JsonError::BytesError)? != b':' {
        Err(JsonError::InvalidKeyValueDelimiter)?;
      }

      // Push how we're reading a value of an unknown type onto the stack
      advance_whitespace::<_, S>(bytes)?;
      stack.push(State::Unknown).map_err(JsonError::StackError)?;
      Ok(SingleStepResult::Field { key })
    }
    State::Array => {
      advance_whitespace::<_, S>(bytes)?;

      // Check if the array terminates
      if bytes.peek(0).map_err(JsonError::BytesError)? == b']' {
        stack.pop().ok_or(JsonError::InternalError)?;
        bytes.advance(1).map_err(JsonError::BytesError)?;
        return Ok(SingleStepResult::Closed);
      }

      // Since the array doesn't terminate, read the next value
      stack.push(State::Unknown).map_err(JsonError::StackError)?;
      Ok(SingleStepResult::ArrayValue)
    }
    State::Unknown => {
      let mut result = SingleStepResult::Advanced;
      match bytes.peek(0).map_err(JsonError::BytesError)? {
        // Handle if this opens an object
        b'{' => {
          bytes.advance(1).map_err(JsonError::BytesError)?;
          stack.push(State::Object).map_err(JsonError::StackError)?;
          return Ok(SingleStepResult::ObjectOpened);
        }
        // Handle if this opens an array
        b'[' => {
          bytes.advance(1).map_err(JsonError::BytesError)?;
          stack.push(State::Array).map_err(JsonError::StackError)?;
          return Ok(SingleStepResult::ArrayOpened);
        }
        // Handle if this opens an string
        b'"' => {
          bytes.advance(1).map_err(JsonError::BytesError)?;
          // Read past the string
          result = SingleStepResult::String(read_string(bytes).map_err(JsonError::BytesError)?);
        }
        // This is a distinct unit value
        _ => {}
      }

      // We now have to read past the next comma, or to the next closing of a structure
      loop {
        match bytes.peek(0).map_err(JsonError::BytesError)? {
          b',' => {
            bytes.advance(1).map_err(JsonError::BytesError)?;
            break;
          }
          b']' | b'}' => break,
          _ => bytes.advance(1).map_err(JsonError::BytesError)?,
        }
      }
      stack.pop().ok_or(JsonError::InternalError)?;

      Ok(result)
    }
  }
}

/// A deserializer for a JSON-encoded structure.
pub struct Deserializer<'bytes, B: BytesLike<'bytes>, S: Stack> {
  bytes: B,
  stack: S,
  error: Option<JsonError<'bytes, B, S>>,
}

/// A JSON value.
pub struct Value<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> {
  deserializer: Option<&'parent mut Deserializer<'bytes, B, S>>,
}

// When this value is dropped, advance the deserializer past it
impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> Drop for Value<'bytes, 'parent, B, S> {
  fn drop(&mut self) {
    if let Some(deserializer) = self.deserializer.take() {
      if deserializer.error.is_some() {
        return;
      }

      let Some(current) = deserializer.stack.peek() else {
        deserializer.error = Some(JsonError::InternalError);
        return;
      };

      let mut depth;
      match current {
        State::Object | State::Array => depth = 1,
        State::Unknown => {
          let step = match single_step(&mut deserializer.bytes, &mut deserializer.stack) {
            Ok(step) => step,
            Err(e) => {
              deserializer.error = Some(e);
              return;
            }
          };
          match step {
            SingleStepResult::String(_) | SingleStepResult::Advanced => return,
            SingleStepResult::ObjectOpened | SingleStepResult::ArrayOpened => depth = 1,
            SingleStepResult::Field { .. } |
            SingleStepResult::ArrayValue |
            SingleStepResult::Closed => {
              deserializer.error = Some(JsonError::InternalError);
              return;
            }
          }
        }
      }
      while depth != 0 {
        let step = match single_step(&mut deserializer.bytes, &mut deserializer.stack) {
          Ok(step) => step,
          Err(e) => {
            deserializer.error = Some(e);
            return;
          }
        };
        match step {
          SingleStepResult::Field { .. } |
          SingleStepResult::ArrayValue |
          SingleStepResult::String(_) |
          SingleStepResult::Advanced => {}
          SingleStepResult::ObjectOpened | SingleStepResult::ArrayOpened => depth += 1,
          SingleStepResult::Closed => depth -= 1,
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
  pub fn value(&mut self) -> Result<Value<'bytes, '_, B, S>, JsonError<'bytes, B, S>> {
    if self.stack.depth() != 1 {
      Err(JsonError::ReusedDeserializer)?;
    }
    Ok(Value { deserializer: Some(self) })
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

#[rustfmt::skip]
impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>
  FieldIterator<'bytes, 'parent, B, S>
{
  /// The next entry (key, value) within the object.
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
  ) -> Option<Result<(String<'bytes, B>, Value<'bytes, '_, B, S>), JsonError<'bytes, B, S>>>
  {
    if let Some(err) = self.deserializer.error {
      return Some(Err(err));
    }

    if self.done {
      None?;
    }

    loop {
      match single_step(&mut self.deserializer.bytes, &mut self.deserializer.stack) {
        Ok(SingleStepResult::Field { key }) => {
          break Some(Ok((key, Value { deserializer: Some(self.deserializer) })))
        }
        Ok(SingleStepResult::Closed) => {
          self.done = true;
          None?
        }
        Ok(SingleStepResult::Advanced) => {}
        Ok(
          SingleStepResult::ArrayValue |
          SingleStepResult::ObjectOpened |
          SingleStepResult::ArrayOpened |
          SingleStepResult::String(_),
        ) => break Some(Err(JsonError::InternalError)),
        Err(e) => break Some(Err(e)),
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
      match single_step(&mut self.deserializer.bytes, &mut self.deserializer.stack) {
        Ok(SingleStepResult::ArrayValue) => {
          break Some(Ok(Value { deserializer: Some(self.deserializer) }))
        }
        Ok(SingleStepResult::Closed) => {
          self.done = true;
          None?;
        }
        Ok(SingleStepResult::Advanced) => {}
        Ok(
          SingleStepResult::Field { .. } |
          SingleStepResult::ObjectOpened |
          SingleStepResult::ArrayOpened |
          SingleStepResult::String(_),
        ) => break Some(Err(JsonError::InternalError)),
        Err(e) => break Some(Err(e)),
      }
    }
  }
}

impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> Value<'bytes, 'parent, B, S> {
  /// Check if the current item is an object.
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
  pub fn fields(mut self) -> Result<FieldIterator<'bytes, 'parent, B, S>, JsonError<'bytes, B, S>> {
    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    if let Some(err) = deserializer.error {
      Err(err)?;
    }

    match single_step(&mut deserializer.bytes, &mut deserializer.stack)? {
      SingleStepResult::ObjectOpened => Ok(FieldIterator { deserializer, done: false }),
      _ => Err(JsonError::TypeError),
    }
  }

  /// Check if the current item is an array.
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

  /// Get an iterator of all items within this container.
  ///
  /// If you want to index a specific item, you may use `.iterate()?.nth(i)?`. An `index` method
  /// isn't provided as each index operation is of O(n) complexity and single indexes SHOULD NOT be
  /// used. Only exposing `iterate` attempts to make this clear to the user.
  pub fn iterate(
    mut self,
  ) -> Result<ArrayIterator<'bytes, 'parent, B, S>, JsonError<'bytes, B, S>> {
    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    if let Some(err) = deserializer.error {
      Err(err)?;
    }

    match single_step(&mut deserializer.bytes, &mut deserializer.stack)? {
      SingleStepResult::ArrayOpened => Ok(ArrayIterator { deserializer, done: false }),
      _ => Err(JsonError::TypeError),
    }
  }

  /// Check if the current item is a string.
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

  /// Get the current item as a 'string' (represented as a `B`).
  ///
  /// This will NOT de-escape the string.
  #[inline(always)]
  pub fn to_str(mut self) -> Result<String<'bytes, B>, JsonError<'bytes, B, S>> {
    let deserializer = self.deserializer.take().ok_or(JsonError::InternalError)?;
    match single_step(&mut deserializer.bytes, &mut deserializer.stack)? {
      SingleStepResult::String(str) => Ok(str),
      _ => Err(JsonError::TypeError),
    }
  }

  /// Get the current item as an `i64`.
  #[inline(always)]
  pub fn as_i64(&self) -> Result<i64, JsonError<'bytes, B, S>> {
    let bytes = &self.deserializer.as_ref().ok_or(JsonError::InternalError)?.bytes;

    let mut res: i64 = 0;
    let mut negative = false;

    let mut i = 0;
    loop {
      let digit = bytes.peek(i).map_err(|_| JsonError::TypeError)?;
      #[allow(clippy::match_same_arms)]
      match digit {
        b'-' => {
          if i != 0 {
            Err(JsonError::TypeError)?;
          }
          negative = true;
        }
        b'0' ..= b'9' => {
          res = res.checked_mul(10).ok_or(JsonError::TypeError)?;
          res = res.checked_add((digit - b'0').into()).ok_or(JsonError::TypeError)?
        }
        b',' | b']' | b'}' => break,
        // Float
        b'.' => Err(JsonError::TypeError)?,
        // This may be an invalid integer or it could be whitespace before the object's terminator
        // As we assume this is valid JSON, we assume it's a valid integer
        _ => break,
      }
      i += 1;
    }

    if negative {
      res = res.checked_neg().ok_or(JsonError::TypeError)?;
    }

    Ok(res)
  }

  /// Get the current item as an `f64`.
  #[inline(always)]
  pub fn as_f64(&self) -> Result<f64, JsonError<'bytes, B, S>> {
    let bytes = &self.deserializer.as_ref().ok_or(JsonError::InternalError)?.bytes;

    const MAX_FLOAT_LEN: usize = 128;
    let mut dst: [u8; MAX_FLOAT_LEN] = [0; MAX_FLOAT_LEN];

    let mut i = 0;
    loop {
      let digit = bytes.peek(i).map_err(|_| JsonError::TypeError)?;
      if !matches!(digit, b'+' | b'-' | b'.' | b'e' | b'0' ..= b'9') {
        break;
      }
      if i == MAX_FLOAT_LEN {
        Err(JsonError::TypeError)?;
      }
      dst[i] = digit;
      i += 1;
    }

    use core::str::FromStr;
    f64::from_str(core::str::from_utf8(&dst[.. i]).map_err(|_| JsonError::NotUtf8)?.trim())
      .map_err(|_| JsonError::TypeError)
  }

  /// Get the current item as a `bool`.
  #[inline(always)]
  pub fn as_bool(&self) -> Result<bool, JsonError<'bytes, B, S>> {
    let bytes = &self.deserializer.as_ref().ok_or(JsonError::InternalError)?.bytes;

    let first = bytes.peek(0).ok();
    let second = bytes.peek(1).ok();
    let third = bytes.peek(2).ok();
    let fourth = bytes.peek(3).ok();
    let fifth = bytes.peek(4).ok();

    let is_true =
      (first, second, third, fourth) == (Some(b't'), Some(b'r'), Some(b'u'), Some(b'e'));
    let is_false = (first, second, third, fourth, fifth) ==
      (Some(b'f'), Some(b'a'), Some(b'l'), Some(b's'), Some(b'e'));

    if !(is_true | is_false) {
      Err(JsonError::TypeError)?;
    }

    Ok(is_true)
  }

  /// Check if the current item is `null`.
  #[inline(always)]
  pub fn is_null(&self) -> Result<bool, JsonError<'bytes, B, S>> {
    let bytes = &self.deserializer.as_ref().ok_or(JsonError::InternalError)?.bytes;

    let first = bytes.peek(0).ok();
    let second = bytes.peek(1).ok();
    let third = bytes.peek(2).ok();
    let fourth = bytes.peek(3).ok();

    Ok((first, second, third, fourth) == (Some(b'n'), Some(b'u'), Some(b'l'), Some(b'l')))
  }
}
