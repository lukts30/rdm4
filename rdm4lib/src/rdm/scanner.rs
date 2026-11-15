use std::{
    io::{self, Read, Seek, SeekFrom},
    ops::Range,
};

use super::{
    framing::{checked_end, preamble_issues},
    RdmBlock, RdmBlockInfo, RdmEndian, RdmError, RdmHeader,
};

/// Physical framing and diagnostics; no record types or references are inferred.
/// This report owns metadata only and does not retain the source or its payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdmScanReport {
    /// Detected order, absent if the preamble could not be identified.
    pub endian: Option<RdmEndian>,
    /// Decoded words at `0x04`, `0x08`, `0x0c` and `0x10`, when available.
    pub preamble: Option<[u32; 4]>,
    /// Source length measured before scanning.
    pub source_len: usize,
    /// Valid blocks in physical order, without claims about their meaning.
    pub blocks: Vec<RdmBlockInfo>,
    /// Preamble mismatches and the framing failure, if any.
    pub issues: Vec<RdmError>,
    /// Failed block and remaining bytes, or the preamble if it could not be read.
    /// `None` means complete physical framing, even if preamble values are unusual.
    pub unscanned: Option<Range<usize>>,
}

/// Discovers block boundaries by reading headers and seeking over payloads.
#[derive(Debug)]
pub struct RdmScanner<R> {
    source: R,
}

impl<R> RdmScanner<R> {
    /// Wrap a seekable source; use `BufReader<File>` for buffered file access.
    pub fn new(source: R) -> Self {
        Self { source }
    }

    /// Recover the source for subsequent decoding. Its position is unspecified.
    pub fn into_inner(self) -> R {
        self.source
    }
}

impl<R: Read + Seek> RdmScanner<R> {
    /// Scan from byte zero. Format failures retain the valid prefix in the report;
    /// I/O failures return `Err`. Invalid boundaries stop scanning without guessing.
    pub fn scan(&mut self) -> io::Result<RdmScanReport> {
        let source_len = usize::try_from(self.source.seek(SeekFrom::End(0))?).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "RDM source length exceeds usize",
            )
        })?;
        let mut report = RdmScanReport {
            endian: None,
            preamble: None,
            source_len,
            blocks: Vec::new(),
            issues: Vec::new(),
            unscanned: Some(0..source_len),
        };
        if let Err(issue) = checked_end(source_len, 0, RdmHeader::SIZE, "preamble") {
            report.issues.push(issue);
            return Ok(report);
        }
        let raw = self.read_at::<{ RdmHeader::SIZE }>(0)?;
        let header = match RdmHeader::detect(&raw) {
            Ok(header) => header,
            Err(issue) => {
                report.issues.push(issue);
                return Ok(report);
            }
        };
        let words = header.endian.decode(&raw[4..]);
        report.endian = Some(header.endian);
        report.preamble = Some(words);
        report.issues = preamble_issues(words);
        report.unscanned = None;

        let mut offset = RdmHeader::SIZE;
        while offset < source_len || report.blocks.is_empty() {
            let block = match checked_end(source_len, offset, RdmBlock::HEADER_SIZE, "block header")
            {
                Ok(_) => {
                    let raw = self.read_at::<{ RdmBlock::HEADER_SIZE }>(offset)?;
                    RdmBlockInfo::from_header(&raw, offset, source_len, header.endian)
                }
                Err(issue) => Err(issue),
            };
            match block {
                Ok(block) => {
                    offset = block.payload.end;
                    report.blocks.push(block);
                }
                Err(issue) => {
                    report.issues.push(issue);
                    report.unscanned = Some(offset..source_len);
                    break;
                }
            }
        }
        Ok(report)
    }

    fn read_at<const N: usize>(&mut self, offset: usize) -> io::Result<[u8; N]> {
        self.source.seek(SeekFrom::Start(offset as u64))?;
        let mut raw = [0; N];
        self.source.read_exact(&mut raw)?;
        Ok(raw)
    }
}
