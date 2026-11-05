use std::{
    fs,
    io::{BufReader, Cursor, Read, Seek},
    path::PathBuf,
};

use rdm4lib::rdm::{
    self, RdmEndian, RdmError, RdmExportInfo, RdmFieldLayout, RdmHeader, RdmReadField,
    RdmReadResult, RdmReader,
};
use rdm_derive::RdmRecord;

#[derive(Debug, PartialEq, RdmRecord)]
#[rdm(strides(4))]
struct Leaf {
    value: u32,
}

#[derive(Debug, PartialEq, RdmRecord)]
#[rdm(strides(12))]
struct Root {
    one: Leaf,
    many: Vec<Leaf>,
    optional: Option<String>,
}

fn word(bytes: &mut Vec<u8>, value: u32, endian: RdmEndian) {
    bytes.extend_from_slice(&match endian {
        RdmEndian::Little => value.to_le_bytes(),
        RdmEndian::Big => value.to_be_bytes(),
    });
}

fn patch(bytes: &mut [u8], at: usize, value: u32, endian: RdmEndian) {
    bytes[at..at + 4].copy_from_slice(&match endian {
        RdmEndian::Little => value.to_le_bytes(),
        RdmEndian::Big => value.to_be_bytes(),
    });
}

fn block(bytes: &mut Vec<u8>, count: u32, stride: u32, values: &[u32], endian: RdmEndian) {
    word(bytes, count, endian);
    word(bytes, stride, endian);
    for &value in values {
        word(bytes, value, endian);
    }
}

fn sample(endian: RdmEndian) -> Vec<u8> {
    let mut bytes = RdmHeader { endian }.encode().to_vec();
    block(&mut bytes, 1, 12, &[64, 48, 76], endian);
    // Physical order differs from field order: many, one, then text.
    block(&mut bytes, 2, 4, &[10, 11], endian);
    block(&mut bytes, 1, 4, &[7], endian);
    word(&mut bytes, 3, endian);
    word(&mut bytes, 1, endian);
    bytes.extend_from_slice(b"oak");
    bytes
}

fn read_root<T: rdm::RdmReadRecord>(bytes: &[u8]) -> Result<RdmReadResult<T>, RdmError> {
    RdmReader::new(Cursor::new(bytes))?.read_root::<T>()
}

#[test]
fn follows_references_in_field_order_independent_of_physical_order() {
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        let result = read_root::<Root>(&sample(endian)).unwrap();
        assert_eq!(
            result.value,
            Root {
                one: Leaf { value: 7 },
                many: vec![Leaf { value: 10 }, Leaf { value: 11 }],
                optional: Some("oak".into()),
            }
        );
        assert_eq!(result.layout.endian, endian);
        let origins: Vec<_> = result
            .layout
            .blocks
            .iter()
            .map(|block| {
                (
                    block.source.payload.start,
                    block.reference_offset,
                    block.known_size,
                )
            })
            .collect();
        assert_eq!(
            origins,
            [(28, 16, 12), (48, 32, 4), (64, 28, 4), (76, 36, 1)]
        );
        assert!(result
            .layout
            .blocks
            .iter()
            .all(|block| block.tails.is_empty()));
    }
}

#[test]
fn optional_null_references_do_not_claim_a_block() {
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        let mut bytes = sample(endian);
        patch(&mut bytes, 36, 0, endian);
        bytes.truncate(68);
        let result = read_root::<Root>(&bytes).unwrap();
        assert_eq!(result.value.optional, None);
        assert_eq!(result.layout.blocks.len(), 3);
    }
}

#[test]
fn rejects_required_null_and_invalid_payload_targets_at_the_reference_field() {
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        for target in [0, 29, 60, 65, u32::MAX] {
            let mut bytes = sample(endian);
            patch(&mut bytes, 28, target, endian);
            let error = read_root::<Root>(&bytes).unwrap_err();
            assert_eq!(error.offset(), 28);
            assert_eq!(error.context(), "Root.one");
            assert_eq!(
                error.expected(),
                if target == 0 {
                    "non-null block reference"
                } else {
                    "indexed block payload boundary"
                }
            );
        }
    }
}

#[test]
fn duplicate_ownership_reports_both_reference_origins() {
    let mut bytes = sample(RdmEndian::Little);
    patch(&mut bytes, 32, 64, RdmEndian::Little);
    let error = read_root::<Root>(&bytes).unwrap_err();
    assert_eq!(error.offset(), 32);
    assert_eq!(error.context(), "Root.many");
    assert_eq!(error.expected(), "unique block ownership");
    assert_eq!(
        error.actual(),
        "0x40 already referenced by Root.one at 0x1c"
    );
}

#[derive(Debug, RdmRecord)]
#[rdm(strides(4))]
struct Node {
    children: Option<Vec<Node>>,
}

#[test]
fn rejects_backward_and_cyclic_references_before_recursing() {
    for target in [0, 28, 40] {
        let endian = RdmEndian::Little;
        let mut bytes = RdmHeader { endian }.encode().to_vec();
        block(&mut bytes, 1, 4, &[40], endian);
        block(&mut bytes, 1, 4, &[target], endian);
        if target == 0 {
            let result = read_root::<Node>(&bytes).unwrap();
            assert!(result.value.children.unwrap()[0].children.is_none());
            continue;
        }
        let error = read_root::<Node>(&bytes).unwrap_err();
        assert_eq!(error.offset(), 40);
        assert_eq!(error.context(), "Node.children");
        assert_eq!(error.expected(), "forward block reference");
    }
}

