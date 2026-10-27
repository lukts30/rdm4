//! Reads RDM documents into typed records and writes unchanged documents
//! back byte for byte.
//!
//! ```no_run
//! let document = rdm_core::Document::open("model.rdm").unwrap();
//! let bytes = document.to_bytes().unwrap();
//! ```
//!
//! Debug messages go through the `log` crate. They are off unless a logger
//! is installed and can be compiled out with `log`'s `max_level_off` feature.

// `as_chunks::<T::WIDTH>()` cannot name a generic parameter's constant;
// `chunks_exact(T::WIDTH)` is constant after monomorphization.
#![allow(clippy::chunks_exact_to_as_chunks)]

mod buffers;
mod document;
mod error;
mod read;
mod records;
mod wire;
mod write;

pub use buffers::{AttributeValues, Indices, Vertices};
pub use document::Document;
pub use error::Error;
pub use half::f16;
pub use records::*;
pub use wire::Endian;
