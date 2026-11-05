//! Incremental RDM codec. Framing does not validate record layouts or references.

mod error;
mod framing;
mod layout;
mod read;
mod records;
mod scanner;
mod wire;

pub use error::RdmError;
pub use framing::{RdmBlock, RdmBlockInfo, RdmHeader};
pub use layout::{RdmBlockLayout, RdmReadResult, RdmSourceLayout};
pub use read::{RdmDecodedBlock, RdmReadField, RdmReadRecord, RdmReader};
pub use records::{RdmAnimation, RdmExportInfo, RdmFrame, RdmMeshGroup, RdmTrack};
pub use scanner::{RdmScanReport, RdmScanner};
pub use wire::{RdmEndian, RdmFieldLayout, RdmRecordLayout};