#[test]
fn rejects_wrong_cardinality_and_stride_with_reference_context() {
    let mut bytes = sample(RdmEndian::Little);
    patch(&mut bytes, 28, 48, RdmEndian::Little);
    let error = read_root::<Root>(&bytes).unwrap_err();
    assert_eq!(error.offset(), 40);
    assert_eq!(error.context(), "Root.one");
    assert_eq!(error.expected(), "1 Leaf record");
    assert_eq!(error.actual(), "2");

    let mut bytes = sample(RdmEndian::Little);
    // One three-byte record has the same extent as three byte records.
    patch(&mut bytes, 68, 1, RdmEndian::Little);
    patch(&mut bytes, 72, 3, RdmEndian::Little);
    let error = read_root::<Root>(&bytes).unwrap_err();
    assert_eq!(error.offset(), 72);
    assert_eq!(error.context(), "Root.optional");
    assert_eq!(error.expected(), "string stride 1");
}

#[test]
fn text_errors_point_to_the_invalid_utf8_byte() {
    let mut bytes = sample(RdmEndian::Little);
    bytes[77] = 0xff;
    let error = read_root::<Root>(&bytes).unwrap_err();
    assert_eq!(error.offset(), 77);
    assert_eq!(error.context(), "Root.optional");
    assert_eq!(error.expected(), "UTF-8 string");
}

#[test]
fn complete_reads_reject_unclaimed_blocks_even_when_scalar_values_match_them() {
    let endian = RdmEndian::Little;
    let mut bytes = RdmHeader { endian }.encode().to_vec();
    block(&mut bytes, 1, 4, &[40], endian);
    block(&mut bytes, 1, 4, &[99], endian);
    let error = read_root::<Leaf>(&bytes).unwrap_err();
    assert_eq!(error.offset(), 32);
    assert_eq!(error.context(), "block ownership");
    assert_eq!(error.actual(), "unclaimed block");
}

struct ScaledLeaf(u32);

impl RdmFieldLayout for ScaledLeaf {
    const WIDTH: usize = 4;
}

impl RdmReadField for ScaledLeaf {
    type Args<'a> = &'a u32;

    fn read_field<R: Read + Seek>(
        reader: &mut RdmReader<R>,
        raw: &[u8],
        at: usize,
        context: &'static str,
        factor: Self::Args<'_>,
    ) -> Result<Self, RdmError> {
        let leaf = reader.read_single::<Leaf>(raw, at, context)?;
        Ok(Self(leaf.value * factor))
    }
}

#[derive(RdmRecord)]
#[rdm(strides(8))]
struct ContextRoot {
    factor: u32,
    #[rdm(args(factor))]
    child: Option<ScaledLeaf>,
}

#[test]
fn optional_reference_fields_forward_borrowed_gat_args() {
    for target in [0, 44] {
        let endian = RdmEndian::Little;
        let mut bytes = RdmHeader { endian }.encode().to_vec();
        block(&mut bytes, 1, 8, &[3, target], endian);
        if target != 0 {
            block(&mut bytes, 1, 4, &[7], endian);
        }
        let result = read_root::<ContextRoot>(&bytes).unwrap();
        assert_eq!(result.value.factor, 3);
        assert_eq!(
            result.value.child.map(|child| child.0),
            if target == 0 { None } else { Some(21) }
        );
    }
}

#[derive(RdmRecord)]
#[rdm(strides(8))]
struct PaddedRoot {
    children: Vec<PaddedLeaf>,
}

#[derive(RdmRecord)]
#[rdm(strides(8))]
struct PaddedLeaf {
    value: u32,
}

#[test]
fn preserves_parent_and_child_tails_in_original_byte_order() {
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        let mut bytes = RdmHeader { endian }.encode().to_vec();
        block(&mut bytes, 1, 8, &[44, 0x12345678], endian);
        block(&mut bytes, 2, 8, &[7, 0xaabbccdd, 8, 0x87654321], endian);
        let result = read_root::<PaddedRoot>(&bytes).unwrap();
        assert_eq!(
            result
                .value
                .children
                .iter()
                .map(|child| child.value)
                .collect::<Vec<_>>(),
            [7, 8]
        );
        assert_eq!(result.layout.blocks[0].tails, bytes[32..36]);
        assert_eq!(
            result.layout.blocks[1].tails,
            [&bytes[48..52], &bytes[56..60]].concat()
        );
    }
}

#[derive(Debug, RdmRecord)]
#[rdm(strides(48, 52))]
struct MetadataRoot {
    export_info: Option<RdmExportInfo>,
}

#[test]
fn reads_metadata_only_corpus_files_with_complete_ownership_and_preserved_tails() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rdm/benchmark");
    for name in [
        "00-stadium_fence_lod4.rdm",
        "01-weaving_machine_simple_idle_01.rdm",
    ] {
        let path = base.join(name);
        let bytes = fs::read(&path).unwrap();
        let result = RdmReader::new(BufReader::new(fs::File::open(path).unwrap()))
            .unwrap()
            .read_root::<MetadataRoot>()
            .unwrap();
        assert!(result.value.export_info.unwrap().source_path.is_some());
        for layout in result.layout.blocks {
            let expected: Vec<_> = bytes[layout.source.payload.clone()]
                .chunks_exact(layout.source.stride as usize)
                .flat_map(|record| record[layout.known_size..].iter().copied())
                .collect();
            assert_eq!(layout.tails, expected);
        }
    }
}
