use crate::{BytesLike, EpeeError, Type, EpeeEntry, EpeeDecode};

impl EpeeDecode for i8 {
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError> {
    entry.to_i8()
  }
}
impl EpeeDecode for i16 {
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError> {
    match entry.kind() {
      Type::Uint8 => Ok(entry.to_i8()?.into()),
      _ => entry.to_i16(),
    }
  }
}
impl EpeeDecode for i32 {
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError> {
    match entry.kind() {
      Type::Uint8 => Ok(entry.to_i8()?.into()),
      Type::Uint16 => Ok(entry.to_i16()?.into()),
      _ => entry.to_i32(),
    }
  }
}
impl EpeeDecode for i64 {
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError> {
    match entry.kind() {
      Type::Uint8 => Ok(entry.to_i8()?.into()),
      Type::Uint16 => Ok(entry.to_i16()?.into()),
      Type::Uint32 => Ok(entry.to_i32()?.into()),
      _ => entry.to_i64(),
    }
  }
}

impl EpeeDecode for u8 {
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError> {
    entry.to_u8()
  }
}
impl EpeeDecode for u16 {
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError> {
    match entry.kind() {
      Type::Uint8 => Ok(entry.to_u8()?.into()),
      _ => entry.to_u16(),
    }
  }
}
impl EpeeDecode for u32 {
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError> {
    match entry.kind() {
      Type::Uint8 => Ok(entry.to_u8()?.into()),
      Type::Uint16 => Ok(entry.to_u16()?.into()),
      _ => entry.to_u32(),
    }
  }
}
impl EpeeDecode for u64 {
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError> {
    match entry.kind() {
      Type::Uint8 => Ok(entry.to_u8()?.into()),
      Type::Uint16 => Ok(entry.to_u16()?.into()),
      Type::Uint32 => Ok(entry.to_u32()?.into()),
      _ => entry.to_u64(),
    }
  }
}

impl EpeeDecode for f64 {
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError> {
    entry.to_f64()
  }
}

impl EpeeDecode for bool {
  fn decode<'encoding, 'parent, B: BytesLike<'encoding>>(
    entry: EpeeEntry<'encoding, 'parent, B>,
  ) -> Result<Self, EpeeError> {
    entry.to_bool()
  }
}
