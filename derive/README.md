# `core-json` Derive

A macro to automatically derive `JsonDeserialize` from
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
