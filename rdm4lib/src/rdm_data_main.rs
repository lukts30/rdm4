use std::{
    any::TypeId,
    fs::{self, OpenOptions},
    io::SeekFrom,
    path::PathBuf,
};

use binrw::{binrw, BinWriterExt};
use std::marker::PhantomData;

use crate::{rdm_container::*, rdm_data_anim::AnimMeta, RdModell};
use rdm_derive::RdmStructSize;

pub trait RDMStructSizeTr {
    fn get_struct_byte_size() -> usize;
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[derive(RdmStructSize)]
struct ModelName {
    #[bw(args_raw = end)]
    name: NullableAnnoPtr<RdmString>,
    _padding: [u8; 24],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[derive(RdmStructSize)]
pub struct VertId {
    #[bw(args_raw = end)]
    pub rdm_container: AnnoPtr<RdmTypedContainer<crate::vertex::VertexIdentifier>>,
    unknown_shader_id: u8,
    _padding: [u8; 19],
}

#[derive(Debug, Clone)]
#[binrw]
#[bw(import_raw(_dst: &mut u64))]
#[derive(RdmStructSize)]
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
#[derive(RdmStructSize)]
struct MetaUnknown {
    _unknown: u32,
    _padding: [u8; 16],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[derive(RdmStructSize)]
pub struct Meta {
    #[bw(args_raw = end)]
    model_name: AnnoPtr<RdmTypedT<ModelName>>,
    #[bw(args_raw = end)]
    pub format_identifiers: AnnoPtr<RdmTypedT<VertId>>,
    #[bw(args_raw = end)]
    unknown: AnnoPtr<RdmTypedT<MetaUnknown>>,

    #[bw(args_raw = {
        debug!("{:?}", &end);
        *end += mesh_info.get_direct_and_pointed_data_size();
        debug!("{:?}", &end);
        end
    })]
    pub vertex: AnnoPtr<RdmUntypedContainer>,
    #[bw(args_raw = end)]
    pub triangle_list: AnnoPtr<RdmUntypedContainer>,

    #[bw(args_raw = {
        let negative_off = mesh_info.get_direct_and_pointed_data_size() + vertex.get_direct_and_pointed_data_size() + triangle_list.get_direct_and_pointed_data_size();
        *end -= negative_off;
        end
    })]
    pub mesh_info: AnnoPtr<RdmTypedContainer<MeshInfo>>,

    #[br(ignore)]
    #[bw(calc = {
        let off = vertex.get_direct_and_pointed_data_size() + triangle_list.get_direct_and_pointed_data_size();
        *end += off;
        debug!("reset to end {:?}",&end);
    })]
    d: (),

    _padding_ff: u32, // 0x_FF_FF_FF_FF or 0x0
    _unknown_box: [u8; 24],
    _padding_zero: [u8; 40],
}

impl Meta {
    pub fn triangle_list_len(&self) -> u32 {
        self.triangle_list.info.count
    }

    pub fn triangle_list(&self) -> impl Iterator<Item = u32> + '_ {
        let part_size = self.triangle_list.info.part_size as usize;
        let convert = match part_size {
            2 => |chunk: &[AnnoU8]| -> u32 {
                let chunk: [u8; 2] = [chunk[0].0, chunk[1].0];
                let value: u32 = u16::from_le_bytes(chunk).into();
                value
            },
            4 => |chunk: &[AnnoU8]| -> u32 {
                let chunk: [u8; 4] = [chunk[0].0, chunk[1].0, chunk[2].0, chunk[3].0];
                let value: u32 = u32::from_le_bytes(chunk);
                value
            },
            _ => {
                panic!("Unexpected indices part_size: {}", part_size)
            }
        };

        let result = self
            .triangle_list
            .storage
            .items
            .chunks_exact(part_size)
            .map(convert);
        result
    }
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[derive(RdmStructSize)]
struct RdmBlobToMat {
    #[bw(args_raw = end)]
    mat: AnnoPtr<RdmTypedT<RdmMat>>,
    _padding: [u8; 24],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[derive(RdmStructSize)]
struct RdmMat {
    #[bw(args_raw = end)]
    name: NullableAnnoPtr<RdmString>,
    #[bw(args_raw = end)]
    png: NullableAnnoPtr<RdmString>,
    _padding: [u8; 40],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[derive(RdmStructSize)]
pub struct RdmBlobToJoint {
    #[bw(args_raw = end)]
    pub joint: AnnoPtr<RdmTypedContainer<RdmJoint>>,
    _padding: [u8; 32 - 4],
}

#[derive(Debug)]
#[binrw]
#[bw(import_raw(end: &mut u64))]
#[derive(RdmStructSize)]
pub struct RdmJoint {
    #[bw(args_raw = end)]
    pub name: AnnoPtr<RdmString>,
    pub t: [f32; 3],
    pub r: [f32; 4],
    pub parent_id: u32,
    _padding: [u8; 84 - 20 - 16],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[derive(RdmStructSize)]
pub struct RdmHeader1 {
    #[bw(args_raw = end)]
    pub header2: NullableAnnoPtr<RdmTypedT<ExportInfo>>,

