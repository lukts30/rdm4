//! Check conversion compatibility and the inspection command.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("rdm4lib/rdm")
        .join(name)
}

fn legacy() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rdm4-bin"))
}
fn ng() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rdm4ng"))
}

fn succeeds(command: &mut Command) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn same_gltf_data(actual: &Path, expected: &Path) {
    let read = |path: &Path| {
        let bytes = fs::read(path).unwrap();
        let json = if bytes.starts_with(b"glTF") {
            gltf::Glb::from_slice(&bytes).unwrap().json.into_owned()
        } else {
            bytes
        };
        let mut root: serde_json::Value = serde_json::from_slice(&json).unwrap();
        // The scene exporter additionally retains source model/node names.
        for section in ["meshes", "nodes"] {
            for item in root[section].as_array_mut().unwrap() {
                item.as_object_mut().unwrap().remove("name");
            }
        }
        let (_, buffers, _) = gltf::import(path).unwrap();
        (
            root,
            buffers
                .into_iter()
                .map(|buffer| buffer.0)
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(read(actual), read(expected));
}

#[test]
fn help_version_and_argument_errors() {
    let help = ng().arg("--help").output().unwrap();
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    for name in [
        "rdm4ng", "gltf", "rdm", "inspect", "split", "rdm2gltf", "gltf2rdm",
    ] {
        assert!(text.contains(name), "{text}");
    }
    let version = ng().arg("--version").output().unwrap();
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8(version.stdout).unwrap().trim(),
        concat!("rdm4ng ", env!("CARGO_PKG_VERSION"))
    );
    let input = fixture("gltf/stormtrooper.gltf");
    for invalid in [
        vec![],
        vec!["rdm"],
        vec!["inspect"],
        vec!["split"],
        vec!["split", input.to_str().unwrap()],
        vec!["inspect", input.to_str().unwrap(), "--format", "invalid"],
        vec!["rdm", input.to_str().unwrap()],
        vec!["rdm", input.to_str().unwrap(), "--format", "invalid"],
        vec![
            "rdm",
            input.to_str().unwrap(),
            "--format",
            "P4h_N4b_G4b_B4b_T2h",
            "--animation",
        ],
        vec![
            "rdm",
            input.to_str().unwrap(),
            "--format",
            "P4h_N4b_G4b_B4b_T2h",
            "--skeleton",
            "--mirror-x",
        ],
    ] {
        let output = ng().args(&invalid).output().unwrap();
        assert_eq!(output.status.code(), Some(2), "{invalid:?}");
    }
}

#[test]
fn split_preserves_model_records_and_shared_data() {
    use rdm4lib::rdm::{RdmDocument, RdmReader};
    use std::io::Cursor;

    let output = tempfile::tempdir().unwrap();
    for (case, name) in [
        "fishery_others_lod2.rdm",
        "excavator_tycoons_lod1.rdm",
        "basalt_crusher_others_lod0.rdm",
        "benchmark/06-heavy_04_conveyor_lod2.rdm",
        "benchmark/11-dropgoods_factory_depot_mines_static_lod2.rdm",
        "benchmark/15-mining_08_slot_01_lod0.rdm",
        "benchmark/20-monument_01_04_var3_lod0.rdm",
    ]
    .iter()
    .enumerate()
    {
        let input = fixture(name);
        let bytes = fs::read(&input).unwrap();
        let expected = RdmReader::new(Cursor::new(&bytes))
            .unwrap()
            .read_root::<RdmDocument>()
            .unwrap();
        let models = expected.value.models.as_ref().unwrap();
        let directory = output.path().join(case.to_string());
        let result = ng()
            .arg("split")
            .arg(&input)
            .arg("-o")
            .arg(&directory)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let paths: Vec<_> = String::from_utf8(result.stdout)
            .unwrap()
            .lines()
            .map(PathBuf::from)
            .collect();
        assert_eq!(paths.len(), models.len());
        for (index, path) in paths.iter().enumerate() {
            let split_bytes = fs::read(path).unwrap();
            let actual = RdmReader::new(Cursor::new(&split_bytes))
                .unwrap()
                .read_root::<RdmDocument>()
                .unwrap();
            assert_eq!(
                actual.value.models.as_deref(),
                Some(&models[index..index + 1]),
                "{name} model {index}"
            );
            assert_eq!(actual.value.export_info, expected.value.export_info);
            assert_eq!(actual.value.materials, expected.value.materials);
            assert_eq!(actual.value.skin, expected.value.skin);
            assert_eq!(actual.value.animations, expected.value.animations);
            assert_eq!(actual.layout.endian, expected.layout.endian);
            let original = expected
                .layout
                .blocks
                .iter()
                .find(|b| b.reference_offset == 0x20)
                .unwrap();
            let written = actual
                .layout
                .blocks
                .iter()
                .find(|b| b.reference_offset == 0x20)
                .unwrap();
            let tail = original.source.stride as usize - original.known_size;
            assert_eq!(
                written.tails,
                original.tails[index * tail..(index + 1) * tail]
            );
            if models.len() == 1 {
                assert_eq!(split_bytes, bytes);
            }
        }
        if case == 3 {
            let first = fs::read(&paths[0]).unwrap();
            let second = fs::read(&paths[1]).unwrap();
            fs::remove_file(&paths[0]).unwrap();
            let collision = ng()
                .arg("split")
                .arg(&input)
                .arg("-o")
                .arg(&directory)
                .output()
                .unwrap();
            assert_eq!(collision.status.code(), Some(1));
            assert!(!paths[0].exists());
            assert_eq!(fs::read(&paths[1]).unwrap(), second);
            succeeds(
                ng().arg("split")
                    .arg(&input)
                    .arg("-o")
                    .arg(&directory)
                    .arg("--force"),
            );
            assert_eq!(fs::read(&paths[0]).unwrap(), first);
        }
        assert_eq!(fs::read(input).unwrap(), bytes);
    }
    let directory = output.path().join("no models");
    let result = ng()
        .arg("split")
        .arg(fixture("basalt_crusher_others_idle01.rdm"))
        .arg("-o")
        .arg(&directory)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(!directory.exists());
}

#[test]
fn inspect_reports_unknown_blocks_and_retains_partial_results() {
    use rdm4lib::rdm::{RdmEndian, RdmHeader};

    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("unknown records.rdm");
    for endian in [RdmEndian::Little, RdmEndian::Big] {
        let mut bytes = RdmHeader { endian }.encode().to_vec();
        for value in [1u32, 3] {
            bytes.extend_from_slice(&match endian {
                RdmEndian::Little => value.to_le_bytes(),
                RdmEndian::Big => value.to_be_bytes(),
            });
        }
        bytes.extend_from_slice(&[0xaa, 0xbb, 0xcc]);
        for case in ["valid", "preamble", "truncated", "magic"] {
            let mut data = bytes.clone();
            match case {
                "preamble" => data[4..8].fill(0xff),
                "truncated" => data.push(0xff),
                "magic" => data[0] = 0,
                _ => {}
            }
            fs::write(&input, data).unwrap();
            let output = ng().arg("inspect").arg(&input).output().unwrap();
            assert_eq!(output.status.code(), Some(i32::from(case != "valid")));
            assert!(output.stderr.is_empty(), "{output:?}");
            let text = String::from_utf8(output.stdout).unwrap();
            let expected = match case {
                "preamble" => "preamble word at 0x4",
                "truncated" => "Unscanned: 0x1f..0x20",
                "magic" => "magic at 0x0",
                _ => "Issues: 0",
            };
            assert!(text.contains(expected), "{text}");
            if case != "magic" {
                let order = if endian == RdmEndian::Little {
                    "little"
                } else {
                    "big"
                };
                assert!(text.contains(&format!("Byte order: {order}")), "{text}");
                assert!(text.contains("Blocks: 1"), "{text}");
                assert!(text.contains("0x0000001c..0x0000001f"), "{text}");
            }
            let output = ng()
                .arg("inspect")
                .arg(&input)
                .args(["--format", "json"])
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(i32::from(case != "valid")));
            assert!(output.stderr.is_empty(), "{output:?}");
            let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(json["schema_version"], 1);
            assert_eq!(json["file"], input.to_str().unwrap());
            assert_eq!(
                json["source_len"],
                if case == "truncated" { 32 } else { 31 }
            );
            assert_eq!(
                json["framing_complete"],
                matches!(case, "valid" | "preamble")
            );
            assert_eq!(
                json["issues"].as_array().unwrap().is_empty(),
                case == "valid"
            );
            if case == "magic" {
                assert!(json["endian"].is_null());
                assert!(json["preamble"].is_null());
                assert!(json["blocks"].as_array().unwrap().is_empty());
            } else {
                assert_eq!(
                    json["endian"],
                    if endian == RdmEndian::Little {
                        "little"
                    } else {
                        "big"
                    }
                );
                assert_eq!(
                    json["blocks"][0],
                    serde_json::json!({
                        "offset": 20, "count": 1, "stride": 3,
                        "payload": { "start": 28, "end": 31 }
                    })
                );
            }
            if case == "truncated" {
                assert_eq!(
                    json["unscanned"],
                    serde_json::json!({ "start": 31, "end": 32 })
                );
                assert_eq!(json["issues"][0]["offset"], 31);
                assert_eq!(json["issues"][0]["context"], "block header");
                assert!(json["issues"][0]["expected"].is_string());
                assert!(json["issues"][0]["actual"].is_string());
            } else if case != "magic" {
                assert!(json["unscanned"].is_null());
            }
        }
    }
    succeeds(ng().arg("inspect").arg(fixture("fishery_others_lod2.rdm")));
    for (name, models) in [
        ("fishery_others_lod2.rdm", 1),
        ("benchmark/15-mining_08_slot_01_lod0.rdm", 258),
        ("excavator_tycoons_lod1.rdm", 1),
        ("basalt_crusher_others_idle01.rdm", 0),
    ] {
        let output = ng()
            .arg("inspect")
            .arg(fixture(name))
            .args(["--records", "--format", "json"])
            .output()
            .unwrap();
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(output.status.success(), "{name}: {report}");
        assert_eq!(
            report["records"]["models"].as_array().unwrap().len(),
            models
        );
        assert!(report["issues"].as_array().unwrap().is_empty());
    }
    let output = ng()
        .arg("inspect")
        .arg(fixture("fishery_others_lod2.rdm"))
        .arg("--records")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("Models: 1"));
}

