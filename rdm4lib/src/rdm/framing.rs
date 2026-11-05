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
        for (at, expected) in PREAMBLE_WORDS {
            let actual = endian.decode::<u32>(&raw[at..at + 4]);
            if actual != expected {
                return Err(RdmError::new(at, "preamble word", expected, actual));
            }
        }
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
        let payload = checked_slice(bytes, offset + Self::HEADER_SIZE, size, "block payload")?;
        Ok(Self {
            offset,
            count,
            stride,
            payload,
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
    let end = offset.checked_add(size).ok_or_else(|| {
        RdmError::new(
            offset,
            context,
            "end fitting address space",
            format!("{offset} + {size}"),
        )
    })?;
    bytes.get(offset..end).ok_or_else(|| {
        RdmError::new(
            offset,
            context,
            format!("at least {end} bytes"),
            bytes.len(),
        )
    })
}
