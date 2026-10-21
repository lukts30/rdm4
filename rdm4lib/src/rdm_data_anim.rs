use binrw::binread;

use std::{
    fs::{self, OpenOptions},
    path::PathBuf,
};

use crate::rdm_data_main::{RdmFile, RdmKindAnim};
use crate::{rdm_anim::RdAnim, rdm_container::*};

#[binread]
pub struct AnimMeta {
    #[br(parse_with = read_bytes)]
    pub name: Vec<u8>,

    #[br(parse_with = read_array)]
    pub anims: Vec<AnimInner>,
    pub time_max: u32,
    /// Nominal samples per second.
    pub sampling_rate: u32,
    _padding: [u8; 32],
}

#[binread]
pub struct AnimInner {
    #[br(parse_with = read_bytes)]
    pub j_name: Vec<u8>,

    #[br(parse_with = read_array)]
    pub j_data: Vec<Frame>,
    _padding: [u8; 16],
}

#[derive(Debug, Copy, Clone)]
#[binread]
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
            rdm.header1.meta_anim.as_mut().unwrap().sampling_rate = sampling_rate;
            let mut writer = std::io::Cursor::new(Vec::new());
            rdm.write_rdm(&mut writer).unwrap();

            let anim = RdAnim::new(writer.into_inner(), "idle".into());
            assert_eq!(anim.sampling_rate, sampling_rate);
            let output = RdAnimWriter2::new(anim);
            let mut writer = std::io::Cursor::new(Vec::new());
            output.export.write_rdm(&mut writer).unwrap();
            writer.set_position(0);
            let written: RdmFile<RdmKindAnim> = writer.read_le().unwrap();
            assert_eq!(
                written.header1.meta_anim.as_ref().unwrap().sampling_rate,
                sampling_rate
            );
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

        anim.header1.header2.as_mut().unwrap().export_name1 = Some(export_name.to_vec());
        let meta = anim.header1.meta_anim.as_mut().unwrap();
        meta.name = br"basalt_crusher_others_lod0".to_vec();
        meta.sampling_rate = anim_input.sampling_rate;
        meta.time_max = anim_input.time_max;
        meta.anims = anim_input
            .anim_vec
            .into_iter()
            .map(|track| AnimInner {
                j_name: track.name.into_bytes(),
                j_data: track.frames,
                _padding: [0; 16],
            })
            .collect();

        RdAnimWriter2 {
            name: anim_input.name,
            export: anim,
        }
    }
}

use crate::rdm_writer::{Describe, Plan, Record};
impl Describe for AnimMeta {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut r = Record::new(plan);
        r.offset(Some(plan.bytes(&self.name)));
        r.offset(Some(plan.array(&self.anims)));
        r.u32(self.time_max);
        r.u32(self.sampling_rate);
        r.bytes(&self._padding);
        r
    }
}
impl Describe for AnimInner {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut r = Record::new(plan);
        r.offset(Some(plan.bytes(&self.j_name)));
        r.offset(Some(plan.array(&self.j_data)));
        r.bytes(&self._padding);
        r
    }
}
impl Describe for Frame {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut r = Record::new(plan);
        r.floats(&self.rotation);
        r.floats(&self.translation);
        r.floats(&[self.time]);
        r
    }
}
