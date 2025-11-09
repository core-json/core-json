# `core-json-traits 0.4.1`

- [Add support for (de)serializing `BTreeMap<String, _>` and `HashMap<String, _>`](https://github.com/core-json/core-json/commit/55d8e397697b3c36359cd4a73f7131132df925a0)

# `core-json-traits 0.4.0`

- Updates to `core-json 0.4.0`
- [Add support for `()` as `null`](https://github.com/core-json/core-json/commit/3a49de5f80f800349b66124877d05e3bbce3ba21)

# `core-json-traits 0.2.1`

- [Update repository URLs](https://github.com/core-json/core-json/commit/da434e18e426fb5bd3abf7dffa1462011770379e)
- [Add `JsonSerialize`](https://github.com/core-json/core-json/commit/e726edc5a23c086be9f15ced9c76507e44708401)
- [Add an optional feature for `ryu`](https://github.com/core-json/core-json/commit/7ddf5caf32b1cc599cfb51c3db77c0b454a0e43c)

# `core-json-traits 0.2.0`

- Updates to `core-json 0.2.0`
- [Support deserializing `[T; N]`, `Vec<T>` at the root-level in` core-json-traits`](https://github.com/core-json/core-json/commit/2bb8623c889b88fac748cc8fc7b13d7b352c232c)

  This commit renamed `JsonObject::deserialize_object` to
  `JsonStructure::deserialize_structure` due to how it was now inclusive to
  lists.

# `core-json-traits 0.1.2`

- [Support deserializing into `String` on `alloc`](https://github.com/core-json/core-json/commit/0107bc97c25ddd3e8abe356f693c687750269b2d)

# `core-json-traits 0.1.1`

- [`doc_auto_cfg` -> `doc_cfg`](https://github.com/core-json/core-json/commit/775367b8b4ad040ed9973af6f504ceb192683f0a)

# `core-json-traits 0.1.0`

- Initial release
