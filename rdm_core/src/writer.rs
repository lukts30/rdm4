//! Encoding of records into RDM bytes.
//!
//! Blocks are appended in the same physical order the reader visits them,
//! replaying preserved strides and unknown bytes.

mod payload;

use crate::codec::{ByteOrder, Number, Value};
use crate::error::{Error, ErrorKind, Result};
use crate::preserved::Preserved;
use crate::records::{FileHeader, HEADER_OFFSET, SIGNATURE};
use crate::schema::{unknown_ranges, RecordLayout};

const BLOCK_HEADER_SIZE: usize = 8;

pub(crate) struct Writer<'p> {
    out: Vec<u8>,
    order: ByteOrder,
    strides: std::slice::Iter<'p, u32>,
    unknown: &'p [u8],
}

/// A record encoded into a fixed-stride slot at output position `pos`.
pub(crate) trait WriteRecord: RecordLayout {
    fn write_record(&self, w: &mut Writer<'_>, pos: usize) -> Result<()>;
}

/// The contents of a referenced block, appended at the end of the output.
pub(crate) trait WriteBlock {
    type Args<'a>;

    fn write_block(
        &self,
        w: &mut Writer<'_>,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<()>;
}

/// The value of a reference field: a required or optional block.
pub(crate) trait WriteRef {
    type Args<'a>;

    fn write_ref(
        &self,
        w: &mut Writer<'_>,
        pos: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<()>;
}

/// Encode a document read by [`crate::reader::read`].
pub(crate) fn write(
    header: &FileHeader,
    order: ByteOrder,
    preserved: &Preserved,
) -> Result<Vec<u8>> {
    let header_end = HEADER_OFFSET + FileHeader::STRIDES[0] as usize;
    let mut out = Vec::with_capacity(header_end);
    out.extend_from_slice(&SIGNATURE);
    out.push(order.signature_byte());
    out.resize(header_end, 0);

    let mut writer = Writer {
        out,
        order,
        strides: preserved.strides.iter(),
        unknown: &preserved.unknown,
    };
    header.write_record(&mut writer, HEADER_OFFSET)?;

    if writer.strides.len() != 0 || !writer.unknown.is_empty() {
        return Err(Error::new(
            writer.out.len(),
            "end of file",
            ErrorKind::LayoutChanged,
        ));
    }
    Ok(writer.out)
}

impl Writer<'_> {
    pub(crate) fn order(&self) -> ByteOrder {
        self.order
    }

    /// Current output length.
    pub(crate) fn position(&self) -> usize {
        self.out.len()
    }

    /// Encode an inline value at `pos`.
    #[inline]
    pub(crate) fn value<T: Value>(&mut self, pos: usize, value: &T) {
        value.encode(&mut self.out[pos..pos + T::SIZE], self.order);
    }

    /// Encode a reference field at `pos` and append the block it points to.
    pub(crate) fn reference<T: WriteRef>(
        &mut self,
        pos: usize,
        value: &T,
        context: &'static str,
        args: T::Args<'_>,
    ) -> Result<()> {
        value.write_ref(self, pos, context, args)
    }

    /// Append a block header and a zeroed payload; return the payload range
    /// start.
    pub(crate) fn begin_block(
        &mut self,
        count: usize,
        stride: usize,
        context: &'static str,
    ) -> Result<usize> {
        let overflow = || Error::new(self.out.len(), context, ErrorKind::Overflow);
        let count_word = u32::try_from(count).map_err(|_| overflow())?;
        let stride_word = u32::try_from(stride).map_err(|_| overflow())?;
        let start = self.out.len() + BLOCK_HEADER_SIZE;
        let end = count
            .checked_mul(stride)
            .and_then(|size| size.checked_add(start))
            .filter(|&end| u32::try_from(end).is_ok())
            .ok_or_else(overflow)?;

        self.out.resize(end, 0);
        count_word.to_bytes(&mut self.out[start - 8..start - 4], self.order);
        stride_word.to_bytes(&mut self.out[start - 4..start], self.order);
        Ok(start)
    }

    pub(crate) fn payload_mut(&mut self, start: usize) -> &mut [u8] {
        &mut self.out[start..]
    }

    /// Store a reference at `pos` to the block about to be appended.
    fn link(&mut self, pos: usize, context: &'static str) -> Result<()> {
        let target = self.out.len() + BLOCK_HEADER_SIZE;
        let target =
            u32::try_from(target).map_err(|_| Error::new(pos, context, ErrorKind::Overflow))?;
        self.value(pos, &target);
        Ok(())
    }

    /// Append a block of records.
    fn records<T: WriteRecord>(&mut self, items: &[T], context: &'static str) -> Result<()> {
        let layout_changed = |w: &Self| Error::new(w.out.len(), context, ErrorKind::LayoutChanged);
        let stride = if T::STRIDES.len() > 1 {
            *self.strides.next().ok_or_else(|| layout_changed(self))?
        } else {
            T::STRIDES[0]
        };
        if !T::STRIDES.contains(&stride) {
            return Err(layout_changed(self));
        }

        let stride = stride as usize;
        let gaps = unknown_ranges(T::FIELDS, stride);
        let start = self.begin_block(items.len(), stride, context)?;
        for (index, item) in items.iter().enumerate() {
            let pos = start + index * stride;
            for gap in &gaps {
                if self.unknown.len() < gap.len() {
                    return Err(layout_changed(self));
                }
                let (bytes, rest) = self.unknown.split_at(gap.len());
                self.out[pos + gap.start..pos + gap.end].copy_from_slice(bytes);
                self.unknown = rest;
            }
            item.write_record(self, pos)?;
        }
        Ok(())
    }
}

impl<T: WriteBlock> WriteRef for T {
    type Args<'a> = T::Args<'a>;

    fn write_ref(
        &self,
        w: &mut Writer<'_>,
        pos: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<()> {
        w.link(pos, context)?;
        self.write_block(w, context, args)
    }
}

impl<T: WriteBlock> WriteRef for Option<T> {
    type Args<'a> = T::Args<'a>;

    fn write_ref(
        &self,
        w: &mut Writer<'_>,
        pos: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<()> {
        match self {
            Some(value) => value.write_ref(w, pos, context, args),
            None => {
                w.value(pos, &0u32);
                Ok(())
            }
        }
    }
}

/// A block holding exactly one record.
impl<T: WriteRecord> WriteBlock for T {
    type Args<'a> = ();

    fn write_block(&self, w: &mut Writer<'_>, context: &'static str, _args: ()) -> Result<()> {
        w.records(std::slice::from_ref(self), context)
    }
}

impl<T: WriteRecord> WriteBlock for Vec<T> {
    type Args<'a> = ();

    fn write_block(&self, w: &mut Writer<'_>, context: &'static str, _args: ()) -> Result<()> {
        w.records(self, context)
    }
}

/// A byte string.
impl WriteBlock for Vec<u8> {
    type Args<'a> = ();

    fn write_block(&self, w: &mut Writer<'_>, context: &'static str, _args: ()) -> Result<()> {
        let start = w.begin_block(self.len(), 1, context)?;
        w.payload_mut(start).copy_from_slice(self);
        Ok(())
    }
}