#[test]
fn gltf_command_and_alias_match_legacy_output() {
    let output = tempfile::Builder::new()
        .prefix("rdm4ng cli ")
        .tempdir()
        .unwrap();
    let input = output.path().join("mesh with spaces.rdm");
    fs::copy(fixture("fishery_others_lod2.rdm"), &input).unwrap();
    let expected = output.path().join("legacy.glb");
    succeeds(legacy().arg("-i").arg(&input).arg("-o").arg(&expected));
    for command in ["gltf", "rdm2gltf"] {
        let directory = output.path().join(command);
        fs::create_dir(&directory).unwrap();
        succeeds(
            ng().arg(command)
                .arg(&input)
                .arg("-v")
                .arg("-n")
                .arg("-o")
                .arg(&directory),
        );
        same_gltf_data(&directory.join("mesh with spaces.glb"), &expected);
    }
    let legacy_directory = output.path().join("legacy separate");
    let ng_directory = output.path().join("ng separate");
    fs::create_dir(&legacy_directory).unwrap();
    fs::create_dir(&ng_directory).unwrap();
    let expected = legacy_directory.join("mesh.gltf");
    succeeds(
        legacy()
            .arg("-i")
            .arg(&input)
            .arg("-o")
            .arg(&expected)
            .args(["--gltf-export-format", "gltf"]),
    );
    let actual = ng_directory.join("mesh.gltf");
    succeeds(
        ng().arg("gltf")
            .arg(&input)
            .arg("-o")
            .arg(&actual)
            .args(["--format", "gltf"]),
    );
    same_gltf_data(&actual, &expected);
    for name in [
        "excavator_tycoons_lod1.rdm",
        "destruction_stone_building_04_decal_detail01_lod0.rdm",
    ] {
        let expected = legacy_directory.join("variant.glb");
        let actual = ng_directory.join("variant.glb");
        succeeds(
            legacy()
                .arg("-i")
                .arg(fixture(name))
                .arg("-o")
                .arg(&expected)
                .arg("--force"),
        );
        succeeds(
            ng().arg("gltf")
                .arg(fixture(name))
                // Selected export keeps the legacy attribute coverage.
                .args(["--mesh-index", "0"])
                .arg("-o")
                .arg(&actual)
                .arg("--force"),
        );
        same_gltf_data(&actual, &expected);
    }
    let input = fixture("benchmark/06-heavy_04_conveyor_lod2.rdm");
    let actual = ng_directory.join("selected.glb");
    let all = ng_directory.join("all.glb");
    succeeds(ng().arg("gltf").arg(&input).arg("-o").arg(&all));
    let exported = gltf::Gltf::open(all).unwrap();
    assert_eq!(exported.meshes().count(), 2);
    assert_eq!(exported.scenes().next().unwrap().nodes().count(), 2);
    succeeds(
        ng().arg("gltf")
            .arg(&input)
            .args(["--mesh-index", "1"])
            .arg("-o")
            .arg(&actual),
    );
    let bytes = fs::read(&input).unwrap();
    let word = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let models = word(0x20);
    let second = models + word(models - 4);
    let (document, _, _) = gltf::import(actual).unwrap();
    let primitives: Vec<_> = document.meshes().next().unwrap().primitives().collect();
    assert_eq!(
        primitives[0]
            .get(&gltf::Semantic::Positions)
            .unwrap()
            .count(),
        word(word(second + 12) - 8)
    );
    assert_eq!(
        primitives
            .iter()
            .map(|p| p.indices().unwrap().count())
            .sum::<usize>(),
        word(word(second + 16) - 8)
    );
}

