use bytes::{BufMut, BytesMut};

use std::fs;
use std::io::Write;

use crate::*;

pub struct RDWriter {
    meta_deref: u32,
    input: RDModell,
    buf: BytesMut,
}

impl RDWriter {
    fn new(rdm: RDModell) -> Self {
        let mut rdw = RDWriter {
            meta_deref: 331,
            input: rdm,
            buf: BytesMut::with_capacity(5000),
        };

        /*
        rdw.read_inv();

        let gltf_imp = gltf_reader::start().unwrap();
        rdw.input.vertices = gltf_imp.0;
        rdw.input.vertex_buffer_size = 28;
        rdw.input.vertices_count = rdw.input.vertices.len() as u32;

        rdw.input.triangle_indices = gltf_imp.1;
        rdw.input.triangles_idx_count = rdw.input.triangle_indices.len() as u32*3;
        rdw.input.triangles_idx_size = 2 as u32;
        //rdw.read_pos_norm_tang_bi();
        */

        rdw.put_header();
        rdw.put_vertex_buffer();
        rdw.put_indexed_triangle_list();

        rdw.put_blob();

        rdw.put_skin();

        rdw
    }

    fn put_header(&mut self) {
        static RAW_DATA: [u8; 156] = [
            0x52, 0x44, 0x4D, 0x01, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
            0x00, 0x00, 0x1C, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00,
            0x54, 0x00, 0x00, 0x00, 0x4B, 0x01, 0x00, 0x00, 0x7F, 0x57, 0x01, 0x00, 0x72, 0x58,
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x48, 0x00, 0x00, 0x00,
            0xA4, 0x00, 0x00, 0x00, 0x25, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];

        self.buf.put_slice(&RAW_DATA);

        //let export_name = br"G:\graphic_backup\danny\Anno5\preproduction\buildings\others\basalt_crusher_others\scenes\basalt_crusher_others_rig02.max";
        let export_name = br"\\rds.local\data\Art\graphic_backup\christian\#ANNO5\buildings\others\basalt_crusher_others\Lowpoly\basalt_crusher_others_low_05.max";

        // len str + u32:1
        self.buf.put_u32_le(export_name.len() as u32);
        self.buf.put_u32_le(1);
        {
            let path_str_ptr = (self.buf.len() as u32).to_le_bytes();
            let buff_off = 84 as usize;
            self.buf[buff_off] = path_str_ptr[0];
            self.buf[buff_off + 1] = path_str_ptr[1];
            self.buf[buff_off + 2] = path_str_ptr[2];
            self.buf[buff_off + 3] = path_str_ptr[3];
        }
        self.buf.put_slice(export_name);

        let export_name_2 = br"Anno5_Building_Skin_1Blend.rmp";
        self.buf.put_u32_le(export_name_2.len() as u32);
        self.buf.put_u32_le(1);
        {
            let file_str_ptr = (self.buf.len() as u32).to_le_bytes();
            let buff_off = 88 as usize;
            self.buf[buff_off] = file_str_ptr[0];
            self.buf[buff_off + 1] = file_str_ptr[1];
            self.buf[buff_off + 2] = file_str_ptr[2];
            self.buf[buff_off + 3] = file_str_ptr[3];
        }
        self.buf.put_slice(export_name_2);

        // meta table
        {
            self.buf.put_u32_le(1);
            self.buf.put_u32_le(92);

            {
                let meta_ptr = self.buf.len() as u32;
                self.meta_deref = meta_ptr;
                let meta_ptr_arr = meta_ptr.to_le_bytes();
                let buff_off = (32) as usize;
                self.buf[buff_off] = meta_ptr_arr[0];
                self.buf[buff_off + 1] = meta_ptr_arr[1];
                self.buf[buff_off + 2] = meta_ptr_arr[2];
                self.buf[buff_off + 3] = meta_ptr_arr[3];
            }

            // 52 bytes data + 50 Bytes 0x0  = 92 bytes
            // 0-20 ptr's
            // u32: 0x00_00_00_00 or 0x_FF_FF_FF_FF
            // 24 bytes: 12 f16 with bounding box like data (3*4 a f16)

            self.buf.put_u32_le(self.buf.len() as u32 + 8 + 92);

            static META_TABLE: [u8; 24] = [
                0xF5, 0x01, 0x00, 0x00, 0x7D, 0x02, 0x00, 0x00, 0xBD, 0x02, 0x00, 0x00, 0xC9, 0x20,
                0x01, 0x00, 0x99, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ];

            self.buf.put_slice(&META_TABLE);

            static META_BOX: [u8; 24] = [
                0x00, 0x80, 0xF8, 0xBF, 0x00, 0x40, 0x22, 0xC0, 0x00, 0x60, 0xF7, 0xBF, 0x00, 0x80,
                0xFB, 0x3F, 0x00, 0xC0, 0xF2, 0x3F, 0x00, 0x80, 0xFC, 0x3F,
            ];

            self.buf.put_slice(&META_BOX);

            static META_ZERO: [u8; 40] = [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ];

            self.buf.put_slice(&META_ZERO);
        }

        {
            // MODEL_NAME_PTR
            self.buf.put_u32_le(1);
            self.buf.put_u32_le(28);

            self.buf.put_u32_le(self.buf.len() as u32 + 8 + 28);

            static MODEL_PTR_ZERO: [u8; 24] = [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ];

            self.buf.put_slice(&MODEL_PTR_ZERO);
        }

        {
            // MODEL_STR
            let model_str = br"basalt_crusher_others_lod0";
            self.buf.put_u32_le(model_str.len() as u32);
            self.buf.put_u32_le(1);

            self.buf.put_slice(model_str);
        }

        {
            // VERTEX_FORMAT_IDENTIFIER_PTR

            static VERTEX_FORMAT_IDENTIFIER_PTR_ZERO: [u8; 16] = [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ];

            self.buf.put_u32_le(1);
            self.buf.put_u32_le(24);

            {
                let meta_id_ptr = (self.buf.len() as u32).to_le_bytes();
                let buff_off = (self.meta_deref + 4) as usize;
                self.buf[buff_off] = meta_id_ptr[0];
                self.buf[buff_off + 1] = meta_id_ptr[1];
                self.buf[buff_off + 2] = meta_id_ptr[2];
                self.buf[buff_off + 3] = meta_id_ptr[3];
            }

            self.buf.put_u32_le(self.buf.len() as u32 + 8 + 24);

            // unknown maybe shader id
            // 0: no anim
            // 1: _Ib4
            // 2:
            // 3: I4b_W4b (eve)
            // 4: I4b_W4b (other npc)
            if self.input.has_skin() {
                self.buf.put_u32_le(1);
            } else {
                self.buf.put_u32_le(0);
            }

            self.buf.put_slice(&VERTEX_FORMAT_IDENTIFIER_PTR_ZERO);
        }

        {
            // VERTEX_FORMAT_IDENTIFIER
            static P4H_IDENTIFIER: [u8; 16] = [
                0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
                0x00, 0x00,
            ];

            static N4B_IDENTIFIER: [u8; 16] = [
                0x01, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x01, 0x00,
                0x00, 0x00,
            ];

            static G4B_IDENTIFIER: [u8; 16] = [
                0x02, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x01, 0x00,
                0x00, 0x00,
            ];

            static B4B_IDENTIFIER: [u8; 16] = [
                0x03, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x01, 0x00,
                0x00, 0x00,
            ];

            static T2H_IDENTIFIER: [u8; 16] = [
                0x04, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00,
                0x00, 0x00,
            ];

            static I4B_IDENTIFIER: [u8; 16] = [
                0x07, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
                0x00, 0x00,
            ];

            //P4h_N4b_G4b_B4b_T2h_I4b
            self.buf.put_u32_le(6);
            self.buf.put_u32_le(16);

            //self.buf.put_u32_le(4);
            //self.buf.put_u32_le(16);

            self.buf.put_slice(&P4H_IDENTIFIER);
            self.buf.put_slice(&N4B_IDENTIFIER);
            self.buf.put_slice(&G4B_IDENTIFIER);
            self.buf.put_slice(&B4B_IDENTIFIER);
            self.buf.put_slice(&T2H_IDENTIFIER);
            self.buf.put_slice(&I4B_IDENTIFIER);
        }

        {
            // unknown const
            self.buf.put_u32_le(1);
            self.buf.put_u32_le(20);

            {
                let meta_unknown_ptr = (self.buf.len() as u32).to_le_bytes();
                let buff_off = (self.meta_deref + 8) as usize;
                self.buf[buff_off] = meta_unknown_ptr[0];
                self.buf[buff_off + 1] = meta_unknown_ptr[1];
                self.buf[buff_off + 2] = meta_unknown_ptr[2];
                self.buf[buff_off + 3] = meta_unknown_ptr[3];
            }

            static UNKNOWN: [u8; 20] = [
                0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ];
            self.buf.put_slice(&UNKNOWN);
        }

        {
            self.buf.put_u32_le(1);
            self.buf.put_u32_le(28);

            {
                let triangle_count_ptr = (self.buf.len() as u32).to_le_bytes();
                let buff_off = (self.meta_deref + 20) as usize;
                self.buf[buff_off] = triangle_count_ptr[0];
                self.buf[buff_off + 1] = triangle_count_ptr[1];
                self.buf[buff_off + 2] = triangle_count_ptr[2];
                self.buf[buff_off + 3] = triangle_count_ptr[3];
            }

            self.buf.put_u32_le(0);
            self.buf.put_u32_le(self.input.triangles_idx_count);
            static ZERO_20_OF_28: [u8; 20] = [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ];

            self.buf.put_slice(&ZERO_20_OF_28);
        }

        assert_eq!(self.buf.len(), 704);

        //to be patched:
        // anim off 40
        // off 36 -> end ?!
        // off 24 -> file end header
        //
        // off 32 -> 331 dez
        // 331 + 12 = vertex data start
        // 311 + 16 = indexed_triangle_list data start

        // raw data cont. start till (vertex data start -8)
    }

