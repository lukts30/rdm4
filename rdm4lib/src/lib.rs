use rdm_data_main::MeshInfo;
use rdm_data_main::*;
use std::{fs::File, io::BufReader, path::Path};

use half::f16;

#[macro_use]
extern crate log;

#[macro_use]
extern crate approx;

pub mod gltf_export;
pub mod gltf_reader;
pub mod gltf_reader_vertex;
pub mod rdm;
pub mod rdm_anim;
pub mod rdm_material;
pub mod vertex;
pub mod vertex_transform;
use crate::rdm_anim::RdAnim;
use rdm_material::RdMaterial;

use vertex::VertexFormat2;

pub mod rdm_container;
pub mod rdm_data_anim;
pub mod rdm_data_main;

pub struct RdModell {
    pending_joints: Option<Vec<RdJoint>>,
    pub mesh_info: Vec<MeshInfo>,
    pub joints: Option<Vec<RdJoint>>,
    pub triangle_indices: Vec<Triangle>,

    pub vertex: VertexFormat2,
    anim: Option<RdAnim>,
    pub mat: Option<RdMaterial>,
}

#[derive(Debug, Clone)]
pub struct RdJoint {
    name: String,
    translation: [f32; 3],
    quaternion: [f32; 4],
    parent: u32,
}

impl RdModell {
    pub fn has_skin(&self) -> bool {
        self.joints.is_some()
    }

    pub fn add_anim(&mut self, anim: RdAnim) {
        self.anim = Some(anim);
    }

    /// Activate the skeleton loaded with the model.
    pub fn add_skin(&mut self) {
        if self.joints.is_none() {
            self.joints = Some(
                self.pending_joints
                    .take()
                    .expect("requested skeleton is absent"),
            );
        }
    }

    fn read<R: std::io::Read + std::io::Seek>(source: R) -> Self {
        rdm::RdmReader::new(source)
            .expect("invalid RDM framing")
            .read_root::<rdm::RdmDocument>()
            .expect("invalid RDM records")
            .value
            .into_model(0, false)
            .expect("input has no usable model")
    }

    #[cfg(test)]
    fn new(bytes: Vec<u8>) -> Self {
        Self::read(std::io::Cursor::new(bytes))
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Triangle {
    indices: [u32; 3],
}

impl<P: AsRef<Path>> From<P> for RdModell {
    fn from(f_path: P) -> Self {
        info!("Loaded {:?}", f_path.as_ref());
        Self::read(BufReader::new(
            File::open(f_path).expect("cannot open RDM file"),
        ))
    }
}

#[cfg(test)]
mod tests_intern {
    use super::*;

    #[test]
    fn reads_u32_indices_without_truncation() {
        let model = RdModell::from("rdm/destruction_stone_building_04_decal_detail01_lod0.rdm");
        assert_eq!(model.triangle_indices.len(), 37_240);
        let indices = model.triangle_indices.iter().flat_map(|t| t.indices);
        assert_eq!(indices.max(), Some(74_479));
        let expected = model.vertex.as_bytes().to_vec();
        let document = rdm::RdmDocument::try_from(model).unwrap();
        assert_eq!(
            document.models.as_ref().unwrap()[0]
                .index_format
                .storage_type,
            vertex::StorageType::U32
        );
        let output = rdm::RdmWriter::new(std::io::Cursor::new(Vec::new()), rdm::RdmEndian::Little)
            .write_root(&document)
            .unwrap()
            .into_inner();
        let model = RdModell::new(output);
        assert_eq!(model.vertex.as_bytes(), expected);
        assert_eq!(
            model
                .triangle_indices
                .iter()
                .flat_map(|triangle| triangle.indices)
                .max(),
            Some(74_479)
        );
    }

    #[test]
    fn rejects_unknown_float_and_mismatched_index_types() {
        let bytes = include_bytes!("../rdm/fishery_others_lod2.rdm");
        let word =
            |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let meta = word(word(16) + 4);
        let index_format = word(meta + 8);
        for (code, case) in [
            (99u32, "unknown storage code"),
            (5, "U32 descriptor with a U16 index buffer"),
            (6, "F16 indices"),
            (7, "F32 indices"),
        ] {
            let mut invalid = bytes.to_vec();
            invalid[index_format..index_format + 4].copy_from_slice(&code.to_le_bytes());
            assert!(
                rdm::RdmReader::new(std::io::Cursor::new(invalid))
                    .unwrap()
                    .read_root::<rdm::RdmDocument>()
                    .is_err(),
                "accepted {case}"
            );
        }
    }

    #[test]
    fn decodes_be_vertices_and_both_index_widths_like_le() {
        let bytes = include_bytes!("../rdm/fishery_others_lod2.rdm");
        let expected = RdModell::new(bytes.to_vec());
        for storage_type in [vertex::StorageType::U16, vertex::StorageType::U32] {
            let mut document = rdm::RdmReader::new(std::io::Cursor::new(bytes))
                .unwrap()
                .read_root::<rdm::RdmDocument>()
                .unwrap()
                .value;
            document.models.as_mut().unwrap()[0]
                .index_format
                .storage_type = storage_type;
            let output = rdm::RdmWriter::new(std::io::Cursor::new(Vec::new()), rdm::RdmEndian::Big)
                .write_root(&document)
                .unwrap()
                .into_inner();
            let actual = RdModell::new(output);
            assert_eq!(actual.vertex.as_bytes(), expected.vertex.as_bytes());
            let indices = |model: &RdModell| {
                model
                    .triangle_indices
                    .iter()
                    .map(|t| t.indices)
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                indices(&actual),
                indices(&expected),
                "{storage_type:?} indices"
            );
        }
    }

    #[test]
    fn reads_position_only_mesh_from_memory() {
        // for Miri test include bytes since there is no i/o.
        let bytes = include_bytes!("../rdm/fishery_others_cutout_lod0.rdm");
        let v = bytes.to_vec();

        let rdm = RdModell::new(v);
        assert_eq!(rdm.vertex.len(), 32);
        assert_eq!(rdm.vertex.get_size(), 8);
        assert_eq!(rdm.triangle_indices.len() * 3, 78);
    }
}
