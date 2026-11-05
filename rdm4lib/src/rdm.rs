//! Incremental RDM codec. Framing does not validate record layouts or references.

mod error;
mod framing;
mod wire;

pub use error::RdmError;
pub use framing::{RdmBlock, RdmHeader};
pub use wire::RdmEndian;
