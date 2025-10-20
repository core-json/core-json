use crate::*;

/// Advance the reader until there's a non-whitespace character.
#[inline(always)]
fn advance_whitespace<'read, R: Read<'read>, S: Stack>(
  reader: &mut PeekableRead<'read, R>,
) -> Result<(), JsonError<'read, R, S>> {
  let mut next;
  while {
    next = reader.peek();
    // https://datatracker.ietf.org/doc/html/rfc8259#section-2 defines whitespace as follows
    matches!(next, b'\x20' | b'\x09' | b'\x0A' | b'\x0D')
  } {
    reader.read_byte().map_err(JsonError::ReadError)?;
  }
  Ok(())
}

/// Advance past a colon.
#[inline(always)]
pub(super) fn advance_past_colon<'read, R: Read<'read>, S: Stack>(
  reader: &mut PeekableRead<'read, R>,
) -> Result<(), JsonError<'read, R, S>> {
  advance_whitespace(reader)?;
  match reader.read_byte().map_err(JsonError::ReadError)? {
    b':' => advance_whitespace(reader)?,
    _ => Err(JsonError::InvalidKeyValueDelimiter)?,
  }
  Ok(())
}

/// Advance past a comma, or to the close of the structure.
#[inline(always)]
pub(super) fn advance_past_comma_or_to_close<'read, R: Read<'read>, S: Stack>(
  reader: &mut PeekableRead<'read, R>,
) -> Result<(), JsonError<'read, R, S>> {
  advance_whitespace(reader)?;
  match reader.peek() {
    b',' => {
      reader.read_byte().map_err(JsonError::ReadError)?;
      advance_whitespace(reader)?;
      if matches!(reader.peek(), b']' | b'}') {
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
/// This assumes there is no leading whitespace present in `reader` and will advance past any
/// whitespace present before the next logical unit.
fn single_step<'read, 'parent, R: Read<'read>, S: Stack>(
  reader: &'parent mut PeekableRead<'read, R>,
  stack: &'parent mut S,
) -> Result<SingleStepResult, JsonError<'read, R, S>> {
  match stack.peek().ok_or(JsonError::InternalError)? {
    State::Object => {
      let next = reader.peek();

      // Check if the object terminates
      if next == b'}' {
        stack.pop().ok_or(JsonError::InternalError)?;

        // If this isn't the outer object, advance past the comma after
        if stack.depth() != 0 {
          // Advance past the '}'
          /*
            We only do this when the object *isn't* closing to prevent reading past the boundary of
            the object, as the '}' was already internally read (consumed from the underlying
            reader) by `PeekableRead`.
          */
          reader.read_byte().map_err(JsonError::ReadError)?;
          advance_past_comma_or_to_close(reader)?;
        }

        return Ok(SingleStepResult::Object(SingleStepObjectResult::Closed));
      }

      // Read the name of this field
      if next != b'"' {
        Err(JsonError::InvalidKey)?;
      }
      // Advance past the '"'
      reader.read_byte().map_err(JsonError::ReadError)?;

      // Push how we're reading a value of an unknown type onto the stack, for the value
      stack.push(State::Unknown).map_err(JsonError::StackError)?;
      Ok(SingleStepResult::Object(SingleStepObjectResult::Field))
    }
    State::Array => {
      // Check if the array terminates
      if reader.peek() == b']' {
        stack.pop().ok_or(JsonError::InternalError)?;

        // If this isn't the outer object, advance past the comma after
        if stack.depth() != 0 {
          reader.read_byte().map_err(JsonError::ReadError)?;
          advance_past_comma_or_to_close(reader)?;
        }

        return Ok(SingleStepResult::Array(SingleStepArrayResult::Closed));
      }

      // Since the array doesn't terminate, read the next value
      stack.push(State::Unknown).map_err(JsonError::StackError)?;
      Ok(SingleStepResult::Array(SingleStepArrayResult::Value))
    }
    State::Unknown => {
      stack.pop().ok_or(JsonError::InternalError)?;

      let result = match kind(reader) {
        // Handle if this opens an object
        Type::Object => {
          reader.read_byte().map_err(JsonError::ReadError)?;
          advance_whitespace(reader)?;
          stack.push(State::Object).map_err(JsonError::StackError)?;
          return Ok(SingleStepResult::Unknown(SingleStepUnknownResult::ObjectOpened));
        }
        // Handle if this opens an array
        Type::Array => {
          reader.read_byte().map_err(JsonError::ReadError)?;
          advance_whitespace(reader)?;
          stack.push(State::Array).map_err(JsonError::StackError)?;
          return Ok(SingleStepResult::Unknown(SingleStepUnknownResult::ArrayOpened));
        }
        // Handle if this opens an string
        Type::String => {
          reader.read_byte().map_err(JsonError::ReadError)?;
          return Ok(SingleStepResult::Unknown(SingleStepUnknownResult::String));
        }
        Type::Number => {
          SingleStepResult::Unknown(SingleStepUnknownResult::Number(number::to_number_str(reader)?))
        }
        Type::Bool => {
          let mut bool_string = [0; 4];
          reader.read_exact_into_non_empty_slice(&mut bool_string).map_err(JsonError::ReadError)?;
          let bool = if &bool_string == b"true" {
            true
          } else {
            let e = reader.read_byte().map_err(JsonError::ReadError)?;
            if !((bool_string == *b"fals") & (e == b'e')) {
              Err(JsonError::TypeError)?;
            }
            false
          };
          SingleStepResult::Unknown(SingleStepUnknownResult::Bool(bool))
        }
        Type::Null => {
          let null_string = [
            reader.read_byte().map_err(JsonError::ReadError)?,
            reader.read_byte().map_err(JsonError::ReadError)?,
            reader.read_byte().map_err(JsonError::ReadError)?,
            reader.read_byte().map_err(JsonError::ReadError)?,
          ];
          if null_string != *b"null" {
            Err(JsonError::InvalidValue)?;
          }
          SingleStepResult::Unknown(SingleStepUnknownResult::Null)
        }
      };

      // We now have to read past the next comma, or to the next closing of a structure
      advance_past_comma_or_to_close(reader)?;

      Ok(result)
    }
  }
}

/// A deserializer for a JSON-encoded structure.
pub struct Deserializer<'read, R: Read<'read>, S: Stack> {
  pub(crate) reader: PeekableRead<'read, R>,
  stack: S,
  /*
    We advance the deserializer within `Drop` which cannot return an error. If an error is raised
    within drop, we store it here to be consumed upon the next call to a method which can return an
    error (if one is ever called).
  */
  pub(crate) error: Option<JsonError<'read, R, S>>,
}

impl<'read, R: Read<'read>, S: Stack> Deserializer<'read, R, S> {
  #[inline(always)]
  pub(super) fn single_step(&mut self) -> Result<SingleStepResult, JsonError<'read, R, S>> {
    if let Some(e) = self.error {
      Err(e)?;
    }
    let res = single_step(&mut self.reader, &mut self.stack);
    if let Some(e) = res.as_ref().err() {
      self.error = Some(*e);
    }
    res
  }
}

