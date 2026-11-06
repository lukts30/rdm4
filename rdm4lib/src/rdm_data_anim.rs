//! Legacy animation reader schema and compatibility exports.

use binrw::binread;
use rdm_derive::RdmStructSize;

use crate::RDMStructSizeTr;

use crate::rdm_container::*;
/// Compatibility paths for the current keyframe and animation output helper.
pub use crate::{rdm::RdAnimWriter2, rdm_anim::Frame};

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
    pub j_data: AnnoPtr<RdmTypedContainer<RdmLegacyFrame>>,
    _padding: [u8; 16],
}

/// Independent wire record used only by the legacy reader.
#[derive(Debug, Copy, Clone)]
#[binread]
#[derive(RdmStructSize)]
pub struct RdmLegacyFrame {
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub time: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        rdm_anim::RdAnim,
        rdm_data_main::{RdmFile, RdmKindAnim},
    };
    use binrw::BinReaderExt;

    #[test]
    fn preserves_sampling_rate_when_loading_and_writing_animation() {
        for sampling_rate in [1u32, 30, 60, 78] {
            let mut data = include_bytes!("../rdm/basalt_crusher_others_idle01.rdm").to_vec();
            let animation = u32::from_le_bytes(data[0x2c..0x30].try_into().unwrap()) as usize;
            data[animation + 12..animation + 16].copy_from_slice(&sampling_rate.to_le_bytes());
            let anim = RdAnim::new(data, "idle".into());
            assert_eq!(anim.sampling_rate, sampling_rate);
            let document = crate::rdm::RdmDocument::from(anim);
            let writer = std::io::Cursor::new(Vec::new());
            let mut writer = crate::rdm::RdmWriter::new(writer, crate::rdm::RdmEndian::Little)
                .write_root(&document)
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