    fn put_vertex_buffer(&mut self) {
        self.buf.put_u32_le(self.input.vertices_count);
        self.buf.put_u32_le(self.input.vertex_buffer_size);

        {
            let vertex_ptr = (self.buf.len() as u32).to_le_bytes();
            let buff_off = (self.meta_deref + RDModell::VERTEX_META) as usize;
            self.buf[buff_off] = vertex_ptr[0];
            self.buf[buff_off + 1] = vertex_ptr[1];
            self.buf[buff_off + 2] = vertex_ptr[2];
            self.buf[buff_off + 3] = vertex_ptr[3];
        }

        for vert in self.input.vertices.iter() {
            match vert {
                VertexFormat::P4h_N4b_G4b_B4b_T2h_C4c(p4h, n4b, g4b, b4b, t2h, c4c) => {
                    self.buf.put_p4h(p4h);
                    self.buf.put_n4b(n4b);
                    self.buf.put_g4b(g4b);
                    self.buf.put_b4b(b4b);
                    self.buf.put_t2h(t2h);
                    self.buf.put_c4c(c4c);
                }
                VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(p4h, n4b, g4b, b4b, t2h, i4b) => {
                    self.buf.put_p4h(p4h);
                    self.buf.put_n4b(n4b);
                    self.buf.put_g4b(g4b);
                    self.buf.put_b4b(b4b);
                    self.buf.put_t2h(t2h);
                    self.buf.put_i4b(i4b);
                }
                VertexFormat::P4h_N4b_T2h_I4b(p4h, n4b, t2h, i4b) => {
                    self.buf.put_p4h(p4h);
                    self.buf.put_n4b(n4b);
                    self.buf.put_t2h(t2h);
                    self.buf.put_i4b(i4b);
                }
                _ => todo!(),
            }
        }
    }

