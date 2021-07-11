use bytes::{Buf, Bytes};

use std::mem::*;
use std::{fmt, str::FromStr};

use crate::*;

#[repr(C)]
#[derive(Clone, Debug)]
pub struct VertexIdentifier {
    pub uniq: UniqueIdentifier,
    pub unit_size: IdentifierSize,
    pub interpretation: u32,
    pub count: u32,
}

impl fmt::Display for VertexIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: cleanup
        let tmp = format!("{:?}", &self.uniq);
        let unit_size = match self.unit_size {
            IdentifierSize::U32 => 'b',
            IdentifierSize::U16 => 'h',
            IdentifierSize::F32 => 'f',
        };
        let r = if self.count == 0x1 { 4 } else { self.count };
        write!(f, "{}{}{}", tmp.chars().next().unwrap(), r, unit_size)
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum UniqueIdentifier {
    Position = 0x0,
    Normal = 0x1,
    GTangent = 0x2,
    Bitangent = 0x3,
    Texcoord = 0x4,
    C4c = 0x5,
    I4b = 0x7,
    W4b = 0x6,
    Invalid = 0x128,
}

impl UniqueIdentifier {
    const fn from(i: u32) -> Self {
        match i {
            0x0 => UniqueIdentifier::Position,
            0x1 => UniqueIdentifier::Normal,
            0x2 => UniqueIdentifier::GTangent,
            0x3 => UniqueIdentifier::Bitangent,
            0x4 => UniqueIdentifier::Texcoord,
            0x5 => UniqueIdentifier::C4c,
            0x7 => UniqueIdentifier::I4b,
            0x6 => UniqueIdentifier::W4b,
            _ => UniqueIdentifier::Invalid,
        }
    }
}

pub trait GetUniqueIdentifier {
    fn get_unique_identifier() -> UniqueIdentifier;
}

impl<T, const I: u32, const N: usize> GetUniqueIdentifier for AnnoData<T, I, N> {
    fn get_unique_identifier() -> UniqueIdentifier {
        AnnoData::<T, I, N>::TYPE
    }
}

impl From<u32> for UniqueIdentifier {
    fn from(i: u32) -> Self {
        UniqueIdentifier::from(i)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AnnoData<T, const I: u32, const N: usize> {
    pub data: [T; N],
}

impl<T, const I: u32, const N: usize> AnnoData<T, I, N> {
    const TYPE: UniqueIdentifier = UniqueIdentifier::from(I);
}

// rust const generics enum are unstable
pub(crate) type P3f = AnnoData<f32, { UniqueIdentifier::Position as u32 }, 3>;
pub(crate) type P4h = AnnoData<f16, { UniqueIdentifier::Position as u32 }, 4>;
pub(crate) type N3f = AnnoData<f32, { UniqueIdentifier::Normal as u32 }, 3>;
pub(crate) type N4b = AnnoData<u8, { UniqueIdentifier::Normal as u32 }, 4>;
pub(crate) type G3f = AnnoData<f32, { UniqueIdentifier::GTangent as u32 }, 3>;
pub(crate) type G4b = AnnoData<u8, { UniqueIdentifier::GTangent as u32 }, 4>;
#[allow(dead_code)]
pub(crate) type B3f = AnnoData<f32, { UniqueIdentifier::Bitangent as u32 }, 3>;
pub(crate) type B4b = AnnoData<u8, { UniqueIdentifier::Bitangent as u32 }, 4>;
pub(crate) type T2f = AnnoData<f32, { UniqueIdentifier::Texcoord as u32 }, 2>;
pub(crate) type T2h = AnnoData<f16, { UniqueIdentifier::Texcoord as u32 }, 2>;
pub(crate) type I4b = AnnoData<u8, { UniqueIdentifier::I4b as u32 }, 4>;
pub(crate) type W4b = AnnoData<u8, { UniqueIdentifier::W4b as u32 }, 4>;

impl<T: Default, const I: u32, const N: usize> Default for AnnoData<T, I, N> {
    fn default() -> Self {
        let data = {
            // Array Default is limited to N <= 32 and const generic allow arbitrary N. Using unsafe to workaround.
            let mut data: [MaybeUninit<T>; N] = unsafe { MaybeUninit::uninit().assume_init() };

            for elem in &mut data[..].iter_mut() {
                // slow but safe
                *elem = MaybeUninit::new(Default::default());
            }
            unsafe {
                // TODO: proper way unstable MaybeUninit::array_assume_init(data)
                let ptr = &mut data as *mut _ as *mut [T; N];
                let res = ptr.read();
                std::mem::forget(data);
                res
            }
        };
        Self { data }
    }
}

impl<const I: u32, const N: usize, const M: usize> From<AnnoData<f16, I, N>>
    for AnnoData<f32, I, M>
{
    fn from(input: AnnoData<f16, I, N>) -> Self {
        let mut data = Self::default().data;
        for (elem, k) in &mut data[..].iter_mut().zip(input.data.iter()) {
            *elem = f32::from(*k);
        }
        Self { data }
    }
}

impl<const I: u32, const M: usize> From<AnnoData<u8, I, 4>> for AnnoData<f32, I, M> {
    fn from(input: AnnoData<u8, I, 4>) -> Self {
        let mut data = Self::default().data;
        for (elem, k) in &mut data[..].iter_mut().zip(input.data.iter()) {
            *elem = ((2.0f32 * *k as f32) / 255.0f32) - 1.0f32;
        }
        Self { data }
    }
}

pub trait GetVertex {
    fn get_unit(b: &mut Bytes) -> Self;
}

impl<const I: u32, const N: usize> GetVertex for AnnoData<u8, I, N> {
    fn get_unit(b: &mut Bytes) -> Self {
        let mut o = Self::default();
        for n in o.data.iter_mut() {
            *n = b.get_u8();
        }
        o
    }
}

impl<const I: u32, const N: usize> GetVertex for AnnoData<f16, I, N> {
    fn get_unit(b: &mut Bytes) -> Self {
        let mut o = Self::default();
        for n in o.data.iter_mut() {
            *n = f16::from_bits(b.get_u16_le());
        }
        o
    }
}

impl<const I: u32, const N: usize> GetVertex for AnnoData<f32, I, N> {
    fn get_unit(b: &mut Bytes) -> Self {
        let mut o = Self::default();
        for n in o.data.iter_mut() {
            *n = b.get_f32_le();
        }
        o
    }
}

pub trait Normalise {
    /// calculate unit vector from self
    fn normalise(&self) -> Self;
}

impl<const I: u32, const N: usize> Normalise for AnnoData<f32, I, N> {
    fn normalise(&self) -> Self {
        let mut o = Self::default();
        let mut sq = 0f32;
        for e in self.data.iter() {
            sq += e * e;
        }
        sq = sq.sqrt();
        for (n, inp) in o.data.iter_mut().zip(self.data.iter()) {
            *n = inp / sq;
        }
        o
    }
}

#[derive(Debug)]
pub struct VertexFormat2 {
    identifiers: Vec<VertexIdentifier>,
    offsets: Vec<usize>,
    text: String,
    vertex_offset: u32,
    pub vertex_count: u32,
    size: u32,
    vertex_buffer: Bytes,
    pub weight_sum: Option<Vec<u32>>,
}

impl fmt::Display for VertexFormat2 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.text)
    }
}

