use crate::{AsyncRead, PeekableRead, Stack, JsonError};

/// Calculate the length of the non-ASCII UTF-8 codepoint from its first byte.
///
/// Returns an undefined value if the codepoint is ASCII.
#[inline(always)]
fn non_ascii_utf8_codepoint_len(b: u8) -> usize {
  // The amount of zeroes in a `u8` will be positive and fit within a `usize`
  ((!(b | 0b0100_0000)) | 0b1111).leading_zeros() as usize
}

/// Convert a UTF-8 codepoint to a `char`.
#[inline(always)]
fn utf8_codepoint_to_char<'read, R: AsyncRead<'read>, S: Stack>(
  c: &[u8],
) -> Result<char, JsonError<'read, R, S>> {
  // https://en.wikipedia.org/wiki/UTF-8#Description
  // The last six bits of every byte, except the first for which it depends on the length of the
  // entire codepoint
  const SIX_BITS: u8 = 0b0011_1111;
  char::from_u32(match c.len() {
    1 => u32::from(c[0]),
    2 => (u32::from(c[0] & 0b0001_1111) << 6) | u32::from(c[1] & SIX_BITS),
    3 => {
      (((u32::from(c[0] & 0b0000_1111) << 6) | u32::from(c[1] & SIX_BITS)) << 6) |
        u32::from(c[2] & SIX_BITS)
    }
    4 => {
      (((((u32::from(c[0] & 0b0000_0111) << 6) | u32::from(c[1] & SIX_BITS)) << 6) |
        u32::from(c[2] & SIX_BITS)) <<
        6) |
        u32::from(c[3] & SIX_BITS)
    }
    _ => unreachable!("non-ASCII codepoints have length in `2 ..= 4`"),
  })
  .ok_or(JsonError::InvalidValue)
}

/// Read a non-ASCII UTF-8 character from a `AsyncRead`.
#[inline(always)]
pub(super) async fn read_non_ascii_utf8<'read, R: AsyncRead<'read>, S: Stack>(
  reader: &mut PeekableRead<'read, R>,
  first_byte: u8,
) -> Result<char, JsonError<'read, R, S>> {
  let utf8_codepoint_len = non_ascii_utf8_codepoint_len(first_byte);

  let mut utf8_codepoint = [0; 4];
  let utf8_codepoint = &mut utf8_codepoint[.. utf8_codepoint_len];
  utf8_codepoint[0] = first_byte;
  for byte in &mut utf8_codepoint[1 ..] {
    *byte = reader.read_byte().await.map_err(JsonError::ReadError)?;
  }
  utf8_codepoint_to_char(utf8_codepoint)
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
fn test_utf8_codepoint_to_char() {
  let mut unicode = 1;
  while char::from_u32(unicode).map(char::len_utf8).unwrap_or(0) != 2 {
    unicode <<= 1;
  }
  assert_eq!(
    utf8_codepoint_to_char::<&[u8], crate::ConstStack<0>>(
      char::from_u32(unicode).unwrap().to_string().as_bytes()
    )
    .unwrap(),
    char::from_u32(unicode).unwrap()
  );
  while char::from_u32(unicode).map(char::len_utf8).unwrap_or(0) != 3 {
    unicode <<= 1;
  }
  assert_eq!(
    utf8_codepoint_to_char::<&[u8], crate::ConstStack<0>>(
      char::from_u32(unicode).unwrap().to_string().as_bytes()
    )
    .unwrap(),
    char::from_u32(unicode).unwrap()
  );
  while char::from_u32(unicode).map(char::len_utf8).unwrap_or(0) != 4 {
    unicode <<= 1;
  }
  assert_eq!(
    utf8_codepoint_to_char::<&[u8], crate::ConstStack<0>>(
      char::from_u32(unicode).unwrap().to_string().as_bytes()
    )
    .unwrap(),
    char::from_u32(unicode).unwrap()
  );
}

#[test]
fn bench_utf8_codepoint_to_char() {
  #[cfg(debug_assertions)]
  const ITERATIONS: u64 = 1_000_000_000u64;
  #[cfg(not(debug_assertions))]
  const ITERATIONS: u64 = 20_000_000_000u64;

  let utf8_codepoint = "\u{FFFF}".as_bytes();
  {
    let start = std::time::Instant::now();
    for _ in 0 .. ITERATIONS {
      let _ = core::hint::black_box(
        utf8_codepoint_to_char::<&[u8], crate::ConstStack<0>>(core::hint::black_box(
          utf8_codepoint,
        ))
        .unwrap(),
      );
    }
    println!("`utf8_codepoint_to_char` took {}ms", start.elapsed().as_millis());
  }

  {
    let start = std::time::Instant::now();
    for _ in 0 .. ITERATIONS {
      let str = core::str::from_utf8(core::hint::black_box(utf8_codepoint)).unwrap();
      let _ = core::hint::black_box(str.chars().next().unwrap());
    }
    println!("`core::str::from_utf8().chars().next()` took {}ms", start.elapsed().as_millis());
  }
}
