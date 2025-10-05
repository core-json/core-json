use core::marker::PhantomData;

use crate::{BytesLike, String, Stack, JsonError};

/// Peek a UTF-8 codepoint from bytes.
fn peek_utf8<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &B,
  i: usize,
) -> Result<char, JsonError<'bytes, B, S>> {
  let mut utf8_codepoint = [0; 4];
  utf8_codepoint[0] = bytes.peek(i).map_err(JsonError::BytesError)?;
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
  for (j, byte) in utf8_codepoint.iter_mut().enumerate().skip(1) {
    *byte = bytes.peek(i + j).map_err(JsonError::BytesError)?;
  }

  let str = core::str::from_utf8(utf8_codepoint).map_err(|_| JsonError::InvalidValue)?;
  str.chars().next().ok_or(JsonError::InternalError)
}

/// Read a just-opened string from a JSON serialization.
pub(crate) fn read_string<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &mut B,
) -> Result<String<'bytes, B>, JsonError<'bytes, B, S>> {
  // Find the location of the terminating quote
  let mut i = 0;
  {
    let mut escaping = false;
    loop {
      let this = peek_utf8(bytes, i)?;

      // https://datatracker.ietf.org/doc/html/rfc8259#section-7
      let unescaped =
        matches!(this, '\x20' ..= '\x21' | '\x23' ..= '\x5b' | '\x5d' ..= '\u{10ffff}');

      // If we're escaping the current character, check it's valid to be escaped
      if escaping {
        if !matches!(
          this,
          '\x22' | '\x5c' | '\x2f' | '\x62' | '\x66' | '\x6e' | '\x72' | '\x74' | '\x75'
        ) {
          Err(JsonError::InvalidValue)?;
        }

        // If this is "\u", check it's followed by hex characters
        if (this == '\x75') &&
          (!(peek_utf8(bytes, i + 1)?.is_ascii_hexdigit() &&
            peek_utf8(bytes, i + 2)?.is_ascii_hexdigit() &&
            peek_utf8(bytes, i + 3)?.is_ascii_hexdigit() &&
            peek_utf8(bytes, i + 4)?.is_ascii_hexdigit()))
        {
          Err(JsonError::InvalidValue)?;
        }
      } else if this == '"' {
        break;
      }

      if !(unescaped || escaping || (this == '\\')) {
        Err(JsonError::InvalidValue)?;
      }
      escaping = (!escaping) && (this == '\\');
      i += this.len_utf8();
    }
  }

  let (len, str_bytes) = bytes.read_bytes(i).map_err(JsonError::BytesError)?;
  // Advance past the closing `"`
  bytes.advance(1).map_err(JsonError::BytesError)?;
  Ok(String { len, bytes: str_bytes, _encoding: PhantomData })
}

