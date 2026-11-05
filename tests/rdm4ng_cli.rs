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

#[test]
fn help_version_and_argument_errors() {
    let help = ng().arg("--help").output().unwrap();
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    for name in ["rdm4ng", "gltf", "rdm", "inspect", "rdm2gltf", "gltf2rdm"] {
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
        assert_eq!(
            fs::read(directory.join("mesh with spaces.glb")).unwrap(),
            fs::read(&expected).unwrap()
        );
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
    assert_eq!(fs::read(actual).unwrap(), fs::read(expected).unwrap());
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
fn rdm_command_and_alias_match_legacy_output() {
    let output = tempfile::tempdir().unwrap();
    let input = make_glb(output.path());
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
        assert_eq!(
            fs::read(directory.join("mesh.rdm")).unwrap(),
            fs::read(&expected).unwrap()
        );
    }
}

#[test]
fn external_animation_includes_skeleton_and_matches_legacy_output() {
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
            .arg("--force")
            .arg("-o")
            .arg(&actual),
    );
    assert_eq!(fs::read(actual).unwrap(), fs::read(expected).unwrap());
}
