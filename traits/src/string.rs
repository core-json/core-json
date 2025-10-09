use crate::JsonSerialize;

#[cfg(feature = "alloc")]
use crate::{BytesLike, Stack, JsonError, Value, JsonDeserialize};
#[cfg(feature = "alloc")]
impl JsonDeserialize for alloc::string::String {
  fn deserialize<'bytes, 'parent, B: BytesLike<'bytes>, S: Stack>(
    value: Value<'bytes, 'parent, B, S>,
  ) -> Result<Self, JsonError<'bytes, B, S>> {
    value.to_str()?.collect()
  }
}

struct CharIterator<I: Iterator<Item = char>> {
  iter: I,
  buf: [char; 12],
  queued: usize,
}
impl<I: Iterator<Item = char>> Iterator for CharIterator<I> {
  type Item = char;
  fn next(&mut self) -> Option<Self::Item> {
    // If we don't have a character in progress, fetch the next character
    if self.queued == 0 {
      let char = self.iter.next()?;

      match u32::from(char) {
        // Unescaped
        0x20 ..= 0x21 | 0x23 ..= 0x5b | 0x5d ..= 0x10ffff => {
          self.buf[self.buf.len() - 1] = char;
          self.queued = 1;
        }
        // Escaped with a special case
        #[allow(unreachable_patterns)]
        0x22 | 0x5c | 0x2f | 0x62 | 0x66 | 0x6e | 0x72 | 0x74 => {
          self.buf[self.buf.len() - 2] = '\\';
          self.buf[self.buf.len() - 1] = match u32::from(char) {
            // Handle characters whose aliases are distinct from them themselves
            0x62 => 'b',
            0x66 => 'f',
            0x6e => 'n',
            0x72 => 'r',
            0x74 => 't',
            // Handle characters whose aliases are equivalent to them themselves
            _ => char,
          };
          self.queued = 2;
        }
        _ => {
          // Encode this character as UTF-16
          let mut elems = [0; 2];
          let elems = char.encode_utf16(&mut elems);
          for (i, b) in elems.iter().enumerate() {
            // If we only have one element, write it to the second position
            let i = (((2 - elems.len()) + i) * 6) + 2;
            // Convert to hex
            for n in 0 .. 4 {
              // Safe to cast as this is masked with 0b1111 (a 4-bit value)
              let nibble = (((*b) >> (16 - ((n + 1) * 4))) & 0b1111) as u8;
              // Safe to cast as this will be `'0' ..= '9' | 'a' ..= 'f``
              self.buf[i + n] = (if let Some(value) = nibble.checked_sub(10) {
                b'a' + value
              } else {
                b'0' + nibble
              }) as char;
            }
          }
          self.queued = elems.len() * 6;
        }
      }
    }

    // Yield the next character queued
    let res = self.buf[12 - self.queued];
    // Increment
    self.queued -= 1;
    Some(res)
  }
}
impl JsonSerialize for str {
  fn serialize(&self) -> impl Iterator<Item = char> {
    core::iter::once('"')
      .chain(CharIterator {
        iter: self.chars(),
        buf: ['\\', 'u', 'F', 'F', 'F', 'F', '\\', 'u', 'F', 'F', 'F', 'F'],
        queued: 0,
      })
      .chain(core::iter::once('"'))
  }
}

#[cfg(feature = "alloc")]
impl JsonSerialize for alloc::string::String {
  fn serialize(&self) -> impl Iterator<Item = char> {
    self.as_str().serialize()
  }
}
