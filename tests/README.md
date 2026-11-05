# Blender compatibility tests

`blender_contract.rs` runs `rdm4-bin` with arguments used by
[Blender-Anno-1800 at e0391b3](https://github.com/anno-mods/Blender-Anno-1800/tree/e0391b394f12e8fe9134524fd90508e606877e81).
It checks output filenames, paths with spaces, overwrite behavior,
coordinates, winding, material associations, skinning and animation times.
The binary unit test parses all six add-on vertex-format choices.

No Blender installation or external checkout is needed. The export test
uses a converter-generated GLB. Blender import behavior, Windows shell
quoting and DDS conversion need separate tests. `rdm4ng_cli.rs` checks the
new CLI and aliases against the legacy executable's conversion output.

```sh
nix develop --command cargo test --release --locked -p rdm4-bin --test blender_contract
nix develop --command cargo test --release --locked -p rdm4-bin --bin rdm4-bin
```
