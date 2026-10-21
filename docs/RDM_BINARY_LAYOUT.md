# RDM binary layout

RDM stores models and animations used by Anno games.

**This document covers binary framing: the file header, blocks, offsets, and
byte order. It does not describe the semantics of individual model or animation
records.**

The layout resembles a bump-allocated object graph written to disk: blocks
follow one another, and records refer to other blocks by offset. The file
stores 32-bit absolute offsets in place of memory pointers. Records may be
unaligned, and numeric fields use the file's byte order.

## File header and byte order

RDM files start with a 20-byte preamble, followed by the root block:

```text
Offset  Size  Contents
0x00    4     "RDM\x01" (little-endian) or "RDM\x00" (big-endian)
0x04    4     0x14
0x08    4     0
0x0c    4     4
0x10    4     root payload offset: 0x1c
0x14    4     root count
0x18    4     root stride
0x1c    ...   root payload
```

Preamble words, block headers, offsets, and decoded numeric fields use the
file's byte order. Big-endian RDM files occur in Anno 2070 assets.
Packed vertex components also need endian-aware decoding; a
four-byte component is not necessarily four independent bytes.

The root block starts at `0x14`; its payload starts at `0x1c`. The word at
`0x10` points to that payload. The word at `0x04` contains `0x14`, matching
the root header position, but its meaning is unknown. It has not been
established as a header pointer. The words at `0x08` and `0x0c` are also
unexplained.

The root has the same block header as other blocks. All fixtures have one
48-byte root record. Its populated offsets distinguish models from animations.
A 52-byte variant is also reported, but has no retained sample for verification.

## Blocks and offsets

Each block has an eight-byte header followed by `count` records:

```text
H + 0       count       u32
H + 4       stride      u32
H + 8 = P   payload     count * stride bytes
```

```text
payload_offset = H + 8
payload_size   = count * stride
next_header    = H + 8 + count * stride
```

`stride` is the on-disk width of one record, excluding any referenced blocks.
It need not match a language type's native `sizeof`. Model metadata, for
example, occurs with both 68-byte and 92-byte strides. Metadata blocks can
also contain multiple records.

Offsets are unsigned 32-bit absolute file positions:

- `0` is a null reference.
- Nonzero offsets point to a block's first payload byte.
- The target's header starts at `offset - 8`.
- All identified references in the fixtures point forward to payload
  boundaries, not to individual records within a block.

**RDM is not self-describing.** `count` and `stride` give block boundaries,
but do not identify record types or tell a reader which fields contain offsets.
Those require knowledge of the record layout.

Readers should check offsets and bounds before access and use checked
arithmetic for `count * stride` and offset calculations.

## C view

A flexible array member illustrates why offsets point eight bytes past the
block header. Byte arrays avoid host byte-order and integer-alignment assumptions:

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

/* file_base points to byte 0 of the file loaded into memory.
   payload_offset is a decoded, nonzero file offset.
   Validate the offset and block bounds before forming these pointers. */
const uint8_t *payload = file_base + payload_offset;
const RdmBlock *block = container_of(payload, RdmBlock, payload);
```

`file_base` is a `const uint8_t *`, so adding `payload_offset` advances by
that many bytes from the start of the file. `container_of` then steps back
eight bytes to the block header. Decode `count_bytes` and `stride_bytes` in
the file's byte order.

## Block order and references

In the fixtures, blocks fill the file from `0x14` to EOF without gaps or
trailing bytes:

```text
[ preamble ][ root header | root payload ][ header | payload ] ...

file_size = 0x14 + sum(8 + count * stride)
```

Headers and payloads can start at odd offsets; there is no alignment padding.

The identified references form a rooted tree: each non-root block has exactly
one incoming reference, and every block is reachable. Blocks appear in
depth-first preorder. Each block stores all its direct records before any
child blocks; each child's subtree precedes the next sibling.

```text
        Root
       /    \
      A      C          Physical order: Root, A, B, D, C, E
     / \      \
    B   D      E
```

Children usually follow offset-field order. Model metadata is an exception:
its fields point to vertex data, indices, then mesh information, but those
three blocks are stored as mesh information, vertex data, then indices.

These are observed properties, not restrictions of absolute offsets. The
encoding could represent shared blocks, backward references, or cycles.
None occur among the identified references in the fixtures; whether games
accept them is unknown.

### Game reader and structural scanner

The examined Anno 2070 `2.00.7792` reader:

1. Loads the whole file into memory.
1. Follows root and child offsets using `file_base + offset`.
1. Reads each target's count and stride at `offset - 8` and `offset - 4`.
1. Uses record-specific code to interpret fields and follow child references.

A structural scanner instead walks the blocks using `8 + count * stride`,
then infers offsets by matching stored values to valid block payload starts.
These matches are candidate graph edges: ordinary values can match too. The
scanner needs no record layouts; the game reader knows which fields are offsets.
