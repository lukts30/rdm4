//! Frozen fixture values and compatibility contracts for the RDM corpus.

use std::{collections::BTreeSet, fs, io::Cursor, path::PathBuf};

use rdm4lib::{
    gltf_export::{self, GltfExportFormat},
    rdm::{
        RdmAnimation, RdmAttributeValues, RdmBlock, RdmDocument, RdmEndian, RdmMaterialEntry,
        RdmReader, RdmSkin, RdmWriter,
    },
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

// Captured from the independent legacy reader before retiring it. Lengths frame
// strings/lists; float bits preserve signed zero and avoid normalization.
fn hash_text(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u32).to_le_bytes());
    hash.update(value.as_bytes());
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
        let document = RdmReader::new(Cursor::new(&bytes))
            .unwrap()
            .read_root::<RdmDocument>()
            .unwrap();
        let decoded_models = document.value.models.as_deref().unwrap_or_default();
        assert_eq!(decoded_models.len(), count, "{path:?}");
        for endian in [RdmEndian::Little, RdmEndian::Big] {
            let output = RdmWriter::new(Cursor::new(Vec::new()), endian)
                .write_root(&document.value)
                .unwrap()
                .into_inner();
            let written = RdmReader::new(Cursor::new(&output))
                .unwrap()
                .read_root::<RdmDocument>()
                .unwrap();
            assert_eq!(written.value, document.value, "{path:?}");
            assert_eq!(written.layout.blocks[0].source.stride, 48);
            assert!(written
                .layout
                .blocks
                .iter()
                .all(|block| block.tails.iter().all(|&byte| byte == 0)));
            if count != 0 {
                let models = written
                    .layout
                    .blocks
                    .iter()
                    .find(|block| block.context == "RdmDocument.models")
                    .unwrap();
                assert_eq!(models.source.stride, 92);
            }
            if endian == RdmEndian::Little && count != 0 {
                let new_models = word(&output, 0x20);
                // Compare the first model's encoded buffers directly with fixture bytes.
                for field in [12, 16] {
                    let original =
                        RdmBlock::decode(&bytes, word(&bytes, models + field) - 8, endian).unwrap();
                    let new =
                        RdmBlock::decode(&output, word(&output, new_models + field) - 8, endian)
                            .unwrap();
                    assert_eq!(new.payload, original.payload, "{path:?}");
                }
            }
        }
        if count == 0 {
            continue;
        }
        let stride = word(&bytes, models - 4);
        assert_eq!(stride, number(&fixture.expected["first_model"]["stride"]));
        let mut vertices = 0;
        let mut indices = 0;
        for (index, decoded) in decoded_models.iter().enumerate() {
            let model = models + index * stride;
            vertices += word(&bytes, word(&bytes, model + 0x0c) - 8);
            indices += word(&bytes, word(&bytes, model + 0x10) - 8);
            let format_at = word(&bytes, model + 4);
            let attributes = word(&bytes, format_at);
            assert_eq!(
                decoded.vertex_format.selector as usize,
                word(&bytes, format_at + 4)
            );
            assert_eq!(
                decoded.vertex_format.attributes.len(),
                word(&bytes, attributes - 8)
            );
            for (i, descriptor) in decoded.vertex_format.attributes.iter().enumerate() {
                let at = attributes + i * 16;
                assert_eq!(
                    [
                        descriptor.semantic.clone() as u32,
                        descriptor.storage_type as u32,
                        descriptor.interpretation.0,
                        descriptor.storage_count
                    ],
                    std::array::from_fn(|j| word(&bytes, at + j * 4) as u32),
                    "{path:?} model {index}"
                );
            }
            let vertex_at = word(&bytes, model + 0x0c);
            let vertex_stride = word(&bytes, vertex_at - 4);
            assert_eq!(decoded.vertices.count, word(&bytes, vertex_at - 8));
            let mut offset = 0;
            for (attribute, values) in decoded
                .vertex_format
                .attributes
                .iter()
                .zip(&decoded.vertices.attributes)
            {
                let actual: Vec<u32> = match values {
                    RdmAttributeValues::U16(v) => v.iter().map(|&v| u32::from(v)).collect(),
                    RdmAttributeValues::U32(v) => v.clone(),
                    RdmAttributeValues::F16(v) => {
                        v.iter().map(|v| u32::from(v.to_bits())).collect()
                    }
                    RdmAttributeValues::F32(v) => v.iter().map(|v| v.to_bits()).collect(),
                };
                let width = attribute.storage_type.byte_size() as usize;
                let size = width * attribute.storage_count as usize;
                let expected: Vec<u32> = (0..decoded.vertices.count)
                    .flat_map(|i| {
                        let at = vertex_at + i * vertex_stride + offset;
                        bytes[at..at + size].chunks_exact(width).map(|raw| {
                            if width == 2 {
                                u32::from(u16::from_le_bytes(raw.try_into().unwrap()))
                            } else {
                                u32::from_le_bytes(raw.try_into().unwrap())
                            }
                        })
                    })
                    .collect();
                assert_eq!(actual, expected, "{path:?} model {index}");
                offset += size;
            }
            let index_at = word(&bytes, model + 0x10);
            let index_stride = word(&bytes, index_at - 4);
            assert_eq!(decoded.indices.values.len(), word(&bytes, index_at - 8));
            assert_eq!(
                decoded.index_format.storage_type as usize,
                word(&bytes, word(&bytes, model + 8))
            );
            for (i, &value) in decoded.indices.values.iter().enumerate() {
                let at = index_at + i * index_stride;
                assert_eq!(
                    value,
                    if index_stride == 2 {
                        u32::from(u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap()))
                    } else {
                        word(&bytes, at) as u32
                    },
                    "{path:?} model {index}"
                );
            }
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
    let model_at = word(&bytes, 0x20);
    let expected = &fixture.expected["first_model"];
    assert_eq!(word(&bytes, model_at - 8), fixture.model_count());
    assert_eq!(word(&bytes, model_at - 4), number(&expected["stride"]));
    assert_eq!(
        word(&bytes, word(&bytes, model_at + 12) - 4),
        number(&expected["vertex_stride"])
    );
    assert_eq!(
        word(&bytes, word(&bytes, model_at + 16) - 4),
        number(&expected["index_stride"])
    );
    let document = RdmReader::new(Cursor::new(&bytes))
        .unwrap()
        .read_root::<RdmDocument>()
        .unwrap();
    let model = &document.value.models.as_ref().unwrap()[0];
    assert_eq!(
        model.vertices.count,
        number(&expected["vertices"]),
        "{path:?}"
    );
    assert_eq!(
        model.indices.values.len(),
        number(&expected["indices"]),
        "{path:?}"
    );
    assert_eq!(
        model.vertex_format.to_string(),
        expected["format"].as_str().unwrap(),
        "{path:?}"
    );
    let groups: Vec<_> = model
        .groups
        .iter()
        .map(|group| [group.first_index, group.index_count, group.material])
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
fn reads_first_model_of_each_multi_model_corpus_mesh() {
    let meshes: Vec<_> = fixtures()
        .into_iter()
        .filter(|fixture| fixture.model_count() > 1)
        .collect();
    assert_eq!(meshes.len(), 4);
    for fixture in meshes {
        assert_first_model(&fixture);
        let first = number(&fixture.expected["first_model"]["vertices"]);
        assert!(first < number(&fixture.expected["total_vertices"]));
    }
}

