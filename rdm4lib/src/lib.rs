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
use crate::rdm_anim::RdAnim;
use rdm_material::RdMaterial;

use vertex::VertexFormat2;

pub mod rdm_container;
pub mod rdm_data_anim;
pub mod rdm_data_main;

pub struct RdModell {
    rdmf: Option<RdmFile>,
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
        let rdm: RdmFile = reader.read_ne().unwrap();

        let vvert = VertexFormat2::read_format_via_data(&rdm);
        info!(
            "Read {} vertices of type {} ({} bytes)",
            vvert.len(),
            vvert,
            vvert.get_size()
        );

        let triangles_idx_count = rdm.header1.meta.0.triangles.len() as u32;
        let triangles_real_count = triangles_idx_count / 3;
        let mut triangles = Vec::with_capacity(triangles_real_count as usize);
        for x in rdm.header1.meta.0.triangles.chunks(3) {
            let t = Triangle {
                indices: [x[0].0, x[1].0, x[2].0],
            };
            triangles.push(t);
        }

        let mesh_info = rdm.header1.meta.0.mesh_info.iter().cloned().collect();

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
    indices: [u16; 3],
}

impl<P: AsRef<Path>> From<P> for RdModell {
    fn from(f_path: P) -> Self {
        let mut f = File::open(&f_path).unwrap();
        let metadata = f.metadata().unwrap();
        let len = metadata.len() as usize;
        let mut buffer = vec![0; len];
        std::io::Read::read_exact(&mut f, &mut buffer).expect("I/O ERROR");

        let buffer_len = buffer.len();

        info!("loaded {:?} into buffer", f_path.as_ref().to_str().unwrap());

        info!("buffer size: {}", buffer_len);
        RdModell::new(buffer)
    }
}

#[cfg(test)]
mod tests_intern {
    use crate::rdm_container::RdmString;

    use super::*;

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

    use crate::rdm_container::AnnoPtr;
    use binrw::binread;
    use rdm_derive::RdmStructSize;

    #[derive(RdmStructSize)]
    #[binread]
    struct MyStruct {
        _my_number: f32,
        _my_other_number: [u8; 16],
        _name: AnnoPtr<RdmString>,
    }

    #[test]
    fn test() {
        dbg!(MyStruct::get_struct_byte_size());
    }
}
