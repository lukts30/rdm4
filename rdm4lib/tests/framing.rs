use std::{fs, path::PathBuf};

use rdm4lib::rdm::{Block, Endian, Header};

const LITTLE: [u8; 20] = [
    b'R', b'D', b'M', 1, 20, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 28, 0, 0, 0,
];
const BIG: [u8; 20] = [
    b'R', b'D', b'M', 0, 0, 0, 0, 20, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 28,
];

#[test]
fn preambles_match_known_bytes_in_both_orders() {
    for (endian, bytes) in [(Endian::Little, LITTLE), (Endian::Big, BIG)] {
        let header = Header::decode(&bytes).unwrap();
        assert_eq!(header.endian, endian);
        assert_eq!(header.encode(), bytes);
        for end in 0..Header::SIZE {
            assert!(Header::decode(&bytes[..end]).is_err());
        }
        for at in [0, 3, 4, 8, 12, 16] {
            let mut invalid = bytes;
            invalid[at] = 0xff;
            let error = Header::decode(&invalid).unwrap_err();
            assert_eq!(error.offset(), at);
            assert!(!error.expected().is_empty());
            assert!(!error.actual().is_empty());
        }
    }
}

#[test]
fn block_headers_can_be_unaligned_and_leave_payload_opaque() {
    for (endian, header) in [
        (Endian::Little, [2, 0, 0, 0, 3, 0, 0, 0]),
        (Endian::Big, [0, 0, 0, 2, 0, 0, 0, 3]),
    ] {
        let mut bytes = vec![0xff];
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(&[1, 2, 3, 4, 5, 6]);
        let block = Block::decode(&bytes, 1, endian).unwrap();
        assert_eq!((block.offset, block.count, block.stride), (1, 2, 3));
        assert_eq!(block.payload, &[1, 2, 3, 4, 5, 6]);
        assert_eq!(block.next_offset(), bytes.len());
        for end in 0..bytes.len() {
            assert!(Block::decode(&bytes[..end], 1, endian).is_err());
        }
        assert!(Block::decode(&bytes, usize::MAX, endian).is_err());
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
        let error = Block::decode(&bytes, 0, Endian::Little).unwrap_err();
        assert_eq!(error.context(), context);
        assert_eq!(error.offset(), offset);
        assert!(error.to_string().contains(context));
    }
}

#[test]
fn every_fixture_is_fully_framed_without_interpreting_records() {
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
        let header = Header::decode(&bytes).unwrap();
        assert_eq!(header.encode(), bytes[..Header::SIZE], "{path:?}");
        let root = Block::decode(&bytes, Header::SIZE, header.endian).unwrap();
        assert_eq!(root.count, 1, "{path:?}");
        assert!([48, 52].contains(&root.stride), "{path:?}");
        let mut at = Header::SIZE;
        while at < bytes.len() {
            let block = Block::decode(&bytes, at, header.endian)
                .unwrap_or_else(|error| panic!("{path:?}: {error}"));
            at = block.next_offset();
            blocks += 1;
        }
        assert_eq!(at, bytes.len(), "{path:?}");
    }
    assert_eq!(blocks, 3786);
}
