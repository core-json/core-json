use alloc::{vec, string::String};

use crate::{BytesLike, Stack, JsonError, Value, JsonDeserialize};

impl JsonDeserialize for String {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    // Read the bytes into an owned buffer
    let mut bytes = {
      let str = value.to_str()?;
      let mut bytes = vec![0; str.len()];
      str.consume().read_into_slice(&mut bytes).map_err(JsonError::BytesError)?;
      bytes
    };

    // Un-escape the string
    /*
      We do this in a single-pass, without additional allocations, as follows.
      - `dst = 0, src = 0`
      - Iterate over each byte
        - If not escape sequence, `bytes[dst] = bytes[src]`, increment `src` and `dst`
        - If escape sequence, increment `src`
      - `bytes.truncate(dst)`
      This executes in time linear to the length of the string and avoids the quadratic complexity
      `Vec::remove` would incur (due to the linear-complexity shifts incurred with _each_ removal).
    */
    {
      let mut dst = 0;
      let mut src = 0;

      // The following loop works off the invariant `dst <= src`
      while src < bytes.len() {
        // If this isn't an escape character, move to the next
        if bytes[src] != b'\\' {
          bytes[dst] = bytes[src];
          src += 1;
          dst += 1;
          continue;
        }

        // Advance past the '\` character
        src += 1;

        // Definitions from https://datatracker.ietf.org/doc/html/rfc8259#section-7
        match bytes.get(src).ok_or(JsonError::InternalError)? {
          /*
            If this is to escape the intended character, copy it now.
          */
          b'"' | b'\\' | b'/' => {
            bytes[dst] = bytes[src];
            src += 1;
            dst += 1;
          }
          /*
            If this is a control sequence, overwrite it with the intended value. The next iteration
            will perform the actual copy.
          */
          b'b' => {
            bytes[src] = b'\x08';
          }
          b'f' => {
            bytes[src] = b'\x0c';
          }
          b'n' => {
            bytes[src] = b'\n';
          }
          b'r' => {
            bytes[src] = b'\r';
          }
          b't' => {
            bytes[src] = b'\t';
          }

          // Handle if this is a unicode codepoint
          b'u' => {
            // Advance past the 'u' character
            src += 1;

            let read_hex = |bytes: &[u8]| {
              let hex = core::str::from_utf8(bytes).map_err(|_| JsonError::InternalError)?;
              u16::from_str_radix(hex, 16).map(u32::from).map_err(|_| JsonError::InternalError)
            };

            // Read the hex digits
            if bytes.len() < src.checked_add(4).ok_or(JsonError::InternalError)? {
              Err(JsonError::InternalError)?;
            }
            let first = read_hex(&bytes[src .. (src + 4)])?;
            src += 4;

            /*
              If the intended value of this codepoint exceeds 0xffff, it's specified to be encoded
              with its UTF-16 surrogate pair. We distinguish and fetch the second part if necessary
              now. For the actual conversion algorithm from the UTF-16 surrogate pair to the UTF-8
              codepoint, https://en.wikipedia.org/wiki/UTF-16#U+D800_to_U+DFFF_(surrogates) is
              used as reference.
            */
            let codepoint = if let Some(high) = first.checked_sub(0xd800) {
              // Read the low part of the surrogate pair
              if bytes.len() < src.checked_add(4).ok_or(JsonError::InternalError)? {
                // This is `InvalidValue`, not `InternalError`, as this isn't a valid UTF-8
                // character without this second part (which was omitted)
                Err(JsonError::InvalidValue)?;
              }
              let low = read_hex(&bytes[src .. (src + 4)])?;
              src += 4;

              // Calculate the codepoint
              let high = high << 10;
              let low = low.checked_sub(0xdc00).ok_or(JsonError::InvalidValue)?;
              high + low + 0x10000
            } else {
              first
            };

            // Write the codepoint to `bytes[dst ..]`
            let char = char::from_u32(codepoint).ok_or(JsonError::InvalidValue)?;
            // https://datatracker.ietf.org/doc/html/rfc8259#section-8.1
            if (dst == 0) && (char == '\u{feff}') {
              Err(JsonError::InvalidValue)?;
            }
            // This uses `str::len` which is in term of bytes (surprisingly)
            dst += char.encode_utf8(&mut bytes[dst .. (dst + 4)]).len();
          }
          // Unrecognized escaped character
          _ => Err(JsonError::InternalError)?,
        }
      }

      // Truncate to only the part written to
      bytes.truncate(dst);
    }

    String::from_utf8(bytes).map_err(|_| JsonError::InvalidValue)
  }
}
