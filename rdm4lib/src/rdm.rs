//! Incremental RDM codec. Framing does not validate record layouts or references.

mod error;
mod framing;
mod wire;

pub use error::Error;
pub use framing::{Block, Header};
pub use wire::Endian;
