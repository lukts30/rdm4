use std::ops::Range;

use super::{RdmEndian, RdmError};

const PREAMBLE_WORDS: [(usize, u32); 4] = [(4, 0x14), (8, 0), (12, 4), (16, 0x1c)];

/// The fixed preamble, excluding the root block's count and stride.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RdmHeader {
    /// Byte order used throughout the document.
    pub endian: RdmEndian,
}

impl RdmHeader {
    /// Offset of the first block header and length of the preamble.
    pub const SIZE: usize = 0x14;

    /// Validate the magic, byte order marker and observed preamble words.
    pub fn decode(bytes: &[u8]) -> Result<Self, RdmError> {
        let header = Self::detect(bytes)?;
        let words = header.endian.decode(&bytes[4..Self::SIZE]);
        if let Some(issue) = preamble_issues(words).into_iter().next() {
            return Err(issue);
        }
        Ok(header)
    }

    pub(super) fn detect(bytes: &[u8]) -> Result<Self, RdmError> {
        let raw = checked_slice(bytes, 0, Self::SIZE, "preamble")?;
        if &raw[..3] != b"RDM" {
            return Err(RdmError::new(
                0,
                "magic",
                "RDM",
                format!("{:02x?}", &raw[..3]),
            ));
        }
        let endian = match raw[3] {
            1 => RdmEndian::Little,
            0 => RdmEndian::Big,
            byte => return Err(RdmError::new(3, "byte order", "0 or 1", byte)),
        };
        Ok(Self { endian })
    }

    /// Encode the preamble with the root payload offset fixed at `0x1c`.
    pub fn encode(self) -> [u8; Self::SIZE] {
        let mut bytes = [0; Self::SIZE];
        bytes[..3].copy_from_slice(b"RDM");
        bytes[3] = u8::from(self.endian == RdmEndian::Little);
        for (at, value) in PREAMBLE_WORDS {
            self.endian.encode(value, &mut bytes[at..at + 4]);
        }
        bytes
    }
}

pub(super) fn preamble_issues(words: [u32; 4]) -> Vec<RdmError> {
    words
        .into_iter()
        .zip(PREAMBLE_WORDS)
        .filter(|(actual, (_, expected))| actual != expected)
        .map(|(actual, (at, expected))| RdmError::new(at, "preamble word", expected, actual))
        .collect()
}

/// Physical block boundaries, independent of record types and source ownership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdmBlockInfo {
    /// Absolute byte offset of the count word.
    pub offset: usize,
    /// Number of records in the payload.
    pub count: u32,
    /// On-disk byte width of one record.
    pub stride: u32,
    /// Absolute payload range, excluding the eight-byte block header.
    pub payload: Range<usize>,
}

impl RdmBlockInfo {
    pub(super) fn from_header(
        raw: &[u8; RdmBlock::HEADER_SIZE],
        offset: usize,
        source_len: usize,
        endian: RdmEndian,
    ) -> Result<Self, RdmError> {
        let start = checked_end(source_len, offset, RdmBlock::HEADER_SIZE, "block header")?;
        let count = endian.decode::<u32>(&raw[..4]);
        let stride = endian.decode::<u32>(&raw[4..]);
        for (at, context, value) in [
            (offset, "block count", count),
            (offset + 4, "block stride", stride),
        ] {
            if value == 0 {
                return Err(RdmError::new(at, context, "nonzero", value));
            }
        }
        let size = (count as usize)
            .checked_mul(stride as usize)
            .ok_or_else(|| {
                RdmError::new(
                    offset,
                    "block size",
                    "payload fitting address space",
                    format!("{count} * {stride}"),
                )
            })?;
        let end = checked_end(source_len, start, size, "block payload")?;
        Ok(Self {
            offset,
            count,
            stride,
            payload: start..end,
        })
    }
}

/// A bounded physical block. Its record type and incoming references are unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RdmBlock<'a> {
    /// Absolute byte offset of the count word.
    pub offset: usize,
    /// Number of records in the payload.
    pub count: u32,
    /// On-disk byte width of one record.
    pub stride: u32,
    /// Borrowed payload, without its block header.
    pub payload: &'a [u8],
}

impl<'a> RdmBlock<'a> {
    /// Two words: count, then stride.
    pub const HEADER_SIZE: usize = 8;

    /// Read a block at a header offset, rejecting empty or out-of-bounds blocks.
    /// This does not establish that a reference legitimately targets this block.
    pub fn decode(bytes: &'a [u8], offset: usize, endian: RdmEndian) -> Result<Self, RdmError> {
        let raw = checked_slice(bytes, offset, Self::HEADER_SIZE, "block header")?;
        let info = RdmBlockInfo::from_header(raw.try_into().unwrap(), offset, bytes.len(), endian)?;
        Ok(Self {
            offset,
            count: info.count,
            stride: info.stride,
            payload: &bytes[info.payload],
        })
    }

    /// Offset immediately following this physical block.
    pub fn next_offset(&self) -> usize {
        self.offset + Self::HEADER_SIZE + self.payload.len()
    }
}

fn checked_slice<'a>(
    bytes: &'a [u8],
    offset: usize,
    size: usize,
    context: &'static str,
) -> Result<&'a [u8], RdmError> {
    let end = checked_end(bytes.len(), offset, size, context)?;
    Ok(&bytes[offset..end])
}

pub(super) fn checked_end(
    source_len: usize,
    offset: usize,
    size: usize,
    context: &'static str,
) -> Result<usize, RdmError> {
    let end = offset.checked_add(size).ok_or_else(|| {
        RdmError::new(
            offset,
            context,
            "end fitting address space",
            format!("{offset} + {size}"),
        )
    })?;
    if end > source_len {
        return Err(RdmError::new(
            offset,
            context,
            format!("at least {end} bytes"),
            source_len,
        ));
    }
    Ok(end)
}
