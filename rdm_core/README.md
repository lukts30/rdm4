# rdm_core

Reads RDM documents into typed records and writes unchanged documents back
byte for byte. No scene model, glTF or conversion.

```rust
let document = rdm_core::Document::open("model.rdm")?;
let models = document.root.models.as_deref().unwrap_or_default();
assert_eq!(document.to_bytes()?, std::fs::read("model.rdm")?);
```

## Design

- `records.rs` declares each record as a plain struct. Fields are in
  on-disk order; `T`, `Vec<T>`, `String` and `Option<_>` fields are
  references to child blocks. `#[derive(Record)]` (`derive/`) generates
  the known-field size, field offsets and separate read and write functions
  from that one declaration. Struct attributes declare accepted strides,
  child block order (`Model` stores groups, vertices, indices) and
  `plain` records decoded with one infallible block loop.
- `read.rs` makes a single pass over the input. A cursor marks the end of
  the last block; every reference must point at the next block's payload.
  That one check enforces the observed depth-first preorder and rejects
  backward references, shared blocks, cycles, gaps, other block orders and
  trailing bytes. Counts, strides and bounds are validated per block.
- `write.rs` appends blocks in the same order and patches offsets.
- `read/vertices.rs`, `read/indices.rs` and their `write/` counterparts
  decode numeric payloads using the model's `VertexFormat` and
  `IndexFormat` records: one typed column per vertex attribute and
  `U16`/`U32` index arrays.
- Byte order is a type parameter of the reader and writer; decoded values
  are native.
- Unknown bytes after each record's known fields, and the stride of blocks
  whose record type has several (`Root` 48/52, `Model` 68/92), are kept in a
  private list in traversal order. The input buffer is not retained.

## Limitations

- Only the observed preorder layout is accepted. Files with shared blocks,
  backward references or a different child order are rejected, not
  rewritten.
- Values may be edited in place and strings or plain arrays such as
  `Frame`s resized. Adding or removing records that carry unknown bytes
  fails at write time, because preserved bytes are matched by position.
- Unknown bytes are kept in the source byte order. Changing
  `Document::endian` rewrites decoded fields only; transcoding is not
  supported.
- Strings must be UTF-8. Vertex semantics must be 0 to 7 and storage types
  3, 5, 6 or 7; unknown codes are rejected.

## Commands

```sh
nix develop --command cargo test -p rdm_core -p rdm_core_derive
nix develop --command cargo run --release -p rdm_core --example roundtrip -- rdm4lib/rdm/benchmark/*.rdm rdm4lib/rdm/*.rdm
nix develop --command cargo run --release -p rdm_core --example bench -- --rounds 5
```

`RUST_LOG=debug` enables log output in the `roundtrip` example. Logging uses
the `log` crate and can be compiled out with its `max_level_off` feature.
