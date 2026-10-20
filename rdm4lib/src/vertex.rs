use bytes::{Buf, Bytes};
use rdm_derive::RdmStructSize;
use std::{fmt, str::FromStr};

use crate::{rdm_data_main::RdmFile, *};
use binrw::binrw;

/// One 16-byte descriptor for an attribute in each vertex.
#[repr(C)]
#[derive(Clone, Debug)]
#[binrw]
#[bw(import_raw(end: &mut u64))]
#[derive(RdmStructSize)]
pub struct VertexIdentifier {
    pub semantic: VertexSemantic,
    #[br(assert(matches!(storage_type, StorageType::U32 | StorageType::F16 | StorageType::F32),
        "Unsupported vertex storage type"))]
    pub storage_type: StorageType,
    pub interpretation: VertexInterpretation,
    pub storage_count: u32,
}

impl fmt::Display for VertexIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let format = match self.storage_type {
            StorageType::F16 => Some((self.storage_count, 'h')),
            StorageType::F32 => Some((self.storage_count, 'f')),
            StorageType::U32 if self.storage_count == 1 => {
                let suffix = if (3..=5).contains(&self.interpretation.0) {
                    'c'
                } else {
                    'b'
                };
                Some((4, suffix))
            }
            _ => None,
        };
        match (self.semantic.format_letter(), format) {
            (Some(letter), Some((count, suffix))) => write!(f, "{letter}{count}{suffix}"),
            _ => write!(
                f,
                "{:?}(type={:?},count={},interpretation={})",
                self.semantic, self.storage_type, self.storage_count, self.interpretation.0
            ),
        }
    }
}

/// Controls how a vertex attribute's stored values are interpreted.
///
/// Observed uses in the examined Anno 2070 and Anno 1800 files:
/// - 0: float attributes and packed joint indices.
/// - 2: packed weights; also some Anno 1800 colors.
/// - 4: packed colors.
/// - 6: packed normals, tangents, and bitangents;
///   also some Anno 1800 colors.
///
/// Unknown values are preserved.
///
/// For packed U32 attributes, Anno 2070's format-name builder
/// uses `4c` for codes 3 through 5 and `4b` for other codes.
#[repr(transparent)]
#[binrw]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VertexInterpretation(pub u32);

/// Role of an attribute within a vertex, encoded by the RDM semantic ID.
#[repr(u32)]
#[derive(Debug, Clone, Eq, PartialEq)]
#[binrw]
#[brw(repr(u32))]
pub enum VertexSemantic {
    Position = 0x0,
    Normal = 0x1,
    Tangent = 0x2,
    Bitangent = 0x3,
    Texcoord = 0x4,
    Color = 0x5,
    JointIndex = 0x7,
    Weight = 0x6,
    Invalid = 0xFF,
}

impl VertexSemantic {
    // PNGBTCWI
    fn format_letter(&self) -> Option<char> {
        Some(match self {
            Self::Position => 'P',
            Self::Normal => 'N',
            Self::Tangent => 'G',
            Self::Bitangent => 'B',
            Self::Texcoord => 'T',
            Self::Color => 'C',
            Self::Weight => 'W',
            Self::JointIndex => 'I',
            Self::Invalid => return None,
        })
    }

    const fn from(i: u32) -> Self {
        match i {
            0x0 => VertexSemantic::Position,
            0x1 => VertexSemantic::Normal,
            0x2 => VertexSemantic::Tangent,
            0x3 => VertexSemantic::Bitangent,
            0x4 => VertexSemantic::Texcoord,
            0x5 => VertexSemantic::Color,
            0x7 => VertexSemantic::JointIndex,
            0x6 => VertexSemantic::Weight,
            _ => VertexSemantic::Invalid,
        }
    }
}

/// Associates a decoded vertex component with its semantic.
pub trait GetVertexSemantic {
    /// Return the role of this component.
    fn get_semantic() -> VertexSemantic;
}

