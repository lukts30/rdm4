//! Check both CLI parsers against the same conversion pipeline.

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
    for name in ["rdm4ng", "gltf", "rdm", "rdm2gltf", "gltf2rdm"] {
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
