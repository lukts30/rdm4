//! Decodes an index block using the model's `IndexFormat`.

use super::{ReadField, Reader};
use crate::buffers::{Indices, Vertices};
use crate::records::{IndexFormat, StorageType};
use crate::wire::{ByteOrder, Scalar};
use crate::Error;

impl ReadField for Indices {
    type Args<'a> = (&'a IndexFormat, &'a Vertices);

    fn read_field<E: ByteOrder>(
        reader: &mut Reader<'_, E>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        (format, vertices): Self::Args<'_>,
    ) -> Result<Self, Error> {
        let block = reader.enter_block(raw, at, context)?;
        let storage_type = format.storage_type;
        if !matches!(storage_type, StorageType::U16 | StorageType::U32) {
            return Err(Error::new(
                block.payload - 4,
                context,
                "index storage type U16 or U32",
                format!("{storage_type:?}"),
            ));
        }
        if block.stride != storage_type.byte_width() {
            return Err(Error::new(
                block.payload - 4,
                context,
                format!(
                    "index stride {} for {storage_type:?}",
                    storage_type.byte_width()
                ),
                block.stride,
            ));
        }

        let payload = reader.block_bytes(&block);
        let indices = match storage_type {
            StorageType::U16 => Indices::U16(decode_indices::<E, u16>(payload)),
            _ => Indices::U32(decode_indices::<E, u32>(payload)),
        };

        if let Some(position) = first_out_of_range(&indices, vertices.count) {
            return Err(Error::new(
                block.payload + position * block.stride,
                context,
                format!("index below vertex count {}", vertices.count),
                indices.get(position).unwrap(),
            ));
        }
        Ok(indices)
    }
}

fn decode_indices<E: ByteOrder, T: Scalar>(payload: &[u8]) -> Vec<T> {
    payload.chunks_exact(T::WIDTH).map(T::decode::<E>).collect()
}

fn first_out_of_range(indices: &Indices, vertex_count: usize) -> Option<usize> {
    match indices {
        Indices::U16(values) => values.iter().position(|&i| usize::from(i) >= vertex_count),
        Indices::U32(values) => values.iter().position(|&i| i as usize >= vertex_count),
    }
}
