use crate::{Read, Stack, JsonError, Value, JsonDeserialize, JsonSerialize};

struct IntInterator<const CAPACITY: usize> {
  buf: [u8; CAPACITY],
  i: usize,
  len: usize,
}
impl<const CAPACITY: usize> IntInterator<CAPACITY> {
  fn new(value: impl core::fmt::Display) -> Self {
    use core::fmt::Write;

    /// A `core::fmt::Write` which writes to a slice.
    ///
    /// We use this to achieve a non-allocating `core::fmt::Write` for primitives we know a bound
    /// for.
    struct SliceWrite<'a>(&'a mut [u8], usize);
    impl<'a> Write for SliceWrite<'a> {
      #[inline(always)]
      fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let remaining = self.0.len() - self.1;
        if remaining < s.len() {
          Err(core::fmt::Error)?;
        }
        self.0[self.1 .. (self.1 + s.len())].copy_from_slice(s.as_bytes());
        self.1 += s.len();
        Ok(())
      }
    }

    let mut buf = [0; CAPACITY];
    let mut writer = SliceWrite(&mut buf, 0);
    write!(&mut writer, "{}", value)
      .expect("integer primitive exceeded CAPACITY of base-10 digits");
    let len = writer.1;

    IntInterator { buf, i: 0, len }
  }
}
impl<const CAPACITY: usize> Iterator for IntInterator<CAPACITY> {
  type Item = char;
  fn next(&mut self) -> Option<Self::Item> {
    if self.i == self.len {
      None?;
    }
    let result = self.buf[self.i];
    self.i += 1;
    // This is a safe cast so long as Rust's display of an `u64` yields ASCII
    Some(result as char)
  }
}

macro_rules! int_primitive {
  ($int: ident) => {
    impl JsonDeserialize for $int {
      fn deserialize<'read, 'parent, B: Read<'read>, S: Stack>(
        value: Value<'read, 'parent, B, S>,
      ) -> Result<Self, JsonError<'read, B, S>> {
        value
          .to_number()?
          .i64()
          .ok_or(JsonError::TypeError)?
          .try_into()
          .map_err(|_| JsonError::TypeError)
      }
    }

    impl JsonSerialize for $int {
      fn serialize(&self) -> impl Iterator<Item = char> {
        const CAPACITY: usize = {
          const BITS: usize = 8 * core::mem::size_of::<$int>();
          /*
            Since this number may be up to `2^{BITS}`, we check `(1 + {BITS / 3}) > CAPACITY`.
            This handles one digit for `+/-` and conservatively approximates `10` as `2^3`.

            This makes the `expect` in `IntInterator` safe for any sane definition of Rust.
          */
          1 + BITS.div_ceil(3)
        };

        IntInterator::<CAPACITY>::new(*self)
      }
    }
  };
}
int_primitive!(i8);
int_primitive!(i16);
int_primitive!(i32);
int_primitive!(i64);
int_primitive!(i128);
int_primitive!(isize);
int_primitive!(u8);
int_primitive!(u16);
int_primitive!(u32);
int_primitive!(u64);
int_primitive!(u128);
int_primitive!(usize);

impl JsonDeserialize for bool {
  fn deserialize<'read, 'parent, B: Read<'read>, S: Stack>(
    value: Value<'read, 'parent, B, S>,
  ) -> Result<Self, JsonError<'read, B, S>> {
    value.to_bool()
  }
}

/// Deserialize `null` as `()`.
impl JsonDeserialize for () {
  fn deserialize<'read, 'parent, B: Read<'read>, S: Stack>(
    value: Value<'read, 'parent, B, S>,
  ) -> Result<Self, JsonError<'read, B, S>> {
    value.to_null()
  }
}

impl JsonSerialize for bool {
  fn serialize(&self) -> impl Iterator<Item = char> {
    (if *self { "true" } else { "false" }).chars()
  }
}

/// Serialize `()` as `null`.
impl JsonSerialize for () {
  fn serialize(&self) -> impl Iterator<Item = char> {
    "null".chars()
  }
}

#[test]
fn test_int_iterator() {
  assert_eq!(JsonSerialize::serialize(&0u8).collect::<String>(), "0");
  assert_eq!(JsonSerialize::serialize(&1u8).collect::<String>(), "1");
  assert_eq!(JsonSerialize::serialize(&u64::MAX).collect::<String>(), format!("{}", u64::MAX));
  assert_eq!(JsonSerialize::serialize(&i64::MAX).collect::<String>(), format!("{}", i64::MAX));
  assert_eq!(JsonSerialize::serialize(&i64::MIN).collect::<String>(), format!("{}", i64::MIN));
}
