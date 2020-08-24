use bytes::{Buf, Bytes};
use std::path::Path;

use std::fs::File;

use std::str;

use half::f16;
use std::fmt;

use nalgebra::*;

#[macro_use]
extern crate log;
use env_logger::Env;

#[macro_use]
extern crate approx;

pub mod gltf_export;
pub mod gltf_reader;
pub mod rdm_anim;
pub mod rdm_writer;
pub mod rdm_anim_writer;
use crate::rdm_anim::RDAnim;

pub struct RDModell {
    size: u32,
    buffer: Bytes,
    pub joints: Option<Vec<RDJoint>>,
    vertices: Vec<VertexFormat>,
    pub triangle_indices: Vec<Triangle>,

    meta: u32,
    vertex_offset: u32,
    pub vertices_count: u32,
    vertex_buffer_size: u32,

    triangles_offset: u32,
    pub triangles_idx_count: u32,
    triangles_idx_size: u32,

    anim: Option<RDAnim>,
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

#[derive(Debug, Clone)]
pub struct RDJoint {
    name: String,
    nameptr: u32,
    transition: [f32; 3],
    quaternion: [f32; 4],
    parent: u8,
    locked: bool,
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

    pub fn add_skin(&mut self) {
        let mut skin_buffer = self.buffer.clone();
        skin_buffer.advance(40);
        let skin_offset = skin_buffer.get_u32_le();
        assert_eq!(skin_offset != 0, true);

        let rel_skin_offset: usize =
            (skin_offset - (self.size - skin_buffer.remaining() as u32)) as usize;
        skin_buffer.advance(rel_skin_offset);
        let first_skin_offset = skin_buffer.get_u32_le();

        let joint_count_ptr = first_skin_offset - RDModell::META_COUNT;
        let rel_joint_count: usize =
            (joint_count_ptr - (self.size - skin_buffer.remaining() as u32)) as usize;
        skin_buffer.advance(rel_joint_count);

        let joint_count = skin_buffer.get_u32_le();
        let joint_size = skin_buffer.get_u32_le();

        let mut joints_vec: Vec<RDJoint> = Vec::with_capacity(joint_count as usize);

        let mut joint_name_buffer = skin_buffer.clone();

        let len_first_joint_name_ptr = joint_name_buffer.get_u32_le() - RDModell::META_COUNT;
        let rel_len_first_joint_name_ptr: usize = (len_first_joint_name_ptr
            - (self.size - joint_name_buffer.remaining() as u32))
            as usize;
        joint_name_buffer.advance(rel_len_first_joint_name_ptr);

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
        let size = buf.len() as u32;
        let buffer = Bytes::from(buf);
        let mut nbuffer = buffer.clone();

        nbuffer.advance(RDModell::META_OFFSET as usize);
        let meta = nbuffer.get_u32_le();

        nbuffer.get_u32_le();
        let skin_there = nbuffer.get_u32_le() > 0;

        nbuffer.advance((meta - (size - nbuffer.remaining() as u32)) as usize);
        nbuffer.advance(RDModell::VERTEX_META as usize);
        let vertex_offset = nbuffer.get_u32_le();

        let triangles_offset = nbuffer.get_u32_le();

        let vertex_count_off = vertex_offset - RDModell::META_COUNT;
        info!("off : {}", vertex_count_off);
        nbuffer.advance((vertex_count_off - (size - nbuffer.remaining() as u32)) as usize);
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
        nbuffer.advance((triangles_count_off - (size - nbuffer.remaining() as u32)) as usize);
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
        }
    }

