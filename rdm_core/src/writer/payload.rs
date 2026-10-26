//! Encoding of vertex and index blocks using their format records.

use crate::codec::{ByteOrder, Number};
use crate::error::{Error, ErrorKind, Result};
use crate::payload::{AttributeData, Indices, Vertices};
use crate::records::{StorageType, VertexDescriptor};
use crate::writer::{WriteBlock, Writer};

impl WriteBlock for Vertices {
    type Args<'a> = &'a [VertexDescriptor];

    fn write_block(
        &self,
        w: &mut Writer<'_>,
        context: &'static str,
        descriptors: &[VertexDescriptor],
    ) -> Result<()> {
        if self.attributes.len() != descriptors.len() {
            let kind = ErrorKind::CountMismatch {
                expected: descriptors.len() as u64,
                actual: self.attributes.len() as u64,
            };
            return Err(Error::new(w.position(), context, kind));
        }
        let stride: usize = descriptors.iter().map(VertexDescriptor::byte_size).sum();
        let start = w.begin_block(self.count, stride, context)?;
        let error = |kind| Error::new(start, context, kind);

        let order = w.order();
        let payload = w.payload_mut(start);
        let mut offset = 0;
        for (data, descriptor) in self.attributes.iter().zip(descriptors) {
            let column = Column {
                stride,
                offset,
                components: descriptor.count as usize,
                order,
            };
            let written = match (data, descriptor.storage) {
                (AttributeData::U32(values), StorageType::U32) => column.write(payload, values),
                (AttributeData::F16(values), StorageType::F16) => column.write(payload, values),
                (AttributeData::F32(values), StorageType::F32) => column.write(payload, values),
                (data, expected) => {
                    return Err(error(ErrorKind::StorageMismatch {
                        expected,
                        actual: data.storage(),
                    }));
                }
            };
            written.map_err(error)?;
            offset += descriptor.byte_size();
        }
        Ok(())
    }
}

/// Position of one attribute within each vertex.
struct Column {
    stride: usize,
    offset: usize,
    components: usize,
    order: ByteOrder,
}

impl Column {
    /// Scatter one attribute's values into every vertex of `payload`.
    fn write<T: Number>(
        &self,
        payload: &mut [u8],
        values: &[T],
    ) -> std::result::Result<(), ErrorKind> {
        let vertex_count = payload.len() / self.stride;
        if values.len() != vertex_count * self.components {
            return Err(ErrorKind::CountMismatch {
                expected: (vertex_count * self.components) as u64,
                actual: values.len() as u64,
            });
        }
        let size = self.components * T::SIZE;
        for (vertex, chunk) in payload
            .chunks_exact_mut(self.stride)
            .zip(values.chunks_exact(self.components))
        {
            let slots = vertex[self.offset..self.offset + size].chunks_exact_mut(T::SIZE);
            for (bytes, value) in slots.zip(chunk) {
                value.to_bytes(bytes, self.order);
            }
        }
        Ok(())
    }
}

impl AttributeData {
    fn storage(&self) -> StorageType {
        match self {
            AttributeData::U32(_) => StorageType::U32,
            AttributeData::F16(_) => StorageType::F16,
            AttributeData::F32(_) => StorageType::F32,
        }
    }
}

impl WriteBlock for Indices {
    type Args<'a> = StorageType;

    fn write_block(
        &self,
        w: &mut Writer<'_>,
        context: &'static str,
        storage: StorageType,
    ) -> Result<()> {
        match (self, storage) {
            (Indices::U16(values), StorageType::U16) => write_indices(w, values, context),
            (Indices::U32(values), StorageType::U32) => write_indices(w, values, context),
            (indices, expected) => {
                let actual = match indices {
                    Indices::U16(_) => StorageType::U16,
                    Indices::U32(_) => StorageType::U32,
                };
                let kind = ErrorKind::StorageMismatch { expected, actual };
                Err(Error::new(w.position(), context, kind))
            }
        }
    }
}

fn write_indices<T: Number>(w: &mut Writer<'_>, values: &[T], context: &'static str) -> Result<()> {
    let start = w.begin_block(values.len(), T::SIZE, context)?;
    let order = w.order();
    let payload = w.payload_mut(start);
    for (bytes, value) in payload.chunks_exact_mut(T::SIZE).zip(values) {
        value.to_bytes(bytes, order);
    }
    Ok(())
}