impl<T: Default + Copy, const I: u32, const N: usize> GetVertexSemantic for AnnoData<T, I, N> {
    fn get_semantic() -> VertexSemantic {
        AnnoData::<T, I, N>::TYPE
    }
}

impl From<u32> for VertexSemantic {
    fn from(i: u32) -> Self {
        VertexSemantic::from(i)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AnnoData<DataType, const IDENTIFIER: u32, const DATA_SIZE: usize> {
    pub data: [DataType; DATA_SIZE],
}

impl<DataType: Default + Copy, const IDENTIFIER: u32, const DATA_SIZE: usize>
    AnnoData<DataType, IDENTIFIER, DATA_SIZE>
{
    const TYPE: VertexSemantic = VertexSemantic::from(IDENTIFIER);

    #[inline]
    /// Will call f N times to fill the struct's data array.
    fn from_fn_generic<F>(mut f: F) -> AnnoData<DataType, IDENTIFIER, DATA_SIZE>
    where
        F: FnMut(usize) -> DataType,
    {
        let mut data = Self::default();
        for (i, dst) in data.data.iter_mut().enumerate() {
            *dst = f(i);
        }
        data
    }
}

// Stable Rust cannot use VertexSemantic as a const-parameter type.
// Pass its numeric code instead; the aliases keep the semantic explicit.
pub(crate) type P3f = AnnoData<f32, { VertexSemantic::Position as u32 }, 3>;
pub(crate) type P4h = AnnoData<f16, { VertexSemantic::Position as u32 }, 4>;

pub(crate) type N3f = AnnoData<f32, { VertexSemantic::Normal as u32 }, 3>;
pub(crate) type N4b = AnnoData<u8, { VertexSemantic::Normal as u32 }, 4>;
#[allow(dead_code)]
pub(crate) type N3b = AnnoData<u8, { VertexSemantic::Normal as u32 }, 3>;

pub(crate) type G3f = AnnoData<f32, { VertexSemantic::Tangent as u32 }, 3>;
pub(crate) type G4b = AnnoData<u8, { VertexSemantic::Tangent as u32 }, 4>;
#[allow(dead_code)]
pub(crate) type B3f = AnnoData<f32, { VertexSemantic::Bitangent as u32 }, 3>;
pub(crate) type B4b = AnnoData<u8, { VertexSemantic::Bitangent as u32 }, 4>;

pub(crate) type T2f = AnnoData<f32, { VertexSemantic::Texcoord as u32 }, 2>;
pub(crate) type T2h = AnnoData<f16, { VertexSemantic::Texcoord as u32 }, 2>;

pub(crate) type I4b = AnnoData<u8, { VertexSemantic::JointIndex as u32 }, 4>;
pub(crate) type W4b = AnnoData<u8, { VertexSemantic::Weight as u32 }, 4>;

pub(crate) type C4b = AnnoData<u8, { VertexSemantic::Color as u32 }, 4>;
pub(crate) type C4c = AnnoData<i8, { VertexSemantic::Color as u32 }, 4>;

impl<T: Default + Copy, const I: u32, const N: usize> Default for AnnoData<T, I, N> {
    fn default() -> Self {
        Self {
            data: [Default::default(); N],
        }
    }
}

impl<const I: u32, const N: usize, const M: usize> From<AnnoData<f16, I, N>>
    for AnnoData<f32, I, M>
{
    fn from(input: AnnoData<f16, I, N>) -> Self {
        // This may panick if M > N
        Self::from_fn_generic(|i| f32::from(input.data[i]))
    }
}

impl<const I: u32, const M: usize> From<AnnoData<u8, I, 4>> for AnnoData<f32, I, M> {
    fn from(input: AnnoData<u8, I, 4>) -> Self {
        // This may panick if M > 4
        Self::from_fn_generic(|i| ((2.0f32 * input.data[i] as f32) / 255.0f32) - 1.0f32)
    }
}

pub trait GetVertex {
    fn get_unit(b: &mut Bytes) -> Self;
}

impl<const I: u32, const N: usize> GetVertex for AnnoData<u8, I, N> {
    fn get_unit(b: &mut Bytes) -> Self {
        Self::from_fn_generic(|_| b.get_u8())
    }
}

impl<const I: u32, const N: usize> GetVertex for AnnoData<f16, I, N> {
    fn get_unit(b: &mut Bytes) -> Self {
        Self::from_fn_generic(|_| f16::from_bits(b.get_u16_le()))
    }
}

impl<const I: u32, const N: usize> GetVertex for AnnoData<f32, I, N> {
    fn get_unit(b: &mut Bytes) -> Self {
        Self::from_fn_generic(|_| b.get_f32_le())
    }
}

pub trait Normalise {
    /// calculate unit vector from self
    fn normalise(&self) -> Self;
}

impl<const I: u32, const N: usize> Normalise for AnnoData<f32, I, N> {
    fn normalise(&self) -> Self {
        let vlen = self.data.iter().map(|v| v * v).sum::<f32>().sqrt();
        Self::from_fn_generic(|i| self.data[i] / vlen)
    }
}

#[derive(Debug)]
pub struct VertexFormat2 {
    pub identifiers: Box<[VertexIdentifier]>,
    offsets: Box<[usize]>,
    text: String,
    pub vertex_count: u32,
    size: u32,
    vertex_buffer: Bytes,
    pub weight_sum: Option<Vec<u32>>,
}

impl fmt::Display for VertexFormat2 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

impl VertexFormat2 {
    /// Calculate minimum and maximum XYZ from stored positions, before animation.
    /// Panics if positions are missing, empty, or non-finite.
    pub fn calculate_bounds(&self) -> ([f32; 3], [f32; 3]) {
        assert!(
            self.vertex_count > 0,
            "Cannot calculate bounds without vertices"
        );
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for position in self.iter::<P4h, P3f>(0).expect("Missing vertex positions") {
            for axis in 0..3 {
                let value = position.data[axis];
                assert!(value.is_finite(), "Non-finite vertex position");
                min[axis] = min[axis].min(value);
                max[axis] = max[axis].max(value);
            }
        }
        (min, max)
    }

    pub fn get_size(&self) -> u32 {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.get_size() > 0
    }

    pub fn identifiers_len(&self) -> u32 {
        self.identifiers.len() as u32
    }

    // TODO: remove for_each?
    pub fn set_weight_sum(&mut self) {
        let n = self.find_component_offsets(VertexSemantic::Weight).count();
        if n == 0 {
            self.weight_sum = Some(vec![255; self.len() as usize]);
        } else {
            let mut vec: Vec<u32> = vec![0; self.len() as usize];
            for i in 0..n {
                if let Some(iter) = self.iter::<W4b, W4b>(i) {
                    iter.zip(vec.iter_mut())
                        .for_each(|(e, dst)| *dst += e.data.iter().map(|&w| w as u32).sum::<u32>());
                }
            }
            self.weight_sum = Some(vec);
        }
    }

    pub fn identifiers_as_bytes(&self) -> &[u8] {
        let bytes = unsafe { self.identifiers.align_to::<u8>().1 };
        assert_eq!(bytes.len(), self.identifiers.len() * 0x10);
        bytes
    }

    pub fn new(
        identifiers: Box<[VertexIdentifier]>,
        vertex_count: u32,
        vertex_size: u32,
        vertex_buffer: Bytes,
    ) -> Self {
        let mut offsets = Vec::with_capacity(identifiers.len());
        let mut off = 0;

        let mut text = String::with_capacity(identifiers.len() * 4);
        for e in identifiers.iter() {
            text.push_str(&e.to_string());
            text.push('_');
            offsets.push(off);
            off += e.get_size() as usize;
        }
        text.pop(); // Remove the trailing separator.

        let size: u32 = identifiers.iter().map(|x| x.get_size()).sum();
        debug_assert_eq!(size, off as u32);
        assert_eq!(vertex_size, off as u32);

        VertexFormat2 {
            identifiers,
            offsets: offsets.into_boxed_slice(),
            text,
            vertex_count,
            size: off as u32,
            vertex_buffer,
            weight_sum: None,
        }
    }

    pub fn read_format_via_data(rdm: &RdmFile<RdmKindMesh>) -> Self {
        let meta: &rdm_data_main::Meta = &rdm.header1.meta.0;
        let format_identifiers = &meta.format_identifiers;

        let ids = &format_identifiers.rdm_container;
        assert_eq!(ids.info.part_size, 16);

        let vec: Vec<VertexIdentifier> = ids.iter().cloned().collect();

        let vertex_count = meta.vertex.info.count;
        let vertex_size = meta.vertex.info.part_size;
        let mut vertex_buffer: Vec<u8> = meta.vertex.iter().map(|x| x.0).collect();
        if rdm.big_endian {
            // Normalize to the LE representation used by all vertex consumers.
            // Packed N4b/I4b/W4b/etc. are U32 words, not four independent bytes.
            for vertex in vertex_buffer.chunks_exact_mut(vertex_size as usize) {
                let mut offset = 0;
                for id in &vec {
                    let width = id.storage_type.byte_size() as usize;
                    let end = offset + id.get_size() as usize;
                    for component in vertex[offset..end].chunks_exact_mut(width) {
                        component.reverse();
                    }
                    offset = end;
                }
            }
        }

        Self::new(
            vec.into_boxed_slice(),
            vertex_count,
            vertex_size,
            Bytes::from(vertex_buffer),
        )
    }

    #[allow(clippy::needless_lifetimes)]
    pub fn find_component_offsets<'a>(
        &'a self,
        search_ident: VertexSemantic,
    ) -> impl Iterator<Item = usize> + 'a {
        self.identifiers
            .iter()
            .enumerate()
            .filter_map(move |(index, value)| {
                if value.semantic == search_ident {
                    Some(index)
                } else {
                    None
                }
            })
    }

