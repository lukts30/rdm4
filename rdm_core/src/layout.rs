//! Private bookkeeping that the typed records cannot carry.

/// Reconstruction data for one block, mirroring the reference tree.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BlockLayout {
    pub stride: u32,
    /// Bytes after each record's known fields, concatenated over all records.
    /// `None` when every such byte is zero.
    pub unknown: Option<Box<[u8]>>,
    /// Child blocks of all records, in record and field order. Null offsets
    /// have no entry.
    pub children: Vec<BlockLayout>,
    /// Indices into `children` in file order; `None` when it equals field order.
    pub physical_order: Option<Box<[u32]>>,
}

impl BlockLayout {
    /// A block without unknown bytes or children, such as a numeric payload.
    pub(crate) fn leaf(stride: u32) -> Self {
        BlockLayout {
            stride,
            unknown: None,
            children: Vec::new(),
            physical_order: None,
        }
    }

    /// Unknown bytes of record `index`, or `None` if they are all zero.
    pub(crate) fn unknown_of(&self, index: usize, known: u32) -> Option<&[u8]> {
        let width = (self.stride - known) as usize;
        let unknown = self.unknown.as_deref()?;
        Some(&unknown[index * width..(index + 1) * width])
    }

    /// Children in the order they appear in the file.
    pub(crate) fn emission_order(&self) -> Vec<usize> {
        match &self.physical_order {
            Some(order) => order.iter().map(|&index| index as usize).collect(),
            None => (0..self.children.len()).collect(),
        }
    }
}