fn make_glb(directory: &Path) -> PathBuf {
    let glb = directory.join("mesh.glb");
    succeeds(
        legacy()
            .arg("-i")
            .arg(fixture("fishery_others_lod2.rdm"))
            .arg("-o")
            .arg(&glb),
    );
    glb
}

#[test]
fn rdm_commands_keep_payloads_and_import_active_scene_models() {
    let output = tempfile::tempdir().unwrap();
    let input = make_glb(output.path());
    use rdm4lib::rdm::{RdmDocument, RdmReader};
    let read = |path: &Path| {
        RdmReader::new(std::io::Cursor::new(fs::read(path).unwrap()))
            .unwrap()
            .read_root::<RdmDocument>()
            .unwrap()
            .value
    };
    let expected = output.path().join("legacy.rdm");
    succeeds(
        legacy()
            .arg("--gltf=P4h_N4b_G4b_B4b_T2h")
            .arg("-i")
            .arg(&input)
            .arg("-o")
            .arg(&expected),
    );
    for command in ["rdm", "gltf2rdm"] {
        let directory = output.path().join(command);
        fs::create_dir(&directory).unwrap();
        succeeds(
            ng().arg(command)
                .arg(&input)
                .args([
                    "--format",
                    "P4h_N4b_G4b_B4b_T2h",
                    "--mesh-index",
                    "0",
                    "--joint-names",
                    "UniqueName",
                ])
                .arg("-n")
                .arg("-o")
                .arg(&directory),
        );
        let actual = read(&directory.join("mesh.rdm"));
        let mut original = read(&expected);
        assert!(actual.export_info.as_ref().unwrap().source_path.is_none());
        assert_eq!(
            actual.export_info.as_ref().unwrap().rmp_name,
            original.export_info.as_ref().unwrap().rmp_name
        );
        assert_eq!(
            actual.materials.as_ref().unwrap().len(),
            original.materials.as_ref().unwrap().len()
        );
        assert!(actual.materials.as_ref().unwrap().iter().all(|entry| entry
            .material
            .name
            .is_none()
            && entry.material.texture.is_none()));
        for (original, current) in original
            .models
            .as_mut()
            .unwrap()
            .iter_mut()
            .zip(actual.models.as_ref().unwrap())
        {
            original.name = current.name.clone();
        }
        original.export_info = actual.export_info.clone();
        original.materials = actual.materials.clone();
        assert_eq!(
            actual, original,
            "all fields except intentional metadata changes"
        );
        let expected_bytes = fs::read(directory.join("mesh.rdm")).unwrap();
        let destination = directory.join("mesh.rdm");
        let mut retry = ng();
        retry
            .arg(command)
            .arg(&input)
            .args(["--format", "P4h_N4b_G4b_B4b_T2h"])
            .arg("-o")
            .arg(&destination);
        let collision = retry.output().unwrap();
        assert_eq!(collision.status.code(), Some(1));
        let error = String::from_utf8(collision.stderr).unwrap();
        assert!(
            error.contains(&format!("rdm: {}:", destination.display())),
            "{error}"
        );
        assert!(!error.contains("panicked"), "{error}");
        assert_eq!(fs::read(&destination).unwrap(), expected_bytes);
        succeeds(retry.arg("--force"));
        assert_eq!(fs::read(&destination).unwrap(), expected_bytes);
    }
    // Only active-scene instances are imported; repeated mesh definitions stay separate models.
    let bytes = fs::read(&input).unwrap();
    let glb = gltf::Glb::from_slice(&bytes).unwrap();
    let mut root: serde_json::Value = serde_json::from_slice(&glb.json).unwrap();
    root["buffers"][0]["uri"] = "multi.bin".into();
    fs::write(output.path().join("multi.bin"), glb.bin.unwrap()).unwrap();
    root["materials"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({}));
    let mut mesh = root["meshes"][0].clone();
    mesh["primitives"][0]["material"] = 2.into();
    mesh["primitives"][1]["material"] = 0.into();
    root["meshes"] = serde_json::json!([mesh.clone(), mesh.clone(), mesh]);
    root["nodes"] = serde_json::json!([
        {"name": "outside", "mesh": 0, "translation": [90, 0, 0]},
        {"name": "parent", "translation": [1, 2, 3], "children": [2, 3]},
        {"name": "first", "mesh": 1},
        {"name": "instance", "mesh": 1, "translation": [4, 0, 0]},
        {"name": "unreferenced", "mesh": 2}
    ]);
    root["scenes"] = serde_json::json!([{"nodes": [0]}, {"nodes": [1]}]);
    root["scene"] = 1.into();
    let input = output.path().join("multi.gltf");
    fs::write(&input, serde_json::to_vec(&root).unwrap()).unwrap();
    let destination = output.path().join("all.rdm");
    succeeds(
        ng().arg("rdm")
            .arg(&input)
            .args(["--format", "P4h_N4b_G4b_B4b_T2h"])
            .arg("-o")
            .arg(&destination),
    );
    let document = read(&destination);
    let models = document.models.unwrap();
    assert_eq!(models.len(), 2);
    assert_eq!(
        models
            .iter()
            .map(|model| model.name.name.as_deref())
            .collect::<Vec<_>>(),
        [Some("first"), Some("instance")]
    );
    // Baking into half precision can round the two translated extrema differently.
    assert!((models[1].bounds_min[0] - models[0].bounds_min[0] - 4.0).abs() < 0.01);
    assert!((models[1].bounds_max[0] - models[0].bounds_max[0] - 4.0).abs() < 0.01);
    for model in &models {
        assert_eq!(
            model
                .groups
                .iter()
                .map(|group| group.material)
                .collect::<Vec<_>>(),
            [2, 0]
        );
    }
    succeeds(
        ng().arg("rdm")
            .arg(&input)
            .args([
                "--format",
                "P4h_N4b_G4b_B4b_T2h",
                "--material-indices",
                "4",
                "3",
                "2",
                "1",
                "--force",
            ])
            .arg("-o")
            .arg(&destination),
    );
    assert_eq!(
        read(&destination)
            .models
            .unwrap()
            .iter()
            .flat_map(|model| model.groups.iter().map(|group| group.material))
            .collect::<Vec<_>>(),
        [4, 3, 2, 1]
    );
    succeeds(
        legacy()
            .arg("--gltf=P4h_N4b_G4b_B4b_T2h")
            .arg("-i")
            .arg(&input)
            .arg("-o")
            .arg(output.path().join("first.rdm")),
    );
    assert_eq!(
        read(&output.path().join("first.rdm")).models.unwrap().len(),
        1
    );
    succeeds(
        ng().arg("rdm")
            .arg(&input)
            .args([
                "--format",
                "P4h_N4b_G4b_B4b_T2h",
                "--mesh-index",
                "1",
                "--force",
            ])
            .arg("-o")
            .arg(&destination),
    );
    assert_eq!(read(&destination).models.unwrap().len(), 1);
    let invalid = ng()
        .arg("rdm")
        .arg(&input)
        .args(["--format", "P4h_N4b_G4b_B4b_T2h", "--mesh-index", "99"])
        .arg("-o")
        .arg(output.path().join("invalid.rdm"))
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(1));
    let error = String::from_utf8(invalid.stderr).unwrap();
    assert!(
        error.contains("mesh index 99") && !error.contains("panicked"),
        "{error}"
    );
}

