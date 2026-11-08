use std::io::{Read, Seek};

use super::{RdmReadField, RdmReadRecord, RdmReader};
use crate::rdm::{RdmBlockInfo, RdmBlockLayout, RdmError, RdmFieldLayout, RdmRecordLayout};

impl<R: Read + Seek> RdmReader<R> {
    pub(in crate::rdm) fn read_payload(
        &mut self,
        raw: &[u8],
        at: usize,
        context: &'static str,
        stride: usize,
    ) -> Result<(RdmBlockInfo, Vec<u8>), RdmError> {
        let index = self.claim_reference(raw, at, context, stride)?;
        let block = self.blocks[index].clone();
        if block.stride as usize != stride {
            return Err(RdmError::new(
                block.offset + 4,
                context,
                format!("buffer stride {stride} from its format"),
                block.stride,
            ));
        }
        let mut bytes = vec![0; block.payload.len()];
        self.read_into(block.payload.start, &mut bytes, context)?;
        Ok((block, bytes))
    }

    fn claim_reference(
        &mut self,
        raw: &[u8],
        at: usize,
        context: &'static str,
        known_size: usize,
    ) -> Result<usize, RdmError> {
        let target = u32::read_field(self, raw, at, context, ())? as usize;
        if target == 0 {
            return Err(RdmError::new(
                at,
                context,
                "non-null block reference",
                "null reference",
            ));
        }
        if target <= at {
            return Err(RdmError::new(
                at,
                context,
                "forward block reference",
                format!("{target:#x} (backward or cyclic reference)"),
            ));
        }
        let index = self.block_index(target, at, context)?;
        if let Some(owner) = &self.layouts[index] {
            return Err(RdmError::new(
                at,
                context,
                "unique block ownership",
                format!(
                    "{target:#x} already referenced by {} at {:#x}",
                    owner.context, owner.reference_offset
                ),
            ));
        }
        self.layouts[index] = Some(RdmBlockLayout {
            source: self.blocks[index].clone(),
            reference_offset: at,
            context,
            known_size,
            tails: Vec::new(),
        });
        Ok(index)
    }

    fn read_records<T: RdmReadRecord>(
        &mut self,
        raw: &[u8],
        at: usize,
        context: &'static str,
        count: Option<u32>,
    ) -> Result<Vec<T>, RdmError> {
        let index = self.claim_reference(raw, at, context, T::SIZE)?;
        let source = &self.blocks[index];
        if let Some(count) = count {
            if source.count != count {
                return Err(RdmError::new(
                    source.offset,
                    context,
                    format!("{count} {} record", T::NAME),
                    source.count,
                ));
            }
        }
        let decoded = self.decode_block::<T>(index, context)?;
        self.layouts[index]
            .as_mut()
            .expect("reference was claimed")
            .tails = decoded.tails;
        Ok(decoded.records)
    }

    /// Follow a required singleton reference. Used by derived reference fields.
    pub fn read_single<T: RdmReadRecord>(
        &mut self,
        raw: &[u8],
        at: usize,
        context: &'static str,
    ) -> Result<T, RdmError> {
        Ok(self
            .read_records::<T>(raw, at, context, Some(1))?
            .pop()
            .expect("singleton count was checked"))
    }
}

impl<T: RdmRecordLayout> RdmFieldLayout for Vec<T> {
    const WIDTH: usize = 4;
}

impl<T: RdmReadRecord> RdmReadField for Vec<T> {
    type Args<'a> = ();

    fn read_field<R: Read + Seek>(
        reader: &mut RdmReader<R>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<Self, RdmError> {
        reader.read_records(raw, at, context, None)
    }
}

impl RdmFieldLayout for String {
    const WIDTH: usize = 4;
}

impl RdmReadField for String {
    type Args<'a> = ();

    fn read_field<R: Read + Seek>(
        reader: &mut RdmReader<R>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<Self, RdmError> {
        let index = reader.claim_reference(raw, at, context, 1)?;
        let source = reader.blocks[index].clone();
        if source.stride != 1 {
            return Err(RdmError::new(
                source.offset + 4,
                context,
                "string stride 1",
                source.stride,
            ));
        }
        let mut bytes = vec![0; source.payload.len()];
        reader.read_into(source.payload.start, &mut bytes, context)?;
        String::from_utf8(bytes).map_err(|error| {
            RdmError::new(
                source.payload.start + error.utf8_error().valid_up_to(),
                context,
                "UTF-8 string",
                error,
            )
        })
    }
}

/// Only reference field types should be wrapped in `Option`.
impl<T: RdmFieldLayout> RdmFieldLayout for Option<T> {
    const WIDTH: usize = 4;
}

impl<T: RdmReadField> RdmReadField for Option<T> {
    type Args<'a> = T::Args<'a>;

    fn read_field<R: Read + Seek>(
        reader: &mut RdmReader<R>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<Self, RdmError> {
        if u32::read_field(reader, raw, at, context, ())? == 0 {
            return Ok(None);
        }
        T::read_field(reader, raw, at, context, args).map(Some)
    }
}
