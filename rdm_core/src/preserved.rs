//! Private bookkeeping for byte-identical round trips.

/// Data the records do not carry, in traversal order.
///
/// The reader appends while visiting blocks in physical order; the writer
/// consumes in the same order. Structural edits to a document would
/// desynchronize the two and are reported as [`LayoutChanged`].
///
/// [`LayoutChanged`]: crate::ErrorKind::LayoutChanged
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Preserved {
    /// Stride of each block whose record type allows several strides.
    pub(crate) strides: Vec<u32>,
    /// Record bytes not covered by known fields.
    pub(crate) unknown: Vec<u8>,
}

impl Preserved {
    pub(crate) fn shrink_to_fit(&mut self) {
        self.strides.shrink_to_fit();
        self.unknown.shrink_to_fit();
    }
}