    pub fn len(&self) -> u32 {
        self.vertex_count
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.vertex_buffer
    }

    pub fn iter<
        'a,
        Z: GetVertexSemantic + GetVertex,
        T: GetVertexSemantic + GetVertex + From<Z> + 'a,
    >(
        &'a self,
        set: usize,
    ) -> Option<impl Iterator<Item = T> + 'a> {
        let offset_idx: Vec<usize> = self.find_component_offsets(T::get_semantic()).collect();
        if offset_idx.is_empty() {
            return None;
        }
        assert!(offset_idx.len() < 5);
        let offset = self.offsets[offset_idx[set]];

        let component_size = self.identifiers[offset_idx[set]].get_size() as usize;
        let mem_size_small = std::mem::size_of::<Z>();
        let mem_size_target = std::mem::size_of::<T>();
        assert!(mem_size_small == component_size || mem_size_target == component_size);

        let unit = &self.identifiers[offset_idx[set]];
        // TODO: this needs far better checking!
        let need_convert = unit.storage_type == StorageType::F16
            || unit.storage_count == 0x1 && unit.interpretation.0 != 0;

        let mut count = 0;

        let mut vbuffer = self.vertex_buffer.clone();
        assert_eq!(vbuffer.len() as u32 % self.size, 0);
        let n = vbuffer.len() as u32 / self.size;
        assert_eq!(self.vertex_count, n);

