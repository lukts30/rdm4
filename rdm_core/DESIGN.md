# rdm_core: summary and design

## Summary

`rdm_core` reads RDM documents into fully decoded, typed records and writes
unchanged documents back byte for byte. It has no scene model, glTF support
or conversion pipeline.

- All 26 benchmark fixtures (`rdm4lib/rdm/benchmark`, from commit
  `56b210cd`) and all 17 repository fixtures round-trip byte-identically,
  including the big-endian `n_nature_small_stone_02_lod0.rdm`. No fixture
  is unsupported, including the multi-model files 6, 11, 15 and 20.
- Core size: 1,703 non-blank, non-comment lines, including the derive
  macro (311). Attributes and brace-only lines count. Tests, examples and
  benchmark tooling are excluded.
- Dependencies: `half`, `log` and the local `rdm_core_derive` (`syn`,
  `quote`, `proc-macro2`). Everything was already in `Cargo.lock`.

## Evidence

Record layouts follow `rdm4lib/src/rdm_data_main.rs` and
`rdm_data_anim.rs`; framing follows `docs/RDM_BINARY_LAYOUT.md`. Before
writing Rust, a prototype walker tested the reading rule below against all
43 files and found:

- Every non-null reference points exactly at the block after the last one
  read: a depth-first preorder, with `Model`'s children stored groups →
  vertices → indices.
- No gaps, trailing bytes, sharing, non-UTF-8 strings or out-of-range
  indices.
- Every record is a known-field prefix plus an unknown tail. Only `Root`
  (48 or 52 bytes) and `Model` (68 or 92 bytes) have more than one stride.
- Non-zero tails occur only in `VertexFormat`, `Material` and `Joint`.

Fields with unestablished meaning are left in the tail rather than named,
for example the byte that `rdm4lib` calls `unknown_shader_id`.

## Design

```text
records.rs       plain structs + #[derive(Record)] attributes
derive/          generates layout constants, read and write functions
wire.rs          header constants, byte order, scalar codecs
read.rs          single-pass reader: framing, references, validation
read/vertices.rs, read/indices.rs     format-driven numeric decoding
write.rs         writer: block placement and offset patching
write/vertices.rs, write/indices.rs   numeric encoding
document.rs      Document and private preservation data
error.rs         offset + record.field + expected/actual
```

### Records

Records are ordinary structs. Field declaration order is the on-disk order,
starting at the record's first byte. The field type decides the encoding:

| Field type                                 | On disk                                     |
| ------------------------------------------ | ------------------------------------------- |
| `u8`, `u32`, `i32`, `f32`, `[T; N]`, enums | inline scalar                               |
| `T` (a record)                             | reference to a block with exactly one `T`   |
| `Vec<T>`                                   | reference to a block of `T` records         |
| `String`                                   | reference to a byte block (stride 1, UTF-8) |
| `Option<_>`                                | nullable reference                          |
| `Vertices`, `Indices`                      | reference to a numeric block                |

Struct attributes:

- `strides(...)`: accepted record widths, checked per block.
- `block_order(groups, vertices, indices)`: physical child order of the
  listed fields, independent of field order. Fields not listed keep
  declaration order.
- `plain`: every field is a scalar, so a block of these records is decoded
  in one infallible loop. The generated code calls `Scalar::decode` for
  every field, so a non-scalar field fails to compile.

Field attribute `args(a, b)` passes already-decoded sibling fields to a
payload decoder; `Model.vertices` receives `vertex_format`, and
`Model.indices` receives `index_format` and `vertices`.

The derive emits a separate `ReadRecord` and `WriteRecord` implementation
from one declaration, so there is no handwritten second schema and no code
that branches on direction.

### Reading strategy

The reader decodes an in-memory slice in one recursive pass. A cursor marks
the end of the last block read. Every reference must point at
`cursor + 8`, the payload of the block that starts at the cursor. That one
check enforces the observed preorder layout and rejects:

- backward references, shared blocks and cycles (target below the cursor),
- gaps and unsupported block orders (target above the cursor),
- trailing bytes (cursor short of end of file after the root).

