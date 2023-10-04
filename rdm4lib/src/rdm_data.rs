#![allow(dead_code, unused_variables)]

use std::{
    fs::{self, OpenOptions},
    io::SeekFrom,
    path::PathBuf,
};

use binrw::{binrw, BinRead, BinWrite, BinWriterExt};

use crate::{rdm_container::*, vertex::VertexIdentifier, RdModell};

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
    unknown_shader_id: u8,
    _padding: [u8; 20 - 1],
}

#[derive(Debug, Clone)]
#[binrw]
#[bw(import_raw(_dst: &mut u64))]
pub struct MeshInfo {
    pub start_index_location: u32,
    pub index_count: u32,
    pub material: u32,
    pub _padding: [u8; 16],
}

impl MeshInfo {
    pub fn get_max_material(instances: &[MeshInfo]) -> u32 {
        instances.iter().map(|e| e.material).max().unwrap()
    }
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
pub struct RdmBlobToJoint {
    #[bw(args_raw = end)]
    pub joint: AnnoPtr<RdmTypedContainer<RdmJoint>>,
    _padding: [u8; 32 - 4],
}

#[derive(Debug)]
#[binrw]
#[bw(import_raw(end: &mut u64))]
pub struct RdmJoint {
    #[bw(args_raw = end)]
    pub name: AnnoPtr<RdmString>,
    pub t: [f32; 3],
    pub r: [f32; 4],
    pub parent_id: u8,
    _padding: [u8; 84 - 33],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
pub struct RdmHeader1 {
    _data0: [u8; 4],
    #[bw(args_raw = end)]
    pub meta: AnnoPtr<RdmTypedT<Meta>>,
    #[bw(args_raw = end)]
    rdm_blob_to_mat: AnnoPtr<RdmTypedContainer<RdmBlobToMat>>,

    #[bw(args_raw = end)]
    // Nullable
    pub skin: AnnoPtr<RdmTypedT<RdmBlobToJoint>>,

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
    use super::*;
    use binrw::{BinReaderExt, BinWriterExt};
    use std::fs;

    #[test]
    fn rdm_file_serialisation_roundtrip() {
        //let data = fs::read("rdm/fishery_others_cutout_lod0.rdm").unwrap();
        let data = fs::read("rdm/basalt_crusher_others_lod0.rdm").unwrap();

        let mut reader = std::io::Cursor::new(&data);

        let rdm: RdmFile = reader.read_ne().unwrap();

        dbg!(&rdm.header1.skin.e.x.joint.0.e.x);

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

        dbg!(file.metadata().unwrap().len());
        dbg!(data.len());
        assert_eq!(data, fs::read("/tmp/rdm_out.rdm").unwrap())
    }
}

pub struct RdWriter2 {
    inner: RdmFile,
}

impl RdWriter2 {
    pub fn write_rdm(self, dir: Option<PathBuf>, create_new: bool) {
        let mut file = dir.unwrap_or_else(|| {
            let f = PathBuf::from("rdm_out");
            let _ = fs::create_dir(&f);
            f
        });
        if file.is_dir() {
            file.push("out.rdm");
        }

        let mut writer = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .create_new(create_new)
            .open(&file)
            .expect("I/O error");

        writer
            .write_type_args(&self.inner, binrw::Endian::Little, ())
            .unwrap();
    }

