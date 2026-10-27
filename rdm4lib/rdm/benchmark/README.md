# RDM reader benchmark fixtures

The exact 26 unmodified RDM files used in the reader comparisons: 22 files
from the Anno 1800 corpus extracted on 2026-10-21, plus four existing
`rdm4lib/rdm` fixtures. Total input size: 27,378,902 bytes.

Numeric filename prefixes preserve the benchmark IDs. `manifest.json`
records each file's size, SHA-256, classification and source path;
`groups.json` defines the full suite and the animation/non-animation splits.
The six animation files are IDs 2, 9, 16, 17, 22 and 24. The other 20 include
two files with neither mesh nor animation data.

IDs 6, 11, 15 and 20 contain multiple models. Readers that only support one
model cannot be compared on the full suite; report their compatibility
failures and use the same supported subset for timing comparisons.

The four repository fixtures are duplicated here so this directory is a
self-contained benchmark input set. Their bytes are unchanged.

Verify all files from this directory:

```sh
sha256sum --check SHA256SUMS
```
