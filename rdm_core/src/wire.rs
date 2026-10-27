//! Wire-level layout shared by the reader and the writer.
//!
//! See `docs/RDM_BINARY_LAYOUT.md` for the framing these constants describe.

use half::f16;

/// The first three bytes of every RDM file. The fourth selects the byte order.
pub(crate) const MAGIC: [u8; 3] = *b"RDM";

/// Preamble words and their only observed values. Their meaning is unknown.
pub(crate) const PREAMBLE_WORDS: [(usize, u32); 3] = [(0x04, 0x14), (0x08, 0), (0x0c, 4)];

/// Position of the root reference; the root block's header follows it.
pub(crate) const ROOT_REFERENCE_AT: usize = 0x10;
pub(crate) const FIRST_BLOCK_AT: usize = 0x14;

/// `count: u32` followed by `stride: u32`.
pub(crate) const BLOCK_HEADER_SIZE: usize = 8;

/// References are absolute 32-bit offsets of a block's first payload byte.
pub(crate) const REFERENCE_WIDTH: usize = 4;

/// Byte order of a document's numeric fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endian {
    /// Magic byte `0x01`.
    Little,
    /// Magic byte `0x00`, seen in Anno 2070 assets.
    Big,
}

impl Endian {
    pub(crate) fn from_magic_byte(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(Self::Little),
            0 => Some(Self::Big),
            _ => None,
        }
    }

    pub(crate) fn magic_byte(self) -> u8 {
        match self {
            Self::Little => 1,
            Self::Big => 0,
        }
    }
}

/// Compile-time byte order, so that decoding loops do not branch on it.
pub(crate) trait ByteOrder {
    fn u16_from(bytes: [u8; 2]) -> u16;
    fn u32_from(bytes: [u8; 4]) -> u32;
    fn u16_to(value: u16) -> [u8; 2];
    fn u32_to(value: u32) -> [u8; 4];
}

pub(crate) struct LittleEndian;
pub(crate) struct BigEndian;

impl ByteOrder for LittleEndian {
    fn u16_from(bytes: [u8; 2]) -> u16 {
        u16::from_le_bytes(bytes)
    }

    fn u32_from(bytes: [u8; 4]) -> u32 {
        u32::from_le_bytes(bytes)
    }

    fn u16_to(value: u16) -> [u8; 2] {
        value.to_le_bytes()
    }

    fn u32_to(value: u32) -> [u8; 4] {
        value.to_le_bytes()
    }
}

impl ByteOrder for BigEndian {
    fn u16_from(bytes: [u8; 2]) -> u16 {
        u16::from_be_bytes(bytes)
    }

    fn u32_from(bytes: [u8; 4]) -> u32 {
        u32::from_be_bytes(bytes)
    }

    fn u16_to(value: u16) -> [u8; 2] {
        value.to_be_bytes()
    }

    fn u32_to(value: u32) -> [u8; 4] {
        value.to_be_bytes()
    }
}

/// Fixed-width numeric value stored inline in a record or numeric block.
///
/// `raw` and `out` are exactly `WIDTH` bytes long.
pub(crate) trait Scalar: Copy {
    const WIDTH: usize;
    fn decode<E: ByteOrder>(raw: &[u8]) -> Self;
    fn encode<E: ByteOrder>(self, out: &mut [u8]);
}

impl Scalar for u8 {
    const WIDTH: usize = 1;

    fn decode<E: ByteOrder>(raw: &[u8]) -> Self {
        raw[0]
    }

    fn encode<E: ByteOrder>(self, out: &mut [u8]) {
        out[0] = self;
    }
}

impl Scalar for u16 {
    const WIDTH: usize = 2;

    fn decode<E: ByteOrder>(raw: &[u8]) -> Self {
        E::u16_from(raw.try_into().unwrap())
    }

    fn encode<E: ByteOrder>(self, out: &mut [u8]) {
        out.copy_from_slice(&E::u16_to(self));
    }
}

impl Scalar for u32 {
    const WIDTH: usize = 4;

    fn decode<E: ByteOrder>(raw: &[u8]) -> Self {
        E::u32_from(raw.try_into().unwrap())
    }

    fn encode<E: ByteOrder>(self, out: &mut [u8]) {
        out.copy_from_slice(&E::u32_to(self));
    }
}

impl Scalar for i32 {
    const WIDTH: usize = 4;

    fn decode<E: ByteOrder>(raw: &[u8]) -> Self {
        u32::decode::<E>(raw) as i32
    }

    fn encode<E: ByteOrder>(self, out: &mut [u8]) {
        (self as u32).encode::<E>(out);
    }
}

impl Scalar for f16 {
    const WIDTH: usize = 2;

    fn decode<E: ByteOrder>(raw: &[u8]) -> Self {
        f16::from_bits(u16::decode::<E>(raw))
    }

    fn encode<E: ByteOrder>(self, out: &mut [u8]) {
        self.to_bits().encode::<E>(out);
    }
}

impl Scalar for f32 {
    const WIDTH: usize = 4;

    fn decode<E: ByteOrder>(raw: &[u8]) -> Self {
        f32::from_bits(u32::decode::<E>(raw))
    }

    fn encode<E: ByteOrder>(self, out: &mut [u8]) {
        self.to_bits().encode::<E>(out);
    }
}

impl<T: Scalar, const N: usize> Scalar for [T; N] {
    const WIDTH: usize = N * T::WIDTH;

    fn decode<E: ByteOrder>(raw: &[u8]) -> Self {
        std::array::from_fn(|i| T::decode::<E>(&raw[i * T::WIDTH..(i + 1) * T::WIDTH]))
    }

    fn encode<E: ByteOrder>(self, out: &mut [u8]) {
        for (value, out) in self.into_iter().zip(out.chunks_exact_mut(T::WIDTH)) {
            value.encode::<E>(out);
        }
    }
}

/// Number of bytes a field occupies inside its record.
pub(crate) trait FieldLayout {
    const WIDTH: usize;
}

impl<T: Scalar> FieldLayout for T {
    const WIDTH: usize = T::WIDTH;
}

impl FieldLayout for String {
    const WIDTH: usize = REFERENCE_WIDTH;
}

impl<T> FieldLayout for Vec<T> {
    const WIDTH: usize = REFERENCE_WIDTH;
}

/// A nullable reference; only meaningful for reference field types.
impl<T: FieldLayout> FieldLayout for Option<T> {
    const WIDTH: usize = REFERENCE_WIDTH;
}

/// Layout of a record type, generated by `#[derive(Record)]`.
pub(crate) trait RecordLayout {
    const NAME: &'static str;
    /// Bytes covered by known fields. The rest of each stride is preserved.
    const SIZE: usize;
    /// Accepted on-disk record widths.
    const STRIDES: &'static [u32];
}
