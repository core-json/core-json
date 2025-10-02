# `core-json` Traits

Traits for working with objects which may be deserialized from JSON.

Internally, this uses the [`core-json`](https://docs.rs/core-json) crate. That
crate focuses on being minimal, only offering a dynamically-typed view as
JSON-serialized data is processed. This crate additionally adds traits for
deserializing into typed objects, with an optional `alloc` feature in order to
provide implementations over `Vec`.

For automatic derivation of `JsonDeserialize`, please see
[`core-json-derive`](https://docs.rs/core-json-derive).
