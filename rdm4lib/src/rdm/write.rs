//! Record writing adapted from `feat/rdm-core-derive-reader`.

use std::io::{self, Read, Seek, SeekFrom, Write};

use super::{
    wire::Scalar, RdmBlock, RdmEndian, RdmError, RdmFieldLayout, RdmHeader, RdmRecordLayout,
};
use crate::vertex::{StorageType, VertexInterpretation, VertexSemantic};

/// Writes known prefixes and appends referenced blocks to a seekable target.
/// New blocks use the first declared stride with zero-filled tails, not source tails.
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

    /// Write a preamble and typed root to an empty target, returning the target.
    /// Errors may leave partial output; this does not preserve an input's layout.
    pub fn write_root<T: RdmWriteRecord>(mut self, value: &T) -> Result<W, RdmError> {
        let end = self
            .target
            .seek(SeekFrom::End(0))
            .map_err(|error| RdmError::new(0, "output", "seekable output", error))?;
        if end != 0 {
            return Err(RdmError::new(0, "output", "empty output", end));
        }
        self.record_bytes(
            0,
            &RdmHeader {
                endian: self.endian,
            }
            .encode(),
            "preamble",
        )?;
        let mut raw = [0; 4];
        self.write_single(value, &mut raw, 0x10, "RdmHeader.root")?;
        self.record_bytes(0x10, &raw, "RdmHeader.root")?;
        Ok(self.target)
    }

    /// Append a singleton block and encode its reference into the enclosing prefix.
    pub fn write_single<T: RdmWriteRecord>(
        &mut self,
        value: &T,
        raw: &mut [u8],
        at: usize,
        context: &'static str,
    ) -> Result<(), RdmError> {
        self.write_records(std::slice::from_ref(value), raw, at, context)
    }

    fn write_records<T: RdmWriteRecord>(
        &mut self,
        values: &[T],
        raw: &mut [u8],
        at: usize,
        context: &'static str,
    ) -> Result<(), RdmError> {
        let stride = T::STRIDES
            .first()
            .copied()
            .filter(|&stride| stride != 0 && stride as usize >= T::SIZE)
            .ok_or_else(|| {
                RdmError::new(
                    at,
                    context,
                    format!("nonzero stride covering {} known bytes", T::SIZE),
                    format!("{:?}", T::STRIDES),
                )
            })?;
        let start = self.write_reference(raw, at, context, values.len(), stride)?;
        // Reserve the entire list before following any child's references.
        for (i, value) in values.iter().enumerate() {
            value.write(self, start + i * stride as usize)?;
        }
        Ok(())
    }

    fn write_reference(
        &mut self,
        raw: &mut [u8],
        at: usize,
        context: &'static str,
        count: usize,
        stride: u32,
    ) -> Result<usize, RdmError> {
        if raw.len() != 4 {
            return Err(RdmError::new(at, context, 4, raw.len()));
        }
        let count = u32::try_from(count)
            .ok()
            .filter(|&count| count != 0)
            .ok_or_else(|| RdmError::new(at, context, "nonzero count fitting u32", count))?;
        let offset = self
            .target
            .seek(SeekFrom::End(0))
            .map_err(|error| RdmError::new(at, context, "seekable output", error))?;
        if (at as u64).checked_add(4).is_none_or(|end| end > offset) {
            return Err(RdmError::new(
                at,
                context,
                "reference inside reserved output",
                offset,
            ));
        }
        let start = offset
            .checked_add(RdmBlock::HEADER_SIZE as u64)
            .and_then(|start| u32::try_from(start).ok())
            .ok_or_else(|| RdmError::new(at, context, "block offset fitting u32", offset))?;
        let size = u64::from(count) * u64::from(stride);
        if u64::from(start) + size > u64::from(u32::MAX) {
            return Err(RdmError::new(
                at,
                context,
                "block end fitting u32",
                u64::from(start) + size,
            ));
        }
        let mut header = [0; RdmBlock::HEADER_SIZE];
        self.endian.encode([count, stride], &mut header);
        self.target
            .write_all(&header)
            .and_then(|_| io::copy(&mut io::repeat(0).take(size), &mut self.target).map(|_| ()))
            .map_err(|error| {
                RdmError::new(offset as usize, context, "writable block bytes", error)
            })?;
        self.endian.encode(start, raw);
        Ok(start as usize)
    }

    pub(super) fn write_payload(
        &mut self,
        raw: &mut [u8],
        at: usize,
        context: &'static str,
        count: usize,
        stride: u32,
        bytes: &[u8],
    ) -> Result<(), RdmError> {
        let start = self.write_reference(raw, at, context, count, stride)?;
        self.record_bytes(start, bytes, context)
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

impl<T: RdmWriteRecord> RdmWriteField for Vec<T> {
    type Args<'a> = ();

    fn write_field<W: Write + Seek>(
        &self,
        writer: &mut RdmWriter<W>,
        raw: &mut [u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<(), RdmError> {
        writer.write_records(self, raw, at, context)
    }
}

impl RdmWriteField for String {
    type Args<'a> = ();

    fn write_field<W: Write + Seek>(
        &self,
        writer: &mut RdmWriter<W>,
        raw: &mut [u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<(), RdmError> {
        let start = writer.write_reference(raw, at, context, self.len(), 1)?;
        writer.record_bytes(start, self.as_bytes(), context)
    }
}

impl<T: RdmWriteField> RdmWriteField for Option<T> {
    type Args<'a> = T::Args<'a>;

    fn write_field<W: Write + Seek>(
        &self,
        writer: &mut RdmWriter<W>,
        raw: &mut [u8],
        at: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<(), RdmError> {
        match self {
            Some(value) => value.write_field(writer, raw, at, context, args),
            None => 0u32.write_field(writer, raw, at, context, ()),
        }
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

impl RdmWriteField for StorageType {
    type Args<'a> = ();

    fn write_field<W: Write + Seek>(
        &self,
        writer: &mut RdmWriter<W>,
        raw: &mut [u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<(), RdmError> {
        (*self as u32).write_field(writer, raw, at, context, ())
    }
}

impl RdmWriteField for VertexSemantic {
    type Args<'a> = ();

    fn write_field<W: Write + Seek>(
        &self,
        writer: &mut RdmWriter<W>,
        raw: &mut [u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<(), RdmError> {
        (self.clone() as u32).write_field(writer, raw, at, context, ())
    }
}

impl RdmWriteField for VertexInterpretation {
    type Args<'a> = ();

    fn write_field<W: Write + Seek>(
        &self,
        writer: &mut RdmWriter<W>,
        raw: &mut [u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<(), RdmError> {
        self.0.write_field(writer, raw, at, context, ())
    }
}
