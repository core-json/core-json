# core-json

A non-allocating no-`std` JSON deserializer.

These crates follow the
[RFC 8259](https://datatracker.ietf.org/doc/html/rfc8259) specification of
JSON.

### Goals

- Offer a way to deserialize JSON without performing any allocations
- Don't rely on recursion to ensure the stack cannot overflow
- Never have a reachable panic
- Never use `unsafe`
- Use a minimal amount of memory
- Require zero dependencies
- Additionally offer deserializing into typed structures

### Testing

This library passes [JSON_checker](https://www.json.org/JSON_checker/)'s test
suite, [JSONTestSuite](https://github.com/nst/JSONTestSuite), and is able to
deserialize [JSON-Schema-Test-Suite](
  https://github.com/json-schema-org/JSON-Schema-Test-Suite
). These are the same test suites identified by
[`tinyjson`](https://docs.rs/tinyjson) for its testing.

Additionally, we have a fuzz tester which generates random objects via
[`serde_json`](https://docs.rs/serde_json) before ensuring `core-json` is able
to deserialize an equivalent structure.

### Implementation Details

The deserializer is represented using a stack of the current state. The stack
is parameterized by a constant for the maximum depth allowed for the
deserialized objects, which will be used for a fixed allocation on the stack.
The deserializer's state is approximately two bits per allowed nested object.

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

[`serde-json-core`](https://docs.rs/serde-json-core) is akin to `serde-json`,
still depending on `serde`, yet only requiring `core`. It does not support
dynamic typing with a `serde_json::Value` analogue (as `core-json` does) nor
does it support handling unknown fields within objects (which `core-json` does
to a bounded depth).

If `core-json` does not work for you, please see if `miniserde` works for you.
If `miniserde` does not work for you, then `serde_json` may be justified. The
point of this crate, other than a safe and minimal way to perform
deserialization of JSON objects, is to encourage more light-weight (by
complexity) alternatives to `serde_json`.

### History

There's a bespoke self-describing binary format whose implementations have
historically faced multiple security issues related to memory exhaustion. To
solve this, `monero-epee` was published as a *non-allocating* deserializer. By
always using a fixed amount of memory, it was impossible to increase the amount
of memory consumed. This also inherently meant it worked on `core` and `core`
alone.

Having already implemented such a deserializer once for a self-describing
format, the same design and principles were applied to JSON, bringing us here.