    fn put_indexed_triangle_list(&mut self) {
        self.buf.put_u32_le(self.input.triangles_idx_count);
        self.buf.put_u32_le(2);

        {
            let vertex_ptr = (self.buf.len() as u32).to_le_bytes();
            let buff_off = (self.meta_deref + RDModell::TRIANGLES_META) as usize;
            self.buf[buff_off] = vertex_ptr[0];
            self.buf[buff_off + 1] = vertex_ptr[1];
            self.buf[buff_off + 2] = vertex_ptr[2];
            self.buf[buff_off + 3] = vertex_ptr[3];
        }

        for triangle in self.input.triangle_indices.iter() {
            self.buf.put_u16_le(triangle.indices[0]);
            self.buf.put_u16_le(triangle.indices[1]);
            self.buf.put_u16_le(triangle.indices[2]);
        }
    }

    fn put_blob(&mut self) {
        {
            let vertex_ptr = (self.buf.len() as u32 + 8).to_le_bytes();
            let buff_off = (36) as usize;
            self.buf[buff_off] = vertex_ptr[0];
            self.buf[buff_off + 1] = vertex_ptr[1];
            self.buf[buff_off + 2] = vertex_ptr[2];
            self.buf[buff_off + 3] = vertex_ptr[3];
        }

        let start = self.buf.len();

        {
            // unknown png

            self.buf.put_u32_le(1);
            self.buf.put_u32_le(28);

            self.buf.put_u32_le(self.buf.len() as u32 + 8 + 28);

            static UNKNOWN: [u8; 24] = [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ];
            self.buf.put_slice(&UNKNOWN);
        }

        {
            self.buf.put_u32_le(1);
            self.buf.put_u32_le(48);

            static UNKNOWN2: [u8; 40] = [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ];

            let material = br"Default Standard12432142134";
            let dummy_png_path = br"d:/projekte/anno5/game/testdata/graphics/dummy_objects/dummy_christian/rdm/basalt_crusher_others/diffuse.png";

            self.buf.put_u32_le(self.buf.len() as u32 + 8 + 48);
            self.buf
                .put_u32_le(self.buf.len() as u32 + 8 + 48 + material.len() as u32 + 8 - 4); // -4 advanced: 4 bytes
            self.buf.put_slice(&UNKNOWN2);

            self.buf.put_u32_le(material.len() as u32);
            self.buf.put_u32_le(1);
            self.buf.put_slice(material);

            self.buf.put_u32_le(dummy_png_path.len() as u32);
            self.buf.put_u32_le(1);
            self.buf.put_slice(dummy_png_path);
        }

        let end = self.buf.len();

        let written = end - start;

        assert_eq!(written, 243);

        // 8+1*28 : -> (0 -> next)
        // 8+1*48 : (0 -> next) (4 -> next+1)
        // 8+27*1
        // 8+108*1
        // = 243
    }

