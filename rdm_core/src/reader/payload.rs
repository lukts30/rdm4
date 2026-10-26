//! Decoding of vertex and index blocks using their format records.

use crate::codec::{ByteOrder, Number};
use crate::error::{ErrorKind, Result};
use crate::payload::{AttributeData, Indices, Vertices};
use crate::reader::{Block, ReadBlock, Reader};
use crate::records::{StorageType, VertexDescriptor};

impl ReadBlock for Vertices {
    type Args<'a> = &'a [VertexDescriptor];

    fn read_block<'d>(
        r: &mut Reader<'d>,
        block: Block<'d>,
        descriptors: &[VertexDescriptor],
    ) -> Result<Self> {
        if descriptors.is_empty() {
            return Err(block.error(ErrorKind::EmptyVertexFormat));
        }
        let stride: usize = descriptors.iter().map(VertexDescriptor::byte_size).sum();
        if block.stride != stride {
            let kind = ErrorKind::StrideMismatch {
                expected: stride as u64,
                actual: block.stride as u64,
            };
            return Err(block.error(kind));
        }

        let mut attributes = Vec::with_capacity(descriptors.len());
        let mut offset = 0;
        for descriptor in descriptors {
            let components = descriptor.count as usize;
            let data = match descriptor.storage {
                StorageType::U32 => {
                    AttributeData::U32(column(&block, offset, components, r.order()))
                }
                StorageType::F16 => {
                    AttributeData::F16(column(&block, offset, components, r.order()))
                }
                StorageType::F32 => {
                    AttributeData::F32(column(&block, offset, components, r.order()))
                }
                actual => return Err(block.error(ErrorKind::UnsupportedStorage { actual })),
            };
            attributes.push(data);
            offset += descriptor.byte_size();
        }
        Ok(Vertices {
            count: block.count,
            attributes,
        })
    }
}

/// Gather one attribute's values from every vertex.
fn column<T: Number>(
    block: &Block<'_>,
    offset: usize,
    components: usize,
    order: ByteOrder,
) -> Vec<T> {
    let size = components * T::SIZE;
    let mut values = Vec::with_capacity(block.count * components);
    for vertex in block.payload.chunks_exact(block.stride) {
        let slots = vertex[offset..offset + size].chunks_exact(T::SIZE);
        values.extend(slots.map(|bytes| T::from_bytes(bytes, order)));
    }
    values
}

impl ReadBlock for Indices {
    type Args<'a> = StorageType;

    fn read_block<'d>(r: &mut Reader<'d>, block: Block<'d>, storage: StorageType) -> Result<Self> {
        let indices = match storage {
            StorageType::U16 => Indices::U16(index_values(&block, r.order())?),
            StorageType::U32 => Indices::U32(index_values(&block, r.order())?),
            actual => return Err(block.error(ErrorKind::UnsupportedStorage { actual })),
        };
        Ok(indices)
    }
}

fn index_values<T: Number>(block: &Block<'_>, order: ByteOrder) -> Result<Vec<T>> {
    if block.stride != T::SIZE {
        let kind = ErrorKind::StrideMismatch {
            expected: T::SIZE as u64,
            actual: block.stride as u64,
        };
        return Err(block.error(kind));
    }
    let values = block
        .payload
        .chunks_exact(T::SIZE)
        .map(|bytes| T::from_bytes(bytes, order))
        .collect();
    Ok(values)
}
