# Vertex format names and round trips

**Format strings do not uniquely identify raw RDM descriptors.** Each descriptor
stores `(semantic, storage type, interpretation, storage count)`. Display names
such as `C4b` omit distinctions between interpretation codes.

Static inspection confirms the same packed naming rule in both games:

| Interpretation (storage type 5) | Anno 2070 8051 | Anno 1800 |
| ------------------------------- | -------------- | --------- |
| 2                               | `4b`           | `4b`      |
| 3, 4, 5                         | `4c`           | `4c`      |
| 6                               | `4b`           | `4b`      |

Native branches: 2070 VA `0x140914dbf` (EXE SHA-256 prefix
`e6c21fb8541a5e67`); installed 1800 VA `0x1442861e2` (prefix
`b02b7b98761d6138`).

Original configurations agree:

- **2070:** `jungletree_big_03.cfg` declares `…_C4c` for
  `jungletree_big_stem.rdm`, whose color descriptor is `(5,5,4,1)`.
  Both are from `Data2.rda`.
- **1800:** `tree_jungle_palm_01_deco.cfg` references
  `tree_jungle_palm_01_02.prp`, declaring `…_C4b_C4b`. Its
  `palm_tree_01_03_lod0.rdm` has `(5,5,2,1)` then `(5,5,6,1)`, matching
  our plant preset. CFG/PRP: `data8.rda`; checked RDM: `data21.rda`.
- **1800 cloth:** `clothline.cfg` declares `P3f_N3b,T2f`, but the referenced
  RDM stores `P3f_N3f_G3f_B3f_T2f_C4c`.

## Future round-trip goals

- Keep raw descriptors, attribute order and color-layer count through
  `RDM → Scene → RDM`. Use display strings with known presets to select layouts;
  retain original interpretation codes when available.
- Support `plant/cloth RDM → GLB → Blender edit → RDM`, preserving untouched
  color/control bytes where no conversion is needed. Consider metadata or a
  reference RDM for the source layout; check what survives Blender export.
- Keep existing CLI presets and older GLBs compatible, including vegetation
  with different color-layer counts.
- Compare descriptors and attribute bytes separately from whole-file hashes.
  Track losses from quantization, topology edits and shading conversions.

See [current round-trip behavior](RDM_ROUNDTRIP.md) for measured limitations.