/// A JSON value.
// Internally, we assume whenever this is held, the top item on the stack is `State::Unknown`
pub struct Value<'read, 'parent, R: Read<'read>, S: Stack> {
  pub(crate) deserializer: Option<&'parent mut Deserializer<'read, R, S>>,
}

impl<'read, 'parent, R: Read<'read>, S: Stack> Drop for Value<'read, 'parent, R, S> {
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

impl<'read, R: Read<'read>, S: Stack> Deserializer<'read, R, S> {
  /// Create a new deserializer.
  ///
  /// This will advance past any whitespace present at the start of the reader, per RFC 8259's
  /// definition of whitespace.
  ///
  /// If `reader` is aligned to valid JSON, this will read past the immediately present structure
  /// yet no further. If `reader` is not aligned to valid JSON, the state of `reader` is undefined
  /// after this.
  #[inline(always)]
  pub fn new(reader: R) -> Result<Self, JsonError<'read, R, S>> {
    let mut reader = PeekableRead::try_from(reader).map_err(JsonError::ReadError)?;
    advance_whitespace(&mut reader)?;

    let mut stack = S::empty();
    stack.push(State::Unknown).map_err(JsonError::StackError)?;

    Ok(Deserializer { reader, stack, error: None })
  }

  /// Obtain the `Value` representing the serialized structure.
  ///
  /// This takes a mutable reference as `Deserializer` is the owned object representing the
  /// deserializer's state. However, this is not eligible to be called more than once, even after
  /// the initial mutable borrow is dropped. Multiple calls to this function will cause an error to
  /// be returned.
  #[inline(always)]
  pub fn value(&mut self) -> Result<Value<'read, '_, R, S>, JsonError<'read, R, S>> {
    if (self.stack.depth() != 1) || self.error.is_some() {
      Err(JsonError::ReusedDeserializer)?;
    }
    let mut result = Value { deserializer: Some(self) };
    if !matches!(result.kind()?, Type::Object | Type::Array) {
      Err(JsonError::TypeError)?;
    }
    Ok(result)
  }
}
