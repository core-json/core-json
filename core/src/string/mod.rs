use crate::{BytesLike, Stack, Deserializer, JsonError};

mod unicode;
mod hex;

use unicode::*;
use hex::*;

/// An iterator which validates a string, yielding the items within.
///
/// This will yield `None` upon reaching a `"` and for all successive calls to `Iterator::next`.
///
/// This does not implement `Drop`. It is the caller's responsibility to exhaust this iterator to
/// ensure the deserializer is advanced correctly.
pub(crate) struct ValidateString<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> {
  deserializer: &'parent mut Deserializer<'bytes, B, S>,
  done: bool,
}

/// A character within a JSON-serialized string.
pub(crate) enum StringCharacter {
  /// The character itself.
  Character(char),
  /// The UTF-16 hex corresponding to the character.
  EscapedUnicode([u8; 4]),
}

impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> Iterator
  for ValidateString<'bytes, 'parent, B, S>
{
  type Item = Result<StringCharacter, JsonError<'bytes, B, S>>;
  #[inline(always)]
  fn next(&mut self) -> Option<Self::Item> {
    if let Some(e) = self.deserializer.error {
      return Some(Err(e));
    }
    if self.done {
      None?;
    }

    let res = (|| {
      let this = read_utf8(&mut self.deserializer.bytes)?;

      // https://datatracker.ietf.org/doc/html/rfc8259#section-7
      Ok(match this {
        // The characters allowed to be unescaped
        '\x20' ..= '\x21' | '\x23' ..= '\x5b' | '\x5d' ..= '\u{10ffff}' => {
          Some(StringCharacter::Character(this))
        }
        // The escaping character
        '\\' => {
          // All characters which are valid to be escaped are ASCII, allowing us to use `read_byte`
          // here
          let escaped = self.deserializer.bytes.read_byte().map_err(JsonError::BytesError)?;
          match escaped {
            b'"' => Some(StringCharacter::Character('"')),
            b'\\' => Some(StringCharacter::Character('\\')),
            b'/' => Some(StringCharacter::Character('/')),
            b'b' => Some(StringCharacter::Character('\x08')),
            b'f' => Some(StringCharacter::Character('\x0c')),
            b'n' => Some(StringCharacter::Character('\n')),
            b'r' => Some(StringCharacter::Character('\r')),
            b't' => Some(StringCharacter::Character('\t')),
            // If this is "\u", check it's followed by hex characters
            b'\x75' => {
              // We use `read_into_slice`, not `read_utf8`, here as hex characters will be ASCII
              let mut bytes = [0; 4];
              self.deserializer.bytes.read_into_slice(&mut bytes).map_err(JsonError::BytesError)?;
              if !validate_hex(bytes) {
                Err(JsonError::InvalidValue)?;
              }
              Some(StringCharacter::EscapedUnicode(bytes))
            }
            _ => Err(JsonError::InvalidValue)?,
          }
        }
        '"' => {
          self.done = true;
          None
        }
        _ => Err(JsonError::InvalidValue)?,
      })
    })();

    if let Some(e) = res.as_ref().err() {
      self.deserializer.error = Some(*e);
    }

    res.transpose()
  }
}

/// An iterator which yields the characters of a string represented within a JSON serialization.
pub(crate) struct String<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> {
  validation: ValidateString<'bytes, 'parent, B, S>,
  errored: bool,
}

impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> String<'bytes, 'parent, B, S> {
  /// Read a just-opened string from a JSON serialization.
  #[inline(always)]
  pub(crate) fn read(deserializer: &'parent mut Deserializer<'bytes, B, S>) -> Self {
    String { validation: ValidateString { deserializer, done: false }, errored: false }
  }

  #[inline(always)]
  fn drop(&mut self) -> &mut &'parent mut Deserializer<'bytes, B, S> {
    while let Some(Ok(_)) = self.validation.next() {}
    &mut self.validation.deserializer
  }
}

