use rdm_derive::RdmRecord;

/// Source scene and resource names stored in an RDM export-info block.
#[derive(Debug, Clone, PartialEq, Eq, RdmRecord)]
#[rdm(strides(72))]
pub struct RdmExportInfo {
    /// Source scene or baking path, when present.
    pub source_path: Option<String>,
    /// Resource `.rmp` file name, when present.
    pub rmp_name: Option<String>,
}

/// An index range drawn with one material.
#[derive(Debug, Clone, PartialEq, Eq, RdmRecord)]
#[rdm(strides(28))]
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
#[rdm(strides(28))]
pub struct RdmMaterialEntry {
    /// Material associated with this table index.
    pub material: RdmMaterial,
}

/// Material name and texture path stored in the RDM file.
#[derive(Debug, Clone, PartialEq, Eq, RdmRecord)]
#[rdm(strides(48))]
pub struct RdmMaterial {
    /// Optional material name.
    pub name: Option<String>,
    /// Optional texture path, without loading or converting the texture.
    pub texture: Option<String>,
}

/// Skeleton joint table referenced by a file root.
#[derive(Debug, Clone, PartialEq, RdmRecord)]
#[rdm(strides(32))]
pub struct RdmSkin {
    /// Joints in the order used by vertex joint indices.
    pub joints: Vec<RdmJoint>,
}

/// Named skeleton joint and its stored transform.
#[derive(Debug, Clone, PartialEq, RdmRecord)]
#[rdm(strides(84))]
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
