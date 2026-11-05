use std::io::{Cursor, Read, Seek};

use rdm4lib::rdm::{
    self, RdmEndian, RdmError, RdmFieldLayout, RdmFrame, RdmHeader, RdmReadField, RdmReader,
    RdmRecordLayout,
};
use rdm_derive::RdmRecord;

#[derive(Debug, PartialEq)]
struct Scaled(u32);

impl RdmFieldLayout for Scaled {
    const WIDTH: usize = 4;
}

impl RdmReadField for Scaled {
    type Args<'a> = &'a u32;

    fn read_field<R: Read + Seek>(
        reader: &mut RdmReader<R>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        scale: Self::Args<'_>,
    ) -> Result<Self, RdmError> {
        let value = u32::read_field(reader, raw, at, context, ())?;
        value
            .checked_mul(*scale)
            .map(Self)
            .ok_or_else(|| RdmError::new(at, context, "scaled value fitting u32", value))
    }
}

#[derive(Debug, PartialEq)]
struct Bounded(u32);

impl RdmFieldLayout for Bounded {
    const WIDTH: usize = 4;
}

impl RdmReadField for Bounded {
    type Args<'a> = (&'a u32, &'a Scaled);

    fn read_field<R: Read + Seek>(
        reader: &mut RdmReader<R>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        (min, max): Self::Args<'_>,
    ) -> Result<Self, RdmError> {
        let value = u32::read_field(reader, raw, at, context, ())?;
        if !(*min..=max.0).contains(&value) {
            return Err(RdmError::new(
                at,
                context,
                format!("{min}..={}", max.0),
                value,
            ));
        }
        Ok(Self(value))
    }
}

#[derive(Debug, PartialEq, RdmRecord)]
#[rdm(strides(12, 16))]
struct Contextual {
    // Exercise ordinary field names that could collide with generated locals.
    reader: u32,
    #[rdm(args(reader))]
    bytes: Scaled,
    #[rdm(args(reader, bytes))]
    at: Bounded,
}

fn word(bytes: &mut Vec<u8>, value: u32, endian: RdmEndian) {
    bytes.extend_from_slice(&match endian {
        RdmEndian::Little => value.to_le_bytes(),
        RdmEndian::Big => value.to_be_bytes(),
    });
}

fn contextual(endian: RdmEndian, stride: u32, second_value: u32) -> Vec<u8> {
    let mut bytes = RdmHeader { endian }.encode().to_vec();
    word(&mut bytes, 2, endian);
    word(&mut bytes, stride, endian);
    for value in [7, second_value] {
        for field in [3, 4, value] {
            word(&mut bytes, field, endian);
        }
        bytes.extend(std::iter::repeat_n(0xa5, stride as usize - 12));
    }
    bytes
}

#[test]
fn derive_passes_borrowed_sibling_args_and_handles_multiple_strides() {
    assert_eq!(Contextual::SIZE, 12);
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        for stride in [12, 16] {
            let mut reader = RdmReader::new(Cursor::new(contextual(endian, stride, 9))).unwrap();
            let block = reader.read_block::<Contextual>(28).unwrap();
            assert_eq!(
                block.records,
                [
                    Contextual {
                        reader: 3,
                        bytes: Scaled(12),
                        at: Bounded(7)
                    },
                    Contextual {
                        reader: 3,
                        bytes: Scaled(12),
                        at: Bounded(9)
                    },
                ]
            );
            assert_eq!(block.tails, vec![0xa5; 2 * (stride as usize - 12)]);
        }
    }
}

#[test]
fn field_errors_identify_the_record_field_and_absolute_position() {
    let mut reader = RdmReader::new(Cursor::new(contextual(RdmEndian::Little, 16, 13))).unwrap();
    let error = reader.read_block::<Contextual>(28).unwrap_err();
    assert_eq!(error.offset(), 52);
    assert_eq!(error.context(), "Contextual.at");
    assert_eq!(error.expected(), "3..=12");
    assert_eq!(error.actual(), "13");
}

#[test]
fn derives_animation_array_fields_in_both_orders() {
    assert_eq!(RdmFrame::SIZE, 32);
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        let mut bytes = RdmHeader { endian }.encode().to_vec();
        word(&mut bytes, 1, endian);
        word(&mut bytes, 32, endian);
        for value in [0.0f32, 0.0, 0.0, 1.0, 1.5, -2.0, 3.0, 0.25] {
            word(&mut bytes, value.to_bits(), endian);
        }
        let mut reader = RdmReader::new(Cursor::new(bytes)).unwrap();
        let block = reader.read_block::<RdmFrame>(28).unwrap();
        assert_eq!(
            block.records,
            [RdmFrame {
                rotation: [0.0, 0.0, 0.0, 1.0],
                translation: [1.5, -2.0, 3.0],
                time: 0.25,
            }]
        );
        assert!(block.tails.is_empty());
    }
}
