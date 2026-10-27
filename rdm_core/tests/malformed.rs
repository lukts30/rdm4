//! Malformed inputs derived from `03-impostor.rdm` (little-endian, 648 bytes).
//!
//! Block layout of the fixture (payload offset, count x stride):
//!
//! ```text
//! 0x01c Root 1x48           0x0e8 Model 1x92         0x1d4 MeshGroup 1x28
//! 0x054 ExportInfo 1x72     0x14c ModelName 1x28     0x1f8 vertices 4x8 (P4h)
//! 0x0a4 string 34x1         0x170 string 8x1         0x220 indices 6x2
//! 0x0ce string 18x1         0x180 VertexFormat 1x24  0x234 MaterialEntry 1x28
//!                           0x1a0 VertexAttribute 1x16  0x258 Material 1x48
//!                           0x1b8 IndexFormat 1x20
//! ```
//!
//! Model references: name 0xe8, vertex_format 0xec, index_format 0xf0,
//! vertices 0xf4, indices 0xf8, groups 0xfc.

use std::{fs, path::PathBuf};

use rdm_core::{Document, Error};

fn impostor() -> Vec<u8> {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rdm4lib/rdm/benchmark/03-impostor.rdm");
    fs::read(path).unwrap()
}

fn patched(at: usize, value: u32) -> Vec<u8> {
    let mut input = impostor();
    input[at..at + 4].copy_from_slice(&value.to_le_bytes());
    input
}

fn rejected(input: &[u8]) -> Error {
    let error = Document::from_bytes(input).expect_err("input must be rejected");
    println!("{error}");
    error
}

fn assert_error(error: Error, context: &str, offset: usize, expected: &str, actual: &str) {
    assert_eq!(error.context(), context, "{error}");
    assert_eq!(error.offset(), offset, "{error}");
    assert!(error.expected().contains(expected), "{error}");
    assert!(error.actual().contains(actual), "{error}");
}

#[test]
fn the_unmodified_fixture_is_accepted() {
    let input = impostor();
    assert_eq!(
        Document::from_bytes(&input).unwrap().to_bytes().unwrap(),
        input
    );
}

#[test]
fn rejects_bad_headers() {
    assert_error(
        rejected(&[]),
        "Header",
        0,
        "at least 0x14 bytes",
        "0x0 bytes",
    );

    let mut input = impostor();
    input[0] = b'X';
    assert_error(rejected(&input), "Header.magic", 0, "\"RDM\"", "58");

    let mut input = impostor();
    input[3] = 2;
    assert_error(rejected(&input), "Header.byte_order", 3, "0x01", "0x02");

    assert_error(
        rejected(&patched(0x08, 1)),
        "Header.preamble",
        0x08,
        "0x0",
        "0x1",
    );
}

#[test]
fn rejects_truncated_and_trailing_bytes() {
    let mut input = impostor();
    input.pop();
    let error = rejected(&input);
    assert_error(
        error,
        "MaterialEntry.material",
        0x250,
        "block ending by 0x287",
        "1 records of 48 bytes",
    );

    let mut input = impostor();
    input.push(0);
    assert_error(
        rejected(&input),
        "Header",
        0x288,
        "end of file",
        "1 trailing bytes",
    );

    let error = rejected(&patched(0x14, u32::MAX));
    assert_error(
        error,
        "Header.root",
        0x14,
        "block ending by",
        "4294967295 records of 48 bytes",
    );
}

#[test]
fn rejects_references_off_the_preorder() {
    let error = rejected(&patched(0x10, 0x20));
    assert_error(
        error,
        "Header.root",
        0x10,
        "next block at 0x1c",
        "0x20 (gap",
    );

    let error = rejected(&patched(0xe8, 0x1c));
    assert_error(
        error,
        "Model.name",
        0xe8,
        "next block at 0x14c",
        "backward or shared",
    );

    // Model.vertex_format points at the ModelName block read just before.
    let error = rejected(&patched(0xec, 0x14c));
    assert_error(
        error,
        "Model.vertex_format",
        0xec,
        "next block at 0x180",
        "backward or shared",
    );

    let error = rejected(&patched(0xec, 0));
    assert_error(
        error,
        "Model.vertex_format",
        0xec,
        "reference to 0x180",
        "null reference",
    );
}

/// Stores the model's children vertices, indices, groups (field order)
/// instead of the observed groups, vertices, indices.
#[test]
fn rejects_unsupported_block_order() {
    let input = impostor();
    let mut reordered = input[..0x1cc].to_vec();
    reordered.extend_from_slice(&input[0x1f0..0x218]); // vertices
    reordered.extend_from_slice(&input[0x218..0x22c]); // indices
    reordered.extend_from_slice(&input[0x1cc..0x1f0]); // groups
    reordered.extend_from_slice(&input[0x22c..]);
    for (at, target) in [(0xf4, 0x1d4u32), (0xf8, 0x1fc), (0xfc, 0x210)] {
        reordered[at..at + 4].copy_from_slice(&target.to_le_bytes());
    }
    assert_eq!(reordered.len(), input.len());

    let error = rejected(&reordered);
    assert_error(
        error,
        "Model.groups",
        0xfc,
        "next block at 0x1d4",
        "0x210 (gap or unsupported block order)",
    );
}

#[test]
fn rejects_unexpected_strides_and_counts() {
    let error = rejected(&patched(0x50, 76));
    assert_error(
        error,
        "Root.export_info",
        0x50,
        "ExportInfo stride [72]",
        "76",
    );

    let error = rejected(&patched(0x1b0, 2));
    assert_error(
        error,
        "Model.index_format",
        0x1b0,
        "1 IndexFormat record",
        "2 records",
    );

    let error = rejected(&patched(0x16c, 2));
    assert_error(error, "ModelName.name", 0x16c, "string stride 1", "2");
}

#[test]
fn rejects_inconsistent_vertex_and_index_formats() {
    // P4h becomes P3h: 6 bytes per vertex instead of the stored 8.
    let error = rejected(&patched(0x1ac, 3));
    assert_error(error, "Model.vertices", 0x1f4, "vertex stride 6", "8");

    let error = rejected(&patched(0x1a4, 9));
    assert_error(
        error,
        "VertexAttribute.storage_type",
        0x1a4,
        "storage type",
        "9",
    );

    let error = rejected(&patched(0x1a0, 8));
    assert_error(
        error,
        "VertexAttribute.semantic",
        0x1a0,
        "vertex semantic",
        "8",
    );

    let error = rejected(&patched(0x1b8, 6));
    assert_error(error, "Model.indices", 0x21c, "U16 or U32", "F16");

    let error = rejected(&patched(0x1b8, 5));
    assert_error(error, "Model.indices", 0x21c, "index stride 4 for U32", "2");
}

#[test]
fn rejects_invalid_values() {
    let mut input = impostor();
    input[0x222..0x224].copy_from_slice(&4u16.to_le_bytes());
    let error = rejected(&input);
    assert_error(
        error,
        "Model.indices",
        0x222,
        "index below vertex count 4",
        "4",
    );

    let mut input = impostor();
    input[0x171] = 0xff;
    assert_error(rejected(&input), "ModelName.name", 0x171, "UTF-8", "0xff");
}
