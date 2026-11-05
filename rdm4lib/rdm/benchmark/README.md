# RDM corpus

26 unchanged benchmark files from `56b210c`: 22 Anno 1800 files and four
existing fixtures, totaling 27,378,902 bytes. Combined with the top-level
fixtures, there are 39 distinct files across 43 entries.

`manifest.json` records provenance, sizes and hashes; `groups.json` defines
benchmark subsets. `expectations.json` records counts, strides, formats,
material ranges (`[first_index, index_count, material]`) and animation
properties inspected in the source bytes.

Legacy limitations:

- IDs 0 and 1 contain neither mesh nor animation; both kind APIs reject them.
- IDs 6, 11, 15 and 20 contain 2, 2, 258 and 20 models respectively; only
  the first survives conversion.

Four ignored tests require complete multi-model geometry. They currently
fail; enable them and remove the truncation assertions during migration.
The planned new CLI is `rdm4ng`; the legacy entry point remains `rdm4-bin`.

```sh
# From this directory:
sha256sum --check SHA256SUMS
# From the repository root:
nix develop --command cargo test --release --locked -p rdm4lib --test corpus
# Known failures:
nix develop --command cargo test --release --locked -p rdm4lib --test corpus -- --ignored
```