#[test]
fn external_animation_preserves_legacy_bytes_and_scene_keys() {
    let output = tempfile::tempdir().unwrap();
    let input = fixture("basalt_crusher_others_lod0.rdm");
    let animation = fixture("basalt_crusher_others_idle01.rdm");
    let expected = output.path().join("legacy.glb");
    succeeds(
        legacy()
            .arg("-i")
            .arg(&input)
            .arg("-sam")
            .arg(&animation)
            .arg("-o")
            .arg(&expected),
    );
    let actual = output.path().join("new.glb");
    fs::write(&actual, b"existing output").unwrap();
    succeeds(
        ng().arg("gltf")
            .arg(&input)
            .arg("--animation")
            .arg(&animation)
            .args(["--mesh-index", "0"])
            .arg("--force")
            .arg("-o")
            .arg(&actual),
    );
    assert_eq!(fs::read(&actual).unwrap(), fs::read(&expected).unwrap());
    succeeds(
        ng().arg("gltf")
            .arg(&input)
            .arg("--animation")
            .arg(&animation)
            .arg("--force")
            .arg("-o")
            .arg(&actual),
    );
    let clips = |path: &Path| {
        let (document, buffers, _) = gltf::import(path).unwrap();
        document
            .animations()
            .map(|animation| {
                let channels = animation
                    .channels()
                    .map(|channel| {
                        let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
                        let values: Vec<_> = match reader.read_outputs().unwrap() {
                            gltf::animation::util::ReadOutputs::Rotations(values) => {
                                values.into_f32().flatten().map(f32::to_bits).collect()
                            }
                            gltf::animation::util::ReadOutputs::Translations(values) => {
                                values.flatten().map(f32::to_bits).collect()
                            }
                            _ => unreachable!(),
                        };
                        (
                            channel.target().node().name().unwrap().to_owned(),
                            channel.target().property(),
                            channel.sampler().interpolation(),
                            reader
                                .read_inputs()
                                .unwrap()
                                .map(f32::to_bits)
                                .collect::<Vec<_>>(),
                            values,
                        )
                    })
                    .collect::<Vec<_>>();
                (
                    animation.name().unwrap().to_owned(),
                    animation.extras().as_ref().unwrap().get().to_owned(),
                    channels,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(clips(&actual), clips(&expected));
    // An external clip drives the shared skeleton of every model by default.
    use rdm4lib::rdm::{RdmDocument, RdmEndian, RdmReader, RdmWriter};
    let mut document = RdmReader::new(std::io::Cursor::new(fs::read(&input).unwrap()))
        .unwrap()
        .read_root::<RdmDocument>()
        .unwrap()
        .value;
    let models = document.models.as_mut().unwrap();
    models.push(models[0].clone());
    let multi = output.path().join("multi.rdm");
    RdmWriter::new(fs::File::create(&multi).unwrap(), RdmEndian::Little)
        .write_root(&document)
        .unwrap();
    let multi_output = output.path().join("multi.glb");
    succeeds(
        ng().arg("gltf")
            .arg(&multi)
            .arg("--animation")
            .arg(&animation)
            .arg("-o")
            .arg(&multi_output),
    );
    let (document, _, _) = gltf::import(&multi_output).unwrap();
    assert_eq!(document.meshes().count(), 2);
    assert_eq!(document.skins().count(), 1);
    assert_eq!(
        document
            .nodes()
            .filter(|node| node.skin().is_some())
            .count(),
        2
    );
    assert_eq!(clips(&multi_output), clips(&expected));
    let destination = output.path().join("restored.rdm");
    let animation = output.path().join("restored_anim_0.rdm");
    let mut import = ng();
    import
        .arg("rdm")
        .arg(&actual)
        .args([
            "--format",
            "P4h_N4b_G4b_B4b_T2h_I4b",
            "--skeleton",
            "--animation",
            "--no-transform",
        ])
        .arg("-o")
        .arg(&destination);
    succeeds(&mut import);
    let mesh = fs::read(&destination).unwrap();
    let mut expected = fs::read(&animation).unwrap();
    let record = u32::from_le_bytes(expected[0x2c..0x30].try_into().unwrap()) as usize;
    expected[record + 12..record + 16].copy_from_slice(&60u32.to_le_bytes());
    succeeds(import.args(["--sampling-rate", "60", "--force"]));
    assert_eq!(
        fs::read(animation).unwrap(),
        expected,
        "only the nominal rate changes"
    );
    assert_eq!(fs::read(destination).unwrap(), mesh);
}