        vbuffer.advance(offset);
        let it = std::iter::from_fn(move || {
            let ret = if count < n {
                if need_convert {
                    Some(T::from(<Z as GetVertex>::get_unit(&mut vbuffer)))
                } else {
                    Some(<T as GetVertex>::get_unit(&mut vbuffer))
                }
            } else {
                None
            };
            count += 1;
            if count < n {
                vbuffer.advance((self.size as usize) - component_size);
            }
            ret
        });
        Some(it)
    }

    pub fn w4b_default_iter(&self) -> impl Iterator<Item = W4b> + '_ {
        std::iter::from_fn(|| {
            Some(W4b {
                data: [255, 0, 0, 0],
            })
        })
    }
}

/// Supported RDM scalar storage codes, shared by vertex and index formats.
#[binrw]
#[brw(repr(u32))]
#[repr(u32)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StorageType {
    U16 = 3,
    U32 = 5,
    F16 = 6,
    F32 = 7,
}

impl StorageType {
    /// Number of bytes occupied by one scalar or packed word.
    pub fn byte_size(self) -> u32 {
        match self {
            Self::U16 | Self::F16 => 2,
            Self::U32 | Self::F32 => 4,
        }
    }
}

impl From<u32> for StorageType {
    fn from(i: u32) -> Self {
        match i {
            3 => Self::U16,
            5 => Self::U32,
            6 => Self::F16,
            7 => Self::F32,
            _ => panic!("Unsupported RDM storage type: {}", i),
        }
    }
}