/// An interator which yields the characters for an escaped string serialized within JSON.
pub struct UnescapeString<'bytes, B: BytesLike<'bytes>, S: Stack> {
  string: B,
  remaining: usize,
  first_iter: bool,
  _stack: PhantomData<(&'bytes (), S)>,
}
impl<'bytes, B: BytesLike<'bytes>, S: Stack> From<String<'bytes, B>>
  for UnescapeString<'bytes, B, S>
{
  fn from(string: String<'bytes, B>) -> Self {
    Self {
      remaining: string.len(),
      string: string.consume(),
      first_iter: true,
      _stack: PhantomData,
    }
  }
}
impl<'bytes, B: BytesLike<'bytes>, S: Stack> Iterator for UnescapeString<'bytes, B, S> {
  type Item = Result<char, JsonError<'bytes, B, S>>;
  fn next(&mut self) -> Option<Self::Item> {
    // Check if the string is empty
    if self.remaining == 0 {
      None?;
    }

    let res = (|| {
      {
        let next_char = peek_utf8(&self.string, 0)?;

        let len = next_char.len_utf8();
        // `InternalError`: `BytesLike` read past its declared length
        self.remaining = self.remaining.checked_sub(len).ok_or(JsonError::InternalError)?;
        self.string.advance(len).map_err(JsonError::BytesError)?;

        // If this isn't an escape character, yield it
        if next_char != '\\' {
          return Ok(next_char);
        }
      }

      // Definitions from https://datatracker.ietf.org/doc/html/rfc8259#section-7
      match {
        // `InternalError`: Escape character without following escaped values
        self.remaining = self.remaining.checked_sub(1).ok_or(JsonError::InternalError)?;
        self.string.read_byte().map_err(JsonError::BytesError)?
      } {
        // If this is to escape the intended character, yield it now
        b'"' => Ok('"'),
        b'\\' => Ok('\\'),
        b'/' => Ok('/'),
        // If this is to escape a control sequence, yield it now
        b'b' => Ok('\x08'),
        b'f' => Ok('\x0c'),
        b'n' => Ok('\n'),
        b'r' => Ok('\r'),
        b't' => Ok('\t'),

        // Handle if this is a unicode codepoint
        b'u' => {
          let mut read_hex = |with_u| {
            if with_u {
              let mut backslash_u = [0; 2];
              self.string.read_into_slice(&mut backslash_u).map_err(JsonError::BytesError)?;
              if &backslash_u != b"\\u" {
                Err(JsonError::InvalidValue)?;
              }
            }

            let mut hex = [0; 4];
            self.string.read_into_slice(&mut hex).map_err(JsonError::BytesError)?;
            // `InternalError`: `\u` without following 'hex' bytes being UTF-8
            let hex = core::str::from_utf8(&hex).map_err(|_| JsonError::InternalError)?;
            // `InternalError`: `\u` with UTF-8 bytes which weren't hex
            u16::from_str_radix(hex, 16).map(u32::from).map_err(|_| JsonError::InternalError)
          };

          // Read the hex digits
          // `InternalError`: `\u` without following hex bytes
          self.remaining = self.remaining.checked_sub(4).ok_or(JsonError::InternalError)?;
          let next = read_hex(false)?;

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

            // `InvalidValue`: Caller provided an incomplete code point
            /*
              https://datatracker.ietf.org/doc/html/rfc8259#section-8.2 notes how the syntax allows
              an incomplete codepoint, further noting the behavior of implementations is
              unpredictable. The definition of "interoperable" is if the strings are composed
              entirely of Unicode characters, with an unpaired surrogate being considered as unable
              to encode a Unicode character.

              As Rust requires `char` be a UTF codepoint, we require the strings be "interoperable"
              per the RFC 8259 definition. While this may be slightly stricter than the
              specification alone, it already has plenty of ambiguities due to how many slight
              differences exist with JSON encoders/decoders.

              Additionally, we'll still decode JSON objects with invalidly specified UTF codepoints
              within their strings. We just won't support converting them to characters with this
              iterator. This iterator failing will not cause the deserializer as a whole to fail.
            */
            self.remaining = self.remaining.checked_sub(6).ok_or(JsonError::InvalidValue)?;
            let low = read_hex(true)?;

            let Some(low) = low.checked_sub(0xdc00) else { Err(JsonError::InvalidValue)? };
            high + low + 0x10000
          } else {
            // If `next` isn't a surrogate, it's interpreted as a codepoint as-is
            next
          };

          // Yield the codepoint
          let Some(char) = char::from_u32(codepoint) else { Err(JsonError::InvalidValue)? };
          // https://datatracker.ietf.org/doc/html/rfc8259#section-8.1
          if self.first_iter && (char == '\u{feff}') {
            Err(JsonError::InvalidValue)?
          }
          Ok(char)
        }
        // `InternalError`: `\` without a recognized following character
        _ => Err(JsonError::InternalError),
      }
    })();

    // Mark this is no longer the first iteration
    self.first_iter = false;

    // If the result was an error, set `remaining = 0` so all future calls to `next` yield `None`
    if res.is_err() {
      self.remaining = 0;
    }

    Some(res)
  }
}
