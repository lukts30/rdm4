//! Decoding of RDM bytes into records.
//!
//! A single pass visits blocks in physical order. Each reference must point
//! exactly to the next block after everything read so far; this one check
//! rejects gaps, reordering, shared targets, backward references and cycles.

mod payload;

use std::ops::Range;

use crate::codec::{ByteOrder, Number, Value};
use crate::error::{Error, ErrorKind, Result};
use crate::preserved::Preserved;
use crate::records::{FileHeader, HEADER_OFFSET, SIGNATURE};
use crate::schema::{unknown_ranges, RecordLayout};

const BLOCK_HEADER_SIZE: usize = 8;

pub(crate) struct Reader<'d> {
    input: &'d [u8],
    order: ByteOrder,
    /// Where the next block header must start.
    cursor: usize,
    preserved: Preserved,
}

/// One record's bytes and their input offset.
pub(crate) struct RecordAt<'d> {
    bytes: &'d [u8],
    offset: usize,
}

/// A block whose header and payload bounds have been validated.
pub(crate) struct Block<'d> {
    pub(crate) payload: &'d [u8],
    /// Input offset of the first payload byte.
    pub(crate) offset: usize,
    pub(crate) count: usize,
    pub(crate) stride: usize,
    pub(crate) context: &'static str,
}

impl Block<'_> {
    /// An error located at this block's header.
    pub(crate) fn error(&self, kind: ErrorKind) -> Error {
        Error::new(self.offset - BLOCK_HEADER_SIZE, self.context, kind)
    }
}

/// A record decoded from a fixed-stride slot.
pub(crate) trait ReadRecord: RecordLayout + Sized {
    fn read_record<'d>(r: &mut Reader<'d>, rec: RecordAt<'d>) -> Result<Self>;
}

/// The contents of a referenced block.
pub(crate) trait ReadBlock: Sized {
    type Args<'a>;

    fn read_block<'d>(r: &mut Reader<'d>, block: Block<'d>, args: Self::Args<'_>) -> Result<Self>;
}

/// The value of a reference field: a required or optional block.
pub(crate) trait ReadRef: Sized {
    type Args<'a>;

    fn read_ref(
        r: &mut Reader<'_>,
        target: u32,
        field_offset: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<Self>;
}

/// Decode a whole file. Returns the header record, the byte order and the
/// bytes needed to reproduce the input.
pub(crate) fn read(input: &[u8]) -> Result<(FileHeader, ByteOrder, Preserved)> {
    let context = "signature";
    let signature = input.get(..HEADER_OFFSET).ok_or_else(|| {
        let kind = ErrorKind::BadSignature {
            actual: input.to_vec(),
        };
        Error::new(0, context, kind)
    })?;
    let order = ByteOrder::from_signature_byte(signature[3]);
    let order = match order {
        Some(order) if signature[..3] == SIGNATURE => order,
        _ => {
            let kind = ErrorKind::BadSignature {
                actual: signature.to_vec(),
            };
            return Err(Error::new(0, context, kind));
        }
    };

    let header_end = HEADER_OFFSET + FileHeader::STRIDES[0] as usize;
    let header = input.get(HEADER_OFFSET..header_end).ok_or_else(|| {
        let kind = ErrorKind::OutOfBounds {
            end: header_end as u64,
            len: input.len() as u64,
        };
        Error::new(HEADER_OFFSET, FileHeader::NAME, kind)
    })?;

    let mut reader = Reader {
        input,
        order,
        cursor: header_end,
        preserved: Preserved::default(),
    };
    let rec = RecordAt {
        bytes: header,
        offset: HEADER_OFFSET,
    };
    let header = FileHeader::read_record(&mut reader, rec)?;

    if reader.cursor != input.len() {
        let kind = ErrorKind::TrailingBytes {
            len: (input.len() - reader.cursor) as u64,
        };
        return Err(Error::new(reader.cursor, "end of file", kind));
    }

    let mut preserved = reader.preserved;
    preserved.shrink_to_fit();
    log::debug!(
        "read {} bytes ({order:?}); preserved {} unknown bytes",
        input.len(),
        preserved.unknown.len()
    );
    Ok((header, order, preserved))
}

impl<'d> Reader<'d> {
    pub(crate) fn order(&self) -> ByteOrder {
        self.order
    }

    /// Decode an inline value of a record.
    #[inline]
    pub(crate) fn value<T: Value>(
        &self,
        rec: &RecordAt<'d>,
        offset: usize,
        context: &'static str,
    ) -> Result<T> {
        let bytes = &rec.bytes[offset..offset + T::SIZE];
        T::decode(bytes, self.order).map_err(|kind| Error::new(rec.offset + offset, context, kind))
    }

    /// Decode a reference field and the block it points to.
    pub(crate) fn reference<T: ReadRef>(
        &mut self,
        rec: &RecordAt<'d>,
        offset: usize,
        context: &'static str,
        args: T::Args<'_>,
    ) -> Result<T> {
        let target = self.value::<u32>(rec, offset, context)?;
        T::read_ref(self, target, rec.offset + offset, context, args)
    }

