use std::{
    any::TypeId,
    fs::{self, OpenOptions},
    io::{BufWriter, SeekFrom, Write},
    path::PathBuf,
};

use binrw::{binrw, BinRead};
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

    fn read_source(path: &str) -> RdmFile<RdmKindMesh> {
        fs::File::open(path).unwrap().read_le().unwrap()
    }

    #[test]
    fn reads_68_byte_metadata_without_consuming_the_next_block() {
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
    fn converts_be_68_byte_metadata_to_le_92_byte_metadata() {
        use crate::vertex::{N3f, N4b};
        let model = crate::RdModell::from("rdm/n_nature_small_stone_02_lod0.rdm");
        let source = read_source("rdm/n_nature_small_stone_02_lod0.rdm");
        assert!(source.big_endian);
        assert_eq!(source.header1.meta.info.part_size, 68);
        // The packed normal must be decoded as a BE word, not four unchanged bytes.
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
        let output = std::io::Cursor::new(Vec::new());
        let mut output = crate::rdm::RdmWriter::new(output, crate::rdm::RdmEndian::Little)
            .write_root(&writer.inner)
            .unwrap();
        output.set_position(0);
        let result: RdmFile<RdmKindMesh> = output.read_le().unwrap();
        assert!(!result.big_endian);
        assert_eq!(result.header1.meta.info.part_size, 92);
        assert_eq!(result.header1.meta._unknown_words, [0; 10]);
        assert_eq!(
            RdModell::new(output.into_inner()).vertex.calculate_bounds(),
            bounds
        );
    }

    #[test]
    fn writes_bounds_from_quantized_vertices_instead_of_source_bounds() {
        let model = crate::RdModell::from("rdm/park_props_system_3x3_04_decal_detail04_lod0.rdm");
        let stored = read_source("rdm/park_props_system_3x3_04_decal_detail04_lod0.rdm")
            .header1
            .meta
            .bounds();
        let calculated = model.vertex.calculate_bounds();
        assert_eq!(model.vertex.identifiers[0].storage_type, StorageType::F16);
        assert_ne!(stored, calculated);
        let round = |value| half::f16::from_f32(value).to_f32();
        assert_eq!((stored.0.map(round), stored.1.map(round)), calculated);
        assert_eq!(
            {
                let document = RdWriter2::new(model).inner;
                let model = &document.models.as_ref().unwrap()[0];
                (model.bounds_min, model.bounds_max)
            },
            calculated
        );
    }

    #[test]
    fn preserves_f32_position_bounds_when_writing_rdm() {
        let model = crate::RdModell::from("rdm/eden_line_flagpole_variation_cloth_lod0.rdm");
        let position = &model.vertex.identifiers[0];
        assert_eq!(position.to_string(), "P3f");
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
            read_source("rdm/eden_line_flagpole_variation_cloth_lod0.rdm")
                .header1
                .meta
                .bounds(),
            calculated
        );
        assert_eq!(
            {
                let document = RdWriter2::new(model).inner;
                let model = &document.models.as_ref().unwrap()[0];
                (model.bounds_min, model.bounds_max)
            },
            calculated
        );
    }

    #[test]
    fn calculates_bounds_matching_existing_mesh_metadata() {
        for data in [
            include_bytes!("../rdm/fishery_others_lod2.rdm").as_slice(),
            include_bytes!("../rdm/ark_waterfall2.rdm").as_slice(),
        ] {
            let rdm: RdmFile<RdmKindMesh> = std::io::Cursor::new(data).read_le().unwrap();
            let model = RdModell::new(data.to_vec());
            assert_eq!(rdm.header1.meta.bounds(), model.vertex.calculate_bounds());
        }
    }

    #[test]
    fn matches_serialized_record_sizes() {
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
    fn preserves_mesh_bytes_when_read_and_written() {
        let data = fs::read("rdm/fishery_others_cutout_lod0.rdm").unwrap();

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

        assert_eq!(data, fs::read(output_path).unwrap())
    }
}

/// Compatibility entrypoint backed by the current document writer.
pub struct RdWriter2 {
    inner: crate::rdm::RdmDocument,
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
        let writer = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .create_new(create_new)
            .open(&file)
            .expect("I/O error");

        crate::rdm::RdmWriter::new(BufWriter::new(writer), crate::rdm::RdmEndian::Little)
            .write_root(&self.inner)
            .expect("cannot write RDM records")
            .flush()
            .expect("cannot flush RDM output");
        info!("Wrote RDM {:?}", file);

        file.as_path().into()
    }

    /// Adapt the model for the current record writer.
    pub fn new(rdm_in: RdModell) -> RdWriter2 {
        Self {
            inner: crate::rdm::RdmDocument::try_from(rdm_in).expect("invalid export model"),
        }
    }
}
