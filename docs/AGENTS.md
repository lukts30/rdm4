# AGENTS.md

Guidance for coding agents working on rdm4.

Before committing, read [CONTRIBUTING.md](CONTRIBUTING.md) and follow its
Conventional Commit rules. Load other referenced files when relevant.

## Release scope

Finish the converter migration, preserve existing CLI and Blender workflows,
and fix edge cases in supported conversions. Keep commits small and docs short.
Broader layouts, material features and shading convention changes are follow-ups.

## Workspace

| Crate        | Paths                              | Role                            |
| ------------ | ---------------------------------- | ------------------------------- |
| `rdm4-bin`   | `src/main.rs`, `src/bin/rdm4ng.rs` | Legacy CLI and subcommand CLI   |
| `rdm4lib`    | `rdm4lib/src/`                     | RDM codecs and scene conversion |
| `rdm_derive` | `rdm_derive/src/`                  | `RdmRecord` derive              |

The root package and both path dependencies are workspace members.
Dependency changes may touch all three Cargo manifests.

## Tooling

Run Rust tooling through the default Nix development shell:

```sh
nix develop --command cargo build --release --locked --workspace
nix develop --command cargo test --release --locked --workspace
nix develop --command cargo test --release --locked -p rdm4lib --test integration_test
nix develop --command cargo test --release --locked -p rdm4-bin --test blender_contract
nix develop --command cargo clippy --workspace --tests --locked -- -D warnings
nix develop --command cargo fmt -- --check
nix develop --command treefmt
```

Do not use host-installed Rust tools. Formatting and hook configuration live in
`flake.nix`; do not add separate formatter or Clippy configuration files.
Pre-commit hooks run Clippy and treefmt. CI tests Linux and Windows in release
mode with `--locked`. The Nix shell includes `gltf_validator`; DDS tests need
Windows and `texconv.exe`.

## Architecture

Both CLIs call `src/commands.rs`:

```text
RDM input -> RdmReader -> RdmDocument -> Scene -> GltfWriter
 glTF input -> ImportedGltf -> Scene -> RdmDocument::from_scene -> RdmWriter
```

- `rdm/scanner.rs`: sequential framing scan and physical block index. Inspection
  can report unknown records and retain partial results.
- `rdm/read.rs`: typed decoding over `Read + Seek`, using the block index and
  following references in field order. Complete reads require known block owners.
  CLI inputs use `BufReader<File>`; the reader does not require a whole-file copy.
- `rdm/records/mesh.rs` and `animation.rs`: plain owned records, independent of
  byte order and source placement. Unknown tails live in `RdmSourceLayout`.
- `rdm/write.rs`: fresh serialization and `write_preserving`, which retains
  original placement and tails when shapes and byte order stay unchanged.
- `scene.rs`: shared meshes, typed attribute columns, skeletons and animation.
- `rdm/packing.rs` and `convert.rs`: target presets, coordinate conversion and
  document/scene adapters. No template-based writing remains.
- `gltf_buffers.rs`: payload alignment, views, accessors and final merging.
  `gltf_export/` builds scene objects; `gltf_export.rs` packages files.

`rdm4ng` imports active-scene mesh instances and exports all RDM models by default.
The old CLI keeps first-model defaults and explicit compatibility settings.
Selecting a model in `rdm4ng` does not enable legacy export behavior.

## Records and code style

Keep plain annotated structs and the GAT `RdmReadField::Args<'a>` API:

```rust
#[derive(Debug, Clone, PartialEq, RdmRecord)]
#[rdm(strides(92, 68), write)]
pub struct RdmModel {
    pub vertex_format: RdmVertexFormat,
    #[rdm(args(vertex_format))]
    pub vertices: RdmVertices,
    // Remaining fields follow their wire order.
}
```

The derive uses explicit wire sizes and supported strides. Do not replace
structs with a generation DSL or reintroduce `binrw` or `RdmStructSize`.
Use file-per-module layout without `mod.rs`. Keep records in the mesh and
animation modules rather than adding one file per record.

Document public APIs. Use `RdmError` for codec errors and propagate conversion
and I/O errors with `Result`; reserve assertions and `expect` for internal
invariants. Keep dependencies small and reuse the existing glTF JSON types and
GLB serializer. Hashing dependencies belong under dev-dependencies.
Do not introduce unsafe code or implicit float/quaternion normalization.
Preserve untouched scalar bits where no conversion is needed.

## Tests and compatibility

Fixtures live in `rdm4lib/rdm/`; benchmark expectations are checked by
`rdm4lib/tests/corpus.rs`. Extend existing checks where practical instead of
adding large test scaffolding. Conversion hashes check compatibility; preserving
rewrites compare original bytes. See [RDM_ROUNDTRIP.md](RDM_ROUNDTRIP.md) and
[RDM_VERTEX_FORMATS.md](RDM_VERTEX_FORMATS.md) for known fidelity limits.

`tests/blender_contract.rs` checks the add-on's command arguments, filenames,
transforms, materials, skinning and animation times. It does not run Blender.
See [the test notes](../tests/README.md) for the pinned add-on revision.

- Existing six output presets remain supported. `rdm4ng` rejects active weights
  beyond the first joint/weight set; the legacy CLI retains truncation and warns.
- Joint names must be unique by default; `UnstableIndex` handles duplicates or
  missing names.
- Animation import requires keys at t=0 and compatible sampled channels by
  default. `--interpolate-animations` explicitly enables baking.
- Animated imports generally need `--no-transform`; preserve existing coordinate
  and shading behavior unless the task explicitly changes it.
