//! Incremental RDM codec. Framing does not validate record layouts or references.

mod buffers;
mod convert;
mod error;
mod framing;
mod layout;
mod packing;
mod read;
mod records;
mod scanner;
mod split;
mod wire;
mod write;

pub use convert::write_scene;
pub use error::RdmError;
pub use framing::{RdmBlock, RdmBlockInfo, RdmHeader};
pub use layout::{RdmBlockLayout, RdmReadResult, RdmSourceLayout};
pub use packing::RdmExportOptions;
pub use read::{RdmDecodedBlock, RdmReadField, RdmReadRecord, RdmReader};
pub use records::{
    RdmAnimation, RdmAttributeValues, RdmDocument, RdmExportInfo, RdmFrame, RdmIndexFormat,
    RdmIndices, RdmJoint, RdmMaterial, RdmMaterialEntry, RdmMeshGroup, RdmModel, RdmModelName,
    RdmSkin, RdmTrack, RdmVertexFormat, RdmVertices,
};
pub use scanner::{RdmScanReport, RdmScanner};
pub use split::RdmSplitter;
pub use wire::{RdmEndian, RdmFieldLayout, RdmRecordLayout};
pub use write::{RdmWriteField, RdmWriteRecord, RdmWriter};
