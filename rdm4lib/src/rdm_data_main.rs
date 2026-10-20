use std::{
    any::TypeId,
    fs::{self, OpenOptions},
    io::SeekFrom,
    path::PathBuf,
};

use binrw::{binrw, BinRead, BinWriterExt};
use std::marker::PhantomData;

use crate::vertex::StorageType;
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
pub(crate) struct IndexFormat {
    #[br(assert(matches!(element_type, StorageType::U16 | StorageType::U32),
        "Index format requires U16 or U32"))]
    pub(crate) element_type: StorageType,
    _padding: [u8; 16],
}

#[binrw]
#[bw(import_raw(end: &mut u64))]
#[derive(RdmStructSize)]
pub struct Meta {
    #[br(temp, restore_position, seek_before = SeekFrom::Current(-4))]
    #[bw(ignore)]
    part_size: u32,

    #[bw(args_raw = end)]
    model_name: AnnoPtr<RdmTypedT<ModelName>>,
    #[bw(args_raw = end)]
    pub format_identifiers: AnnoPtr<RdmTypedT<VertId>>,
    #[bw(args_raw = end)]
    pub(crate) index_format: AnnoPtr<RdmTypedT<IndexFormat>>,

    #[bw(args_raw = {
        debug!("{:?}", end);
        *end += mesh_info.get_direct_and_pointed_data_size();
        debug!("{:?}", end);
        end
    })]
    pub vertex: AnnoPtr<RdmUntypedContainer>,
    #[bw(args_raw = end)]
    #[br(assert(triangle_list.info.part_size == index_format.element_type.byte_size(),
        "Index type and index-buffer stride disagree"))]
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
        debug!("reset to end {:?}", end);
    })]
    d: (),

    skeleton_index: i32, // -1 means no skeleton.
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
    #[br(parse_with = read_meta_words, args(part_size))]
    _unknown_words: [u32; 10], // 68-byte records store only the first four.
}

#[binrw::parser(reader, endian)]
fn read_meta_words(part_size: u32) -> binrw::BinResult<[u32; 10]> {
    let word_count = match part_size {
        68 => 4,
        92 => 10,
        _ => panic!("Unsupported metadata size: {part_size}"),
    };
    let mut words = [0; 10];
    for word in &mut words[..word_count] {
        *word = u32::read_options(reader, endian, ())?;
    }
    Ok(words)
}