    fn read_vertices_vec(
        vertex_buffer_size: u32,
        vertices_count: u32,
        skin_there: bool,
        vert_read_buf: Bytes,
    ) -> Option<Vec<VertexFormat>> {
        match vertex_buffer_size {
            VertexFormatSize::P4h => {
                RDModell::read_p4h(vertex_buffer_size, vertices_count, vert_read_buf)
            }
            VertexFormatSize::P4h_N4b_T2h if !skin_there => {
                RDModell::read_p4h_n4b_t2h(vertex_buffer_size, vertices_count, vert_read_buf)
            }
            VertexFormatSize::P4h_N4b_T2h_C4c if !skin_there => {
                RDModell::read_p4h_n4b_t2h_c4c(vertex_buffer_size, vertices_count, vert_read_buf)
            }
            VertexFormatSize::P4h_N4b_T2h_I4b if skin_there => {
                RDModell::read_p4h_n4b_t2h_i4b(vertex_buffer_size, vertices_count, vert_read_buf)
            }
            VertexFormatSize::P4h_N4b_G4b_B4b_T2h if !skin_there => {
                RDModell::read_p4h_n4b_g4b_b4b_t2h(
                    vertex_buffer_size,
                    vertices_count,
                    vert_read_buf,
                )
            }
            VertexFormatSize::P4h_N4b_T2h_I4b_W4b if skin_there => {
                RDModell::read_p4h_n4b_t2h_i4b_w4b(
                    vertex_buffer_size,
                    vertices_count,
                    vert_read_buf,
                )
            }
            VertexFormatSize::P4h_N4b_G4b_B4b_T2h_C4c if !skin_there => {
                RDModell::read_p4h_n4b_g4b_b4b_t2h_c4c(
                    vertex_buffer_size,
                    vertices_count,
                    vert_read_buf,
                )
            }
            VertexFormatSize::P4h_N4b_G4b_B4b_T2h_I4b if skin_there => {
                RDModell::read_p4h_n4b_g4b_b4b_t2h_i4b(
                    vertex_buffer_size,
                    vertices_count,
                    vert_read_buf,
                )
            }
            _ => {
                error!("vertices use unrecognised size of {}", vertex_buffer_size);
                None
            }
        }
    }

    fn read_p4h(
        vertex_buffer_size: u32,
        vertices_count: u32,
        mut vert_read_buf: Bytes,
    ) -> Option<Vec<VertexFormat>> {
        vert_read_buf.truncate((vertices_count * vertex_buffer_size) as usize);
        assert_eq!(vert_read_buf.remaining() % vertex_buffer_size as usize, 0);
        let mut verts_vec: Vec<VertexFormat> = Vec::with_capacity(vertices_count as usize);

        for _ in 0..vertices_count {
            let p4h = vert_read_buf.get_p4h();
            let k = VertexFormat::P4h(p4h);
            verts_vec.push(k);
        }
        assert_eq!(verts_vec.len(), vertices_count as usize);
        assert_eq!(vert_read_buf.is_empty(), true);
        info!(
            "Read {} vertices of type P4h ({} bytes)",
            verts_vec.len(),
            vertex_buffer_size
        );
        Some(verts_vec)
    }

    fn read_p4h_n4b_t2h(
        vertex_buffer_size: u32,
        vertices_count: u32,
        mut vert_read_buf: Bytes,
    ) -> Option<Vec<VertexFormat>> {
        vert_read_buf.truncate((vertices_count * vertex_buffer_size) as usize);
        assert_eq!(vert_read_buf.remaining() % vertex_buffer_size as usize, 0);
        let mut verts_vec: Vec<VertexFormat> = Vec::with_capacity(vertices_count as usize);

        for _ in 0..vertices_count {
            let p4h = vert_read_buf.get_p4h();
            let n4b = vert_read_buf.get_n4b();
            let t2h = vert_read_buf.get_t2h();

            let k = VertexFormat::P4h_N4b_T2h(p4h, n4b, t2h);
            verts_vec.push(k);
        }
        assert_eq!(verts_vec.len(), vertices_count as usize);
        assert_eq!(vert_read_buf.is_empty(), true);
        info!(
            "Read {} vertices of type P4h_N4b_T2h ({} bytes)",
            verts_vec.len(),
            vertex_buffer_size
        );
        Some(verts_vec)
    }

