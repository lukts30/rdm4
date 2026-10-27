use std::{fs, io, path::Path};

use crate::records::Root;
use crate::wire::Endian;
use crate::{read, write, Error};

/// A fully decoded RDM file.
///
/// Writing an unchanged document reproduces the input bytes. Values may be
/// edited in place; adding or removing records makes writing fail, because
/// unknown bytes are kept per record in reading order.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub endian: Endian,
    pub root: Root,
    pub(crate) preserved: Preserved,
}

/// Data needed to reproduce bytes that records do not decode.
///
/// Both lists are in traversal order: `strides` has one entry per block of a
/// record type with several accepted strides, `tails` the unknown bytes after
/// each record's known fields.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Preserved {
    pub(crate) strides: Vec<u32>,
    pub(crate) tails: Vec<u8>,
}

impl Document {
    pub fn from_bytes(input: &[u8]) -> Result<Self, Error> {
        read::read_document(input)
    }

    /// Reads and decodes a file. The file contents are released before returning.
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let input = fs::read(path)?;
        Self::from_bytes(&input).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        write::write_document(self)
    }

    /// Heap bytes held for reproducing undecoded data.
    pub fn preserved_len(&self) -> usize {
        self.preserved.tails.capacity() + self.preserved.strides.capacity() * 4
    }
}
