use rdm_derive::RdmRecord;

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
