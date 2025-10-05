# `core-json` `embedded-io`

[`embedded-io`](https://docs.rs/embedded-io) support for
[`core-json`](https://docs.rs/core-json).

### Why?

`core-json` is a `core`-only JSON deserializer. It is abstract to the reader
yet uses its own
[`BytesLike`](https://docs.rs/core-json/latest/core_json/trait.BytesLike.html)
trait in order to maintain zero dependencies. `embedded-io` is the prominent
crate for performing IO operations without `std`, `alloc`.

This crate offers adapters so implementors of
[`embedded_io::Read`](
  https://docs.rs/embedded-io/latest/embedded_io/trait.Read.html
) or [`embedded_io::Seek`](
  https://docs.rs/embedded-io/latest/embedded_io/trait.Seek.html
) can be used with `core-json`.

Note `core-json` effectively requires `BytesLike` be efficient to fork (as
bytes read from a `BytesLike` are themselves returned as a `BytesLike`, letting
the implementor delegate the ownership of the underlying bytes within memory).
This means the library bounds `Clone + Read` and not `Read` for its
`ReadAdapter`. The `SeekAdapter` also bounds `Clone + Seek`, not `Seek`, yet
`ClonableSeek` is provided as a wrapper to efficiently turn any `S: Seek` into
`ClonableSeek<S>: Clone + Seek` (at the cost of one extra seek operation per
each read).
