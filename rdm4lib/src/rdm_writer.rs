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

        rdw.read_inv();

        rdw.put_header();
        rdw.put_vertex_buffer();
        rdw.put_indexed_triangle_list();

        rdw.put_blob();

        rdw.put_skin();

        rdw
    }

    fn put_header(&mut self) {

        static RAW_DATA: [u8; 693] = [
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
            0x00, 0x00, 0x79, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x47, 0x3A, 0x5C, 0x67,
            0x72, 0x61, 0x70, 0x68, 0x69, 0x63, 0x5F, 0x62, 0x61, 0x63, 0x6B, 0x75, 0x70, 0x5C,
            0x64, 0x61, 0x6E, 0x6E, 0x79, 0x5C, 0x41, 0x6E, 0x6E, 0x6F, 0x35, 0x5C, 0x70, 0x72,
            0x65, 0x70, 0x72, 0x6F, 0x64, 0x75, 0x63, 0x74, 0x69, 0x6F, 0x6E, 0x5C, 0x62, 0x75,
            0x69, 0x6C, 0x64, 0x69, 0x6E, 0x67, 0x73, 0x5C, 0x6F, 0x74, 0x68, 0x65, 0x72, 0x73,
            0x5C, 0x62, 0x61, 0x73, 0x61, 0x6C, 0x74, 0x5F, 0x63, 0x72, 0x75, 0x73, 0x68, 0x65,
            0x72, 0x5F, 0x6F, 0x74, 0x68, 0x65, 0x72, 0x73, 0x5C, 0x73, 0x63, 0x65, 0x6E, 0x65,
            0x73, 0x5C, 0x62, 0x61, 0x73, 0x61, 0x6C, 0x74, 0x5F, 0x63, 0x72, 0x75, 0x73, 0x68,
            0x65, 0x72, 0x5F, 0x6F, 0x74, 0x68, 0x65, 0x72, 0x73, 0x5F, 0x72, 0x69, 0x67, 0x30,
            0x31, 0x2E, 0x6D, 0x61, 0x78, 0x1E, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x41,
            0x6E, 0x6E, 0x6F, 0x35, 0x5F, 0x42, 0x75, 0x69, 0x6C, 0x64, 0x69, 0x6E, 0x67, 0x5F,
            0x53, 0x6B, 0x69, 0x6E, 0x5F, 0x31, 0x42, 0x6C, 0x65, 0x6E, 0x64, 0x2E, 0x72, 0x6D,
            0x70, 0x01, 0x00, 0x00, 0x00, 0x5C, 0x00, 0x00, 0x00, 0xAF, 0x01, 0x00, 0x00, 0xF5,
            0x01, 0x00, 0x00, 0x7D, 0x02, 0x00, 0x00, 0xBD, 0x02, 0x00, 0x00, 0xC9, 0x20, 0x01,
            0x00, 0x99, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0xF8, 0xBF, 0x00,
            0x40, 0x22, 0xC0, 0x00, 0x60, 0xF7, 0xBF, 0x00, 0x80, 0xFB, 0x3F, 0x00, 0xC0, 0xF2,
            0x3F, 0x00, 0x80, 0xFC, 0x3F, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x1C, 0x00, 0x00, 0x00, 0xD3, 0x01, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1A, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x62, 0x61, 0x73, 0x61, 0x6C, 0x74, 0x5F, 0x63, 0x72,
            0x75, 0x73, 0x68, 0x65, 0x72, 0x5F, 0x6F, 0x74, 0x68, 0x65, 0x72, 0x73, 0x5F, 0x6C,
            0x6F, 0x64, 0x32, 0x01, 0x00, 0x00, 0x00, 0x18, 0x00, 0x00, 0x00, 0x15, 0x02, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00,
            0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x06,
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00,
            0x00, 0x06, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x06,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x07, 0x00, 0x00,
            0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x14, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x00, 0x1C, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x57, 0x1B, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        self.buf.put_slice(&RAW_DATA);

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

        // 8+1*28 : -> (0 -> next)
        // 8+1*48 : (0 -> next) (4 -> next+1)
        // 8+27*1
        // 8+108*1
        // = 243

        static RAW_DATA: [u8; 243] = [
            0x01, 0x00, 0x00, 0x00, 0x1C, 0x00, 0x00, 0x00, 0xA3, 0x57, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x30, 0x00,
            0x00, 0x00, 0xDB, 0x57, 0x01, 0x00, 0xFE, 0x57, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1B, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00, 0x44, 0x65, 0x66, 0x61, 0x75, 0x6C, 0x74, 0x20, 0x53, 0x74, 0x61, 0x6E,
            0x64, 0x61, 0x72, 0x64, 0x31, 0x32, 0x34, 0x33, 0x32, 0x31, 0x34, 0x32, 0x31, 0x33,
            0x34, 0x6C, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x64, 0x3A, 0x2F, 0x70, 0x72,
            0x6F, 0x6A, 0x65, 0x6B, 0x74, 0x65, 0x2F, 0x61, 0x6E, 0x6E, 0x6F, 0x35, 0x2F, 0x67,
            0x61, 0x6D, 0x65, 0x2F, 0x74, 0x65, 0x73, 0x74, 0x64, 0x61, 0x74, 0x61, 0x2F, 0x67,
            0x72, 0x61, 0x70, 0x68, 0x69, 0x63, 0x73, 0x2F, 0x64, 0x75, 0x6D, 0x6D, 0x79, 0x5F,
            0x6F, 0x62, 0x6A, 0x65, 0x63, 0x74, 0x73, 0x2F, 0x64, 0x75, 0x6D, 0x6D, 0x79, 0x5F,
            0x63, 0x68, 0x72, 0x69, 0x73, 0x74, 0x69, 0x61, 0x6E, 0x2F, 0x72, 0x64, 0x6D, 0x2F,
            0x62, 0x61, 0x73, 0x61, 0x6C, 0x74, 0x5F, 0x63, 0x72, 0x75, 0x73, 0x68, 0x65, 0x72,
            0x5F, 0x6F, 0x74, 0x68, 0x65, 0x72, 0x73, 0x2F, 0x64, 0x69, 0x66, 0x66, 0x75, 0x73,
            0x65, 0x2E, 0x70, 0x6E, 0x67,
        ];

        self.buf.put_slice(&RAW_DATA);
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

                println!("ct : {:#?}",ct);

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

    pub fn read_inv(&mut self) {
        let file = "rdm/buffer2.bin";
        let mut f = File::open(file).unwrap();
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut buffer).ok();

        let buffer_len = buffer.len();
        println!("loaded {:?} into buffer", file);

        println!("buffer size: {}", buffer_len);

        assert_eq!(buffer_len%(4*4*4),0);
        let count = buffer_len/(4*4*4);

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
                mat4.m11, mat4.m12, mat4.m13,
                mat4.m21, mat4.m22, mat4.m23,
                mat4.m31, mat4.m32, mat4.m33
            );
            let rot = Rotation3::from_matrix(&mat3);
            let q = UnitQuaternion::from_rotation_matrix(&rot).inverse().coords;

            let qq = Quaternion::new(q.w,q.x ,q.y,q.z);
            let uq = UnitQuaternion::from_quaternion(qq);

            println!("rots : {:#?}",uq);

            let tx = mat4.m14;
            let ty = mat4.m24;
            let tz = mat4.m34;
            let joint_translatio: Translation3<f32> = Translation3::new(tx, ty, tz);

            

            let inv_bindmat =
                    (uq.to_homogeneous()) * (joint_translatio.to_homogeneous());
            let iv_x = inv_bindmat.m14;
            let iv_y = inv_bindmat.m24;
            let iv_z = inv_bindmat.m34;

            let trans_point = Translation3::new(iv_x, iv_y, iv_z).inverse();

            println!("trans : {:#?}",trans_point);

            let quaternion_mat4 = uq.quaternion().coords;
            assert_relative_eq!(quaternion_mat4.x,input_joints[i].quaternion[0]);
            assert_relative_eq!(quaternion_mat4.y,input_joints[i].quaternion[1]);
            assert_relative_eq!(quaternion_mat4.z,input_joints[i].quaternion[2]);
            assert_relative_eq!(quaternion_mat4.w,input_joints[i].quaternion[3]);

            
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
