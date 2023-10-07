#![allow(dead_code, unused_variables)]

use binrw::{binrw, BinWriterExt};
use std::{
    fs::{self, OpenOptions},
    io::SeekFrom,
    path::PathBuf,
};

use crate::{rdm_anim::RdAnim, rdm_container::*, rdm_data_main::RdmHeader2};

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[br(assert(_unknown0_15 == 15))]
pub struct AnimMeta {
    #[bw(args_raw = end)]
    pub name: AnnoPtr<RdmString>,

    #[bw(args_raw = end)]
    pub anims: AnnoPtr<RdmTypedContainer<AnimInner>>,
    pub time_max: u32,
    _unknown0_15: u32,
    _padding: [u8; 32],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
pub struct AnimInner {
    #[bw(args_raw = end)]
    pub j_name: AnnoPtr<RdmString>,
    #[bw(args_raw = end)]
    pub j_data: AnnoPtr<RdmTypedContainer<Frame>>,
    _padding: [u8; 16],
}

#[derive(Debug, Copy, Clone)]
#[binrw]
#[bw(import_raw(end: &mut u64))]
pub struct Frame {
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub time: f32,
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[br(assert(_unknown0_84 == 84))]
pub struct RdmHeader1b {
    _unknown0_84: u32,
    _unknown1: [u8; 12],

    #[bw(args_raw = end)]
    pub meta: AnnoPtr<RdmTypedT<AnimMeta>>,
    _padding: [u8; 28],
}

#[binrw]
#[brw(magic = b"RDM\x01\x14\x00\x00\x00\x00\x00\x00\x00\x04\x00\x00\x00\x1c\x00\x00\x00")]
pub struct RdmAnimFile {
    #[bw(args_raw = RdmContainerArgs {ptr: None, end_offset: crate::rdm_data_main::DataAndPointedToSize::get_direct_and_pointed_data_size(header2)})]
    #[brw(seek_before = SeekFrom::Start(0x00000014))]
    pub header1: RdmTypedT<RdmHeader1b>,

    #[bw(args_raw = RdmContainerArgs::default())]
    #[brw(seek_before = SeekFrom::Start(0x0000004C))]
    pub header2: RdmTypedT<RdmHeader2>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use binrw::{BinReaderExt, BinWriterExt};
    use std::fs;

    #[test]
    fn rdm_anim_serialisation_roundtrip() {
        let data = fs::read("rdm/basalt_crusher_others_work01.rdm").unwrap();
        //let data = fs::read("rdm/basalt_crusher_others_idle01.rdm").unwrap();

        let mut reader = std::io::Cursor::new(&data);
        let rdm: RdmAnimFile = reader.read_ne().unwrap();

        let mut dst = Vec::new();
        let mut writer = std::io::Cursor::new(&mut dst);

        writer
            .write_type_args(&rdm, binrw::Endian::Little, ())
            .unwrap();

        let mut file = fs::File::create("/tmp/anim_out.rdm").unwrap();
        std::io::Write::write_all(&mut file, &dst).unwrap();
        assert_eq!(data, fs::read("/tmp/anim_out.rdm").unwrap())
    }

    #[test]
    fn run_conv() {
        let anim = RdAnim::from("rdm/basalt_crusher_others_work01.rdm");
        let rdaw = RdAnimWriter2::new(anim);
        rdaw.write_anim_rdm(Some("/tmp/".into()), false);
    }
}

pub struct RdAnimWriter2 {
    name: String,
    export: RdmAnimFile,
}

impl RdAnimWriter2 {
    pub fn write_anim_rdm(self, dir: Option<PathBuf>, create_new: bool) {
        let mut file = dir.unwrap_or_else(|| {
            let f = PathBuf::from("rdm_out");
            let _ = fs::create_dir(&f);
            f
        });
        if file.is_dir() {
            file.push(self.name);
        } else {
            let n = file.file_stem().unwrap();
            let anim_name = format!("{}_{}", n.to_string_lossy(), self.name);
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

        writer
            .write_type_args(&self.export, binrw::Endian::Little, ())
            .unwrap();
    }

    pub fn new(anim_input: RdAnim) -> Self {
        use super::*;

        let data = include_bytes!("../rdm/basalt_crusher_others_idle01.rdm");
        let mut reader = std::io::Cursor::new(&data);
        let mut anim: RdmAnimFile = reader.read_ne().unwrap();

        let export_name = br"G:\graphic\danny\Anno5\preproduction\buildings\others\basalt_crusher_others\scenes\basalt_crusher_others_idle01_01.max";

        anim.header2.export_name1.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(RdmContainer {
                info: RdmContainerPrefix {
                    count: export_name.len() as u32,
                    part_size: 1,
                },
                e: rdm_container::VectorN {
                    x: export_name.map(AnnoChar).into(),
                },
            }),
        };

        dbg!(&anim.header2.export_name1);

        let model_str: &[u8; 26] = br"basalt_crusher_others_lod0";
        anim.header1.meta.name.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(RdmContainer {
                info: RdmContainerPrefix {
                    count: model_str.len() as u32,
                    part_size: 1,
                },
                e: rdm_container::VectorN {
                    x: model_str.map(AnnoChar).into(),
                },
            }),
        };

        anim.header1.meta.time_max = anim_input.time_max;

        let mut anim_data: Vec<AnimInner> = vec![];
        for x in anim_input.anim_vec {
            let o: AnimInner = AnimInner {
                j_name: AnnoPtr2(binrw::FilePtr32 {
                    ptr: 0,
                    value: Some(RdmContainer {
                        info: RdmContainerPrefix {
                            count: x.name.as_bytes().len() as u32,
                            part_size: 1,
                        },
                        e: rdm_container::VectorN {
                            x: x.name.as_bytes().iter().map(|c| AnnoChar(*c)).collect(),
                        },
                    }),
                }),
                j_data: AnnoPtr2(binrw::FilePtr32 {
                    ptr: 0,
                    value: Some(RdmContainer {
                        info: RdmContainerPrefix {
                            count: x.frames.len() as u32,
                            part_size: 32,
                        },
                        e: rdm_container::VectorN { x: x.frames },
                    }),
                }),
                _padding: [0; 16],
            };
            anim_data.push(o);
        }
        anim.header1.meta.anims.info.count = anim_data.len() as u32;
        anim.header1.meta.anims.e.x = anim_data;

        RdAnimWriter2 {
            name: anim_input.name,
            export: anim,
        }
    }
}
