use monero_epee_traits::EpeeDecode;
use monero_epee_derive::EpeeDecode;

#[derive(PartialEq, Debug, Default, EpeeDecode)]
struct MyStruct<T: 'static + core::fmt::Debug + Default + EpeeDecode> {
  pub abc: u64,
  de: u8,
  pub(crate) ghij: Vec<u8>,
  klmo: Vec<T>,
  missing: Option<u64>,
}

#[derive(PartialEq, Debug, Default, EpeeDecode)]
struct WithoutT {
  abc: u64,
  de: u8,
  ghij: Vec<u8>,
  hash: [u8; 32],
}

#[rustfmt::skip]
#[test]
fn test_derive() {
  use monero_epee_traits::*;

  let res = MyStruct {
    abc: 0xd07bc37ed42c062d,
    de: 0xdd,
    ghij: vec![
      0xee, 0x90, 0x65, 0x00, 0x52, 0x57, 0x1c, 0x9b, 0x94, 0x30, 0x84, 0x68, 0xd7, 0xef, 0xc7,
      0xa6, 0xef, 0xc1, 0xdc, 0xa9, 0x9b, 0xa7, 0x97, 0xf5, 0x48, 0xc9, 0x4c, 0x51, 0xe7, 0x89,
      0xcb, 0x36, 0xf3, 0xd7, 0xa3, 0x2c, 0xe2, 0x09, 0x1f, 0x60, 0x23, 0x35, 0x9b, 0x36, 0x45,
      0xd4, 0x73, 0x3d, 0xcf, 0xcd, 0xd0, 0x01, 0xc7, 0xfa, 0xb6, 0xc3, 0xe7, 0x75, 0x58, 0xe4,
    ],
    klmo: vec![
      WithoutT {
        abc: 0x3b4443c3b3494a61,
        de: 0x2f,
        ghij: vec![
          0x90, 0xba, 0xaa, 0x1f, 0xd9, 0xad, 0xda, 0x28, 0x1f, 0xd2, 0xb7, 0xb3, 0xef, 0x5b,
          0xbc, 0x66, 0x55, 0xc8, 0x74, 0xa6, 0x7b, 0xbf, 0x3f, 0x2a, 0xf0, 0x6d, 0x2c, 0x31,
          0x2a, 0x46, 0x3f, 0x13, 0xf2, 0x77, 0x57,
        ],
        hash: [
          0x32, 0xa8, 0xa1, 0xb9, 0x41, 0xca, 0x82, 0x3d, 0xc9, 0x52, 0xa0, 0x02, 0x91, 0x37, 0xfb,
          0xc0, 0x72, 0x8c, 0xde, 0x18, 0xe2, 0xd5, 0xb8, 0x40, 0xa7, 0x32, 0xae, 0x95, 0x1e, 0xab,
          0x64, 0xce,
        ]
      }
    ],
    missing: None,
  };

  let encoding = [
    HEADER.as_slice(),
    &[VERSION],

    &[4 << 2],

    &[3],
    b"abc",
    &[Type::Uint64 as u8],
    &res.abc.to_le_bytes(),

    &[2],
    b"de",
    &[Type::Uint8 as u8],
    &[res.de],

    &[4],
    b"ghij",
    &[Type::String as u8],
    &((u64::try_from(res.ghij.len()).unwrap() << 2) | 0b11).to_le_bytes(),
    &res.ghij,

    &[4],
    b"klmo",
    &[(Type::Object as u8) | (Array::Array as u8)],
    &[1 << 2],

    &[4 << 2],

    &[3],
    b"abc",
    &[Type::Uint64 as u8],
    &res.klmo[0].abc.to_le_bytes(),

    &[2],
    b"de",
    &[Type::Uint8 as u8],
    &[res.klmo[0].de],

    &[4],
    b"ghij",
    &[Type::String as u8],
    &((u64::try_from(res.klmo[0].ghij.len()).unwrap() << 2) | 0b11).to_le_bytes(),
    &res.klmo[0].ghij,

    &[4],
    b"hash",
    &[Type::String as u8],
    &((u64::try_from(res.klmo[0].hash.len()).unwrap() << 2) | 0b11).to_le_bytes(),
    &res.klmo[0].hash,
  ].concat();
  let encoding: &[u8] = encoding.as_slice();

  assert_eq!(MyStruct::<WithoutT>::decode_root(encoding).unwrap(), res);
}
