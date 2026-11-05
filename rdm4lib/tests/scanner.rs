use std::io::{self, Cursor, Read, Seek, SeekFrom};

use rdm4lib::rdm::{RdmEndian, RdmHeader, RdmScanReport, RdmScanner};

fn word(bytes: &mut Vec<u8>, value: u32, endian: RdmEndian) {
    bytes.extend_from_slice(&match endian {
        RdmEndian::Little => value.to_le_bytes(),
        RdmEndian::Big => value.to_be_bytes(),
    });
}

fn sample(endian: RdmEndian) -> Vec<u8> {
    let mut bytes = RdmHeader { endian }.encode().to_vec();
    // Arbitrary record layouts are valid framing, including odd boundaries.
    word(&mut bytes, 1, endian);
    word(&mut bytes, 3, endian);
    bytes.extend_from_slice(&[0xaa; 3]);
    word(&mut bytes, 2, endian);
    word(&mut bytes, 1, endian);
    bytes.extend_from_slice(&[0xbb; 2]);
    bytes
}

fn scan(bytes: &[u8]) -> RdmScanReport {
    RdmScanner::new(Cursor::new(bytes)).scan().unwrap()
}

#[test]
fn unknown_records_are_indexed_in_both_byte_orders() {
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        // The owned report remains usable after the source is dropped.
        let report = scan(&sample(endian));
        assert_eq!(report.endian, Some(endian));
        assert_eq!(report.preamble, Some([20, 0, 4, 28]));
        assert_eq!(report.source_len, 41);
        assert!(report.issues.is_empty());
        assert_eq!(report.unscanned, None);
        assert_eq!(report.blocks.len(), 2);
        assert_eq!(report.blocks[0].payload, 28..31);
        assert_eq!(report.blocks[1].offset, 31);
        assert_eq!(report.blocks[1].payload, 39..41);
        assert_eq!((report.blocks[1].count, report.blocks[1].stride), (2, 1));
    }
}

#[test]
fn unusual_preamble_words_are_reported_without_losing_blocks() {
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        let mut bytes = sample(endian);
        bytes[4..20].fill(0xff);
        let report = scan(&bytes);
        assert_eq!(report.preamble, Some([u32::MAX; 4]));
        assert_eq!(report.blocks.len(), 2);
        assert_eq!(report.unscanned, None);
        assert_eq!(
            report
                .issues
                .iter()
                .map(|issue| issue.offset())
                .collect::<Vec<_>>(),
            [4, 8, 12, 16]
        );
        assert!(RdmHeader::decode(&bytes).is_err());
    }
}

#[test]
fn incomplete_or_unidentifiable_preambles_keep_the_input_unscanned() {
    let bytes = sample(RdmEndian::Little);
    for end in 0..RdmHeader::SIZE {
        let report = scan(&bytes[..end]);
        assert_eq!(report.endian, None);
        assert_eq!(report.preamble, None);
        assert!(report.blocks.is_empty());
        assert_eq!(report.unscanned, Some(0..end));
        assert_eq!(report.issues[0].context(), "preamble");
    }
    for at in [0, 3] {
        let mut bytes = bytes.clone();
        bytes[at] = 0xff;
        let report = scan(&bytes);
        assert!(report.blocks.is_empty());
        assert_eq!(report.endian, None);
        assert_eq!(report.unscanned, Some(0..bytes.len()));
        assert_eq!(report.issues[0].offset(), at);
    }
}

#[test]
fn missing_or_partial_block_headers_preserve_the_valid_prefix() {
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        let bytes = sample(endian);
        for (offset, valid_blocks) in [(20, 0), (31, 1)] {
            for width in 0..8 {
                if offset == 31 && width == 0 {
                    // Ending after a valid block is complete physical framing.
                    continue;
                }
                let end = offset + width;
                let report = scan(&bytes[..end]);
                assert_eq!(report.blocks.len(), valid_blocks);
                assert_eq!(report.unscanned, Some(offset..end));
                assert_eq!(report.issues[0].offset(), offset);
                assert_eq!(report.issues[0].context(), "block header");
            }
        }
        let mut bytes = bytes;
        bytes.extend_from_slice(&[0xff; 3]);
        let report = scan(&bytes);
        assert_eq!(report.blocks.len(), 2);
        assert_eq!(report.unscanned, Some(41..44));
        assert_eq!(report.issues[0].offset(), 41);
    }
}

#[test]
fn invalid_dimensions_and_truncated_payloads_stop_at_the_failed_block() {
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        for (count, stride, context, at) in [
            (0, 1, "block count", 31),
            (1, 0, "block stride", 35),
            (
                u32::MAX,
                u32::MAX,
                if usize::BITS == 32 {
                    "block size"
                } else {
                    "block payload"
                },
                if usize::BITS == 32 { 31 } else { 39 },
            ),
        ] {
            let mut bytes = sample(endian);
            bytes.truncate(31);
            word(&mut bytes, count, endian);
            word(&mut bytes, stride, endian);
            let report = scan(&bytes);
            assert_eq!(report.blocks.len(), 1);
            assert_eq!(report.unscanned, Some(31..39));
            assert_eq!(report.issues[0].context(), context);
            assert_eq!(report.issues[0].offset(), at);
        }
        let bytes = sample(endian);
        let report = scan(&bytes[..40]);
        assert_eq!(report.blocks.len(), 1);
        assert_eq!(report.unscanned, Some(31..40));
        assert_eq!(
            report.issues[0].to_string(),
            "block payload at 0x27: expected at least 41 bytes, found 40"
        );
    }
}

struct Meter<R> {
    source: R,
    bytes_read: usize,
}

impl<R: Read> Read for Meter<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let count = self.source.read(out)?;
        self.bytes_read += count;
        Ok(count)
    }
}

impl<R: Seek> Seek for Meter<R> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.source.seek(position)
    }
}

#[test]
fn scanning_reads_only_headers_and_restarts_from_byte_zero() {
    let mut bytes = sample(RdmEndian::Little);
    word(&mut bytes, 1, RdmEndian::Little);
    word(&mut bytes, 1 << 20, RdmEndian::Little);
    bytes.resize(bytes.len() + (1 << 20), 0xcc);
    let mut cursor = Cursor::new(bytes);
    cursor.set_position(17);
    let mut scanner = RdmScanner::new(Meter {
        source: cursor,
        bytes_read: 0,
    });
    let first = scanner.scan().unwrap();
    let second = scanner.scan().unwrap();
    assert_eq!(first, second);
    assert_eq!(first.blocks.len(), 3);
    assert_eq!(first.unscanned, None);
    assert_eq!(scanner.into_inner().bytes_read, 2 * (20 + 3 * 8));
}

struct FailingSource {
    source: Cursor<Vec<u8>>,
    fail_seek: bool,
}

impl Read for FailingSource {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "injected read failure",
        ))
    }
}

impl Seek for FailingSource {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if self.fail_seek {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected seek failure",
            ))
        } else {
            self.source.seek(position)
        }
    }
}

#[test]
fn io_failures_are_returned_as_errors() {
    for fail_seek in [false, true] {
        let error = RdmScanner::new(FailingSource {
            source: Cursor::new(sample(RdmEndian::Little)),
            fail_seek,
        })
        .scan()
        .unwrap_err();
        assert_eq!(
            error.kind(),
            if fail_seek {
                io::ErrorKind::PermissionDenied
            } else {
                io::ErrorKind::TimedOut
            }
        );
    }
}
