use crate::{BytesLike, Stack, JsonError};

/// Interpret the immediate value within the bytes as a number.
///
/// Returns the length of the number's serialization and the number as a `f64`.
pub fn as_number<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &B,
) -> Result<(usize, f64), JsonError<'bytes, B, S>> {
  /*
    https://datatracker.ietf.org/doc/html/rfc8259#section-6 lets us specify the range, precision
    of numbers. We take advantage of this to only work with floats where the sum of the integer's
    length, sum of the fraction's length, and exponent's length fits within the following bound.
  */
  const MAX_FLOAT_LEN: usize = 128;
  let mut str: [u8; MAX_FLOAT_LEN] = [0; MAX_FLOAT_LEN];

  let mut i = 0;
  let mut frac = None;
  let mut has_exponent = false;
  let mut immediately_after_e = false;
  let mut first_char_in_int = None;
  loop {
    let char = bytes.peek(i).map_err(|_| JsonError::TypeError)?;
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
      b'0' => first_char_in_int = first_char_in_int.or(Some(char)),
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
      _ => break,
    }
    immediately_after_e = matches!(char, b'e' | b'E');
    if i == MAX_FLOAT_LEN {
      Err(JsonError::TypeError)?;
    }
    str[i] = char;
    i += 1;
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
  if has_exponent && (!str[i - 1].is_ascii_digit()) {
    Err(JsonError::TypeError)?;
  }

  let str = core::str::from_utf8(&str[.. i]).map_err(|_| JsonError::InternalError)?;
  let res = <f64 as core::str::FromStr>::from_str(str).map_err(|_| JsonError::TypeError)?;
  Ok((str.len(), res))
}
