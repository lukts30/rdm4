# RDM binary format

## Overview

RDM is a proprietary binary model and animation format used by Anno games from
[Ubisoft Mainz](https://www.ubisoft.com/en-us/company/careers/locations/germany/mainz),
formerly Related Designs. "RDM" may refer to the former studio name, but no
official expansion is known.

No public specification is available. The physical layout described here was
derived from reverse-engineering; payload-record semantics are out of scope.

Structurally, RDM is a flat, packed sequence of blocks connected by 32-bit
absolute file offsets. It resembles a bump-allocated C object graph frozen into
a file, but it is not a native memory image: offsets replace pointers, records
may be unaligned, and multi-byte values use the file byte order. Each block has
an eight-byte header immediately before the addressed payload. The same outer
format stores models and animations; populated root offsets distinguish the
two.

## Byte order

Almost all encountered RDM files are little-endian. A few big-endian files
appear in assets from early Anno 2070 development. Known little-endian files
begin with `RDM\x01`; the investigated big-endian variant begins with
`RDM\x00`. The fourth byte therefore distinguishes the byte order in the
examined files. Preamble words, block headers, offsets, and numeric record
fields use that byte order.

## File header and root

An RDM file begins with a 20-byte preamble. Numeric values are decoded below;
their byte representation depends on the file's byte order.

```text
Offset      Size    Contents

0x0000      4       "RDM\x01" little-endian, "RDM\x00" big-endian
0x0004      4       preamble word: 0x14
0x0008      4       preamble word: 0
0x000c      4       preamble word: 4
0x0010      4       root payload offset: 0x1c
0x0014      4       root count
0x0018      4       root stride
0x001c      ...     root payload
```

The field at `0x10` is the absolute offset of the root payload. It is encoded as
`1c 00 00 00` in little-endian files and `00 00 00 1c` in the examined
big-endian file. The root block's header begins eight bytes earlier, at `0x14`.

The other three preamble words are `0x14`, `0`, and `4` in both byte orders.
The first equals the root block's header position, but the purposes of these
three fields are not established.

The root uses the same block header as every other block. It is special only
because:

- its header is fixed at `0x14`;
- its payload begins at `0x1c`;
- its record is the entry point of the offset graph.

The root stride covers only the direct root record, not its descendants or the
remaining file. Most observed root records are 48 bytes wide; 52-byte records
also exist.

A block following the root is an ordinary referenced block, not a second file
header.

## Block layout

Every block has the following layout:

```text
Header offset H

H + 0       +-------------------------------+
            | count                         | u32
H + 4       +-------------------------------+
            | stride                        | u32
H + 8 = P   +-------------------------------+
            | record 0                      | stride bytes
            +-------------------------------+
            | record 1                      | stride bytes
            +-------------------------------+
            | ...                           |
            +-------------------------------+
            | record count - 1              | stride bytes
            +-------------------------------+
```

```text
payload_offset   P = H + 8
payload_size       = count * stride
block_size         = 8 + count * stride
next_header        = H + block_size
```

`stride` is the width of one direct record slot. It is not a recursive subtree
size and is not necessarily equal to the native `sizeof` of a corresponding
language type.

Records with the same known interpretation can have different on-disk widths.
For example, records interpreted as model metadata occur with both 68-byte and
92-byte strides. The stored `stride` is authoritative. The stored `count` is
likewise authoritative; a block that usually contains one record may contain
several.

The block framing is self-delimiting but not self-describing. `count` and
`stride` locate the next block, but neither identifies the payload's record
type. Record types and offset fields require external knowledge of each record
layout.

## Offsets

An RDM offset is an unsigned 32-bit absolute file offset.

- `0` is a null reference.
- A nonzero offset points to the first payload byte of a block.
- The block header begins at `offset - 8`.
- Known offsets target block payload boundaries, not individual records.
- All known non-null offsets in the examined files point forward.

```text
                  stored offset
                       |
                       v
    +----------+----------+-------------------------+
    | count    | stride   | payload                 |
    +----------+----------+-------------------------+
    ^                     ^
    |                     |
 offset - 8             offset
```

## C flexible-array view

A block can be described as a C flexible-array-member object. Byte arrays
represent the stored integers without imposing host alignment or byte order.

```c
#include <stddef.h>
#include <stdint.h>

typedef struct RdmBlock {
    uint8_t count_bytes[4];
    uint8_t stride_bytes[4];
    uint8_t payload[];
} RdmBlock;

#define container_of(ptr, type, member) \
    ((const type *)((const uint8_t *)(ptr) - offsetof(type, member)))

const uint8_t *payload = file_bytes + payload_offset;
const RdmBlock *block = container_of(payload, RdmBlock, payload);
```

The stored offset addresses the flexible array member. `container_of` recovers
the preceding count and stride bytes.

A reader must decode those byte arrays using the file byte order and validate
offsets, bounds, and `count * stride` before accessing a block.

## Physical block order

Known files contain a gapless sequence of blocks beginning at `0x14`:

```text
+----------+--------+-----------+--------+-----------+--------+-----------+
| preamble | A head | A payload | B head | B payload | C head | C payload |
+----------+--------+-----------+--------+-----------+--------+-----------+
           ^                    ^                    ^
           0x14                 next                 next
```

Repeatedly applying `next_header = H + 8 + count * stride` reaches end of file:

```text
file_size = 0x14 + sum(8 + count * stride)
```

No alignment rule is apparent. Block headers and payloads may start at odd file
offsets.

The normal block order is depth-first:

1. emit a block header;
1. emit all direct records in that block;
1. for each referenced child block, normally in offset-field order, emit its
   complete subtree before the next child.

A few record layouts are exceptions to the normal child order. For example, a
record containing offsets `a`, `b`, and `c` may have its targets stored as `a`,
`c`, `b`. The block header does not record this ordering.

The result is still a depth-first, preorder layout with forward offsets. This
is an observed layout convention, not a restriction imposed by absolute
offsets.

## Reference tree

The packed block sequence and the offset graph are two views of the same blocks:

Each node below is one complete block: its eight-byte header and its payload. An
edge is an absolute offset stored in a record inside the parent's payload. It
points to the child's payload; the child's header begins eight bytes earlier.

```text
Physical order

    [ Root ][ A ][ B ][ D ][ C ][ E ]

Known reference offsets

                  Root
                 /    \
                A      C
               / \     \
              B   D     E

Preorder

    Root, A, B, D, C, E
```

The known reference graph is a rooted tree:

- it is rooted at the fixed block beginning at `0x14`;
- each non-root block has exactly one known incoming offset.

The block sequence is a preorder traversal of this tree. Siblings normally
follow offset-field order; the exceptions described above use a different
sibling order.

Because blocks are contiguous, their boundaries can be enumerated from `count`
and `stride` without interpreting record layouts. This does not reveal which
record fields are offsets. Traversing from the root through those fields
requires record-layout knowledge and random access to the file, or a seekable
input stream. Physical enumeration reveals block boundaries; offset traversal
reveals known graph edges.

The file does not require each block to have only one incoming offset. Reusing
one payload offset would create a shared node:

```text
             Root
             /  \
            A    B
             \  /
            Shared
```

With forward-only offsets this remains a directed acyclic graph. Absolute
offsets could also represent back-edges or cycles, although none were found in
the examined files.

## Examined-file invariants

Across the examined model and animation files, including the big-endian sample:

- blocks cover every byte after the preamble, with no gaps, trailing bytes, or
  partial blocks;
- every known non-null offset points forward to a payload boundary;
- every non-root block has exactly one known incoming offset;
- no shared, unreachable, or invalidly targeted block occurs;
- physical block order is depth-first preorder; sibling order normally
  follows offset-field order, with a few record-specific exceptions;
- block boundaries are not required to be aligned.
