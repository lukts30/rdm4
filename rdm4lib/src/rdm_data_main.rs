use std::{
    any::TypeId,
    fs::{self, OpenOptions},
    io::SeekFrom,
    path::PathBuf,
};

use binrw::BinRead;
use rdm_derive::rdm_read;
use std::marker::PhantomData;

use crate::vertex::StorageType;
use crate::{rdm_container::*, rdm_data_anim::AnimMeta, RdModell};

#[rdm_read]
struct ModelName {
    name: Option<Vec<u8>>,
    _padding: [u8; 24],
}

#[rdm_read]
pub struct VertId {
    pub rdm_container: Vec<crate::vertex::VertexIdentifier>,
    unknown_shader_id: u8,
    _padding: [u8; 19],
}

#[derive(Debug, Clone)]
#[rdm_read]
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

#[rdm_read]
pub(crate) struct IndexFormat {
    #[br(assert(matches!(element_type, StorageType::U16 | StorageType::U32),
        "Index format requires U16 or U32"))]
    pub(crate) element_type: StorageType,
    _padding: [u8; 16],
}

#[rdm_read]
pub struct Meta {
    #[br(restore_position, seek_before = SeekFrom::Current(-4), assert(matches!(part_size, 68 | 92)))]
    part_size: u32,

    #[rdm(offset)]
    model_name: ModelName,

    #[rdm(offset)]
    pub format_identifiers: VertId,

    #[rdm(offset)]
    pub(crate) index_format: IndexFormat,

    #[br(parse_with = read_raw)]
    pub vertex: RawBuffer,

    #[br(assert(triangle_list.stride == index_format.element_type.byte_size(),
        "Index type and index-buffer stride disagree"))]
    #[br(parse_with = read_raw)]
    pub triangle_list: RawBuffer,

    pub mesh_info: Vec<MeshInfo>,

    skeleton_index: i32, // -1 means no skeleton.
    bounds_min: [f32; 3],
    bounds_max: [f32; 3],
    #[br(parse_with = read_meta_words, args(part_size))]
    _unknown_words: [u32; 10], // 68-byte records store only the first four.
}

fn meta_word_count(stride: u32) -> usize {
    match stride {
        68 => 4,
        92 => 10,
        _ => panic!("Unsupported metadata size: {stride}"),
    }
}

#[binrw::parser(reader, endian)]
fn read_meta_words(part_size: u32) -> binrw::BinResult<[u32; 10]> {
    let word_count = meta_word_count(part_size);
    let mut words = [0; 10];
    for word in &mut words[..word_count] {
        *word = u32::read_options(reader, endian, ())?;
    }
    Ok(words)
}

impl Meta {
    /// Metadata record width retained from the source file.
    pub fn source_stride(&self) -> u32 {
        self.part_size
    }
    /// Stored minimum and maximum XYZ bounds.
    pub fn bounds(&self) -> ([f32; 3], [f32; 3]) {
        (self.bounds_min, self.bounds_max)
    }

    pub fn triangle_list_len(&self) -> u32 {
        self.triangle_list.count
    }

    /// Decode indices using the source file's byte order.
    pub fn triangle_list(&self, big_endian: bool) -> impl Iterator<Item = u32> + '_ {
        let index_type = self.index_format.element_type;
        let part_size = index_type.byte_size() as usize;
        let convert = match index_type {
            StorageType::U16 => |chunk: &[u8]| -> u32 {
                let chunk: [u8; 2] = [chunk[0], chunk[1]];
                let value: u32 = u16::from_le_bytes(chunk).into();
                value
            },
            StorageType::U32 => |chunk: &[u8]| -> u32 {
                let chunk: [u8; 4] = [chunk[0], chunk[1], chunk[2], chunk[3]];
                let value: u32 = u32::from_le_bytes(chunk);
                value
            },
            _ => panic!("Index format requires U16 or U32"),
        };

