use crate::{BytesLike, Stack, JsonError};

#[must_use]
#[inline(always)]
pub(super) fn validate_hex(bytes: [u8; 4]) -> bool {
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

/// Read a `u16` from its big-endian hexadecimal encoding.
#[inline(always)]
pub(super) fn read_hex<'bytes, B: BytesLike<'bytes>, S: Stack>(
  hex: [u8; 4],
) -> Result<u32, JsonError<'bytes, B, S>> {
  #[inline(always)]
  fn hex_char<'bytes, B: BytesLike<'bytes>, S: Stack>(
    char: u8,
  ) -> Result<u16, JsonError<'bytes, B, S>> {
    Ok(match char {
      b'0' => 0,
      b'1' => 1,
      b'2' => 2,
      b'3' => 3,
      b'4' => 4,
      b'5' => 5,
      b'6' => 6,
      b'7' => 7,
      b'8' => 8,
      b'9' => 9,
      b'a' | b'A' => 10,
      b'b' | b'B' => 11,
      b'c' | b'C' => 12,
      b'd' | b'D' => 13,
      b'e' | b'E' => 14,
      b'f' | b'F' => 15,
      _ => Err(JsonError::InternalError)?,
    })
  }
  Ok(u32::from(
    (hex_char(hex[0])? << 12) |
      (hex_char(hex[1])? << 8) |
      (hex_char(hex[2])? << 4) |
      hex_char(hex[3])?,
  ))
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