    #[bw(args_raw = end)]
    pub meta: NullableAnnoPtr<RdmTypedT<Meta>>,
    #[bw(args_raw = end)]
    rdm_blob_to_mat: NullableAnnoPtr<RdmTypedContainer<RdmBlobToMat>>,
    #[bw(args_raw = end)]
    pub skin: NullableAnnoPtr<RdmTypedT<RdmBlobToJoint>>,

    #[bw(args_raw = end)]
    pub meta_anim: NullableAnnoPtr<RdmTypedT<AnimMeta>>,
    _data: [u8; 48 - 5 * 4],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[derive(RdmStructSize)]
pub struct ExportInfo {
    #[bw(args_raw = end)]
    pub export_name1: NullableAnnoPtr<RdmString>,
    #[bw(args_raw = end)]
    pub export_name2: NullableAnnoPtr<RdmString>,
    _data: [u8; 72 - 8],
}

pub struct RdmKindMesh;
pub struct RdmKindAnim;
pub trait RdmFileType {}
impl RdmFileType for RdmKindMesh {}
impl RdmFileType for RdmKindAnim {}

#[binrw]
#[brw(magic = b"RDM\x01\x14\x00\x00\x00\x00\x00\x00\x00\x04\x00\x00\x00\x1c\x00\x00\x00")]
pub struct RdmFile<T: RdmFileType + 'static> {
    #[brw(seek_before = SeekFrom::Start(0x00000014))]
    #[br(assert(header1.header2.ptr == 0x1C + header1.info.part_size))]
    // RdmHeader1 size is usually 48 but sometimes 52
    // TODO: fix some rdm anim's have a NULL meta_anim.ptr
    #[br(assert(TypeId::of::<RdmKindMesh>() == TypeId::of::<T>() || header1.storage.item[0].meta_anim.ptr != 0 && header1.storage.item[0].meta.ptr == 0, "the input file is not a valid rdm anim!"))]
    #[br(assert(TypeId::of::<RdmKindAnim>() == TypeId::of::<T>() || header1.storage.item[0].meta.ptr != 0 && header1.storage.item[0].meta_anim.ptr == 0, "the input file is not a rdm mesh!"))]
    pub header1: RdmTypedT<RdmHeader1>,

    #[bw(ignore)]
    kind: PhantomData<T>,
}

pub trait DataAndPointedToSize {
    fn get_direct_and_pointed_data_size(&self) -> u64;
}

impl<C, T> DataAndPointedToSize for RdmContainer<true, C, T>
where
    C: VectorSize2,
    C::Storage<T>: RdmContainerRead,
    for<'a> &'a C::Storage<T>: IntoIterator<Item = &'a T>,
    T: RdmRead + RdmContainerWrite + DataAndPointedToSize,
{
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        let mut sum = 0;
        for x in self.storage.into_iter() {
            sum += x.get_direct_and_pointed_data_size();
        }
        8 + sum
    }
}

impl<C, T> DataAndPointedToSize for RdmContainer<false, C, T>
where
    C: VectorSize2,
    C::Storage<T>: RdmContainerRead,
    for<'a> &'a C::Storage<T>: IntoIterator<Item = &'a T>,
    T: RdmRead + RdmContainerWrite,
{
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        (self.info.count * self.info.part_size) as u64 + 8
    }
}

impl DataAndPointedToSize for ExportInfo {
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        ExportInfo::get_struct_byte_size() as u64
            + self.export_name1.get_direct_and_pointed_data_size()
            + self.export_name2.get_direct_and_pointed_data_size()
    }
}

impl DataAndPointedToSize for MeshInfo {
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        MeshInfo::get_struct_byte_size() as u64
    }
}

