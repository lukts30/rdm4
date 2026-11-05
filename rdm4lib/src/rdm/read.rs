use std::io::{Read, Seek, SeekFrom};

use super::wire::Scalar;

use super::{
    RdmBlockInfo, RdmBlockLayout, RdmEndian, RdmError, RdmFieldLayout, RdmReadResult,
    RdmRecordLayout, RdmScanner, RdmSourceLayout,
};

mod references;

/// Typed records and the codec metadata needed to retain their original tails.
#[derive(Debug, Clone, PartialEq)]
pub struct RdmDecodedBlock<T> {
    /// Ordinary owned records, without input lifetimes or byte-order state.
    pub records: Vec<T>,
    /// Original block boundaries, count and stride.
    pub source: RdmBlockInfo,
    /// Bytes after the known fields, concatenated in record order, including zeros.
    pub tails: Vec<u8>,
}

/// Reads typed records through the scanner's physical block index.
/// `read_root` requires complete ownership; `read_block` decodes selected blocks.
#[derive(Debug)]
pub struct RdmReader<R> {
    source: R,
    endian: RdmEndian,
    blocks: Vec<RdmBlockInfo>,
    layouts: Vec<Option<RdmBlockLayout>>,
}

impl<R: Read + Seek> RdmReader<R> {
    /// Scan a source and require complete framing and the supported preamble.
    pub fn new(mut source: R) -> Result<Self, RdmError> {
        let report = RdmScanner::new(&mut source)
            .scan()
            .map_err(|error| RdmError::new(0, "source", "readable, seekable input", error))?;
        if let Some(issue) = report.issues.into_iter().next() {
            return Err(issue);
        }
        Ok(Self {
            source,
            endian: report
                .endian
                .expect("successful scan identifies byte order"),
            layouts: vec![None; report.blocks.len()],
            blocks: report.blocks,
        })
    }

    /// Byte order detected by the codec, independent of the decoded records.
    pub fn endian(&self) -> RdmEndian {
        self.endian
    }

    /// Decode a single root and require one known incoming reference per block.
    /// Failure to account for a block rejects the read; use the scanner to inspect it.
    pub fn read_root<T: RdmReadRecord>(mut self) -> Result<RdmReadResult<T>, RdmError> {
        let raw = self.record_bytes::<4>(0x10, "RdmHeader.root")?;
        let value = self.read_single::<T>(&raw, 0x10, "RdmHeader.root")?;
        let mut blocks = Vec::with_capacity(self.blocks.len());
        for (source, layout) in self.blocks.into_iter().zip(self.layouts) {
            let Some(layout) = layout else {
                return Err(RdmError::new(
                    source.offset,
                    "block ownership",
                    "one known incoming reference",
                    "unclaimed block",
                ));
            };
            blocks.push(layout);
        }
        Ok(RdmReadResult {
            value,
            layout: RdmSourceLayout {
                endian: self.endian,
                blocks,
            },
        })
    }

    /// Decode all records at an indexed payload boundary. Unknown tails are kept.
    /// The source and index remain available for reading other blocks.
    pub fn read_block<T: RdmReadRecord>(
        &mut self,
        payload_offset: u32,
    ) -> Result<RdmDecodedBlock<T>, RdmError> {
        let target = payload_offset as usize;
        let index = self.block_index(target, target, T::NAME)?;
        self.decode_block::<T>(index, T::NAME)
    }

    fn block_index(
        &self,
        target: usize,
        at: usize,
        context: &'static str,
    ) -> Result<usize, RdmError> {
        self.blocks
            .binary_search_by_key(&target, |block| block.payload.start)
            .map_err(|_| {
                RdmError::new(
                    at,
                    context,
                    "indexed block payload boundary",
                    format!("{target:#x}"),
                )
            })
    }

    fn decode_block<T: RdmReadRecord>(
        &mut self,
        index: usize,
        context: &'static str,
    ) -> Result<RdmDecodedBlock<T>, RdmError> {
        let block = self.blocks[index].clone();
        if !T::STRIDES.contains(&block.stride) || T::SIZE > block.stride as usize {
            return Err(RdmError::new(
                block.offset + 4,
                context,
                format!("stride {:?} covering {} known bytes", T::STRIDES, T::SIZE),
                block.stride,
            ));
        }
        let mut records = Vec::with_capacity(block.count as usize);
        let mut tails = Vec::new();
        for at in (block.payload.clone()).step_by(block.stride as usize) {
            records.push(T::read(self, at)?);
            let start = tails.len();
            tails.resize(start + block.stride as usize - T::SIZE, 0);
            if tails.len() != start {
                self.read_into(at + T::SIZE, &mut tails[start..], T::NAME)?;
            }
        }
        Ok(RdmDecodedBlock {
            records,
            source: block,
            tails,
        })
    }

    /// Read a fixed record prefix. Intended for `RdmReadRecord` implementations.
    pub fn record_bytes<const N: usize>(
        &mut self,
        at: usize,
        context: &'static str,
    ) -> Result<[u8; N], RdmError> {
        let mut raw = [0; N];
        self.read_into(at, &mut raw, context)?;
        Ok(raw)
    }

    fn read_into(
        &mut self,
        at: usize,
        out: &mut [u8],
        context: &'static str,
    ) -> Result<(), RdmError> {
        self.source
            .seek(SeekFrom::Start(at as u64))
            .and_then(|_| self.source.read_exact(out))
            .map_err(|error| RdmError::new(at, context, "readable record bytes", error))
    }
}

/// Construct a record at a location inside a validated block.
pub trait RdmReadRecord: RdmRecordLayout + Sized {
    /// Decode fields; reference fields may read other indexed blocks.
    fn read<R: Read + Seek>(reader: &mut RdmReader<R>, at: usize) -> Result<Self, RdmError>;
}

/// Decode a field with borrowed sibling context, independently of source storage.
pub trait RdmReadField: RdmFieldLayout + Sized {
    /// Context needed by this field, such as a borrowed vertex format.
    type Args<'a>;

    /// Decode a bounded field slice at its absolute source offset.
    fn read_field<R: Read + Seek>(
        reader: &mut RdmReader<R>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        args: Self::Args<'_>,
    ) -> Result<Self, RdmError>;
}

impl<T: Scalar> RdmReadField for T {
    type Args<'a> = ();

    fn read_field<R: Read + Seek>(
        reader: &mut RdmReader<R>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        _args: (),
    ) -> Result<Self, RdmError> {
        if raw.len() != T::WIDTH {
            return Err(RdmError::new(at, context, T::WIDTH, raw.len()));
        }
        Ok(reader.endian().decode(raw))
    }
}