impl VertexIdentifier {
    pub fn get_size(&self) -> u32 {
        self.storage_type.byte_size() * self.storage_count
    }

    pub const fn p4h() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Position,
            storage_type: StorageType::F16,
            interpretation: VertexInterpretation(0),
            storage_count: 4,
        }
    }

    pub const fn p3f() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Position,
            storage_type: StorageType::F32,
            interpretation: VertexInterpretation(0),
            storage_count: 3,
        }
    }

    pub const fn n4b() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Normal,
            storage_type: StorageType::U32,
            interpretation: VertexInterpretation(0x6),
            storage_count: 1,
        }
    }

    pub const fn n3f() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Normal,
            storage_type: StorageType::F32,
            interpretation: VertexInterpretation(0x0),
            storage_count: 3,
        }
    }

    pub const fn g4b() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Tangent,
            storage_type: StorageType::U32,
            interpretation: VertexInterpretation(0x6),
            storage_count: 1,
        }
    }

    pub const fn g3f() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Tangent,
            storage_type: StorageType::F32,
            interpretation: VertexInterpretation(0x0),
            storage_count: 3,
        }
    }

    pub const fn b4b() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Bitangent,
            storage_type: StorageType::U32,
            interpretation: VertexInterpretation(0x6),
            storage_count: 1,
        }
    }

    pub const fn b3f() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Bitangent,
            storage_type: StorageType::U32,
            interpretation: VertexInterpretation(0x0),
            storage_count: 3,
        }
    }

    pub const fn t2h() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Texcoord,
            storage_type: StorageType::F16,
            interpretation: VertexInterpretation(0x0),
            storage_count: 2,
        }
    }

    pub const fn t2f() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Texcoord,
            storage_type: StorageType::F32,
            interpretation: VertexInterpretation(0x0),
            storage_count: 2,
        }
    }

    pub const fn i4b() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::JointIndex,
            storage_type: StorageType::U32,
            interpretation: VertexInterpretation(0x0),
            storage_count: 1,
        }
    }

    pub const fn w4b() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Weight,
            storage_type: StorageType::U32,
            interpretation: VertexInterpretation(0x2),
            storage_count: 1,
        }
    }

    pub const fn c4b() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Color,
            storage_type: StorageType::U32,
            interpretation: VertexInterpretation(0x4),
            storage_count: 1,
        }
    }

    pub const fn c4b_interpret2() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Color,
            storage_type: StorageType::U32,
            interpretation: VertexInterpretation(0x2),
            storage_count: 1,
        }
    }
    pub const fn c4b_interpret6() -> Self {
        VertexIdentifier {
            semantic: VertexSemantic::Color,
            storage_type: StorageType::U32,
            interpretation: VertexInterpretation(0x6),
            storage_count: 1,
        }
    }
}

