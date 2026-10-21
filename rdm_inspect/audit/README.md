# Corpus audit helpers

Run these from `/home/lukas/.codex/worktrees/43bc/rdm4`.
[REPORT.md](REPORT.md) contains the findings. This directory adds audit tooling;
it does not change the converter or inspector's inference algorithm.

## Inputs

Keep extracted members under `<corpus>/<archive-name>/<asset-path>` so patches
and duplicate archive members remain distinguishable. `extract.py` accepts a
`maindata` directory, output directory, and RdaConsole executable. It copies
archives to scratch before extraction because RdaConsole opens inputs writable.

The checked inputs for this run are:

- Anno 2070: `/tmp/rdm4-be-audit-20261020/extracted`
- Anno 1800: `/tmp/rdm4-anno1800-corpus-20261021/extracted`
- Fresh verification extraction: `/tmp/rdm-layout-audit-20261021/fresh1800/extracted`

Archive hashes for 2070 and the fresh member comparison for 1800 are recorded
with the results. No proprietary game files are added to this worktree.

## Run

Build the inspector batch example in this worktree:

```sh
nix develop --command cargo build --release -p rdm-inspect --example corpus
```

The Rust batch example now reports `known_missing_incoming`,
`known_multi_incoming`, `known_root_incoming`, `known_unreached`, and
`known_tree`, using recovered pointer fields. It counts every reference before
skipping previously visited target blocks. `known_error` reports unsupported
record widths, conflicting types, or invalid pointer targets. The existing
candidate columns remain separate and do not claim semantic pointers.

Build the converter from the revision being evaluated. This run used the
master-based checkout at `/home/lukas/w/rdm4`:

```sh
cd /home/lukas/w/rdm4
nix develop --command cargo build --release -p rdm4-bin
```

Run from the inspector worktree, with a fresh output directory:

```sh
python3 rdm_inspect/audit/run.py \
  --anno2070 /tmp/rdm4-be-audit-20261020/extracted \
  --anno1800 /tmp/rdm4-anno1800-corpus-20261021/extracted \
  --converter /home/lukas/w/rdm4/target/release/rdm4-bin \
  --inspector target/release/examples/corpus \
  --output /tmp/rdm-audit-rerun
```

Validate generated glTF inputs and intermediate exports separately:

```sh
nix develop --command gltf_validator /tmp/rdm-audit-rerun/generated
```

The validator is expected to return failure for reproduced invalid exports;
check individual reports. Conversion failures are captured in `generation.json`
and `probes.json`; they do not abort the corpus audit.

For just the layout and schema checks:

```sh
python3 rdm_inspect/audit/audit.py EXTRACTED_ROOT OUTPUT_DIRECTORY
```

## Outputs

- `anno2070/files.jsonl`, `anno1800/files.jsonl`, `generated-audit/files.jsonl`:
  relative path, SHA-256, framing status, schema graph checks, model descriptors,
  and findings. `summary.json` beside each file aggregates counts.
- `*-inspector.tsv`: results from the actual inspector, including forward and
  backward candidate counts, inferred multiple incoming edges, and warnings.
- `*-skin.json`: additional checks on unique file contents, including packed
  weight sums, active influences, and joint ranges. Counts here are deduplicated
  by file SHA-256, unlike the primary archive-member counts.
- `selected-sources.json`: exact source paths/hashes and why their formats differ.
- `generation.json`, `probes.json`: full converter commands, exit codes and logs.
- `generated/`: test inputs, outputs, logs and optional validator reports.

`verify_probes.py` inspects specific output cases from the recorded selection
(`source-033` and `source-053`). Those labels belong to this corpus snapshot;
recheck `selected-sources.json` before using it with other archives. Set
`RDM_AUDIT_ROOT` to the result directory when running it directly.

The runners use Python's standard library only. `run.py` supplies the
`RDM_AUDIT_ROOT`, `RDM_AUDIT_2070`, `RDM_AUDIT_1800`, and
`RDM_AUDIT_CONVERTER` environment variables used by the subordinate scripts.

## Checks

```sh
python3 -m unittest discover -s rdm_inspect/audit -p 'test_*.py'
nix develop --command cargo test -p rdm-inspect --lib
```

The audit uses only declared pointer fields for tree checks. Width mismatches,
unreached blocks, and schema errors remain visible in the output. Inferred
boundary matches are not promoted to semantic pointers. Physical framing,
schema validity, and conversion success are reported separately.
