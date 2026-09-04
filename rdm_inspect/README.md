# rdm-inspect

`rdm-inspect` is a standalone structural viewer for RDM files. It deliberately
does not link to `rdm4lib` or use model and animation record definitions.

## How discovery works

1. Decode the byte order from `RDM\x01` or `RDM\x00`.
1. Starting at `0x14`, walk the gapless block sequence using
   `8 + count * stride` until EOF.
1. Build a set of every discovered payload start.
1. Scan every possible four-byte field position inside every record. A value
   equal to a payload start is an inferred offset.
1. Treat forward matches as strong edges because all files covered by the
   format documentation use forward offsets. Preserve self and backward
   matches as lower-confidence candidates so scalar collisions remain visible
   without contaminating the main graph.

The labels are structural: root record, record or byte arrays, records with
references, and conservative UTF-8 text candidates. The tool never labels a
block as a mesh, vertex buffer, joint, or animation frame without a semantic
schema.

## Build and run

Install only the inspector:

```console
cargo install --path rdm_inspect
rdm-inspect model.rdm
```

Or run it from the workspace:

```console
cargo run -p rdm-inspect -- model.rdm
```

The default launches the TUI when stdout is a terminal. It falls back to the
text dump when output is redirected.

```console
rdm-inspect model.rdm --dump
rdm-inspect model.rdm --graph
rdm-inspect model.rdm --dot > model.dot
```

`--dump` starts with a compact physical block table. String-like byte blocks
receive an inline, truncated preview; all other meanings remain conservative.
The `In` and `Out` columns count every inferred boundary match, while the edge
list below the table keeps strong and lower-confidence matches separate:

```text
Block  Header      Payload     Meaning                         Count     Stride      Bytes  In Out
---------------------------------------------------------------------------------------------------
#000   0x00000014  0x0000001c  root record                            1        48        48   1   3
#001   0x0000004c  0x00000054  opaque record                          1        72        72   1   2
#002   0x0000009c  0x000000a4  probable string "G:\graphic_b…       100         1       100   1   0
#011   0x0000028b  0x00000293  record array                        3291        24     78984   1   0
#012   0x0001371b  0x00013723  16-bit scalar-shaped array          7473         2     14946   1   0
```

## TUI controls

- Arrow keys or `h`, `j`, `k`, `l`: move by byte or row.
- `v`: open or close the linear block inventory.
- Tab: jump to the next inferred outgoing offset in the selected record.
- Shift+Tab: jump to the next physical record boundary.
- `[` and `]`: move to the preceding or following block payload.
- Enter: follow the inferred offset under the cursor.
- `i`: jump from the selected block to its first strong incoming offset field.
- Backspace: return from a followed offset.
- `b`: toggle the current block and record-boundary overlay. Count and stride
  headers get separate backgrounds; payload records alternate backgrounds at
  every stored stride.
- `g` and `G`: jump to the beginning or end of the file and return to hex view.
- Ctrl+G: switch between the hex map and graph view.
- Page Up, Page Down, Home, and End: navigate quickly.
- `q`, Escape, or Ctrl-C: quit.

Colors distinguish the magic and preamble, count and stride headers, inferred
offset fields, text candidates, and ordinary record payloads. Adjacent offset
fields alternate colors and subtle backgrounds. Hovering an offset marks its
destination as `DST` when the target is visible. Selecting a referenced block
marks its first incoming field as `SRC`; `i` jumps there when it is off screen.
`[` and `]` gutter markers show visible block boundaries while the overlay is
enabled. The fixed-layout detail pane shows the selected block, record index,
field-relative offset, incoming source, generic numeric interpretations, graph
target when applicable, and the outgoing offsets in the current record.

The block inventory uses the same conservative meanings and `In`/`Out` counts
as `--dump`. Every block with inferred outgoing offsets has a `▸` marker,
including root and single-record blocks. Select one and press Enter, Space, or
Right to expand its per-record offset candidates; Left collapses it. Strong and
lower-confidence candidates use distinct colors. Press Enter on an expanded
candidate to inspect its source field in the hex view.
