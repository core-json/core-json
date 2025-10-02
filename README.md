# core-json

A no-`std` no-`alloc` JSON deserializer.

### Goals

- Offer a way to deserialize JSON without performing any allocations
- Never rely on recursion to ensure the stack cannot overflow
- Never panic, avoiding any/all `unsafe`
- Use a minimal amount of memory
- Require zero dependencies
- Additionally offer deserializing into typed structures, when allocations are
  allowed (adding a dependency on `hashbrown` when `alloc` but not `std`)

### Non-Goals

- Assert the deserialized JSON is valid. While some checks are performed for
  the sanity of deserialization, this is not intended to reject inputs other
  deserializers would likely reject

### Implementation Details

The deserializer is represented using a stack of the current state. The stack
is parameterized by a constant for the maximum depth allowed for the
deserialized objects, which will be used for a fixed allocation on the stack.
The deserializer's state is approximately one byte per allowed nested object.

Optionally, the caller may specify a stack which does dynamically allocate and
supports an unbounded depth accordingly.

### Drawbacks

The deserializer is premised on a single-pass algorithm. Fields are yielded in the
order they were serialized, and one cannot advance to a specific field without
skipping past all fields prior to it. To access a prior field requires
deserializing the entire object again.

Additionally, the deserializer state has a mutable borrow of it taken while
deserializing, which can make it a bit annoying to directly work with. The
[`core-json-derive`](https://docs.rs/core-json-derive) crate offers automatic
derivation of deserializing into typed objects however.

Due to being no-`std`, we are unable to use `std::io::Read` and instead define
our trait, `BytesLike`. While we implement this for `&[u8]`, it should be
possible to implement for `bytes::Buf` (and similar constructions) without
issue.

### Comparisons to Other Crates

[`serde_json`](https://docs.rs/serde_json) is the de-facto standard for working
with JSON in Rust. Its author, dtolnay, has spent an extensive amount of time
on optimizing its compilation however due to its weight. The most recent
improvement was with the introduction of
[`serde_core`](https://docs.rs/serde_core) which allows compiling more of
[`serde`](https://docs.rs/serde) in parallel.

[`miniserde`](https://docs.rs/miniserde) is dtolnay's JSON-only alternative to
`serde`. It's a much more minimal alternative and likely _should_ be preferred
to `serde_json` by everyone who doesn't explicitly need `serde_json`.
`miniserde` does depend on `alloc` however.

[`tinyjson`](https://docs.rs/tinyjson) has no dependencies, yet requires `std`
(for `std::collections::HashMap`) and doesn't support deserializing into typed
structures. It also will allocate as it deserializes.

### History

There's a bespoke self-describing binary format whose implementations have
historically faced multiple security issues related to memory exhaustion. To
solve this, `monero-epee` was published as a *non-allocating* deserializer. By
always using a fixed amount of memory, it was impossible to increase the amount
of memory consumed. This also inherently meant it worked on `core` and `core`
alone.

Having already implemented such a deserializer once for a self-describing
format, the same design and principles were applied to JSON, bringing us here.