pub const fn p4h_n4b_g4b_b4b_t2h_i4b() -> [VertexIdentifier; 6] {
    [
        VertexIdentifier::p4h(),
        VertexIdentifier::n4b(),
        VertexIdentifier::g4b(),
        VertexIdentifier::b4b(),
        VertexIdentifier::t2h(),
        VertexIdentifier::i4b(),
    ]
}

pub const fn p4h_n4b_g4b_b4b_t2h_i4b_w4b() -> [VertexIdentifier; 7] {
    [
        VertexIdentifier::p4h(),
        VertexIdentifier::n4b(),
        VertexIdentifier::g4b(),
        VertexIdentifier::b4b(),
        VertexIdentifier::t2h(),
        VertexIdentifier::i4b(),
        VertexIdentifier::w4b(),
    ]
}

pub const fn p4h_n4b_g4b_b4b_t2h() -> [VertexIdentifier; 5] {
    [
        VertexIdentifier::p4h(),
        VertexIdentifier::n4b(),
        VertexIdentifier::g4b(),
        VertexIdentifier::b4b(),
        VertexIdentifier::t2h(),
    ]
}

pub const fn p4h_n4b_g4b_b4b_t2h_c4b_c4b() -> [VertexIdentifier; 7] {
    [
        VertexIdentifier::p4h(),
        VertexIdentifier::n4b(),
        VertexIdentifier::g4b(),
        VertexIdentifier::b4b(),
        VertexIdentifier::t2h(),
        VertexIdentifier::c4b_interpret2(),
        VertexIdentifier::c4b_interpret6(),
    ]
}

pub const fn p3f_n3f_g3f_b3f_t2f_c4b() -> [VertexIdentifier; 6] {
    [
        VertexIdentifier::p3f(),
        VertexIdentifier::n3f(),
        VertexIdentifier::g3f(),
        VertexIdentifier::b3f(),
        VertexIdentifier::t2f(),
        VertexIdentifier::c4b(),
    ]
}

pub const fn p4h_t2h_c4c() -> [VertexIdentifier; 3] {
    [
        VertexIdentifier::p4h(),
        VertexIdentifier::t2h(),
        VertexIdentifier::c4b(),
    ]
}

#[derive(Debug, Clone, Eq, PartialEq)]
#[allow(non_camel_case_types)]
pub enum TargetVertexFormat {
    P4h_N4b_G4b_B4b_T2h,
    P4h_N4b_G4b_B4b_T2h_I4b,
    P4h_N4b_G4b_B4b_T2h_I4b_W4b,
    P3f_N3f_G3f_B3f_T2f_C4b,
    P4h_N4b_G4b_B4b_T2h_C4b_C4b,
    P4h_T2h_C4b,
}
impl FromStr for TargetVertexFormat {
    type Err = String;

    fn from_str(input: &str) -> Result<TargetVertexFormat, Self::Err> {
        match input {
            "P4h_N4b_G4b_B4b_T2h" => Ok(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h),
            "P4h_N4b_G4b_B4b_T2h_I4b" => Ok(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b),
            "P4h_N4b_G4b_B4b_T2h_I4b_W4b" => Ok(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_W4b),
            "P3f_N3f_G3f_B3f_T2f_C4b" => Ok(TargetVertexFormat::P3f_N3f_G3f_B3f_T2f_C4b),
            "P4h_N4b_G4b_B4b_T2h_C4b_C4b" => Ok(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_C4b_C4b),
            "P4h_T2h_C4b" => Ok(TargetVertexFormat::P4h_T2h_C4b),
            _ => Err(format!("Invalid value for VertexFormat: {}", input)),
        }
    }
}

