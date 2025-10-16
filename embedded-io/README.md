# `core-json` `embedded-io`

[`embedded-io`](https://docs.rs/embedded-io) support for
[`core-json`](https://docs.rs/core-json).

### Why?

`core-json` is a `core`-only JSON deserializer. It is abstract to the reader
yet uses its own
[`Read`](https://docs.rs/core-json/latest/core_json/trait.Read.html) trait in
order to maintain zero dependencies. `embedded-io` is the most prominent crate
for performing IO operations without `std`, `alloc`.

This crate offers adapters so implementors of
[`embedded_io::Read`](
  https://docs.rs/embedded-io/latest/embedded_io/trait.Read.html
) can be used with `core-json`.

### Changelog

A changelog may be found
[here](
  https://github.com/core-json/core-json/tree/master/embedded-io/CHANGELOG.md
).
