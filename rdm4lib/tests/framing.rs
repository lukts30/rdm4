use std::{
    fs,
    io::{BufReader, Cursor},
    path::PathBuf,
};

use rdm4lib::rdm::{RdmBlock, RdmDocument, RdmEndian, RdmHeader, RdmReader, RdmScanner, RdmWriter};

const LITTLE: [u8; 20] = [
    b'R', b'D', b'M', 1, 20, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 28, 0, 0, 0,
];
const BIG: [u8; 20] = [
    b'R', b'D', b'M', 0, 0, 0, 0, 20, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 28,
];

#[test]
fn preambles_match_known_bytes_in_both_orders() {
    for (endian, bytes) in [(RdmEndian::Little, LITTLE), (RdmEndian::Big, BIG)] {
        let header = RdmHeader::decode(&bytes).unwrap();
        assert_eq!(header.endian, endian);
        assert_eq!(header.encode(), bytes);
        for end in 0..RdmHeader::SIZE {
            assert!(RdmHeader::decode(&bytes[..end]).is_err());
        }
        for at in [0, 3, 4, 8, 12, 16] {
            let mut invalid = bytes;
            invalid[at] = 0xff;
            let error = RdmHeader::decode(&invalid).unwrap_err();
            assert_eq!(error.offset(), at);
            assert!(!error.expected().is_empty());
            assert!(!error.actual().is_empty());
        }
    }
}

#[test]
fn block_headers_can_be_unaligned_and_leave_payload_opaque() {
    for (endian, header) in [
        (RdmEndian::Little, [2, 0, 0, 0, 3, 0, 0, 0]),
        (RdmEndian::Big, [0, 0, 0, 2, 0, 0, 0, 3]),
    ] {
        let mut bytes = vec![0xff];
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        let block = RdmBlock::decode(&bytes, 1, endian).unwrap();
        assert_eq!((block.offset, block.count, block.stride), (1, 2, 3));
        assert_eq!(block.payload, &[1, 2, 3, 4, 5, 6]);
        assert_eq!(block.next_offset(), bytes.len());
        for end in 0..bytes.len() {
            assert!(RdmBlock::decode(&bytes[..end], 1, endian).is_err());
        }
        assert!(RdmBlock::decode(&bytes, usize::MAX, endian).is_err());
    }
}

#[test]
fn invalid_block_sizes_return_contextual_errors() {
    for (bytes, context, offset) in [
        ([0, 0, 0, 0, 1, 0, 0, 0], "block count", 0),
        ([1, 0, 0, 0, 0, 0, 0, 0], "block stride", 4),
        (
            [0xff; 8],
            if usize::BITS == 32 {
                "block size"
            } else {
                "block payload"
            },
            if usize::BITS == 32 { 0 } else { 8 },
        ),
    ] {
        let error = RdmBlock::decode(&bytes, 0, RdmEndian::Little).unwrap_err();
        assert_eq!(error.context(), context);
        assert_eq!(error.offset(), offset);
        assert!(error.to_string().contains(context));
    }
}

#[test]
fn scans_and_preserves_every_fixture_from_seekable_sources() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rdm");
    let paths: Vec<_> = [base.clone(), base.join("benchmark")]
        .into_iter()
        .flat_map(|directory| fs::read_dir(directory).unwrap())
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rdm"))
        .collect();
    assert_eq!(paths.len(), 43);
    let mut blocks = 0;
    for path in paths {
        let bytes = fs::read(&path).unwrap();
        let report = RdmScanner::new(Cursor::new(&bytes)).scan().unwrap();
        let file_report = RdmScanner::new(BufReader::new(fs::File::open(&path).unwrap()))
            .scan()
            .unwrap();
        assert_eq!(report, file_report, "{path:?}");
        assert!(report.issues.is_empty(), "{path:?}: {:?}", report.issues);
        assert_eq!(report.unscanned, None, "{path:?}");
        assert_eq!(report.source_len, bytes.len());
        let header = RdmHeader::decode(&bytes).unwrap();
        assert_eq!(report.endian, Some(header.endian));
        assert_eq!(header.encode(), bytes[..RdmHeader::SIZE], "{path:?}");
        let root = RdmBlock::decode(&bytes, RdmHeader::SIZE, header.endian).unwrap();
        assert_eq!(root.count, 1, "{path:?}");
        assert!([48, 52].contains(&root.stride), "{path:?}");
        let mut at = RdmHeader::SIZE;
        for info in &report.blocks {
            assert_eq!(info.offset, at, "{path:?}");
            let block = RdmBlock::decode(&bytes, at, header.endian)
                .unwrap_or_else(|error| panic!("{path:?}: {error}"));
            assert_eq!((info.count, info.stride), (block.count, block.stride));
            assert_eq!(&bytes[info.payload.clone()], block.payload);
            at = block.next_offset();
            blocks += 1;
        }
        assert_eq!(at, bytes.len(), "{path:?}");
        let decoded = RdmReader::new(BufReader::new(fs::File::open(&path).unwrap()))
            .unwrap()
            .read_root::<RdmDocument>()
            .unwrap();
        let output = RdmWriter::new(Cursor::new(Vec::new()), header.endian)
            .write_preserving(&decoded.value, decoded.layout)
            .unwrap()
            .into_inner();
        assert_eq!(output, bytes, "{path:?}");
    }
    assert_eq!(blocks, 3786);
}