Each block also gets checked arithmetic for `count * stride`, bounds,
accepted strides and the exact-one-record rule for single references.
Vertex blocks must have the stride implied by the vertex format; index
blocks need `U16` or `U32` storage, the matching stride and every index
below the vertex count.

Byte order is a zero-sized type parameter (`LittleEndian`, `BigEndian`),
chosen once per document, so decoding loops never branch on it. Decoded
values are native; byte order exists only in `wire.rs` codecs.

Errors carry the hexadecimal offset, the `Record.field` context and
expected versus actual values:

```text
Model.groups at 0xfc: expected reference to the next block at 0x1d4,
found 0x210 (gap or unsupported block order)
```

### Decoded payloads

- `Vertices`: one typed column per vertex attribute (`U16`, `U32`, `F16`,
  `F32`), in vertex order. Packed attributes such as normals are `U32`
  words decoded in the file's byte order.
- `Indices`: `U16` or `U32`, as declared by `IndexFormat`.

### Preservation

`Document` holds the decoded `Root`, the byte order and a private
`Preserved` value:

- `tails`: the unknown bytes after each record's known fields,
- `strides`: one entry per block of a record type with several strides.

Both are recorded in traversal order; the writer consumes them in the same
order. Tails are captured once per block, not once per record. The input
buffer is not retained; `Document::open` drops it before returning.

Values may be edited in place, and strings or plain arrays such as `Frame`s
resized. Adding or removing records that carry tails fails at write time,
because preserved bytes are matched by position.

### Writer

The writer appends each block with zeroed reference slots, writes each
child block when its field is written and patches the slot with the child's
payload offset. Output is therefore in the same preorder the reader
accepts. Writing in the other byte order converts decoded fields only;
unknown tails stay in source order.

## Performance work

All changes were measured first; the rejected ones were reverted.

| Change                                                                                            | Effect                                             |
| ------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| Tail capture moved from per record to per block, in reader and writer                             | frames 3.53 → 2.09 ns                              |
| `plain` block decoding (`chunks_exact(stride).map(decode).collect()`) for `Frame` and `MeshGroup` | frames 2.09 → 1.02 ns; animation group 367 → 95 µs |
| Fixed-size `[T; N]` vertex columns for 1–4 components, flattened with `into_flattened`            | large meshes ~5.5 → ~8.5 GB/s                      |
| Boxed `Error` (smaller `Result`)                                                                  | no effect, reverted                                |
| `#[inline]` on generated `read`                                                                   | no effect, reverted                                |
| `extend` per vertex                                                                               | ~2× slower, reverted                               |

The disassembly before the `plain` change showed about 30 instructions per
frame: an overflow-checked offset, a bounds check reloading the input
length, `Vec::push` capacity bookkeeping and stack spills around the
possible `grow_one` call. An ideal standalone loop took 0.6 ns per frame.

## Measurements

Apple M4, macOS, rustc 1.98.1, release with `overflow-checks = true`.

### Timing

In-process medians (at least 15 runs per file), warm page cache. Process
startup (spawn and exit) was measured separately at a median of 2.2 ms.

| Group                    |      Bytes | I/O (`fs::read`) |          Parse | Parse throughput |
| ------------------------ | ---------: | ---------------: | -------------: | ---------------: |
| Animation (6 files)      |  3,286,789 |           131 µs |       94–95 µs |       ~34.7 GB/s |
| Non-animation (20 files) | 24,092,113 |       794–826 µs | 3,027–3,060 µs |        ~7.9 GB/s |

Gist sweep protocol (input preloaded, parse and drop, 30 shuffled warm-ups,
median of 100 sweeps, 5 processes):

| Group        | Process medians |
| ------------ | --------------- |
| All 26       | 3.22–4.15 ms    |
| Animation    | 0.097–0.099 ms  |
| No animation | 3.08–3.13 ms    |

Instructions per sweep from `/usr/bin/time -l` (200 sweeps minus 0 sweeps):
animation 1.93 M, no animation 85.8 M, all 87.7 M. Instruction counts on
ARM64 are only indicative against x86-64 counts.

### Allocations

