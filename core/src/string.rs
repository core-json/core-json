use core::marker::PhantomData;

use crate::{BytesLike, String, Stack, JsonError};

/// Calculate the length of the non-ASCII UTF-8 codepoint from its first byte.
///
/// Returns an undefined value if the codepoint is ASCII.
#[inline(always)]
fn non_ascii_utf8_codepoint_len(b: u8) -> usize {
  // The amount of zeroes in a `u8` will be positive and fit within a `usize`
  ((!(b | 0b0100_0000)) | 0b1111).leading_zeros() as usize
}

/// Peek a UTF-8 character from bytes.
///
/// Returns the length when encoded and the character itself.
#[inline(always)]
fn peek_utf8<'bytes, B: BytesLike<'bytes>, S: Stack>(
  bytes: &B,
  i: usize,
) -> Result<(usize, char), JsonError<'bytes, B, S>> {
  let mut utf8_codepoint = [0; 4];
  utf8_codepoint[0] = bytes.peek(i).map_err(JsonError::BytesError)?;
  // If this is ASCII, immediately return it.
  if (utf8_codepoint[0] >> 7) == 0 {
    return Ok((1, utf8_codepoint[0] as char));
  }
  let utf8_codepoint_len = non_ascii_utf8_codepoint_len(utf8_codepoint[0]);

  let utf8_codepoint = &mut utf8_codepoint[.. utf8_codepoint_len];
  for (j, byte) in utf8_codepoint.iter_mut().enumerate().skip(1) {
    *byte = bytes.peek(i + j).map_err(JsonError::BytesError)?;
  }

  let str = core::str::from_utf8(utf8_codepoint).map_err(|_| JsonError::InvalidValue)?;
  Ok((utf8_codepoint_len, str.chars().next().ok_or(JsonError::InternalError)?))
}

#[must_use]
#[inline(always)]
fn validate_hex(bytes: [u8; 4]) -> bool {
  /*
    "Mom, can we have SIMD?"
    "We have SIMD at home."
    SIMD at home:
  */

  // We don't care for the order of these bytes within our `u32`
  let bytes = u32::from_ne_bytes(bytes);

  const HIGH_BIT: u32 = 1 << 7;
  const HIGH_BITS: u32 = (HIGH_BIT << 24) | (HIGH_BIT << 16) | (HIGH_BIT << 8) | HIGH_BIT;

  const ZERO_CHAR: u32 =
    ((b'0' as u32) << 24) | ((b'0' as u32) << 16) | ((b'0' as u32) << 8) | (b'0' as u32);
  const DISTANCE_AFTER_NINE: u32 = HIGH_BIT - ((b'9' + 1) as u32);
  const DISTANCES_AFTER_NINE: u32 = (DISTANCE_AFTER_NINE << 24) |
    (DISTANCE_AFTER_NINE << 16) |
    (DISTANCE_AFTER_NINE << 8) |
    DISTANCE_AFTER_NINE;

  const FIFTH_BIT: u32 = 1 << 5;
  const FIFTH_BITS: u32 = (FIFTH_BIT << 24) | (FIFTH_BIT << 16) | (FIFTH_BIT << 8) | FIFTH_BIT;

  const A_CHAR: u32 =
    ((b'a' as u32) << 24) | ((b'a' as u32) << 16) | ((b'a' as u32) << 8) | (b'a' as u32);
  const DISTANCE_AFTER_F: u32 = HIGH_BIT - ((b'f' + 1) as u32);
  const DISTANCES_AFTER_F: u32 = (DISTANCE_AFTER_F << 24) |
    (DISTANCE_AFTER_F << 16) |
    (DISTANCE_AFTER_F << 8) |
    DISTANCE_AFTER_F;

  /*
    If these bytes are ASCII, their high bits won't be set, allowing us to use the eighth bits as
    shields for carries/borrows across the lanes we've defined within the `u32`.
  */
  let bytes_with_high_bits = bytes | HIGH_BITS;

  /*
    We subtract our constants from our packed bytes, with their high bits set. If the
    constant (< 128) exceeds the value within the lower seven bits of each byte, it'll cause the
    eigth bit to be carried, leaving it not set. This lets us efficiently check if the packed
    values are greater than the constants.
  */
  let gte_zero = bytes_with_high_bits.wrapping_sub(ZERO_CHAR);
  /*
    `'a' ..= 'f'` have their fifth bits set. `'A' ..= 'F'` do not, where `A + 32 == 'a'`. This OR
    lets us collapse checking the `'A' ..= 'F'` case into the `'a' ..= 'f'` case.
  */
  let gte_a = (bytes_with_high_bits | FIFTH_BITS).wrapping_sub(A_CHAR);

  /*
    We now add our constants to our packed bytes, where our constants are the distance from a
    boundary to the eight bit. If the constant causes the value's eigth bit to be set, then the
    value was greater than or ewqual to the boundary (as else, it'd be insufficient to carry to the
    eighth bit). This lets us efficiently check if the packed values are less than constants.
  */
  let lte_9 = bytes.wrapping_add(DISTANCES_AFTER_NINE);
  let lte_f = (bytes | FIFTH_BITS).wrapping_add(DISTANCES_AFTER_F);

  /*
    The following use XOR as a combiner, as we want the gte bits set and the lte bits unset. The
    XOR operator would allow the gte bits to not be set, while the lte bits are set, yet any value
    which isn't less than the end of the range will be greater than the start of the range. This
    collapses the possible states to just three:
    - gte bit set, lte bit not set (valid)
    - gte bit set, lte bit set (too high)
    - gte bit not set, lte bit not set (too low)
    The XOR operator is sufficient to isolate the valid state.
  */
  let number = gte_zero ^ lte_9;
  let alpha = gte_a ^ lte_f;
  let number_or_alpha = number | alpha;
  // Finally, require these values to have been ASCII to so these values are well-defined
  let ascii = (!bytes) & HIGH_BITS;
  (ascii & number_or_alpha) == HIGH_BITS
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
      let (_, this) = peek_utf8(bytes, i)?;

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
        if this == '\x75' {
          // We use `peek`, not `peek_utf8` here, as hex characters will be ASCII
          let bytes = [
            bytes.peek(i + 1).map_err(JsonError::BytesError)?,
            bytes.peek(i + 2).map_err(JsonError::BytesError)?,
            bytes.peek(i + 3).map_err(JsonError::BytesError)?,
            bytes.peek(i + 4).map_err(JsonError::BytesError)?,
          ];

          if !validate_hex(bytes) {
            Err(JsonError::InvalidValue)?;
          }
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
pub(crate) struct UnescapeString<'bytes, B: BytesLike<'bytes>, S: Stack> {
  string: B,
  remaining: usize,
  _stack: PhantomData<(&'bytes (), S)>,
}
impl<'bytes, B: BytesLike<'bytes>, S: Stack> From<String<'bytes, B>>
  for UnescapeString<'bytes, B, S>
{
  #[inline(always)]
  fn from(string: String<'bytes, B>) -> Self {
    Self { remaining: string.len(), string: string.consume(), _stack: PhantomData }
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
        let (len, next_char) = peek_utf8(&self.string, 0)?;
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
          char::from_u32(codepoint).ok_or(JsonError::InvalidValue)
        }
        // `InternalError`: `\` without a recognized following character
        _ => Err(JsonError::InternalError),
      }
    })();

    // If the result was an error, set `remaining = 0` so all future calls to `next` yield `None`
    if res.is_err() {
      self.remaining = 0;
    }

    Some(res)
  }
}

