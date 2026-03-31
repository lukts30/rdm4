# AGENTS.md

Guidance for AI coding agents operating in this repository.

## Project Overview

**rdm4** is a Rust CLI tool that converts 3D model files between the
proprietary RDM format (Anno 1800) and glTF 2.0. It is a Cargo workspace
with four crates:

| Crate        | Path          | Kind                          |
| ------------ | ------------- | ----------------------------- |
| `rdm4-bin`   | `src/main.rs` | Binary (CLI entry point)      |
| `rdm4lib`    | `rdm4lib/`    | Library (core RDM/glTF logic) |
| `rdm_derive` | `rdm_derive/` | Proc-macro (`RdmStructSize`)  |
| `cfghelper`  | `cfghelper/`  | Library (XML cfg parser)      |

## Repository Layout

Dependency updates typically touch three `Cargo.toml` files:

- `Cargo.toml` (workspace root / CLI package)
- `rdm4lib/Cargo.toml` (core conversion logic)
- `cfghelper/Cargo.toml` (XML cfg parser)

## External Instructions

- `AGENTS.md` references external files; load them with the Read tool when relevant.
- Do not preemptively load all references; use on-demand loading.
- Treat loaded instructions as mandatory and higher priority than defaults.
- CRITICAL: Before any git commit, read `docs/CONTRIBUTING.md` and follow the commit message rules.

## Build / Lint / Test Commands

```sh
# Build
cargo build --workspace
cargo build --release --workspace          # as CI runs it

# Lint
cargo fmt -- --check                       # check formatting
cargo fmt                                  # auto-fix formatting
cargo clippy --tests -- -D warnings        # clippy (CI mode)

# Test — all
cargo test --workspace                     # debug, all crates
cargo test --release --workspace           # release, all crates (as CI)

# Test — single test by name
cargo test -p rdm4lib -- tests::fishery_others_lod2
cargo test -p cfghelper -- cfg_xml_tests::test_parse_cfg

# Test — single crate
cargo test -p rdm4lib
cargo test -p cfghelper

# Test — integration tests only
cargo test -p rdm4lib --test integration_test

# Test — unit tests only
cargo test -p rdm4lib --lib
```

CI runs tests in **release mode** with `--locked`. Some integration tests
require `gltf_validator` on PATH and (on Windows) `texconv.exe`.

## Formatting and Linting

All formatting and linting is configured in `flake.nix` (single source of
truth). The Nix dev shell (`nix develop` / `direnv allow`) installs
pre-commit hooks that run **clippy** and **treefmt** automatically.

Formatters managed by treefmt: rustfmt (edition 2021), nixfmt, yamlfmt,
actionlint, mdformat. Do not add standalone config files (`rustfmt.toml`,
`clippy.toml`, etc.) — configure via `flake.nix` instead.

Run manually: `treefmt` or `cargo fmt && cargo clippy`.

## Code Style

### Imports

Imports are loosely grouped. No strict ordering is enforced beyond `rustfmt`.
Common patterns:

- `extern crate` is used for `rdm4lib` and `log` (legacy, do not add new ones)
- Glob imports appear sparingly (`use nalgebra::*;`)
- Multi-item braces are preferred: `use crate::{foo, bar};`

### Error Handling

The codebase uses **`unwrap()` and `assert!`** as the primary error handling
strategy. `anyhow` / `thiserror` are not used. The `?` operator appears in
only a few functions. When adding new code:

- Prefer `unwrap()` or `expect("descriptive message")` for internal invariants
- Use `?` with `Result<T, Box<dyn Error>>` when writing functions that
  naturally propagate errors (e.g., file I/O entry points)
- Use `assert!` / `assert_eq!` for precondition checks, even outside tests

### Naming

- **Modules**: `snake_case`, domain modules prefixed with `rdm_` or `gltf_`
- **Structs**: `PascalCase` with `Rd` prefix for domain types (`RdModell`,
  `RdJoint`) and `Rdm` prefix for binary format types (`RdmFile`,
  `RdmContainer`)
- **Enums**: `PascalCase` variants; vertex format enums use
  `#[allow(non_camel_case_types)]` to match the RDM spec naming
  (e.g., `P4h_N4b_G4b_B4b_T2h`)
- **Functions**: `snake_case`; short domain helpers like `p4h()`, `n4b()`
- **Constants**: `UPPER_SNAKE_CASE`
- **cfghelper structs** use `#[allow(non_snake_case)]` to match XML field names

### Types and Generics

- Heavy use of **const generics** for vertex format abstractions
  (`AnnoData<DataType, IDENTIFIER, DATA_SIZE>`)
- Type aliases for binary containers: `RdmString`, `RdmTypedContainer<T>`,
  `AnnoPtr<T>`
- `From<P: AsRef<Path>>` implemented on `RdModell` and `RdAnim` for
  constructing from file paths

### Unsafe Code

Unsafe is limited to `align_to` calls for zero-copy byte reinterpretation of
`#[repr(C)]` structs. Do not introduce new categories of unsafe code. All
structs reinterpreted via `align_to` must be `#[repr(C)]`.

### Derives

Common derive stacking pattern on binary format structs:

```rust
#[repr(C)]
#[derive(Clone, Debug)]
#[binrw]
#[bw(import_raw(end: &mut u64))]
#[derive(RdmStructSize)]
pub struct SomeStruct { ... }
```

`RdmStructSize` is a custom proc-macro from `rdm_derive` used on nearly all
binary format structs. Other common derives: `Debug`, `Clone`, `Copy`,
`PartialEq`, `Eq`.

### Module Structure

File-per-module style (no `mod.rs`). Modules declared flat in `lib.rs`.
`cfghelper` wraps everything in a `pub mod cfghelper { }` block.

### Documentation

Minimal. Doc comments (`///`) exist mainly on CLI args (`clap` fields). Inline
comments are sparse. When adding code, at minimum document public API items.

### Testing

- Unit tests: `#[cfg(test)] mod tests { }` inside source files
- Integration tests: `rdm4lib/tests/` and `cfghelper/tests/`
- Tests are data-driven, verifying vertex counts, format strings, and SHA-256
  hashes of output files via `check_hash()`
- Many tests are gated with `#[cfg_attr(miri, ignore)]` (file I/O)
- Platform-specific tests use `#[cfg(target_os = "linux")]` /
  `#[cfg(target_os = "windows")]`

### Dependencies

`binrw = "=0.11.2"` is pinned to an exact version because the code relies
on specific API details. Do not bump it without verifying compatibility.
Key crates: `binrw`, `nalgebra`, `gltf`, `half`, `clap`, `quick-xml`.

## Commit Messages

Follow Conventional Commits: `<type>(<scope>): <description>`

- **Types**: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `build`,
  `ci`, `perf`, `chore`
- **Scopes**: `lib`, `cli`, `ci`, `nix`, `docs`
- Summary: imperative mood, no capitalization, no trailing period

See [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for full details.
