//! Single-pass reader.
//!
//! Blocks must form a rooted tree stored in depth-first preorder: a block's
//! records come first, then each record's child blocks in the record's block
//! order. The reader keeps a cursor at the end of the last block and accepts
//! a reference only if it targets the payload of the block at the cursor.
//! This rejects backward references, shared blocks, cycles, gaps,
//! unsupported block orders and trailing bytes in one check.

mod indices;
mod vertices;

use std::marker::PhantomData;

use crate::document::{Document, Preserved};
use crate::records::{Root, StorageType, VertexSemantic};
use crate::wire::{
    BigEndian, ByteOrder, Endian, LittleEndian, RecordLayout, Scalar, BLOCK_HEADER_SIZE,
    FIRST_BLOCK_AT, MAGIC, PREAMBLE_WORDS, ROOT_REFERENCE_AT,
};
use crate::Error;

pub(crate) struct Reader<'a, E> {
    input: &'a [u8],
    cursor: usize,
    preserved: Preserved,
    byte_order: PhantomData<E>,
}

/// A validated block: `count * stride` payload bytes lie inside the input.
pub(crate) struct Block {
    pub(crate) payload: usize,
    pub(crate) count: usize,
    pub(crate) stride: usize,
}

/// A record decoded from `RecordLayout::SIZE` bytes at `at`.
pub(crate) trait ReadRecord: RecordLayout + Sized {
    fn read<E: ByteOrder>(reader: &mut Reader<'_, E>, at: usize) -> Result<Self, Error>;

    /// Reads every record of a validated block. `#[rdm(plain)]` records
    /// replace this with an infallible typed loop.
    fn read_block<E: ByteOrder>(
        reader: &mut Reader<'_, E>,
        block: &Block,
    ) -> Result<Vec<Self>, Error> {
        let mut records = Vec::with_capacity(block.count);
        let mut record_at = block.payload;
        for _ in 0..block.count {
            records.push(Self::read(reader, record_at)?);
            record_at += block.stride;
        }
        Ok(records)
    }
}

/// A field decoded from its `raw` bytes at input offset `at`.
///
/// Reference fields read their child block through `reader`. `Args` carries
/// sibling fields that describe a dynamic payload.
pub(crate) trait ReadField: Sized {
    type Args<'a>;

    fn read_field<E: ByteOrder>(
        reader: &mut Reader<'_, E>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<Self, Error>;
}

pub(crate) fn read_document(input: &[u8]) -> Result<Document, Error> {
    if input.len() < FIRST_BLOCK_AT {
        return Err(Error::new(
            0,
            "Header",
            format!("at least {FIRST_BLOCK_AT:#x} bytes"),
            format!("{:#x} bytes", input.len()),
        ));
    }
    if input[..3] != MAGIC {
        return Err(Error::new(
            0,
            "Header.magic",
            "\"RDM\"",
            format!("{:02x?}", &input[..3]),
        ));
    }
    let Some(endian) = Endian::from_magic_byte(input[3]) else {
        return Err(Error::new(
            3,
            "Header.byte_order",
            "0x01 (little-endian) or 0x00 (big-endian)",
            format!("{:#04x}", input[3]),
        ));
    };

    let (root, preserved) = match endian {
        Endian::Little => read_root::<LittleEndian>(input)?,
        Endian::Big => read_root::<BigEndian>(input)?,
    };
    log::debug!(
        "read {} bytes ({endian:?}), preserved {} unknown bytes and {} strides",
        input.len(),
        preserved.tails.len(),
        preserved.strides.len()
    );
    Ok(Document {
        endian,
        root,
        preserved,
    })
}

fn read_root<E: ByteOrder>(input: &[u8]) -> Result<(Root, Preserved), Error> {
    for (at, expected) in PREAMBLE_WORDS {
        let actual = u32::decode::<E>(&input[at..at + 4]);
        if actual != expected {
            return Err(Error::new(
                at,
                "Header.preamble",
                format!("{expected:#x}"),
                format!("{actual:#x}"),
            ));
        }
    }

    let mut reader = Reader {
        input,
        cursor: FIRST_BLOCK_AT,
        preserved: Preserved::default(),
        byte_order: PhantomData::<E>,
    };
    let raw = &input[ROOT_REFERENCE_AT..ROOT_REFERENCE_AT + 4];
    let root = Root::read_field(&mut reader, raw, ROOT_REFERENCE_AT, "Header.root", ())?;

    if reader.cursor != input.len() {
        return Err(Error::new(
            reader.cursor,
            "Header",
            format!("end of file after the last block at {:#x}", reader.cursor),
            format!("{} trailing bytes", input.len() - reader.cursor),
        ));
    }

    let mut preserved = reader.preserved;
    preserved.tails.shrink_to_fit();
    preserved.strides.shrink_to_fit();
    Ok((root, preserved))
}

impl<'a, E: ByteOrder> Reader<'a, E> {
    pub(crate) fn record_bytes<const N: usize>(&self, at: usize) -> &'a [u8; N] {
        self.input[at..at + N]
            .try_into()
            .expect("record lies inside a validated block")
    }

    pub(crate) fn block_bytes(&self, block: &Block) -> &'a [u8] {
        &self.input[block.payload..block.payload + block.count * block.stride]
    }

