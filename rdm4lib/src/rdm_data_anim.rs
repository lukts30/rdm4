use binrw::binread;
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

#[binread]
#[derive(RdmStructSize)]
pub struct AnimMeta {
    pub name: AnnoPtr<RdmString>,

    pub anims: AnnoPtr<RdmTypedContainer<AnimInner>>,
    pub time_max: u32,
    /// Nominal samples per second.
    pub sampling_rate: u32,
    _padding: [u8; 32],
}

#[binread]
#[derive(RdmStructSize)]
pub struct AnimInner {
    pub j_name: AnnoPtr<RdmString>,

    pub j_data: AnnoPtr<RdmTypedContainer<Frame>>,
    _padding: [u8; 16],
}

#[derive(Debug, Copy, Clone)]
#[binread]
#[derive(RdmStructSize)]
pub struct Frame {
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub time: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use binrw::BinReaderExt;
    use std::fs;

    #[test]
    #[cfg(target_os = "linux")]
    fn preserves_animation_bytes_when_read_and_written() {
        use crate::rdm_data_main::{RdmFile, RdmKindAnim};

        let data = fs::read("rdm/basalt_crusher_others_work01.rdm").unwrap();
        //let data = fs::read("rdm/basalt_crusher_others_idle01.rdm").unwrap();

        let mut reader = std::io::Cursor::new(&data);
        let rdm: RdmFile<RdmKindAnim> = reader.read_le().unwrap();

        let mut dst = Vec::new();
        let mut writer = std::io::Cursor::new(&mut dst);

        rdm.write_rdm(&mut writer).unwrap();

        let output_dir = tempfile::tempdir().unwrap();
        let output_path = output_dir.path().join("anim_out.rdm");
        let mut file = fs::File::create(&output_path).unwrap();
        std::io::Write::write_all(&mut file, &dst).unwrap();
        assert_eq!(data, fs::read(output_path).unwrap())
    }

    #[test]
    fn preserves_sampling_rate_when_loading_and_writing_animation() {
        for sampling_rate in [1, 30, 60, 78] {
            let data = include_bytes!("../rdm/basalt_crusher_others_idle01.rdm");
            let mut reader = std::io::Cursor::new(data);
            let mut rdm: RdmFile<RdmKindAnim> = reader.read_le().unwrap();
            rdm.header1.meta_anim.sampling_rate = sampling_rate;
            let mut writer = std::io::Cursor::new(Vec::new());
            rdm.write_rdm(&mut writer).unwrap();

            let anim = RdAnim::new(writer.into_inner(), "idle".into());
            assert_eq!(anim.sampling_rate, sampling_rate);
            let output = RdAnimWriter2::new(anim);
            let mut writer = std::io::Cursor::new(Vec::new());
            output.export.write_rdm(&mut writer).unwrap();
            writer.set_position(0);
            let written: RdmFile<RdmKindAnim> = writer.read_le().unwrap();
            assert_eq!(written.header1.meta_anim.sampling_rate, sampling_rate);
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn writes_loaded_animation_without_errors() {
        let anim = RdAnim::from("rdm/basalt_crusher_others_work01.rdm");
        let rdaw = RdAnimWriter2::new(anim);
        let output_dir = tempfile::tempdir().unwrap();
        rdaw.write_anim_rdm(Some(output_dir.path().into()), false);
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

        info!("Writing animation {:?}", file);
        let mut writer = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .create_new(create_new)
            .open(&file)
            .expect("I/O error");

        self.export.write_rdm(&mut writer).unwrap();
        info!("Wrote animation {:?}", file);
    }

    pub fn new(anim_input: RdAnim) -> Self {
        use super::*;

        let data = include_bytes!("../rdm/basalt_crusher_others_idle01.rdm");
        let mut reader = std::io::Cursor::new(&data);
        let mut anim: RdmFile<RdmKindAnim> = reader.read_le().unwrap();

        let export_name = br"G:\graphic\danny\Anno5\preproduction\buildings\others\basalt_crusher_others\scenes\basalt_crusher_others_idle01_01.max";

        anim.header1.header2.export_name1.0 = RdmFilePtr {
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
        anim.header1.meta_anim.name.0 = RdmFilePtr {
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

        anim.header1.meta_anim.sampling_rate = anim_input.sampling_rate;
        anim.header1.meta_anim.time_max = anim_input.time_max;
        info!("SEQUENCE EndTime (Max): {}", anim_input.time_max);

        let mut anim_data: Vec<AnimInner> = vec![];
        for x in anim_input.anim_vec {
            let o: AnimInner = AnimInner {
                j_name: AnnoPtr2(RdmFilePtr {
                    ptr: 0,
                    value: Some(RdmContainer {
                        info: RdmContainerPrefix {
                            count: x.name.len() as u32,
                            part_size: 1,
                        },
                        storage: rdm_container::VectorN {
                            items: x.name.as_bytes().iter().map(|c| AnnoChar(*c)).collect(),
                        },
                    }),
                }),
                j_data: AnnoPtr2(RdmFilePtr {
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

use crate::rdm_writer::{Describe, Plan, Record};
impl Describe for AnimMeta {
    fn describe(&self, plan: &mut Plan) -> Record {
        let _ = &plan;
        let mut r = Record::new(plan);
        r.offset(self.name.plan(plan));
        r.offset(self.anims.plan(plan));
        r.u32(self.time_max);
        r.u32(self.sampling_rate);
        r.bytes(&self._padding);
        r
    }
}
impl Describe for AnimInner {
    fn describe(&self, plan: &mut Plan) -> Record {
        let _ = &plan;
        let mut r = Record::new(plan);
        r.offset(self.j_name.plan(plan));
        r.offset(self.j_data.plan(plan));
        r.bytes(&self._padding);
        r
    }
}
impl Describe for Frame {
    fn describe(&self, plan: &mut Plan) -> Record {
        let _ = &plan;
        let mut r = Record::new(plan);
        r.floats(&self.rotation);
        r.floats(&self.translation);
        r.floats(&[self.time]);
        r
    }
}
