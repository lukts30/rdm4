//! Record descriptions build a block graph before any output is written.
use std::{
    borrow::Cow,
    io::{self, Write},
    ops::{Index, IndexMut},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BlockId(usize);
#[derive(Clone, Copy, Debug)]
struct RecordIndex(usize);
#[derive(Clone, Copy, Debug)]
struct FieldOffset(usize);

// Block tables accept the ID directly, keeping its integer representation here.
impl<T> Index<BlockId> for Vec<T> {
    type Output = T;

    fn index(&self, id: BlockId) -> &T {
        &self[id.0]
    }
}

impl<T> IndexMut<BlockId> for Vec<T> {
    fn index_mut(&mut self, id: BlockId) -> &mut T {
        &mut self[id.0]
    }
}

impl RecordIndex {
    fn byte_offset(self, stride: u32) -> usize {
        self.0 * stride as usize
    }
}

impl FieldOffset {
    fn bytes(self) -> usize {
        self.0
    }
}

// A four-byte offset field within one record of a source block.
struct OffsetSlot {
    block: BlockId,
    record: RecordIndex,
    field: FieldOffset,
}

// The source is a specific slot; the target is the start of a block's payload.
// IDs remain stable while the block and payload vectors grow during planning.
struct Edge {
    source: OffsetSlot,
    target: BlockId,
}

struct Block {
    count: u32,
    stride: u32,
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
        }
        self.append_u32(0);
    }
}

/// Append fields directly into the destination record, including inline composites.
pub(crate) trait Describe {
    fn append_to<'a>(&'a self, plan: &mut Plan<'a>, record: &mut Record);

    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut record = Record::new(plan);
        self.append_to(plan, &mut record);
        record
    }
}

