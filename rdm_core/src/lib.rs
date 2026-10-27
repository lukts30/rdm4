//! Typed reading and byte-identical writing of RDM files.
//!
//! [`Document::from_bytes`] decodes a file into plain [`records`] and
//! [`Document::to_bytes`] writes it back. Wire layout follows from the
//! record field types; see the [`records`] module.
//!
//! Logging goes through the `log` crate and is off unless the application
//! installs a logger.

mod byte_order;
mod document;
mod error;
mod layout;
mod read;
mod read_payload;
pub mod records;
mod wire;
mod write;
mod write_payload;

pub use byte_order::ByteOrder;
pub use document::Document;
pub use error::{Error, ErrorKind};
