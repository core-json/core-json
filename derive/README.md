# `core-json` Derive

A macro to automatically derive `JsonDeserialize` and `JsonStructure` from
[`core-json-traits`](https://docs.rs/core-json-traits).

### Usage

This crate can be used quite directly as follows.

```rs
#[derive(core_json_derive::JsonDeserialize)]
struct MyStruct {
  abc_def: Vec<u8>,
}
```

We do support deserializing fields under a distinct name with the `rename` attribute.

```rs
#[derive(core_json_derive::JsonDeserialize)]
struct MyStruct {
  #[rename("abcDef")]
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