    fn put_skin(&mut self) {
        self.buf.put_u32_le(1);
        self.buf.put_u32_le(32);
        {
            let skin_ptr_ptr = (self.buf.len() as u32).to_le_bytes();
            let buff_off = (40) as usize;
            self.buf[buff_off] = skin_ptr_ptr[0];
            self.buf[buff_off + 1] = skin_ptr_ptr[1];
            self.buf[buff_off + 2] = skin_ptr_ptr[2];
            self.buf[buff_off + 3] = skin_ptr_ptr[3];
        }
        self.buf.put_u32_le((self.buf.len() + 32 + 8) as u32); //first joint ptr

        for _ in 0..28 {
            self.buf.put_u8(0);
        }

        let joints = self.input.joints.clone().unwrap(); // stupid !
        let joint_count = joints.len();
        self.buf.put_u32_le(joint_count as u32);
        self.buf.put_u32_le(84);

        let mut name_ptr_vec = Vec::with_capacity(joint_count);
        for joint in &joints {
            let start = self.buf.len();

            // joint name ptr
            name_ptr_vec.push(start);
            self.buf.put_u32_le(0xAAAAAAAA);

            {
                let child_quaternion = joint.quaternion;

                let rx = -child_quaternion[0];
                let ry = -child_quaternion[1];
                let rz = -child_quaternion[2];
                let rw = -child_quaternion[3];

                let q = Quaternion::new(rw, rx, ry, rz);
                let uq = UnitQuaternion::from_quaternion(q);

                let trans = joint.transition;
                let tx = trans[0];
                let ty = trans[1];
                let tz = trans[2];
                let ct: Translation3<f32> = Translation3::new(tx, ty, tz);

                println!("ct : {:#?}", ct);

                let bindmat = (ct.to_homogeneous()) * (uq.to_homogeneous()) * Matrix4::identity();

                let inv_bindmat = bindmat.try_inverse().unwrap();

                println!("{}", uq.quaternion().coords);

                // write Translation
                self.buf.put_f32_le(inv_bindmat.m14);
                self.buf.put_f32_le(inv_bindmat.m24);
                self.buf.put_f32_le(inv_bindmat.m34);

                // write rotation
                let rot = uq.quaternion().coords;
                self.buf.put_f32_le(rot.x);
                self.buf.put_f32_le(rot.y);
                self.buf.put_f32_le(rot.z);
                self.buf.put_f32_le(rot.w);
            }

            // write parent u8
            if joint.parent == 255 {
                self.buf.put_u32_le(0xFFFFFFFF);
            } else {
                self.buf.put_u8(joint.parent); // (33 bytes of 84)
                self.buf.put_u8(0);
                self.buf.put_u8(0);
                self.buf.put_u8(0);
            }

            // 36 + 48 = 84
            for _ in 0..48 {
                self.buf.put_u8(0);
            }
            let end = self.buf.len();
            let lenj = end - start;
            assert_eq!(lenj, 84);
        }

        let mut name_ptr_itr = name_ptr_vec.iter();

        for joint in &joints {
            let len_jname = joint.name.len() as u32;
            self.buf.put_u32_le(len_jname);
            self.buf.put_u32_le(1);

            {
                let jname_ptr = (self.buf.len() as u32).to_le_bytes();
                let buff_off = *name_ptr_itr.next().unwrap();
                self.buf[buff_off] = jname_ptr[0];
                self.buf[buff_off + 1] = jname_ptr[1];
                self.buf[buff_off + 2] = jname_ptr[2];
                self.buf[buff_off + 3] = jname_ptr[3];
            }

            self.buf.put_slice(joint.name.as_ref());
        }
    }

