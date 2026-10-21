//! Meaning-agnostic structural analysis for RDM files.
//!
//! The analyzer deliberately knows only the physical format: the preamble,
//! block headers, record sizes, and absolute payload offsets. It does not use
//! any model- or animation-specific record definitions.

use std::collections::{HashMap, VecDeque};
use std::fmt;

pub mod output;
pub mod tui;

const PREAMBLE_SIZE: usize = 0x14;
const ROOT_HEADER_OFFSET: usize = 0x14;
const BLOCK_HEADER_SIZE: usize = 8;

/// Byte order selected by the fourth byte in the RDM magic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Endian {
    /// `RDM\x01`, with the least-significant byte stored first.
    Little,
    /// `RDM\x00`, with the most-significant byte stored first.
    Big,
}

impl Endian {
    fn read_u32(self, bytes: &[u8]) -> u32 {
        let value: [u8; 4] = bytes.try_into().unwrap();
        match self {
            Self::Little => u32::from_le_bytes(value),
            Self::Big => u32::from_be_bytes(value),
        }
    }
}

impl fmt::Display for Endian {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Little => write!(f, "little-endian"),
            Self::Big => write!(f, "big-endian"),
        }
    }
}

/// The four decoded numeric words in the RDM preamble.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Preamble {
    pub word_04: u32,
    pub word_08: u32,
    pub word_0c: u32,
    pub root_payload_offset: u32,
}

/// Index of a physical block, distinct from a byte offset into the file.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BlockId(usize);

impl BlockId {
    /// First block found by the physical scan.
    pub const ROOT: Self = Self(0);

    /// Construct an ID from an index in `Analysis::blocks`.
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Index used to access block-indexed vectors.
    pub const fn index(self) -> usize {
        self.0
    }
}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// One block discovered by walking the file linearly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub id: BlockId,
    /// Absolute byte offset of the block header.
    pub header_offset: usize,
    /// Absolute byte offset of the first record.
    pub payload_offset: usize,
    /// Exclusive absolute byte offset of the payload end.
    pub payload_end: usize,
    pub count: u32,
    pub stride: u32,
}

impl Block {
    /// Number of payload bytes in this block.
    pub fn payload_size(&self) -> usize {
        self.payload_end - self.payload_offset
    }

    /// Human-readable, structural label that does not claim record semantics.
    pub fn structural_label(&self, bytes: &[u8]) -> String {
        if self.id == BlockId::ROOT {
            return "Root record".to_owned();
        }
        if self.text_preview(bytes).is_some() {
            return "Text candidate".to_owned();
        }
        match (self.count, self.stride) {
            (_, 1) => "Byte records".to_owned(),
            (_, 2) => "2-byte records".to_owned(),
            (1, _) => "Single record".to_owned(),
            _ => "Record array".to_owned(),
        }
    }

    /// A conservative preview for stride-one blocks containing mostly text.
    pub fn text_preview(&self, bytes: &[u8]) -> Option<String> {
        if self.stride != 1 || self.payload_size() < 2 {
            return None;
        }
        let payload = &bytes[self.payload_offset..self.payload_end];
        let text = std::str::from_utf8(payload).ok()?;
        let visible = text
            .chars()
            .filter(|character| !character.is_control() || matches!(character, '\n' | '\r' | '\t'))
            .count();
        let total = text.chars().count();
        if total == 0 || visible * 100 / total < 85 {
            return None;
        }

        let mut preview = String::new();
        for character in text.chars().flat_map(char::escape_default).take(48) {
            preview.push(character);
        }
        if text.chars().flat_map(char::escape_default).count() > 48 {
            preview.push('…');
        }
        Some(preview)
    }
}

/// Direction of a payload-boundary match, not evidence that it is a pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EdgeDirection {
    /// Candidate targeting a later block.
    Forward,
    /// Candidate targeting the same block or an earlier block.
    BackwardOrSelf,
}

impl fmt::Display for EdgeDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Forward => write!(f, "forward candidate"),
            Self::BackwardOrSelf => write!(f, "backward/self candidate"),
        }
    }
}

