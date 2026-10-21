//! Record descriptions build a block graph before any output is written.
use std::io::{self, Write};

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
    bytes: Vec<u8>,
    offsets: Vec<(FieldOffset, BlockId)>,
    children: Vec<BlockId>,
}

impl Record {
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        assert!(self.offsets.is_empty());
        self.bytes
    }
    pub(crate) fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }
    pub(crate) fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }
    pub(crate) fn floats(&mut self, values: &[f32]) {
        for value in values {
            self.u32(value.to_bits());
        }
    }
    pub(crate) fn offset(&mut self, target: Option<BlockId>) {
        if let Some(target) = target {
            self.offsets.push((FieldOffset(self.bytes.len()), target));
            self.children.push(target);
        }
        self.u32(0);
    }
    // Change only traversal order; slots retain their positions in the record.
    pub(crate) fn children(&mut self, children: &[Option<BlockId>]) {
        let children: Vec<_> = children.iter().flatten().copied().collect();
        assert_eq!(children.len(), self.children.len());
        assert!(self.children.iter().all(|child| children.contains(child)));
        self.children = children;
    }
}

pub(crate) trait Describe {
    fn byte(&self) -> u8 {
        panic!("not a byte record")
    }
    fn describe(&self, plan: &mut Plan) -> Record;
}

/// Writer-owned bytes are separate from the structural graph.
#[derive(Default)]
pub(crate) struct Plan {
    graph: Graph,
    payloads: Vec<Vec<u8>>,
}

impl Plan {
    pub(crate) fn records(&mut self, records: Vec<Record>) -> BlockId {
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
        self.payloads.push(payload);
        id
    }

    pub(crate) fn raw(&mut self, count: u32, stride: u32, bytes: Vec<u8>) -> BlockId {
        assert!(count > 0 && stride > 0);
        assert_eq!(bytes.len() as u64, u64::from(count) * u64::from(stride));
        let id = BlockId(self.graph.blocks.len());
        self.graph.blocks.push(Block {
            count,
            stride,
            children: Vec::new(),
        });
        self.payloads.push(bytes);
        id
    }

    pub(crate) fn write(mut self, root: BlockId, out: &mut impl Write) -> io::Result<()> {
        let mut order = Vec::new();
        let mut seen = vec![false; self.graph.blocks.len()];
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            if !seen[id.0] {
                seen[id.0] = true;
                order.push(id);
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
        for edge in &self.graph.edges {
            let block = &self.graph.blocks[edge.source.0];
            let at = edge.record.0 * block.stride as usize + edge.field.0;
            self.payloads[edge.source.0][at..at + 4]
                .copy_from_slice(&offsets[edge.target.0].to_le_bytes());
        }
        out.write_all(b"RDM\x01")?;
        for word in [20u32, 0, 4, offsets[root.0]] {
            out.write_all(&word.to_le_bytes())?;
        }
        for id in order {
            let block = &self.graph.blocks[id.0];
            out.write_all(&block.count.to_le_bytes())?;
            out.write_all(&block.stride.to_le_bytes())?;
            out.write_all(&self.payloads[id.0])?;
        }
        Ok(())
    }
}
