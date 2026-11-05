use rdm_derive::RdmRecord;

use crate::vertex::{StorageType, VertexIdentifier};

use super::RdmAnimation;

/// Owned file contents, independent of source layout and byte order.
#[derive(Debug, Clone, PartialEq, RdmRecord)]
#[rdm(strides(48, 52), write)]
pub struct RdmDocument {
    /// Source scene and resource names.
    pub export_info: Option<RdmExportInfo>,
    /// All models in file order.
    pub models: Option<Vec<RdmModel>>,
    /// Material table shared by the models.
    pub materials: Option<Vec<RdmMaterialEntry>>,
    /// Optional skeleton.
    pub skin: Option<RdmSkin>,
    /// Optional animations.
    pub animations: Option<Vec<RdmAnimation>>,
}

/// Model metadata and decoded buffers. Fresh output uses the 92-byte variant.
/// Unknown record tails live in the source layout.
#[derive(Debug, Clone, PartialEq, RdmRecord)]
#[rdm(strides(92, 68), write)]
pub struct RdmModel {
    /// Optional name stored through a wrapper record.
    pub name: RdmModelName,
    /// Attribute layout in each vertex.
    pub vertex_format: RdmVertexFormat,
    /// Original index scalar type.
    pub index_format: RdmIndexFormat,
    /// Values decoded using the preceding vertex format.
    #[rdm(args(vertex_format))]
    pub vertices: RdmVertices,
    /// Indices checked against the preceding vertex count.
    #[rdm(args(index_format, vertices))]
    pub indices: RdmIndices,
    /// Draw ranges and their materials.
    pub groups: Vec<RdmMeshGroup>,
    /// Skeleton index, or -1 for an unskinned model.
    pub skeleton_index: i32,
    /// Stored minimum XYZ bounds.
    pub bounds_min: [f32; 3],
    /// Stored maximum XYZ bounds.
    pub bounds_max: [f32; 3],
}

/// Name wrapper referenced by a model.
#[derive(Debug, Clone, PartialEq, Eq, RdmRecord)]
#[rdm(strides(28), write)]
pub struct RdmModelName {
    /// Optional model name.
    pub name: Option<String>,
}

/// Decoded vertex attributes, in descriptor order.
#[derive(Debug, Clone, PartialEq)]
pub struct RdmVertices {
    /// Number of vertices.
    pub count: usize,
    /// Each column contains count times storage_count values, in vertex order.
    pub attributes: Vec<RdmAttributeValues>,
}

/// Scalar values of one vertex attribute; packed values remain integer words.
#[derive(Debug, Clone, PartialEq)]
pub enum RdmAttributeValues {
    /// Unsigned 16-bit components.
    U16(Vec<u16>),
    /// Unsigned 32-bit components or packed words.
    U32(Vec<u32>),
    /// Half-precision components.
    F16(Vec<half::f16>),
    /// Single-precision components.
    F32(Vec<f32>),
}

/// Decoded indices, widened from the original U16 or U32 representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdmIndices {
    /// Indices into the model's vertex buffer.
    pub values: Vec<u32>,
}

/// Source scene and resource names stored in an RDM export-info block.
#[derive(Debug, Clone, PartialEq, Eq, RdmRecord)]
#[rdm(strides(72), write)]
pub struct RdmExportInfo {
    /// Source scene or baking path, when present.
    pub source_path: Option<String>,
    /// Resource `.rmp` file name, when present.
    pub rmp_name: Option<String>,
}

/// Vertex attribute table used to interpret a model's vertex buffer.
#[derive(Debug, Clone, PartialEq, Eq, RdmRecord)]
#[rdm(strides(24), write)]
pub struct RdmVertexFormat {
    /// Attributes in their order within each vertex.
    pub attributes: Vec<VertexIdentifier>,
    /// Stored vertex/skin selector, preserved without interpreting its values.
    pub selector: u8,
}

impl std::fmt::Display for RdmVertexFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, attribute) in self.attributes.iter().enumerate() {
            if i != 0 {
                f.write_str("_")?;
            }
            write!(f, "{attribute}")?;
        }
        Ok(())
    }
}

/// Scalar representation of the model's index buffer.
#[derive(Debug, Clone, PartialEq, Eq, RdmRecord)]
#[rdm(strides(20), write)]
pub struct RdmIndexFormat {
    /// Buffer decoding additionally requires an unsigned integer type.
    pub storage_type: StorageType,
}

/// An index range drawn with one material.
#[derive(Debug, Clone, PartialEq, Eq, RdmRecord)]
#[rdm(strides(28), write)]
pub struct RdmMeshGroup {
    /// First index in the model's index buffer.
    pub first_index: u32,
    /// Number of indices in the group.
    pub index_count: u32,
    /// Material table index.
    pub material: u32,
}

/// One entry in the file's material table.
#[derive(Debug, Clone, PartialEq, Eq, RdmRecord)]
#[rdm(strides(28), write)]
pub struct RdmMaterialEntry {
    /// Material associated with this table index.
    pub material: RdmMaterial,
}

/// Material name and texture path stored in the RDM file.
#[derive(Debug, Clone, PartialEq, Eq, RdmRecord)]
#[rdm(strides(48), write)]
pub struct RdmMaterial {
    /// Optional material name.
    pub name: Option<String>,
    /// Optional texture path, without loading or converting the texture.
    pub texture: Option<String>,
}

/// Skeleton joint table referenced by a file root.
#[derive(Debug, Clone, PartialEq, RdmRecord)]
#[rdm(strides(32), write)]
pub struct RdmSkin {
    /// Joints in the order used by vertex joint indices.
    pub joints: Vec<RdmJoint>,
}

/// Named skeleton joint and its stored transform.
#[derive(Debug, Clone, PartialEq, RdmRecord)]
#[rdm(strides(84), write)]
pub struct RdmJoint {
    /// Name used to match animation tracks.
    pub name: String,
    /// Stored translation vector.
    pub translation: [f32; 3],
    /// Rotation quaternion in `x, y, z, w` order.
    pub rotation: [f32; 4],
    /// Parent joint index as stored in the file.
    pub parent: u32,
}
