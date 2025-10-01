# Monero EPEE

`epee` is a bespoke serialization format seen within the Monero project without
any official documentation. The best specification is available [here](
  https://github.com/jeffro256/serde_epee/tree/cbebe75475fb2c6073f7b2e058c88ceb2531de17/PORTABLE_STORAGE.md
).

This library implements the `epee` 'portable storage' encoding (itself referred
to as EPEE throughout this library), with the following exceptions:
- We don't support the `Array` type (type 13) as it's unused in practice and
  lacking documentation
- We may accept a _wider_ class of inputs than the `epee` library itself. Our
  definition of compatibility is explicitly if we can decode anything encoded
  by the `epee` library and all encodings we produce may be decoded by the
  `epee` library. We do not expect completeness, so some successfully decoded
  objects may not be able to be encoded, and vice versa.

At this time, we do not support:
- Encoding objects
- Decoding objects into typed data structures. For that, please review the
  `monero-epee-traits` crate.

Instead, we support indexing `epee`-encoded values and decoding individual
fields in a manner comparable to `serde_json::Value` (albeit without
allocating, recursing, or using a proc macro). This is sufficient for basic
needs, much simpler, and should be trivial to verify won't panic/face various
exhaustion attacks compared to more complex implementations.

Because of this, we are also able to support no-`std` and no-`alloc`, without
any dependencies other than `core`, while only consuming approximately one
kibibyte of memory on the stack. We also have no `unsafe` code.

For a more functional library, please check out
[`cuprate-epee-encoding`](
  https://github.com/cuprate/cuprate/tree/9c2c942d2fcf26ed8916dc3f9be6db43d8d2ae78/net/epee-encoding
).
