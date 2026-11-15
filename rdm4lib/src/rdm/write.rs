//! Record writing adapted from `feat/rdm-core-derive-reader`.

use std::{
    collections::BTreeMap,
    io::{self, Read, Seek, SeekFrom, Write},
};

use super::{
    wire::Scalar, RdmBlock, RdmBlockInfo, RdmBlockLayout, RdmEndian, RdmError, RdmFieldLayout,
    RdmHeader, RdmRecordLayout, RdmSourceLayout,
};
use crate::vertex::{StorageType, VertexInterpretation, VertexSemantic};

/// Writes known prefixes and appends referenced blocks to a seekable target.
/// Fresh blocks use the first declared stride and zero-filled tails.
/// `write_preserving` instead retains the original placements, strides and tail bytes.
pub struct RdmWriter<W> {
    target: W,
    endian: RdmEndian,
    preserved: Option<BTreeMap<usize, RdmBlockLayout>>,
}

impl<W: Write + Seek> RdmWriter<W> {
    /// Wrap a seekable target with the byte order used for numeric fields.
    pub fn new(target: W, endian: RdmEndian) -> Self {
        Self {
            target,
            endian,
            preserved: None,
        }
    }

    /// Byte order used by field encoders.
    pub fn endian(&self) -> RdmEndian {
        self.endian
    }

    /// Recover the target after writing; its position is unspecified.
    pub fn into_inner(self) -> W {
        self.target
    }

    /// Write fresh records to an empty target using each type's first declared stride.
    /// Tails are zero-filled; errors may leave partial output.
    pub fn write_root<T: RdmWriteRecord>(mut self, value: &T) -> Result<W, RdmError> {
        self.begin()?;
        self.finish_root(value)
    }

    /// Rewrite records with a complete reader layout, retaining all source placements and tails.
    /// Byte order, block counts, string lengths and buffer strides must stay unchanged.
    /// Known values may change. Errors may leave partial output in the initially empty target.
    pub fn write_preserving<T: RdmWriteRecord>(
        mut self,
        value: &T,
        layout: RdmSourceLayout,
    ) -> Result<W, RdmError> {
        if self.endian != layout.endian {
            return Err(RdmError::new(
                3,
                "source byte order",
                format!("{:?}", layout.endian),
                format!("{:?}", self.endian),
            ));
        }
        self.begin()?;
        let mut preserved = BTreeMap::new();
        let mut next = RdmHeader::SIZE;
        for block in layout.blocks {
            let source = &block.source;
            let mut header = [0; RdmBlock::HEADER_SIZE];
            self.endian
                .encode([source.count, source.stride], &mut header);
            let expected =
                RdmBlockInfo::from_header(&header, next, u32::MAX as usize, self.endian)?;
            if *source != expected {
                return Err(RdmError::new(
                    source.offset,
                    block.context,
                    "contiguous source block boundaries",
                    format!("{source:?}"),
                ));
            }
            let tail_size = (source.stride as usize)
                .checked_sub(block.known_size)
                .ok_or_else(|| {
                    RdmError::new(
                        source.offset,
                        block.context,
                        "known prefix fitting source stride",
                        block.known_size,
                    )
                })?;
            let tail_len = tail_size * source.count as usize;
            if block.tails.len() != tail_len {
                return Err(RdmError::new(
                    source.offset,
                    block.context,
                    format!("{tail_len} preserved tail bytes"),
                    block.tails.len(),
                ));
            }
            self.write_reference(
                &mut [0; 4],
                block.reference_offset,
                block.context,
                source.count as usize,
                source.stride,
            )?;
            if tail_size != 0 {
                for (i, tail) in block.tails.chunks_exact(tail_size).enumerate() {
                    self.record_bytes(
                        source.payload.start + i * source.stride as usize + block.known_size,
                        tail,
                        block.context,
                    )?;
                }
            }
            next = source.payload.end;
            let at = block.reference_offset;
            let context = block.context;
            if preserved.insert(at, block).is_some() {
                return Err(RdmError::new(
                    at,
                    context,
                    "unique source reference",
                    "duplicate reference",
                ));
            }
        }
        self.preserved = Some(preserved);
        self.finish_root(value)
    }

    fn begin(&mut self) -> Result<(), RdmError> {
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
        )
    }

    fn finish_root<T: RdmWriteRecord>(mut self, value: &T) -> Result<W, RdmError> {
        let mut raw = [0; 4];
        self.write_single(value, &mut raw, 0x10, "RdmHeader.root")?;
        self.record_bytes(0x10, &raw, "RdmHeader.root")?;
        if let Some(block) = self
            .preserved
            .as_ref()
            .and_then(|blocks| blocks.values().next())
        {
            return Err(RdmError::new(
                block.reference_offset,
                block.context,
                "all source blocks retained",
                "source reference was removed",
            ));
        }
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
        let stride = self
            .preserved
            .as_ref()
            .and_then(|blocks| blocks.get(&at))
            .map(|block| block.source.stride)
            .or_else(|| T::STRIDES.first().copied())
            .filter(|&stride| {
                stride != 0 && stride as usize >= T::SIZE && T::STRIDES.contains(&stride)
            })
            .ok_or_else(|| {
                RdmError::new(
                    at,
                    context,
                    format!("nonzero stride covering {} known bytes", T::SIZE),
                    format!("{:?}", T::STRIDES),
                )
            })?;
        self.check_known_size(at, context, T::SIZE)?;
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
        if let Some(blocks) = &mut self.preserved {
            let block = blocks.remove(&at).ok_or_else(|| {
                RdmError::new(at, context, "existing source reference", "new reference")
            })?;
            if (block.context, block.source.count, block.source.stride) != (context, count, stride)
            {
                return Err(RdmError::new(
                    at,
                    context,
                    format!(
                        "{}: {} records, stride {}",
                        block.context, block.source.count, block.source.stride
                    ),
                    format!("{context}: {count} records, stride {stride}"),
                ));
            }
            self.endian.encode(block.source.payload.start as u32, raw);
            return Ok(block.source.payload.start);
        }
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
        self.check_known_size(at, context, stride as usize)?;
        let start = self.write_reference(raw, at, context, count, stride)?;
        self.record_bytes(start, bytes, context)
    }

    fn check_known_size(
        &self,
        at: usize,
        context: &'static str,
        size: usize,
    ) -> Result<(), RdmError> {
        if let Some(block) = self.preserved.as_ref().and_then(|blocks| blocks.get(&at)) {
            if block.known_size != size {
                return Err(RdmError::new(
                    at,
                    context,
                    format!("{} known bytes per source record", block.known_size),
                    size,
                ));
            }
        }
        Ok(())
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
        writer.write_payload(raw, at, context, self.len(), 1, self.as_bytes())
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
