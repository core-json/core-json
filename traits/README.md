# `core-json` Traits

Traits for working with objects which may be deserialized from/serialized into
JSON.

Internally, this uses the [`core-json`](https://docs.rs/core-json) crate for
deserialization. That crate focuses on being minimal, only offering a
dynamically-typed view as JSON-serialized data is processed. This crate
additionally adds traits for deserializing into typed objects.

This crate also defines a trait and implementations for serializing into JSON.
In order to maintain support for `core`, the serializers return
`impl Iterator<Item = char>` (which may be transformed into a `String` by
calling `.collect::<String>()` on the iterator).

When the `alloc` feature is enabled, additional implementations are provided
for `Box`, `Vec`, and `String`.

For automatic derivation of `JsonDeserialize` and `JsonSerialize`, please see
[`core-json-derive`](https://docs.rs/core-json-derive).
