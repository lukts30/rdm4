use bytes::{Buf, Bytes};
use std::path::Path;

use std::fs::File;

use std::str;

use half::f16;
use std::slice;

use nalgebra::*;

#[macro_use]
extern crate log;

#[macro_use]
extern crate approx;

#[allow(unused_imports)]
#[macro_use]
extern crate memoffset;

pub mod gltf_export;
pub mod gltf_reader;
pub mod rdm_anim;
pub mod rdm_anim_writer;
pub mod rdm_material;
pub mod rdm_writer;
use crate::rdm_anim::RDAnim;
use rdm_material::RDMaterial;

#[derive(Debug)]
pub struct RDModell {
    size: u32,
    buffer: Bytes,
    pub joints: Option<Vec<RDJoint>>,
    vertices: VertexFormat,
    pub triangle_indices: Vec<Triangle>,

    meta: u32,
    vertex_offset: u32,
    pub vertices_count: u32,
    vertex_buffer_size: u32,

    triangles_offset: u32,
    pub triangles_idx_count: u32,
    triangles_idx_size: u32,

    anim: Option<RDAnim>,
    pub mat: Option<RDMaterial>,
}

trait GetVertex {
    fn get_p4h(&mut self) -> P4h;
    fn get_n4b(&mut self) -> N4b;
    fn get_g4b(&mut self) -> G4b;
    fn get_b4b(&mut self) -> B4b;
    fn get_t2h(&mut self) -> T2h;
    fn get_i4b(&mut self) -> I4b;
    fn get_w4b(&mut self) -> W4b;
    fn get_c4c(&mut self) -> C4c;
}

impl GetVertex for Bytes {
    fn get_p4h(&mut self) -> P4h {
        P4h {
            pos: [
                f16::from_bits(self.get_u16_le()),
                f16::from_bits(self.get_u16_le()),
                f16::from_bits(self.get_u16_le()),
                f16::from_bits(self.get_u16_le()),
            ],
        }
    }
    fn get_n4b(&mut self) -> N4b {
        N4b {
            normals: [self.get_u8(), self.get_u8(), self.get_u8(), self.get_u8()],
        }
    }
    fn get_g4b(&mut self) -> G4b {
        G4b {
            tangent: [self.get_u8(), self.get_u8(), self.get_u8(), self.get_u8()],
        }
    }
    fn get_b4b(&mut self) -> B4b {
        B4b {
            binormal: [self.get_u8(), self.get_u8(), self.get_u8(), self.get_u8()],
        }
    }
    fn get_t2h(&mut self) -> T2h {
        T2h {
            tex: [
                f16::from_bits(self.get_u16_le()),
                f16::from_bits(self.get_u16_le()),
            ],
        }
    }
    fn get_i4b(&mut self) -> I4b {
        I4b {
            blend_idx: [self.get_u8(), self.get_u8(), self.get_u8(), self.get_u8()],
        }
    }

    fn get_w4b(&mut self) -> W4b {
        W4b {
            blend_weight: [self.get_u8(), self.get_u8(), self.get_u8(), self.get_u8()],
        }
    }

    fn get_c4c(&mut self) -> C4c {
        C4c {
            unknown: [self.get_u8(), self.get_u8(), self.get_u8(), self.get_u8()],
        }
    }
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
pub struct RDJoint {
    name: String,
    nameptr: u32,
    transition: [f32; 3],
    quaternion: [f32; 4],
    parent: u8,
    locked: bool,
}

#[derive(Debug)]
pub struct MeshInstance {
    start_index_location: u32,
    index_count: u32,
    mesh: u32,
}

#[allow(dead_code)]
impl RDModell {
    const META_OFFSET: u32 = 32;
    const META_COUNT: u32 = 8; //neg
    const META_SIZE: u32 = 4; //neg
    const VERTEX_META: u32 = 12;
    const TRIANGLES_META: u32 = 16;

    pub fn has_skin(&self) -> bool {
        self.joints.is_some()
    }

    pub fn add_anim(&mut self, anim: RDAnim) {
        self.anim = Some(anim);
    }

    pub fn check_has_magic_byte(bytes: &[u8]) {
        assert_eq!(
            bytes[0], 0x52,
            "Magic Bytes 0x52, 0x44, 0x4D, 0x01, 0x14 not found !"
        );
        assert_eq!(
            bytes[1], 0x44,
            "Magic Bytes 0x52, 0x44, 0x4D, 0x01, 0x14 not found !"
        );
        assert_eq!(
            bytes[2], 0x4D,
            "Magic Bytes 0x52, 0x44, 0x4D, 0x01, 0x14 not found !"
        );
        assert_eq!(
            bytes[3], 0x01,
            "Magic Bytes 0x52, 0x44, 0x4D, 0x01, 0x14 not found !"
        );
        assert_eq!(
            bytes[4], 0x14,
            "Magic Bytes 0x52, 0x44, 0x4D, 0x01, 0x14 not found !"
        );
    }

    pub fn check_multi_mesh(&self) {
        let mut multi_buffer = self.buffer.clone();
        multi_buffer.seek(self.meta + 20, self.size);
        let first_instance = multi_buffer.get_u32_le();

        multi_buffer.seek(first_instance - RDModell::META_COUNT, self.size);
        let mesh_count = multi_buffer.get_u32_le();
        assert_eq!(multi_buffer.get_u32_le(), 28);
        warn!("mesh_count: {}", mesh_count);
        let mut v = Vec::with_capacity(mesh_count as usize);
        for _ in 0..mesh_count {
            v.push(MeshInstance {
                start_index_location: multi_buffer.get_u32_le(),
                index_count: multi_buffer.get_u32_le(),
                mesh: multi_buffer.get_u32_le(),
            });
            multi_buffer.advance(28 - 12);
        }
        warn!("meshes: {:?}", v);
    }

