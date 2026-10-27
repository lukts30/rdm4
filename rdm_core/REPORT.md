# rdm_core MVP report

`rdm_core/` is a new workspace crate. It reads all 17 fixtures in
`rdm4lib/rdm/`, including the big-endian one, and writes each back
byte-identically. All 21 of its tests pass, and the full workspace test suite
passes in release mode with `--locked`, as CI runs it.

## Design

- **Records** (`records.rs`) are plain `#[derive(Facet)]` structs with no DSL.
  The field types are the wire schema:
  - Scalars, `[T; N]` and field-less enums are stored inline.
  - `Box<T>` is an offset to a block with exactly one record; `Vec<T>` is an
    offset to a block of records; `Vec<u8>` is a byte string.
  - `Option<…>` means the offset may be null.
  - The file header (`Preamble`) is a record too, decoded by the same reader
    and compared against the one observed value.
- **Reader** (`read.rs`) is one recursive interpreter over Facet `Shape`s that
  builds values with `Partial`, so it isn't instantiated per record type. Each
  block reports where its subtree ends, and the parent requires its children,
  sorted by offset, to follow it with no gaps. That one check rejects gaps,
  trailing bytes, overlapping or shared blocks, and non-preorder layouts.
  Backward offsets are rejected before recursing, which rules out cycles.
- **Writer** (`write.rs`) walks the values with `Peek`, writes each block, then
  writes its children in the recorded file order and patches their offsets.
- **Private bookkeeping** (`layout.rs`) is a small tree mirroring the
  references. Per block it holds the stride, any unknown trailing bytes (only
  when non-zero), and the child order when it differs from field order. That is
  how the `Model` groups → vertices → indices order is kept. Treating bytes past
  the known fields as unknown also covers the 48/52-byte root and the
  68/92-byte `Model` with no special cases.
- **Vertex and index decoding** (`read_payload.rs` / `write_payload.rs`) is
  typed and kept apart from reference resolution. It uses the format records the
  reader has just read for that model, and checks strides against them.
- **Byte order** is handled only in `byte_order.rs`. Records hold native
  values; packed vertex words such as normals are decoded as `u32`.
- **Generic code** is limited to a few typed helpers: per-scalar `set`/`get`,
  the four storage types in the codecs, and the `Root`/`Preamble` entry points.

## Limitations

- Unknown enum codes (vertex semantic, storage type) are rejected, not
  preserved.
- Non-standard preambles are rejected.
- f16 values are kept as raw bits (`F16(u16)`), because Facet has no `half`
  support.
- There is no editing API. `Document` exposes only `root()` and `into_root()`.
- Facet 0.46.5 is used, the latest stable release; 0.50 is still a release
  candidate.
- The vertex and index codecs find their format by taking the most recent
  vertex and index format read. This relies on `Model` listing the formats
  before the data.

## Tests

- **Round trips:** byte-identical for all 17 fixtures (13 meshes, 4
  animations, 1 big-endian).
- **Independent cross-check:** vertices and indices of all 13 meshes match the
  existing `rdm4lib` in both byte orders. It is a test-only dependency.
- **Malformed input (13 cases):** each asserts the error kind, offset and path.
  Covered: truncation, trailing bytes, bad magic or byte order, a changed
  preamble word, stride too small, wrong record count, backward offset, null
  where required, shared block, unreferenced gap, unknown enum code, vertex
  stride and index stride mismatches.
- **Unknown bytes:** bytes patched into the root, `Model` and mesh-group tails
  survive a round trip.
- **Unit tests:** the `Model` block order is recorded, and a generated 52-byte
  root reads and round-trips. No fixture has one, so the test widens a 48-byte
  root and lets the writer recompute every offset.

Example error:
`` at 0x1df in `models[0].vertex_format.attributes[0].semantic`: enum code: expected one of Position=0, …, found 9 ``.
Logging uses `log` at debug level and is off unless a logger is installed.

## Core size

1,543 lines of code across 11 files in `src/`, counting non-blank lines that
aren't comments, with `#[cfg(test)]` modules removed. There is no macro or
code-generation machinery. Excluded: tests (347) and the two examples,
`rdm_roundtrip` (73) and `rdm_profile` (106).

## Reader timings and allocations

Release build, Apple Silicon, files already in the OS cache, 200 iterations.
Process startup and exit with no files is about 2.2 ms.

| File                           | Size    | I/O    | First read | Median read       | Allocations / reallocations | Retained    |
| ------------------------------ | ------- | ------ | ---------- | ----------------- | --------------------------- | ----------- |
| destruction_stone…lod0 (mesh)  | 1.64 MB | 759 µs | 345 µs     | 236 µs (6.9 GB/s) | 49 / 6                      | 1,640,688 B |
| excavator_tycoons_lod1 (mesh)  | 174 KB  | 281 µs | 149 µs     | 65 µs             | 141 / 13                    | 177,876 B   |
| jellyfish_idle01 (6622 frames) | 213 KB  | 275 µs | 1.5 ms     | 1.5 ms (145 MB/s) | 622 / 76                    | 219,611 B   |
| fishery_others_cutout_lod0     | 1.2 KB  | 167 µs | 27 µs      | 19 µs             | 88 / 8                      | 3,588 B     |

"Retained" is what the document still holds after the input buffer is freed,
measured with a counting allocator in the profiling example; it is roughly the
file size. Each read also allocates about 2× the file size in short-lived
`Partial` state.

**Profiling:** animation reads were 4.3 ms, with about 70% of the time inside
Facet's `Partial` per-field bookkeeping. The per-field type lookup was about
5%, so caching it wasn't worth it. Setting `[f32; 3]` and `[f32; 4]` arrays in
one step cut that to 1.5 ms (2.9×). The remaining cost per frame is `Partial`
overhead that only unsafe or record-specific code would remove.

## Known issue outside this crate

`cargo clippy --workspace --tests -- -D warnings` fails on macOS, but only in
`rdm4lib`: its test modules import `std::fs` only for Linux-gated tests, so the
import is unused there. `rdm_core` itself is clippy-clean.

## Running

```sh
nix develop --command cargo run -p rdm_core --example rdm_roundtrip -- rdm4lib/rdm/*.rdm
nix develop --command cargo run --release -p rdm_core --example rdm_profile -- --iterations 200 rdm4lib/rdm/*.rdm
```
