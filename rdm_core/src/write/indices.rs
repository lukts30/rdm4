//! Encodes an index block using the model's `IndexFormat`.

use super::{WriteField, Writer};
use crate::buffers::{Indices, Vertices};
use crate::records::{IndexFormat, StorageType};
use crate::wire::{ByteOrder, Scalar};
use crate::Error;

impl WriteField for Indices {
    type Args<'a> = (&'a IndexFormat, &'a Vertices);

    fn write_field<E: ByteOrder>(
        &self,
        writer: &mut Writer<'_, E>,
        at: usize,
        context: &'static str,
        (format, _vertices): Self::Args<'_>,
    ) -> Result<(), Error> {
        let payload = match (self, format.storage_type) {
            (Indices::U16(values), StorageType::U16) => encode_indices(writer, values, context)?,
            (Indices::U32(values), StorageType::U32) => encode_indices(writer, values, context)?,
            (indices, storage_type) => {
                let actual = match indices {
                    Indices::U16(_) => "U16 indices",
                    Indices::U32(_) => "U32 indices",
                };
                return Err(Error::new(
                    at,
                    context,
                    format!("{storage_type:?} indices"),
                    actual,
                ));
            }
        };
        writer.put_reference(at, payload);
        Ok(())
    }
}

fn encode_indices<E: ByteOrder, T: Scalar>(
    writer: &mut Writer<'_, E>,
    values: &[T],
    context: &'static str,
) -> Result<usize, Error> {
    let payload = writer.begin_block(values.len(), T::WIDTH, context)?;
    let block = writer.bytes_mut(payload, values.len() * T::WIDTH);
    for (out, value) in block.chunks_exact_mut(T::WIDTH).zip(values) {
        value.encode::<E>(out);
    }
    Ok(payload)
}