/// A four-byte record field whose value equals a discovered payload boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edge {
    pub source_block: BlockId,
    pub target_block: BlockId,
    /// Absolute byte offset of the four-byte candidate value.
    pub file_offset: usize,
    pub record_index: u32,
    /// Byte offset within one record, not within the file.
    pub field_offset: u32,
    pub direction: EdgeDirection,
}

/// Complete structural interpretation of one RDM file.
#[derive(Clone, Debug)]
pub struct Analysis {
    pub bytes: Vec<u8>,
    pub endian: Endian,
    pub preamble: Preamble,
    pub blocks: Vec<Block>,
    pub edges: Vec<Edge>,
    pub warnings: Vec<String>,
}

impl Analysis {
    /// Parse bytes by first discovering every block, then scanning record
    /// payloads byte-by-byte for absolute offsets to those block payloads.
    pub fn parse(bytes: Vec<u8>) -> Result<Self, ParseError> {
        if bytes.len() < PREAMBLE_SIZE + BLOCK_HEADER_SIZE {
            return Err(ParseError::new(
                0,
                "file is shorter than the RDM preamble and root header",
            ));
        }
        if bytes.len() > u32::MAX as usize {
            return Err(ParseError::new(
                0,
                "file is larger than the 32-bit RDM offset address space",
            ));
        }
        if &bytes[..3] != b"RDM" {
            return Err(ParseError::new(0, "magic is not RDM"));
        }
        let endian = match bytes[3] {
            1 => Endian::Little,
            0 => Endian::Big,
            marker => {
                return Err(ParseError::new(
                    3,
                    format!("unknown byte-order marker 0x{marker:02x}"),
                ))
            }
        };
        let read_at = |offset: usize| endian.read_u32(&bytes[offset..offset + 4]);
        let preamble = Preamble {
            word_04: read_at(0x04),
            word_08: read_at(0x08),
            word_0c: read_at(0x0c),
            root_payload_offset: read_at(0x10),
        };

        let mut warnings = Vec::new();
        if (preamble.word_04, preamble.word_08, preamble.word_0c) != (0x14, 0, 4) {
            warnings.push(format!(
                "unusual preamble words: 0x{:x}, 0x{:x}, 0x{:x} (expected 0x14, 0, 4)",
                preamble.word_04, preamble.word_08, preamble.word_0c
            ));
        }
        if preamble.root_payload_offset != 0x1c {
            warnings.push(format!(
                "root payload offset is 0x{:x}, but the physical root payload is fixed at 0x1c",
                preamble.root_payload_offset
            ));
        }

        let mut blocks = Vec::new();
        let mut header_offset = ROOT_HEADER_OFFSET;
        loop {
            if bytes.len() - header_offset < BLOCK_HEADER_SIZE {
                return Err(ParseError::new(
                    header_offset,
                    "partial block header at end of file",
                ));
            }
            let count = read_at(header_offset);
            let stride = read_at(header_offset + 4);
            if count == 0 {
                return Err(ParseError::new(header_offset, "block count is zero"));
            }
            if stride == 0 {
                return Err(ParseError::new(header_offset + 4, "block stride is zero"));
            }
            let payload_size = (count as usize)
                .checked_mul(stride as usize)
                .ok_or_else(|| {
                    ParseError::new(header_offset, "block payload size overflows address space")
                })?;
            let payload_offset = header_offset + BLOCK_HEADER_SIZE;
            let payload_end = payload_offset.checked_add(payload_size).ok_or_else(|| {
                ParseError::new(header_offset, "block end overflows address space")
            })?;
            if payload_end > bytes.len() {
                return Err(ParseError::new(
                    header_offset,
                    format!(
                        "block {} payload ends at 0x{payload_end:x}, beyond file end 0x{:x}",
                        blocks.len(),
                        bytes.len()
                    ),
                ));
            }
            blocks.push(Block {
                id: BlockId::new(blocks.len()),
                header_offset,
                payload_offset,
                payload_end,
                count,
                stride,
            });
            if payload_end == bytes.len() {
                break;
            }
            header_offset = payload_end;
        }

        let targets: HashMap<u32, BlockId> = blocks
            .iter()
            .map(|block| (block.payload_offset as u32, block.id))
            .collect();
        let mut edges = Vec::new();
        for block in &blocks {
            if block.stride < 4 {
                continue;
            }
            let stride = block.stride as usize;
            for record_index in 0..block.count as usize {
                let record_start = block.payload_offset + record_index * stride;
                for field_offset in 0..=stride - 4 {
                    let file_offset = record_start + field_offset;
                    let value = endian.read_u32(&bytes[file_offset..file_offset + 4]);
                    let Some(&target_block) = targets.get(&value) else {
                        continue;
                    };
                    let direction = if target_block > block.id {
                        EdgeDirection::Forward
                    } else {
                        EdgeDirection::BackwardOrSelf
                    };
                    edges.push(Edge {
                        source_block: block.id,
                        target_block,
                        file_offset,
                        record_index: record_index as u32,
                        field_offset: field_offset as u32,
                        direction,
                    });
                }
            }
        }

        Ok(Self {
            bytes,
            endian,
            preamble,
            blocks,
            edges,
            warnings,
        })
    }

