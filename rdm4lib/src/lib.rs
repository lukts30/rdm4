use binrw::BinReaderExt;
use rdm_data_main::*;
use rdm_data_main::{MeshInfo, RdmFile};
use std::path::Path;

use std::fs::File;

use std::str;

use half::f16;

use nalgebra::*;

#[macro_use]
extern crate log;

#[macro_use]
extern crate approx;

pub mod gltf_export;
pub mod gltf_reader;
pub mod gltf_reader_vertex;
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
    rdmf: Option<RdmFile<RdmKindMesh>>,
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
    transition: [f32; 3],
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

    pub fn add_skin(&mut self) {
        let rdm = self.rdmf.as_ref().unwrap();

        let raw_joints = &***rdm.header1.skin.joint;
        let mut joints_vec = vec![];

        for raw_joint in raw_joints {
            let tx = raw_joint.t[0];
            let ty = raw_joint.t[1];
            let tz = raw_joint.t[2];

            let rx = raw_joint.r[0];
            let ry = raw_joint.r[1];
            let rz = raw_joint.r[2];
            let rw = raw_joint.r[3];

            let quaternion = Quaternion::new(rw, rx, ry, rz);
            let unit_quaternion = UnitQuaternion::from_quaternion(quaternion);

            let quaternion_mat4 = unit_quaternion.quaternion().coords;

            // apply rotation and negate vector
            // aka -1 * (UnitQuaternion*Vector)
            let v: Vector3<f32> = Vector3::new(tx, ty, tz);
            let v_transformed = unit_quaternion.transform_vector(&v).scale(-1.0);

            let joint = RdJoint {
                name: String::from(raw_joint.name.as_ascii()),
                transition: [v_transformed.x, v_transformed.y, v_transformed.z],
                quaternion: [
                    quaternion_mat4.x,
                    quaternion_mat4.y,
                    quaternion_mat4.z,
                    quaternion_mat4.w,
                ],
                parent: raw_joint.parent_id,
            };
            joints_vec.push(joint);
        }

        self.joints = Some(joints_vec);
    }

    fn new(buf: Vec<u8>) -> Self {
        let mut reader = std::io::Cursor::new(&buf);

        let rdm: RdmFile<RdmKindMesh> = reader.read_le().unwrap();
        info!(
            "  Endianness: {}",
            if rdm.big_endian { "big" } else { "little" }
        );

        let vvert = VertexFormat2::read_format_via_data(&rdm);
        info!("  Mesh #0");
        info!(
            "    Vertices: {}, format: {}, stride: {} bytes",
            vvert.len(),
            vvert,
            vvert.get_size()
        );

        debug!(
            "    Stored bounds (min, max): {:?}",
            rdm.header1.meta.bounds()
        );
        debug!(
            "    Calculated bounds (min, max): {:?}",
            vvert.calculate_bounds()
        );

        let mut offset = 0;
        for component in &vvert.identifiers {
            debug!(
                "      {:?}: type={:?}, count={}, interpretation={}, offset={}, bytes={}",
                component.semantic,
                component.storage_type,
                component.storage_count,
                component.interpretation.0,
                offset,
                component.get_size()
            );
            offset += component.get_size();
        }
        let index_type = rdm.header1.meta.index_format.element_type;
        info!(
            "    Index type: {:?} ({} bytes)",
            index_type,
            index_type.byte_size()
        );

        let triangles_idx_count = rdm.header1.meta.triangle_list_len();
        let triangles_real_count = triangles_idx_count / 3;
        let mut triangles = Vec::with_capacity(triangles_real_count as usize);
        let ts: Vec<_> = rdm.header1.meta.triangle_list(rdm.big_endian).collect();
        for x in ts.chunks(3) {
            let t = Triangle {
                indices: [x[0], x[1], x[2]],
            };
            triangles.push(t);
        }

        let mesh_info: Vec<_> = rdm.header1.meta.0.mesh_info.iter().cloned().collect();
        info!("    Triangles: {}", triangles_real_count);
        for (index, primitive) in mesh_info.iter().enumerate() {
            info!("    Primitive #{}", index);
            info!(
                "      Material: {}, first index: {}, index count: {}",
                primitive.material, primitive.start_index_location, primitive.index_count
            );
        }

        RdModell {
            rdmf: Some(rdm),
            mesh_info,
            joints: None,
            triangle_indices: triangles,
            vertex: vvert,
            anim: None,
            mat: None,
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Triangle {
    indices: [u32; 3],
}

impl<P: AsRef<Path>> From<P> for RdModell {
    fn from(f_path: P) -> Self {
        let mut f = File::open(&f_path).unwrap();
        let metadata = f.metadata().unwrap();
        let len = metadata.len() as usize;
        let mut buffer = vec![0; len];
        std::io::Read::read_exact(&mut f, &mut buffer).expect("I/O ERROR");

        let buffer_len = buffer.len();

        info!("Loaded {:?} ({} bytes)", f_path.as_ref(), buffer_len);
        RdModell::new(buffer)
    }
}

#[cfg(test)]
mod tests_intern {
    use super::*;
    use binrw::{BinWriterExt, Endian};

    #[test]
    fn reject_invalid_index_format() {
        let bytes = include_bytes!("../rdm/fishery_others_lod2.rdm");
        let word =
            |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let meta = word(word(16) + 4);
        let index_format = word(meta + 8);
        // Unknown code, mismatched width, and floating-point index types.
        for code in [99u32, 5, 6, 7] {
            let mut invalid = bytes.to_vec();
            invalid[index_format..index_format + 4].copy_from_slice(&code.to_le_bytes());
            assert!(std::io::Cursor::new(invalid)
                .read_le::<RdmFile<RdmKindMesh>>()
                .is_err());
        }
    }

    #[test]
    fn big_endian_mesh_matches_little_endian() {
        let bytes = include_bytes!("../rdm/fishery_others_lod2.rdm");
        let expected = RdModell::new(bytes.to_vec());
        assert_eq!(expected.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h");
        for index_size in [2, 4] {
            let mut raw: RdmFile<RdmKindMesh> = std::io::Cursor::new(bytes).read_le().unwrap();
            let meta = &mut raw.header1.meta;
            // Build a BE fixture independently of the reader's normalization.
            for vertex in meta.vertex.storage.items.chunks_exact_mut(24) {
                for field in vertex[..8].chunks_exact_mut(2) {
                    field.reverse();
                }
                for field in vertex[8..20].chunks_exact_mut(4) {
                    field.reverse();
                }
                for field in vertex[20..].chunks_exact_mut(2) {
                    field.reverse();
                }
            }
            meta.index_format.element_type = if index_size == 2 {
                vertex::StorageType::U16
            } else {
                vertex::StorageType::U32
            };
            meta.triangle_list.info.part_size = index_size;
            meta.triangle_list.storage.items = expected
                .triangle_indices
                .iter()
                .flat_map(|t| t.indices)
                .flat_map(|i| i.to_be_bytes()[4 - index_size as usize..].to_vec())
                .map(rdm_container::AnnoU8)
                .collect();
            let mut output = std::io::Cursor::new(Vec::new());
            output.write_type_args(&raw, Endian::Big, ()).unwrap();
            let mut output = output.into_inner();
            // The public writer deliberately continues to emit an LE preamble.
            output[3] = 0;
            for word in output[4..20].chunks_exact_mut(4) {
                word.reverse();
            }
            let actual = RdModell::new(output);
            assert_eq!(actual.vertex.as_bytes(), expected.vertex.as_bytes());
            assert_eq!(
                actual.triangle_indices.len(),
                expected.triangle_indices.len()
            );
            assert!(actual
                .triangle_indices
                .iter()
                .zip(&expected.triangle_indices)
                .all(|(a, b)| a.indices == b.indices));
            let normals = |model: &RdModell| {
                model
                    .vertex
                    .iter::<vertex::N4b, vertex::N3f>(0)
                    .unwrap()
                    .map(|n| n.data)
                    .collect::<Vec<_>>()
            };
            assert_eq!(normals(&actual), normals(&expected));
        }
    }

    #[test]
    fn fishery_others_cutout_lod0() {
        // for Miri test include bytes since there is no i/o.
        let bytes = include_bytes!("../rdm/fishery_others_cutout_lod0.rdm");
        let v = bytes.to_vec();

        let rdm = RdModell::new(v);
        assert_eq!(rdm.vertex.len(), 32);
        assert_eq!(rdm.vertex.get_size(), 8);
        assert_eq!(rdm.triangle_indices.len() * 3, 78);
    }
}
