//! RDM record definitions.
//!
//! Each struct lists the known leading fields of one on-disk record, in wire
//! order. Bytes after the known fields, up to the block stride, are unknown;
//! the document preserves them privately.
//!
//! Field types map to the wire as follows:
//!
//! - `u8`, `u16`, `u32`, `i32`, `f32`, `[T; N]` and field-less enums are
//!   stored inline; enums use their `repr` integer.
//! - `Box<T>` is an offset to a block holding exactly one `T`.
//! - `Vec<T>` is an offset to a block of `T` records; `Vec<u8>` is a byte
//!   string.
//! - `Option<_>` allows a null (zero) offset.
//! - [`Vertices`] and [`Indices`] are offsets to numeric blocks described by
//!   the model's [`VertexFormat`] and [`IndexFormat`].

use facet::Facet;

use crate::byte_order::ByteOrder;

/// The 20-byte file preamble preceding the root block.
#[derive(Facet, Debug, Clone, PartialEq)]
pub(crate) struct Preamble {
    pub magic: [u8; 3],
    pub byte_order: ByteOrder,
    /// Equals the root header position; meaning unknown.
    pub unknown_04: u32,
    pub unknown_08: u32,
    pub unknown_0c: u32,
    pub root_offset: u32,
}

impl Preamble {
    pub(crate) const SIZE: u64 = 0x14;
    /// Position of `root_offset` within the file.
    pub(crate) const ROOT_OFFSET_AT: u64 = 0x10;

    /// The only preamble observed, for the given byte order.
    pub(crate) fn expected(byte_order: ByteOrder) -> Self {
        Preamble {
            magic: *b"RDM",
            byte_order,
            unknown_04: 0x14,
            unknown_08: 0,
            unknown_0c: 4,
            root_offset: 0x1c,
        }
    }
}

/// The single record of the root block (48 or 52 bytes on disk).
#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Root {
    pub export_info: Option<Box<ExportInfo>>,
    pub models: Option<Vec<Model>>,
    pub materials: Option<Vec<MaterialSlot>>,
    pub skin: Option<Box<Skin>>,
    pub animation: Option<Box<Animation>>,
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct ExportInfo {
    pub export_name1: Option<Vec<u8>>,
    pub export_name2: Option<Vec<u8>>,
}

/// Model metadata (68 or 92 bytes on disk).
///
/// Blocks are stored as groups, vertices, indices although the fields point
/// to vertices, indices, groups.
#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Model {
    pub name: Box<ModelName>,
    pub vertex_format: Box<VertexFormat>,
    pub index_format: Box<IndexFormat>,
    pub vertices: Vertices,
    pub indices: Indices,
    pub groups: Vec<MeshGroup>,
    /// `-1` means no skeleton.
    pub skeleton_index: i32,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct ModelName {
    pub name: Option<Vec<u8>>,
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct VertexFormat {
    pub attributes: Vec<VertexAttribute>,
    pub unknown_shader_id: u8,
}

/// Describes one attribute in each vertex.
#[derive(Facet, Debug, Clone, PartialEq)]
pub struct VertexAttribute {
    pub semantic: VertexSemantic,
    pub storage_type: StorageType,
    /// How stored values are interpreted; unknown codes are kept as is.
    pub interpretation: u32,
    pub storage_count: u32,
}

#[derive(Facet, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum VertexSemantic {
    Position = 0,
    Normal = 1,
    Tangent = 2,
    Bitangent = 3,
    Texcoord = 4,
    Color = 5,
    Weight = 6,
    JointIndex = 7,
}

/// Scalar storage codes shared by vertex and index formats.
#[derive(Facet, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum StorageType {
    U16 = 3,
    U32 = 5,
    F16 = 6,
    F32 = 7,
}

impl StorageType {
    /// Bytes occupied by one scalar or packed word.
    pub fn byte_size(self) -> u32 {
        match self {
            StorageType::U16 | StorageType::F16 => 2,
            StorageType::U32 | StorageType::F32 => 4,
        }
    }
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct IndexFormat {
    pub storage_type: StorageType,
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct MeshGroup {
    pub start_index: u32,
    pub index_count: u32,
    pub material: u32,
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct MaterialSlot {
    pub material: Box<Material>,
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Material {
    pub name: Option<Vec<u8>>,
    pub png_path: Option<Vec<u8>>,
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Skin {
    pub joints: Vec<Joint>,
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Joint {
    pub name: Vec<u8>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
    pub parent: u32,
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Animation {
    pub name: Vec<u8>,
    pub tracks: Vec<Track>,
    pub time_max: u32,
    /// Nominal samples per second.
    pub sampling_rate: u32,
}

#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Track {
    pub name: Vec<u8>,
    pub frames: Vec<Frame>,
}

#[derive(Facet, Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub time: f32,
}

/// Decoded vertex block: one value column per [`VertexAttribute`].
#[derive(Facet, Debug, Clone, PartialEq)]
pub struct Vertices {
    pub count: u32,
    /// Same order as the format's attributes.
    pub attributes: Vec<AttributeValues>,
}

/// Values of one attribute, `storage_count` per vertex, in vertex order.
#[derive(Facet, Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum AttributeValues {
    U16(Vec<u16>),
    /// Packed words, such as four normal bytes, decoded as one `u32`.
    U32(Vec<u32>),
    F16(Vec<F16>),
    F32(Vec<f32>),
}

/// IEEE 754 binary16 value, kept as its bit pattern.
#[derive(Facet, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct F16(pub u16);

/// Decoded index block.
#[derive(Facet, Debug, Clone, PartialEq)]
#[repr(u8)]
pub enum Indices {
    U16(Vec<u16>),
    U32(Vec<u32>),
}