impl VertexFormat2 {
    pub fn get_size(&self) -> u32 {
        self.size
    }

    pub fn is_empty(&self) -> bool {
        self.get_size() > 0
    }

    pub fn identifiers_len(&self) -> u32 {
        self.identifiers.len() as u32
    }

    // TODO FIX THIS IS **totally inefficient**
    pub fn set_weight_sum(&mut self) {
        let n = self.find_component_offsets(UniqueIdentifier::W4b).count();
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
        identifiers: Vec<VertexIdentifier>,
        vertex_count: u32,
        vertex_size: u32,
        vertex_offset: u32,
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
        text.truncate((identifiers.len() * 4) - 1);

        let size: u32 = identifiers.iter().map(|x| x.get_size()).sum();
        debug_assert_eq!(size, off as u32);
        assert_eq!(vertex_size, off as u32);

        VertexFormat2 {
            identifiers,
            offsets,
            text,
            vertex_count,
            size: off as u32,
            vertex_offset,
            vertex_buffer,
            weight_sum: None,
        }
    }

    pub fn read_format(mut buf: Bytes, rdm_size: u32) -> Self {
        buf.advance(32);
        let meta_deref = buf.get_u32_le();
        buf.seek(meta_deref, rdm_size);
        buf.advance(4);

        let format_identifiers_ptr = buf.get_u32_le();
        buf.advance((RdModell::VERTEX_META - 8) as usize);
        let vertex_offset = buf.get_u32_le();

        buf.seek(format_identifiers_ptr, rdm_size);

        let format_identifiers = buf.get_u32_le();
        assert_eq!(
            format_identifiers,
            (rdm_size - buf.remaining() as u32) - 4 + 8 + 24
        );

        buf.seek(format_identifiers - RdModell::META_COUNT, rdm_size);
        let num = buf.get_u32_le();
        let size = buf.get_u32_le();
        assert_eq!(size, 0x10);

        let mut vec: Vec<VertexIdentifier> = Vec::with_capacity(num as usize);

        for _ in 0..num {
            let dst = VertexIdentifier {
                uniq: UniqueIdentifier::from(buf.get_u32_le()),
                unit_size: IdentifierSize::from(buf.get_u32_le()),
                interpretation: buf.get_u32_le(),
                count: buf.get_u32_le(),
            };
            trace!("{}", dst.get_size());
            trace!("{}", dst.to_string());
            vec.push(dst);
        }
        buf.seek(vertex_offset - RdModell::META_COUNT, rdm_size);
        let vertex_count = buf.get_u32_le();
        let vertex_size = buf.get_u32_le();
        let mut vertex_buffer = buf;
        vertex_buffer.truncate((vertex_size * vertex_count) as usize);
        Self::new(vec, vertex_count, vertex_size, vertex_offset, vertex_buffer)
    }