fn handle_escaped_unicode<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
  hex: [u8; 4],
  validation: &mut ValidateString<'bytes, 'parent, B, S>,
) -> Result<char, JsonError<'bytes, B, S>> {
  let next = read_hex(hex)?;

  /*
    If the intended value of this codepoint exceeds 0xffff, it's specified to be encoded
    with its UTF-16 surrogate pair. We distinguish and fetch the second part if necessary
    now. For the actual conversion algorithm from the UTF-16 surrogate pair to the UTF
    codepoint, https://en.wikipedia.org/wiki/UTF-16#U+D800_to_U+DFFF_(surrogates) is
    used as reference.
  */
  let next_is_utf16_high_surrogate = matches!(next, 0xd800 ..= 0xdbff);
  let codepoint = if next_is_utf16_high_surrogate {
    let high = (next - 0xd800) << 10;

    /*
      https://datatracker.ietf.org/doc/html/rfc8259#section-8.2 notes how the syntax
      allows an incomplete codepoint, further noting the behavior of implementations is
      unpredictable. The definition of "interoperable" is if the strings are composed
      entirely of Unicode characters, with an unpaired surrogate being considered as
      unable to encode a Unicode character.

      As Rust requires `char` be a UTF codepoint, we require the strings be
      "interoperable" per the RFC 8259 definition. While this may be slightly stricter
      than the specification alone, it already has plenty of ambiguities due to how many
      slight differences exist with JSON encoders/decoders.

      Additionally, we'll still decode JSON objects with invalidly specified UTF
      codepoints within their strings. We just won't support converting them to
      characters with this iterator. This iterator failing will not cause the
      deserializer as a whole to fail.
    */
    let Some(Ok(StringCharacter::EscapedUnicode(hex))) = validation.next() else {
      Err(JsonError::NotUtf8)?
    };
    let low = read_hex(hex)?;

    let Some(low) = low.checked_sub(0xdc00) else { Err(JsonError::NotUtf8)? };
    high + low + 0x10000
  } else {
    // If `next` isn't a surrogate, it's interpreted as a codepoint as-is
    next
  };

  // Yield the codepoint
  char::from_u32(codepoint).ok_or(JsonError::NotUtf8)
}

impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> Iterator for String<'bytes, 'parent, B, S> {
  type Item = Result<char, JsonError<'bytes, B, S>>;
  #[inline(always)]
  fn next(&mut self) -> Option<Self::Item> {
    if self.errored {
      None?;
    }

    Some(match self.validation.next()? {
      Ok(StringCharacter::Character(char)) => Ok(char),
      Ok(StringCharacter::EscapedUnicode(hex)) => {
        let res = handle_escaped_unicode(hex, &mut self.validation);
        if res.is_err() {
          self.errored = true;
        }
        res
      }
      Err(e) => Err(e),
    })
  }
}

/// A wrapper for a `String` which is a key.
///
/// When dropped, this additionally reads past the colon separating the key from the value.
pub(crate) struct StringKey<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
  pub(crate) Option<String<'bytes, 'parent, B, S>>,
);
impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> StringKey<'bytes, 'parent, B, S> {
  #[inline(always)]
  pub(super) fn drop(&mut self) -> Option<&'parent mut Deserializer<'bytes, B, S>> {
    let mut string = self.0.take()?;
    string.drop();
    let deserializer = string.validation.deserializer;
    if deserializer.error.is_none() {
      match crate::advance_past_colon(&mut deserializer.bytes) {
        Ok(()) => {}
        Err(e) => deserializer.error = Some(e),
      }
    }
    Some(deserializer)
  }
}
impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> Iterator
  for StringKey<'bytes, 'parent, B, S>
{
  type Item = Result<char, JsonError<'bytes, B, S>>;
  #[inline(always)]
  fn next(&mut self) -> Option<Self::Item> {
    self.0.as_mut().and_then(String::next)
  }
}

/// A wrapper for a `String` which is a value.
///
/// When dropped, this additionally reads past a following comma or to the end of the container.
pub(crate) struct StringValue<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
  pub(crate) String<'bytes, 'parent, B, S>,
);
impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> Drop for StringValue<'bytes, 'parent, B, S> {
  #[inline(always)]
  fn drop(&mut self) {
    let deserializer = self.0.drop();
    if deserializer.error.is_none() {
      match crate::advance_past_comma_or_to_close(&mut deserializer.bytes) {
        Ok(()) => {}
        Err(e) => deserializer.error = Some(e),
      }
    }
  }
}
impl<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack> Iterator
  for StringValue<'bytes, 'parent, B, S>
{
  type Item = Result<char, JsonError<'bytes, B, S>>;
  #[inline(always)]
  fn next(&mut self) -> Option<Self::Item> {
    self.0.next()
  }
}