        let result = self
            .triangle_list
            .bytes
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

#[rdm_read]
struct RdmBlobToMat {
    #[rdm(offset)]
    mat: RdmMat,
    _padding: [u8; 24],
}

#[rdm_read]
struct RdmMat {
    name: Option<Vec<u8>>,

    png: Option<Vec<u8>>,
    _padding: [u8; 40],
}

#[rdm_read]
pub struct RdmBlobToJoint {
    pub joint: Vec<RdmJoint>,
    _padding: [u8; 32 - 4],
}

#[derive(Debug)]
#[rdm_read]
pub struct RdmJoint {
    pub name: Vec<u8>,
    pub t: [f32; 3],
    pub r: [f32; 4],
    pub parent_id: u32,
    _padding: [u8; 84 - 20 - 16],
}

#[rdm_read]
pub struct RdmHeader1 {
    #[br(temp, restore_position, seek_before = SeekFrom::Current(-4), assert(matches!(part_size, 48 | 52)))]
    part_size: u32,
    #[br(temp, restore_position, assert(export_offset == 36 + part_size))]
    export_offset: u32,
    pub header2: Option<ExportInfo>,

    #[br(parse_with = read_first)]
    pub meta: Option<Meta>,

    rdm_blob_to_mat: Option<Vec<RdmBlobToMat>>,

    pub skin: Option<RdmBlobToJoint>,

    pub meta_anim: Option<AnimMeta>,
    #[rdm(inline)]
    #[br(count = part_size - 20)]
    _data: Vec<u8>,
}

#[rdm_read]
pub struct ExportInfo {
    pub export_name1: Option<Vec<u8>>,

    pub export_name2: Option<Vec<u8>>,
    _data: [u8; 72 - 8],
}

pub struct RdmKindMesh;
pub struct RdmKindAnim;
pub trait RdmFileType {}
impl RdmFileType for RdmKindMesh {}
impl RdmFileType for RdmKindAnim {}

#[rdm_read]
#[br(magic = b"RDM")]

pub struct RdmFile<T: RdmFileType + 'static> {
    #[br(temp, assert(byte_order <= 1))]
    byte_order: u8,
    #[br(temp, is_big = byte_order == 0, assert(preamble == [0x14, 0, 4, 0x1c]))]
    preamble: [u32; 4],
    #[br(calc = byte_order == 0)]
    pub(crate) big_endian: bool,
    #[br(is_big = big_endian)]
    #[br(seek_before = SeekFrom::Start(0x00000014))]
    // RdmHeader1 size is usually 48 but sometimes 52
    #[br(assert(TypeId::of::<RdmKindMesh>() == TypeId::of::<T>() || header1.meta_anim.is_some() && header1.meta.is_none(), "the input file is not a valid rdm anim!"))]
    #[br(assert(TypeId::of::<RdmKindAnim>() == TypeId::of::<T>() || header1.meta.is_some() && header1.meta_anim.is_none(), "the input file is not a rdm mesh!"))]
    #[br(parse_with = read_root)]
    pub header1: RdmHeader1,