impl Describe for u8 {
    fn append_to<'a>(&'a self, _: &mut Plan<'a>, record: &mut Record) {
        record.append_bytes(&[*self]);
    }
}
impl Describe for u32 {
    fn append_to<'a>(&'a self, _: &mut Plan<'a>, record: &mut Record) {
        record.append_u32(*self);
    }
}
impl Describe for f32 {
    fn append_to<'a>(&'a self, _: &mut Plan<'a>, record: &mut Record) {
        record.append_u32(self.to_bits());
    }
}
impl<T: Describe, const N: usize> Describe for [T; N] {
    fn append_to<'a>(&'a self, plan: &mut Plan<'a>, record: &mut Record) {
        for value in self {
            value.append_to(plan, record);
        }
    }
}
impl<T: Describe> Describe for Vec<T> {
    fn append_to<'a>(&'a self, plan: &mut Plan<'a>, record: &mut Record) {
        for value in self {
            value.append_to(plan, record);
        }
    }
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
        let id = self.reserve_record_block();
        let record = value.describe(self);
        self.fill_record_block(id, vec![record]);
        id
    }
    pub(crate) fn add_record_array(&mut self, values: &'a [impl Describe]) -> BlockId {
        assert!(!values.is_empty(), "RDM record blocks cannot be empty");
        let id = self.reserve_record_block();
        let records = values.iter().map(|value| value.describe(self)).collect();
        self.fill_record_block(id, records);
        id
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
    // Reserve the parent's identity before describing any descendants. The first
    // record block (the file root) is therefore BlockId(0), regardless of its children.
    fn reserve_record_block(&mut self) -> BlockId {
        let id = BlockId(self.graph.blocks.len());
        self.graph.blocks.push(Block {
            count: 0,
            stride: 0,
        });
        self.payloads.push(Cow::Owned(Vec::new()));
        id
    }

    fn fill_record_block(&mut self, id: BlockId, records: Vec<Record>) {
        assert!(!records.is_empty(), "RDM record blocks cannot be empty");
        let stride = records[0].bytes.len();
        assert_eq!(self.graph.blocks[id].count, 0, "block already filled");
        let mut payload = Vec::new();
        let count = u32::try_from(records.len()).unwrap();
        for (index, record) in records.into_iter().enumerate() {
            assert_eq!(record.bytes.len(), stride, "mixed record strides");
            for (field, target) in record.offsets {
                self.graph.edges.push(Edge {
                    source: OffsetSlot {
                        block: id,
                        record: RecordIndex(index),
                        field,
                    },
                    target,
                });
            }
            payload.extend(record.bytes);
        }
        self.graph.blocks[id] = Block {
            count,
            stride: u32::try_from(stride).unwrap(),
        };
        self.payloads[id] = Cow::Owned(payload);
    }

    pub(crate) fn add_raw_block(&mut self, count: u32, stride: u32, bytes: &'a [u8]) -> BlockId {
        assert!(count > 0 && stride > 0);
        assert_eq!(bytes.len() as u64, u64::from(count) * u64::from(stride));
        let id = BlockId(self.graph.blocks.len());
        self.graph.blocks.push(Block { count, stride });
        self.payloads.push(Cow::Borrowed(bytes));
        id
    }

    pub(crate) fn write_file(mut self, root: BlockId, out: &mut impl Write) -> io::Result<()> {
        // Parents are reserved before their descendants, so registration already
        // gives preorder output. Creating a block schedules it; appending an offset
        // only records a reference and cannot change that order.
        // Final payload offsets are indexed by stable BlockId, like both block tables.
        let mut offsets = vec![0u32; self.graph.blocks.len()];
        let mut end = 20u64;
        for (index, block) in self.graph.blocks.iter().enumerate() {
            let id = BlockId(index);
            offsets[id] = u32::try_from(end + 8).expect("RDM offset exceeds u32");
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
            let source = &edge.source;
            let source_block = &self.graph.blocks[source.block];
            // Convert the record-relative field position to a position within the payload.
            let slot_start = source.record.byte_offset(source_block.stride) + source.field.bytes();
            // RDM stores an absolute file offset to the target payload, after its header.
            let target_payload_offset = offsets[edge.target];
            let encoded_offset = encode(target_payload_offset);

            // Resolve the placeholder in memory before writing anything to the output.
            // Edge sources are owned record bytes, so to_mut() does not clone a buffer.
            let source_payload = self.payloads[source.block].to_mut();
            let slot = &mut source_payload[slot_start..slot_start + encoded_offset.len()];
            slot.copy_from_slice(&encoded_offset);
        }
        out.write_all(b"RDM")?;
        out.write_all(&[u8::from(!self.big_endian)])?;
        for word in [20u32, 0, 4, offsets[root]] {
            out.write_all(&encode(word))?;
        }
        for (index, block) in self.graph.blocks.iter().enumerate() {
            let id = BlockId(index);
            out.write_all(&encode(block.count))?;
            out.write_all(&encode(block.stride))?;
            out.write_all(&self.payloads[id])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rdm_derive::RdmDescribe;

    #[test]
    fn derives_nested_offset_slots_without_a_reader() {
        #[derive(RdmDescribe)]
        struct Named {
            name: Vec<u8>,
        }
        #[derive(RdmDescribe)]
        struct Leaf {
            value: u32,
        }
        #[derive(RdmDescribe)]
        struct Parent {
            tag: u32,
            nested: Named,
            #[rdm(offset)]
            child: Leaf,
            #[rdm(skip)]
            _runtime_flag: bool,
        }
        let value = Parent {
            tag: 7,
            nested: Named {
                name: b"ab".to_vec(),
            },
            child: Leaf { value: 99 },
            _runtime_flag: true,
        };
        let mut plan = Plan::new(false);
        let root = plan.add_record(&value);
        let mut bytes = Vec::new();
        plan.write_file(root, &mut bytes).unwrap();
        let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        assert_eq!(word(24), 12); // Parent stride: tag and two offset slots.
        assert_eq!(word(28), 7);
        assert_eq!(word(32), 48); // Nested name slot appended after the tag.
        assert_eq!(&bytes[48..50], b"ab");
        assert_eq!(word(36), 58); // Separate single-record child block.
        assert_eq!(word(58), 99);
    }
}
