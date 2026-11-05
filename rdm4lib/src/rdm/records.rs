use std::io::{Read, Seek};

use super::{RdmError, RdmReadField, RdmReadRecord, RdmReader, RdmRecordLayout};

/// An index range drawn with one material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdmMeshGroup {
    /// First index in the model's index buffer.
    pub first_index: u32,
    /// Number of indices in the group.
    pub index_count: u32,
    /// Material table index.
    pub material: u32,
}

impl RdmRecordLayout for RdmMeshGroup {
    const NAME: &'static str = "RdmMeshGroup";
    const SIZE: usize = 12;
    const STRIDES: &'static [u32] = &[28];
}

impl RdmReadRecord for RdmMeshGroup {
    fn read<R: Read + Seek>(reader: &mut RdmReader<R>, at: usize) -> Result<Self, RdmError> {
        let raw = reader.record_bytes::<12>(at, Self::NAME)?;
        Ok(Self {
            first_index: u32::read_field(reader, &raw[..4], at, "RdmMeshGroup.first_index", ())?,
            index_count: u32::read_field(
                reader,
                &raw[4..8],
                at + 4,
                "RdmMeshGroup.index_count",
                (),
            )?,
            material: u32::read_field(reader, &raw[8..12], at + 8, "RdmMeshGroup.material", ())?,
        })
    }
}