    fn read_p4h_n4b_t2h_c4c(
        vertex_buffer_size: u32,
        vertices_count: u32,
        mut vert_read_buf: Bytes,
    ) -> Option<Vec<VertexFormat>> {
        vert_read_buf.truncate((vertices_count * vertex_buffer_size) as usize);
        assert_eq!(vert_read_buf.remaining() % vertex_buffer_size as usize, 0);
        let mut verts_vec: Vec<VertexFormat> = Vec::with_capacity(vertices_count as usize);

        for _ in 0..vertices_count {
            let p4h = vert_read_buf.get_p4h();
            let n4b = vert_read_buf.get_n4b();
            let t2h = vert_read_buf.get_t2h();
            let c4c = vert_read_buf.get_c4c();

            let k = VertexFormat::P4h_N4b_T2h_C4c(p4h, n4b, t2h, c4c);
            verts_vec.push(k);
        }
        assert_eq!(verts_vec.len(), vertices_count as usize);
        assert_eq!(vert_read_buf.is_empty(), true);
        info!(
            "Read {} vertices of type P4h_N4b_T2h_C4c ({} bytes)",
            verts_vec.len(),
            vertex_buffer_size
        );
        Some(verts_vec)
    }

    fn read_p4h_n4b_t2h_i4b(
        vertex_buffer_size: u32,
        vertices_count: u32,
        mut vert_read_buf: Bytes,
    ) -> Option<Vec<VertexFormat>> {
        vert_read_buf.truncate((vertices_count * vertex_buffer_size) as usize);
        assert_eq!(vert_read_buf.remaining() % vertex_buffer_size as usize, 0);
        let mut verts_vec: Vec<VertexFormat> = Vec::with_capacity(vertices_count as usize);

        for _ in 0..vertices_count {
            let p4h = vert_read_buf.get_p4h();
            let n4b = vert_read_buf.get_n4b();
            let t2h = vert_read_buf.get_t2h();
            let i4b = vert_read_buf.get_i4b();

            let k = VertexFormat::P4h_N4b_T2h_I4b(p4h, n4b, t2h, i4b);
            verts_vec.push(k);
        }
        assert_eq!(verts_vec.len(), vertices_count as usize);
        assert_eq!(vert_read_buf.is_empty(), true);
        info!(
            "Read {} vertices of type P4h_N4b_T2h_I4b ({} bytes)",
            verts_vec.len(),
            vertex_buffer_size
        );
        Some(verts_vec)
    }

    fn read_p4h_n4b_g4b_b4b_t2h(
        vertex_buffer_size: u32,
        vertices_count: u32,
        mut vert_read_buf: Bytes,
    ) -> Option<Vec<VertexFormat>> {
        vert_read_buf.truncate((vertices_count * vertex_buffer_size) as usize);
        assert_eq!(vert_read_buf.remaining() % vertex_buffer_size as usize, 0);
        let mut verts_vec: Vec<VertexFormat> = Vec::with_capacity(vertices_count as usize);

        for _ in 0..vertices_count {
            let p4h = vert_read_buf.get_p4h();
            let n4b = vert_read_buf.get_n4b();
            let g4b = vert_read_buf.get_g4b();
            let b4b = vert_read_buf.get_b4b();
            let t2h = vert_read_buf.get_t2h();
            let k = VertexFormat::P4h_N4b_G4b_B4b_T2h(p4h, n4b, g4b, b4b, t2h);
            verts_vec.push(k);
        }
        assert_eq!(verts_vec.len(), vertices_count as usize);
        assert_eq!(vert_read_buf.is_empty(), true);
        info!(
            "Read {} vertices of type P4h_N4b_G4b_B4b_T2h ({} bytes)",
            verts_vec.len(),
            vertex_buffer_size
        );
        Some(verts_vec)
    }