    /// Follows the non-null reference stored in `raw` at `at`.
    pub(crate) fn enter_block(
        &mut self,
        raw: &[u8],
        at: usize,
        context: &'static str,
    ) -> Result<Block, Error> {
        let target = u32::decode::<E>(raw) as usize;
        let header = self.cursor;
        let payload = header + BLOCK_HEADER_SIZE;

        if target == 0 {
            return Err(Error::new(
                at,
                context,
                format!("reference to {payload:#x}"),
                "null reference",
            ));
        }
        if target < payload {
            return Err(Error::new(
                at,
                context,
                format!("reference to the next block at {payload:#x}"),
                format!("{target:#x} (backward or shared reference)"),
            ));
        }
        if target > payload {
            return Err(Error::new(
                at,
                context,
                format!("reference to the next block at {payload:#x}"),
                format!("{target:#x} (gap or unsupported block order)"),
            ));
        }
        if payload > self.input.len() {
            return Err(Error::new(
                header,
                context,
                "block header inside the file",
                format!("file ends at {:#x}", self.input.len()),
            ));
        }

        let count = u32::decode::<E>(&self.input[header..header + 4]) as usize;
        let stride = u32::decode::<E>(&self.input[header + 4..payload]) as usize;
        let end = count
            .checked_mul(stride)
            .and_then(|size| payload.checked_add(size))
            .filter(|&end| end <= self.input.len());
        let Some(end) = end else {
            return Err(Error::new(
                header,
                context,
                format!("block ending by {:#x}", self.input.len()),
                format!("{count} records of {stride} bytes"),
            ));
        };

        self.cursor = end;
        Ok(Block {
            payload,
            count,
            stride,
        })
    }

    /// Enters a block of `T` records and preserves their unknown bytes.
    fn enter_records<T: RecordLayout>(
        &mut self,
        raw: &[u8],
        at: usize,
        context: &'static str,
    ) -> Result<Block, Error> {
        let block = self.enter_block(raw, at, context)?;
        let stride = block.stride as u32;
        if !T::STRIDES.contains(&stride) {
            return Err(Error::new(
                block.payload - 4,
                context,
                format!("{} stride {:?}", T::NAME, T::STRIDES),
                stride,
            ));
        }
        if T::STRIDES.len() > 1 {
            self.preserved.strides.push(stride);
        }
        if block.stride > T::SIZE {
            for record in self.block_bytes(&block).chunks_exact(block.stride) {
                self.preserved.tails.extend_from_slice(&record[T::SIZE..]);
            }
        }
        Ok(block)
    }

    /// Reads a block that must contain exactly one `T`.
    pub(crate) fn read_single<T: ReadRecord>(
        &mut self,
        raw: &[u8],
        at: usize,
        context: &'static str,
    ) -> Result<T, Error> {
        let block = self.enter_records::<T>(raw, at, context)?;
        if block.count != 1 {
            return Err(Error::new(
                block.payload - BLOCK_HEADER_SIZE,
                context,
                format!("1 {} record", T::NAME),
                format!("{} records", block.count),
            ));
        }
        T::read(self, block.payload)
    }
}

impl<T: Scalar> ReadField for T {
    type Args<'a> = ();

    fn read_field<E: ByteOrder>(
        _reader: &mut Reader<'_, E>,
        raw: &[u8],
        _at: usize,
        _context: &'static str,
        _args: (),
    ) -> Result<Self, Error> {
        Ok(T::decode::<E>(raw))
    }
}

impl ReadField for StorageType {
    type Args<'a> = ();

    fn read_field<E: ByteOrder>(
        _reader: &mut Reader<'_, E>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<Self, Error> {
        StorageType::try_from(u32::decode::<E>(raw))
            .map_err(|code| Error::new(at, context, "storage type 3, 5, 6 or 7", code))
    }
}

impl ReadField for VertexSemantic {
    type Args<'a> = ();

    fn read_field<E: ByteOrder>(
        _reader: &mut Reader<'_, E>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<Self, Error> {
        VertexSemantic::try_from(u32::decode::<E>(raw))
            .map_err(|code| Error::new(at, context, "vertex semantic 0 to 7", code))
    }
}

impl ReadField for String {
    type Args<'a> = ();

    fn read_field<E: ByteOrder>(
        reader: &mut Reader<'_, E>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<Self, Error> {
        let block = reader.enter_block(raw, at, context)?;
        if block.stride != 1 {
            return Err(Error::new(
                block.payload - 4,
                context,
                "string stride 1",
                block.stride,
            ));
        }
        let bytes = reader.block_bytes(&block);
        match std::str::from_utf8(bytes) {
            Ok(text) => Ok(text.to_owned()),
            Err(error) => Err(Error::new(
                block.payload + error.valid_up_to(),
                context,
                "UTF-8 string",
                format!("invalid byte {:#04x}", bytes[error.valid_up_to()]),
            )),
        }
    }
}

impl<T: ReadRecord> ReadField for Vec<T> {
    type Args<'a> = ();

    fn read_field<E: ByteOrder>(
        reader: &mut Reader<'_, E>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<Self, Error> {
        let block = reader.enter_records::<T>(raw, at, context)?;
        T::read_block(reader, &block)
    }
}

impl<T: ReadField> ReadField for Option<T> {
    type Args<'a> = T::Args<'a>;

    fn read_field<E: ByteOrder>(
        reader: &mut Reader<'_, E>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<Self, Error> {
        if u32::decode::<E>(raw) == 0 {
            return Ok(None);
        }
        T::read_field(reader, raw, at, context, args).map(Some)
    }
}
