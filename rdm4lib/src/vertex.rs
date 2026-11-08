use rdm_derive::RdmRecord;
use std::{fmt, str::FromStr};

use half::f16;

/// One 16-byte descriptor for an attribute in each vertex.
#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq, RdmRecord)]
#[rdm(strides(16), write)]
pub struct VertexIdentifier {
    pub semantic: VertexSemantic,
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VertexInterpretation(pub u32);

/// Role of an attribute within a vertex, encoded by the RDM semantic ID.
#[repr(u32)]
#[derive(Debug, Clone, Eq, PartialEq)]
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

impl From<u32> for VertexSemantic {
    fn from(i: u32) -> Self {
        VertexSemantic::from(i)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AnnoData<DataType, const IDENTIFIER: u32, const DATA_SIZE: usize> {
    pub data: [DataType; DATA_SIZE],
}

// Stable Rust cannot use VertexSemantic as a const-parameter type.
// Pass its numeric code instead; the aliases keep the semantic explicit.
pub(crate) type P3f = AnnoData<f32, { VertexSemantic::Position as u32 }, 3>;
pub(crate) type P4h = AnnoData<f16, { VertexSemantic::Position as u32 }, 4>;

pub(crate) type N3f = AnnoData<f32, { VertexSemantic::Normal as u32 }, 3>;
pub(crate) type N4b = AnnoData<u8, { VertexSemantic::Normal as u32 }, 4>;

pub(crate) type G3f = AnnoData<f32, { VertexSemantic::Tangent as u32 }, 3>;
pub(crate) type G4b = AnnoData<u8, { VertexSemantic::Tangent as u32 }, 4>;
pub(crate) type B3f = AnnoData<f32, { VertexSemantic::Bitangent as u32 }, 3>;

pub(crate) type T2h = AnnoData<f16, { VertexSemantic::Texcoord as u32 }, 2>;

pub(crate) type I4b = AnnoData<u8, { VertexSemantic::JointIndex as u32 }, 4>;
pub(crate) type W4b = AnnoData<u8, { VertexSemantic::Weight as u32 }, 4>;

/// Supported RDM scalar storage codes, shared by vertex and index formats.
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

impl VertexIdentifier {
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

/// Supported output presets; parsing `default` selects `P4h_N4b_G4b_B4b_T2h`.
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
            "default" | "P4h_N4b_G4b_B4b_T2h" => Ok(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h),
            "P4h_N4b_G4b_B4b_T2h_I4b" => Ok(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b),
            "P4h_N4b_G4b_B4b_T2h_I4b_W4b" => Ok(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_W4b),
            "P3f_N3f_G3f_B3f_T2f_C4b" => Ok(TargetVertexFormat::P3f_N3f_G3f_B3f_T2f_C4b),
            "P4h_N4b_G4b_B4b_T2h_C4b_C4b" => Ok(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_C4b_C4b),
            "P4h_T2h_C4b" => Ok(TargetVertexFormat::P4h_T2h_C4b),
            _ => Err(format!("Invalid value for VertexFormat: {}", input)),
        }
    }
}

impl TargetVertexFormat {
    /// Whether this output preset stores joint indices.
    pub fn has_joints(&self) -> bool {
        matches!(
            self,
            Self::P4h_N4b_G4b_B4b_T2h_I4b | Self::P4h_N4b_G4b_B4b_T2h_I4b_W4b
        )
    }
}

#[cfg(test)]
mod tests_vertex {

    use super::*;

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
        let format = crate::rdm::RdmVertexFormat {
            attributes: vec![component, VertexIdentifier::p3f()],
            selector: 0,
        };
        assert_eq!(
            format.to_string(),
            "Normal(type=U16,count=1,interpretation=99)_P3f"
        );
    }

    #[test]
    fn preserves_unknown_interpretation_codes_when_read_and_written() {
        use crate::rdm::{RdmEndian, RdmReadField, RdmReader, RdmWriteField, RdmWriter};
        let source = include_bytes!("../rdm/fishery_others_lod2.rdm");
        let mut reader = RdmReader::new(std::io::Cursor::new(source)).unwrap();
        let value = VertexInterpretation::read_field(
            &mut reader,
            &99u32.to_le_bytes(),
            0,
            "interpretation",
            (),
        )
        .unwrap();
        assert_eq!(value, VertexInterpretation(99));
        let mut writer = RdmWriter::new(std::io::Cursor::new(Vec::new()), RdmEndian::Little);
        let mut output = [0; 4];
        value
            .write_field(&mut writer, &mut output, 0, "interpretation", ())
            .unwrap();
        assert_eq!(output, 99u32.to_le_bytes());
    }

    #[test]
    fn matches_vertex_descriptor_bytes() {
        let p = p4h_n4b_g4b_b4b_t2h_i4b();
        use crate::rdm::{RdmEndian, RdmWriteRecord, RdmWriter};
        let mut writer = RdmWriter::new(std::io::Cursor::new(Vec::new()), RdmEndian::Little);
        for (i, descriptor) in p.iter().enumerate() {
            descriptor.write(&mut writer, i * 16).unwrap();
        }
        let output = writer.into_inner().into_inner();
        let bytes = output.as_chunks::<16>().0;
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
