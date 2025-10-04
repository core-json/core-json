use core::marker::PhantomData;

use crate::{BytesLike, String, Stack, JsonError};

/// Peek a UTF-8 codepoint from bytes.
fn peek_utf8<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &B,
  i: usize,
) -> Result<(usize, char), JsonError<'bytes, B, S>> {
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
  Ok((utf8_codepoint_len, str.chars().next().ok_or(JsonError::InternalError)?))
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
      let (this_len, this) = peek_utf8(bytes, i)?;

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
          (!(peek_utf8(bytes, i + 1)?.1.is_ascii_hexdigit() &&
            peek_utf8(bytes, i + 2)?.1.is_ascii_hexdigit() &&
            peek_utf8(bytes, i + 3)?.1.is_ascii_hexdigit() &&
            peek_utf8(bytes, i + 4)?.1.is_ascii_hexdigit()))
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
      i += this_len;
    }
  }

  let (len, str_bytes) = bytes.read_bytes(i).map_err(JsonError::BytesError)?;
  // Advance past the closing `"`
  bytes.advance(1).map_err(JsonError::BytesError)?;
  Ok(String { len, bytes: str_bytes, _encoding: PhantomData })
}
