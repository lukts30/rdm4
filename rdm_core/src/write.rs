//! Writer producing blocks in the same preorder the reader accepts.
//!
//! Each block is appended with zeroed reference slots. A child block is
//! appended when its reference field is written, and the slot is then
//! patched with the child's payload offset. Unknown record bytes and
//! variable strides come from the document's preserved data, consumed in
//! the order the reader recorded them.

mod indices;
mod vertices;

use std::marker::PhantomData;

use crate::document::{Document, Preserved};
use crate::records::{StorageType, VertexSemantic};
use crate::wire::{
    BigEndian, ByteOrder, Endian, LittleEndian, RecordLayout, Scalar, BLOCK_HEADER_SIZE,
    FIRST_BLOCK_AT, MAGIC, PREAMBLE_WORDS, ROOT_REFERENCE_AT,
};
use crate::Error;

pub(crate) struct Writer<'p, E> {
    out: Vec<u8>,
    preserved: &'p Preserved,
    next_stride: usize,
    next_tail: usize,
    byte_order: PhantomData<E>,
}

/// A record encoded into the `RecordLayout::SIZE` bytes at `at`.
pub(crate) trait WriteRecord: RecordLayout {
    fn write<E: ByteOrder>(&self, writer: &mut Writer<'_, E>, at: usize) -> Result<(), Error>;
}

/// A field encoded into its slot at output offset `at`.
pub(crate) trait WriteField {
    type Args<'a>;

    fn write_field<E: ByteOrder>(
        &self,
        writer: &mut Writer<'_, E>,
        at: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<(), Error>;
}

pub(crate) fn write_document(document: &Document) -> Result<Vec<u8>, Error> {
    let out = match document.endian {
        Endian::Little => write_root::<LittleEndian>(document)?,
        Endian::Big => write_root::<BigEndian>(document)?,
    };
    log::debug!("wrote {} bytes ({:?})", out.len(), document.endian);
    Ok(out)
}

fn write_root<E: ByteOrder>(document: &Document) -> Result<Vec<u8>, Error> {
    let mut writer = Writer {
        out: Vec::new(),
        preserved: &document.preserved,
        next_stride: 0,
        next_tail: 0,
        byte_order: PhantomData::<E>,
    };
    writer.out.extend_from_slice(&MAGIC);
    writer.out.push(document.endian.magic_byte());
    writer.out.resize(FIRST_BLOCK_AT, 0);
    for (at, word) in PREAMBLE_WORDS {
        writer.put(at, word);
    }

    document
        .root
        .write_field(&mut writer, ROOT_REFERENCE_AT, "Header.root", ())?;

    let preserved = writer.preserved;
    if writer.next_stride != preserved.strides.len() || writer.next_tail != preserved.tails.len() {
        return Err(Error::new(
            writer.out.len(),
            "Document",
            "the records that were read",
            "fewer records; preserved unknown bytes remain unused",
        ));
    }
    Ok(writer.out)
}

impl<E: ByteOrder> Writer<'_, E> {
    pub(crate) fn put<T: Scalar>(&mut self, at: usize, value: T) {
        value.encode::<E>(&mut self.out[at..at + T::WIDTH]);
    }

    pub(crate) fn bytes_mut(&mut self, at: usize, len: usize) -> &mut [u8] {
        &mut self.out[at..at + len]
    }

    /// Appends a zeroed block and returns its payload offset.
    pub(crate) fn begin_block(
        &mut self,
        count: usize,
        stride: usize,
        context: &'static str,
    ) -> Result<usize, Error> {
        let header = self.out.len();
        let payload = header + BLOCK_HEADER_SIZE;
        let end = count
            .checked_mul(stride)
            .and_then(|size| payload.checked_add(size));
        let fits = |value: usize| u32::try_from(value).is_ok();
        let (Some(end), true, true) = (end, fits(count), fits(stride)) else {
            return Err(Error::new(
                header,
                context,
                "block fitting 32-bit offsets",
                format!("{count} records of {stride} bytes"),
            ));
        };
        if !fits(end) {
            return Err(Error::new(
                header,
                context,
                "file below 4 GiB",
                format!("{end:#x} bytes"),
            ));
        }

        self.out.resize(end, 0);
        self.put(header, count as u32);
        self.put(header + 4, stride as u32);
        Ok(payload)
    }

    pub(crate) fn put_reference(&mut self, at: usize, payload: usize) {
        self.put(at, payload as u32);
    }

