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

Fishery's complete legacy round-trip output matched the saved pre-migration
executable. Tangent negation dates to `db468e1` (2020), before the helper
extraction and scene migration. The mismatch does not establish that negating
the imported tangent is wrong.

### Current conversion

For unit glTF vectors, identity transforms and unchanged UVs, let
`B = cross(N, T.xyz) * T.w`, as required by the
[glTF specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#meshes).
Before quantization:

- [RDM packing](../rdm4lib/src/rdm/packing.rs) stores `(N, -T.xyz, +B)`.
- [glTF export](../rdm4lib/src/gltf_export/writer.rs) normalizes decoded packed
  N/T, exports the stored tangent without negating it, assigns `w = +1`, and
  ignores the stored bitangent. Native F32x4 scene tangents retain their W in
  the new CLI.

These operations are not inverses. The export loses tangent handedness; the
import's bitangent sign also disagrees with the shader basis below if normal-map
R/G are unchanged. Both need validation before changing behavior.

### Shader evidence

Inspected standard model shaders use `(-stored_tangent, -stored_bitangent, N)`
after unpacking. They decode normal-map R/G with `2 * value - 1`, without a
separate green inversion. These are source findings, not an in-game render check
or a claim about every material shader.

- **Anno 2070 3.00.8051:** `patch7.rda`,
  `data/system/shaders/include/model.fxh`, lines 476–477 and 887–911.
  SHA-256 prefix `5a955e9f52987b14`. [Local extract](/tmp/rdm-tangent-shaders-8051/data/system/shaders/include/model.fxh).
  The demo source has the same calculations; its differences are outline parameters.
- **Anno 1800:** installed `maindata/data1.rda`,
  `data/shaders/include/model_common.fxh`, lines 179–181 and 393–460;
  included by `data/shaders/fx/model_pbr.fx`.
  SHA-256 prefix `f83198e139a9a41a`. [Local extract](/tmp/rdm-tangent-shaders-1800/data1.rda/data/shaders/include/model_common.fxh).
  `model_base.fxh`, lines 412–420, unpacks N/T/B first.

The Blender addon passes normal-map R/G through without an explicit channel flip
and reconstructs blue: see `io_annocfg/shaders/shader_node_templates.py::add_normal`
in [Blender-Anno-1800](https://github.com/anno-mods/Blender-Anno-1800).
Its X mirror is a separate geometry operation. Our GLB materials currently bind
base color only, so they do not expose tangent errors through normal mapping.

### Proposed conversion — not implemented

For the shader paths above, the same UVs/normal-map R/G and identity transforms:

```text
glTF -> RDM: stored N = N; stored T = -T.xyz; stored B = -B
RDM -> glTF: N = decoded N; T.xyz = -decoded T; B = -decoded B
             T.w = sign(dot(cross(N, T.xyz), B))
```

Validate both new glTF → RDM shading in-game and existing RDM → GLB → RDM
attribute errors. Reflections and transform baking require consistent basis and
winding changes. Texture channel packing is separate: glTF stores normal XYZ in
RGB; these Anno model paths reconstruct Z and can use blue for glow.

Even with corrected signs, glTF reconstructs B while RDM stores it independently.
Quantization and nonorthogonal source vectors can prevent exact shading-byte
recovery without additional source data. No conversion fix is included yet.

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