    #[allow(clippy::needless_lifetimes)]
    pub fn find_component_offsets<'a>(
        &'a self,
        search_ident: UniqueIdentifier,
    ) -> impl Iterator<Item = usize> + 'a {
        self.identifiers
            .iter()
            .enumerate()
            .filter_map(move |(index, value)| {
                if value.uniq == search_ident {
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
        Z: GetUniqueIdentifier + GetVertex,
        T: GetUniqueIdentifier + GetVertex + From<Z> + 'a,
    >(
        &'a self,
        set: usize,
    ) -> Option<impl Iterator<Item = T> + 'a> {
        let offset_idx: Vec<usize> = self
            .find_component_offsets(T::get_unique_identifier())
            .collect();
        if offset_idx.is_empty() {
            return None;
        }
        assert!(offset_idx.len() < 5);
        let offset = self.offsets[offset_idx[set]];

        let unit_size = self.identifiers[offset_idx[set]].get_size() as usize;
        let mem_size_small = std::mem::size_of::<Z>();
        let mem_size_target = std::mem::size_of::<T>();
        assert!(mem_size_small == unit_size || mem_size_target == unit_size);

        let unit = &self.identifiers[offset_idx[set]];
        // TODO: this needs far better checking!
        let need_convert = unit.unit_size == IdentifierSize::U16
            || unit.count == 0x1 && unit.interpretation != 0x0;

        let mut count = 0;

        let mut vbuffer = self.vertex_buffer.clone();
        assert_eq!(vbuffer.len() as u32 % self.size, 0);
        let n = vbuffer.len() as u32 / self.size;
        assert_eq!(self.vertex_count, n);

        vbuffer.advance(offset);
        let it: _ = std::iter::from_fn(move || {
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
                vbuffer.advance((self.size as usize) - unit_size);
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

#[repr(u32)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum IdentifierSize {
    U32 = 0x5,
    U16 = 0x6,
    F32 = 0x7,
}

impl From<u32> for IdentifierSize {
    fn from(i: u32) -> Self {
        match i {
            0x5 => IdentifierSize::U32,
            0x6 => IdentifierSize::U16,
            0x7 => IdentifierSize::F32,
            _ => todo!(),
        }
    }
}

impl VertexIdentifier {
    pub fn get_size(&self) -> u32 {
        match self.unit_size {
            IdentifierSize::U32 => 4 * self.count,
            IdentifierSize::U16 => 2 * self.count,
            IdentifierSize::F32 => 4 * self.count,
        }
    }

    pub const fn p4h() -> Self {
        VertexIdentifier {
            uniq: UniqueIdentifier::Position,
            unit_size: IdentifierSize::U16,
            interpretation: 0,
            count: 4,
        }
    }

    pub const fn n4b() -> Self {
        VertexIdentifier {
            uniq: UniqueIdentifier::Normal,
            unit_size: IdentifierSize::U32,
            interpretation: 0x6,
            count: 1,
        }
    }

    pub const fn g4b() -> Self {
        VertexIdentifier {
            uniq: UniqueIdentifier::GTangent,
            unit_size: IdentifierSize::U32,
            interpretation: 0x6,
            count: 1,
        }
    }

    pub const fn b4b() -> Self {
        VertexIdentifier {
            uniq: UniqueIdentifier::Bitangent,
            unit_size: IdentifierSize::U32,
            interpretation: 0x6,
            count: 1,
        }
    }

    pub const fn t2h() -> Self {
        VertexIdentifier {
            uniq: UniqueIdentifier::Texcoord,
            unit_size: IdentifierSize::U16,
            interpretation: 0x0,
            count: 2,
        }
    }

    pub const fn i4b() -> Self {
        VertexIdentifier {
            uniq: UniqueIdentifier::I4b,
            unit_size: IdentifierSize::U32,
            interpretation: 0x0,
            count: 1,
        }
    }

    pub const fn w4b() -> Self {
        VertexIdentifier {
            uniq: UniqueIdentifier::W4b,
            unit_size: IdentifierSize::U32,
            interpretation: 0x2,
            count: 1,
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

#[derive(Debug, Clone, Eq, PartialEq)]
#[allow(non_camel_case_types)]
pub enum TargetVertexFormat {
    P4h_N4b_G4b_B4b_T2h,
    P4h_N4b_G4b_B4b_T2h_I4b,
    P4h_N4b_G4b_B4b_T2h_I4b_W4b,
}
impl FromStr for TargetVertexFormat {
    type Err = String;

    fn from_str(input: &str) -> Result<TargetVertexFormat, Self::Err> {
        match input {
            "P4h_N4b_G4b_B4b_T2h" => Ok(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h),
            "P4h_N4b_G4b_B4b_T2h_I4b" => Ok(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b),
            "P4h_N4b_G4b_B4b_T2h_I4b_W4b" => Ok(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_W4b),
            _ => Err(format!("Invalid value for VertexFormat: {}", input)),
        }
    }
}

#[cfg(test)]
mod tests_vertex {

    use super::*;

    #[test]
    fn identifier_bytes_equal() {
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
