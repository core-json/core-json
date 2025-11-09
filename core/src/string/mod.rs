use crate::{AsyncRead, Stack, AsyncDeserializer, JsonError};

mod unicode;
mod hex;

use unicode::*;
use hex::*;

/// An iterator which validates a string, yielding the items within.
///
/// This will yield `None` upon reaching a `"`, but is not fused and has undefined behavior upon
/// successive calls to `Iterator::next`.
///
/// This does not implement `Drop`. It is the caller's responsibility to exhaust this iterator to
/// ensure the deserializer is advanced correctly.
pub(crate) struct ValidateString<'read, 'parent, R: AsyncRead<'read>, S: Stack> {
  deserializer: &'parent mut AsyncDeserializer<'read, R, S>,
  done: bool,
}

impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> ValidateString<'read, 'parent, R, S> {
  #[inline(always)]
  async fn next_char(&mut self) -> Result<Option<StringCharacter>, JsonError<'read, R, S>> {
    let this = self.deserializer.reader.read_byte().await.map_err(JsonError::ReadError)?;

    // https://datatracker.ietf.org/doc/html/rfc8259#section-7
    Ok(match this {
      // The characters allowed to be unescaped
      b'\x20' ..= b'\x21' | b'\x23' ..= b'\x5b' | b'\x5d' ..= b'\x7f' => {
        Some(StringCharacter::Character(this as char))
      }
      b'\x80' ..= b'\xff' => Some(StringCharacter::Character(
        read_non_ascii_utf8(&mut self.deserializer.reader, this).await?,
      )),
      // The escaping character
      b'\\' => {
        // All characters which are valid to be escaped are ASCII, allowing us to use `read_byte`
        // here
        let escaped = self.deserializer.reader.read_byte().await.map_err(JsonError::ReadError)?;
        match escaped {
          b'"' | b'\\' | b'/' => Some(StringCharacter::Character(escaped as char)),
          b'b' => Some(StringCharacter::Character('\x08')),
          b'f' => Some(StringCharacter::Character('\x0c')),
          b'n' => Some(StringCharacter::Character('\n')),
          b'r' => Some(StringCharacter::Character('\r')),
          b't' => Some(StringCharacter::Character('\t')),
          // If this is "\u", check it's followed by hex characters
          b'\x75' => {
            // We can use `read_byte` here as valid hex characters will be ASCII (one-byte)
            let mut bytes = [0; 4];
            self
              .deserializer
              .reader
              .read_exact_into_non_empty_slice(&mut bytes)
              .await
              .map_err(JsonError::ReadError)?;
            if !validate_hex(bytes) {
              Err(JsonError::InvalidValue)?;
            }
            Some(StringCharacter::EscapedUnicode(bytes))
          }
          _ => Err(JsonError::InvalidValue)?,
        }
      }
      b'"' => {
        self.done = true;
        None
      }
      _ => Err(JsonError::InvalidValue)?,
    })
  }

  #[inline(always)]
  async fn drop(&mut self) -> Result<(), JsonError<'read, R, S>> {
    while !self.done {
      self.next_char().await?;
    }
    Ok(())
  }
}

/// A character within a JSON-serialized string.
pub(crate) enum StringCharacter {
  /// The character itself.
  Character(char),
  /// The UTF-16 hex corresponding to the character.
  EscapedUnicode([u8; 4]),
}

impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> ValidateString<'read, 'parent, R, S> {
  #[inline(always)]
  async fn next(&mut self) -> Option<Result<StringCharacter, JsonError<'read, R, S>>> {
    match self.next_char().await {
      Ok(Some(res)) => Some(Ok(res)),
      Ok(None) => None,
      Err(e) => {
        self.deserializer.poison(e);
        self.done = true;
        Some(Err(e))
      }
    }
  }
}

/// An iterator which yields the characters of a string represented within a JSON serialization.
pub(crate) struct String<'read, 'parent, R: AsyncRead<'read>, S: Stack> {
  validation: ValidateString<'read, 'parent, R, S>,
  errored: bool,
}

impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> String<'read, 'parent, R, S> {
  /// AsyncRead a just-opened string from a JSON serialization.
  #[inline(always)]
  pub(crate) fn read(deserializer: &'parent mut AsyncDeserializer<'read, R, S>) -> Self {
    String { validation: ValidateString { deserializer, done: false }, errored: false }
  }
}

#[inline(always)]
async fn handle_escaped_unicode<'read, 'parent, R: AsyncRead<'read>, S: Stack>(
  hex: [u8; 4],
  validation: &mut ValidateString<'read, 'parent, R, S>,
) -> Result<char, JsonError<'read, R, S>> {
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
    let Some(Ok(StringCharacter::EscapedUnicode(hex))) = validation.next().await else {
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

impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> String<'read, 'parent, R, S> {
  #[inline(always)]
  pub(super) async fn next(&mut self) -> Option<Result<char, JsonError<'read, R, S>>> {
    if self.errored | self.validation.done {
      None?;
    }

    Some(match self.validation.next().await? {
      Ok(StringCharacter::Character(char)) => Ok(char),
      Ok(StringCharacter::EscapedUnicode(hex)) => {
        let res = handle_escaped_unicode(hex, &mut self.validation).await;
        if res.is_err() {
          self.errored = true;
        }
        res
      }
      Err(e) => {
        self.errored = true;
        Err(e)
      }
    })
  }
}

/// A wrapper for a `String` which is a key.
///
/// When dropped (which MUST be done manually), this additionally reads past the colon separating
/// the key from the value.
pub(crate) struct StringKey<'read, 'parent, R: AsyncRead<'read>, S: Stack>(
  pub(crate) String<'read, 'parent, R, S>,
);
impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> StringKey<'read, 'parent, R, S> {
  #[inline(always)]
  pub(crate) async fn drop_string_key(
    deserializer: &mut AsyncDeserializer<'read, R, S>,
    done: bool,
  ) -> Result<(), JsonError<'read, R, S>> {
    (ValidateString { deserializer, done }).drop().await?;
    crate::advance_past_colon(&mut deserializer.reader).await
  }
}
impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> StringKey<'read, 'parent, R, S> {
  #[inline(always)]
  pub(super) fn drop(self) -> &'parent mut AsyncDeserializer<'read, R, S> {
    self.0.validation.deserializer.drop_string_key(self.0.validation.done);
    self.0.validation.deserializer
  }
}

/// A wrapper for a `String` which is a value.
///
/// When dropped, this additionally reads past a following comma or to the end of the container.
pub(crate) struct StringValue<'read, 'parent, R: AsyncRead<'read>, S: Stack>(
  pub(crate) String<'read, 'parent, R, S>,
);
impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> StringValue<'read, 'parent, R, S> {
  #[inline(always)]
  pub(crate) async fn drop_string_value(
    deserializer: &mut AsyncDeserializer<'read, R, S>,
    done: bool,
  ) -> Result<(), JsonError<'read, R, S>> {
    (ValidateString { deserializer, done }).drop().await?;
    crate::advance_past_comma_or_to_close(&mut deserializer.reader).await
  }
}
impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> Drop for StringValue<'read, 'parent, R, S> {
  #[inline(always)]
  fn drop(&mut self) {
    self.0.validation.deserializer.drop_string_value(self.0.validation.done)
  }
}
