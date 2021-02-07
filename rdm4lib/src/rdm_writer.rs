use bytes::{BufMut, BytesMut};

use std::{convert::TryInto, fs, path::PathBuf};
use std::{fs::OpenOptions, io::Write};

use crate::*;
use byteorder::ByteOrder;

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
            buf: BytesMut::with_capacity(64000),
        };

        rdw.put_header();
        rdw.put_vertex_buffer();
        rdw.put_indexed_triangle_list();

        rdw.put_blob();

        if rdw.input.has_skin() {
            rdw.put_skin();
        } else {
            // RAW_DATA is from template with anim
            // TODO clean up this mess move make the inverse in put_skin
            // (repleace 0x_FF_FF_FF_FF in RAW_DATA with zeros for skin)
            let buff_off = (rdw.meta_deref + 24) as usize;
            byteorder::LittleEndian::write_u32(
                &mut rdw.buf[buff_off..buff_off + 4],
                0x_FF_FF_FF_FF,
            );
        }

        rdw
    }

    fn put_header(&mut self) {
        // 0x72, 0x58,0x01, 0x00
        static RAW_DATA: [u8; 156] = [
            0x52, 0x44, 0x4D, 0x01, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
            0x00, 0x00, 0x1C, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00,
            0x54, 0x00, 0x00, 0x00, 0x4B, 0x01, 0x00, 0x00, 0x7F, 0x57, 0x01, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
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
            let path_str_ptr = self.buf.len() as u32;
            let buff_off = 84;
            byteorder::LittleEndian::write_u32(&mut self.buf[buff_off..buff_off + 4], path_str_ptr);
        }
        self.buf.put_slice(export_name);

        let export_name_2 = br"Anno5_Building_Skin_1Blend.rmp";
        self.buf.put_u32_le(export_name_2.len() as u32);
        self.buf.put_u32_le(1);
        {
            let file_str_ptr = self.buf.len() as u32;
            let buff_off = 88;
            byteorder::LittleEndian::write_u32(&mut self.buf[buff_off..buff_off + 4], file_str_ptr);
        }
        self.buf.put_slice(export_name_2);

        // meta table
        {
            self.buf.put_u32_le(1);
            self.buf.put_u32_le(92);

            {
                let meta_ptr = self.buf.len() as u32;
                self.meta_deref = meta_ptr;
                let buff_off = 32;
                byteorder::LittleEndian::write_u32(&mut self.buf[buff_off..buff_off + 4], meta_ptr);
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

        let id_ptr;
        {
            // VERTEX_FORMAT_IDENTIFIER_PTR

            static VERTEX_FORMAT_IDENTIFIER_PTR_ZERO: [u8; 16] = [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00,
            ];

            self.buf.put_u32_le(1);
            self.buf.put_u32_le(24);

            {
                let meta_id_ptr = self.buf.len() as u32;
                let buff_off = (self.meta_deref + 4) as usize;
                byteorder::LittleEndian::write_u32(
                    &mut self.buf[buff_off..buff_off + 4],
                    meta_id_ptr,
                );
            }
            id_ptr = self.buf.len() as u32 + 8 + 24;
            self.buf.put_u32_le(id_ptr);

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
            // VERTEX_FORMAT_BYTE_IDENTIFIERS
            // 4 bytes: unique value
            //      e.g XML `VertexFormat` P4h_N4b_T2h_I4b_W4b
            // 4 bytes: unit size   0x06 u16
            //                      0x05 u8
            // 4 bytes: unit interpretation ?
            // 4 bytes: unit count

            self.buf.put_u32_le(self.input.vertex.identifiers_len());
            self.buf.put_u32_le(16);
            assert_eq!(self.buf.len() as u32, id_ptr);
            self.buf.put_slice(self.input.vertex.identifiers_as_bytes());
        }

        {
            // unknown const
            self.buf.put_u32_le(1);
            self.buf.put_u32_le(20);

            {
                let meta_unknown_ptr = self.buf.len() as u32;
                let buff_off = (self.meta_deref + 8) as usize;
                byteorder::LittleEndian::write_u32(
                    &mut self.buf[buff_off..buff_off + 4],
                    meta_unknown_ptr,
                );
            }

            static UNKNOWN: [u8; 20] = [
                0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ];
            self.buf.put_slice(&UNKNOWN);
        }

        // for each MeshInstance.
        {
            self.buf
                .put_u32_le(self.input.mesh_info.len().try_into().unwrap());
            self.buf.put_u32_le(28);

            // pointer to the first MeshInstance
            {
                let triangle_count_ptr = self.buf.len() as u32;
                let buff_off = (self.meta_deref + 20) as usize;
                byteorder::LittleEndian::write_u32(
                    &mut self.buf[buff_off..buff_off + 4],
                    triangle_count_ptr,
                );
            }
            for submesh in self.input.mesh_info.iter() {
                self.buf.put_u32_le(submesh.start_index_location);
                self.buf.put_u32_le(submesh.index_count);
                self.buf.put_u32_le(submesh.mesh);
                static ZERO_16_OF_28: [u8; 16] = [
                    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00,
                ];

                self.buf.put_slice(&ZERO_16_OF_28);
            }
        }

        assert_eq!(
            self.buf.len(),
            580 + self.input.vertex.identifiers_as_bytes().len() + 28 * self.input.mesh_info.len()
        );
    }

    #[cfg(target_endian = "little")]
    fn put_vertex_buffer(&mut self) {
        self.buf.put_u32_le(self.input.vertex.len());
        self.buf.put_u32_le(self.input.vertex.get_size());

        {
            let vertex_ptr = self.buf.len() as u32;
            let buff_off = (self.meta_deref + RDModell::VERTEX_META) as usize;
            byteorder::LittleEndian::write_u32(&mut self.buf[buff_off..buff_off + 4], vertex_ptr);
        }
        let start = self.buf.len();
        self.buf.put_slice(self.input.vertex.as_bytes());
        let end = self.buf.len();
        let written = end - start;
        //assert_eq!(written as u32,(self.input.vertices_count/3)*self.input.vertex_buffer_size);
        assert_eq!(
            written as u32,
            self.input.vertex.len() * self.input.vertex.get_size()
        );
    }

    fn put_indexed_triangle_list(&mut self) {
        self.buf.put_u32_le(self.input.triangles_idx_count);
        self.buf.put_u32_le(2);

        {
            let triangle_list_ptr = self.buf.len() as u32;
            let buff_off = (self.meta_deref + RDModell::TRIANGLES_META) as usize;
            byteorder::LittleEndian::write_u32(
                &mut self.buf[buff_off..buff_off + 4],
                triangle_list_ptr,
            );
        }

        let mut p = 0;
        for triangle in self.input.triangle_indices.iter() {
            self.buf.put_u16_le(triangle.indices[0]);
            self.buf.put_u16_le(triangle.indices[1]);
            self.buf.put_u16_le(triangle.indices[2]);
            p = p.max(triangle.indices[0]);
            p = p.max(triangle.indices[1]);
            p = p.max(triangle.indices[2]);
        }
        error!("max idx: {}", p);
    }

    fn put_blob(&mut self) {
        {
            let blob_ptr = self.buf.len() as u32 + 8;
            let buff_off = 36;
            byteorder::LittleEndian::write_u32(&mut self.buf[buff_off..buff_off + 4], blob_ptr);
        }

        let start = self.buf.len();

        {
            // unknown png

            self.buf.put_u32_le(1);
            self.buf.put_u32_le(28);

            self.buf.put_u32_le(self.buf.len() as u32 + 4 + 24 + 8);

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
            let skin_ptr_ptr = self.buf.len() as u32;
            let buff_off = 40;
            byteorder::LittleEndian::write_u32(&mut self.buf[buff_off..buff_off + 4], skin_ptr_ptr);
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

                trace!("ct : {:#?}", ct);

                let bindmat = (ct.to_homogeneous()) * (uq.to_homogeneous()) * Matrix4::identity();

                let inv_bindmat = bindmat.try_inverse().unwrap();

                trace!("{}", uq.quaternion().coords);

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
                let jname_ptr = self.buf.len() as u32;
                let buff_off = *name_ptr_itr.next().unwrap();
                byteorder::LittleEndian::write_u32(
                    &mut self.buf[buff_off..buff_off + 4],
                    jname_ptr,
                );
            }

            self.buf.put_slice(joint.name.as_ref());
        }
    }

    pub fn write_rdm(self, dir: Option<PathBuf>) {
        let mut file = dir.unwrap_or_else(|| {
            let f = PathBuf::from("rdm_out");
            let _ = fs::create_dir(&f);
            f
        });
        if file.is_dir() {
            file.push("out.rdm");
        }

        let mut writer = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file)
            .expect("I/O error");
        writer.write_all(&self.buf).expect("I/O error");
    }
}

