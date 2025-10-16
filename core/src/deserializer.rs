use crate::*;

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
    bytes.read_byte().map_err(JsonError::BytesError)?;
  }
  Ok(())
}

/// Advance past a colon.
pub(super) fn advance_past_colon<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &mut B,
) -> Result<(), JsonError<'bytes, B, S>> {
  advance_whitespace(bytes)?;
  match bytes.read_byte().map_err(JsonError::BytesError)? {
    b':' => advance_whitespace(bytes)?,
    _ => Err(JsonError::InvalidKeyValueDelimiter)?,
  }
  Ok(())
}

/// Advance past a comma, or to the close of the structure.
pub(super) fn advance_past_comma_or_to_close<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &mut B,
) -> Result<(), JsonError<'bytes, B, S>> {
  advance_whitespace(bytes)?;
  match bytes.peek(0).map_err(JsonError::BytesError)? {
    b',' => {
      bytes.read_byte().map_err(JsonError::BytesError)?;
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
pub(super) enum SingleStepObjectResult {
  /// A field within the object was advanced to.
  Field,
  /// The object was closed.
  Closed,
}

/// The result from a single step of the deserialized, if within an array.
pub(super) enum SingleStepArrayResult {
  /// A value within the array was advanced to.
  Value,
  /// The array was closed.
  Closed,
}

/// The result from a single step of the deserializer, if handling an unknown value.
pub(super) enum SingleStepUnknownResult {
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
pub(super) enum SingleStepResult {
  /// The result if within an object.
  Object(SingleStepObjectResult),
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
) -> Result<SingleStepResult, JsonError<'bytes, B, S>> {
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

      // Push how we're reading a value of an unknown type onto the stack, for the value
      stack.push(State::Unknown).map_err(JsonError::StackError)?;
      Ok(SingleStepResult::Object(SingleStepObjectResult::Field))
    }
    State::Array => {
      // Check if the array terminates
      if bytes.peek(0).map_err(JsonError::BytesError)? == b']' {
        stack.pop().ok_or(JsonError::InternalError)?;
        bytes.read_byte().map_err(JsonError::BytesError)?;

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
          bytes.read_byte().map_err(JsonError::BytesError)?;
          advance_whitespace(bytes)?;
          stack.push(State::Object).map_err(JsonError::StackError)?;
          return Ok(SingleStepResult::Unknown(SingleStepUnknownResult::ObjectOpened));
        }
        // Handle if this opens an array
        Type::Array => {
          bytes.read_byte().map_err(JsonError::BytesError)?;
          advance_whitespace(bytes)?;
          stack.push(State::Array).map_err(JsonError::StackError)?;
          return Ok(SingleStepResult::Unknown(SingleStepUnknownResult::ArrayOpened));
        }
        // Handle if this opens an string
        Type::String => {
          bytes.read_byte().map_err(JsonError::BytesError)?;
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

/// A deserializer for a JSON-encoded structure.
pub struct Deserializer<'bytes, B: BytesLike<'bytes>, S: Stack> {
  pub(crate) bytes: B,
  stack: S,
  /*
    We advance the deserializer within `Drop` which cannot return an error. If an error is raised
    within drop, we store it here to be consumed upon the next call to a method which can return an
    error (if one is ever called).
  */
  pub(crate) error: Option<JsonError<'bytes, B, S>>,
}

impl<'bytes, B: BytesLike<'bytes>, S: Stack> Deserializer<'bytes, B, S> {
  pub(super) fn single_step(&mut self) -> Result<SingleStepResult, JsonError<'bytes, B, S>> {
    if let Some(e) = self.error {
      Err(e)?;
    }
    let res = single_step(&mut self.bytes, &mut self.stack);
    if let Some(e) = res.as_ref().err() {
      self.error = Some(*e);
    }
    res
  }
}

/// A JSON value.
// Internally, we assume whenever this is held, the top item on the stack is `State::Unknown`
pub struct Value<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> {
  pub(crate) deserializer: Option<&'parent mut Deserializer<'bytes, B, S>>,
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
          let step = match deserializer.single_step() {
            Ok(SingleStepResult::Unknown(step)) => step,
            Ok(_) => {
              deserializer.error = Some(JsonError::InternalError);
              return;
            }
            Err(_) => return,
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
        let Ok(step) = deserializer.single_step() else { return };
        match step {
          SingleStepResult::Unknown(SingleStepUnknownResult::String) => {
            handle_string_value(deserializer);
          }
          SingleStepResult::Object(SingleStepObjectResult::Field) => {
            handle_field(deserializer);
          }
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
    if !matches!(result.kind()?, Type::Object | Type::Array) {
      Err(JsonError::TypeError)?;
    }
    Ok(result)
  }
}
