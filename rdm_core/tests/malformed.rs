//! Rejection of malformed input and preservation of unknown bytes.
//!
//! Each test patches a small little-endian fixture. Offsets are found by
//! following the record layouts: the root record starts at 0x1c with
//! export_info, models, materials, skin and animation offsets.

use rdm_core::{Document, Error, ErrorKind};

const ROOT: usize = 0x1c;

fn fixture() -> Vec<u8> {
    std::fs::read("../rdm4lib/rdm/fishery_others_cutout_lod0.rdm").unwrap()
}

fn word(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

fn set_word(bytes: &mut [u8], at: usize, value: usize) {
    bytes[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
}

fn model(bytes: &[u8]) -> usize {
    word(bytes, ROOT + 4)
}

fn first_attribute(bytes: &[u8]) -> usize {
    let vertex_format = word(bytes, model(bytes) + 4);
    word(bytes, vertex_format)
}

fn reject(bytes: &[u8]) -> Error {
    match Document::from_bytes(bytes) {
        Ok(_) => panic!("malformed input was accepted"),
        Err(error) => error,
    }
}

fn assert_mismatch(error: &Error, what: &str, offset: usize, path: &str) {
    match error.kind() {
        ErrorKind::Mismatch { what: actual, .. } => assert_eq!(*actual, what, "{error}"),
        _ => panic!("unexpected error {error}"),
    }
    assert_eq!(error.offset(), offset as u64, "{error}");
    assert_eq!(error.path(), path, "{error}");
}

#[test]
fn rejects_truncated_input() {
    let bytes = fixture();
    let error = reject(&bytes[..bytes.len() - 1]);
    assert!(
        matches!(error.kind(), ErrorKind::Truncated { .. }),
        "{error}"
    );

    let error = reject(&bytes[..0x10]);
    assert_eq!(
        *error.kind(),
        ErrorKind::Truncated {
            end: 0x14,
            len: 0x10
        }
    );
}

#[test]
fn rejects_trailing_bytes() {
    let mut bytes = fixture();
    let len = bytes.len();
    bytes.push(0);
    let error = reject(&bytes);
    assert_mismatch(&error, "end of referenced data", len, "root");
    assert!(error
        .to_string()
        .contains(&format!("expected {:#x}", len + 1)));
}

#[test]
fn rejects_bad_magic_and_byte_order() {
    let mut bytes = fixture();
    bytes[0] = b'X';
    assert_mismatch(&reject(&bytes), "preamble", 0, "preamble");

    let mut bytes = fixture();
    bytes[3] = 2;
    assert_mismatch(
        &reject(&bytes),
        "byte order marker",
        3,
        "preamble.byte_order",
    );
}

#[test]
fn rejects_unexpected_preamble_word() {
    let mut bytes = fixture();
    set_word(&mut bytes, 8, 1);
    assert_mismatch(&reject(&bytes), "preamble", 0, "preamble");
}

#[test]
fn rejects_stride_below_known_record_size() {
    let mut bytes = fixture();
    set_word(&mut bytes, 0x18, 16);
    let error = reject(&bytes);
    assert_mismatch(&error, "stride", 0x18, "root");
    assert!(
        error.to_string().contains("expected >= 20, found 16"),
        "{error}"
    );
}

#[test]
fn rejects_multiple_records_where_one_is_required() {
    let mut bytes = fixture();
    let name = word(&bytes, model(&bytes));
    set_word(&mut bytes, name - 8, 2);
    assert_mismatch(&reject(&bytes), "record count", name - 8, "models[0].name");
}

#[test]
fn rejects_backward_reference() {
    let mut bytes = fixture();
    set_word(&mut bytes, ROOT + 4, ROOT);
    assert_mismatch(&reject(&bytes), "offset", ROOT + 4, "models");
}

#[test]
fn rejects_null_for_required_reference() {
    let mut bytes = fixture();
    let at = model(&bytes) + 20;
    set_word(&mut bytes, at, 0);
    assert_mismatch(&reject(&bytes), "offset", at, "models[0].groups");
}

#[test]
fn rejects_shared_block() {
    let mut bytes = fixture();
    let export_info = word(&bytes, ROOT);
    let first_name = word(&bytes, export_info);
    assert_ne!(first_name, 0);
    set_word(&mut bytes, export_info + 4, first_name);
    let error = reject(&bytes);
    assert_mismatch(
        &error,
        "overlapping or shared block",
        first_name - 8,
        "export_info",
    );
}

#[test]
fn rejects_unreferenced_gap() {
    let mut bytes = fixture();
    let export_info = word(&bytes, ROOT);
    set_word(&mut bytes, ROOT, 0);
    let error = reject(&bytes);
    assert_mismatch(&error, "gap before block", model(&bytes) - 8, "root");
    assert!(
        error
            .to_string()
            .contains(&format!("expected block at {:#x}", export_info - 8)),
        "{error}"
    );
}

#[test]
fn rejects_unknown_enum_code() {
    let mut bytes = fixture();
    let attribute = first_attribute(&bytes);
    set_word(&mut bytes, attribute, 9);
    let error = reject(&bytes);
    assert_mismatch(
        &error,
        "enum code",
        attribute,
        "models[0].vertex_format.attributes[0].semantic",
    );
    assert!(error.to_string().contains("Position=0"), "{error}");
}

#[test]
fn rejects_vertex_stride_that_disagrees_with_format() {
    let mut bytes = fixture();
    let attribute = first_attribute(&bytes);
    let storage_count = word(&bytes, attribute + 12);
    set_word(&mut bytes, attribute + 12, storage_count + 1);
    let vertices = word(&bytes, model(&bytes) + 12);
    let error = reject(&bytes);
    assert_mismatch(&error, "vertex stride", vertices - 8, "models[0].vertices");
}

#[test]
fn rejects_index_stride_that_disagrees_with_format() {
    let mut bytes = fixture();
    let index_format = word(&bytes, model(&bytes) + 8);
    let storage_type = word(&bytes, index_format);
    set_word(
        &mut bytes,
        index_format,
        if storage_type == 3 { 5 } else { 3 },
    );
    let indices = word(&bytes, model(&bytes) + 16);
    assert_mismatch(
        &reject(&bytes),
        "index stride",
        indices - 8,
        "models[0].indices",
    );
}

#[test]
fn preserves_unknown_record_bytes() {
    let original = fixture();
    let mut patched = original.clone();
    // Root tail, the unknown words of the model and a mesh group tail.
    let model = model(&original);
    let groups = word(&original, model + 20);
    patched[ROOT + 20] = 0xab;
    patched[model + 52] = 0xcd;
    patched[groups + 12] = 0xef;

    let document = Document::from_bytes(&patched).unwrap();
    assert_eq!(document.to_bytes().unwrap(), patched);
    let unchanged = Document::from_bytes(&original).unwrap();
    assert_eq!(document.root(), unchanged.root());
    assert_ne!(document, unchanged);
}
