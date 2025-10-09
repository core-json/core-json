use crate::{BytesLike, Stack, JsonError};

/// Check if the immediate value within the bytes is a number.
///
/// This returns the length of the number's serialization, if it's valid.
pub(crate) fn is_number_str<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &B,
) -> Result<usize, JsonError<'bytes, B, S>> {
  let mut i = 0;
  let mut frac = None;
  let mut has_exponent = false;
  let mut immediately_after_e = false;
  let mut exponent_has_digit = false;
  let mut first_char_in_int = None;
  loop {
    let char = bytes.peek(i).map_err(|e| JsonError::BytesError(e))?;
    // https://datatracker.ietf.org/doc/html/rfc8259#section-6
    match char {
      // `-` must be at the beginning of the number or immediately following the exponent
      b'-' => {
        if !((i == 0) || immediately_after_e) {
          Err(JsonError::TypeError)?
        }
      }
      // `+` must be immediately following the exponent
      b'+' => {
        if !immediately_after_e {
          Err(JsonError::TypeError)?
        }
      }
      b'0' => {
        first_char_in_int = first_char_in_int.or(Some(char));
        if has_exponent {
          exponent_has_digit = true;
        }
      }
      // `0-9`
      b'1' ..= b'9' => {
        // If we are still within the integer part of the number...
        if !(frac.is_some() || has_exponent) {
          // Require there not be a leading zero
          if first_char_in_int == Some(b'0') {
            Err(JsonError::TypeError)?
          }
          first_char_in_int = first_char_in_int.or(Some(char));
        }
        if has_exponent {
          exponent_has_digit = true;
        }
      }
      b'.' => {
        if first_char_in_int.is_none() || frac.is_some() || has_exponent {
          Err(JsonError::TypeError)?;
        }
        frac = Some(i);
      }
      b'e' | b'E' => {
        if first_char_in_int.is_none() || has_exponent {
          Err(JsonError::TypeError)?;
        }
        // Check if there was a fractional part, it had at least one digit
        if let Some(frac) = frac {
          if frac == (i - 1) {
            Err(JsonError::TypeError)?;
          }
        }
        has_exponent = true;
      }
      // separator, array closure, object closure, whitespace
      // https://datatracker.ietf.org/doc/html/rfc8259#section-2
      b',' | b']' | b'}' | b'\x20' | b'\x09' | b'\x0A' | b'\x0D' => break,
      _ => Err(if i == 0 { JsonError::TypeError } else { JsonError::InvalidValue })?,
    }
    immediately_after_e = matches!(char, b'e' | b'E');
    i += 1;
  }

  if first_char_in_int.is_none() {
    Err(JsonError::TypeError)?;
  }
  // If there was a fractional part yet no exponent, check it had at least one digit
  if !has_exponent {
    if let Some(frac) = frac {
      if frac == (i - 1) {
        Err(JsonError::TypeError)?;
      }
    }
  }
  // If there was an exponent, check it had at least one digit
  if has_exponent && (!exponent_has_digit) {
    Err(JsonError::TypeError)?;
  }

  Ok(i)
}