    pub fn add_skin(&mut self) {
        let mut skin_buffer = self.buffer.clone();
        skin_buffer.advance(40);
        let skin_offset = skin_buffer.get_u32_le();
        assert_eq!(skin_offset != 0, true, "File does not contain a skin !");

        skin_buffer.seek(skin_offset, self.size);

        let first_skin_offset = skin_buffer.get_u32_le();
        let joint_count_ptr = first_skin_offset - RDModell::META_COUNT;

        skin_buffer.seek(joint_count_ptr, self.size);

        let joint_count = skin_buffer.get_u32_le();
        let joint_size = skin_buffer.get_u32_le();

        let mut joints_vec: Vec<RDJoint> = Vec::with_capacity(joint_count as usize);

        let mut joint_name_buffer = skin_buffer.clone();

        let len_first_joint_name_ptr = joint_name_buffer.get_u32_le() - RDModell::META_COUNT;
        joint_name_buffer.seek(len_first_joint_name_ptr, self.size);

        assert_eq!(joint_size, 84);
        for _ in 0..joint_count {
            let len_joint_name = joint_name_buffer.get_u32_le();
            assert_eq!(joint_name_buffer.get_u32_le(), 1);
            let name = str::from_utf8(&joint_name_buffer[..len_joint_name as usize]).unwrap();
            let joint_name = String::from(name);
            joint_name_buffer.advance(len_joint_name as usize);

            let nameptr = skin_buffer.get_u32_le();

            let tx = skin_buffer.get_f32_le();
            let ty = skin_buffer.get_f32_le();
            let tz = skin_buffer.get_f32_le();

            let rx = -skin_buffer.get_f32_le();
            let ry = -skin_buffer.get_f32_le();
            let rz = -skin_buffer.get_f32_le();
            let rw = -skin_buffer.get_f32_le();

            let quaternion = Quaternion::new(rw, rx, ry, rz);
            let unit_quaternion = UnitQuaternion::from_quaternion(quaternion);

            let quaternion_mat4 = unit_quaternion.quaternion().coords;

            let joint_translatio: Translation3<f32> = Translation3::new(tx, ty, tz);

            let inv_bindmat =
                (unit_quaternion.to_homogeneous()) * (joint_translatio.to_homogeneous());
            let iv_x = inv_bindmat.m14;
            let iv_y = inv_bindmat.m24;
            let iv_z = inv_bindmat.m34;

            let trans_point = Translation3::new(iv_x, iv_y, iv_z).inverse();

            let parent_id = skin_buffer.get_u8();

            let joint = RDJoint {
                name: joint_name,
                nameptr,
                transition: [trans_point.x, trans_point.y, trans_point.z],
                quaternion: [
                    quaternion_mat4.x,
                    quaternion_mat4.y,
                    quaternion_mat4.z,
                    quaternion_mat4.w,
                ],
                parent: parent_id,
                locked: false,
            };

            joints_vec.push(joint);
            skin_buffer.advance(84 - 33);
        }

        self.joints = Some(joints_vec);
    }

    fn new(buf: Vec<u8>) -> Self {
        RDModell::check_has_magic_byte(&buf);

        let size = buf.len() as u32;
        let buffer = Bytes::from(buf);
        let mut nbuffer = buffer.clone();

        nbuffer.advance(RDModell::META_OFFSET as usize);
        let meta = nbuffer.get_u32_le();

        nbuffer.get_u32_le();
        let skin_there = nbuffer.get_u32_le() > 0;

        nbuffer.seek(meta, size);
        nbuffer.advance(RDModell::VERTEX_META as usize);
        let vertex_offset = nbuffer.get_u32_le();

        let triangles_offset = nbuffer.get_u32_le();

        let vertex_count_off = vertex_offset - RDModell::META_COUNT;
        info!("off : {}", vertex_count_off);
        nbuffer.seek(vertex_count_off, size);

        let vertices_count = nbuffer.get_u32_le();
        let vertex_buffer_size = nbuffer.get_u32_le();

        let vert_read_buf = nbuffer.clone();

        let vertices_vec = RDModell::read_vertices_vec(
            vertex_buffer_size,
            vertices_count,
            skin_there,
            vert_read_buf,
        );

        let triangles_count_off = triangles_offset - RDModell::META_COUNT;
        nbuffer.seek(triangles_count_off, size);
        let triangles_idx_count = nbuffer.get_u32_le();
        let triangles_idx_size = nbuffer.get_u32_le();

        // read indices for triangles
        assert_eq!(triangles_idx_size, 2);
        assert_eq!(triangles_idx_count % 3, 0);

        //let mut triangles_idx_buffer = nbuffer.clone();
        let mut triangles_idx_buffer = nbuffer;
        triangles_idx_buffer.truncate((triangles_idx_size * triangles_idx_count) as usize);
        let triangles_real_count = triangles_idx_count / 3;
        let mut triangles = Vec::with_capacity(triangles_real_count as usize);
        for _ in 0..triangles_real_count {
            let t = Triangle {
                indices: [
                    triangles_idx_buffer.get_u16_le(),
                    triangles_idx_buffer.get_u16_le(),
                    triangles_idx_buffer.get_u16_le(),
                ],
            };
            triangles.push(t);
        }

        RDModell {
            size,
            buffer,
            joints: None,
            vertices: vertices_vec.unwrap(),
            triangle_indices: triangles,
            meta,
            vertex_offset,
            vertices_count,
            vertex_buffer_size,

            triangles_offset,
            triangles_idx_count,
            triangles_idx_size,

            anim: None,
            mat: None,
        }
    }

