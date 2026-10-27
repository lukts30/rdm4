//! Decodes a vertex block using the model's `VertexFormat`.

use half::f16;

use super::{Block, ReadField, Reader};
use crate::buffers::{AttributeValues, Vertices};
use crate::records::{StorageType, VertexAttribute, VertexFormat};
use crate::wire::{ByteOrder, Scalar};
use crate::Error;

impl ReadField for Vertices {
    type Args<'a> = &'a VertexFormat;

    fn read_field<E: ByteOrder>(
        reader: &mut Reader<'_, E>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        format: &VertexFormat,
    ) -> Result<Self, Error> {
        let block = reader.enter_block(raw, at, context)?;
        let expected_stride = vertex_stride(&format.attributes, block.payload, context)?;
        if block.stride != expected_stride {
            return Err(Error::new(
                block.payload - 4,
                context,
                format!("vertex stride {expected_stride} from the vertex format"),
                block.stride,
            ));
        }

        let payload = reader.block_bytes(&block);
        let mut offset = 0;
        let mut attributes = Vec::with_capacity(format.attributes.len());
        for attribute in &format.attributes {
            attributes.push(decode_attribute::<E>(payload, &block, offset, attribute));
            offset += attribute_size(attribute);
        }

        Ok(Vertices {
            count: block.count,
            attributes,
        })
    }
}

fn attribute_size(attribute: &VertexAttribute) -> usize {
    attribute.storage_type.byte_width() * attribute.components as usize
}

/// Sum of attribute sizes, rejecting formats whose size overflows.
fn vertex_stride(
    attributes: &[VertexAttribute],
    payload: usize,
    context: &'static str,
) -> Result<usize, Error> {
    let mut stride: usize = 0;
    for attribute in attributes {
        let size = attribute
            .storage_type
            .byte_width()
            .checked_mul(attribute.components as usize);
        let Some(next) = size.and_then(|size| stride.checked_add(size)) else {
            return Err(Error::new(
                payload - 4,
                context,
                "vertex format with a representable size",
                format!(
                    "{} components of {:?}",
                    attribute.components, attribute.storage_type
                ),
            ));
        };
        stride = next;
    }
    Ok(stride)
}

fn decode_attribute<E: ByteOrder>(
    payload: &[u8],
    block: &Block,
    offset: usize,
    attribute: &VertexAttribute,
) -> AttributeValues {
    let components = attribute.components as usize;
    match attribute.storage_type {
        StorageType::U16 => {
            AttributeValues::U16(decode_column::<E, u16>(payload, block, offset, components))
        }
        StorageType::U32 => {
            AttributeValues::U32(decode_column::<E, u32>(payload, block, offset, components))
        }
        StorageType::F16 => {
            AttributeValues::F16(decode_column::<E, f16>(payload, block, offset, components))
        }
        StorageType::F32 => {
            AttributeValues::F32(decode_column::<E, f32>(payload, block, offset, components))
        }
    }
}

/// Decodes one attribute of every vertex.
///
/// The block stride equals the format's vertex size, so each attribute lies
/// inside every vertex. Observed component counts use a fixed-size loop.
fn decode_column<E: ByteOrder, T: Scalar>(
    payload: &[u8],
    block: &Block,
    offset: usize,
    components: usize,
) -> Vec<T> {
    match components {
        0 => Vec::new(),
        1 => decode_fixed::<E, T, 1>(payload, block.stride, offset).into_flattened(),
        2 => decode_fixed::<E, T, 2>(payload, block.stride, offset).into_flattened(),
        3 => decode_fixed::<E, T, 3>(payload, block.stride, offset).into_flattened(),
        4 => decode_fixed::<E, T, 4>(payload, block.stride, offset).into_flattened(),
        _ => decode_any::<E, T>(payload, block, offset, components),
    }
}

fn decode_fixed<E: ByteOrder, T: Scalar, const N: usize>(
    payload: &[u8],
    stride: usize,
    offset: usize,
) -> Vec<[T; N]> {
    let size = N * T::WIDTH;
    payload
        .chunks_exact(stride)
        .map(|vertex| <[T; N]>::decode::<E>(&vertex[offset..offset + size]))
        .collect()
}

fn decode_any<E: ByteOrder, T: Scalar>(
    payload: &[u8],
    block: &Block,
    offset: usize,
    components: usize,
) -> Vec<T> {
    let size = components * T::WIDTH;
    let mut values = Vec::with_capacity(block.count * components);
    for vertex in payload.chunks_exact(block.stride) {
        let attribute = &vertex[offset..offset + size];
        for raw in attribute.chunks_exact(T::WIDTH) {
            values.push(T::decode::<E>(raw));
        }
    }
    values
}
