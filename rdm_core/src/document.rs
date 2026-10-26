//! The public document abstraction.

use std::error::Error as StdError;
use std::path::Path;

use crate::codec::ByteOrder;
use crate::error::Error;
use crate::preserved::Preserved;
use crate::records::{FileHeader, Root};
use crate::{reader, writer};

/// A decoded RDM file.
///
/// Holds the typed records plus private data needed to reproduce the input
/// bytes: strides of variable-width records and bytes outside known fields.
/// Values may be edited in place; adding or removing records is not
/// supported and is reported when writing.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    header: FileHeader,
    byte_order: ByteOrder,
    preserved: Preserved,
}

impl Document {
    /// Decode and validate an RDM file held in memory.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let (header, byte_order, preserved) = reader::read(bytes)?;
        Ok(Document {
            header,
            byte_order,
            preserved,
        })
    }

    /// Read and decode an RDM file. The file buffer is released before
    /// returning.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Box<dyn StdError>> {
        let bytes = std::fs::read(path)?;
        Ok(Self::from_bytes(&bytes)?)
    }

    pub fn root(&self) -> &Root {
        &self.header.root
    }

    pub fn root_mut(&mut self) -> &mut Root {
        &mut self.header.root
    }

    /// Byte order of the source file, used again when writing.
    pub fn byte_order(&self) -> ByteOrder {
        self.byte_order
    }

    /// Encode the document. Unchanged documents reproduce their input.
    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        writer::write(&self.header, self.byte_order, &self.preserved)
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn StdError>> {
        std::fs::write(path, self.to_bytes()?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Re-encoding with the other byte order is only sound when no unknown
    /// bytes are preserved, because their word structure is unknown.
    #[test]
    fn decodes_both_byte_orders_to_the_same_records() {
        let little = include_bytes!("../../rdm4lib/rdm/fishery_others_lod2.rdm");
        let document = Document::from_bytes(little).unwrap();
        assert!(document.preserved.unknown.iter().all(|&byte| byte == 0));

        let mut swapped = document.clone();
        swapped.byte_order = ByteOrder::Big;
        let big = swapped.to_bytes().unwrap();
        assert_eq!(&big[..4], b"RDM\x00");
        assert_ne!(&big[..], &little[..]);

        let decoded = Document::from_bytes(&big).unwrap();
        assert_eq!(decoded.byte_order(), ByteOrder::Big);
        assert_eq!(decoded.root(), document.root());
        assert_eq!(decoded.to_bytes().unwrap(), big);
    }
}
