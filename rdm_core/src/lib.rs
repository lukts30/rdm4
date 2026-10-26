//! Minimal RDM document core: decode RDM files into typed records and write
//! unchanged documents back byte-identically.
//!
//! Framing follows `docs/RDM_BINARY_LAYOUT.md`. Record layouts follow those
//! recovered in `rdm4lib`.

// `as_chunks::<T::SIZE>()` is not expressible for generic `T` on stable.
#![allow(clippy::chunks_exact_to_as_chunks)]

#[macro_use]
mod schema;

mod codec;
mod document;
mod error;
mod payload;
mod preserved;
mod reader;
mod records;
mod writer;

pub use codec::ByteOrder;
pub use document::Document;
pub use error::{Error, ErrorKind};
pub use payload::{AttributeData, Indices, Vertices};
pub use records::{
    Animation, ExportInfo, Frame, IndexFormat, Joint, Material, MaterialSlot, MeshGroup, Model,
    ModelName, Root, Skin, StorageType, Track, VertexDescriptor, VertexFormat, VertexSemantic,
};
