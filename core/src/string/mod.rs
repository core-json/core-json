use core::task::Poll;

use crate::{AsyncRead, PeekableRead, Stack, AsyncDeserializer, JsonError};

mod unicode;
mod hex;

use unicode::*;
use hex::*;

/// An sink which validates a string, yielding the items within.
///
/// This will yield `None` upon reaching a `"`, but is not fused and has undefined behavior upon
/// successive calls to `ValidateString::next`. It also has undefined behavior for calls after an
/// error is returned.
///
/// This does not implement `Drop`. It is the caller's responsibility to exhaust this to ensure the
/// deserializer is advanced correctly.
#[allow(private_interfaces)]
pub(crate) enum ValidateString {
  Fresh,
  Unicode(UnicodeSink),
  Escaping,
  EscapedUnicode([u8; 4], u8),
  Done,
}

impl ValidateString {
  #[inline(always)]
  fn push_byte<'read, R: AsyncRead<'read>, S: Stack>(
    &mut self,
    byte: u8,
  ) -> Poll<Result<Option<StringCharacter>, JsonError<'read, R, S>>> {
    match self {
      // https://datatracker.ietf.org/doc/html/rfc8259#section-7
      Self::Fresh => match byte {
        // The characters allowed to be unescaped
        b'\x20' ..= b'\x21' | b'\x23' ..= b'\x5b' | b'\x5d' ..= b'\x7f' => {
          Poll::Ready(Ok(Some(StringCharacter::Character(byte as char))))
        }
        // Unicode characters allowed to be unescpaed
        b'\x80' ..= b'\xff' => {
          *self = Self::Unicode(UnicodeSink::read_non_ascii_utf8(byte));
          Poll::Pending
        }
        // The escaping character
        b'\\' => {
          *self = Self::Escaping;
          Poll::Pending
        }
        // The end of the string
        b'"' => {
          *self = Self::Done;
          Poll::Ready(Ok(None))
        }
        _ => Poll::Ready(Err(JsonError::InvalidValue)),
      },
      Self::Unicode(sink) => match sink.push_byte(byte) {
        Poll::Ready(char) => {
          *self = Self::Fresh;
          Poll::Ready(char.map(|char| Some(StringCharacter::Character(char))))
        }
        Poll::Pending => Poll::Pending,
      },
      Self::Escaping => {
        *self = Self::Fresh;
        Poll::Ready(Ok(Some(match byte {
          b'"' | b'\\' | b'/' => StringCharacter::Character(byte as char),
          b'b' => StringCharacter::Character('\x08'),
          b'f' => StringCharacter::Character('\x0c'),
          b'n' => StringCharacter::Character('\n'),
          b'r' => StringCharacter::Character('\r'),
          b't' => StringCharacter::Character('\t'),
          // If this is "\u", it's the 4-byte hex for a Unicode codepoint
          b'u' => {
            *self = Self::EscapedUnicode([0; 4], 0);
            return Poll::Pending;
          }
          _ => return Poll::Ready(Err(JsonError::InvalidValue)),
        })))
      }
      Self::EscapedUnicode(hex, already) => {
        hex[usize::from(*already)] = byte;
        *already = already.wrapping_add(1);
        if *already == 4 {
          let hex = *hex;
          if !validate_hex(hex) {
            return Poll::Ready(Err(JsonError::InvalidValue));
          }
          *self = Self::Fresh;
          Poll::Ready(Ok(Some(StringCharacter::EscapedUnicode(hex))))
        } else {
          Poll::Pending
        }
      }
      Self::Done => Poll::Ready(Err(JsonError::InternalError)),
    }
  }

  #[inline(always)]
  async fn drop<'read, R: AsyncRead<'read>, S: Stack>(
    mut self,
    reader: &mut PeekableRead<'read, R>,
  ) -> Result<(), JsonError<'read, R, S>> {
    if matches!(self, Self::Done) {
      return Ok(());
    }
    loop {
      match self.push_byte::<R, S>(reader.read_byte().await.map_err(JsonError::ReadError)?) {
        Poll::Ready(Ok(None)) => return Ok(()),
        Poll::Ready(Err(e)) => return Err(e),
        Poll::Ready(Ok(Some(_))) | Poll::Pending => {}
      }
    }
  }
}

/// A character within a JSON-serialized string.
pub(crate) enum StringCharacter {
  /// The character itself.
  Character(char),
  /// The UTF-16 hex corresponding to the character.
  EscapedUnicode([u8; 4]),
}

