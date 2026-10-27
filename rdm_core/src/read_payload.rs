//! Decoding of vertex and index blocks, whose layout is described by
//! format records read earlier in the same model.

use std::ops::Range;

use facet::Facet;
use facet_reflect::Partial;

use crate::byte_order::ByteOrder;
use crate::error::ErrorKind;
use crate::records::{
    AttributeValues, IndexFormat, Indices, StorageType, VertexAttribute, Vertices, F16,
};

/// The payload bytes of a block, already bounds-checked.
pub(crate) struct Payload<'a> {
    pub bytes: &'a [u8],
    pub count: u32,
    pub stride: u32,
    pub byte_order: ByteOrder,
}

/// Formats seen since the last payload, in field order.
///
/// `Model` lists its vertex and index formats before the payloads they
/// describe. Each payload consumes its format, so one model's format is
/// never applied to another model's data.
#[derive(Default)]
pub(crate) struct PayloadContext {
    attributes: Option<Vec<VertexAttribute>>,
    index_type: Option<StorageType>,
}

impl PayloadContext {
    /// Captures a just-built format block.
    pub(crate) fn observe(&mut self, builder: &mut Partial<'static, false>) {
        let shape = builder.shape();
        if shape.id == <Vec<VertexAttribute>>::SHAPE.id {
            self.attributes = builder.read_as::<Vec<VertexAttribute>>().cloned();
        } else if shape.id == IndexFormat::SHAPE.id {
            self.index_type = builder
                .read_as::<IndexFormat>()
                .map(|format| format.storage_type);
        }
    }

    pub(crate) fn decode_vertices(&mut self, payload: &Payload) -> Result<Vertices, ErrorKind> {
        let Some(attributes) = self.attributes.take() else {
            return Err(ErrorKind::mismatch(
                "vertex format",
                "attributes read before vertices",
                "none",
            ));
        };
        let expected: u64 = attributes.iter().map(attribute_width).sum();
        if expected != payload.stride as u64 {
            return Err(ErrorKind::mismatch(
                "vertex stride",
                format_args!("{expected} (sum of attribute sizes)"),
                payload.stride,
            ));
        }

        let mut columns = Vec::with_capacity(attributes.len());
        let mut offset = 0;
        for attribute in &attributes {
            columns.push(decode_column(payload, attribute, offset));
            offset += attribute_width(attribute) as usize;
        }
        Ok(Vertices {
            count: payload.count,
            attributes: columns,
        })
    }

    pub(crate) fn decode_indices(&mut self, payload: &Payload) -> Result<Indices, ErrorKind> {
        let Some(storage_type) = self.index_type.take() else {
            return Err(ErrorKind::mismatch(
                "index format",
                "format read before indices",
                "none",
            ));
        };
        if !matches!(storage_type, StorageType::U16 | StorageType::U32) {
            return Err(ErrorKind::mismatch(
                "index storage type",
                "U16 or U32",
                format_args!("{storage_type:?}"),
            ));
        }
        if storage_type.byte_size() != payload.stride {
            return Err(ErrorKind::mismatch(
                "index stride",
                storage_type.byte_size(),
                payload.stride,
            ));
        }

        let order = payload.byte_order;
        let indices = match storage_type {
            StorageType::U16 => Indices::U16(decode_values(payload.bytes, |b| order.u16(b))),
            _ => Indices::U32(decode_values(payload.bytes, |b| order.u32(b))),
        };
        Ok(indices)
    }
}

fn attribute_width(attribute: &VertexAttribute) -> u64 {
    attribute.storage_type.byte_size() as u64 * attribute.storage_count as u64
}

/// Decodes one attribute column out of interleaved vertices.
fn decode_column(payload: &Payload, attribute: &VertexAttribute, offset: usize) -> AttributeValues {
    let column = offset..offset + attribute_width(attribute) as usize;
    let bytes = payload.bytes;
    let stride = payload.stride as usize;
    let order = payload.byte_order;
    match attribute.storage_type {
        StorageType::U16 => {
            AttributeValues::U16(decode_strided(bytes, stride, column, |b| order.u16(b)))
        }
        StorageType::U32 => {
            AttributeValues::U32(decode_strided(bytes, stride, column, |b| order.u32(b)))
        }
        StorageType::F16 => {
            AttributeValues::F16(decode_strided(bytes, stride, column, |b| F16(order.u16(b))))
        }
        StorageType::F32 => AttributeValues::F32(decode_strided(bytes, stride, column, |b| {
            f32::from_bits(order.u32(b))
        })),
    }
}

/// Decodes the `column` byte range of every `stride`-sized vertex as `N`-byte values.
fn decode_strided<const N: usize, T>(
    bytes: &[u8],
    stride: usize,
    column: Range<usize>,
    decode: impl Fn([u8; N]) -> T,
) -> Vec<T> {
    if stride == 0 {
        return Vec::new();
    }
    let mut values = Vec::with_capacity(bytes.len() / stride * column.len() / N);
    for vertex in bytes.chunks_exact(stride) {
        let (lanes, _) = vertex[column.clone()].as_chunks::<N>();
        values.extend(lanes.iter().map(|&lane| decode(lane)));
    }
    values
}

/// Decodes a tightly packed run of `N`-byte values.
fn decode_values<const N: usize, T>(bytes: &[u8], decode: impl Fn([u8; N]) -> T) -> Vec<T> {
    bytes
        .as_chunks::<N>()
        .0
        .iter()
        .map(|&value| decode(value))
        .collect()
}
