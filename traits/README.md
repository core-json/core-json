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

For automatic derivation of `JsonDeserialize` and `JsonSerialize`, please see
[`core-json-derive`](https://docs.rs/core-json-derive).

### `alloc` Feature

The `alloc` feature enables implementations for `Box`, `Vec`, and `String`.

### `zmij` Feature

The optional `zmij` features enables serializing `f64`s via
[`zmij`](https://docs.rs/zmij). When the `zmij` feature is not enabled, the
implementation present in `core` is used, truncating to the first `f64::DIGITS`
significant digits.

[`ryu` is faster than `core`](https://github.com/rust-lang/rust/issues/52811)
and [`zmij` is faster than `ryu`](
  https://raw.githubusercontent.com/dtolnay/zmij/15be7513551ae46074acb2c9a6da030a36d76653/performance.png
) however, so it SHOULD be enabled for trees which already have `zmij` as a
dependency. The `zmij` feature SHOULD NOT be enabled by libraries which depend
on `core-json-traits` (solely the final consumer).

### Changelog

A changelog may be found
[here](https://github.com/core-json/core-json/tree/master/traits/CHANGELOG.md).