/// An iterator which yields the characters of a string represented within a JSON serialization.
pub(crate) struct String<'read, 'parent, R: AsyncRead<'read>, S: Stack> {
  validation: ValidateString,
  deserializer: &'parent mut AsyncDeserializer<'read, R, S>,
  errored: bool,
}

impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> String<'read, 'parent, R, S> {
  /// AsyncRead a just-opened string from a JSON serialization.
  #[inline(always)]
  pub(crate) fn read(deserializer: &'parent mut AsyncDeserializer<'read, R, S>) -> Self {
    String { validation: ValidateString::Fresh, deserializer, errored: false }
  }
}

impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> String<'read, 'parent, R, S> {
  #[inline(always)]
  pub(super) async fn next(&mut self) -> Option<Result<char, JsonError<'read, R, S>>> {
    if self.errored || matches!(self.validation, ValidateString::Done) {
      None?;
    }

    loop {
      let byte = match self.deserializer.reader.read_byte().await {
        Ok(byte) => byte,
        Err(e) => {
          let e = JsonError::ReadError(e);
          self.deserializer.poison(e);
          self.validation = ValidateString::Done;
          return Some(Err(e));
        }
      };
      match self.validation.push_byte::<R, S>(byte) {
        Poll::Ready(Ok(Some(StringCharacter::Character(char)))) => return Some(Ok(char)),
        Poll::Ready(Ok(Some(StringCharacter::EscapedUnicode(hex)))) => {
          let next = match read_hex(hex) {
            Ok(next) => next,
            Err(e) => {
              self.deserializer.poison(e);
              self.validation = ValidateString::Done;
              return Some(Err(e));
            }
          };

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
            let hex = loop {
              let byte = match self.deserializer.reader.read_byte().await {
                Ok(byte) => byte,
                Err(e) => {
                  let e = JsonError::ReadError(e);
                  self.deserializer.poison(e);
                  self.validation = ValidateString::Done;
                  return Some(Err(e));
                }
              };
              match self.validation.push_byte(byte) {
                Poll::Ready(Ok(Some(StringCharacter::EscapedUnicode(hex)))) => break hex,
                Poll::Ready(Ok(Some(_) | None)) => {
                  self.errored = true;
                  return Some(Err(JsonError::NotUtf8));
                }
                Poll::Ready(Err(e)) => {
                  self.deserializer.poison(e);
                  self.validation = ValidateString::Done;
                  return Some(Err(e));
                }
                Poll::Pending => {}
              }
            };
            let low = match read_hex(hex) {
              Ok(low) => low,
              Err(e) => {
                self.deserializer.poison(e);
                self.validation = ValidateString::Done;
                return Some(Err(e));
              }
            };

            let Some(low) = low.checked_sub(0xdc00) else {
              self.errored = true;
              return Some(Err(JsonError::NotUtf8));
            };
            high + low + 0x10000
          } else {
            // If `next` isn't a surrogate, it's interpreted as a codepoint as-is
            next
          };

          // Yield the codepoint
          return match char::from_u32(codepoint) {
            Some(char) => Some(Ok(char)),
            None => {
              self.errored = true;
              Some(Err(JsonError::NotUtf8))
            }
          };
        }
        Poll::Ready(Err(e)) => {
          self.deserializer.poison(e);
          self.validation = ValidateString::Done;
          return Some(Err(e));
        }
        Poll::Ready(Ok(None)) => return None,
        Poll::Pending => {}
      }
    }
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
    string: ValidateString,
  ) -> Result<(), JsonError<'read, R, S>> {
    string.drop::<R, S>(&mut deserializer.reader).await?;
    crate::advance_past_colon(&mut deserializer.reader).await
  }
}
impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> StringKey<'read, 'parent, R, S> {
  #[inline(always)]
  pub(super) fn drop(self) -> &'parent mut AsyncDeserializer<'read, R, S> {
    self.0.deserializer.drop_string_key(self.0.validation);
    self.0.deserializer
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
    string: ValidateString,
  ) -> Result<(), JsonError<'read, R, S>> {
    string.drop::<R, S>(&mut deserializer.reader).await?;
    crate::advance_past_comma_or_to_close(&mut deserializer.reader).await
  }
}

impl<'read, 'parent, R: AsyncRead<'read>, S: Stack> Drop for StringValue<'read, 'parent, R, S> {
  #[inline(always)]
  fn drop(&mut self) {
    let mut validation = ValidateString::Done;
    core::mem::swap(&mut self.0.validation, &mut validation);
    self.0.deserializer.drop_string_value(validation)
  }
}