    fn next_stride<T: RecordLayout>(&mut self, context: &'static str) -> Result<usize, Error> {
        if let [stride] = T::STRIDES {
            return Ok(*stride as usize);
        }
        let Some(&stride) = self.preserved.strides.get(self.next_stride) else {
            return Err(Error::new(
                self.out.len(),
                context,
                format!("a preserved {} stride", T::NAME),
                "more blocks than were read",
            ));
        };
        self.next_stride += 1;
        Ok(stride as usize)
    }

    /// Copies the preserved unknown bytes of `count` records into a block.
    fn put_tails<T: RecordLayout>(
        &mut self,
        payload: usize,
        count: usize,
        stride: usize,
        context: &'static str,
    ) -> Result<(), Error> {
        let tail_len = stride - T::SIZE;
        if tail_len == 0 {
            return Ok(());
        }
        let start = self.next_tail;
        let Some(tails) = self.preserved.tails.get(start..start + count * tail_len) else {
            return Err(Error::new(
                payload,
                context,
                format!("{tail_len} preserved unknown bytes per {} record", T::NAME),
                "more records than were read",
            ));
        };
        let block = &mut self.out[payload..payload + count * stride];
        for (record, tail) in block
            .chunks_exact_mut(stride)
            .zip(tails.chunks_exact(tail_len))
        {
            record[T::SIZE..].copy_from_slice(tail);
        }
        self.next_tail += count * tail_len;
        Ok(())
    }

    /// Appends a block of records and returns its payload offset.
    fn write_records<T: WriteRecord>(
        &mut self,
        records: &[T],
        context: &'static str,
    ) -> Result<usize, Error> {
        let stride = self.next_stride::<T>(context)?;
        let payload = self.begin_block(records.len(), stride, context)?;
        self.put_tails::<T>(payload, records.len(), stride, context)?;
        let mut record_at = payload;
        for record in records {
            record.write(self, record_at)?;
            record_at += stride;
        }
        Ok(payload)
    }

    pub(crate) fn write_single<T: WriteRecord>(
        &mut self,
        record: &T,
        at: usize,
        context: &'static str,
    ) -> Result<(), Error> {
        let payload = self.write_records(std::slice::from_ref(record), context)?;
        self.put_reference(at, payload);
        Ok(())
    }
}

impl<T: Scalar> WriteField for T {
    type Args<'a> = ();

    fn write_field<E: ByteOrder>(
        &self,
        writer: &mut Writer<'_, E>,
        at: usize,
        _context: &'static str,
        _args: (),
    ) -> Result<(), Error> {
        writer.put(at, *self);
        Ok(())
    }
}

impl WriteField for StorageType {
    type Args<'a> = ();

    fn write_field<E: ByteOrder>(
        &self,
        writer: &mut Writer<'_, E>,
        at: usize,
        _context: &'static str,
        _args: (),
    ) -> Result<(), Error> {
        writer.put(at, *self as u32);
        Ok(())
    }
}

impl WriteField for VertexSemantic {
    type Args<'a> = ();

    fn write_field<E: ByteOrder>(
        &self,
        writer: &mut Writer<'_, E>,
        at: usize,
        _context: &'static str,
        _args: (),
    ) -> Result<(), Error> {
        writer.put(at, *self as u32);
        Ok(())
    }
}

impl WriteField for String {
    type Args<'a> = ();

    fn write_field<E: ByteOrder>(
        &self,
        writer: &mut Writer<'_, E>,
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<(), Error> {
        let payload = writer.begin_block(self.len(), 1, context)?;
        writer
            .bytes_mut(payload, self.len())
            .copy_from_slice(self.as_bytes());
        writer.put_reference(at, payload);
        Ok(())
    }
}

impl<T: WriteRecord> WriteField for Vec<T> {
    type Args<'a> = ();

    fn write_field<E: ByteOrder>(
        &self,
        writer: &mut Writer<'_, E>,
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<(), Error> {
        let payload = writer.write_records(self, context)?;
        writer.put_reference(at, payload);
        Ok(())
    }
}

impl<T: WriteField> WriteField for Option<T> {
    type Args<'a> = T::Args<'a>;

    fn write_field<E: ByteOrder>(
        &self,
        writer: &mut Writer<'_, E>,
        at: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<(), Error> {
        match self {
            Some(value) => value.write_field(writer, at, context, args),
            None => {
                writer.put(at, 0u32);
                Ok(())
            }
        }
    }
}
