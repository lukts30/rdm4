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
