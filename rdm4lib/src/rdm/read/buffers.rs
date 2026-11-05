//! Buffer decoding with borrowed descriptor context.

use std::io::{Read, Seek};

use half::f16;

use super::{RdmReadField, RdmReader};
use crate::{
    rdm::{
        wire::Scalar, RdmAttributeValues, RdmEndian, RdmError, RdmFieldLayout, RdmIndexFormat,
        RdmIndices, RdmVertexFormat, RdmVertices,
    },
    vertex::StorageType,
};

impl RdmFieldLayout for RdmVertices {
    const WIDTH: usize = 4;
}

impl RdmReadField for RdmVertices {
    type Args<'a> = &'a RdmVertexFormat;

    fn read_field<R: Read + Seek>(
        reader: &mut RdmReader<R>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        format: Self::Args<'_>,
    ) -> Result<Self, RdmError> {
        let mut stride = 0usize;
        for attribute in &format.attributes {
            stride = (attribute.storage_type.byte_size() as usize)
                .checked_mul(attribute.storage_count as usize)
                .and_then(|size| stride.checked_add(size))
                .ok_or_else(|| {
                    RdmError::new(at, context, "representable vertex stride", "overflow")
                })?;
        }
        let (block, bytes) = reader.read_payload(raw, at, context, stride)?;
        let mut offset = 0;
        let mut attributes = Vec::with_capacity(format.attributes.len());
        for attribute in &format.attributes {
            let size =
                attribute.storage_type.byte_size() as usize * attribute.storage_count as usize;
            let range = offset..offset + size;
            let endian = reader.endian();
            let values = match attribute.storage_type {
                StorageType::U16 => {
                    RdmAttributeValues::U16(column::<u16>(&bytes, stride, range, endian))
                }
                StorageType::U32 => {
                    RdmAttributeValues::U32(column::<u32>(&bytes, stride, range, endian))
                }
                StorageType::F16 => {
                    RdmAttributeValues::F16(column::<f16>(&bytes, stride, range, endian))
                }
                StorageType::F32 => {
                    RdmAttributeValues::F32(column::<f32>(&bytes, stride, range, endian))
                }
            };
            attributes.push(values);
            offset += size;
        }
        Ok(Self {
            count: block.count as usize,
            attributes,
        })
    }
}

fn column<T: Scalar>(
    bytes: &[u8],
    stride: usize,
    range: std::ops::Range<usize>,
    endian: RdmEndian,
) -> Vec<T> {
    bytes
        .chunks_exact(stride)
        .flat_map(|vertex| {
            (range.start..range.end)
                .step_by(T::WIDTH)
                .map(move |at| endian.decode(&vertex[at..at + T::WIDTH]))
        })
        .collect()
}

impl RdmFieldLayout for RdmIndices {
    const WIDTH: usize = 4;
}

impl RdmReadField for RdmIndices {
    type Args<'a> = (&'a RdmIndexFormat, &'a RdmVertices);

    fn read_field<R: Read + Seek>(
        reader: &mut RdmReader<R>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        (format, vertices): Self::Args<'_>,
    ) -> Result<Self, RdmError> {
        if !matches!(format.storage_type, StorageType::U16 | StorageType::U32) {
            return Err(RdmError::new(
                at,
                context,
                "index storage U16 or U32",
                format!("{:?}", format.storage_type),
            ));
        }
        let stride = format.storage_type.byte_size() as usize;
        let (block, bytes) = reader.read_payload(raw, at, context, stride)?;
        let mut values = Vec::with_capacity(block.count as usize);
        for (i, raw) in bytes.chunks_exact(stride).enumerate() {
            let index = match format.storage_type {
                StorageType::U16 => u32::from(reader.endian().decode::<u16>(raw)),
                _ => reader.endian().decode::<u32>(raw),
            };
            if index as usize >= vertices.count {
                return Err(RdmError::new(
                    block.payload.start + i * stride,
                    context,
                    format!("index below vertex count {}", vertices.count),
                    index,
                ));
            }
            values.push(index);
        }
        Ok(Self { values })
    }
}
