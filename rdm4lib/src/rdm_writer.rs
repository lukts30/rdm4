//! Record descriptions build a block graph before any output is written.
use std::{
    borrow::Cow,
    io::{self, Write},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BlockId(usize);
#[derive(Clone, Copy, Debug)]
struct RecordIndex(usize);
#[derive(Clone, Copy, Debug)]
struct FieldOffset(usize);

struct Edge {
    source: BlockId,
    record: RecordIndex,
    field: FieldOffset,
    target: BlockId,
}

struct Block {
    count: u32,
    stride: u32,
    children: Vec<BlockId>,
}

// Structural information only: no native values, writer callbacks, or payload bytes.
#[derive(Default)]
struct Graph {
    blocks: Vec<Block>,
    edges: Vec<Edge>,
}

#[derive(Default)]
pub(crate) struct Record {
    big_endian: bool,
    bytes: Vec<u8>,
    offsets: Vec<(FieldOffset, BlockId)>,
    children: Vec<BlockId>,
}

impl Record {
    pub(crate) fn new(plan: &Plan<'_>) -> Self {
        Self {
            big_endian: plan.big_endian,
            ..Self::default()
        }
    }
    pub(crate) fn append_bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
    pub(crate) fn append_u32(&mut self, value: u32) {
        self.append_bytes(&if self.big_endian {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        });
    }
    pub(crate) fn append_f32s(&mut self, values: &[f32]) {
        for value in values {
            self.append_u32(value.to_bits());
        }
    }
    pub(crate) fn append_offset(&mut self, target: Option<BlockId>) {
        if let Some(target) = target {
            self.offsets.push((FieldOffset(self.bytes.len()), target));
            self.children.push(target);
        }
        self.append_u32(0);
    }
    // Set the desired sibling order in the file (Meta places mesh groups before buffers).
    // This does not reverse anything for DFS or move the offset slots in this record.
    pub(crate) fn set_child_order(&mut self, children: &[Option<BlockId>]) {
        let children: Vec<_> = children.iter().flatten().copied().collect();
        assert_eq!(children.len(), self.children.len());
        assert!(self.children.iter().all(|child| children.contains(child)));
        self.children = children;
    }
}

pub(crate) trait Describe {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record;
}

/// Writer-owned bytes are separate from the structural graph.
#[derive(Default)]
pub(crate) struct Plan<'a> {
    big_endian: bool,
    graph: Graph,
    // Structured records own their newly encoded bytes; leaf blocks borrow strings and
    // raw buffers from the model. Offset resolution mutates only owned record payloads,
    // so it does not copy the borrowed buffers. Indexed by the same BlockId as graph.blocks.
    payloads: Vec<Cow<'a, [u8]>>,
}

impl<'a> Plan<'a> {
    pub(crate) fn add_record(&mut self, value: &'a impl Describe) -> BlockId {
        let record = value.describe(self);
        self.add_encoded_records(vec![record])
    }
    pub(crate) fn add_record_array(&mut self, values: &'a [impl Describe]) -> BlockId {
        let records = values.iter().map(|value| value.describe(self)).collect();
        self.add_encoded_records(records)
    }
    pub(crate) fn add_byte_block(&mut self, bytes: &'a [u8]) -> BlockId {
        self.add_raw_block(u32::try_from(bytes.len()).unwrap(), 1, bytes)
    }

    pub(crate) fn new(big_endian: bool) -> Self {
        Self {
            big_endian,
            ..Self::default()
        }
    }
    fn add_encoded_records(&mut self, records: Vec<Record>) -> BlockId {
        assert!(!records.is_empty(), "RDM record blocks cannot be empty");
        let stride = records[0].bytes.len();
        let id = BlockId(self.graph.blocks.len());
        let mut payload = Vec::new();
        let mut children = Vec::new();
        let count = u32::try_from(records.len()).unwrap();
        for (index, record) in records.into_iter().enumerate() {
            assert_eq!(record.bytes.len(), stride, "mixed record strides");
            for (field, target) in record.offsets {
                self.graph.edges.push(Edge {
                    source: id,
                    record: RecordIndex(index),
                    field,
                    target,
                });
            }
            children.extend(record.children);
            payload.extend(record.bytes);
        }
        self.graph.blocks.push(Block {
            count,
            stride: u32::try_from(stride).unwrap(),
            children,
        });
        self.payloads.push(Cow::Owned(payload));
        id
    }

    pub(crate) fn add_raw_block(&mut self, count: u32, stride: u32, bytes: &'a [u8]) -> BlockId {
        assert!(count > 0 && stride > 0);
        assert_eq!(bytes.len() as u64, u64::from(count) * u64::from(stride));
        let id = BlockId(self.graph.blocks.len());
        self.graph.blocks.push(Block {
            count,
            stride,
            children: Vec::new(),
        });
        self.payloads.push(Cow::Borrowed(bytes));
        id
    }

    pub(crate) fn write_file(mut self, root: BlockId, out: &mut impl Write) -> io::Result<()> {
        let mut order = Vec::new();
        let mut seen = vec![false; self.graph.blocks.len()];
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if !seen[id.0] {
                seen[id.0] = true;
                order.push(id);
                // children is already in desired file order. Push [C, B, A] so the
                // LIFO stack pops A first; this preserves that order rather than changing it.
                stack.extend(self.graph.blocks[id.0].children.iter().rev().copied());
            }
        }
        assert!(seen.iter().all(|seen| *seen), "unreachable output block");
        let mut offsets = vec![0u32; self.graph.blocks.len()];
        let mut end = 20u64;
        for id in &order {
            let block = &self.graph.blocks[id.0];
            offsets[id.0] = u32::try_from(end + 8).expect("RDM offset exceeds u32");
            end += 8 + u64::from(block.count) * u64::from(block.stride);
        }
        assert!(
            end <= u64::from(u32::MAX),
            "RDM exceeds 32-bit address space"
        );
        let encode = |value: u32| {
            if self.big_endian {
                value.to_be_bytes()
            } else {
                value.to_le_bytes()
            }
        };
        for edge in &self.graph.edges {
            let block = &self.graph.blocks[edge.source.0];
            let at = edge.record.0 * block.stride as usize + edge.field.0;
            self.payloads[edge.source.0].to_mut()[at..at + 4]
                .copy_from_slice(&encode(offsets[edge.target.0]));
        }
        out.write_all(b"RDM")?;
        out.write_all(&[u8::from(!self.big_endian)])?;
        for word in [20u32, 0, 4, offsets[root.0]] {
            out.write_all(&encode(word))?;
        }
        for id in order {
            let block = &self.graph.blocks[id.0];
            out.write_all(&encode(block.count))?;
            out.write_all(&encode(block.stride))?;
            out.write_all(&self.payloads[id.0])?;
        }
        Ok(())
    }
}