    fn read_p4h_n4b_t2h_i4b_w4b(
        vertex_buffer_size: u32,
        vertices_count: u32,
        mut vert_read_buf: Bytes,
    ) -> Option<Vec<VertexFormat>> {
        vert_read_buf.truncate((vertices_count * vertex_buffer_size) as usize);
        assert_eq!(vert_read_buf.remaining() % vertex_buffer_size as usize, 0);
        let mut verts_vec: Vec<VertexFormat> = Vec::with_capacity(vertices_count as usize);

        for _ in 0..vertices_count {
            let p4h = vert_read_buf.get_p4h();
            let n4b = vert_read_buf.get_n4b();
            let t2h = vert_read_buf.get_t2h();

            let i4b = vert_read_buf.get_i4b();
            let w4b = vert_read_buf.get_w4b();

            let k = VertexFormat::P4h_N4b_T2h_I4b_W4b(p4h, n4b, t2h, i4b, w4b);
            verts_vec.push(k);
        }
        assert_eq!(verts_vec.len(), vertices_count as usize);
        assert_eq!(vert_read_buf.is_empty(), true);
        info!(
            "Read {} vertices of type P4h_N4b_T2h_I4b_W4b ({} bytes)",
            verts_vec.len(),
            vertex_buffer_size
        );
        Some(verts_vec)
    }

    fn read_p4h_n4b_g4b_b4b_t2h_c4c(
        vertex_buffer_size: u32,
        vertices_count: u32,
        mut vert_read_buf: Bytes,
    ) -> Option<Vec<VertexFormat>> {
        vert_read_buf.truncate((vertices_count * vertex_buffer_size) as usize);
        assert_eq!(vert_read_buf.remaining() % vertex_buffer_size as usize, 0);
        let mut verts_vec: Vec<VertexFormat> = Vec::with_capacity(vertices_count as usize);

        for _ in 0..vertices_count {
            let p4h = vert_read_buf.get_p4h();
            let n4b = vert_read_buf.get_n4b();
            let g4b = vert_read_buf.get_g4b();
            let b4b = vert_read_buf.get_b4b();
            let t2h = vert_read_buf.get_t2h();
            let c4c = vert_read_buf.get_c4c();
            let k = VertexFormat::P4h_N4b_G4b_B4b_T2h_C4c(p4h, n4b, g4b, b4b, t2h, c4c);
            verts_vec.push(k);
        }
        assert_eq!(verts_vec.len(), vertices_count as usize);
        assert_eq!(vert_read_buf.is_empty(), true);
        info!(
            "Read {} vertices of type P4h_N4b_G4b_B4b_T2h_C4c ({} bytes)",
            verts_vec.len(),
            vertex_buffer_size
        );

        Some(verts_vec)
    }

    fn read_p4h_n4b_g4b_b4b_t2h_i4b(
        vertex_buffer_size: u32,
        vertices_count: u32,
        mut vert_read_buf: Bytes,
    ) -> Option<Vec<VertexFormat>> {
        vert_read_buf.truncate((vertices_count * vertex_buffer_size) as usize);
        assert_eq!(vert_read_buf.remaining() % vertex_buffer_size as usize, 0);
        let mut verts_vec: Vec<VertexFormat> = Vec::with_capacity(vertices_count as usize);

        for _ in 0..vertices_count {
            let p4h = vert_read_buf.get_p4h();
            let n4b = vert_read_buf.get_n4b();
            let g4b = vert_read_buf.get_g4b();
            let b4b = vert_read_buf.get_b4b();
            let t2h = vert_read_buf.get_t2h();
            let i4b = vert_read_buf.get_i4b();
            let k = VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(p4h, n4b, g4b, b4b, t2h, i4b);
            verts_vec.push(k);
        }
        assert_eq!(verts_vec.len(), vertices_count as usize);
        assert_eq!(vert_read_buf.is_empty(), true);
        info!(
            "Read {} vertices of type P4h_N4b_G4b_B4b_T2h_I4b ({} bytes)",
            verts_vec.len(),
            vertex_buffer_size
        );

        Some(verts_vec)
    }
}