#[test]
fn test_non_ascii_utf8_codepoint_len() {
  let mut unicode = 1;
  while char::from_u32(unicode).map(char::len_utf8).unwrap_or(0) != 2 {
    unicode <<= 1;
  }
  assert_eq!(
    non_ascii_utf8_codepoint_len(char::from_u32(unicode).unwrap().to_string().as_bytes()[0]),
    2
  );
  while char::from_u32(unicode).map(char::len_utf8).unwrap_or(0) != 3 {
    unicode <<= 1;
  }
  assert_eq!(
    non_ascii_utf8_codepoint_len(char::from_u32(unicode).unwrap().to_string().as_bytes()[0]),
    3
  );
  while char::from_u32(unicode).map(char::len_utf8).unwrap_or(0) != 4 {
    unicode <<= 1;
  }
  assert_eq!(
    non_ascii_utf8_codepoint_len(char::from_u32(unicode).unwrap().to_string().as_bytes()[0]),
    4
  );
}

#[test]
fn bench_non_ascii_utf8_codepoint_len() {
  #[cfg(debug_assertions)]
  const ITERATIONS: u64 = 1_000_000_000u64;
  #[cfg(not(debug_assertions))]
  const ITERATIONS: u64 = 20_000_000_000u64;

  let unicode = "\u{FFFF}".as_bytes()[0];
  {
    let start = std::time::Instant::now();
    for _ in 0 .. ITERATIONS {
      let _ = core::hint::black_box(non_ascii_utf8_codepoint_len(core::hint::black_box(unicode)));
    }
    println!("`non_ascii_utf8_codepoint_len` took {}ms", start.elapsed().as_millis());
  }

  {
    let start = std::time::Instant::now();
    for _ in 0 .. ITERATIONS {
      let b = core::hint::black_box(unicode);
      let _ = core::hint::black_box(usize::from({
        let third_bit_set = (b >> 5) & 1;
        // We don't have to `& 1` here as we take it with `third_bit_set` which has `& 1`
        let fourth_bit_set = b >> 4;
        2u8 + third_bit_set + (third_bit_set & fourth_bit_set)
      }));
    }
    println!("bit-shifting implementation took {}ms", start.elapsed().as_millis());
  }
}

#[test]
fn test_validate_hex() {
  let mut hex = [0, b'f', b'f', b'f'];
  for i in u8::MIN ..= u8::MAX {
    hex[0] = i;
    assert_eq!(validate_hex(hex), hex[0].is_ascii_hexdigit());
  }
}

#[test]
fn bench_validate_hex() {
  #[cfg(debug_assertions)]
  const ITERATIONS: u64 = 1_000_000_000u64;
  #[cfg(not(debug_assertions))]
  const ITERATIONS: u64 = 20_000_000_000u64;

  let hex = *b"ffff";
  {
    let start = std::time::Instant::now();
    for _ in 0 .. ITERATIONS {
      let _ = core::hint::black_box(validate_hex(core::hint::black_box(hex)));
    }
    println!("`validate_hex` took {}ms", start.elapsed().as_millis());
  }
  {
    let start = std::time::Instant::now();
    for _ in 0 .. ITERATIONS {
      let hex = core::hint::black_box(hex);
      core::hint::black_box(
        hex[0].is_ascii_hexdigit() &&
          hex[1].is_ascii_hexdigit() &&
          hex[2].is_ascii_hexdigit() &&
          hex[3].is_ascii_hexdigit(),
      );
    }
    println!("4 * `is_ascii_hexdigit` took {}ms", start.elapsed().as_millis());
  }
}