    pub fn write_rdm(self) {
        let _ = fs::create_dir("rdm_exp");

        let mut writer = fs::File::create("rdm_exp/out.rdm").expect("I/O error");
        writer.write_all(&self.buf.to_vec()).expect("I/O error");
    }

    #[deprecated]
    pub fn read_inv(&mut self) {
        let file = "rdm/mat_buffer2.bin";
        let mut f = File::open(file).unwrap();
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut buffer).ok();

        let buffer_len = buffer.len();
        println!("loaded {:?} into buffer", file);

        println!("buffer size: {}", buffer_len);

        assert_eq!(buffer_len % (4 * 4 * 4), 0);
        let count = buffer_len / (4 * 4 * 4);

        let mut mat4: Matrix4<f32> = Matrix4::identity();

        let mut rbuffer = Bytes::from(buffer);

        let mut input_joints = self.input.joints.clone().unwrap();

        for i in 0..count {
            mat4.m11 = rbuffer.get_f32_le();
            mat4.m21 = rbuffer.get_f32_le();
            mat4.m31 = rbuffer.get_f32_le();
            mat4.m41 = rbuffer.get_f32_le();

            mat4.m12 = rbuffer.get_f32_le();
            mat4.m22 = rbuffer.get_f32_le();
            mat4.m32 = rbuffer.get_f32_le();
            mat4.m42 = rbuffer.get_f32_le();

            mat4.m13 = rbuffer.get_f32_le();
            mat4.m23 = rbuffer.get_f32_le();
            mat4.m33 = rbuffer.get_f32_le();
            mat4.m43 = rbuffer.get_f32_le();

            mat4.m14 = rbuffer.get_f32_le();
            mat4.m24 = rbuffer.get_f32_le();
            mat4.m34 = rbuffer.get_f32_le();
            mat4.m44 = rbuffer.get_f32_le();

            let mat3 = Matrix3::new(
                mat4.m11, mat4.m12, mat4.m13, mat4.m21, mat4.m22, mat4.m23, mat4.m31, mat4.m32,
                mat4.m33,
            );
            let rot = Rotation3::from_matrix(&mat3);
            let q = UnitQuaternion::from_rotation_matrix(&rot).inverse().coords;

            let qq = Quaternion::new(q.w, q.x, q.y, q.z);
            let uq = UnitQuaternion::from_quaternion(qq);

            println!("rots : {:#?}", uq);

            let tx = mat4.m14;
            let ty = mat4.m24;
            let tz = mat4.m34;
            let joint_translatio: Translation3<f32> = Translation3::new(tx, ty, tz);

            let inv_bindmat = (uq.to_homogeneous()) * (joint_translatio.to_homogeneous());
            let iv_x = inv_bindmat.m14;
            let iv_y = inv_bindmat.m24;
            let iv_z = inv_bindmat.m34;

            let trans_point = Translation3::new(iv_x, iv_y, iv_z).inverse();

            println!("trans : {:#?}", trans_point);

            let quaternion_mat4 = uq.quaternion().coords;
            assert_relative_eq!(quaternion_mat4.x, input_joints[i].quaternion[0]);
            assert_relative_eq!(quaternion_mat4.y, input_joints[i].quaternion[1]);
            assert_relative_eq!(quaternion_mat4.z, input_joints[i].quaternion[2]);
            assert_relative_eq!(quaternion_mat4.w, input_joints[i].quaternion[3]);

            /*
            will fail because float
            left  = -0.002778834
            right = -0.0027786891

            assert_relative_eq!(trans_point.x,input_joints[i].transition[0]);
            assert_relative_eq!(trans_point.y,input_joints[i].transition[1]);
            assert_relative_eq!(trans_point.z,input_joints[i].transition[2]);
            */

            // exp test

            input_joints[i].quaternion[0] = quaternion_mat4.x;
            input_joints[i].quaternion[1] = quaternion_mat4.y;
            input_joints[i].quaternion[2] = quaternion_mat4.z;
            input_joints[i].quaternion[3] = quaternion_mat4.w;

            input_joints[i].transition[0] = trans_point.x;
            input_joints[i].transition[1] = trans_point.y;
            input_joints[i].transition[2] = trans_point.z;
        }