    #[cfg(target_endian = "little")]
    fn read_vertices_vec(
        vertex_buffer_size: u32,
        vertices_count: u32,
        skin_there: bool,
        mut vert_read_buf: Bytes,
    ) -> Option<VertexFormat> {
        vert_read_buf.truncate((vertices_count * vertex_buffer_size) as usize);
        assert_eq!(vert_read_buf.remaining() % vertex_buffer_size as usize, 0);

        let out = match vertex_buffer_size {
            VertexFormatSize::P4h => {
                let mut x = Vec::with_capacity(vertices_count as usize);
                unsafe {
                    let dst = slice::from_raw_parts_mut(
                        x.as_mut_ptr() as *mut u8,
                        vert_read_buf.remaining(),
                    );
                    vert_read_buf.copy_to_slice(dst);
                    x.set_len(vertices_count as usize);
                }
                info!(
                    "Read {} vertices of type {} ({} bytes)",
                    x.len(),
                    "P4h",
                    vertex_buffer_size
                );
                Some(VertexFormat::P4h(x))
            }
            VertexFormatSize::P4h_N4b_T2h if !skin_there => {
                let mut x = Vec::with_capacity(vertices_count as usize);
                unsafe {
                    let dst = slice::from_raw_parts_mut(
                        x.as_mut_ptr() as *mut u8,
                        vert_read_buf.remaining(),
                    );
                    vert_read_buf.copy_to_slice(dst);
                    x.set_len(vertices_count as usize);
                }
                info!(
                    "Read {} vertices of type {} ({} bytes)",
                    x.len(),
                    "P4h_N4b_T2h_I4b",
                    vertex_buffer_size
                );
                Some(VertexFormat::P4h_N4b_T2h(x))
            }
            VertexFormatSize::P4h_N4b_T2h_I4b if skin_there => {
                let mut x = Vec::with_capacity(vertices_count as usize);
                unsafe {
                    let dst = slice::from_raw_parts_mut(
                        x.as_mut_ptr() as *mut u8,
                        vert_read_buf.remaining(),
                    );
                    vert_read_buf.copy_to_slice(dst);
                    x.set_len(vertices_count as usize);
                }
                info!(
                    "Read {} vertices of type {} ({} bytes)",
                    x.len(),
                    "P4h_N4b_T2h_I4b",
                    vertex_buffer_size
                );
                Some(VertexFormat::P4h_N4b_T2h_I4b(x))
            }
            VertexFormatSize::P4h_N4b_G4b_B4b_T2h if !skin_there => {
                let mut x = Vec::with_capacity(vertices_count as usize);
                unsafe {
                    let dst = slice::from_raw_parts_mut(
                        x.as_mut_ptr() as *mut u8,
                        vert_read_buf.remaining(),
                    );
                    vert_read_buf.copy_to_slice(dst);
                    x.set_len(vertices_count as usize);
                }
                info!(
                    "Read {} vertices of type {} ({} bytes)",
                    x.len(),
                    "P4h_N4b_G4b_B4b_T2h",
                    vertex_buffer_size
                );
                Some(VertexFormat::P4h_N4b_G4b_B4b_T2h(x))
            }
            VertexFormatSize::P4h_N4b_T2h_I4b_W4b if skin_there => {
                let mut x = Vec::with_capacity(vertices_count as usize);
                unsafe {
                    let dst = slice::from_raw_parts_mut(
                        x.as_mut_ptr() as *mut u8,
                        vert_read_buf.remaining(),
                    );
                    vert_read_buf.copy_to_slice(dst);
                    x.set_len(vertices_count as usize);
                }
                info!(
                    "Read {} vertices of type {} ({} bytes)",
                    x.len(),
                    "P4h_N4b_T2h_I4b_W4b",
                    vertex_buffer_size
                );
                Some(VertexFormat::P4h_N4b_T2h_I4b_W4b(x))
            }
            VertexFormatSize::P4h_N4b_G4b_B4b_T2h_I4b if skin_there => {
                let mut x = Vec::with_capacity(vertices_count as usize);
                unsafe {
                    let dst = slice::from_raw_parts_mut(
                        x.as_mut_ptr() as *mut u8,
                        vert_read_buf.remaining(),
                    );
                    vert_read_buf.copy_to_slice(dst);
                    x.set_len(vertices_count as usize);
                }
                info!(
                    "Read {} vertices of type {} ({} bytes)",
                    x.len(),
                    "P4h_N4b_G4b_B4b_T2h_I4b",
                    vertex_buffer_size
                );
                Some(VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(x))
            }
            VertexFormatSize::P4h_N4b_T2h_C4c if !skin_there => {
                let mut x = Vec::with_capacity(vertices_count as usize);
                unsafe {
                    let dst = slice::from_raw_parts_mut(
                        x.as_mut_ptr() as *mut u8,
                        vert_read_buf.remaining(),
                    );
                    vert_read_buf.copy_to_slice(dst);
                    x.set_len(vertices_count as usize);
                }
                info!(
                    "Read {} vertices of type {} ({} bytes)",
                    x.len(),
                    "P4h_N4b_T2h_C4c",
                    vertex_buffer_size
                );
                Some(VertexFormat::P4h_N4b_T2h_C4c(x))
            }
            VertexFormatSize::P4h_N4b_G4b_B4b_T2h_C4c if !skin_there => {
                let mut x = Vec::with_capacity(vertices_count as usize);
                unsafe {
                    let dst = slice::from_raw_parts_mut(
                        x.as_mut_ptr() as *mut u8,
                        vert_read_buf.remaining(),
                    );
                    vert_read_buf.copy_to_slice(dst);
                    x.set_len(vertices_count as usize);
                }
                info!(
                    "Read {} vertices of type {} ({} bytes)",
                    x.len(),
                    "P4h_N4b_G4b_B4b_T2h_C4c",
                    vertex_buffer_size
                );
                Some(VertexFormat::P4h_N4b_G4b_B4b_T2h_C4c(x))
            }
            VertexFormatSize::P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b if skin_there => {
                let mut x = Vec::with_capacity(vertices_count as usize);
                unsafe {
                    let dst = slice::from_raw_parts_mut(
                        x.as_mut_ptr() as *mut u8,
                        vert_read_buf.remaining(),
                    );
                    vert_read_buf.copy_to_slice(dst);
                    x.set_len(vertices_count as usize);
                }
                info!(
                    "Read {} vertices of type {} ({} bytes)",
                    x.len(),
                    "P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b",
                    vertex_buffer_size
                );
                Some(VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b(x))
            }
            _ => unimplemented!("vertices use unrecognized size of {}", vertex_buffer_size),
        };
        assert_eq!(vert_read_buf.remaining(), 0);
        out
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Triangle {
    indices: [u16; 3],
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct P4h {
    pos: [f16; 4],
}
#[derive(Clone, Debug)]
#[repr(C)]
pub struct N4b {
    normals: [u8; 4],
}
#[derive(Clone, Debug)]
#[repr(C)]
pub struct G4b {
    tangent: [u8; 4],
}
#[derive(Clone, Debug)]
#[repr(C)]
pub struct B4b {
    binormal: [u8; 4],
}
#[derive(Clone, Debug)]
#[repr(C)]
pub struct T2h {
    tex: [f16; 2],
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct I4b {
    blend_idx: [u8; 4],
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct W4b {
    blend_weight: [u8; 4],
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct C4c {
    unknown: [u8; 4],
}

#[derive(Clone, Debug)]
#[repr(C)]
pub struct VertexGeneric<N, G, B, T, C, I0, I1, I2, I3, W0, W1, W2, W3> {
    p4h: P4h,
    n4b: N,
    g4b: G,
    b4b: B,
    t2h: T,
    c4c: C,
    i4b0: I0,
    i4b1: I1,
    i4b2: I2,
    i4b3: I3,
    w4b0: W0,
    w4b1: W1,
    w4b2: W2,
    w4b3: W3,
}

impl<N, G, B, T, C, I0, I1, I2, I3, W0, W1, W2, W3>
    VertexGeneric<N, G, B, T, C, I0, I1, I2, I3, W0, W1, W2, W3>
{
    fn get_p4h(&self) -> &P4h {
        &self.p4h
    }
}

impl<G, B, T, C, I0, I1, I2, I3, W0, W1, W2, W3>
    VertexGeneric<N4b, G, B, T, C, I0, I1, I2, I3, W0, W1, W2, W3>
{
    fn get_n4b(&self) -> &N4b {
        &self.n4b
    }
}

impl<N, G, B, T, C, I0, I1, I2, I3, W1, W2, W3>
    VertexGeneric<N, G, B, T, C, I0, I1, I2, I3, W4b, W1, W2, W3>
{
    fn get_w4b(&self) -> &W4b {
        &self.w4b0
    }
}

impl<N, G, B, T, C, I1, I2, I3, W0, W1, W2, W3>
    VertexGeneric<N, G, B, T, C, I4b, I1, I2, I3, W0, W1, W2, W3>
{
    fn get_i4b(&self) -> &I4b {
        &self.i4b0
    }
}

impl<N, G, B, C, I0, I1, I2, I3, W0, W1, W2, W3>
    VertexGeneric<N, G, B, T2h, C, I0, I1, I2, I3, W0, W1, W2, W3>
{
    fn get_t2h(&self) -> &T2h {
        &self.t2h
    }
}

#[allow(non_camel_case_types)]
#[allow(dead_code)]
type P4h_ = VertexGeneric<(), (), (), (), (), (), (), (), (), (), (), (), ()>;

#[allow(non_camel_case_types)]
#[allow(dead_code)]
type P4h_N4b_T2h = VertexGeneric<N4b, (), (), T2h, (), (), (), (), (), (), (), (), ()>;

#[allow(non_camel_case_types)]
#[allow(dead_code)]
type P4h_N4b_T2h_C4c = VertexGeneric<N4b, (), (), T2h, C4c, (), (), (), (), (), (), (), ()>;

#[allow(non_camel_case_types)]
#[allow(dead_code)]
type P4h_N4b_T2h_I4b = VertexGeneric<N4b, (), (), T2h, (), I4b, (), (), (), (), (), (), ()>;

#[allow(non_camel_case_types)]
#[allow(dead_code)]
type P4h_N4b_G4b_B4b_T2h = VertexGeneric<N4b, G4b, B4b, T2h, (), (), (), (), (), (), (), (), ()>;

#[allow(non_camel_case_types)]
#[allow(dead_code)]
type P4h_N4b_G4b_B4b_T2h_C4c =
    VertexGeneric<N4b, G4b, B4b, T2h, C4c, (), (), (), (), (), (), (), ()>;

#[allow(non_camel_case_types)]
#[allow(dead_code)]
type P4h_N4b_T2h_I4b_W4b = VertexGeneric<N4b, (), (), T2h, (), I4b, (), (), (), W4b, (), (), ()>;

#[allow(non_camel_case_types)]
#[allow(dead_code)]
type P4h_N4b_G4b_B4b_T2h_I4b =
    VertexGeneric<N4b, G4b, B4b, T2h, (), I4b, (), (), (), (), (), (), ()>;

#[allow(non_camel_case_types)]
#[allow(dead_code)]
type P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b =
    VertexGeneric<N4b, G4b, B4b, T2h, (), I4b, I4b, I4b, I4b, W4b, W4b, W4b, W4b>;

#[derive(Debug)]
#[repr(C)]
#[allow(non_camel_case_types)]
pub enum VertexFormat {
    P4h(Vec<P4h_>),
    P4h_N4b_T2h(Vec<P4h_N4b_T2h>),
    P4h_N4b_T2h_C4c(Vec<P4h_N4b_T2h_C4c>),
    P4h_N4b_T2h_I4b(Vec<P4h_N4b_T2h_I4b>),
    P4h_N4b_G4b_B4b_T2h(Vec<P4h_N4b_G4b_B4b_T2h>),
    P4h_N4b_T2h_I4b_W4b(Vec<P4h_N4b_T2h_I4b_W4b>),
    P4h_N4b_G4b_B4b_T2h_C4c(Vec<P4h_N4b_G4b_B4b_T2h_C4c>),
    P4h_N4b_G4b_B4b_T2h_I4b(Vec<P4h_N4b_G4b_B4b_T2h_I4b>),
    P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b(
        Vec<P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b>,
    ),
}

impl VertexFormat {
    pub fn len(&self) -> usize {
        match self {
            VertexFormat::P4h(v) => v.len(),
            VertexFormat::P4h_N4b_T2h(v) => v.len(),
            VertexFormat::P4h_N4b_T2h_I4b(v) => v.len(),
            VertexFormat::P4h_N4b_G4b_B4b_T2h(v) => v.len(),
            VertexFormat::P4h_N4b_T2h_I4b_W4b(v) => v.len(),
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(v) => v.len(),
            VertexFormat::P4h_N4b_T2h_C4c(v) => v.len(),
            VertexFormat::P4h_N4b_G4b_B4b_T2h_C4c(v) => v.len(),
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    // https://stackoverflow.com/questions/25445761/returning-a-closure-from-a-function
    // https://stackoverflow.com/questions/27535289/what-is-the-correct-way-to-return-an-iterator-or-any-other-trait
    fn iter_p4h(&self) -> Box<dyn Iterator<Item = &'_ P4h> + '_> {
        match self {
            VertexFormat::P4h(v) => {
                let n = v.iter().map(|x| x.get_p4h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_T2h(v) => {
                let n = v.iter().map(|x| x.get_p4h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_T2h_I4b(v) => {
                let n = v.iter().map(|x| x.get_p4h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_G4b_B4b_T2h(v) => {
                let n = v.iter().map(|x| x.get_p4h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_T2h_I4b_W4b(v) => {
                let n = v.iter().map(|x| x.get_p4h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(v) => {
                let n = v.iter().map(|x| x.get_p4h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_T2h_C4c(v) => {
                let n = v.iter().map(|x| x.get_p4h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_G4b_B4b_T2h_C4c(v) => {
                let n = v.iter().map(|x| x.get_p4h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b(v) => {
                let n = v.iter().map(|x| x.get_p4h());
                Box::new(n)
            }
        }
    }

    fn iter_t2h(&self) -> Box<dyn Iterator<Item = &'_ T2h> + '_> {
        match self {
            VertexFormat::P4h_N4b_T2h(v) => {
                let n = v.iter().map(|x| x.get_t2h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_T2h_I4b(v) => {
                let n = v.iter().map(|x| x.get_t2h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_G4b_B4b_T2h(v) => {
                let n = v.iter().map(|x| x.get_t2h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_T2h_I4b_W4b(v) => {
                let n = v.iter().map(|x| x.get_t2h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(v) => {
                let n = v.iter().map(|x| x.get_t2h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_T2h_C4c(v) => {
                let n = v.iter().map(|x| x.get_t2h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_G4b_B4b_T2h_C4c(v) => {
                let n = v.iter().map(|x| x.get_t2h());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b(v) => {
                let n = v.iter().map(|x| x.get_t2h());
                Box::new(n)
            }
            _ => unimplemented!("tex / uv"),
        }
    }

    fn iter_n4b(&self) -> Box<dyn Iterator<Item = &'_ N4b> + '_> {
        match self {
            VertexFormat::P4h_N4b_T2h(v) => {
                let n = v.iter().map(|x| x.get_n4b());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_T2h_I4b(v) => {
                let n = v.iter().map(|x| x.get_n4b());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_G4b_B4b_T2h(v) => {
                let n = v.iter().map(|x| x.get_n4b());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_T2h_I4b_W4b(v) => {
                let n = v.iter().map(|x| x.get_n4b());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(v) => {
                let n = v.iter().map(|x| x.get_n4b());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_T2h_C4c(v) => {
                let n = v.iter().map(|x| x.get_n4b());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_G4b_B4b_T2h_C4c(v) => {
                let n = v.iter().map(|x| x.get_n4b());
                Box::new(n)
            }
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b(v) => {
                let n = v.iter().map(|x| x.get_n4b());
                Box::new(n)
            }
            _ => unimplemented!("normal"),
        }
    }
}

struct VertexFormatSize;

#[allow(non_upper_case_globals)]
impl VertexFormatSize {
    const P4h: u32 = 8;
    const P4h_N4b_T2h: u32 = 16;
    const P4h_N4b_T2h_C4c: u32 = 20;
    const P4h_N4b_T2h_I4b: u32 = 20;
    const P4h_N4b_G4b_B4b_T2h: u32 = 24;
    const P4h_N4b_T2h_I4b_W4b: u32 = 24;
    const P4h_N4b_G4b_B4b_T2h_C4c: u32 = 28;
    const P4h_N4b_G4b_B4b_T2h_I4b: u32 = 28;
    const P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b: u32 = 56;
}

impl From<&Path> for RDModell {
    fn from(f_path: &Path) -> Self {
        let mut f = File::open(f_path).unwrap();
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut buffer).ok();

        let buffer_len = buffer.len();
        info!("loaded {:?} into buffer", f_path.to_str().unwrap());

        info!("buffer size: {}", buffer_len);
        let rd = RDModell::new(buffer);
        rd.check_multi_mesh();
        rd
    }
}

impl From<&str> for RDModell {
    fn from(str_path: &str) -> Self {
        RDModell::from(Path::new(str_path))
    }
}

impl From<&String> for RDModell {
    fn from(string_path: &String) -> Self {
        RDModell::from(Path::new(string_path))
    }
}

#[cfg(test)]
mod tests_intern {

    use super::*;
    use std::mem;

    #[test]
    fn fishery_others_lod2() {
        // for Miri test
        // fishery_others_lod2.rdm
        let v = vec![
            0x52, 0x44, 0x4D, 0x01, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
            0x00, 0x00, 0x1C, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00,
            0x54, 0x00, 0x00, 0x00, 0x29, 0x01, 0x00, 0x00, 0xE3, 0x03, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x48, 0x00, 0x00, 0x00,
            0xA4, 0x00, 0x00, 0x00, 0x17, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x6B, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x47, 0x3A, 0x5C, 0x67,
            0x72, 0x61, 0x70, 0x68, 0x69, 0x63, 0x5F, 0x62, 0x61, 0x63, 0x6B, 0x75, 0x70, 0x5C,
            0x74, 0x6F, 0x62, 0x69, 0x61, 0x73, 0x5C, 0x61, 0x6E, 0x6E, 0x6F, 0x35, 0x5C, 0x61,
            0x73, 0x73, 0x65, 0x74, 0x73, 0x5C, 0x62, 0x75, 0x69, 0x6C, 0x64, 0x69, 0x6E, 0x67,
            0x73, 0x5C, 0x6F, 0x74, 0x68, 0x65, 0x72, 0x73, 0x5C, 0x66, 0x69, 0x73, 0x68, 0x65,
            0x72, 0x79, 0x5F, 0x6F, 0x74, 0x68, 0x65, 0x72, 0x73, 0x5C, 0x70, 0x6F, 0x6C, 0x69,
            0x73, 0x68, 0x5C, 0x75, 0x6D, 0x62, 0x61, 0x75, 0x5C, 0x66, 0x69, 0x73, 0x68, 0x65,
            0x72, 0x79, 0x5F, 0x75, 0x6D, 0x62, 0x61, 0x75, 0x5F, 0x62, 0x61, 0x6B, 0x69, 0x6E,
            0x67, 0x2E, 0x6D, 0x61, 0x78, 0x0A, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x43,
            0x75, 0x74, 0x6F, 0x75, 0x74, 0x2E, 0x72, 0x6D, 0x70, 0x01, 0x00, 0x00, 0x00, 0x5C,
            0x00, 0x00, 0x00, 0x8D, 0x01, 0x00, 0x00, 0xBF, 0x01, 0x00, 0x00, 0xF7, 0x01, 0x00,
            0x00, 0x37, 0x02, 0x00, 0x00, 0x3F, 0x03, 0x00, 0x00, 0x13, 0x02, 0x00, 0x00, 0xFF,
            0xFF, 0xFF, 0xFF, 0x00, 0x20, 0xBE, 0xBF, 0x00, 0x80, 0xE3, 0xBE, 0x00, 0xE0, 0x59,
            0xC0, 0x00, 0x00, 0xBA, 0x3F, 0x00, 0x80, 0xCF, 0xBE, 0x00, 0xC0, 0x03, 0xBF, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
            0x00, 0x1C, 0x00, 0x00, 0x00, 0xB1, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x63,
            0x75, 0x74, 0x6F, 0x75, 0x74, 0x01, 0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0xDF,
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x10,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00, 0x03,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x1C, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x4E, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20,
            0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0xD0, 0x3D, 0x12, 0xB7, 0xDC, 0xBA, 0x00,
            0x00, 0xCC, 0x3D, 0x12, 0xB7, 0x3A, 0xBC, 0x00, 0x00, 0x25, 0x3D, 0x12, 0xB7, 0xF2,
            0xBC, 0x00, 0x00, 0x8D, 0x35, 0x12, 0xB7, 0xDC, 0xBA, 0x00, 0x00, 0x8D, 0x35, 0x12,
            0xB7, 0xEE, 0xBC, 0x00, 0x00, 0x9E, 0x32, 0x7C, 0xB6, 0x89, 0xBD, 0x00, 0x00, 0x9E,
            0x32, 0x7C, 0xB6, 0x24, 0xC0, 0x00, 0x00, 0x9E, 0x32, 0x7C, 0xB6, 0x07, 0xC1, 0x00,
            0x00, 0x9E, 0x32, 0x7C, 0xB6, 0xEA, 0xC1, 0x00, 0x00, 0x8D, 0x30, 0x7C, 0xB6, 0x89,
            0xBD, 0x00, 0x00, 0x8D, 0x30, 0x7C, 0xB6, 0x24, 0xC0, 0x00, 0x00, 0x8D, 0x30, 0x7C,
            0xB6, 0x07, 0xC1, 0x00, 0x00, 0x8D, 0x30, 0x7C, 0xB6, 0xEA, 0xC1, 0x00, 0x00, 0x14,
            0xAE, 0x7C, 0xB6, 0xCF, 0xC2, 0x00, 0x00, 0xB5, 0xAF, 0x7C, 0xB6, 0xAE, 0xC2, 0x00,
            0x00, 0x8B, 0xB4, 0x12, 0xB7, 0xDC, 0xBA, 0x00, 0x00, 0x90, 0xB4, 0x12, 0xB7, 0xB6,
            0xBF, 0x00, 0x00, 0x93, 0xB4, 0x16, 0xB7, 0xCD, 0xC1, 0x00, 0x00, 0x57, 0xB7, 0x1C,
            0xB7, 0x24, 0xC2, 0x00, 0x00, 0x45, 0xBC, 0x1C, 0xB7, 0x24, 0xC2, 0x00, 0x00, 0x9E,
            0xBC, 0x7C, 0xB6, 0xAE, 0xC2, 0x00, 0x00, 0xB8, 0xBC, 0x7C, 0xB6, 0xCF, 0xC2, 0x00,
            0x00, 0xF6, 0xBC, 0x16, 0xB7, 0xCD, 0xC1, 0x00, 0x00, 0xF7, 0xBC, 0x12, 0xB7, 0x62,
            0xBD, 0x00, 0x00, 0xAF, 0xBD, 0x7C, 0xB6, 0xDB, 0xC1, 0x00, 0x00, 0xAF, 0xBD, 0x7C,
            0xB6, 0x5D, 0xBE, 0x00, 0x00, 0xAF, 0xBD, 0x7C, 0xB6, 0xD9, 0xC0, 0x00, 0x00, 0xAF,
            0xBD, 0x7C, 0xB6, 0x55, 0xB8, 0x00, 0x00, 0xF1, 0xBD, 0x7C, 0xB6, 0xEA, 0xC1, 0x00,
            0x00, 0xF1, 0xBD, 0x7C, 0xB6, 0xD9, 0xC0, 0x00, 0x00, 0xF1, 0xBD, 0x7C, 0xB6, 0x5D,
            0xBE, 0x00, 0x00, 0xF1, 0xBD, 0x7C, 0xB6, 0x1E, 0xB8, 0x00, 0x00, 0x4E, 0x00, 0x00,
            0x00, 0x02, 0x00, 0x00, 0x00, 0x16, 0x00, 0x10, 0x00, 0x11, 0x00, 0x17, 0x00, 0x10,
            0x00, 0x16, 0x00, 0x17, 0x00, 0x0F, 0x00, 0x10, 0x00, 0x04, 0x00, 0x01, 0x00, 0x02,
            0x00, 0x03, 0x00, 0x01, 0x00, 0x04, 0x00, 0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x1D,
            0x00, 0x19, 0x00, 0x1A, 0x00, 0x19, 0x00, 0x1D, 0x00, 0x1E, 0x00, 0x18, 0x00, 0x15,
            0x00, 0x1C, 0x00, 0x15, 0x00, 0x18, 0x00, 0x14, 0x00, 0x14, 0x00, 0x0D, 0x00, 0x15,
            0x00, 0x0D, 0x00, 0x14, 0x00, 0x0E, 0x00, 0x09, 0x00, 0x06, 0x00, 0x0A, 0x00, 0x06,
            0x00, 0x09, 0x00, 0x05, 0x00, 0x0C, 0x00, 0x0D, 0x00, 0x0E, 0x00, 0x0D, 0x00, 0x0C,
            0x00, 0x08, 0x00, 0x10, 0x00, 0x03, 0x00, 0x04, 0x00, 0x03, 0x00, 0x10, 0x00, 0x0F,
            0x00, 0x16, 0x00, 0x12, 0x00, 0x13, 0x00, 0x12, 0x00, 0x16, 0x00, 0x11, 0x00, 0x0C,
            0x00, 0x07, 0x00, 0x08, 0x00, 0x07, 0x00, 0x0C, 0x00, 0x0B, 0x00, 0x1D, 0x00, 0x18,
            0x00, 0x1C, 0x00, 0x18, 0x00, 0x1D, 0x00, 0x1A, 0x00, 0x1B, 0x00, 0x1E, 0x00, 0x1F,
            0x00, 0x1E, 0x00, 0x1B, 0x00, 0x19, 0x00, 0x01, 0x00, 0x00, 0x00, 0x1C, 0x00, 0x00,
            0x00, 0x07, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00, 0x3F, 0x04, 0x00, 0x00, 0x4E,
            0x04, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x07, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x66, 0x69, 0x73, 0x68, 0x65,
            0x72, 0x79, 0x5E, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x67, 0x3A, 0x2F, 0x67,
            0x72, 0x61, 0x70, 0x68, 0x69, 0x63, 0x5F, 0x62, 0x61, 0x63, 0x6B, 0x75, 0x70, 0x2F,
            0x74, 0x6F, 0x62, 0x69, 0x61, 0x73, 0x2F, 0x61, 0x6E, 0x6E, 0x6F, 0x35, 0x2F, 0x61,
            0x73, 0x73, 0x65, 0x74, 0x73, 0x2F, 0x62, 0x75, 0x69, 0x6C, 0x64, 0x69, 0x6E, 0x67,
            0x73, 0x2F, 0x6F, 0x74, 0x68, 0x65, 0x72, 0x73, 0x2F, 0x66, 0x69, 0x73, 0x68, 0x65,
            0x72, 0x79, 0x5F, 0x6F, 0x74, 0x68, 0x65, 0x72, 0x73, 0x2F, 0x70, 0x6F, 0x6C, 0x69,
            0x73, 0x68, 0x2F, 0x75, 0x6D, 0x62, 0x61, 0x75, 0x2F, 0x64, 0x69, 0x66, 0x66, 0x75,
            0x73, 0x65, 0x2E, 0x70, 0x73, 0x64,
        ];

        let rdm = RDModell::new(v);
        assert_eq!(rdm.vertices_count, 32);
        assert_eq!(rdm.triangles_idx_count, 78);

        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );
    }

    #[test]
    fn vertex_generic_size_of() {
        assert_eq!(mem::size_of::<P4h_>(), VertexFormatSize::P4h as usize);
        assert_eq!(
            mem::size_of::<P4h_N4b_T2h>(),
            VertexFormatSize::P4h_N4b_T2h as usize
        );
        assert_eq!(
            mem::size_of::<P4h_N4b_T2h_C4c>(),
            VertexFormatSize::P4h_N4b_T2h_C4c as usize
        );
        assert_eq!(
            mem::size_of::<P4h_N4b_T2h_I4b>(),
            VertexFormatSize::P4h_N4b_T2h_I4b as usize
        );
        assert_eq!(
            mem::size_of::<P4h_N4b_G4b_B4b_T2h>(),
            VertexFormatSize::P4h_N4b_G4b_B4b_T2h as usize
        );
        assert_eq!(
            mem::size_of::<P4h_N4b_T2h_I4b_W4b>(),
            VertexFormatSize::P4h_N4b_T2h_I4b_W4b as usize
        );
        assert_eq!(
            mem::size_of::<P4h_N4b_G4b_B4b_T2h_C4c>(),
            VertexFormatSize::P4h_N4b_G4b_B4b_T2h_C4c as usize
        );
        assert_eq!(
            mem::size_of::<P4h_N4b_G4b_B4b_T2h_I4b>(),
            VertexFormatSize::P4h_N4b_G4b_B4b_T2h_I4b as usize
        );

        assert_eq!(
            mem::size_of::<P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b>(),
            VertexFormatSize::P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b as usize
        );
    }

    #[test]
    fn vertex_generic_offset() {
        assert_eq!(offset_of!(P4h_N4b_G4b_B4b_T2h_I4b, n4b), 8);
        assert_eq!(offset_of!(P4h_N4b_G4b_B4b_T2h_I4b, g4b), 12);
        assert_eq!(offset_of!(P4h_N4b_G4b_B4b_T2h_I4b, b4b), 16);
        assert_eq!(offset_of!(P4h_N4b_G4b_B4b_T2h_I4b, t2h), 20);
        assert_eq!(offset_of!(P4h_N4b_G4b_B4b_T2h_I4b, i4b0), 24);
    }

    #[test]
    fn vertex_generic_offset2() {
        assert_eq!(offset_of!(P4h_N4b_T2h_I4b_W4b, n4b), 8);
        assert_eq!(offset_of!(P4h_N4b_T2h_I4b_W4b, t2h), 12);
        assert_eq!(offset_of!(P4h_N4b_T2h_I4b_W4b, i4b0), 16);
        assert_eq!(offset_of!(P4h_N4b_T2h_I4b_W4b, w4b0), 20);
    }

    #[test]
    fn vertex_generic_offset3() {
        assert_eq!(offset_of!(P4h_N4b_G4b_B4b_T2h, n4b), 8);
        assert_eq!(offset_of!(P4h_N4b_G4b_B4b_T2h, g4b), 12);
        assert_eq!(offset_of!(P4h_N4b_G4b_B4b_T2h, b4b), 16);
        assert_eq!(offset_of!(P4h_N4b_G4b_B4b_T2h, t2h), 20);
    }

    #[test]
    fn joint_16_anno_7() {
        assert_eq!(
            offset_of!(P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b, n4b),
            8
        );
        assert_eq!(
            offset_of!(P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b, g4b),
            12
        );
        assert_eq!(
            offset_of!(P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b, b4b),
            16
        );
        assert_eq!(
            offset_of!(P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b, t2h),
            20
        );
        assert_eq!(
            offset_of!(P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b, i4b0),
            24
        );
        assert_eq!(
            offset_of!(P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b, i4b1),
            28
        );
        assert_eq!(
            offset_of!(P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b, i4b2),
            32
        );
        assert_eq!(
            offset_of!(P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b, i4b3),
            36
        );

        assert_eq!(
            offset_of!(P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b, w4b0),
            40
        );
        assert_eq!(
            offset_of!(P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b, w4b1),
            44
        );
        assert_eq!(
            offset_of!(P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b, w4b2),
            48
        );
        assert_eq!(
            offset_of!(P4h_N4b_G4b_B4b_T2h_I4b_I4b_I4b_I4b_W4b_W4b_W4b_W4b, w4b3),
            52
        );
    }
}