Counted by a global allocator in the example binaries only. Allocs are
`alloc` calls, reallocs are `realloc` calls, allocated bytes are requested
sizes plus realloc growth, retained values are live requested heap storage
while the document exists. Allocator rounding and metadata, peak RSS and
hardware counters are unavailable. Counts were identical across all
repetitions, and dropping a document returns live counts to zero.

Measured through `Document::open`, so file loading is included:

| Files     | Allocs | Reallocs | Allocated bytes | Retained count | Retained bytes |
| --------- | -----: | -------: | --------------: | -------------: | -------------: |
| Common 22 |  1,117 |      123 |      40,568,794 |          1,095 |     20,285,777 |
| All 26    |  3,982 |      156 |      54,848,092 |          3,956 |     27,440,598 |

The common 22 inputs total 20,274,717 bytes, so retained data is the input
size plus 0.05%. Animation needs about two allocations per track (joint
name and frame array).

## Comparison with earlier readers

Gist `lukts30/f5a58a41baeaf9bbc0e62399796a4891` measured four earlier
readers on a Ryzen 5800X under Linux. Its allocation counts are
allocator-independent, so they compare directly. Timings and cycles do not.

| Common 22                 | Allocs | Reallocs | Allocated bytes | Retained count | Retained bytes |
| ------------------------- | -----: | -------: | --------------: | -------------: | -------------: |
| New Facet/Partial         | 11,053 |      941 |      44,375,790 |          1,294 |     20,416,317 |
| Core macros               |  1,726 |      123 |      40,598,954 |          1,075 |     20,283,075 |
| Previous derive-generated |  1,052 |       72 |      40,852,488 |          1,030 |     20,577,771 |
| Previous Facet/Poke       |  1,048 |       69 |      40,924,608 |          1,004 |     20,596,899 |
| **rdm_core**              |  1,117 |      123 |  **40,568,794** |          1,095 |     20,285,777 |

- Allocated bytes are the lowest of the five; retained bytes tie Core for
  lowest and are 1.4% below derive-generated and Poke.
- Allocation calls are 6.6% above the best. Most files need one or two
  more than derive-generated: the separate `strides` list is always
  allocated because `Root` has two strides.
- File 15 (258 models) needs 2,588 allocations against 1,555, about four
  more per model, because vertices are one column per attribute instead of
  two shared buffers.
- Reallocations (5–12 per file) come from the growing tail list and the
  final `shrink_to_fit` calls.
- Animation instructions match the derive-generated reader (1.93 M vs
  1.88 M). Non-animation uses about 38% fewer (85.8 M vs 138.0 M), across
  different instruction sets.
- Code size: 1,703 lines against 1,325 (derive-generated) and 1,847
  (Facet/Poke).

Possible follow-ups: store strides in the tail buffer (one less allocation
and one or two fewer reallocations per file) and reconsider
`shrink_to_fit` (fewer reallocations, some spare capacity retained).

## Tests

20 tests, all passing in debug and release:

- `fixtures.rs`: hashes from `SHA256SUMS`, round trips of all 26 benchmark
  and 17 repository fixtures, every `manifest.json` group, vertex columns
  and group ranges of all 258 models in file 15.
- `byte_order.rs`: big-endian values checked against bytes read directly
  from the file, and a little → big → little-endian round trip of decoded
  values.
- `malformed.rs`: 23 mutations, each asserting offset, context, expected
  and actual values. Covers headers, truncation, trailing bytes, overflow,
  backward, shared, null and reordered references, strides, counts,
  formats, index range and UTF-8.
- `preservation.rs`: mutated unknown bytes, in-place edits, resized strings
  and frame arrays, and rejected structural edits.
- Unit test: known-field sizes of every record.

## Reproduction

```sh
nix develop --command cargo test --release -p rdm_core -p rdm_core_derive
nix develop --command cargo run --release -p rdm_core --example roundtrip -- rdm4lib/rdm/benchmark/*.rdm rdm4lib/rdm/*.rdm
nix develop --command cargo run --release -p rdm_core --example bench -- --rounds 5
nix develop --command cargo run --release -p rdm_core --example alloc_open
nix develop --command cargo build --release -p rdm_core --example sweep
for g in all animation no-animation; do target/release/examples/sweep $g; done
/usr/bin/time -l target/release/examples/sweep animation 200   # macOS
```