        self.input.joints = Some(input_joints);
    }

    #[deprecated]
    pub fn read_pos_norm_tang_bi(&mut self) {
        println!("read_pos_norm_tang_bi");

        let mut pos_buffer;
        let mut idx_buffer;
        let mut uv_buffer;
        let mut normal_buffer;
        let mut tang_buffer;

        let mut jw_buffer;

        {
            // pos
            let file = "rdm/buffer0.bin";
            let mut f = File::open(file).unwrap();
            let mut buffer = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut buffer).ok();

            let buffer_len = buffer.len();
            println!("loaded {:?} into buffer", file);

            println!("buffer size: {}", buffer_len);
            pos_buffer = Bytes::from(buffer);
        }

        {
            // index list
            let file = "rdm/buffer1.bin";
            let mut f = File::open(file).unwrap();
            let mut buffer = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut buffer).ok();

            let buffer_len = buffer.len();
            println!("loaded {:?} into buffer", file);

            println!("buffer size: {}", buffer_len);
            idx_buffer = Bytes::from(buffer);
        }

        {
            // uv
            let file = "rdm/buffer2.bin";
            let mut f = File::open(file).unwrap();
            let mut buffer = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut buffer).ok();

            let buffer_len = buffer.len();
            println!("loaded {:?} into buffer", file);

            println!("buffer size: {}", buffer_len);
            uv_buffer = Bytes::from(buffer);
        }

        {
            // normal list
            let file = "rdm/buffer3.bin";
            let mut f = File::open(file).unwrap();
            let mut buffer = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut buffer).ok();

            let buffer_len = buffer.len();
            println!("loaded {:?} into buffer", file);

            println!("buffer size: {}", buffer_len);
            normal_buffer = Bytes::from(buffer);
        }

        {
            // tang list

            let file = "rdm/buffer4.bin";
            let mut f = File::open(file).unwrap();
            let mut buffer = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut buffer).ok();

            let buffer_len = buffer.len();
            println!("loaded {:?} into buffer", file);

            println!("buffer size: {}", buffer_len);
            tang_buffer = Bytes::from(buffer);
        }

        {
            // tang list

            let file = "rdm/jw_buffer3.bin";
            let mut f = File::open(file).unwrap();
            let mut buffer = Vec::new();
            std::io::Read::read_to_end(&mut f, &mut buffer).ok();

            let buffer_len = buffer.len();
            println!("loaded {:?} into buffer", file);

            println!("buffer size: {}", buffer_len);
            jw_buffer = Bytes::from(buffer);
        }

        let vertices_count = pos_buffer.len() / (3 * 4);
        let mut verts_vec: Vec<VertexFormat> = Vec::with_capacity(vertices_count as usize);
        for _ in 0..vertices_count {
            let p4h = P4h {
                pos: [
                    f16::from_f32(pos_buffer.get_f32_le()),
                    f16::from_f32(pos_buffer.get_f32_le()),
                    f16::from_f32(pos_buffer.get_f32_le()),
                    f16::from_f32(0.0),
                ],
            };

            let nx = normal_buffer.get_f32_le();
            let ny = normal_buffer.get_f32_le();
            let nz = normal_buffer.get_f32_le();

            let n4b = N4b {
                normals: [
                    (nx * (255.0 / 2.0) + 255.0 / 2.0) as u8,
                    (ny * (255.0 / 2.0) + 255.0 / 2.0) as u8,
                    (nz * (255.0 / 2.0) + 255.0 / 2.0) as u8,
                    0,
                ],
            };

            let tx = tang_buffer.get_f32_le();
            let ty = tang_buffer.get_f32_le();
            let tz = tang_buffer.get_f32_le();
            let tw = tang_buffer.get_f32_le();

            let g4b = G4b {
                tangent: [
                    (tx * (255.0 / 2.0) + 255.0 / 2.0) as u8,
                    (ty * (255.0 / 2.0) + 255.0 / 2.0) as u8,
                    (tz * (255.0 / 2.0) + 255.0 / 2.0) as u8,
                    {
                        let is_neg = relative_eq!(tw, -1.0);
                        if is_neg {
                            0
                        } else {
                            1
                        }
                    },
                ],
            };

            let normal = Vector3::new(nx, ny, nz);
            let tangent = Vector3::new(tx, ty, tz);

            let b: Matrix3x1<f32> = (normal.cross(&tangent)) * (tw);

            //println!("bbbbb: {:?}",b);

            let b4b = B4b {
                binormal: [
                    ((b.x * (255.0 / 2.0) + 255.0 / 2.0) as u8).saturating_add(1),
                    ((b.y * (255.0 / 2.0) + 255.0 / 2.0) as u8).saturating_add(1),
                    ((b.z * (255.0 / 2.0) + 255.0 / 2.0) as u8).saturating_add(1),
                    0,
                ],
            };

            let t2h = T2h {
                tex: [
                    f16::from_f32(uv_buffer.get_f32_le()),
                    f16::from_f32(uv_buffer.get_f32_le()),
                ],
            };

            let i4b = I4b {
                blend_idx: [
                    jw_buffer.get_u8(),
                    jw_buffer.get_u8(),
                    jw_buffer.get_u8(),
                    jw_buffer.get_u8(),
                ],
            };

            jw_buffer.advance(20 - 4);

            let k = VertexFormat::P4h_N4b_G4b_B4b_T2h_I4b(p4h, n4b, g4b, b4b, t2h, i4b);
            verts_vec.push(k);
        }
        assert_eq!(verts_vec.len(), 2615);

        self.input.vertices = verts_vec;
    }
}

