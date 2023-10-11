use binrw::{binrw, BinWriterExt};
use rdm_derive::RdmStructSize;
use std::{
    fs::{self, OpenOptions},
    path::PathBuf,
};

use crate::{rdm_anim::RdAnim, rdm_container::*};
use crate::{
    rdm_data_main::{RdmFile, RdmKindAnim},
    RDMStructSizeTr,
};

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[br(assert(_unknown0_15 == 15))]
#[derive(RdmStructSize)]
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
#[derive(RdmStructSize)]
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
#[derive(RdmStructSize)]
pub struct Frame {
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub time: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use binrw::{BinReaderExt, BinWriterExt};
    use std::fs;

    #[test]
    #[cfg(target_os = "linux")]
    fn rdm_anim_serialisation_roundtrip() {
        use crate::rdm_data_main::{RdmFile, RdmKindAnim};

        let data = fs::read("rdm/basalt_crusher_others_work01.rdm").unwrap();
        //let data = fs::read("rdm/basalt_crusher_others_idle01.rdm").unwrap();

        let mut reader = std::io::Cursor::new(&data);
        let rdm: RdmFile<RdmKindAnim> = reader.read_le().unwrap();

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
    #[cfg(target_os = "linux")]
    fn run_conv() {
        let anim = RdAnim::from("rdm/basalt_crusher_others_work01.rdm");
        let rdaw = RdAnimWriter2::new(anim);
        rdaw.write_anim_rdm(Some("/tmp/".into()), false);
    }
}

pub struct RdAnimWriter2 {
    name: String,
    export: RdmFile<RdmKindAnim>,
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
        let mut anim: RdmFile<RdmKindAnim> = reader.read_le().unwrap();

        let export_name = br"G:\graphic\danny\Anno5\preproduction\buildings\others\basalt_crusher_others\scenes\basalt_crusher_others_idle01_01.max";

        anim.header1.header2.export_name1.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(RdmContainer {
                info: RdmContainerPrefix {
                    count: export_name.len() as u32,
                    part_size: 1,
                },
                storage: rdm_container::VectorN {
                    items: export_name.map(AnnoChar).into(),
                },
            }),
        };

        let model_str: &[u8; 26] = br"basalt_crusher_others_lod0";
        anim.header1.meta_anim.name.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(RdmContainer {
                info: RdmContainerPrefix {
                    count: model_str.len() as u32,
                    part_size: 1,
                },
                storage: rdm_container::VectorN {
                    items: model_str.map(AnnoChar).into(),
                },
            }),
        };

        anim.header1.meta_anim.time_max = anim_input.time_max;
        info!("SEQUENCE EndTime (Max): {}", anim_input.time_max);

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
                        storage: rdm_container::VectorN {
                            items: x.name.as_bytes().iter().map(|c| AnnoChar(*c)).collect(),
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
                        storage: rdm_container::VectorN { items: x.frames },
                    }),
                }),
                _padding: [0; 16],
            };
            anim_data.push(o);
        }
        anim.header1.meta_anim.anims.info.count = anim_data.len() as u32;
        anim.header1.meta_anim.anims.storage.items = anim_data;

        RdAnimWriter2 {
            name: anim_input.name,
            export: anim,
        }
    }
}