    /// Return the block whose header or payload contains `offset`.
    pub fn block_at(&self, offset: usize) -> Option<&Block> {
        self.blocks
            .iter()
            .find(|block| offset >= block.header_offset && offset < block.payload_end)
    }

    /// Structural label refined with the inferred reference topology.
    pub fn structural_label(&self, block_id: BlockId) -> String {
        let block = &self.blocks[block_id.index()];
        if block.id == BlockId::ROOT || block.text_preview(&self.bytes).is_some() {
            return block.structural_label(&self.bytes);
        }
        match (block.count, self.forward_outgoing_count(block_id)) {
            (1, 1..) => "Record + refs".to_owned(),
            (_, 1..) => "Array + refs".to_owned(),
            _ => block.structural_label(&self.bytes),
        }
    }

    /// Return an inferred offset field containing `offset`.
    pub fn edge_at(&self, offset: usize) -> Option<&Edge> {
        self.edges
            .iter()
            .find(|edge| offset >= edge.file_offset && offset < edge.file_offset + 4)
    }

    /// Incoming forward-candidate count for a block.
    pub fn forward_incoming_count(&self, block_id: BlockId) -> usize {
        self.edges
            .iter()
            .filter(|edge| {
                edge.target_block == block_id && edge.direction == EdgeDirection::Forward
            })
            .count()
    }

    /// Outgoing forward-candidate count for a block.
    pub fn forward_outgoing_count(&self, block_id: BlockId) -> usize {
        self.edges
            .iter()
            .filter(|edge| {
                edge.source_block == block_id && edge.direction == EdgeDirection::Forward
            })
            .count()
    }

    /// Blocks reachable from the root through forward candidates.
    pub fn forward_reachable(&self) -> Vec<bool> {
        let mut reached = vec![false; self.blocks.len()];
        let mut queue = VecDeque::from([BlockId::ROOT]);
        while let Some(block_id) = queue.pop_front() {
            if reached[block_id.index()] {
                continue;
            }
            reached[block_id.index()] = true;
            for edge in self.edges.iter().filter(|edge| {
                edge.source_block == block_id && edge.direction == EdgeDirection::Forward
            }) {
                queue.push_back(edge.target_block);
            }
        }
        reached
    }
}

/// Location-aware failure produced while enumerating physical blocks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseError {
    pub offset: usize,
    pub message: String,
}

