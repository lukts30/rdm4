//! Scalar layout adapted from `feat/rdm-core-derive-reader`.

use byteorder::{BigEndian, ByteOrder, LittleEndian};
use half::f16;

/// Byte order of numeric fields, selected by the fourth magic byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RdmEndian {
    /// `RDM\x01`.
    Little,
    /// `RDM\x00`, observed in Anno 2070 files.
    Big,
}

impl RdmEndian {
    pub(super) fn decode<T: Scalar>(self, raw: &[u8]) -> T {
        match self {
            Self::Little => T::decode::<LittleEndian>(raw),
            Self::Big => T::decode::<BigEndian>(raw),
        }
    }

    pub(super) fn encode<T: Scalar>(self, value: T, out: &mut [u8]) {
        match self {
            Self::Little => value.encode::<LittleEndian>(out),
            Self::Big => value.encode::<BigEndian>(out),
        }
    }
}

/// Inline value; callers provide exactly `WIDTH` bytes after checking bounds.
pub(super) trait Scalar: Copy {
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
        E::read_u16(raw)
    }

    fn encode<E: ByteOrder>(self, out: &mut [u8]) {
        E::write_u16(out, self);
    }
}

impl Scalar for u32 {
    const WIDTH: usize = 4;

    fn decode<E: ByteOrder>(raw: &[u8]) -> Self {
        E::read_u32(raw)
    }

    fn encode<E: ByteOrder>(self, out: &mut [u8]) {
        E::write_u32(out, self);
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
        Self::from_bits(u16::decode::<E>(raw))
    }

    fn encode<E: ByteOrder>(self, out: &mut [u8]) {
        self.to_bits().encode::<E>(out);
    }
}

impl Scalar for f32 {
    const WIDTH: usize = 4;

    fn decode<E: ByteOrder>(raw: &[u8]) -> Self {
        Self::from_bits(u32::decode::<E>(raw))
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
        for (i, value) in self.into_iter().enumerate() {
            value.encode::<E>(&mut out[i * T::WIDTH..(i + 1) * T::WIDTH]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check<T: Scalar + PartialEq + std::fmt::Debug>(value: T, little: &[u8], big: &[u8]) {
        for (endian, expected) in [(RdmEndian::Little, little), (RdmEndian::Big, big)] {
            // Start at an odd offset: decoding must not require alignment.
            let mut buffer = vec![0xa5; T::WIDTH + 2];
            endian.encode(value, &mut buffer[1..1 + T::WIDTH]);
            assert_eq!(&buffer[1..1 + T::WIDTH], expected);
            assert_eq!(endian.decode::<T>(&buffer[1..1 + T::WIDTH]), value);
            assert_eq!(buffer[0], 0xa5);
            assert_eq!(buffer[T::WIDTH + 1], 0xa5);
        }
    }

    #[test]
    fn scalars_and_nested_arrays_use_component_byte_order() {
        check(0xa5u8, &[0xa5], &[0xa5]);
        check(0x1234u16, &[0x34, 0x12], &[0x12, 0x34]);
        check(
            0x12345678u32,
            &[0x78, 0x56, 0x34, 0x12],
            &[0x12, 0x34, 0x56, 0x78],
        );
        check(-2i32, &[0xfe, 0xff, 0xff, 0xff], &[0xff, 0xff, 0xff, 0xfe]);
        check(
            [[0x1234u16, 0x5678], [0x9abc, 0xdef0]],
            &[0x34, 0x12, 0x78, 0x56, 0xbc, 0x9a, 0xf0, 0xde],
            &[0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0],
        );
        check([1u8, 2, 3, 4], &[1, 2, 3, 4], &[1, 2, 3, 4]);
        check([[0u8; 0]; 2], &[], &[]);
    }

    #[test]
    fn floats_preserve_nan_payloads_and_signed_zero() {
        for endian in [RdmEndian::Little, RdmEndian::Big] {
            for bits in [0u32, 0x80000000, 0x7f800000, 0x7fc12345, 0xff812345] {
                let mut raw = [0; 4];
                endian.encode(f32::from_bits(bits), &mut raw);
                assert_eq!(endian.decode::<u32>(&raw), bits);
                endian.encode(bits, &mut raw);
                assert_eq!(endian.decode::<f32>(&raw).to_bits(), bits);
            }
            for bits in [0u16, 0x8000, 0x7c00, 0x7e35, 0xfc35] {
                let mut raw = [0; 2];
                endian.encode(f16::from_bits(bits), &mut raw);
                assert_eq!(endian.decode::<u16>(&raw), bits);
                endian.encode(bits, &mut raw);
                assert_eq!(endian.decode::<f16>(&raw).to_bits(), bits);
            }
        }
    }
}