impl From<RDModell> for RDWriter {
    fn from(rdm: RDModell) -> Self {
        RDWriter::new(rdm)
    }
}

trait PutVertex {
    fn put_p4h(&mut self, p4h: &P4h);
    fn put_n4b(&mut self, n4b: &N4b);
    fn put_g4b(&mut self, g4b: &G4b);
    fn put_b4b(&mut self, b4b: &B4b);
    fn put_t2h(&mut self, t2h: &T2h);
    fn put_i4b(&mut self, i4b: &I4b);
    fn put_w4b(&mut self, w4b: &W4b);
    fn put_c4c(&mut self, c4c: &C4c);
}

impl PutVertex for BytesMut {
    fn put_p4h(&mut self, p4h: &P4h) {
        self.put_u16_le(p4h.pos[0].to_bits());
        self.put_u16_le(p4h.pos[1].to_bits());
        self.put_u16_le(p4h.pos[2].to_bits());
        self.put_u16_le(p4h.pos[3].to_bits());
    }
    fn put_n4b(&mut self, n4b: &N4b) {
        self.put_slice(&n4b.normals);
    }
    fn put_g4b(&mut self, g4b: &G4b) {
        self.put_slice(&g4b.tangent);
    }
    fn put_b4b(&mut self, b4b: &B4b) {
        self.put_slice(&b4b.binormal);
    }
    fn put_t2h(&mut self, t2h: &T2h) {
        self.put_u16_le(t2h.tex[0].to_bits());
        self.put_u16_le(t2h.tex[1].to_bits());
    }
    fn put_i4b(&mut self, i4b: &I4b) {
        self.put_slice(&i4b.blend_idx);
    }
    fn put_w4b(&mut self, w4b: &W4b) {
        self.put_slice(&w4b.blend_weight);
    }
    fn put_c4c(&mut self, c4c: &C4c) {
        self.put_slice(&c4c.unknown);
    }
}