impl DataAndPointedToSize for AnnoChar {
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        AnnoChar::get_struct_byte_size() as u64
    }
}

impl DataAndPointedToSize for AnnoU8 {
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        AnnoU8::get_struct_byte_size() as u64
    }
}

impl DataAndPointedToSize for AnnoU16 {
    fn get_direct_and_pointed_data_size(&self) -> u64 {
        AnnoU16::get_struct_byte_size() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use binrw::{BinReaderExt, BinWriterExt};
    use std::fs;

    #[test]
    fn struct_sizes() {
        //assert_eq!(RdmHeader1::get_struct_byte_size(), 48);
        assert_eq!(RdmBlobToMat::get_struct_byte_size(), 28);
        assert_eq!(RdmBlobToJoint::get_struct_byte_size(), 32);

        assert_eq!(Meta::get_struct_byte_size(), 92);
        assert_eq!(ModelName::get_struct_byte_size(), 28);
        assert_eq!(VertId::get_struct_byte_size(), 24);
        assert_eq!(MeshInfo::get_struct_byte_size(), 28);

        assert_eq!(RdmJoint::get_struct_byte_size(), 84);

        assert_eq!(ExportInfo::get_struct_byte_size(), 72);

        assert_eq!(AnnoU16::get_struct_byte_size(), 2);
        assert_eq!(AnnoU8::get_struct_byte_size(), 1);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn rdm_file_serialisation_roundtrip() {
        let data = fs::read("rdm/fishery_others_cutout_lod0.rdm").unwrap();
        //let data = fs::read("rdm/basalt_crusher_others_lod0.rdm").unwrap();

        let mut reader = std::io::Cursor::new(&data);

        let rdm: RdmFile<RdmKindMesh> = reader.read_le().unwrap();

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
    inner: RdmFile<RdmKindMesh>,
}

impl RdWriter2 {
    pub fn write_rdm(self, dir: Option<PathBuf>, create_new: bool) -> PathBuf {
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

        file.as_path().into()
    }

    pub fn new(rdm_in: RdModell) -> RdWriter2 {
        use super::*;
        // let data = fs::read("rdm/basalt_crusher_others_lod0.rdm").unwrap();
        let data = include_bytes!("../rdm/basalt_crusher_others_lod0.rdm");

        let mut reader = std::io::Cursor::new(&data);
        let mut rdm: RdmFile<RdmKindMesh> = reader.read_le().unwrap();

        let export_name = br"\\060.alpha\data\Art\graphic_backup\christian\#ANNO5\buildings\others\basalt_crusher_others\Lowpoly\basalt_crusher_others_low_05.max";

        rdm.header1.header2.export_name1.0 = binrw::FilePtr32 {
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
            u32::MAX
        };

        rdm.header1.meta.format_identifiers.rdm_container.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(RdmContainer {
                info: RdmContainerPrefix {
                    count: rdm_in.vertex.identifiers.len() as u32,
                    part_size: 16,
                },
                storage: rdm_container::VectorN {
                    items: rdm_in.vertex.identifiers.clone().into(),
                },
            }),
        };

        rdm.header1.meta.mesh_info.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(RdmContainer {
                info: RdmContainerPrefix {
                    count: rdm_in.mesh_info.len() as u32,
                    part_size: 28,
                },
                storage: rdm_container::VectorN {
                    items: rdm_in.mesh_info.clone(),
                },
            }),
        };

        rdm.header1.meta._unknown_box = [
            0x00, 0x80, 0xF8, 0xBF, 0x00, 0x40, 0x22, 0xC0, 0x00, 0x60, 0xF7, 0xBF, 0x00, 0x80,
            0xFB, 0x3F, 0x00, 0xC0, 0xF2, 0x3F, 0x00, 0x80, 0xFC, 0x3F,
        ];

        rdm.header1.meta.triangle_list.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(RdmContainer {
                info: RdmContainerPrefix {
                    count: rdm_in.triangle_indices.len() as u32 * 3,
                    part_size: 2,
                },
                storage: rdm_container::VectorN {
                    items: {
                        let mut p = 0;
                        let mut o = Vec::new();
                        for x in &rdm_in.triangle_indices {
                            o.push(AnnoU16(u16::try_from(x.indices[0]).unwrap()));
                            o.push(AnnoU16(u16::try_from(x.indices[1]).unwrap()));
                            o.push(AnnoU16(u16::try_from(x.indices[2]).unwrap()));
                            p = p.max(x.indices[0]);
                            p = p.max(x.indices[1]);
                            p = p.max(x.indices[2]);
                        }
                        info!("Max Triangle List Index: {}", p);

                        o.iter()
                            .map(|x| x.0)
                            .flat_map(u16::to_le_bytes)
                            .map(AnnoU8)
                            .collect()
                    },
                },
            }),
        };

        rdm.header1.meta.vertex.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(RdmContainer {
                info: RdmContainerPrefix {
                    count: rdm_in.vertex.len(),
                    part_size: rdm_in.vertex.get_size(),
                },
                storage: rdm_container::VectorN {
                    items: rdm_in
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
        for _ in 0..MeshInfo::get_max_material(&rdm_in.mesh_info) + 1 {
            let dummy_mat = RdmBlobToMat {
                mat: AnnoPtr2(binrw::FilePtr32 {
                    ptr: 0,
                    value: Some(RdmContainer {
                        info: RdmContainerPrefix {
                            count: 1,
                            part_size: 48,
                        },
                        storage: rdm_container::Vector1 {
                            item: [RdmMat {
                                name: AnnoPtr2(binrw::FilePtr32 {
                                    ptr: 0,
                                    value: Some(RdmContainer {
                                        info: RdmContainerPrefix {
                                            count: material.len() as u32,
                                            part_size: 1,
                                        },
                                        storage: rdm_container::VectorN {
                                            items: material.map(AnnoChar).into(),
                                        },
                                    }),
                                }),
                                png: AnnoPtr2(binrw::FilePtr32 {
                                    ptr: 0,
                                    value: Some(RdmContainer {
                                        info: RdmContainerPrefix {
                                            count: dummy_png_path.len() as u32,
                                            part_size: 1,
                                        },
                                        storage: rdm_container::VectorN {
                                            items: dummy_png_path.map(AnnoChar).into(),
                                        },
                                    }),
                                }),
                                _padding: [0; 40],
                            }],
                        },
                    }),
                }),
                _padding: Default::default(),
            };
            mats.push(dummy_mat);
        }

        if rdm_in.has_skin() {
            let mut replacement_raw_joints = vec![];
            for j in &rdm_in.joints.unwrap() {
                let joint_quaternion = j.quaternion;

                let rx = joint_quaternion[0];
                let ry = joint_quaternion[1];
                let rz = joint_quaternion[2];
                let rw = joint_quaternion[3];

                let q = Quaternion::new(rw, rx, ry, rz);
                let unit_quaternion = UnitQuaternion::from_quaternion(q);

                let trans = j.transition;
                let tx = trans[0];
                let ty = trans[1];
                let tz = trans[2];
                let v: Vector3<f32> = Vector3::new(tx, ty, tz);

                // undo rotation since it will be applied on load
                // rdm -> internal representation -> rdm: v vector in add_skin should be equal to v_init
                let v_init = unit_quaternion.inverse_transform_vector(&v).scale(-1.0);
                let rot = unit_quaternion.quaternion().coords;

                let res = RdmJoint {
                    name: AnnoPtr2(binrw::FilePtr32 {
                        ptr: 0,
                        value: Some(RdmContainer {
                            info: RdmContainerPrefix {
                                count: j.name.as_bytes().len() as u32,
                                part_size: 1,
                            },
                            storage: rdm_container::VectorN {
                                items: j.name.as_bytes().iter().map(|c| AnnoChar(*c)).collect(),
                            },
                        }),
                    }),
                    t: [v_init.x, v_init.y, v_init.z],
                    r: [rot.x, rot.y, rot.z, rot.w],
                    parent_id: j.parent,
                    _padding: [0; 48],
                };

                replacement_raw_joints.push(res);
            }
            assert_eq!(rdm.header1.skin.joint.info.part_size, 84);
            rdm.header1.skin.joint.info.count = replacement_raw_joints.len() as u32;
            rdm.header1.skin.joint.storage.items = replacement_raw_joints;
        } else {
            rdm.header1.skin.0 = binrw::FilePtr32 {
                ptr: 0,
                value: None,
            }
        }

        rdm.header1.rdm_blob_to_mat.0 = binrw::FilePtr32 {
            ptr: 0,
            value: Some(RdmContainer {
                info: RdmContainerPrefix {
                    count: mats.len() as u32,
                    part_size: 28,
                },
                storage: rdm_container::VectorN { items: mats },
            }),
        };

        RdWriter2 { inner: rdm }
    }
}