impl ParseError {
    fn new(offset: usize, message: impl Into<String>) -> Self {
        Self {
            offset,
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at file offset 0x{:x}: {}", self.offset, self.message)
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn push_u32(bytes: &mut Vec<u8>, value: u32, endian: Endian) {
        bytes.extend(match endian {
            Endian::Little => value.to_le_bytes(),
            Endian::Big => value.to_be_bytes(),
        });
    }

    fn synthetic(endian: Endian, unaligned_field: bool) -> Vec<u8> {
        let mut bytes = match endian {
            Endian::Little => b"RDM\x01".to_vec(),
            Endian::Big => b"RDM\x00".to_vec(),
        };
        for value in [0x14, 0, 4, 0x1c] {
            push_u32(&mut bytes, value, endian);
        }
        push_u32(&mut bytes, 1, endian);
        push_u32(&mut bytes, if unaligned_field { 9 } else { 8 }, endian);
        let child_payload_offset = if unaligned_field { 0x2d } else { 0x2c };
        if unaligned_field {
            bytes.push(0xaa);
            push_u32(&mut bytes, child_payload_offset, endian);
            bytes.extend([0xbb; 4]);
        } else {
            push_u32(&mut bytes, child_payload_offset, endian);
            push_u32(&mut bytes, 0, endian);
        }
        push_u32(&mut bytes, 5, endian);
        push_u32(&mut bytes, 1, endian);
        bytes.extend(b"hello");
        bytes
    }

    #[test]
    fn discovers_blocks_then_offsets() {
        let analysis = Analysis::parse(synthetic(Endian::Little, false)).unwrap();
        assert_eq!(analysis.blocks.len(), 2);
        assert_eq!(analysis.blocks[1].payload_offset, 0x2c);
        assert_eq!(
            analysis.blocks[1].text_preview(&analysis.bytes).as_deref(),
            Some("hello")
        );
        assert_eq!(analysis.edges.len(), 1);
        assert_eq!(analysis.edges[0].source_block, BlockId::ROOT);
        assert_eq!(analysis.edges[0].target_block, BlockId::new(1));
        assert_eq!(analysis.edges[0].field_offset, 0);
        assert_eq!(analysis.edges[0].direction, EdgeDirection::Forward);
    }

    #[test]
    fn finds_unaligned_fields_in_big_endian_records() {
        let analysis = Analysis::parse(synthetic(Endian::Big, true)).unwrap();
        assert_eq!(analysis.endian, Endian::Big);
        assert_eq!(analysis.blocks[1].header_offset, 0x25);
        assert_eq!(analysis.blocks[1].payload_offset, 0x2d);
        assert_eq!(analysis.edges.len(), 1);
        assert_eq!(analysis.edges[0].field_offset, 1);
    }

    #[test]
    fn keeps_backward_matches_as_candidates() {
        let mut bytes = synthetic(Endian::Little, false);
        bytes.truncate(0x2c);
        bytes[0x24..0x28].copy_from_slice(&1_u32.to_le_bytes());
        bytes[0x28..0x2c].copy_from_slice(&4_u32.to_le_bytes());
        bytes.extend(0x1c_u32.to_le_bytes());
        let analysis = Analysis::parse(bytes).unwrap();
        assert!(analysis.edges.iter().any(|edge| {
            edge.source_block == BlockId::new(1)
                && edge.target_block == BlockId::ROOT
                && edge.direction == EdgeDirection::BackwardOrSelf
        }));
    }

    #[test]
    fn rejects_a_truncated_payload() {
        let mut bytes = synthetic(Endian::Little, false);
        bytes.pop();
        let error = Analysis::parse(bytes).unwrap_err();
        assert!(error.message.contains("beyond file end"));
    }

    #[test]
    fn every_repository_fixture_forms_a_reachable_tree() {
        let fixture_directory =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../rdm4lib/rdm");
        let mut checked = 0;
        for entry in std::fs::read_dir(fixture_directory).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|value| value.to_str()) != Some("rdm") {
                continue;
            }
            let analysis = Analysis::parse(std::fs::read(&path).unwrap())
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            let forward_edges = analysis
                .edges
                .iter()
                .filter(|edge| edge.direction == EdgeDirection::Forward)
                .count();
            assert_eq!(
                forward_edges,
                analysis.blocks.len() - 1,
                "{}",
                path.display()
            );
            assert!(
                analysis.forward_reachable().into_iter().all(|value| value),
                "{}",
                path.display()
            );
            checked += 1;
        }
        assert!(checked >= 10);
    }
}
