#![allow(dead_code, unused_variables)]

use std::io::SeekFrom;

use binrw::{binrw, BinRead, BinWrite};

use crate::rdm_container::*;

#[binrw]
#[bw(import_raw(end: &mut u64))]
struct ModelName {
    #[bw(args_raw = end)]
    name: AnnoPtr<RdmString>,
    _padding: [u8; 24],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
pub struct VertId {
    #[bw(args_raw = end)]
    pub rdm_container: AnnoPtr<RdmTypedContainer<crate::vertex::VertexIdentifier>>,
    _padding: [u8; 20],
}

#[binrw]
#[bw(import_raw(_dst: &mut u64))]
pub struct MeshInfo {
    pub start_index_location: u32,
    pub index_count: u32,
    pub material: u32,
    _padding: [u8; 16],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
pub struct Meta {
    #[bw(args_raw = end)]
    model_name: AnnoPtr<RdmTypedT<ModelName>>,
    #[bw(args_raw = end)]
    pub format_identifiers: AnnoPtr<RdmTypedT<VertId>>,
    #[bw(args_raw = end)]
    unknown: AnnoPtr<RdmUntypedContainer>,

    #[bw(args_raw = {
        dbg!("v" , &end);
        *end += mesh_info.get_direct_and_pointed_data_size();
        dbg!("v" , &end);
        end
    })]
    pub vertex: AnnoPtr<RdmUntypedContainer>,
    #[bw(args_raw = end)]
    pub triangles: AnnoPtr<RdmTypedContainer<AnnoU16>>,

    #[bw(args_raw = {
        let negative_off = mesh_info.get_direct_and_pointed_data_size() + vertex.get_direct_and_pointed_data_size() + triangles.get_direct_and_pointed_data_size();
        *end -= negative_off;
        end
    })]
    pub mesh_info: AnnoPtr<RdmTypedContainer<MeshInfo>>,

    #[br(ignore)]
    #[bw(calc = {
        let off = vertex.get_direct_and_pointed_data_size() + triangles.get_direct_and_pointed_data_size();
        *end += off;
        dbg!("reset to end" , &end); 
    })]
    d: (),

    _padding_ff: [u8; 4], // 0x_FF_FF_FF_FF or 0x0
    _padding_box: [u8; 24],
    _padding_zero: [u8; 40],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
struct RdmBlobToMat {
    #[bw(args_raw = end)]
    mat: AnnoPtr<RdmTypedT<RdmMat>>,
    _padding: [u8; 24],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
struct RdmMat {
    #[bw(args_raw = end)]
    name: AnnoPtr<RdmString>,
    #[bw(args_raw = end)]
    png: AnnoPtr<RdmString>,
    _padding: [u8; 40],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
pub struct RdmHeader1 {
    _data0: [u8; 4],
    #[bw(args_raw = end)]
    pub meta: AnnoPtr<RdmTypedT<Meta>>,
    #[bw(args_raw = end)]
    rdm_blob_to_mat: AnnoPtr<RdmTypedContainer<RdmBlobToMat>>,
    skin: u32,
    _data: [u8; 48 - 4 * 4],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
pub struct RdmHeader2 {
    #[bw(args_raw = end)]
    export_name1: AnnoPtr<RdmString>,
    #[bw(args_raw = end)]
    export_name2: AnnoPtr<RdmString>,
    _data: [u8; 72 - 8],
}

#[binrw]
#[br(magic = b"RDM\x01\x14\x00\x00\x00\x00\x00\x00\x00\x04\x00\x00\x00\x1c\x00\x00\x00")]
#[bw(magic = b"\x52\x44\x4d\x01\x14\x00\x00\x00\x00\x00\x00\x00\x04\x00\x00\x00\x1c\x00\x00\x00")]
pub struct RdmFile {
    #[bw(args_raw = RdmContainerArgs {ptr: None, end_offset: header2.get_direct_and_pointed_data_size()})]
    #[brw(seek_before = SeekFrom::Start(0x00000014))]
    pub header1: RdmTypedT<RdmHeader1>,

    #[bw(args_raw = RdmContainerArgs::default())]
    #[brw(seek_before = SeekFrom::Start(0x0000004C))]
    pub header2: RdmTypedT<RdmHeader2>,
}

pub trait DataAndPointedToSize {
    fn get_direct_and_pointed_data_size(&self) -> u64;
}
impl DataAndPointedToSize for RdmHeader2 {
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        72 + self.export_name1.get_direct_and_pointed_data_size()
            + self.export_name2.get_direct_and_pointed_data_size()
    }
}

impl DataAndPointedToSize for MeshInfo {
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        28
    }
}

impl<Z> DataAndPointedToSize for RdmContainer<true, Z>
where
    Z: VectorSize,
    Z::Data: DataAndPointedToSize,
    Z::Data: for<'a> BinRead<Args<'a> = u32> + 'static,
    Z::Data: for<'a> BinWrite<Args<'a> = &'a mut u64> + 'static,
{
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        8 + self.e.get_direct_and_pointed_data_size()
    }
}

impl<Z> DataAndPointedToSize for RdmContainer<false, Z>
where
    Z: VectorSize,
    Z::Data: DataAndPointedToSize,
    Z::Data: for<'a> BinRead<Args<'a> = u32> + 'static,
    Z::Data: for<'a> BinWrite<Args<'a> = &'a mut u64> + 'static,
{
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        (self.info.count * self.info.part_size) as u64 + 8
    }
}

impl<Z> DataAndPointedToSize for VectorN<Z>
where
    Z: DataAndPointedToSize,
    Z: for<'a> BinRead<Args<'a> = ()> + 'static,
    Z: for<'a> BinWrite<Args<'a> = &'a mut u64> + 'static,
{
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        let mut sum = 0;
        for x in self.x.iter() {
            sum += x.get_direct_and_pointed_data_size();
        }
        sum
    }
}

impl<Z> DataAndPointedToSize for Vector1<Z>
where
    Z: DataAndPointedToSize,
    Z: for<'a> BinRead<Args<'a> = ()> + 'static,
    Z: for<'a> BinWrite<Args<'a> = &'a mut u64> + 'static,
{
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        self.x.get_direct_and_pointed_data_size()
    }
}

impl DataAndPointedToSize for AnnoChar {
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        1
    }
}

impl DataAndPointedToSize for AnnoU8 {
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        1
    }
}

impl DataAndPointedToSize for AnnoU16 {
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        2
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use binrw::{BinReaderExt, BinWriterExt};

    #[test]
    fn it_works() {
        let data = fs::read("/home/lukas/Downloads/residence_tier_02_estate_02_lod2.rdm")
            .expect("Unable to read file");

        let mut reader = std::io::Cursor::new(&data);

        let rdm: RdmFile = reader.read_ne().unwrap();

        dbg!(rdm.header2.get_direct_and_pointed_data_size());

        let negative_off = rdm
            .header1
            .meta
            .mesh_info
            .get_direct_and_pointed_data_size()
            + rdm.header1.meta.vertex.get_direct_and_pointed_data_size()
            + rdm
                .header1
                .meta
                .triangles
                .get_direct_and_pointed_data_size();
        dbg!(negative_off);

        let mut dst = Vec::new();
        let mut writer = std::io::Cursor::new(&mut dst);

        writer
            .write_type_args(&rdm, binrw::Endian::Little, ())
            .unwrap();

        let mut file = fs::File::create("/tmp/rdm_out.rdm").unwrap();
        std::io::Write::write_all(&mut file, &dst).unwrap();
    }
}
