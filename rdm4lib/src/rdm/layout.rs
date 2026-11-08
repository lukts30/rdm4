use super::{RdmBlockInfo, RdmEndian};

/// Owned value and its original codec metadata, kept separate from the records.
#[derive(Debug, Clone, PartialEq)]
pub struct RdmReadResult<T> {
    /// Decoded records, independent of byte order and source storage.
    pub value: T,
    /// Original placements, reference origins and undecoded record tails.
    pub layout: RdmSourceLayout,
}

/// Original file layout after every physical block has been claimed.
/// Complete reference coverage alone does not establish endian-conversion safety.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdmSourceLayout {
    /// Original byte order, including the order of opaque data.
    pub endian: RdmEndian,
    /// Decoded blocks in physical file order.
    pub blocks: Vec<RdmBlockLayout>,
}

/// Provenance and preservation data for one referenced block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdmBlockLayout {
    /// Original physical boundaries, count and stride.
    pub source: RdmBlockInfo,
    /// Absolute offset of the incoming reference field.
    pub reference_offset: usize,
    /// Name of the field that owns this block.
    pub context: &'static str,
    /// Bytes understood per record, excluding the preserved tail.
    pub known_size: usize,
    /// Undecoded bytes in record order, including zeros; not assumed to be padding.
    pub tails: Vec<u8>,
}
