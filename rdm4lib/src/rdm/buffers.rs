//! Buffer codecs with borrowed descriptor context.

use std::io::{Read, Seek, Write};

use half::f16;

use super::{RdmReadField, RdmReader, RdmWriteField, RdmWriter};
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
        let stride = vertex_stride(format, at, context)? as usize;
        let (block, bytes) = reader.read_payload(raw, at, context, stride)?;
        vertices_from_bytes(
            &bytes,
            block.count as usize,
            format,
            reader.endian(),
            block.payload.start,
            context,
        )
    }
}

pub(super) fn vertices_from_bytes(
    bytes: &[u8],
    count: usize,
    format: &RdmVertexFormat,
    endian: RdmEndian,
    at: usize,
    context: &'static str,
) -> Result<RdmVertices, RdmError> {
    let stride = vertex_stride(format, at, context)? as usize;
    let size = count.checked_mul(stride).ok_or_else(|| {
        RdmError::new(at, context, "representable vertex buffer size", "overflow")
    })?;
    if bytes.len() != size {
        return Err(RdmError::new(
            at,
            context,
            format!("{size} vertex bytes"),
            bytes.len(),
        ));
    }
    let mut offset = 0;
    let mut attributes = Vec::with_capacity(format.attributes.len());
    for attribute in &format.attributes {
        let size = attribute.storage_type.byte_size() as usize * attribute.storage_count as usize;
        let range = offset..offset + size;
        let values = match attribute.storage_type {
            StorageType::U16 => {
                RdmAttributeValues::U16(column::<u16>(bytes, stride, range, endian))
            }
            StorageType::U32 => {
                RdmAttributeValues::U32(column::<u32>(bytes, stride, range, endian))
            }
            StorageType::F16 => {
                RdmAttributeValues::F16(column::<f16>(bytes, stride, range, endian))
            }
            StorageType::F32 => {
                RdmAttributeValues::F32(column::<f32>(bytes, stride, range, endian))
            }
        };
        attributes.push(values);
        offset += size;
    }
    Ok(RdmVertices { count, attributes })
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
        let stride = index_stride(format, at, context)? as usize;
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

impl RdmWriteField for RdmVertices {
    type Args<'a> = &'a RdmVertexFormat;

    fn write_field<W: Write + Seek>(
        &self,
        writer: &mut RdmWriter<W>,
        raw: &mut [u8],
        at: usize,
        context: &'static str,
        format: Self::Args<'_>,
    ) -> Result<(), RdmError> {
        let (stride, bytes) = vertex_bytes(self, format, writer.endian(), at, context)?;
        writer.write_payload(raw, at, context, self.count, stride, &bytes)
    }
}

impl RdmWriteField for RdmIndices {
    type Args<'a> = (&'a RdmIndexFormat, &'a RdmVertices);

    fn write_field<W: Write + Seek>(
        &self,
        writer: &mut RdmWriter<W>,
        raw: &mut [u8],
        at: usize,
        context: &'static str,
        (format, vertices): Self::Args<'_>,
    ) -> Result<(), RdmError> {
        let stride = index_stride(format, at, context)?;
        let size = self
            .values
            .len()
            .checked_mul(stride as usize)
            .ok_or_else(|| {
                RdmError::new(at, context, "representable index buffer size", "overflow")
            })?;
        let mut bytes = vec![0; size];
        for (&index, raw) in self
            .values
            .iter()
            .zip(bytes.chunks_exact_mut(stride as usize))
        {
            if index as usize >= vertices.count {
                return Err(RdmError::new(
                    at,
                    context,
                    format!("index below vertex count {}", vertices.count),
                    index,
                ));
            }
            match format.storage_type {
                StorageType::U16 => writer.endian().encode(
                    u16::try_from(index)
                        .map_err(|_| RdmError::new(at, context, "index fitting U16", index))?,
                    raw,
                ),
                _ => writer.endian().encode(index, raw),
            }
        }
        writer.write_payload(raw, at, context, self.values.len(), stride, &bytes)
    }
}

fn vertex_stride(
    format: &RdmVertexFormat,
    at: usize,
    context: &'static str,
) -> Result<u32, RdmError> {
    let stride = format
        .attributes
        .iter()
        .try_fold(0u32, |stride, attribute| {
            attribute
                .storage_type
                .byte_size()
                .checked_mul(attribute.storage_count)
                .and_then(|size| stride.checked_add(size))
        })
        .filter(|&stride| stride != 0);
    stride.ok_or_else(|| {
        RdmError::new(
            at,
            context,
            "nonzero vertex stride fitting u32",
            "zero or overflow",
        )
    })
}

fn index_stride(
    format: &RdmIndexFormat,
    at: usize,
    context: &'static str,
) -> Result<u32, RdmError> {
    if !matches!(format.storage_type, StorageType::U16 | StorageType::U32) {
        return Err(RdmError::new(
            at,
            context,
            "index storage U16 or U32",
            format!("{:?}", format.storage_type),
        ));
    }
    Ok(format.storage_type.byte_size())
}

pub(super) fn vertex_bytes(
    vertices: &RdmVertices,
    format: &RdmVertexFormat,
    endian: RdmEndian,
    at: usize,
    context: &'static str,
) -> Result<(u32, Vec<u8>), RdmError> {
    let stride = vertex_stride(format, at, context)?;
    if vertices.attributes.len() != format.attributes.len() {
        return Err(RdmError::new(
            at,
            context,
            format!("{} attribute columns", format.attributes.len()),
            vertices.attributes.len(),
        ));
    }
    let size = vertices.count.checked_mul(stride as usize).ok_or_else(|| {
        RdmError::new(at, context, "representable vertex buffer size", "overflow")
    })?;
    for (i, (attribute, values)) in format
        .attributes
        .iter()
        .zip(&vertices.attributes)
        .enumerate()
    {
        let (storage, len) = match values {
            RdmAttributeValues::U16(v) => (StorageType::U16, v.len()),
            RdmAttributeValues::U32(v) => (StorageType::U32, v.len()),
            RdmAttributeValues::F16(v) => (StorageType::F16, v.len()),
            RdmAttributeValues::F32(v) => (StorageType::F32, v.len()),
        };
        let components = attribute.storage_count as usize;
        let count = vertices.count * components;
        if storage != attribute.storage_type || len != count {
            return Err(RdmError::new(
                at,
                context,
                format!(
                    "attribute {i}: {:?}, {count} values",
                    attribute.storage_type
                ),
                format!("{storage:?}, {len} values"),
            ));
        }
    }
    let mut bytes = vec![0; size];
    let mut offset = 0;
    for (attribute, values) in format.attributes.iter().zip(&vertices.attributes) {
        let components = attribute.storage_count as usize;
        match values {
            RdmAttributeValues::U16(v) => {
                interleave(v, &mut bytes, stride as usize, offset, components, endian)
            }
            RdmAttributeValues::U32(v) => {
                interleave(v, &mut bytes, stride as usize, offset, components, endian)
            }
            RdmAttributeValues::F16(v) => {
                interleave(v, &mut bytes, stride as usize, offset, components, endian)
            }
            RdmAttributeValues::F32(v) => {
                interleave(v, &mut bytes, stride as usize, offset, components, endian)
            }
        }
        offset += attribute.storage_type.byte_size() as usize * components;
    }
    Ok((stride, bytes))
}

fn interleave<T: Scalar>(
    values: &[T],
    bytes: &mut [u8],
    stride: usize,
    offset: usize,
    components: usize,
    endian: RdmEndian,
) {
    for i in 0..bytes.len() / stride {
        for component in 0..components {
            let at = i * stride + offset + component * T::WIDTH;
            endian.encode(
                values[i * components + component],
                &mut bytes[at..at + T::WIDTH],
            );
        }
    }
}