impl fmt::Debug for RDModell {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let felm = self.vertices.first().unwrap();
        let vformat = match felm {
            VertexFormat::P4h(_) => "P4h",
            VertexFormat::P4h_N4b_T2h(_, _, _) => "P4h_N4b_T2h",
            VertexFormat::P4h_N4b_T2h_C4c(_, _, _, _) => "P4h_N4b_T2h_C4c",
            VertexFormat::P4h_N4b_T2h_I4b(_, _, _, _) => "P4h_N4b_T2h_I4b",
            VertexFormat::P4h_N4b_G4b_B4b_T2h(_, _, _, _, _) => "P4h_N4b_G4b_B4b_T2h",
            VertexFormat::P4h_N4b_T2h_I4b_W4b(_, _, _, _, _) => "P4h_N4b_G4b_B4b_T2h",
            VertexFormat::P4h_N4b_G4b_B4b_T2h_C4c(_, _, _, _, _, _) => "P4h_N4b_G4b_B4b_T2h_C4c",
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(_, _, _, _, _, _) => "P4h_N4b_G4b_B4b_T2h_I4b",
        };
        f.debug_struct("RDModell")
            .field("meta", &self.meta)
            .field("vertex_format", &vformat)
            .field("vertex_offset", &self.vertex_offset)
            .field("vertices_count", &self.vertices_count)
            .field("vertex_buffer_size", &self.vertex_buffer_size)
            .field("triangles_offset", &self.triangles_offset)
            .field("triangles_idx_count", &self.triangles_idx_count)
            .field("triangles_idx_size", &self.triangles_idx_size)
            .finish()
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct Triangle {
    indices: [u16; 3],
}

#[derive(Debug)]
#[repr(C)]
pub struct P4h {
    pos: [f16; 4],
}
#[derive(Debug)]
#[repr(C)]
pub struct N4b {
    normals: [u8; 4],
}
#[derive(Debug)]
#[repr(C)]
pub struct G4b {
    tangent: [u8; 4],
}
#[derive(Debug)]
#[repr(C)]
pub struct B4b {
    binormal: [u8; 4],
}
#[derive(Debug)]
#[repr(C)]
pub struct T2h {
    tex: [f16; 2],
}

#[derive(Debug)]
#[repr(C)]
pub struct I4b {
    blend_idx: [u8; 4],
}

#[derive(Debug)]
#[repr(C)]
pub struct W4b {
    blend_weight: [u8; 4],
}

#[derive(Debug)]
#[repr(C)]
pub struct C4c {
    unknown: [u8; 4],
}

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum VertexFormat {
    P4h(P4h),
    P4h_N4b_T2h(P4h, N4b, T2h),
    P4h_N4b_T2h_C4c(P4h, N4b, T2h, C4c),
    P4h_N4b_T2h_I4b(P4h, N4b, T2h, I4b),
    P4h_N4b_G4b_B4b_T2h(P4h, N4b, G4b, B4b, T2h),
    P4h_N4b_T2h_I4b_W4b(P4h, N4b, T2h, I4b, W4b),
    P4h_N4b_G4b_B4b_T2h_C4c(P4h, N4b, G4b, B4b, T2h, C4c),
    P4h_N4b_G4b_B4b_T2h_I4b(P4h, N4b, G4b, B4b, T2h, I4b),
}

impl VertexFormat {
    fn get_p4h(&self) -> &P4h {
        match self {
            VertexFormat::P4h(p4h) => p4h,
            VertexFormat::P4h_N4b_T2h(p4h, _, _) => p4h,
            VertexFormat::P4h_N4b_T2h_C4c(p4h, _, _, _) => p4h,
            VertexFormat::P4h_N4b_T2h_I4b(p4h, _, _, _) => p4h,
            VertexFormat::P4h_N4b_G4b_B4b_T2h(p4h, _, _, _, _) => p4h,
            VertexFormat::P4h_N4b_T2h_I4b_W4b(p4h, _, _, _, _) => p4h,
            VertexFormat::P4h_N4b_G4b_B4b_T2h_C4c(p4h, _, _, _, _, _) => p4h,
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(p4h, _, _, _, _, _) => p4h,
        }
    }

