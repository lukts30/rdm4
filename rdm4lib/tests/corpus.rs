//! Migration baseline for the unmodified reader-comparison corpus.

use std::{collections::BTreeSet, fs, io::Cursor, path::PathBuf};

use binrw::BinReaderExt;
use rdm4lib::{
    gltf_export::{self, GltfExportFormat},
    rdm::{
        RdmAnimation, RdmAttributeValues, RdmBlock, RdmDocument, RdmEndian, RdmMaterialEntry,
        RdmReader, RdmSkin, RdmWriter,
    },
    rdm_anim::RdAnim,
    rdm_data_main::{RdmFile, RdmKindAnim, RdmKindMesh},
    vertex::VertexIdentifier,
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
            let legacy: Vec<VertexIdentifier> = (0..word(&bytes, attributes - 8))
                .map(|i| {
                    Cursor::new(&bytes[attributes + i * 16..])
                        .read_le()
                        .unwrap()
                })
                .collect();
            assert_eq!(
                decoded.vertex_format.attributes, legacy,
                "{path:?} model {index}"
            );
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
        // The compatibility model API intentionally selects only the first model.
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
fn migrated_animation_records_match_legacy_values_across_the_corpus() {
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
        let legacy: RdmFile<RdmKindAnim> = Cursor::new(&bytes).read_le().unwrap();
        let legacy = &legacy.header1.meta_anim;
        assert_eq!(animation.time_max, legacy.time_max, "{path:?}");
        assert_eq!(animation.sampling_rate, legacy.sampling_rate, "{path:?}");
        assert_eq!(animation.tracks.len(), legacy.anims.len(), "{path:?}");
        let name_at = word(&bytes, at);
        assert_eq!(
            animation.name.as_bytes(),
            &bytes[name_at..name_at + word(&bytes, name_at - 8)]
        );
        for (track, legacy) in animation.tracks.iter().zip(legacy.anims.iter()) {
            assert_eq!(track.joint_name, legacy.j_name.as_ascii(), "{path:?}");
            assert_eq!(
                track.frames.len(),
                legacy.j_data.storage.items.len(),
                "{path:?}"
            );
            for (frame, legacy) in track.frames.iter().zip(&legacy.j_data.storage.items) {
                assert_eq!(
                    frame.rotation.map(f32::to_bits),
                    legacy.rotation.map(f32::to_bits)
                );
                assert_eq!(
                    frame.translation.map(f32::to_bits),
                    legacy.translation.map(f32::to_bits)
                );
                assert_eq!(frame.time.to_bits(), legacy.time.to_bits());
            }
        }
        // Exercise nested track lists and strings without switching the file writer yet.
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
            let legacy: RdmFile<RdmKindMesh> = Cursor::new(&bytes).read_le().unwrap();
            let joints = &***legacy.header1.skin.joint;
            assert_eq!(decoded.records[0].joints.len(), joints.len(), "{path:?}");
            for (joint, legacy) in decoded.records[0].joints.iter().zip(joints) {
                assert_eq!(joint.name, legacy.name.as_ascii(), "{path:?}");
                assert_eq!(
                    joint.translation.map(f32::to_bits),
                    legacy.t.map(f32::to_bits)
                );
                assert_eq!(joint.rotation.map(f32::to_bits), legacy.r.map(f32::to_bits));
                assert_eq!(joint.parent, legacy.parent_id, "{path:?}");
            }
            skin_files += 1;
        }
    }
    assert_eq!((material_files, skin_files), (18, 4));
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
            mesh.mesh_info
                .iter()
                .map(|group| (group.material, group.index_count as usize))
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
