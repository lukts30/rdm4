use bytes::{BufMut, BytesMut};

use std::fs;
use std::io::Write;

use crate::*;

pub struct RDAnimWriter {
    jtable_deref: u32,
    input: RDAnim,
    buf: BytesMut,
}

impl RDAnimWriter {
    fn new(rdm: RDAnim) -> Self {
        let mut rdw = RDAnimWriter {
            jtable_deref: 0,
            input: rdm,
            buf: BytesMut::with_capacity(5000),
        };
        rdw.put_header();
        rdw.put_frame_collections();
        rdw
    }

    fn put_header(&mut self) {
        static RAW_DATA: [u8; 156] = [
            0x52, 0x44, 0x4D, 0x01, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
            0x00, 0x00, 0x1C, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00,
            0x54, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x55, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x48, 0x00, 0x00, 0x00,
            0xA4, 0x00, 0x00, 0x00, 0x29, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];

        self.buf.put_slice(&RAW_DATA);

        let export_name = br"G:\graphic\danny\Anno5\preproduction\buildings\others\basalt_crusher_others\scenes\basalt_crusher_others_idle01_01.max";

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

        let export_name_2 = br"Anno5_Building_Anim_UnCompressed.rmp";
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

        // MODEL_NAME_PTR
        let model_str = br"basalt_crusher_others_lod0";

        self.buf.put_u32_le(1);
        self.buf.put_u32_le(48);

        {
            let meta_ptr = (self.buf.len() as u32).to_le_bytes();
            let buff_off = 44 as usize;
            self.buf[buff_off] = meta_ptr[0];
            self.buf[buff_off + 1] = meta_ptr[1];
            self.buf[buff_off + 2] = meta_ptr[2];
            self.buf[buff_off + 3] = meta_ptr[3];
        }

        self.buf.put_u32_le(self.buf.len() as u32 + 8 + 48);

        self.buf
            .put_u32_le(self.buf.len() as u32 + 8 + 48 + model_str.len() as u32 + 8 - 4); // -4 advanced: 4 bytes

        debug!("self.input.time_max {}", self.input.time_max);
        self.buf.put_u32_le(self.input.time_max);

        self.buf.put_u32_le(0xF);

        static MODEL_PTR_ZERO: [u8; 32] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        self.buf.put_slice(&MODEL_PTR_ZERO);

        // MODEL_STR

        self.buf.put_u32_le(model_str.len() as u32);
        self.buf.put_u32_le(1);

        self.buf.put_slice(model_str);

        // joint table

        self.buf.put_u32_le(self.input.anim_vec.len() as u32);
        self.buf.put_u32_le(24);

        static EMPTY_TABLE_ENTRY: [u8; 24] = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        self.jtable_deref = self.buf.len() as u32;

        for _ in 0..self.input.anim_vec.len() {
            self.buf.put_slice(&EMPTY_TABLE_ENTRY);
        }
    }

    fn put_frame_collections(&mut self) {
        for (i, collection) in self.input.anim_vec.iter().enumerate() {
            self.buf.put_u32_le(collection.name.len() as u32);
            self.buf.put_u32_le(1);

            //TODO only ascii
            {
                let joint_target_str_u32 = self.buf.len() as u32;
                let joint_target_str = joint_target_str_u32.to_le_bytes();
                let buff_off = self.jtable_deref as usize + i * 24;
                self.buf[buff_off] = joint_target_str[0];
                self.buf[buff_off + 1] = joint_target_str[1];
                self.buf[buff_off + 2] = joint_target_str[2];
                self.buf[buff_off + 3] = joint_target_str[3];

                let joint_target_data =
                    (joint_target_str_u32 + collection.name.len() as u32 + 8).to_le_bytes();
                self.buf[buff_off + 4] = joint_target_data[0];
                self.buf[buff_off + 5] = joint_target_data[1];
                self.buf[buff_off + 6] = joint_target_data[2];
                self.buf[buff_off + 7] = joint_target_data[3];
            }
            self.buf.put_slice(&collection.name.as_bytes());

            self.buf.put_u32_le(collection.len);
            self.buf.put_u32_le(32);

            for frame in collection.frames.iter() {
                self.buf.put_f32_le(frame.rotation[0]);
                self.buf.put_f32_le(frame.rotation[1]);
                self.buf.put_f32_le(frame.rotation[2]);
                self.buf.put_f32_le(frame.rotation[3]);

                self.buf.put_f32_le(frame.translation[0]);
                self.buf.put_f32_le(frame.translation[1]);
                self.buf.put_f32_le(frame.translation[2]);

                self.buf.put_f32_le(frame.time);
            }
        }
    }

    pub fn write_anim_rdm(self) {
        let _ = fs::create_dir("rdm_out");

        let mut writer = fs::File::create("rdm_out/anim.rdm").expect("I/O error");
        writer.write_all(&self.buf.to_vec()).expect("I/O error");
    }
}

impl From<RDAnim> for RDAnimWriter {
    fn from(anim: RDAnim) -> Self {
        RDAnimWriter::new(anim)
    }
}
