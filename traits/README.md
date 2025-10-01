# Monero EPEE Traits

Traits for working with objects which may be decoded from an EPEE encoding.

Internally, this uses the [`monero-epee`](https://docs.rs/monero-epee) crate.
That crates itself focuses on being minimal, only offering a dynamically-typed
view as EPEE-encoded data is ingested. This crate additionally adds traits for
decoding into typed objects, requiring `alloc` in order to provide
implementations over `Vec`.

For automatic derivation of `EpeeDecode`, please see
[`monero-epee-derive`](https://docs.rs/monero-epee-derive).
