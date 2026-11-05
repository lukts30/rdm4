use binrw::binrw;
use rdm_derive::RdmStructSize;
use std::{
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::PathBuf,
};

use crate::RDMStructSizeTr;
use crate::{rdm_anim::RdAnim, rdm_container::*};

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[derive(RdmStructSize)]
pub struct AnimMeta {
    #[bw(args_raw = end)]
    pub name: AnnoPtr<RdmString>,

    #[bw(args_raw = end)]
    pub anims: AnnoPtr<RdmTypedContainer<AnimInner>>,
    pub time_max: u32,
    /// Nominal samples per second.
    pub sampling_rate: u32,
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
    use crate::rdm_data_main::{RdmFile, RdmKindAnim};
    use binrw::{BinReaderExt, BinWriterExt};
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

        writer
            .write_type_args(&rdm, binrw::Endian::Little, ())
            .unwrap();

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
            writer.write_le(&rdm).unwrap();

            let anim = RdAnim::new(writer.into_inner(), "idle".into());
            assert_eq!(anim.sampling_rate, sampling_rate);
            let output = RdAnimWriter2::new(anim);
            let writer = std::io::Cursor::new(Vec::new());
            let mut writer = crate::rdm::RdmWriter::new(writer, crate::rdm::RdmEndian::Little)
                .write_root(&output.export)
                .unwrap();
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

/// Compatibility entrypoint backed by the current document writer.
pub struct RdAnimWriter2 {
    name: String,
    export: crate::rdm::RdmDocument,
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
        let writer = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .create_new(create_new)
            .open(&file)
            .expect("I/O error");

        crate::rdm::RdmWriter::new(BufWriter::new(writer), crate::rdm::RdmEndian::Little)
            .write_root(&self.export)
            .expect("cannot write animation records")
            .flush()
            .expect("cannot flush animation output");
        info!("Wrote animation {:?}", file);
    }

    /// Adapt the animation for the current record writer.
    pub fn new(anim_input: RdAnim) -> Self {
        Self {
            name: anim_input.name.clone(),
            export: anim_input.into(),
        }
    }
}