    fn get_t2h(&self) -> &T2h {
        match self {
            VertexFormat::P4h(_) => unimplemented!(),
            VertexFormat::P4h_N4b_T2h(_, _, t2h) => t2h,
            VertexFormat::P4h_N4b_T2h_C4c(_, _, t2h, _) => t2h,
            VertexFormat::P4h_N4b_T2h_I4b(_, _, t2h, _) => t2h,
            VertexFormat::P4h_N4b_G4b_B4b_T2h(_, _, _, _, t2h) => t2h,
            VertexFormat::P4h_N4b_T2h_I4b_W4b(_, _, t2h, _, _) => t2h,
            VertexFormat::P4h_N4b_G4b_B4b_T2h_C4c(_, _, _, _, t2h, _) => t2h,
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(_, _, _, _, t2h, _) => t2h,
        }
    }

    fn get_n4b(&self) -> &N4b {
        match self {
            VertexFormat::P4h(_) => unimplemented!(),
            VertexFormat::P4h_N4b_T2h(_, n4b, _) => n4b,
            VertexFormat::P4h_N4b_T2h_C4c(_, n4b, _, _) => n4b,
            VertexFormat::P4h_N4b_T2h_I4b(_, n4b, _, _) => n4b,
            VertexFormat::P4h_N4b_G4b_B4b_T2h(_, n4b, _, _, _) => n4b,
            VertexFormat::P4h_N4b_T2h_I4b_W4b(_, n4b, _, _, _) => n4b,
            VertexFormat::P4h_N4b_G4b_B4b_T2h_C4c(_, n4b, _, _, _, _) => n4b,
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(_, n4b, _, _, _, _) => n4b,
        }
    }

    fn get_g4b(&self) -> &G4b {
        match self {
            VertexFormat::P4h(_) => unimplemented!(),
            VertexFormat::P4h_N4b_T2h(_, _, _) => unimplemented!(),
            VertexFormat::P4h_N4b_T2h_C4c(_, _, _, _) => unimplemented!(),
            VertexFormat::P4h_N4b_T2h_I4b(_, _, _, _) => unimplemented!(),
            VertexFormat::P4h_N4b_G4b_B4b_T2h(_, _, g4b, _, _) => g4b,
            VertexFormat::P4h_N4b_T2h_I4b_W4b(_, _, _, _, _) => unimplemented!(),
            VertexFormat::P4h_N4b_G4b_B4b_T2h_C4c(_, _, g4b, _, _, _) => g4b,
            VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(_, _, g4b, _, _, _) => g4b,
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
}

impl From<&Path> for RDModell {
    fn from(f_path: &Path) -> Self {
        let mut f = File::open(f_path).unwrap();
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut buffer).ok();

        let buffer_len = buffer.len();
        info!("loaded {:?} into buffer", f_path.to_str().unwrap());

        info!("buffer size: {}", buffer_len);
        RDModell::new(buffer)
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

pub fn nain() {
    //env_logger::init();
    env_logger::from_env(Env::default().default_filter_or("info")).init();

    info!("init !");

    let mut rdm = RDModell::from("tests/basalt_crusher_others_lod2.rdm");
    //info!("rdm: {:#?}", rdm);

    rdm.add_skin();

    let anim = RDAnim::from("tests/basalt_crusher_others_work01.rdm");
    rdm.add_anim(anim);

    gltf_export::build(rdm);
}
