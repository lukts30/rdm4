use std::fs;
use std::path::{Path, PathBuf};

use binrw::BinReaderExt;
use rdm4lib::rdm_data_main::{RdmFile, RdmKindMesh};

use rdm_core::records::{AttributeValues, Indices, Model, VertexSemantic};
use rdm_core::{ByteOrder, Document};

fn fixtures() -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fs::read_dir("../rdm4lib/rdm")
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "rdm"))
        .collect();
    paths.sort();
    paths
}

fn open(name: &str) -> Document {
    Document::open(format!("../rdm4lib/rdm/{name}")).unwrap()
}

fn only_model(document: &Document) -> &Model {
    let models = document.root().models.as_ref().unwrap();
    assert_eq!(models.len(), 1);
    &models[0]
}

/// Interleaves decoded columns as little-endian vertices, as rdm4lib stores them.
fn little_endian_vertices(model: &Model) -> Vec<u8> {
    let count = model.vertices.count as usize;
    let mut bytes = Vec::new();
    for vertex in 0..count {
        for values in &model.vertices.attributes {
            match values {
                AttributeValues::U16(v) => {
                    let lanes = v.len() / count;
                    for value in &v[vertex * lanes..(vertex + 1) * lanes] {
                        bytes.extend_from_slice(&value.to_le_bytes());
                    }
                }
                AttributeValues::U32(v) => {
                    let lanes = v.len() / count;
                    for value in &v[vertex * lanes..(vertex + 1) * lanes] {
                        bytes.extend_from_slice(&value.to_le_bytes());
                    }
                }
                AttributeValues::F16(v) => {
                    let lanes = v.len() / count;
                    for value in &v[vertex * lanes..(vertex + 1) * lanes] {
                        bytes.extend_from_slice(&value.0.to_le_bytes());
                    }
                }
                AttributeValues::F32(v) => {
                    let lanes = v.len() / count;
                    for value in &v[vertex * lanes..(vertex + 1) * lanes] {
                        bytes.extend_from_slice(&value.to_le_bytes());
                    }
                }
            }
        }
    }
    bytes
}

fn indices_as_u32(indices: &Indices) -> Vec<u32> {
    match indices {
        Indices::U16(values) => values.iter().map(|&v| v as u32).collect(),
        Indices::U32(values) => values.clone(),
    }
}

fn reference_indices(path: &Path, byte_order: ByteOrder) -> Vec<u32> {
    let bytes = fs::read(path).unwrap();
    let mut reader = std::io::Cursor::new(bytes);
    let rdm: RdmFile<RdmKindMesh> = reader.read_le().unwrap();
    let big_endian = byte_order == ByteOrder::Big;
    rdm.header1.meta.triangle_list(big_endian).collect()
}

#[test]
fn writes_every_fixture_back_byte_identically() {
    let paths = fixtures();
    assert_eq!(paths.len(), 17);
    for path in paths {
        let bytes = fs::read(&path).unwrap();
        let document = Document::from_bytes(&bytes)
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let written = document.to_bytes().unwrap();
        assert!(
            written == bytes,
            "{} differs after round trip",
            path.display()
        );
    }
}

#[test]
fn decodes_meshes_like_rdm4lib_in_both_byte_orders() {
    let mut big_endian = 0;
    for path in fixtures() {
        let document = Document::open(&path).unwrap();
        if document.root().models.is_none() {
            continue;
        }
        big_endian += (document.byte_order() == ByteOrder::Big) as usize;
        let model = only_model(&document);
        let reference = rdm4lib::RdModell::from(path.as_path());

        assert_eq!(model.vertices.count, reference.vertex.len());
        assert_eq!(
            model.vertex_format.attributes.len(),
            reference.vertex.identifiers.len()
        );
        assert!(
            little_endian_vertices(model) == reference.vertex.as_bytes(),
            "{} vertices differ",
            path.display()
        );
        let expected = reference_indices(&path, document.byte_order());
        assert_eq!(
            indices_as_u32(&model.indices),
            expected,
            "{}",
            path.display()
        );
        assert_eq!(model.groups.len(), reference.mesh_info.len());
    }
    assert_eq!(big_endian, 1);
}

#[test]
fn decodes_packed_big_endian_normal_as_one_word() {
    let document = open("n_nature_small_stone_02_lod0.rdm");
    assert_eq!(document.byte_order(), ByteOrder::Big);
    let model = only_model(&document);
    let attributes = &model.vertex_format.attributes;
    let normal = attributes
        .iter()
        .position(|a| a.semantic == VertexSemantic::Normal)
        .unwrap();
    let AttributeValues::U32(normals) = &model.vertices.attributes[normal] else {
        panic!("normals are packed words");
    };
    // rdm4lib decodes these as 0.90588236, 0.42745098, 0.03529412 (2 * b / 255 - 1).
    assert_eq!(normals[0].to_le_bytes()[..3], [243, 182, 132]);
}

#[test]
fn decodes_animation_tracks() {
    let document = open("basalt_crusher_others_idle01.rdm");
    let root = document.root();
    assert!(root.models.is_none());
    let animation = root.animation.as_ref().unwrap();
    assert_eq!(animation.name, b"basalt_crusher_others_lod2");
    assert!(!animation.tracks.is_empty());
    for track in &animation.tracks {
        assert!(!track.name.is_empty());
        assert!(track
            .frames
            .iter()
            .all(|frame| frame.time <= animation.time_max as f32));
    }
}
