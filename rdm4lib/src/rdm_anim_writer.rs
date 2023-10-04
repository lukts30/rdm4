use bytes::{BufMut, BytesMut};

use byteorder::ByteOrder;
use std::{fs, path::PathBuf};
use std::{fs::OpenOptions, io::Write};

use crate::*;

pub struct RdAnimWriter {
    jtable_deref: u32,
    input: RdAnim,
    buf: BytesMut,
}

impl RdAnimWriter {
    fn new(rdm: RdAnim) -> Self {
        let mut rdw = RdAnimWriter {
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
            let path_str_ptr = self.buf.len() as u32;
            let buff_off = 84_usize;
            byteorder::LittleEndian::write_u32(&mut self.buf[buff_off..buff_off + 4], path_str_ptr);
        }
        self.buf.put_slice(export_name);

        let export_name_2 = br"Anno5_Building_Anim_UnCompressed.rmp";
        self.buf.put_u32_le(export_name_2.len() as u32);
        self.buf.put_u32_le(1);
        {
            let file_str_ptr = self.buf.len() as u32;
            let buff_off = 88_usize;
            byteorder::LittleEndian::write_u32(&mut self.buf[buff_off..buff_off + 4], file_str_ptr);
        }
        self.buf.put_slice(export_name_2);

        // MODEL_NAME_PTR
        let model_str = br"basalt_crusher_others_lod0";

        self.buf.put_u32_le(1);
        self.buf.put_u32_le(48);

        {
            let meta_ptr = self.buf.len() as u32;
            let buff_off = 44_usize;
            byteorder::LittleEndian::write_u32(&mut self.buf[buff_off..buff_off + 4], meta_ptr);
        }

        self.buf.put_u32_le(self.buf.len() as u32 + 8 + 48);

        self.buf
            .put_u32_le(self.buf.len() as u32 + 8 + 48 + model_str.len() as u32 + 8 - 4); // -4 advanced: 4 bytes

        info!("SEQUENCE EndTime (Max): {}", self.input.time_max);
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
                let joint_target_str = joint_target_str_u32;
                let buff_off = self.jtable_deref as usize + i * 24;
                byteorder::LittleEndian::write_u32(
                    &mut self.buf[buff_off..buff_off + 4],
                    joint_target_str,
                );
                let joint_target_data = joint_target_str_u32 + collection.name.len() as u32 + 8;
                byteorder::LittleEndian::write_u32(
                    &mut self.buf[(buff_off + 4)..(buff_off + 8)],
                    joint_target_data,
                );
            }
            self.buf.put_slice(collection.name.as_bytes());

            self.buf.put_u32_le(collection.frames.len() as u32);
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

    pub fn write_anim_rdm(self, dir: Option<PathBuf>, create_new: bool) {
        let mut file = dir.unwrap_or_else(|| {
            let f = PathBuf::from("rdm_out");
            let _ = fs::create_dir(&f);
            f
        });
        if file.is_dir() {
            file.push(self.input.name);
        } else {
            let n = file.file_stem().unwrap();
            let anim_name = format!("{}_{}", n.to_string_lossy(), self.input.name);
            file.set_file_name(anim_name);
        }
        file.set_extension("rdm");

        let mut writer = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .create_new(create_new)
            .open(&file)
            .expect("I/O error");
        writer.write_all(&self.buf).expect("I/O error");
    }
}

impl From<RdAnim> for RdAnimWriter {
    fn from(anim: RdAnim) -> Self {
        RdAnimWriter::new(anim)
    }
}
