use core_json_traits::{JsonF64, JsonDeserialize, JsonSerialize};
use core_json_derive::{JsonDeserialize, JsonSerialize};

#[derive(Clone, Debug, Default, JsonDeserialize, JsonSerialize)]
pub(crate) struct MyStruct<
  T: 'static + core::fmt::Debug + Default + JsonDeserialize + JsonSerialize,
> {
  pub abc: u64,
  de: u8,
  pub(crate) ghij: Vec<u8>,
  klmo: Vec<T>,
  missing: Option<u64>,
  float: JsonF64,
  #[skip]
  skipped: Option<u64>,
}

impl<T: 'static + core::fmt::Debug + Default + JsonDeserialize + JsonSerialize> PartialEq
  for MyStruct<T>
where
  T: PartialEq,
{
  fn eq(&self, other: &Self) -> bool {
    (self.abc == other.abc) &&
      (self.de == other.de) &&
      (self.ghij == other.ghij) &&
      (self.klmo == other.klmo) &&
      (self.missing == other.missing) &&
      (f64::from(self.float) == f64::from(other.float))
  }
}

#[derive(Clone, PartialEq, Debug, Default, JsonDeserialize, JsonSerialize)]
pub struct WithoutT {
  #[key("xyza")]
  abc: i64,
  de: u8,
  ghij: Vec<u8>,
  hash: [u8; 32],
}

#[rustfmt::skip]
#[test]
fn test_derive() {
  use core_json_traits::*;

  let res = MyStruct {
    abc: 0x707bc37ed42c062d,
    de: 0xdd,
    ghij: vec![
      0xee, 0x90, 0x65, 0x00, 0x52, 0x57, 0x1c, 0x9b, 0x94, 0x30, 0x84, 0x68, 0xd7, 0xef, 0xc7,
      0xa6, 0xef, 0xc1, 0xdc, 0xa9, 0x9b, 0xa7, 0x97, 0xf5, 0x48, 0xc9, 0x4c, 0x51, 0xe7, 0x89,
      0xcb, 0x36, 0xf3, 0xd7, 0xa3, 0x2c, 0xe2, 0x09, 0x1f, 0x60, 0x23, 0x35, 0x9b, 0x36, 0x45,
      0xd4, 0x73, 0x3d, 0xcf, 0xcd, 0xd0, 0x01, 0xc7, 0xfa, 0xb6, 0xc3, 0xe7, 0x75, 0x58, 0xe4,
    ],
    klmo: vec![
      WithoutT {
        abc: -0x3b4443c3b3494a61,
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
    float: JsonF64::try_from(-123.456).unwrap(),
    skipped: None,
  };

  let serialized = r#"
    {
      "abc": 8105286903876290093,
      "de": 221,
      "ghij": [
        238, 144, 101, 0, 82, 87, 28, 155, 148, 48, 132, 104, 215, 239, 199, 166, 239, 193, 220,
        169, 155, 167, 151, 245, 72, 201, 76, 81, 231, 137, 203, 54, 243, 215, 163, 44, 226, 9,
        31, 96, 35, 53, 155, 54, 69, 212, 115, 61, 207, 205, 208, 1, 199, 250, 182, 195, 231, 117,
        88, 228
      ],
      "klmo":[
        {
          "xyza": -4270612854459681377,
          "de": 47,
          "ghij": [
            144, 186, 170, 31, 217, 173, 218, 40, 31, 210, 183, 179, 239, 91, 188, 102, 85, 200,
            116, 166, 123, 191, 63, 42, 240, 109, 44, 49, 42, 70, 63, 19, 242, 119, 87
          ],
          "hash": [
            50, 168, 161, 185, 65, 202, 130, 61, 201, 82, 160, 2, 145, 55, 251, 192, 114, 140, 222,
            24, 226, 213, 184, 64, 167, 50, 174, 149, 30, 171, 100, 206
          ]
        }
      ],
      "missing": null,
      "float": -123.456,
      "skipped": 10
    }
  "#;

  {
    let deserialized =
      MyStruct::<WithoutT>::deserialize_structure::<_, core_json_traits::ConstStack<128>>(
        serialized.as_bytes(),
      ).unwrap();
    assert!(deserialized.skipped.is_none(), "`skipped` was `Some` despite `skip` attribute");
    assert_eq!(deserialized, res);
  }

  {
    let mut res = res.clone();
    res.skipped = Some(10);
    let serialization = res.serialize().collect::<String>();
    assert!(
      !serialization.contains("skipped"),
      "`skipped` was serialized despite `skip` attribute",
    );
    assert_eq!(
      MyStruct::<WithoutT>::deserialize_structure::<_, core_json_traits::ConstStack<128>>(
        serialization.as_bytes(),
      ).unwrap(),
      res,
    );
  }

  assert_eq!(
    <[MyStruct::<WithoutT>; 1]>::deserialize_structure::<_, core_json_traits::ConstStack<128>>(
      ("[".to_string() + serialized + "]").as_bytes(),
    ).unwrap(),
    core::slice::from_ref(&res),
  );

  assert_eq!(
    <[MyStruct::<WithoutT>; 1]>::deserialize_structure::<_, core_json_traits::ConstStack<128>>(
      [res.clone()].serialize().collect::<String>().as_bytes(),
    ).unwrap(),
    [res],
  );
}