#[test]
fn animation_records_match_frozen_legacy_values_across_the_corpus() {
    let mut animations = 0;
    for fixture in fixtures()
        .into_iter()
        .filter(|fixture| fixture.expected.get("animation").is_some())
    {
        let path = fixture.path();
        let bytes = fs::read(&path).unwrap();
        let at = word(&bytes, 0x2c);
        let mut reader =
            RdmReader::new(std::io::BufReader::new(fs::File::open(&path).unwrap())).unwrap();
        let decoded = reader.read_block::<RdmAnimation>(at as u32).unwrap();
        assert_eq!(decoded.records.len(), 1, "{path:?}");
        let animation = &decoded.records[0];
        let mut hash = Sha256::new();
        hash_text(&mut hash, &animation.name);
        hash.update(animation.time_max.to_le_bytes());
        hash.update(animation.sampling_rate.to_le_bytes());
        hash.update((animation.tracks.len() as u32).to_le_bytes());
        for track in &animation.tracks {
            hash_text(&mut hash, &track.joint_name);
            hash.update((track.frames.len() as u32).to_le_bytes());
            for frame in &track.frames {
                for value in frame
                    .rotation
                    .into_iter()
                    .chain(frame.translation)
                    .chain([frame.time])
                {
                    hash.update(value.to_bits().to_le_bytes());
                }
            }
        }
        assert_eq!(
            base16ct::lower::encode_string(&hash.finalize()),
            fixture.expected["animation"]["decoded_sha256"]
                .as_str()
                .unwrap(),
            "{path:?}"
        );
        // Exercise nested track lists and strings in both byte orders.
        for endian in [RdmEndian::Little, RdmEndian::Big] {
            let output = RdmWriter::new(Cursor::new(Vec::new()), endian)
                .write_root(animation)
                .unwrap();
            let written = RdmReader::new(output)
                .unwrap()
                .read_root::<RdmAnimation>()
                .unwrap();
            assert_eq!(&written.value, animation, "{path:?}");
        }
        animations += 1;
    }
    assert_eq!(animations, 6);
}

