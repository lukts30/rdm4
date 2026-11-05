//! Record-prefix writing adapted from `feat/rdm-core-derive-reader`.

use std::io::{Seek, SeekFrom, Write};

use super::{wire::Scalar, RdmEndian, RdmError, RdmFieldLayout, RdmRecordLayout};

/// Writes known record prefixes at absolute output offsets.
/// File framing, references and opaque tails are not written by this low-level API.
pub struct RdmWriter<W> {
    target: W,
    endian: RdmEndian,
}

impl<W: Write + Seek> RdmWriter<W> {
    /// Wrap a seekable target with the byte order used for numeric fields.
    pub fn new(target: W, endian: RdmEndian) -> Self {
        Self { target, endian }
    }

    /// Byte order used by field encoders.
    pub fn endian(&self) -> RdmEndian {
        self.endian
    }

    /// Recover the target after writing; its position is unspecified.
    pub fn into_inner(self) -> W {
        self.target
    }

    /// Write a fixed record prefix. Intended for `RdmWriteRecord` implementations.
    pub fn record_bytes(
        &mut self,
        at: usize,
        bytes: &[u8],
        context: &'static str,
    ) -> Result<(), RdmError> {
        self.target
            .seek(SeekFrom::Start(at as u64))
            .and_then(|_| self.target.write_all(bytes))
            .map_err(|error| RdmError::new(at, context, "writable record bytes", error))
    }
}

/// Encode a record's known fields, leaving any trailing bytes untouched.
pub trait RdmWriteRecord: RdmRecordLayout {
    /// Write a prefix at an absolute offset; enclosing blocks own the tails.
    fn write<W: Write + Seek>(&self, writer: &mut RdmWriter<W>, at: usize) -> Result<(), RdmError>;
}

/// Encode a field with borrowed sibling context, independently of output storage.
pub trait RdmWriteField: RdmFieldLayout {
    /// Context needed by this field, such as a borrowed vertex format.
    type Args<'a>;

    /// Encode into a bounded field slice at its absolute output offset.
    fn write_field<W: Write + Seek>(
        &self,
        writer: &mut RdmWriter<W>,
        raw: &mut [u8],
        at: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<(), RdmError>;
}

impl<T: Scalar> RdmWriteField for T {
    type Args<'a> = ();

    fn write_field<W: Write + Seek>(
        &self,
        writer: &mut RdmWriter<W>,
        raw: &mut [u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<(), RdmError> {
        if raw.len() != T::WIDTH {
            return Err(RdmError::new(at, context, T::WIDTH, raw.len()));
        }
        writer.endian().encode(*self, raw);
        Ok(())
    }
}
