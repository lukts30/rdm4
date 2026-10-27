use std::path::Path;

use crate::byte_order::ByteOrder;
use crate::error::Error;
use crate::layout::BlockLayout;
use crate::records::Root;
use crate::{read, write};

/// A decoded RDM file that can be written back unchanged.
///
/// Records are fully decoded; bytes the records do not model (unknown
/// record tails, block order, strides) are kept privately for writing.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    root: Root,
    byte_order: ByteOrder,
    layout: BlockLayout,
}

impl Document {
    /// Decodes and validates a complete RDM file.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let output = read::read(bytes)?;
        Ok(Document {
            root: output.root,
            byte_order: output.byte_order,
            layout: output.layout,
        })
    }

    /// Reads a file; the input buffer is released before returning.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(path)?;
        Ok(Self::from_bytes(&bytes)?)
    }

    pub fn root(&self) -> &Root {
        &self.root
    }

    pub fn into_root(self) -> Root {
        self.root
    }

    /// Byte order of the source file, which writing reproduces.
    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    /// Encodes the document; identical to the input it was read from.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        write::write(&self.root, self.byte_order, &self.layout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(name: &str) -> Document {
        Document::open(format!("../rdm4lib/rdm/{name}")).unwrap()
    }

    #[test]
    fn records_model_block_order_differing_from_field_order() {
        let document = open("fishery_others_cutout_lod0.rdm");
        let models = &document.layout.children[1];
        // Fields: name, vertex_format, index_format, vertices, indices, groups.
        let order = models.physical_order.as_deref();
        assert_eq!(order, Some([0, 1, 2, 5, 3, 4].as_slice()));
        assert!(document.layout.physical_order.is_none());
    }

    #[test]
    fn reads_52_byte_root_records() {
        let mut document = open("basalt_crusher_others_lod0.rdm");
        assert_eq!(document.layout.stride, 48);
        document.layout.stride = 52;
        let bytes = document.to_bytes().unwrap();

        let widened = Document::from_bytes(&bytes).unwrap();
        assert_eq!(widened.layout.stride, 52);
        assert_eq!(widened.root(), document.root());
        assert_eq!(widened.to_bytes().unwrap(), bytes);
    }
}
