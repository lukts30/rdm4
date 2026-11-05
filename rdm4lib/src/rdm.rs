//! Incremental RDM codec. Framing does not validate record layouts or references.

mod error;
mod framing;
mod read;
mod record;
mod records;
mod scanner;
mod wire;

pub use error::RdmError;
pub use framing::{RdmBlock, RdmBlockInfo, RdmHeader};
pub use read::{RdmDecodedBlock, RdmReader};
pub use record::{RdmFieldLayout, RdmReadField, RdmReadRecord, RdmRecordLayout};
pub use records::RdmMeshGroup;
pub use scanner::{RdmScanReport, RdmScanner};
pub use wire::RdmEndian;