    kind: PhantomData<T>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use binrw::BinReaderExt;
    use std::fs;

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
    fn preserves_metadata_layout_and_words_in_both_byte_orders() {
        for source in [
            include_bytes!("../rdm/n_nature_small_stone_02_lod0.rdm").as_slice(),
            include_bytes!("../rdm/fishery_others_lod2.rdm").as_slice(),
        ] {
            for stride in [68, 92] {
                let mut rdm: RdmFile<RdmKindMesh> = std::io::Cursor::new(source).read_le().unwrap();
                let meta = rdm.header1.meta.as_mut().unwrap();
                meta.part_size = stride;
                meta._unknown_words = [0; 10];
                for (i, word) in meta._unknown_words[..meta_word_count(stride)]
                    .iter_mut()
                    .enumerate()
                {
                    *word = 0x12340000 + i as u32;
                }
                // Vec implements Write but not Seek.
                let mut bytes = Vec::new();
                rdm.write_rdm(&mut bytes).unwrap();
                let word = |at: usize| {
                    let bytes = bytes[at..at + 4].try_into().unwrap();
                    if rdm.big_endian {
                        u32::from_be_bytes(bytes)
                    } else {
                        u32::from_le_bytes(bytes)
                    }
                };
                let root = word(16) as usize;
                let model = word(root + 4) as usize;
                assert_eq!(word(model - 4), stride);
                // The model-name block starts immediately after this record.
                assert_eq!(word(model), model as u32 + stride + 8);
                let written: RdmFile<RdmKindMesh> = std::io::Cursor::new(bytes).read_le().unwrap();
                let original = rdm.header1.meta.as_ref().unwrap();
                let result = written.header1.meta.as_ref().unwrap();
                assert_eq!(result.source_stride(), stride);
                assert_eq!(result._unknown_words, original._unknown_words);
                assert_eq!(result.vertex.bytes, original.vertex.bytes);
                assert_eq!(result.triangle_list.bytes, original.triangle_list.bytes);
                assert_eq!(written.big_endian, rdm.big_endian);
            }
        }
    }

    #[test]
    fn converts_be_68_byte_metadata_to_le_92_byte_metadata() {
        use crate::vertex::{N3f, N4b, VertexFormat2};
        let model = crate::RdModell::from("rdm/n_nature_small_stone_02_lod0.rdm");
        let source = model.rdmf.as_ref().unwrap();
        assert!(source.big_endian);
        assert_eq!(source.header1.meta.as_ref().unwrap().source_stride(), 68);
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
        let mut output = std::io::Cursor::new(Vec::new());
        writer.inner.write_rdm(&mut output).unwrap();
        output.set_position(0);
        let result: RdmFile<RdmKindMesh> = output.read_le().unwrap();
        assert!(!result.big_endian);
        assert_eq!(result.header1.meta.as_ref().unwrap().source_stride(), 92);
        assert_eq!(
            result.header1.meta.as_ref().unwrap()._unknown_words,
            [0; 10]
        );
        assert_eq!(
            VertexFormat2::read_format_via_data(&result).calculate_bounds(),
            bounds
        );
    }

