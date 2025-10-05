# `core-json-traits 0.2.0`

- Updates to `core-json 0.2.0`
- [Support deserializing `[T; N]`, `Vec<T>` at the root-level in` core-json-traits`](https://github.com/kayabaNerve/core-json/commit/2bb8623c889b88fac748cc8fc7b13d7b352c232c)

  This commit renamed `JsonObject::deserialize_object` to
  `JsonStructure::deserialize_structure` due to how it was now inclusive to
  lists.

# `core-json-traits 0.1.2`

- [Support deserializing into `String` on `alloc`](https://github.com/kayabaNerve/core-json/commit/0107bc97c25ddd3e8abe356f693c687750269b2d)

# `core-json-traits 0.1.1`

- [`doc_auto_cfg` -> `doc_cfg`](https://github.com/kayabaNerve/core-json/commit/775367b8b4ad040ed9973af6f504ceb192683f0a)

# `core-json-traits 0.1.0`

- Initial release
