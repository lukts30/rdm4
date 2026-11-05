//! Incremental RDM codec. Framing does not validate record layouts or references.

mod error;
mod framing;
mod read;
mod records;
mod scanner;
mod wire;

pub use error::RdmError;
pub use framing::{RdmBlock, RdmBlockInfo, RdmHeader};
pub use read::{RdmDecodedBlock, RdmReadField, RdmReadRecord, RdmReader};
pub use records::{RdmFrame, RdmMeshGroup};
pub use scanner::{RdmScanReport, RdmScanner};
pub use wire::{RdmEndian, RdmFieldLayout, RdmRecordLayout};
