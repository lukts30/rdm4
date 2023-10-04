#![allow(dead_code, unused_variables)]

use binrw::binrw;
use std::io::SeekFrom;

use crate::{rdm_container::*, rdm_data_main::RdmHeader2};

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[br(assert(_data2 == 0xF))]
pub struct AnimMeta {
    #[bw(args_raw = end)]
    pub name: AnnoPtr<RdmString>,

    #[bw(args_raw = end)]
    pub anims: AnnoPtr<RdmTypedContainer<AnimInner>>,
    pub time_max: u32,
    _data2: u32,
    _data: [u8; 32],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
pub struct AnimInner {
    #[bw(args_raw = end)]
    pub j_name: AnnoPtr<RdmString>,
    #[bw(args_raw = end)]
    pub j_data: AnnoPtr<RdmTypedContainer<Frame>>,
    _data: [u8; 16],
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
#[br(assert(_data0 == 84))]
pub struct RdmHeader1b {
    _data0: u32,
    _data1: [u8; 12],

    #[bw(args_raw = end)]
    pub meta: AnnoPtr<RdmTypedT<AnimMeta>>,
    _data: [u8; 48 - 20],
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
        //dbg!(rdm.header1.meta.time_max);
        //let v = &rdm.header1.meta.anims;
        //dbg!(&v[1].j_data[1]);

        let mut dst = Vec::new();
        let mut writer = std::io::Cursor::new(&mut dst);

        writer
            .write_type_args(&rdm, binrw::Endian::Little, ())
            .unwrap();

        let mut file = fs::File::create("/tmp/anim_out.rdm").unwrap();
        std::io::Write::write_all(&mut file, &dst).unwrap();

        dbg!(file.metadata().unwrap().len());
        dbg!(data.len());
        assert_eq!(data, fs::read("/tmp/anim_out.rdm").unwrap())
    }
}
