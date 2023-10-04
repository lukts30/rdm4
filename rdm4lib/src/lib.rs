use bytes::{Buf, Bytes};
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
pub mod rdm_anim;
pub mod rdm_anim_writer;
pub mod rdm_material;
pub mod rdm_writer;
pub mod vertex;
use crate::rdm_anim::RdAnim;
use rdm_material::RdMaterial;

use vertex::VertexFormat2;

pub mod rdm_container;
pub mod rdm_data;

#[derive(Debug)]
pub struct RdModell {
    size: u32,
    buffer: Bytes,
    pub mesh_info: Vec<MeshInstance>,
    pub joints: Option<Vec<RdJoint>>,
    pub triangle_indices: Vec<Triangle>,

    pub vertex: VertexFormat2,
    anim: Option<RdAnim>,
    pub mat: Option<RdMaterial>,
}

trait Seek {
    fn seek(&mut self, from_start: u32, file_size: u32);
}

impl Seek for Bytes {
    fn seek(&mut self, offset_from_start: u32, file_size: u32) {
        let already_read = file_size - self.remaining() as u32;
        let cnt: usize = (offset_from_start.checked_sub(already_read).unwrap()) as usize;
        self.advance(cnt);
    }
}

#[derive(Debug, Clone)]
pub struct RdJoint {
    name: String,
    transition: [f32; 3],
    quaternion: [f32; 4],
    parent: u8,
}

#[derive(Debug)]
pub struct MeshInstance {
    start_index_location: u32,
    index_count: u32,
    material: u32,
}

impl MeshInstance {
    pub fn get_max_material(instances: &[MeshInstance]) -> u32 {
        instances.iter().map(|e| e.material).max().unwrap()
    }
}

impl RdModell {
    const META_COUNT: u32 = 8; //neg
    const VERTEX_META: u32 = 12;
    const TRIANGLES_META: u32 = 16;

    pub fn has_skin(&self) -> bool {
        self.joints.is_some()
    }

    pub fn add_anim(&mut self, anim: RdAnim) {
        self.anim = Some(anim);
    }

    pub fn check_has_magic_byte(bytes: &[u8]) {
        static MAGIC: &[u8] = &[0x52, 0x44, 0x4D, 0x01];
        assert_eq!(
            &bytes[0..4],
            MAGIC,
            "Magic Bytes 0x52, 0x44, 0x4D, 0x01, 0x14 not found !"
        );
    }

    pub fn add_skin(&mut self) {
        let mut skin_buffer = self.buffer.clone();
        skin_buffer.advance(40);
        let skin_offset = skin_buffer.get_u32_le();
        assert!(skin_offset != 0, "File does not contain a skin !");

        skin_buffer.seek(skin_offset, self.size);

        let first_skin_offset = skin_buffer.get_u32_le();
        let joint_count_ptr = first_skin_offset - RdModell::META_COUNT;

        skin_buffer.seek(joint_count_ptr, self.size);

        let joint_count = skin_buffer.get_u32_le();
        let joint_size = skin_buffer.get_u32_le();

        let mut joints_vec: Vec<RdJoint> = Vec::with_capacity(joint_count as usize);

        let mut joint_name_buffer = skin_buffer.clone();

        let len_first_joint_name_ptr = joint_name_buffer.get_u32_le() - RdModell::META_COUNT;
        joint_name_buffer.seek(len_first_joint_name_ptr, self.size);

        assert_eq!(joint_size, 84);
        for _ in 0..joint_count {
            let len_joint_name = joint_name_buffer.get_u32_le();
            assert_eq!(joint_name_buffer.get_u32_le(), 1);
            let name = str::from_utf8(&joint_name_buffer[..len_joint_name as usize]).unwrap();
            let joint_name = String::from(name);

            let nameptr = skin_buffer.get_u32_le();
            assert_eq!(nameptr, self.size - joint_name_buffer.len() as u32);
            joint_name_buffer.advance(len_joint_name as usize);

            let tx = skin_buffer.get_f32_le();
            let ty = skin_buffer.get_f32_le();
            let tz = skin_buffer.get_f32_le();

            let rx = skin_buffer.get_f32_le();
            let ry = skin_buffer.get_f32_le();
            let rz = skin_buffer.get_f32_le();
            let rw = skin_buffer.get_f32_le();

            let quaternion = Quaternion::new(rw, rx, ry, rz);
            let unit_quaternion = UnitQuaternion::from_quaternion(quaternion);

            let quaternion_mat4 = unit_quaternion.quaternion().coords;

            // apply rotation and negate vector
            // aka -1 * (UnitQuaternion*Vector)
            let v: Vector3<f32> = Vector3::new(tx, ty, tz);
            let v_transformed = unit_quaternion.transform_vector(&v).scale(-1.0);

            let parent_id = skin_buffer.get_u8();

            let joint = RdJoint {
                name: joint_name,
                transition: [v_transformed.x, v_transformed.y, v_transformed.z],
                quaternion: [
                    quaternion_mat4.x,
                    quaternion_mat4.y,
                    quaternion_mat4.z,
                    quaternion_mat4.w,
                ],
                parent: parent_id,
            };

            joints_vec.push(joint);
            skin_buffer.advance(84 - 33);
        }

        self.joints = Some(joints_vec);
    }

    fn new(buf: Vec<u8>) -> Self {
        use binrw::BinReaderExt;
        use rdm_data::*;

        RdModell::check_has_magic_byte(&buf);

        let mut reader = std::io::Cursor::new(&buf);
        let rdm: RdmFile = reader.read_ne().unwrap();

        let size = buf.len() as u32;
        let buffer = Bytes::from(buf);

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

        let mut mesh_info = vec![];
        for x in rdm.header1.meta.0.mesh_info.iter() {
            mesh_info.push(MeshInstance {
                index_count: x.index_count,
                start_index_location: x.start_index_location,
                material: x.material,
            });
        }

        RdModell {
            size,
            buffer,
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
}