pub trait VertexFormatProperties {
    fn has_weights(vertex_format: &TargetVertexFormat) -> bool;
    fn has_joints(vertex_format: &TargetVertexFormat) -> bool;
    fn has_colors(vertex_format: &TargetVertexFormat) -> bool;

    //this is for later when we want to implementmultiple indices and weights
    fn weight_component_count(vertex_format: &TargetVertexFormat) -> u32;
    fn joint_component_count(vertex_format: &TargetVertexFormat) -> u32;
    fn color_component_count(vertex_format: &TargetVertexFormat) -> u32;
}

impl VertexFormatProperties for TargetVertexFormat {
    fn has_weights(vertex_format: &TargetVertexFormat) -> bool {
        match vertex_format {
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h => false,
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b => false,
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_W4b => true,
            TargetVertexFormat::P3f_N3f_G3f_B3f_T2f_C4b => true,
            TargetVertexFormat::P4h_T2h_C4b => false,
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_C4b_C4b => false,
        }
    }

    fn has_joints(vertex_format: &TargetVertexFormat) -> bool {
        match vertex_format {
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h => false,
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b => true,
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_W4b => true,
            TargetVertexFormat::P3f_N3f_G3f_B3f_T2f_C4b => false,
            TargetVertexFormat::P4h_T2h_C4b => false,
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_C4b_C4b => false,
        }
    }

    fn has_colors(vertex_format: &TargetVertexFormat) -> bool {
        matches!(
            vertex_format,
            TargetVertexFormat::P3f_N3f_G3f_B3f_T2f_C4b
                | TargetVertexFormat::P4h_T2h_C4b
                | TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_C4b_C4b
        )
    }

    fn weight_component_count(vertex_format: &TargetVertexFormat) -> u32 {
        match vertex_format {
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_W4b => 1,
            _ => 0,
        }
    }

    fn joint_component_count(vertex_format: &TargetVertexFormat) -> u32 {
        match vertex_format {
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_W4b => 1,
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b => 1,
            _ => 0,
        }
    }

