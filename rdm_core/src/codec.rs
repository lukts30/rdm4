//! Byte-order-aware encoding of inline values.

use half::f16;

use crate::error::ErrorKind;

/// Byte order of all numeric fields in an RDM file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    Little,
    Big,
}

impl ByteOrder {
    /// The byte following "RDM" in the file signature.
    pub(crate) const fn signature_byte(self) -> u8 {
        match self {
            ByteOrder::Little => 1,
            ByteOrder::Big => 0,
        }
    }

    pub(crate) const fn from_signature_byte(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(ByteOrder::Little),
            0 => Some(ByteOrder::Big),
            _ => None,
        }
    }
}

/// A fixed-size number that always decodes successfully.
pub(crate) trait Number: Copy {
    const SIZE: usize;

    /// Decode from exactly `SIZE` bytes.
    fn from_bytes(bytes: &[u8], order: ByteOrder) -> Self;

    /// Encode into exactly `SIZE` bytes.
    fn to_bytes(self, out: &mut [u8], order: ByteOrder);
}

macro_rules! impl_number {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Number for $ty {
                const SIZE: usize = std::mem::size_of::<$ty>();

                #[inline]
                fn from_bytes(bytes: &[u8], order: ByteOrder) -> Self {
                    let bytes = bytes.try_into().expect("caller passes SIZE bytes");
                    match order {
                        ByteOrder::Little => <$ty>::from_le_bytes(bytes),
                        ByteOrder::Big => <$ty>::from_be_bytes(bytes),
                    }
                }

                #[inline]
                fn to_bytes(self, out: &mut [u8], order: ByteOrder) {
                    let bytes = match order {
                        ByteOrder::Little => self.to_le_bytes(),
                        ByteOrder::Big => self.to_be_bytes(),
                    };
                    out.copy_from_slice(&bytes);
                }
            }
        )*
    };
}

impl_number!(u8, u16, u32, i32, f32);

impl Number for f16 {
    const SIZE: usize = 2;

    #[inline]
    fn from_bytes(bytes: &[u8], order: ByteOrder) -> Self {
        f16::from_bits(u16::from_bytes(bytes, order))
    }

    #[inline]
    fn to_bytes(self, out: &mut [u8], order: ByteOrder) {
        self.to_bits().to_bytes(out, order);
    }
}

impl<T: Number + Default, const N: usize> Number for [T; N] {
    const SIZE: usize = T::SIZE * N;

    fn from_bytes(bytes: &[u8], order: ByteOrder) -> Self {
        let mut values = [T::default(); N];
        for (value, chunk) in values.iter_mut().zip(bytes.chunks_exact(T::SIZE)) {
            *value = T::from_bytes(chunk, order);
        }
        values
    }

    fn to_bytes(self, out: &mut [u8], order: ByteOrder) {
        for (value, chunk) in self.iter().zip(out.chunks_exact_mut(T::SIZE)) {
            value.to_bytes(chunk, order);
        }
    }
}

/// A fixed-size value stored inline in a record. Unlike [`Number`], decoding
/// can reject the stored bits.
pub(crate) trait Value: Sized {
    const SIZE: usize;

    fn decode(bytes: &[u8], order: ByteOrder) -> Result<Self, ErrorKind>;

    fn encode(&self, out: &mut [u8], order: ByteOrder);
}

impl<T: Number> Value for T {
    const SIZE: usize = T::SIZE;

    #[inline]
    fn decode(bytes: &[u8], order: ByteOrder) -> Result<Self, ErrorKind> {
        Ok(T::from_bytes(bytes, order))
    }

    #[inline]
    fn encode(&self, out: &mut [u8], order: ByteOrder) {
        self.to_bytes(out, order);
    }
}

/// A `u32` word whose value is fixed by the format.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Const<const V: u32>;

impl<const V: u32> Value for Const<V> {
    const SIZE: usize = 4;

    fn decode(bytes: &[u8], order: ByteOrder) -> Result<Self, ErrorKind> {
        let actual = u32::from_bytes(bytes, order);
        if actual != V {
            return Err(ErrorKind::UnexpectedValue {
                expected: V,
                actual,
            });
        }
        Ok(Const)
    }

    fn encode(&self, out: &mut [u8], order: ByteOrder) {
        V.to_bytes(out, order);
    }
}
