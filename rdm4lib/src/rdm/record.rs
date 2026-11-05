//! Record contracts adapted from `feat/rdm-core-derive-reader`.

use std::io::{Read, Seek};

use super::{wire::Scalar, RdmError, RdmReader};

/// Bytes occupied by a field inside its containing record.
pub trait RdmFieldLayout {
    /// Inline scalar width, or four bytes for a block reference.
    const WIDTH: usize;
}

impl<T: Scalar> RdmFieldLayout for T {
    const WIDTH: usize = T::WIDTH;
}

/// Known fields and supported on-disk widths of a record.
pub trait RdmRecordLayout {
    /// Record name for diagnostics.
    const NAME: &'static str;
    /// Bytes covered by the declared fields, excluding preserved tails.
    const SIZE: usize;
    /// Accepted record strides.
    const STRIDES: &'static [u32];
}

/// Construct a record at a location inside a validated block.
pub trait RdmReadRecord: RdmRecordLayout + Sized {
    /// Decode fields; reference fields may read other indexed blocks.
    fn read<R: Read + Seek>(reader: &mut RdmReader<R>, at: usize) -> Result<Self, RdmError>;
}

/// Decode a field with borrowed sibling context, independently of source storage.
pub trait RdmReadField: RdmFieldLayout + Sized {
    /// Context needed by this field, such as a borrowed vertex format.
    type Args<'a>;

    /// Decode a bounded field slice at its absolute source offset.
    fn read_field<R: Read + Seek>(
        reader: &mut RdmReader<R>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<Self, RdmError>;
}

impl<T: Scalar> RdmReadField for T {
    type Args<'a> = ();

    fn read_field<R: Read + Seek>(
        reader: &mut RdmReader<R>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<Self, RdmError> {
        if raw.len() != T::WIDTH {
            return Err(RdmError::new(at, context, T::WIDTH, raw.len()));
        }
        Ok(reader.endian().decode(raw))
    }
}
