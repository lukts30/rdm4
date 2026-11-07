# RDM round trips

Measured at `11e8a9e`: `RDM → GLB → RDM`, using
`P4h_N4b_G4b_B4b_T2h` and `--no-transform` on import. These are direct
converter round trips, without editing or re-exporting through Blender.

```sh
rdm4ng gltf rdm4lib/rdm/fishery_others_lod2.rdm -o /tmp/fishery.glb
rdm4ng rdm /tmp/fishery.glb --format P4h_N4b_G4b_B4b_T2h --no-transform -o /tmp/fishery.rdm
```

## Geometry and materials

| Fixture                                                 | Models | Draw groups | Group ranges and material slots (`rdm4ng`) |
| ------------------------------------------------------- | -----: | ----------: | ------------------------------------------ |
| Fishery (`fishery_others_lod2`)                         |      1 |           2 | Exact                                      |
| Residence (`residence_tier_02_estate_02_lod2`)          |      1 |           3 | Exact                                      |
| Palace (`benchmark/12-palace_hall_tpart_building_lod3`) |      1 |           2 | Exact                                      |
| Monument (`benchmark/20-monument_01_04_var3_lod0`)      |     20 |          38 | Exact                                      |

Fixtures are under `rdm4lib/rdm/`, with the `.rdm` extension. All retained
vertex counts, index arrays/order, index widths, position bytes and UV bytes.
The legacy CLI retains its primitive-order material defaults: palace's slots
`[1, 0]` become `[0, 1]`. `rdm4ng` preserves `[1, 0]`.

Whole-file bytes differ: fresh output rebuilds layout and metadata, and recomputes
bounds. Palace's bounds changed despite identical positions. For unchanged
records, `RdmWriter::write_preserving` reproduces the source bytes and layout
without going through GLB.

## Shading: follow-up work

Packed shading attributes do not survive exactly. For fishery's 3,291 vertices:

- 1,874 normal words changed; maximum direction error was approximately 0.33°.
- All tangent words changed; directions were approximately 180° reversed.
- 3,223 bitangent words changed; some directions also reversed.

RDM packing negates tangent XYZ; glTF export supplies `w = 1` for packed tangents,
and RDM packing reconstructs bitangents. This convention needs fixing. Fishery's complete
legacy round-trip output matched the saved pre-migration executable, confirming
the behavior predates the scene migration.

## Hashes and history

Compatibility audits compare converter versions; round-trip audits compare
the converted asset with its original. Matching the previous converter does
not establish lossless conversion.

- `7b9d7a6`: replaced template bounds with calculated bounds; mesh hashes changed.
- `e56cc90`: record writer changed mesh block placement/reference offsets;
  mesh hashes changed again. Animation's `c218…` hash stayed identical.
- `eb06b1e`: removed whole-vertex-buffer hash deduplication. Primitives now share
  columns by attribute-accessor identity; separate equal-valued accessors are
  not automatically welded. Primitive indices and material slots stay separate.
- `e32b259`: moved conversion tests to scene/document APIs, retaining active
  static/skinned/animation conversion hashes. Replaced the old `8f048…` rewrite
  golden with exact source-byte preservation, checking a different operation.
- `11e8a9e`: retired model adapters and writer wrappers; CLI output stayed
  byte-identical in the compatibility audit.