    fn color_component_count(vertex_format: &TargetVertexFormat) -> u32 {
        match vertex_format {
            TargetVertexFormat::P3f_N3f_G3f_B3f_T2f_C4b => 1,
            TargetVertexFormat::P4h_T2h_C4b => 1,
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_C4b_C4b => 2,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests_vertex {

    use super::*;

    #[test]
    fn calculates_xyz_bounds_with_attribute_offsets_and_f16_rounding() {
        for half in [false, true] {
            let mut bytes = Vec::new();
            for point in [[2.0006, -8.0, 3.0], [4.0, -2.0, 5.0]] {
                bytes.extend([0u8; 4]); // Normal before position.
                for value in point {
                    if half {
                        bytes.extend(f16::from_f32(value).to_le_bytes());
                    } else {
                        bytes.extend(value.to_le_bytes());
                    }
                }
                if half {
                    bytes.extend(f16::from_f32(100.0).to_le_bytes()); // Ignore W.
                }
            }
            let position = if half {
                VertexIdentifier::p4h()
            } else {
                VertexIdentifier::p3f()
            };
            let stride = 4 + position.get_size();
            let vertices = VertexFormat2::new(
                vec![VertexIdentifier::n4b(), position].into_boxed_slice(),
                2,
                stride,
                bytes.into(),
            );
            let min_x = if half {
                f16::from_f32(2.0006).to_f32()
            } else {
                2.0006
            };
            assert_eq!(
                vertices.calculate_bounds(),
                ([min_x, -8.0, 3.0], [4.0, -2.0, 5.0])
            );
        }
    }

    #[test]
    fn formats_semantics_with_game_prefix_letters() {
        for (semantic, letter) in [
            (VertexSemantic::Position, 'P'),
            (VertexSemantic::Normal, 'N'),
            (VertexSemantic::Tangent, 'G'),
            (VertexSemantic::Bitangent, 'B'),
            (VertexSemantic::Texcoord, 'T'),
            (VertexSemantic::Color, 'C'),
            (VertexSemantic::Weight, 'W'),
            (VertexSemantic::JointIndex, 'I'),
        ] {
            let mut component = VertexIdentifier::p3f();
            component.semantic = semantic;
            assert_eq!(component.to_string(), format!("{letter}3f"));
        }
    }

    #[test]
    fn formats_single_float_components_without_expanding_to_four() {
        let mut component = VertexIdentifier::p3f();
        component.storage_count = 1;
        assert_eq!(component.to_string(), "P1f");
        component.storage_type = StorageType::F16;
        assert_eq!(component.to_string(), "P1h");
    }

    #[test]
    fn formats_packed_words_as_four_bytes_with_interpretation_suffix() {
        let mut component = VertexIdentifier::n4b();
        for (code, expected) in [
            (0, "N4b"),
            (2, "N4b"),
            (3, "N4c"),
            (4, "N4c"),
            (5, "N4c"),
            (6, "N4b"),
            (99, "N4b"),
        ] {
            component.interpretation = VertexInterpretation(code);
            assert_eq!(component.to_string(), expected, "interpretation {code}");
        }
    }

    #[test]
    fn includes_unsupported_descriptor_details_without_truncating_format_names() {
        let mut component = VertexIdentifier::n4b();
        component.storage_type = StorageType::U16;
        component.interpretation = VertexInterpretation(99);
        let format = VertexFormat2::new(
            vec![component, VertexIdentifier::p3f()].into_boxed_slice(),
            0,
            14,
            Bytes::new(),
        );
        assert_eq!(
            format.to_string(),
            "Normal(type=U16,count=1,interpretation=99)_P3f"
        );
    }

    #[test]
    fn preserves_unknown_interpretation_codes_when_read_and_written() {
        use binrw::{BinReaderExt, BinWriterExt};
        let mut cursor = std::io::Cursor::new(99u32.to_le_bytes());
        let value: VertexInterpretation = cursor.read_le().unwrap();
        assert_eq!(value, VertexInterpretation(99));
        let mut output = std::io::Cursor::new(Vec::new());
        output.write_le(&value).unwrap();
        assert_eq!(output.into_inner(), 99u32.to_le_bytes());
    }

    #[test]
    fn matches_vertex_descriptor_bytes() {
        let p = p4h_n4b_g4b_b4b_t2h_i4b();
        let bytes = unsafe { p.align_to::<[u8; 16]>().1 };
        assert_eq!(bytes.len(), 6);

        const P4H_IDENTIFIER: [u8; 16] = [
            0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00,
            0x00, 0x00,
        ];

        const N4B_IDENTIFIER: [u8; 16] = [
            0x01, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00,
        ];

        const G4B_IDENTIFIER: [u8; 16] = [
            0x02, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00,
        ];

        const B4B_IDENTIFIER: [u8; 16] = [
            0x03, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00,
        ];

        const T2H_IDENTIFIER: [u8; 16] = [
            0x04, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00,
            0x00, 0x00,
        ];

        #[allow(dead_code)]
        const C4C_IDENTIFIER: [u8; 16] = [
            0x05, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00,
        ];

        const I4B_IDENTIFIER: [u8; 16] = [
            0x07, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00,
        ];

        #[allow(dead_code)]
        const W4B_IDENTIFIER: [u8; 16] = [
            0x06, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x00,
            0x00, 0x00,
        ];

        let bytes2: [[u8; 16]; 6] = [
            P4H_IDENTIFIER,
            N4B_IDENTIFIER,
            G4B_IDENTIFIER,
            B4B_IDENTIFIER,
            T2H_IDENTIFIER,
            I4B_IDENTIFIER,
        ];

        assert_eq!(bytes2.len(), bytes.len());
        for i in 0..6 {
            assert_eq!(&bytes2[i], &bytes[i]);
        }
    }
}
