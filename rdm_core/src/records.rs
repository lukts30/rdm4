//! RDM record types.
//!
//! Each struct lists its known fields in on-disk order, starting at the
//! record's first byte. References are stored as 32-bit offsets to child
//! blocks: `T` is a block with one record, `Vec<T>` a block with any number
//! of records, `String` a block of bytes and `Option` a nullable reference.
//! Bytes after the known fields are unknown; the document preserves them.
//!
//! Layouts follow `rdm4lib/src/rdm_data_main.rs` and `rdm_data_anim.rs`.
//! Strides and nullability were checked against the benchmark fixtures.

use rdm_core_derive::Record;

use crate::buffers::{Indices, Vertices};
use crate::wire::{FieldLayout, REFERENCE_WIDTH};

/// The root record at `0x1c`. 48- and 52-byte strides occur.
#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(48, 52))]
pub struct Root {
    pub export_info: Option<ExportInfo>,
    /// One or more models; multi-model files occur.
    pub models: Option<Vec<Model>>,
    pub materials: Option<Vec<MaterialEntry>>,
    pub skin: Option<Skin>,
    pub animations: Option<Vec<Animation>>,
}

#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(72))]
pub struct ExportInfo {
    /// Observed values are source scene or baking paths.
    pub source_path: Option<String>,
    /// Observed values are `.rmp` file names.
    pub rmp_name: Option<String>,
}

/// Model metadata. 68- and 92-byte strides occur.
///
/// The references are declared vertices, indices, groups, but the blocks are
/// stored groups, vertices, indices.
#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(68, 92), block_order(groups, vertices, indices))]
pub struct Model {
    pub name: ModelName,
    pub vertex_format: VertexFormat,
    pub index_format: IndexFormat,
    #[rdm(args(vertex_format))]
    pub vertices: Vertices,
    #[rdm(args(index_format, vertices))]
    pub indices: Indices,
    pub groups: Vec<MeshGroup>,
    /// `-1` when the model has no skeleton.
    pub skeleton_index: i32,
    pub bounds_min: [f32; 3],
    pub bounds_max: [f32; 3],
}

#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(28))]
pub struct ModelName {
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(24))]
pub struct VertexFormat {
    pub attributes: Vec<VertexAttribute>,
}

/// Describes one attribute in each vertex.
#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(16))]
pub struct VertexAttribute {
    pub semantic: VertexSemantic,
    pub storage_type: StorageType,
    /// How stored values are interpreted; only partially understood.
    pub interpretation: u32,
    /// Values per vertex. Packed attributes use one `U32` word.
    pub components: u32,
}

#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(20))]
pub struct IndexFormat {
    pub storage_type: StorageType,
}

/// A range of indices drawn with one material.
#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(28), plain)]
pub struct MeshGroup {
    pub first_index: u32,
    pub index_count: u32,
    pub material: u32,
}

#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(28))]
pub struct MaterialEntry {
    pub material: Material,
}

#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(48))]
pub struct Material {
    pub name: Option<String>,
    /// Observed values are texture paths.
    pub texture: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(32))]
pub struct Skin {
    pub joints: Vec<Joint>,
}

#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(84))]
pub struct Joint {
    pub name: String,
    pub translation: [f32; 3],
    /// Quaternion `x, y, z, w`.
    pub rotation: [f32; 4],
    pub parent: u32,
}

#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(48))]
pub struct Animation {
    pub name: String,
    pub tracks: Vec<Track>,
    pub time_max: u32,
    /// Nominal samples per second.
    pub sampling_rate: u32,
}

/// Keyframes of one joint.
#[derive(Debug, Clone, PartialEq, Record)]
#[rdm(strides(24))]
pub struct Track {
    pub joint_name: String,
    pub frames: Vec<Frame>,
}

/// Animation keyframe. Frame arrays dominate animation files.
#[derive(Debug, Clone, Copy, PartialEq, Record)]
#[rdm(strides(32), plain)]
pub struct Frame {
    /// Quaternion `x, y, z, w`.
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub time: f32,
}

/// Scalar storage codes shared by vertex and index formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    U16 = 3,
    U32 = 5,
    F16 = 6,
    F32 = 7,
}

impl StorageType {
    pub fn byte_width(self) -> usize {
        match self {
            Self::U16 | Self::F16 => 2,
            Self::U32 | Self::F32 => 4,
        }
    }
}

impl TryFrom<u32> for StorageType {
    type Error = u32;

    fn try_from(code: u32) -> Result<Self, u32> {
        match code {
            3 => Ok(Self::U16),
            5 => Ok(Self::U32),
            6 => Ok(Self::F16),
            7 => Ok(Self::F32),
            _ => Err(code),
        }
    }
}

impl FieldLayout for StorageType {
    const WIDTH: usize = 4;
}

/// Role of a vertex attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl TryFrom<u32> for VertexSemantic {
    type Error = u32;

    fn try_from(code: u32) -> Result<Self, u32> {
        match code {
            0 => Ok(Self::Position),
            1 => Ok(Self::Normal),
            2 => Ok(Self::Tangent),
            3 => Ok(Self::Bitangent),
            4 => Ok(Self::Texcoord),
            5 => Ok(Self::Color),
            6 => Ok(Self::Weight),
            7 => Ok(Self::JointIndex),
            _ => Err(code),
        }
    }
}

impl FieldLayout for VertexSemantic {
    const WIDTH: usize = 4;
}

impl FieldLayout for Vertices {
    const WIDTH: usize = REFERENCE_WIDTH;
}

impl FieldLayout for Indices {
    const WIDTH: usize = REFERENCE_WIDTH;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::RecordLayout;

    #[test]
    fn known_fields_match_the_existing_layouts() {
        assert_eq!(Root::SIZE, 20);
        assert_eq!(ExportInfo::SIZE, 8);
        assert_eq!(Model::SIZE, 52);
        assert_eq!(ModelName::SIZE, 4);
        assert_eq!(VertexFormat::SIZE, 4);
        assert_eq!(VertexAttribute::SIZE, 16);
        assert_eq!(IndexFormat::SIZE, 4);
        assert_eq!(MeshGroup::SIZE, 12);
        assert_eq!(MaterialEntry::SIZE, 4);
        assert_eq!(Material::SIZE, 8);
        assert_eq!(Skin::SIZE, 4);
        assert_eq!(Joint::SIZE, 36);
        assert_eq!(Animation::SIZE, 16);
        assert_eq!(Track::SIZE, 8);
        assert_eq!(Frame::SIZE, 32);
    }
}