    #[test]
    fn writes_bounds_from_quantized_vertices_instead_of_source_bounds() {
        let model = crate::RdModell::from("rdm/park_props_system_3x3_04_decal_detail04_lod0.rdm");
        let stored = model
            .rdmf
            .as_ref()
            .unwrap()
            .header1
            .meta
            .as_ref()
            .unwrap()
            .bounds();
        let calculated = model.vertex.calculate_bounds();
        assert_eq!(model.vertex.identifiers[0].storage_type, StorageType::F16);
        assert_ne!(stored, calculated);
        let round = |value| half::f16::from_f32(value).to_f32();
        assert_eq!((stored.0.map(round), stored.1.map(round)), calculated);
        assert_eq!(
            RdWriter2::new(model)
                .inner
                .header1
                .meta
                .as_ref()
                .unwrap()
                .bounds(),
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
            model
                .rdmf
                .as_ref()
                .unwrap()
                .header1
                .meta
                .as_ref()
                .unwrap()
                .bounds(),
            calculated
        );
        assert_eq!(
            RdWriter2::new(model)
                .inner
                .header1
                .meta
                .as_ref()
                .unwrap()
                .bounds(),
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
            let vertices = crate::vertex::VertexFormat2::read_format_via_data(&rdm);
            assert_eq!(
                rdm.header1.meta.as_ref().unwrap().bounds(),
                vertices.calculate_bounds()
            );
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn preserves_mesh_bytes_when_read_and_written() {
        let data = fs::read("rdm/fishery_others_cutout_lod0.rdm").unwrap();

        let mut reader = std::io::Cursor::new(&data);

        let rdm: RdmFile<RdmKindMesh> = reader.read_le().unwrap();

        let mut dst = Vec::new();
        let mut writer = std::io::Cursor::new(&mut dst);

        rdm.write_rdm(&mut writer).unwrap();

        let output_dir = tempfile::tempdir().unwrap();
        let output_path = output_dir.path().join("rdm_out.rdm");
        let mut file = fs::File::create(&output_path).unwrap();
        std::io::Write::write_all(&mut file, &dst).unwrap();

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

        self.inner.write_rdm(&mut writer).unwrap();
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

        rdm.header1.header2.as_mut().unwrap().export_name1 = Some(export_name.to_vec());

        let meta = rdm.header1.meta.as_mut().unwrap();
        // unknown maybe shader id
        // 0: no anim
        // 1: _Ib4
        // 2:
        // 3: I4b_W4b (eve)
        // 4: I4b_W4b (other npc)
        meta.format_identifiers.unknown_shader_id = if rdm_in.has_skin() { 1 } else { 0 };

        meta.skeleton_index = if rdm_in.has_skin() { 0 } else { -1 };

        meta.format_identifiers.rdm_container = rdm_in.vertex.identifiers.to_vec();
        meta.mesh_info = rdm_in.mesh_info.clone();
        (meta.bounds_min, meta.bounds_max) = rdm_in.vertex.calculate_bounds();
        meta.index_format.element_type = StorageType::U16;
        meta.triangle_list = RawBuffer {
            count: rdm_in.triangle_indices.len() as u32 * 3,
            stride: 2,
            bytes: rdm_in
                .triangle_indices
                .iter()
                .flat_map(|triangle| triangle.indices)
                .flat_map(|index| u16::try_from(index).unwrap().to_le_bytes())
                .collect(),
        };
        meta.vertex = RawBuffer {
            count: rdm_in.vertex.len(),
            stride: rdm_in.vertex.get_size(),
            bytes: rdm_in.vertex.as_bytes().to_vec(),
        };

        let material = br"Default Standard12432142134";
        let dummy_png_path = br"d:/projekte/anno5/game/testdata/graphics/dummy_objects/dummy_christian/rdm/basalt_crusher_others/diffuse.png";

        let mut mats = vec![];
        for _ in 0..MeshInfo::get_max_material(&rdm_in.mesh_info) + 1 {
            let dummy_mat = RdmBlobToMat {
                mat: RdmMat {
                    name: Some(material.to_vec()),
                    png: Some(dummy_png_path.to_vec()),
                    _padding: [0; 40],
                },
                _padding: [0; 24],
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
                    name: j.name.as_bytes().to_vec(),
                    t: [v_init.x, v_init.y, v_init.z],
                    r: [rot.x, rot.y, rot.z, rot.w],
                    parent_id: j.parent,
                    _padding: [0; 48],
                };

                replacement_raw_joints.push(res);
            }
            rdm.header1.skin.as_mut().unwrap().joint = replacement_raw_joints;
        } else {
            rdm.header1.skin = None;
        }
        rdm.header1.rdm_blob_to_mat = Some(mats);

        RdWriter2 { inner: rdm }
    }
}

use crate::rdm_writer::{Describe, Plan, Record};
impl Describe for ModelName {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut r = Record::new(plan);
        r.append_offset(self.name.as_ref().map(|value| plan.add_byte_block(value)));
        r.append_bytes(&self._padding);
        r
    }
}
impl Describe for VertId {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut r = Record::new(plan);
        r.append_offset(Some(plan.add_record_array(&self.rdm_container)));
        r.append_bytes(&[self.unknown_shader_id]);
        r.append_bytes(&self._padding);
        r
    }
}
impl Describe for MeshInfo {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut r = Record::new(plan);
        r.append_u32(self.start_index_location);
        r.append_u32(self.index_count);
        r.append_u32(self.material);
        r.append_bytes(&self._padding);
        r
    }
}
impl Describe for IndexFormat {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut r = Record::new(plan);
        r.append_u32(self.element_type as u32);
        r.append_bytes(&self._padding);
        r
    }
}
impl Describe for RdmBlobToMat {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut r = Record::new(plan);
        r.append_offset(Some(plan.add_record(&self.mat)));
        r.append_bytes(&self._padding);
        r
    }
}
impl Describe for RdmMat {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut r = Record::new(plan);
        r.append_offset(self.name.as_ref().map(|value| plan.add_byte_block(value)));
        r.append_offset(self.png.as_ref().map(|value| plan.add_byte_block(value)));
        r.append_bytes(&self._padding);
        r
    }
}
impl Describe for RdmBlobToJoint {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut r = Record::new(plan);
        r.append_offset(Some(plan.add_record_array(&self.joint)));
        r.append_bytes(&self._padding);
        r
    }
}
impl Describe for RdmJoint {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut r = Record::new(plan);
        r.append_offset(Some(plan.add_byte_block(&self.name)));
        r.append_f32s(&self.t);
        r.append_f32s(&self.r);
        r.append_u32(self.parent_id);
        r.append_bytes(&self._padding);
        r
    }
}
impl Describe for RdmHeader1 {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut r = Record::new(plan);
        r.append_offset(self.header2.as_ref().map(|value| plan.add_record(value)));
        r.append_offset(self.meta.as_ref().map(|value| plan.add_record(value)));
        r.append_offset(
            self.rdm_blob_to_mat
                .as_ref()
                .map(|value| plan.add_record_array(value)),
        );
        r.append_offset(self.skin.as_ref().map(|value| plan.add_record(value)));
        r.append_offset(self.meta_anim.as_ref().map(|value| plan.add_record(value)));
        r.append_bytes(&self._data);
        r
    }
}
impl Describe for ExportInfo {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let mut r = Record::new(plan);
        r.append_offset(
            self.export_name1
                .as_ref()
                .map(|value| plan.add_byte_block(value)),
        );
        r.append_offset(
            self.export_name2
                .as_ref()
                .map(|value| plan.add_byte_block(value)),
        );
        r.append_bytes(&self._data);
        r
    }
}

