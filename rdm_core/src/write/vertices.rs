//! Encodes a vertex block using the model's `VertexFormat`.

use super::{WriteField, Writer};
use crate::buffers::{AttributeValues, Vertices};
use crate::records::{StorageType, VertexFormat};
use crate::wire::{ByteOrder, Scalar};
use crate::Error;

impl WriteField for Vertices {
    type Args<'a> = &'a VertexFormat;

    fn write_field<E: ByteOrder>(
        &self,
        writer: &mut Writer<'_, E>,
        at: usize,
        context: &'static str,
        format: &VertexFormat,
    ) -> Result<(), Error> {
        if self.attributes.len() != format.attributes.len() {
            return Err(Error::new(
                at,
                context,
                format!(
                    "{} attributes from the vertex format",
                    format.attributes.len()
                ),
                self.attributes.len(),
            ));
        }

        let mut stride = 0;
        for (values, attribute) in self.attributes.iter().zip(&format.attributes) {
            let expected_len = self.count * attribute.components as usize;
            if storage_type(values) != attribute.storage_type || values.len() != expected_len {
                return Err(Error::new(
                    at,
                    context,
                    format!("{expected_len} {:?} values", attribute.storage_type),
                    format!("{} {:?} values", values.len(), storage_type(values)),
                ));
            }
            stride += attribute.storage_type.byte_width() * attribute.components as usize;
        }

        let payload = writer.begin_block(self.count, stride, context)?;
        let block = writer.bytes_mut(payload, self.count * stride);
        let mut offset = 0;
        for (values, attribute) in self.attributes.iter().zip(&format.attributes) {
            let components = attribute.components as usize;
            match values {
                AttributeValues::U16(v) => {
                    encode_column::<E, _>(block, stride, offset, components, v)
                }
                AttributeValues::U32(v) => {
                    encode_column::<E, _>(block, stride, offset, components, v)
                }
                AttributeValues::F16(v) => {
                    encode_column::<E, _>(block, stride, offset, components, v)
                }
                AttributeValues::F32(v) => {
                    encode_column::<E, _>(block, stride, offset, components, v)
                }
            }
            offset += attribute.storage_type.byte_width() * components;
        }
        writer.put_reference(at, payload);
        Ok(())
    }
}

fn storage_type(values: &AttributeValues) -> StorageType {
    match values {
        AttributeValues::U16(_) => StorageType::U16,
        AttributeValues::U32(_) => StorageType::U32,
        AttributeValues::F16(_) => StorageType::F16,
        AttributeValues::F32(_) => StorageType::F32,
    }
}

fn encode_column<E: ByteOrder, T: Scalar>(
    block: &mut [u8],
    stride: usize,
    offset: usize,
    components: usize,
    values: &[T],
) {
    let size = components * T::WIDTH;
    if size == 0 {
        return;
    }

    for (vertex, values) in block
        .chunks_exact_mut(stride)
        .zip(values.chunks_exact(components))
    {
        let attribute = &mut vertex[offset..offset + size];
        for (out, value) in attribute.chunks_exact_mut(T::WIDTH).zip(values) {
            value.encode::<E>(out);
        }
    }
}
