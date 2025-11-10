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
          let mut null_string = [0; 4];
          reader.read_exact_into_non_empty_slice(&mut null_string).map_err(JsonError::ReadError)?;
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

enum ToDrop {
  None,
  StringKey(bool),
  StringValue(bool),
}

pub(crate) struct DelayedDrop<'read, R: Read<'read>, S: Stack> {
  /// If this is without work.
  nothing_queued: bool,
  /// An error raised within an internal context and cached as to prevent further usage of the
  /// deserializer.
  error: Option<JsonError<'read, R, S>>,
  /// An item to `Drop` whenever the deserializer regains the control flow.
  to_drop: ToDrop,
  /// The amount of structures to drop whenever the deserializer regains the control flow.
  structures_to_drop: u64,
  /// If an unknown value should be dropped whenever the deserializer regains the control flow.
  drop_value: bool,
}

impl<'read, R: Read<'read>, S: Stack> DelayedDrop<'read, R, S> {
  pub(crate) fn drop(
    deserializer: &mut Deserializer<'read, R, S>,
  ) -> Result<(), JsonError<'read, R, S>> {
    if deserializer.delayed_drop.nothing_queued {
      return Ok(());
    }

    if let Some(err) = deserializer.delayed_drop.error {
      Err(err)?;
    }

    'outer: loop {
      // Handle dropping of unit types
      match deserializer.delayed_drop.to_drop {
        ToDrop::None => {}
        ToDrop::StringKey(flag) => {
          deserializer.delayed_drop.to_drop = ToDrop::None;
          StringKey::drop_string_key(deserializer, flag)?;
        }
        ToDrop::StringValue(flag) => {
          deserializer.delayed_drop.to_drop = ToDrop::None;
          StringValue::drop_string_value(deserializer, flag)?;
        }
      }

      // Handle dropping of unknown values
      if deserializer.delayed_drop.drop_value {
        deserializer.delayed_drop.drop_value = false;

        let step = match single_step(&mut deserializer.reader, &mut deserializer.stack)? {
          SingleStepResult::Unknown(step) => step,
          // If we had a `Value`, it's an invariant the top of the stack was `State::Unknown`
          _ => Err(JsonError::InternalError)?,
        };

        match step {
          // We successfully advanced past this item
          SingleStepUnknownResult::Number(_) |
          SingleStepUnknownResult::Bool(_) |
          SingleStepUnknownResult::Null => {}
          // We opened a string we now have to handle
          SingleStepUnknownResult::String => StringValue::drop_string_value(deserializer, false)?,
          // We opened an object/array we now have to advance past
          SingleStepUnknownResult::ObjectOpened | SingleStepUnknownResult::ArrayOpened => {
            deserializer.drop_structure()
          }
        }
      }

      // Handle dropping of any structures
      while deserializer.delayed_drop.structures_to_drop != 0 {
        let step = single_step(&mut deserializer.reader, &mut deserializer.stack)?;
        match step {
          SingleStepResult::Unknown(SingleStepUnknownResult::String) => {
            // Queue the drop for this string, then iteratively restart this function to actually
            // drop it, which will return us into loop
            handle_string_value(deserializer);
            continue 'outer;
          }
          SingleStepResult::Object(SingleStepObjectResult::Field) => {
            handle_field(deserializer);
            continue 'outer;
          }
          SingleStepResult::Object(SingleStepObjectResult::Closed) |
          SingleStepResult::Array(SingleStepArrayResult::Closed) => {
            deserializer.delayed_drop.structures_to_drop -= 1
          }
          SingleStepResult::Unknown(
            SingleStepUnknownResult::ObjectOpened | SingleStepUnknownResult::ArrayOpened,
          ) => deserializer.delayed_drop.structures_to_drop += 1,
          _ => {}
        }
      }

      // If we completed all work, break out of the drop loop
      deserializer.delayed_drop.nothing_queued = true;
      break;
    }

    Ok(())
  }
}

/// A deserializer for a JSON-encoded structure.
pub struct Deserializer<'read, R: Read<'read>, S: Stack> {
  pub(crate) reader: PeekableRead<'read, R>,
  stack: S,
  delayed_drop: DelayedDrop<'read, R, S>,
}

impl<'read, R: Read<'read>, S: Stack> Deserializer<'read, R, S> {
  /// Queue the drop of a `StringKey`.
  #[inline(always)]
  pub(crate) fn drop_string_key(&mut self, flag: bool) {
    self.delayed_drop.nothing_queued = false;
    self.delayed_drop.to_drop = ToDrop::StringKey(flag);
  }
  /// Queue the drop of a `StringValue`.
  #[inline(always)]
  pub(crate) fn drop_string_value(&mut self, flag: bool) {
    self.delayed_drop.nothing_queued = false;
    self.delayed_drop.to_drop = ToDrop::StringValue(flag);
  }
  /// Queue the drop of an object or array.
  #[inline(always)]
  pub(crate) fn drop_structure(&mut self) {
    self.delayed_drop.nothing_queued = false;
    self.delayed_drop.structures_to_drop += 1;
  }
  /// Queue the drop of a value of unknown type.
  #[inline(always)]
  pub(crate) fn drop_value(&mut self) {
    self.delayed_drop.nothing_queued = false;
    self.delayed_drop.drop_value = true;
  }

  /// Poison the deserializer such that all future calls return an error.
  #[inline(always)]
  pub(crate) fn poison(&mut self, error: JsonError<'read, R, S>) {
    self.delayed_drop.nothing_queued = false;
    self.delayed_drop.error = Some(error);
  }

  #[inline(always)]
  pub(super) fn single_step(&mut self) -> Result<SingleStepResult, JsonError<'read, R, S>> {
    let res = DelayedDrop::drop(self);
    let res = res.and_then(|()| single_step(&mut self.reader, &mut self.stack));
    if let Some(e) = res.as_ref().err() {
      self.delayed_drop.nothing_queued = false;
      self.delayed_drop.error = Some(*e);
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
  #[inline(always)]
  fn drop(&mut self) {
    /*
      When this value is dropped, we advance the deserializer past it if it hasn't already been
      converted into a `FieldIterator` or `ArrayIterator` (which each have their own `Drop`
      implementations).
    */
    if let Some(deserializer) = self.deserializer.take() {
      deserializer.drop_value();
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

    Ok(Deserializer {
      reader,
      stack,
      delayed_drop: DelayedDrop {
        nothing_queued: true,
        error: None,
        to_drop: ToDrop::None,
        structures_to_drop: 0,
        drop_value: false,
      },
    })
  }

  /// Obtain the `Value` representing the serialized structure.
  ///
  /// This takes a mutable reference as `Deserializer` is the owned object representing the
  /// deserializer's state. However, this is not eligible to be called more than once, even after
  /// the initial mutable borrow is dropped. Multiple calls to this function will cause an error to
  /// be returned.
  #[inline(always)]
  pub fn value(&mut self) -> Result<Value<'read, '_, R, S>, JsonError<'read, R, S>> {
    if (self.stack.depth() != 1) || (!self.delayed_drop.nothing_queued) {
      Err(JsonError::ReusedDeserializer)?;
    }
    let mut result = Value { deserializer: Some(self) };
    if !matches!(result.kind()?, Type::Object | Type::Array) {
      Err(JsonError::TypeError)?;
    }
    Ok(result)
  }
}
