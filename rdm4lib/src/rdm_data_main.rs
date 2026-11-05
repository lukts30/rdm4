//! Legacy mesh schema and compatibility export entrypoint.

use std::{
    any::TypeId,
    fs::{self, OpenOptions},
    io::{BufWriter, SeekFrom, Write},
    path::PathBuf,
};

use binrw::{binread, BinRead};
use std::marker::PhantomData;

use crate::vertex::StorageType;

/// Compatibility path for the plain conversion draw range.
pub use crate::MeshInfo;
use crate::{rdm_container::*, rdm_data_anim::AnimMeta, RdModell};
use rdm_derive::RdmStructSize;

pub trait RDMStructSizeTr {
    fn get_struct_byte_size() -> usize;
}

#[binread]
#[derive(RdmStructSize)]
struct ModelName {
    _name: NullableAnnoPtr<RdmString>,
    _padding: [u8; 24],
}

#[binread]
#[derive(RdmStructSize)]
pub struct VertId {
    pub rdm_container: AnnoPtr<RdmTypedContainer<crate::vertex::VertexIdentifier>>,
    _selector: u8,
    _padding: [u8; 19],
}

/// Independent wire record used only by the legacy reader.
#[derive(Debug, Clone)]
#[binread]
#[derive(RdmStructSize)]
pub struct RdmLegacyMeshInfo {
    pub start_index_location: u32,
    pub index_count: u32,
    pub material: u32,
    pub _padding: [u8; 16],
}

#[binread]
#[derive(RdmStructSize)]
pub(crate) struct IndexFormat {
    #[br(assert(matches!(element_type, StorageType::U16 | StorageType::U32),
        "Index format requires U16 or U32"))]
    pub(crate) element_type: StorageType,
    _padding: [u8; 16],
}

#[binread]
#[derive(RdmStructSize)]
pub struct Meta {
    #[br(temp, restore_position, seek_before = SeekFrom::Current(-4))]
    part_size: u32,

    _model_name: AnnoPtr<RdmTypedT<ModelName>>,
    pub format_identifiers: AnnoPtr<RdmTypedT<VertId>>,
    _index_format: AnnoPtr<RdmTypedT<IndexFormat>>,

    pub vertex: AnnoPtr<RdmUntypedContainer>,
    #[br(assert(triangle_list.info.part_size == _index_format.element_type.byte_size(),
        "Index type and index-buffer stride disagree"))]
    pub triangle_list: AnnoPtr<RdmUntypedContainer>,

    pub mesh_info: AnnoPtr<RdmTypedContainer<RdmLegacyMeshInfo>>,

    _skeleton_index: i32, // -1 means no skeleton.
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

#[binread]
#[derive(RdmStructSize)]
struct RdmBlobToMat {
    _mat: AnnoPtr<RdmTypedT<RdmMat>>,
    _padding: [u8; 24],
}

#[binread]
#[derive(RdmStructSize)]
struct RdmMat {
    _name: NullableAnnoPtr<RdmString>,
    _png: NullableAnnoPtr<RdmString>,
    _padding: [u8; 40],
}

#[binread]
#[derive(RdmStructSize)]
pub struct RdmBlobToJoint {
    pub joint: AnnoPtr<RdmTypedContainer<RdmJoint>>,
    _padding: [u8; 32 - 4],
}

#[derive(Debug)]
#[binread]
#[derive(RdmStructSize)]
pub struct RdmJoint {
    pub name: AnnoPtr<RdmString>,
    pub t: [f32; 3],
    pub r: [f32; 4],
    pub parent_id: u32,
    _padding: [u8; 84 - 20 - 16],
}

#[binread]
#[derive(RdmStructSize)]
pub struct RdmHeader1 {
    pub header2: NullableAnnoPtr<RdmTypedT<ExportInfo>>,

    pub meta: NullableAnnoPtr<RdmTypedT<Meta>>,
    _materials: NullableAnnoPtr<RdmTypedContainer<RdmBlobToMat>>,
    pub skin: NullableAnnoPtr<RdmTypedT<RdmBlobToJoint>>,

    pub meta_anim: NullableAnnoPtr<RdmTypedT<AnimMeta>>,
    _data: [u8; 48 - 5 * 4],
}

#[binread]
#[derive(RdmStructSize)]
pub struct ExportInfo {
    pub export_name1: NullableAnnoPtr<RdmString>,
    pub export_name2: NullableAnnoPtr<RdmString>,
    _data: [u8; 72 - 8],
}

pub struct RdmKindMesh;
pub struct RdmKindAnim;
pub trait RdmFileType {}
impl RdmFileType for RdmKindMesh {}
impl RdmFileType for RdmKindAnim {}

#[binread]
#[br(magic = b"RDM")]
pub struct RdmFile<T: RdmFileType + 'static> {
    #[br(temp, assert(byte_order <= 1))]
    byte_order: u8,
    #[br(temp, is_big = byte_order == 0, assert(preamble == [0x14, 0, 4, 0x1c]))]
    preamble: [u32; 4],
    #[br(calc = byte_order == 0)]
    pub(crate) _big_endian: bool,
    #[br(is_big = _big_endian)]
    #[br(seek_before = SeekFrom::Start(0x00000014))]
    #[br(assert(header1.header2.ptr == 0x1C + header1.info.part_size))]
    // RdmHeader1 size is usually 48 but sometimes 52
    // TODO: fix some rdm anim's have a NULL meta_anim.ptr
    #[br(assert(TypeId::of::<RdmKindMesh>() == TypeId::of::<T>() || header1.storage.item[0].meta_anim.ptr != 0 && header1.storage.item[0].meta.ptr == 0, "the input file is not a valid rdm anim!"))]
    #[br(assert(TypeId::of::<RdmKindAnim>() == TypeId::of::<T>() || header1.storage.item[0].meta.ptr != 0 && header1.storage.item[0].meta_anim.ptr == 0, "the input file is not a rdm mesh!"))]
    pub header1: RdmTypedT<RdmHeader1>,

    kind: PhantomData<T>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use binrw::BinReaderExt;
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
        assert!(source._big_endian);
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
        assert!(!result._big_endian);
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
        assert_eq!(RdmLegacyMeshInfo::get_struct_byte_size(), 28);

        assert_eq!(RdmJoint::get_struct_byte_size(), 84);

        assert_eq!(ExportInfo::get_struct_byte_size(), 72);

        assert_eq!(AnnoU8::get_struct_byte_size(), 1);
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
