//! Incremental RDM codec. Framing does not validate record layouts or references.

mod error;
mod framing;
mod scanner;
mod wire;

pub use error::RdmError;
pub use framing::{RdmBlock, RdmBlockInfo, RdmHeader};
pub use scanner::{RdmScanReport, RdmScanner};
pub use wire::RdmEndian;
