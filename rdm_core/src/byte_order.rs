use facet::Facet;

/// Byte order of an RDM file, stored as the fourth magic byte.
#[derive(Facet, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ByteOrder {
    /// `RDM\x00`, used by Anno 2070 assets.
    Big = 0,
    /// `RDM\x01`.
    Little = 1,
}

impl ByteOrder {
    pub(crate) fn from_magic_byte(byte: u8) -> Option<Self> {
        match byte {
            0 => Some(ByteOrder::Big),
            1 => Some(ByteOrder::Little),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn u16(self, bytes: [u8; 2]) -> u16 {
        match self {
            ByteOrder::Big => u16::from_be_bytes(bytes),
            ByteOrder::Little => u16::from_le_bytes(bytes),
        }
    }

    #[inline]
    pub(crate) fn u32(self, bytes: [u8; 4]) -> u32 {
        match self {
            ByteOrder::Big => u32::from_be_bytes(bytes),
            ByteOrder::Little => u32::from_le_bytes(bytes),
        }
    }

    #[inline]
    pub(crate) fn u16_bytes(self, value: u16) -> [u8; 2] {
        match self {
            ByteOrder::Big => value.to_be_bytes(),
            ByteOrder::Little => value.to_le_bytes(),
        }
    }

    #[inline]
    pub(crate) fn u32_bytes(self, value: u32) -> [u8; 4] {
        match self {
            ByteOrder::Big => value.to_be_bytes(),
            ByteOrder::Little => value.to_le_bytes(),
        }
    }
}

/// Sequential reader over one record's known bytes.
///
/// The caller bounds-checks the whole block once; a read past the slice is a
/// bug in the known-size calculation, not malformed input.
pub(crate) struct RecordInput<'a> {
    bytes: &'a [u8],
    pos: usize,
    order: ByteOrder,
}

impl<'a> RecordInput<'a> {
    pub(crate) fn new(bytes: &'a [u8], order: ByteOrder) -> Self {
        RecordInput {
            bytes,
            pos: 0,
            order,
        }
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    fn take<const N: usize>(&mut self) -> [u8; N] {
        let bytes = self.bytes[self.pos..self.pos + N].try_into().unwrap();
        self.pos += N;
        bytes
    }

    pub(crate) fn u8(&mut self) -> u8 {
        self.take::<1>()[0]
    }

    pub(crate) fn u16(&mut self) -> u16 {
        let bytes = self.take();
        self.order.u16(bytes)
    }

    pub(crate) fn u32(&mut self) -> u32 {
        let bytes = self.take();
        self.order.u32(bytes)
    }

    pub(crate) fn f32_array<const N: usize>(&mut self) -> [f32; N] {
        std::array::from_fn(|_| f32::from_bits(self.u32()))
    }
}

/// Sequential writer over one record's known bytes inside the output buffer.
pub(crate) struct RecordOutput<'a> {
    bytes: &'a mut [u8],
    pos: usize,
    order: ByteOrder,
}

impl<'a> RecordOutput<'a> {
    pub(crate) fn new(bytes: &'a mut [u8], order: ByteOrder) -> Self {
        RecordOutput {
            bytes,
            pos: 0,
            order,
        }
    }

    pub(crate) fn pos(&self) -> usize {
        self.pos
    }

    fn put(&mut self, bytes: &[u8]) {
        self.bytes[self.pos..self.pos + bytes.len()].copy_from_slice(bytes);
        self.pos += bytes.len();
    }

    pub(crate) fn u8(&mut self, value: u8) {
        self.put(&[value]);
    }

    pub(crate) fn u16(&mut self, value: u16) {
        self.put(&self.order.u16_bytes(value));
    }

    pub(crate) fn u32(&mut self, value: u32) {
        self.put(&self.order.u32_bytes(value));
    }
}
