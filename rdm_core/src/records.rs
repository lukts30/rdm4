//! Record definitions.
//!
//! Offsets and strides follow the layouts recovered in `rdm4lib` and the
//! strides observed in the fixtures. Byte strings are `Vec<u8>` blocks with
//! stride 1. Bytes between and after known fields are preserved by the
//! document.

use crate::codec::Const;
use crate::payload::{Indices, Vertices};

/// File signature; a fourth byte selects the byte order.
pub(crate) const SIGNATURE: [u8; 3] = *b"RDM";

/// Offset of the [`FileHeader`] record, directly after the signature.
pub(crate) const HEADER_OFFSET: usize = 4;

record! {
    /// Preamble between the signature and the root block at 0x14.
    ///
    /// The meaning of the fixed words is unknown; every known file stores
    /// these values.
    pub(crate) struct FileHeader: stride 16 {
        0x00 word_0x04: Const<0x14>,
        0x04 word_0x08: Const<0>,
        0x08 word_0x0c: Const<4>,
        0x0c root: ref Root,
    }
}

record! {
    /// The single record in the root block.
    pub struct Root: stride 48 | 52 {
        0x00 export_info: ref Option<ExportInfo>,
        0x04 model: ref Option<Model>,
        0x08 materials: ref Option<Vec<MaterialSlot>>,
        0x0c skin: ref Option<Skin>,
        0x10 animation: ref Option<Animation>,
    }
}

record! {
    /// Names recorded by the exporting tool.
    pub struct ExportInfo: stride 72 {
        0x00 export_name1: ref Option<Vec<u8>>,
        0x04 export_name2: ref Option<Vec<u8>>,
    }
}

record! {
    /// Mesh metadata with vertex and index buffers.
    ///
    /// Fields are declared in physical block order. The references at
    /// 0x0c, 0x10 and 0x14 point to vertices, indices and groups, but the
    /// blocks are stored as groups, vertices, indices.
    pub struct Model: stride 68 | 92 {
        0x00 name: ref ModelName,
        0x04 vertex_format: ref VertexFormat,
        0x08 index_format: ref IndexFormat,
        0x14 groups: ref Vec<MeshGroup>,
        0x0c vertices: ref Vertices [&vertex_format.attributes],
        0x10 indices: ref Indices [index_format.storage],
        /// Skeleton index, or -1 without a skeleton.
        0x18 skeleton_index: i32,
        0x1c bounds_min: [f32; 3],
        0x28 bounds_max: [f32; 3],
    }
}

record! {
    pub struct ModelName: stride 28 {
        0x00 name: ref Option<Vec<u8>>,
    }
}

record! {
    /// Layout of each vertex in [`Model::vertices`].
    pub struct VertexFormat: stride 24 {
        0x00 attributes: ref Vec<VertexDescriptor>,
        /// Observed values 0 to 4; possibly a shader selector.
        0x04 shader_id: u8,
    }
}

record! {
    /// One attribute within each vertex.
    pub struct VertexDescriptor: stride 16 {
        0x00 semantic: VertexSemantic,
        0x04 storage: StorageType,
        /// How stored values are interpreted; codes are only partly known.
        0x08 interpretation: u32,
        /// Number of stored scalars or packed words.
        0x0c count: u32,
    }
}

record! {
    pub struct IndexFormat: stride 20 {
        0x00 storage: StorageType,
    }
}

record! {
    /// A range of indices drawn with one material.
    pub struct MeshGroup: stride 28 {
        0x00 first_index: u32,
        0x04 index_count: u32,
        0x08 material: u32,
    }
}

record! {
    pub struct MaterialSlot: stride 28 {
        0x00 material: ref Material,
    }
}

record! {
    pub struct Material: stride 48 {
        0x00 name: ref Option<Vec<u8>>,
        0x04 texture: ref Option<Vec<u8>>,
    }
}

record! {
    pub struct Skin: stride 32 {
        0x00 joints: ref Vec<Joint>,
    }
}

record! {
    pub struct Joint: stride 84 {
        0x00 name: ref Vec<u8>,
        0x04 translation: [f32; 3],
        0x10 rotation: [f32; 4],
        0x20 parent: u32,
    }
}

record! {
    pub struct Animation: stride 48 {
        0x00 name: ref Vec<u8>,
        0x04 tracks: ref Vec<Track>,
        0x08 time_max: u32,
        /// Nominal samples per second.
        0x0c sampling_rate: u32,
    }
}

record! {
    /// Keyframes of one joint.
    pub struct Track: stride 24 {
        0x00 name: ref Vec<u8>,
        0x04 frames: ref Vec<Frame>,
    }
}

record! {
    pub struct Frame: stride 32 {
        0x00 rotation: [f32; 4],
        0x10 translation: [f32; 3],
        0x1c time: f32,
    }
}

wire_enum! {
    /// Scalar storage code shared by vertex and index formats.
    pub enum StorageType {
        U16 = 3,
        U32 = 5,
        F16 = 6,
        F32 = 7,
    }
}

impl StorageType {
    /// Width of one stored scalar or packed word.
    pub const fn byte_size(self) -> usize {
        match self {
            StorageType::U16 | StorageType::F16 => 2,
            StorageType::U32 | StorageType::F32 => 4,
        }
    }
}

wire_enum! {
    /// Role of a vertex attribute.
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
}

impl VertexDescriptor {
    /// Bytes this attribute occupies in each vertex.
    pub fn byte_size(&self) -> usize {
        self.storage.byte_size() * self.count as usize
    }
}
