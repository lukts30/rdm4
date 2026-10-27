//! Unchanged round trips over the benchmark suite and the repository fixtures.

use std::{fs, path::PathBuf};

use rdm_core::{Document, Indices};
use sha2::{Digest, Sha256};

fn rdm_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../rdm4lib/rdm")
}

struct Fixture {
    path: String,
    sha256: String,
    groups: Vec<String>,
}

fn benchmark_manifest() -> Vec<Fixture> {
    let text = fs::read_to_string(rdm_dir().join("benchmark/manifest.json")).unwrap();
    let entries: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
    entries
        .iter()
        .map(|entry| Fixture {
            path: entry["path"].as_str().unwrap().to_owned(),
            sha256: entry["sha256"].as_str().unwrap().to_owned(),
            groups: entry["groups"]
                .as_array()
                .unwrap()
                .iter()
                .map(|g| g.as_str().unwrap().to_owned())
                .collect(),
        })
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn assert_round_trip(name: &str, input: &[u8]) -> Document {
    let document = Document::from_bytes(input).unwrap_or_else(|e| panic!("{name}: {e}"));
    let output = document
        .to_bytes()
        .unwrap_or_else(|e| panic!("{name}: {e}"));
    assert!(output == input, "{name}: output differs from input");
    document
}

#[test]
fn round_trips_all_benchmark_fixtures() {
    let manifest = benchmark_manifest();
    assert_eq!(manifest.len(), 26);
    for fixture in &manifest {
        let input = fs::read(rdm_dir().join("benchmark").join(&fixture.path)).unwrap();
        assert_eq!(
            hex(&Sha256::digest(&input)),
            fixture.sha256,
            "{}",
            fixture.path
        );
        assert_round_trip(&fixture.path, &input);
    }
}

#[test]
fn decodes_benchmark_groups() {
    for fixture in benchmark_manifest() {
        let input = fs::read(rdm_dir().join("benchmark").join(&fixture.path)).unwrap();
        let root = Document::from_bytes(&input).unwrap().root;
        let models = root.models.as_deref().unwrap_or_default();
        let name = &fixture.path;

        for group in &fixture.groups {
            match group.as_str() {
                "animation" => {
                    assert!(models.is_empty(), "{name}");
                    let animations = root.animations.as_ref().expect(name);
                    assert!(animations.iter().all(|a| !a.tracks.is_empty()), "{name}");
                }
                "neither-mesh-nor-animation" => {
                    assert!(models.is_empty() && root.animations.is_none(), "{name}");
                }
                "multi-model" => assert!(models.len() > 1, "{name}"),
                "mesh-u16-root48" | "mesh-u16-root52" => {
                    assert_eq!(models.len(), 1, "{name}");
                    assert!(matches!(models[0].indices, Indices::U16(_)), "{name}");
                }
                "mesh-u32" => {
                    assert_eq!(models.len(), 1, "{name}");
                    assert!(matches!(models[0].indices, Indices::U32(_)), "{name}");
                }
                "anchor" => assert!(!models.is_empty() || root.animations.is_some(), "{name}"),
                other => panic!("{name}: unknown group {other}"),
            }
        }
    }
}

#[test]
fn round_trips_repository_fixtures() {
    let mut count = 0;
    for entry in fs::read_dir(rdm_dir()).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rdm") {
            let input = fs::read(&path).unwrap();
            assert_round_trip(&path.display().to_string(), &input);
            count += 1;
        }
    }
    assert_eq!(count, 17);
}

#[test]
fn decodes_vertex_columns_and_mesh_groups_of_every_model() {
    let input = fs::read(rdm_dir().join("benchmark/15-mining_08_slot_01_lod0.rdm")).unwrap();
    let models = Document::from_bytes(&input).unwrap().root.models.unwrap();
    assert_eq!(models.len(), 258);
    for model in &models {
        let vertices = &model.vertices;
        let attributes = &model.vertex_format.attributes;
        assert_eq!(vertices.attributes.len(), attributes.len());
        for (values, attribute) in vertices.attributes.iter().zip(attributes) {
            assert_eq!(values.len(), vertices.count * attribute.components as usize);
        }
        for group in &model.groups {
            assert!((group.first_index + group.index_count) as usize <= model.indices.len());
        }
    }
}
