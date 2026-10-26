use std::fs;
use std::path::PathBuf;

use rdm_core::{
    AttributeData, ByteOrder, Document, ErrorKind, Indices, StorageType, VertexSemantic,
};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rdm4lib/rdm")
}

fn fixture(name: &str) -> Vec<u8> {
    fs::read(fixture_dir().join(name)).unwrap()
}

fn all_fixtures() -> Vec<(String, Vec<u8>)> {
    let mut fixtures: Vec<_> = fs::read_dir(fixture_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rdm"))
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            (name, fs::read(path).unwrap())
        })
        .collect();
    fixtures.sort();
    fixtures
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn set_word(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

const ROOT: usize = 0x1c;

/// Payload offset of the little-endian fixture's model record.
fn model_offset(bytes: &[u8]) -> usize {
    word(bytes, ROOT + 4) as usize
}

fn error_kind(bytes: &[u8]) -> ErrorKind {
    Document::from_bytes(bytes).unwrap_err().kind
}

#[test]
fn round_trips_every_fixture_byte_identically() {
    let fixtures = all_fixtures();
    assert_eq!(fixtures.len(), 17);
    for (name, bytes) in fixtures {
        let document =
            Document::from_bytes(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert!(
            document.to_bytes().unwrap() == bytes,
            "{name} changed in a round trip"
        );
    }
}

#[test]
fn decoded_models_reference_their_own_vertices() {
    let mut models = 0;
    for (name, bytes) in all_fixtures() {
        let document = Document::from_bytes(&bytes).unwrap();
        let Some(model) = &document.root().model else {
            continue;
        };
        models += 1;
        let descriptors = &model.vertex_format.attributes;
        assert_eq!(model.vertices.attributes.len(), descriptors.len(), "{name}");
        let vertex_count = model.vertices.count as u32;
        assert!(
            model.indices.iter().all(|index| index < vertex_count),
            "{name}"
        );
        let index_count = model.indices.len() as u32;
        for group in &model.groups {
            assert!(
                group.first_index + group.index_count <= index_count,
                "{name}"
            );
        }
    }
    assert_eq!(models, 13);
}

#[test]
fn decodes_big_endian_packed_words_and_indices() {
    let document = Document::from_bytes(&fixture("n_nature_small_stone_02_lod0.rdm")).unwrap();
    assert_eq!(document.byte_order(), ByteOrder::Big);
    let model = document.root().model.as_ref().unwrap();
    assert!(matches!(model.indices, Indices::U16(_)));

    let normal = model
        .vertex_format
        .attributes
        .iter()
        .position(|descriptor| descriptor.semantic == VertexSemantic::Normal)
        .unwrap();
    let AttributeData::U32(normals) = &model.vertices.attributes[normal] else {
        panic!("normals are packed words");
    };
    // rdm4lib maps each packed byte b to 2b/255 - 1 and expects this normal.
    let expected = [0.90588236, 0.42745098, 0.03529412];
    for (byte, expected) in normals[0].to_le_bytes().into_iter().zip(expected) {
        let actual = 2.0 * f32::from(byte) / 255.0 - 1.0;
        assert!((actual - expected).abs() < 1e-6);
    }
}

#[test]
fn decodes_little_endian_values() {
    let document =
        Document::from_bytes(&fixture("eden_line_flagpole_variation_cloth_lod0.rdm")).unwrap();
    assert_eq!(document.byte_order(), ByteOrder::Little);
    let model = document.root().model.as_ref().unwrap();
    assert_eq!(
        model.bounds_min.map(f32::to_bits),
        [0x3c70ef5c, 0x3d073c7a, 0xbe9ba652]
    );
    assert_eq!(
        model.bounds_max.map(f32::to_bits),
        [0x3cf1c51e, 0x3fc6c946, 0x3e9ba652]
    );
    assert_eq!(model.vertex_format.attributes[0].storage, StorageType::F32);

    let animation = Document::from_bytes(&fixture("basalt_crusher_others_idle01.rdm")).unwrap();
    let animation = animation.root().animation.as_ref().unwrap();
    assert!(animation
        .tracks
        .iter()
        .all(|track| !track.frames.is_empty()));
}

#[test]
fn preserves_unknown_bytes_outside_records() {
    let original = fixture("jellyfish_lod0.rdm");
    let document = Document::from_bytes(&original).unwrap();
    let skin_record = word(&original, ROOT + 12) as usize;
    let first_joint = word(&original, skin_record) as usize;
    let model = model_offset(&original);

    let mut edited = original.clone();
    edited[first_joint + 0x30] ^= 0x5a; // Joint bytes after `parent`.
    edited[model + 0x40] ^= 0x5a; // Model words after the bounds.
    let edited_document = Document::from_bytes(&edited).unwrap();

    assert_eq!(edited_document.root(), document.root());
    assert_eq!(edited_document.to_bytes().unwrap(), edited);
}

#[test]
fn writes_in_place_value_edits() {
    let original = fixture("basalt_crusher_others_idle01.rdm");
    let mut document = Document::from_bytes(&original).unwrap();
    let frame = &mut document.root_mut().animation.as_mut().unwrap().tracks[0].frames[0];
    frame.time += 1.0;
    let expected_time = frame.time;

    let written = document.to_bytes().unwrap();
    let changed: Vec<_> = (0..original.len())
        .filter(|&i| original[i] != written[i])
        .collect();
    assert!(!changed.is_empty() && changed.len() <= 4);
    let reread = Document::from_bytes(&written).unwrap();
    let track = &reread.root().animation.as_ref().unwrap().tracks[0];
    assert_eq!(track.frames[0].time, expected_time);
}

#[test]
fn reports_structural_edits_instead_of_misplacing_unknown_bytes() {
    let mut document = Document::from_bytes(&fixture("jellyfish_lod0.rdm")).unwrap();
    let joints = &mut document.root_mut().skin.as_mut().unwrap().joints;
    joints.push(joints[0].clone());
    assert_eq!(
        document.to_bytes().unwrap_err().kind,
        ErrorKind::LayoutChanged
    );
}

#[test]
fn rejects_bad_signatures() {
    let mut bytes = fixture("fishery_others_cutout_lod0.rdm");
    bytes[3] = 2;
    assert!(matches!(error_kind(&bytes), ErrorKind::BadSignature { .. }));
    assert!(matches!(
        error_kind(b"RDX\x01"),
        ErrorKind::BadSignature { .. }
    ));
    assert!(matches!(error_kind(b""), ErrorKind::BadSignature { .. }));
}

#[test]
fn rejects_unexpected_preamble_words() {
    let mut bytes = fixture("fishery_others_cutout_lod0.rdm");
    set_word(&mut bytes, 4, 0x15);
    let error = Document::from_bytes(&bytes).unwrap_err();
    assert_eq!(error.offset, 4);
    assert_eq!(error.context, "FileHeader.word_0x04");
    assert_eq!(
        error.kind,
        ErrorKind::UnexpectedValue {
            expected: 0x14,
            actual: 0x15
        }
    );
}

#[test]
fn rejects_every_truncation() {
    let bytes = fixture("fishery_others_cutout_lod0.rdm");
    for len in 0..bytes.len() {
        assert!(
            Document::from_bytes(&bytes[..len]).is_err(),
            "accepted {len} bytes"
        );
    }
}

#[test]
fn rejects_trailing_bytes() {
    let mut bytes = fixture("fishery_others_cutout_lod0.rdm");
    let len = bytes.len() as u64;
    bytes.push(0);
    let error = Document::from_bytes(&bytes).unwrap_err();
    assert_eq!(error.offset, len);
    assert_eq!(error.kind, ErrorKind::TrailingBytes { len: 1 });
}

#[test]
fn rejects_unsupported_record_stride() {
    let mut bytes = fixture("fishery_others_cutout_lod0.rdm");
    set_word(&mut bytes, ROOT - 4, 56);
    let error = Document::from_bytes(&bytes).unwrap_err();
    assert_eq!(error.offset, 0x14);
    assert_eq!(error.context, "FileHeader.root");
    assert_eq!(
        error.kind,
        ErrorKind::UnsupportedStride {
            expected: &[48, 52],
            actual: 56
        }
    );
}

#[test]
fn rejects_shared_and_backward_references() {
    let mut bytes = fixture("fishery_others_cutout_lod0.rdm");
    let export_info = word(&bytes, ROOT);
    set_word(&mut bytes, ROOT + 4, export_info);
    let error = Document::from_bytes(&bytes).unwrap_err();
    assert_eq!(error.offset, ROOT as u64 + 4);
    assert_eq!(error.context, "Root.model");
    assert!(
        matches!(error.kind, ErrorKind::UnexpectedTarget { expected, actual } if actual < expected)
    );
}

#[test]
fn rejects_null_required_reference() {
    let mut bytes = fixture("fishery_others_cutout_lod0.rdm");
    let model = model_offset(&bytes);
    set_word(&mut bytes, model, 0);
    let error = Document::from_bytes(&bytes).unwrap_err();
    assert_eq!(error.context, "Model.name");
    assert_eq!(error.kind, ErrorKind::NullReference);
}

#[test]
fn rejects_index_stride_that_disagrees_with_format() {
    let mut bytes = fixture("fishery_others_cutout_lod0.rdm");
    let index_format = word(&bytes, model_offset(&bytes) + 8) as usize;
    set_word(&mut bytes, index_format, StorageType::U32.code());
    let error = Document::from_bytes(&bytes).unwrap_err();
    assert_eq!(error.context, "Model.indices");
    assert_eq!(
        error.kind,
        ErrorKind::StrideMismatch {
            expected: 4,
            actual: 2
        }
    );
}

#[test]
fn rejects_unknown_enum_codes() {
    let mut bytes = fixture("fishery_others_cutout_lod0.rdm");
    let vertex_format = word(&bytes, model_offset(&bytes) + 4) as usize;
    let first_descriptor = word(&bytes, vertex_format) as usize;
    set_word(&mut bytes, first_descriptor, 9);
    let error = Document::from_bytes(&bytes).unwrap_err();
    assert_eq!(error.offset, first_descriptor as u64);
    assert_eq!(error.context, "VertexDescriptor.semantic");
    assert!(matches!(
        error.kind,
        ErrorKind::UnknownCode { actual: 9, .. }
    ));
}

#[test]
fn rejects_oversized_block_counts() {
    let mut bytes = fixture("fishery_others_cutout_lod0.rdm");
    let vertices = word(&bytes, model_offset(&bytes) + 0x0c) as usize;
    set_word(&mut bytes, vertices - 8, u32::MAX);
    assert!(matches!(error_kind(&bytes), ErrorKind::OutOfBounds { .. }));
}

/// Any accepted input must reproduce itself, including inputs with
/// corrupted values.
#[test]
fn accepted_corruptions_round_trip_and_none_panic() {
    for name in [
        "fishery_others_cutout_lod0.rdm",
        "basalt_crusher_others_idle01.rdm",
    ] {
        let original = fixture(name);
        let mut accepted = 0;
        for offset in 0..original.len() {
            let mut bytes = original.clone();
            bytes[offset] ^= 0xff;
            if let Ok(document) = Document::from_bytes(&bytes) {
                accepted += 1;
                assert_eq!(document.to_bytes().unwrap(), bytes, "{name} @ {offset:#x}");
            }
        }
        assert!(
            accepted > 0 && accepted < original.len(),
            "{name}: {accepted}"
        );
    }
}
