//! Migration baseline for the unmodified reader-comparison corpus.

use std::{collections::BTreeSet, fs, io::Cursor, path::PathBuf};

use binrw::BinReaderExt;
use rdm4lib::{
    gltf_export::{self, GltfExportFormat},
    rdm_anim::RdAnim,
    rdm_data_main::{RdmFile, RdmKindAnim, RdmKindMesh},
    RdModell,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

struct Fixture {
    manifest: Value,
    expected: Value,
}

impl Fixture {
    fn path(&self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("rdm/benchmark")
            .join(self.manifest["path"].as_str().unwrap())
    }

    fn model_count(&self) -> usize {
        number(&self.expected["model_count"])
    }
}

fn number(value: &Value) -> usize {
    usize::try_from(value.as_u64().unwrap()).unwrap()
}

fn fixtures() -> Vec<Fixture> {
    let manifest: Vec<Value> =
        serde_json::from_str(include_str!("../rdm/benchmark/manifest.json")).unwrap();
    let expected: Vec<Value> =
        serde_json::from_str(include_str!("../rdm/benchmark/expectations.json")).unwrap();
    assert_eq!(manifest.len(), 26);
    assert_eq!(expected.len(), manifest.len());
    manifest
        .into_iter()
        .zip(expected)
        .enumerate()
        .map(|(id, (manifest, expected))| {
            assert_eq!(number(&manifest["id"]), id);
            assert_eq!(number(&expected["id"]), id);
            Fixture { manifest, expected }
        })
        .collect()
}

// These fixtures are all LE. Read only the fixed reference/count/stride slots
// independently of the legacy reader, which discards all but the first model.
fn word(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

#[test]
fn corpus_matches_original_hashes_sizes_and_inventory() {
    let fixtures = fixtures();
    let mut hashes = BTreeSet::new();
    let mut total = 0;
    for fixture in &fixtures {
        let path = fixture.path();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(bytes.len(), number(&fixture.manifest["bytes"]), "{path:?}");
        let hash = base16ct::lower::encode_string(&Sha256::digest(&bytes));
        assert_eq!(
            hash,
            fixture.manifest["sha256"].as_str().unwrap(),
            "{path:?}"
        );
        hashes.insert(hash);
        total += bytes.len();
        if fixture.manifest["source"] == "anchor" {
            let anchor = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join(fixture.manifest["source_path"].as_str().unwrap());
            assert_eq!(bytes, fs::read(anchor).unwrap(), "{path:?}");
        }
    }
    assert_eq!(hashes.len(), 26);
    assert_eq!(total, 27_378_902);
    let files = fs::read_dir(fixtures[0].path().parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rdm"))
        .count();
    assert_eq!(files, fixtures.len());
}

#[test]
fn source_headers_account_for_every_model_including_legacy_omissions() {
    for fixture in fixtures() {
        let path = fixture.path();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(&bytes[..4], b"RDM\x01", "{path:?}");
        assert_eq!(word(&bytes, 0x18), number(&fixture.expected["root_stride"]));
        let models = word(&bytes, 0x20);
        let count = if models == 0 {
            0
        } else {
            word(&bytes, models - 8)
        };
        assert_eq!(count, fixture.model_count(), "{path:?}");
        if count == 0 {
            continue;
        }
        let stride = word(&bytes, models - 4);
        assert_eq!(stride, number(&fixture.expected["first_model"]["stride"]));
        let mut vertices = 0;
        let mut indices = 0;
        for index in 0..count {
            let model = models + index * stride;
            vertices += word(&bytes, word(&bytes, model + 0x0c) - 8);
            indices += word(&bytes, word(&bytes, model + 0x10) - 8);
        }
        assert_eq!(
            vertices,
            number(&fixture.expected["total_vertices"]),
            "{path:?}"
        );
        assert_eq!(
            indices,
            number(&fixture.expected["total_indices"]),
            "{path:?}"
        );
    }
}

fn assert_first_model(fixture: &Fixture) {
    let path = fixture.path();
    let bytes = fs::read(&path).unwrap();
    let raw: RdmFile<RdmKindMesh> = Cursor::new(bytes).read_le().unwrap();
    let expected = &fixture.expected["first_model"];
    assert_eq!(raw.header1.meta.info.count as usize, fixture.model_count());
    assert_eq!(
        raw.header1.meta.info.part_size as usize,
        number(&expected["stride"])
    );
    assert_eq!(
        raw.header1.meta.vertex.info.part_size as usize,
        number(&expected["vertex_stride"])
    );
    assert_eq!(
        raw.header1.meta.triangle_list.info.part_size as usize,
        number(&expected["index_stride"])
    );
    let model = RdModell::from(&path);
    assert_eq!(
        model.vertex.len() as usize,
        number(&expected["vertices"]),
        "{path:?}"
    );
    assert_eq!(
        model.triangle_indices.len() * 3,
        number(&expected["indices"]),
        "{path:?}"
    );
    assert_eq!(
        model.vertex.to_string(),
        expected["format"].as_str().unwrap(),
        "{path:?}"
    );
    let groups: Vec<_> = model
        .mesh_info
        .iter()
        .map(|group| {
            [
                group.start_index_location,
                group.index_count,
                group.material,
            ]
        })
        .collect();
    assert_eq!(serde_json::json!(groups), expected["groups"], "{path:?}");
}

#[test]
fn reads_every_single_model_corpus_mesh() {
    let meshes: Vec<_> = fixtures()
        .into_iter()
        .filter(|fixture| fixture.model_count() == 1)
        .collect();
    assert_eq!(meshes.len(), 14);
    for fixture in meshes {
        assert_first_model(&fixture);
    }
}

#[test]
fn characterizes_legacy_multi_model_truncation() {
    let meshes: Vec<_> = fixtures()
        .into_iter()
        .filter(|fixture| fixture.model_count() > 1)
        .collect();
    assert_eq!(meshes.len(), 4);
    for fixture in meshes {
        // Characterization, not the desired behavior: the pending export tests
        // below require the complete geometry rather than this first model.
        assert_first_model(&fixture);
        let first = number(&fixture.expected["first_model"]["vertices"]);
        assert!(first < number(&fixture.expected["total_vertices"]));
    }
}

#[test]
fn reads_every_corpus_animation() {
    let animations: Vec<_> = fixtures()
        .into_iter()
        .filter(|fixture| fixture.expected.get("animation").is_some())
        .collect();
    assert_eq!(animations.len(), 6);
    for fixture in animations {
        let path = fixture.path();
        let animation = RdAnim::from(&path);
        let expected = &fixture.expected["animation"];
        assert_eq!(
            animation.anim_vec.len(),
            number(&expected["tracks"]),
            "{path:?}"
        );
        let frames: usize = animation
            .anim_vec
            .iter()
            .map(|track| track.frames.len())
            .sum();
        assert_eq!(frames, number(&expected["frames"]), "{path:?}");
        assert_eq!(
            animation.time_max as usize,
            number(&expected["time_max"]),
            "{path:?}"
        );
        assert_eq!(
            animation.sampling_rate as usize,
            number(&expected["sampling_rate"]),
            "{path:?}"
        );
    }
}

#[test]
fn characterizes_kind_api_rejection_of_metadata_only_documents() {
    for fixture in fixtures().into_iter().take(2) {
        assert_eq!(fixture.model_count(), 0);
        assert!(fixture.expected.get("animation").is_none());
        let bytes = fs::read(fixture.path()).unwrap();
        let mesh_error = Cursor::new(&bytes)
            .read_le::<RdmFile<RdmKindMesh>>()
            .err()
            .expect("legacy mesh API must reject metadata-only input");
        assert!(mesh_error
            .to_string()
            .contains("the input file is not a rdm mesh!"));
        let anim_error = Cursor::new(&bytes)
            .read_le::<RdmFile<RdmKindAnim>>()
            .err()
            .expect("legacy animation API must reject metadata-only input");
        assert!(anim_error
            .to_string()
            .contains("the input file is not a valid rdm anim!"));
    }
}

fn assert_exports_all_models(id: usize) {
    let fixture = fixtures().swap_remove(id);
    let output = tempfile::tempdir().unwrap();
    gltf_export::build(
        RdModell::from(fixture.path()),
        Some(output.path().into()),
        true,
        GltfExportFormat::Glb,
    );
    let gltf = gltf::Gltf::open(output.path().join("out.glb")).unwrap();
    let mut position_accessors = BTreeSet::new();
    let mut vertices = 0;
    let mut indices = 0;
    for primitive in gltf.meshes().flat_map(|mesh| mesh.primitives()) {
        let positions = primitive.get(&gltf::Semantic::Positions).unwrap();
        if position_accessors.insert(positions.index()) {
            vertices += positions.count();
        }
        indices += primitive.indices().unwrap().count();
    }
    assert_eq!(
        vertices,
        number(&fixture.expected["total_vertices"]),
        "all {} models in {:?} must survive export",
        fixture.model_count(),
        fixture.path()
    );
    assert_eq!(indices, number(&fixture.expected["total_indices"]));
}

#[test]
#[ignore = "legacy reader drops the second model; enable during rdm4ng codec integration"]
fn exports_both_conveyor_models() {
    assert_exports_all_models(6);
}

#[test]
#[ignore = "legacy reader drops the second model; enable during rdm4ng codec integration"]
fn exports_both_factory_depot_models() {
    assert_exports_all_models(11);
}

#[test]
#[ignore = "legacy reader drops 257 models; enable during rdm4ng codec integration"]
fn exports_all_258_mining_models() {
    assert_exports_all_models(15);
}

#[test]
#[ignore = "legacy reader drops 19 models; enable during rdm4ng codec integration"]
fn exports_all_20_monument_models() {
    assert_exports_all_models(20);
}