    /// Validate the block a non-null reference points to and claim its bytes.
    fn enter_block(
        &mut self,
        target: u32,
        field_offset: usize,
        context: &'static str,
    ) -> Result<Block<'d>> {
        let target = target as usize;
        let expected = self.cursor + BLOCK_HEADER_SIZE;
        if target != expected {
            let kind = ErrorKind::UnexpectedTarget {
                expected: expected as u64,
                actual: target as u64,
            };
            return Err(Error::new(field_offset, context, kind));
        }

        let out_of_bounds = |end: usize| {
            let kind = ErrorKind::OutOfBounds {
                end: end as u64,
                len: self.input.len() as u64,
            };
            Error::new(self.cursor, context, kind)
        };
        let header = self
            .input
            .get(self.cursor..target)
            .ok_or_else(|| out_of_bounds(target))?;
        let count = u32::from_bytes(&header[..4], self.order) as usize;
        let stride = u32::from_bytes(&header[4..], self.order) as usize;
        let end = count
            .checked_mul(stride)
            .and_then(|size| size.checked_add(target))
            .ok_or_else(|| Error::new(self.cursor, context, ErrorKind::Overflow))?;
        let payload = self
            .input
            .get(target..end)
            .ok_or_else(|| out_of_bounds(end))?;

        log::trace!("{context}: block at {target:#x}, {count} x {stride} bytes");
        self.cursor = end;
        Ok(Block {
            payload,
            offset: target,
            count,
            stride,
            context,
        })
    }

    /// Check a record block's stride and find the bytes its fields leave
    /// unknown.
    fn record_gaps<T: ReadRecord>(&mut self, block: &Block<'d>) -> Result<Vec<Range<usize>>> {
        let stride = block.stride as u32;
        if !T::STRIDES.contains(&stride) {
            let kind = ErrorKind::UnsupportedStride {
                expected: T::STRIDES,
                actual: stride,
            };
            return Err(block.error(kind));
        }
        if T::STRIDES.len() > 1 {
            self.preserved.strides.push(stride);
        }
        Ok(unknown_ranges(T::FIELDS, block.stride))
    }

    fn record<T: ReadRecord>(
        &mut self,
        block: &Block<'d>,
        index: usize,
        gaps: &[Range<usize>],
    ) -> Result<T> {
        let start = index * block.stride;
        let bytes = &block.payload[start..start + block.stride];
        for gap in gaps {
            self.preserved
                .unknown
                .extend_from_slice(&bytes[gap.clone()]);
        }
        let rec = RecordAt {
            bytes,
            offset: block.offset + start,
        };
        T::read_record(self, rec)
    }
}

impl<T: ReadBlock> ReadRef for T {
    type Args<'a> = T::Args<'a>;

    fn read_ref(
        r: &mut Reader<'_>,
        target: u32,
        field_offset: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<Self> {
        if target == 0 {
            return Err(Error::new(field_offset, context, ErrorKind::NullReference));
        }
        let block = r.enter_block(target, field_offset, context)?;
        T::read_block(r, block, args)
    }
}

impl<T: ReadBlock> ReadRef for Option<T> {
    type Args<'a> = T::Args<'a>;

    fn read_ref(
        r: &mut Reader<'_>,
        target: u32,
        field_offset: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<Self> {
        if target == 0 {
            return Ok(None);
        }
        T::read_ref(r, target, field_offset, context, args).map(Some)
    }
}

/// A block holding exactly one record.
impl<T: ReadRecord> ReadBlock for T {
    type Args<'a> = ();

    fn read_block<'d>(r: &mut Reader<'d>, block: Block<'d>, _args: ()) -> Result<Self> {
        if block.count != 1 {
            let kind = ErrorKind::CountMismatch {
                expected: 1,
                actual: block.count as u64,
            };
            return Err(block.error(kind));
        }
        let gaps = r.record_gaps::<T>(&block)?;
        r.record(&block, 0, &gaps)
    }
}

impl<T: ReadRecord> ReadBlock for Vec<T> {
    type Args<'a> = ();

    fn read_block<'d>(r: &mut Reader<'d>, block: Block<'d>, _args: ()) -> Result<Self> {
        let gaps = r.record_gaps::<T>(&block)?;
        let mut items = Vec::with_capacity(block.count);
        for index in 0..block.count {
            items.push(r.record(&block, index, &gaps)?);
        }
        Ok(items)
    }
}

/// A byte string.
impl ReadBlock for Vec<u8> {
    type Args<'a> = ();

    fn read_block<'d>(_r: &mut Reader<'d>, block: Block<'d>, _args: ()) -> Result<Self> {
        if block.stride != 1 {
            let kind = ErrorKind::StrideMismatch {
                expected: 1,
                actual: block.stride as u64,
            };
            return Err(block.error(kind));
        }
        Ok(block.payload.to_vec())
    }
}