impl Meta {
    /// Stored minimum and maximum XYZ bounds.
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        (self.bounds_min, self.bounds_max)
    }

    pub fn triangle_list_len(&self) -> u32 {
        self.triangle_list.info.count
    }

    /// Decode indices using the source file's byte order.
    pub fn triangle_list(&self, big_endian: bool) -> impl Iterator<Item = u32> + '_ {
        let index_type = self.index_format.element_type;
        let part_size = index_type.byte_size() as usize;
        let convert = match index_type {
            StorageType::U16 => |chunk: &[AnnoU8]| -> u32 {
                let chunk: [u8; 2] = [chunk[0].0, chunk[1].0];
                let value: u32 = u16::from_le_bytes(chunk).into();
                value
            },
            StorageType::U32 => |chunk: &[AnnoU8]| -> u32 {
                let chunk: [u8; 4] = [chunk[0].0, chunk[1].0, chunk[2].0, chunk[3].0];
                let value: u32 = u32::from_le_bytes(chunk);
                value
            },
            _ => panic!("Index format requires U16 or U32"),
        };

        let result = self
            .triangle_list
            .storage
            .items
            .chunks_exact(part_size)
            .map(move |chunk| {
                let value = convert(chunk);
                match (big_endian, index_type) {
                    (true, StorageType::U16) => (value as u16).swap_bytes().into(),
                    (true, StorageType::U32) => value.swap_bytes(),
                    _ => value,
                }
            });
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
#[br(magic = b"RDM")]
#[bw(magic = b"RDM\x01\x14\x00\x00\x00\x00\x00\x00\x00\x04\x00\x00\x00\x1c\x00\x00\x00")]
pub struct RdmFile<T: RdmFileType + 'static> {
    #[br(temp, assert(byte_order <= 1))]
    #[bw(ignore)]
    byte_order: u8,
    #[br(temp, is_big = byte_order == 0, assert(preamble == [0x14, 0, 4, 0x1c]))]
    #[bw(ignore)]
    preamble: [u32; 4],
    #[br(calc = byte_order == 0)]
    #[bw(ignore)]
    pub(crate) big_endian: bool,
    #[br(is_big = big_endian)]
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
    fn short_metadata_reads_only_four_words() {
        for (source, endian) in [
            (
                include_bytes!("../rdm/n_nature_small_stone_02_lod0.rdm").as_slice(),
                binrw::Endian::Big,
            ),
            (
                include_bytes!("../rdm/fishery_others_lod2.rdm").as_slice(),
                binrw::Endian::Little,
            ),
        ] {
            let mut bytes = source.to_vec();
            let word = |offset| match endian {
                binrw::Endian::Big => {
                    u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap())
                }
                _ => u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()),
            };
            let meta = word(word(16) as usize + 4) as usize;
            let encode = |value: u32| match endian {
                binrw::Endian::Big => value.to_be_bytes(),
                _ => value.to_le_bytes(),
            };
            bytes[meta - 4..meta].copy_from_slice(&encode(68));
            for i in 0..4 {
                bytes[meta + 52 + i * 4..meta + 56 + i * 4].copy_from_slice(&encode(i as u32 + 1));
            }
            let mut reader = std::io::Cursor::new(bytes);
            reader.set_position(meta as u64);
            let value: Meta = reader.read_type(endian).unwrap();
            assert_eq!(reader.position(), meta as u64 + 68);
            assert_eq!(value._unknown_words, [1, 2, 3, 4, 0, 0, 0, 0, 0, 0]);
        }
    }

    #[test]
    fn big_endian_short_metadata_converts_to_92_bytes() {
        use crate::vertex::{N3f, N4b, VertexFormat2};
        let model = crate::RdModell::from("rdm/n_nature_small_stone_02_lod0.rdm");
        let source = model.rdmf.as_ref().unwrap();
        assert!(source.big_endian);
        assert_eq!(source.header1.meta.info.part_size, 68);
        assert_eq!(source.header1.meta._unknown_words, [0; 10]);
        assert_eq!(model.vertex.len(), 45);
        assert_eq!(model.triangle_indices.len(), 15);
        let normal = model.vertex.iter::<N4b, N3f>(0).unwrap().next().unwrap();
        for (actual, expected) in normal
            .data
            .into_iter()
            .zip([0.90588236, 0.42745098, 0.03529412])
        {
            assert!((actual - expected).abs() < 0.000001);
        }
        let bounds = model.vertex.calculate_bounds();
        let writer = RdWriter2::new(model);
        let mut output = std::io::Cursor::new(Vec::new());
        output.write_le(&writer.inner).unwrap();
        output.set_position(0);
        let result: RdmFile<RdmKindMesh> = output.read_le().unwrap();
        assert!(!result.big_endian);
        assert_eq!(result.header1.meta.info.part_size, 92);
        assert_eq!(result.header1.meta._unknown_words, [0; 10]);
        assert_eq!(
            VertexFormat2::read_format_via_data(&result).calculate_bounds(),
            bounds
        );
    }

    #[test]
    fn bounds_before_half_float_quantization() {
        let model = crate::RdModell::from("rdm/park_props_system_3x3_04_decal_detail04_lod0.rdm");
        let stored = model.rdmf.as_ref().unwrap().header1.meta.bounds();
        let calculated = model.vertex.calculate_bounds();
        assert_eq!(model.vertex.identifiers[0].storage_type, StorageType::F16);
        assert_ne!(stored, calculated);
        let round = |value| half::f16::from_f32(value).to_f32();
        assert_eq!((stored.0.map(round), stored.1.map(round)), calculated);
        assert_eq!(
            RdWriter2::new(model).inner.header1.meta.bounds(),
            calculated
        );
    }

    #[test]
    fn float32_position_bounds() {
        let model = crate::RdModell::from("rdm/eden_line_flagpole_variation_cloth_lod0.rdm");
        let position = &model.vertex.identifiers[0];
        assert_eq!(position.storage_type, StorageType::F32);
        assert_eq!(position.to_string(), "P3f");
        assert_eq!(model.vertex.len(), 4);
        let calculated = model.vertex.calculate_bounds();
        assert_eq!(
            calculated.0.map(f32::to_bits),
            [0x3c70ef5c, 0x3d073c7a, 0xbe9ba652]
        );
        assert_eq!(
            calculated.1.map(f32::to_bits),
            [0x3cf1c51e, 0x3fc6c946, 0x3e9ba652]
        );
        assert_eq!(
            model.rdmf.as_ref().unwrap().header1.meta.bounds(),
            calculated
        );
        assert_eq!(
            RdWriter2::new(model).inner.header1.meta.bounds(),
            calculated
        );
    }

    #[test]
    fn stored_bounds_match_vertices() {
        for data in [
            include_bytes!("../rdm/fishery_others_lod2.rdm").as_slice(),
            include_bytes!("../rdm/ark_waterfall2.rdm").as_slice(),
        ] {
            let rdm: RdmFile<RdmKindMesh> = std::io::Cursor::new(data).read_le().unwrap();
            let vertices = crate::vertex::VertexFormat2::read_format_via_data(&rdm);
            assert_eq!(rdm.header1.meta.bounds(), vertices.calculate_bounds());
        }
    }

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

        let output_dir = tempfile::tempdir().unwrap();
        let output_path = output_dir.path().join("rdm_out.rdm");
        let mut file = fs::File::create(&output_path).unwrap();
        std::io::Write::write_all(&mut file, &dst).unwrap();

        dbg!(file.metadata().unwrap().len());
        dbg!(data.len());
        assert_eq!(data, fs::read(output_path).unwrap())
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

        info!("Writing RDM {:?}", file);
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
        info!("Wrote RDM {:?}", file);

        file.as_path().into()
    }

    pub fn new(rdm_in: RdModell) -> RdWriter2 {
        use super::*;
        // let data = fs::read("rdm/basalt_crusher_others_lod0.rdm").unwrap();
        let data = include_bytes!("../rdm/basalt_crusher_others_lod0.rdm");

        let mut reader = std::io::Cursor::new(&data);
        let mut rdm: RdmFile<RdmKindMesh> = reader.read_le().unwrap();

        let export_name = br"\\060.alpha\data\Art\graphic_backup\christian\#ANNO5\buildings\others\basalt_crusher_others\Lowpoly\basalt_crusher_others_low_05.max";

        rdm.header1.header2.export_name1.0 = RdmFilePtr {
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

        rdm.header1.meta.skeleton_index = if rdm_in.has_skin() { 0 } else { -1 };

        rdm.header1.meta.format_identifiers.rdm_container.0 = RdmFilePtr {
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

        rdm.header1.meta.mesh_info.0 = RdmFilePtr {
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

        (rdm.header1.meta.bounds_min, rdm.header1.meta.bounds_max) =
            rdm_in.vertex.calculate_bounds();

        rdm.header1.meta.index_format.element_type = StorageType::U16;
        rdm.header1.meta.triangle_list.0 = RdmFilePtr {
            ptr: 0,
            value: Some(RdmContainer {
                info: RdmContainerPrefix {
                    count: rdm_in.triangle_indices.len() as u32 * 3,
                    part_size: StorageType::U16.byte_size(),
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

        rdm.header1.meta.vertex.0 = RdmFilePtr {
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
                mat: AnnoPtr2(RdmFilePtr {
                    ptr: 0,
                    value: Some(RdmContainer {
                        info: RdmContainerPrefix {
                            count: 1,
                            part_size: 48,
                        },
                        storage: rdm_container::Vector1 {
                            item: [RdmMat {
                                name: AnnoPtr2(RdmFilePtr {
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
                                png: AnnoPtr2(RdmFilePtr {
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
                    name: AnnoPtr2(RdmFilePtr {
                        ptr: 0,
                        value: Some(RdmContainer {
                            info: RdmContainerPrefix {
                                count: j.name.len() as u32,
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
            rdm.header1.skin.0 = RdmFilePtr {
                ptr: 0,
                value: None,
            }
        }

        rdm.header1.rdm_blob_to_mat.0 = RdmFilePtr {
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