impl From<RDModell> for RDWriter {
    fn from(rdm: RDModell) -> Self {
        RDWriter::new(rdm)
    }
}

pub trait PutVertex {
    fn put_p4h(&mut self, p4h: &Position<f16>);
    fn put_n4b(&mut self, n4b: &Normal<u8>);
    fn put_g4b(&mut self, g4b: &Tangent<u8>);
    fn put_b4b(&mut self, b4b: &Bitangent);
    fn put_t2h(&mut self, t2h: &Texcoord<f16>);
    fn put_i4b(&mut self, i4b: &I4b);
    fn put_w4b(&mut self, w4b: &W4b);
    fn put_c4c(&mut self, c4c: &C4c);
}

impl PutVertex for BytesMut {
    fn put_p4h(&mut self, p4h: &Position<f16>) {
        self.put_u16_le(p4h.pos[0].to_bits());
        self.put_u16_le(p4h.pos[1].to_bits());
        self.put_u16_le(p4h.pos[2].to_bits());
        self.put_u16_le(p4h.pos[3].to_bits());
    }
    fn put_n4b(&mut self, n4b: &Normal<u8>) {
        self.put_slice(&n4b.normals);
    }
    fn put_g4b(&mut self, g4b: &Tangent<u8>) {
        self.put_slice(&g4b.tangent);
    }
    fn put_b4b(&mut self, b4b: &Bitangent) {
        self.put_slice(&b4b.binormal);
    }
    fn put_t2h(&mut self, t2h: &Texcoord<f16>) {
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
