# `core-json` Derive

A macro to automatically derive `JsonDeserialize` and `JsonStructure` from
[`core-json-traits`](https://docs.rs/core-json-traits).

### Usage

This crate can be used quite directly as follows.

```rs
#[derive(core_json_derive::JsonDeserialize, core_json_derive::JsonSerialize)]
struct MyStruct {
  abc_def: Vec<u8>,
}
```

Then, deserialization may occur as follows.

```rs
use core_json_traits::*;
MyStruct::deserialize_structure::<_, ConstStack<32>>(serialization).unwrap()
```

where `serialization: &[u8]`. The constant parameter for `ConstStack`
determines how deep objects within the serialization are allowed to be. To
support objects of unbounded depth, `Vec` may be used, but this is not
recommended due to denial of service concerns.

Serialization to a `String` may occur as follows.

```rs
use core_json_traits::*;
my_struct.serialize().collect::<String>()
```

### `key` Attribute

We support (de)serializing fields with a key distinct from their names via the
`key` attribute.

```rs
#[derive(core_json_derive::JsonDeserialize, core_json_derive::JsonSerialize)]
struct MyStruct {
  #[key("abcDef")]
  abc_def: Vec<u8>,
}
```

### `skip` Attribute

We support omitting fields from (de)serialization with the `skip` attribute.

```rs
#[derive(core_json_derive::JsonDeserialize, core_json_derive::JsonSerialize)]
struct MyStruct {
  #[skip]
  abc_def: Vec<u8>,
}
```

The attribute will not be serialized and will not be read when deserializing,
even if present within the serialization.