impl Describe for Meta {
    fn describe<'a>(&'a self, plan: &mut Plan<'a>) -> Record {
        let name = Some(plan.add_record(&self.model_name));
        let format = Some(plan.add_record(&self.format_identifiers));
        let index = Some(plan.add_record(&self.index_format));
        let groups = Some(plan.add_record_array(&self.mesh_info));
        let vertices =
            Some(plan.add_raw_block(self.vertex.count, self.vertex.stride, &self.vertex.bytes));
        let triangles = Some(plan.add_raw_block(
            self.triangle_list.count,
            self.triangle_list.stride,
            &self.triangle_list.bytes,
        ));
        let mut r = Record::new(plan);
        for target in [name, format, index, vertices, triangles, groups] {
            r.append_offset(target);
        }
        r.set_child_order(&[name, format, index, groups, vertices, triangles]);
        r.append_u32(self.skeleton_index as u32);
        r.append_f32s(&self.bounds_min);
        r.append_f32s(&self.bounds_max);
        let words = meta_word_count(self.part_size);
        assert!(
            self._unknown_words[words..].iter().all(|word| *word == 0),
            "compact metadata cannot discard extended words"
        );
        for word in &self._unknown_words[..words] {
            r.append_u32(*word);
        }
        r
    }
}
impl<T: RdmFileType + 'static> RdmFile<T> {
    /// Serialize using the source byte order and sequential output only.
    pub fn write_rdm(&self, writer: &mut impl std::io::Write) -> std::io::Result<()> {
        let mut plan = Plan::new(self.big_endian);
        let root = plan.add_record(&self.header1);
        plan.write_file(root, writer)
    }
}
