use std::io::{Read, Seek, SeekFrom};

use super::{RdmBlockInfo, RdmEndian, RdmError, RdmReadRecord, RdmScanner};

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

/// Decodes selected typed blocks using the scanner's physical index.
/// Complete reference coverage is not established by reading individual blocks.
#[derive(Debug)]
pub struct RdmReader<R> {
    source: R,
    endian: RdmEndian,
    blocks: Vec<RdmBlockInfo>,
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
            blocks: report.blocks,
        })
    }

    /// Byte order detected by the codec, independent of the decoded records.
    pub fn endian(&self) -> RdmEndian {
        self.endian
    }

    /// Decode all records at an indexed payload boundary. Unknown tails are kept.
    /// The source and index remain available for reading other blocks.
    pub fn read_block<T: RdmReadRecord>(
        &mut self,
        payload_offset: u32,
    ) -> Result<RdmDecodedBlock<T>, RdmError> {
        let target = payload_offset as usize;
        let index = self
            .blocks
            .binary_search_by_key(&target, |block| block.payload.start)
            .map_err(|_| {
                RdmError::new(
                    target,
                    T::NAME,
                    "indexed block payload boundary",
                    format!("{target:#x}"),
                )
            })?;
        let block = self.blocks[index].clone();
        if !T::STRIDES.contains(&block.stride) || T::SIZE > block.stride as usize {
            return Err(RdmError::new(
                block.offset + 4,
                T::NAME,
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