    pub fn new(rdm_in: RdModell) -> RdWriter2 {
        use super::*;
        // let data = fs::read("rdm/basalt_crusher_others_lod0.rdm").unwrap();
        let data = include_bytes!("../rdm/basalt_crusher_others_lod0.rdm");

        let mut reader = std::io::Cursor::new(&data);
        let mut rdm: RdmFile = reader.read_ne().unwrap();

        let export_name = br"\\060.alpha\data\Art\graphic_backup\christian\#ANNO5\buildings\others\basalt_crusher_others\Lowpoly\basalt_crusher_others_low_05.max";

        dbg!(&rdm.header2.export_name1);

        rdm.header2.export_name1.0 = binrw::FilePtr32 {
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

        // unknown maybe shader id
        // 0: no anim
        // 1: _Ib4
        // 2:
        // 3: I4b_W4b (eve)
        // 4: I4b_W4b (other npc)
        rdm.header1.meta.format_identifiers.unknown_shader_id =
            if rdm_in.has_skin() { 1 } else { 0 };

        rdm.header1.meta._padding_ff = if rdm_in.has_skin() {
            Default::default()
        } else {
            [0xFF; 4]
        };

        rdm.header1.meta.format_identifiers.rdm_container.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(
                RdmContainer::<true, rdm_container::Dynamic<VertexIdentifier>> {
                    info: RdmContainerPrefix {
                        count: rdm_in.vertex.identifiers.len() as u32,
                        part_size: 16,
                    },
                    e: rdm_container::VectorN {
                        x: rdm_in.vertex.identifiers.clone().into(),
                    },
                },
            ),
        };

        rdm.header1.meta.mesh_info.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(RdmContainer::<true, rdm_container::Dynamic<MeshInfo>> {
                info: RdmContainerPrefix {
                    count: rdm_in.mesh_info.len() as u32,
                    part_size: 28,
                },
                e: rdm_container::VectorN {
                    x: rdm_in.mesh_info.clone(),
                },
            }),
        };

        rdm.header1.meta.triangles.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(RdmContainer::<true, rdm_container::Dynamic<AnnoU16>> {
                info: RdmContainerPrefix {
                    count: rdm_in.triangle_indices.len() as u32 * 3,
                    part_size: 2,
                },
                e: rdm_container::VectorN {
                    x: {
                        let mut o = Vec::new();
                        for x in rdm_in.triangle_indices {
                            o.push(AnnoU16(x.indices[0]));
                            o.push(AnnoU16(x.indices[1]));
                            o.push(AnnoU16(x.indices[2]));
                        }
                        o
                    },
                },
            }),
        };

        rdm.header1.meta.vertex.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(RdmContainer::<false, rdm_container::Dynamic<AnnoU8>> {
                info: RdmContainerPrefix {
                    count: rdm_in.vertex.len(),
                    part_size: rdm_in.vertex.get_size(),
                },
                e: rdm_container::VectorN {
                    x: rdm_in
                        .vertex
                        .as_bytes()
                        .iter()
                        .map(|x| AnnoU8(*x))
                        .collect::<Vec<_>>(),
                },
            }),
        };

        let material = br"Default Standard12432142134";
        let dummy_png_path = br"d:/projekte/anno5/game/testdata/graphics/dummy_objects/dummy_christian/rdm/basalt_crusher_others/diffuse.png";

        let mut mats = vec![];
        for i in 0..MeshInfo::get_max_material(&rdm_in.mesh_info) + 1 {
            let dummy_mat = RdmBlobToMat {
                mat: AnnoPtr(binrw::FilePtr32 {
                    ptr: 0,
                    value: Some(RdmContainer {
                        info: RdmContainerPrefix {
                            count: 1,
                            part_size: 48,
                        },
                        e: rdm_container::Vector1 {
                            x: RdmMat {
                                name: AnnoPtr(binrw::FilePtr32 {
                                    ptr: 0,
                                    value: Some(RdmContainer {
                                        info: RdmContainerPrefix {
                                            count: material.len() as u32,
                                            part_size: 1,
                                        },
                                        e: rdm_container::VectorN {
                                            x: material.map(AnnoChar).into(),
                                        },
                                    }),
                                }),
                                png: AnnoPtr(binrw::FilePtr32 {
                                    ptr: 0,
                                    value: Some(RdmContainer {
                                        info: RdmContainerPrefix {
                                            count: dummy_png_path.len() as u32,
                                            part_size: 1,
                                        },
                                        e: rdm_container::VectorN {
                                            x: dummy_png_path.map(AnnoChar).into(),
                                        },
                                    }),
                                }),
                                _padding: [0; 40],
                            },
                        },
                    }),
                }),
                _padding: Default::default(),
            };
            mats.push(dummy_mat);
        }

        //rdm.header1.skin = 0;

        rdm.header1.rdm_blob_to_mat.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(RdmContainer::<true, rdm_container::Dynamic<RdmBlobToMat>> {
                info: RdmContainerPrefix {
                    count: mats.len() as u32,
                    part_size: 28,
                },
                e: rdm_container::VectorN { x: mats },
            }),
        };

        RdWriter2 { inner: rdm }
    }
}