#[test]
fn migrated_materials_and_skins_match_corpus_values() {
    let mut material_files = 0;
    let mut skin_files = 0;
    for fixture in fixtures() {
        let path = fixture.path();
        let bytes = fs::read(&path).unwrap();
        let mut reader =
            RdmReader::new(std::io::BufReader::new(fs::File::open(&path).unwrap())).unwrap();
        let materials = word(&bytes, 0x24);
        if materials != 0 {
            let decoded = reader
                .read_block::<RdmMaterialEntry>(materials as u32)
                .unwrap();
            assert_eq!(
                decoded.records.len(),
                word(&bytes, materials - 8),
                "{path:?}"
            );
            let text = |target: usize| {
                (target != 0).then(|| {
                    std::str::from_utf8(&bytes[target..target + word(&bytes, target - 8)]).unwrap()
                })
            };
            for (index, entry) in decoded.records.iter().enumerate() {
                let at = word(&bytes, materials + index * 28);
                assert_eq!(
                    entry.material.name.as_deref(),
                    text(word(&bytes, at)),
                    "{path:?}"
                );
                assert_eq!(
                    entry.material.texture.as_deref(),
                    text(word(&bytes, at + 4)),
                    "{path:?}"
                );
            }
            material_files += 1;
        }
        let skin = word(&bytes, 0x28);
        if skin != 0 {
            let decoded = reader.read_block::<RdmSkin>(skin as u32).unwrap();
            assert_eq!(decoded.records.len(), 1);
            let joints = &decoded.records[0].joints;
            let mut hash = Sha256::new();
            hash.update((joints.len() as u32).to_le_bytes());
            for joint in joints {
                hash_text(&mut hash, &joint.name);
                for value in joint.translation.into_iter().chain(joint.rotation) {
                    hash.update(value.to_bits().to_le_bytes());
                }
                hash.update(joint.parent.to_le_bytes());
            }
            assert_eq!(
                base16ct::lower::encode_string(&hash.finalize()),
                fixture.expected["skin_sha256"].as_str().unwrap(),
                "{path:?}"
            );
            skin_files += 1;
        }
    }
    assert_eq!((material_files, skin_files), (18, 4));
}

fn assert_exports_all_models(id: usize) {
    let fixture = fixtures().swap_remove(id);
    let output = tempfile::tempdir().unwrap();
    let document = RdmReader::new(std::io::BufReader::new(
        fs::File::open(fixture.path()).unwrap(),
    ))
    .unwrap()
    .read_root::<RdmDocument>()
    .unwrap();
    let scene = document.value.into_static_scene().unwrap();
    let names: Vec<_> = scene.meshes.iter().map(|mesh| mesh.name.clone()).collect();
    let groups: Vec<Vec<_>> = scene
        .meshes
        .iter()
        .map(|mesh| {
            mesh.primitives
                .iter()
                .map(|group| (group.material, group.indices.len()))
                .collect()
        })
        .collect();
    gltf_export::build_scene(
        scene,
        Some(output.path().into()),
        true,
        GltfExportFormat::Glb,
    )
    .unwrap();
    let gltf = gltf::Gltf::open(output.path().join("out.glb")).unwrap();
    assert_eq!(gltf.meshes().count(), fixture.model_count());
    for ((mesh, name), groups) in gltf.meshes().zip(names).zip(groups) {
        assert_eq!(mesh.name(), name.as_deref());
        assert_eq!(mesh.primitives().len(), groups.len());
        for (primitive, (material, count)) in mesh.primitives().zip(groups) {
            assert_eq!(primitive.material().index(), Some(material as usize));
            assert_eq!(primitive.indices().unwrap().count(), count);
        }
    }
    assert_eq!(
        gltf.scenes().next().unwrap().nodes().count(),
        fixture.model_count()
    );
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
fn exports_both_conveyor_models() {
    assert_exports_all_models(6);
}

#[test]
fn exports_both_factory_depot_models() {
    assert_exports_all_models(11);
}

#[test]
fn exports_all_258_mining_models() {
    assert_exports_all_models(15);
}

#[test]
fn exports_all_20_monument_models() {
    assert_exports_all_models(20);
}
