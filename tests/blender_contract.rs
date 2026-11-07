//! CLI contracts observed in anno-mods/Blender-Anno-1800 at e0391b394f12e8fe9134524fd90508e606877e81.
//! Keep these independent of a Blender installation or sibling checkout.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use rdm4lib::{
    rdm::{RdmDocument, RdmReader},
    rdm_anim::RdAnim,
};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("rdm4lib/rdm")
        .join(name)
}

fn converter() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rdm4-bin"))
}

fn succeeds(command: &mut Command) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{command:?}\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn import_command(input: &Path, directory: &Path) -> Command {
    let mut command = converter();
    command
        .arg("--input")
        .arg(input)
        .arg("-n")
        .arg("--outdst")
        .arg(directory);
    command
}

#[test]
fn static_import_preserves_basename_coordinates_winding_and_material_slots() {
    let output = tempfile::Builder::new()
        .prefix("blender contract ")
        .tempdir()
        .unwrap();
    let input = output.path().join("fishery with spaces.rdm");
    fs::copy(fixture("fishery_others_lod2.rdm"), &input).unwrap();
    succeeds(&mut import_command(&input, output.path()));

    // convert_to_glb_if_required expects exactly this basename and extension.
    let glb_path = input.with_extension("glb");
    let (document, buffers, _) = gltf::import(&glb_path).unwrap();
    assert!(document
        .buffers()
        .all(|buffer| matches!(buffer.source(), gltf::buffer::Source::Bin)));
    assert_eq!(document.scenes().next().unwrap().nodes().count(), 1);
    assert_eq!(document.meshes().count(), 1);
    assert_eq!(document.materials().count(), 2);
    let primitives: Vec<_> = document.meshes().next().unwrap().primitives().collect();
    assert_eq!(primitives.len(), 2);
    for (primitive, (material, index_count)) in primitives.iter().zip([(0, 7233), (1, 240)]) {
        assert_eq!(primitive.material().index(), Some(material));
        assert_eq!(primitive.mode(), gltf::mesh::Mode::Triangles);
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
        assert_eq!(reader.read_positions().unwrap().len(), 3291);
        assert_eq!(reader.read_indices().unwrap().into_u32().len(), index_count);
    }
    let reader = primitives[0].reader(|buffer| Some(&buffers[buffer.index()]));
    assert_eq!(
        reader.read_positions().unwrap().next().unwrap(),
        [2.4667969, -1.9433594, -0.3942871]
    );
    assert_eq!(
        reader
            .read_indices()
            .unwrap()
            .into_u32()
            .take(3)
            .collect::<Vec<_>>(),
        [2110, 1034, 1281]
    );

    // The add-on's default import does not pass --force. A collision must fail
    // without truncating an existing cached GLB; explicit --force can replace it.
    let original = fs::read(&glb_path).unwrap();
    let collision = import_command(&input, output.path()).output().unwrap();
    assert!(!collision.status.success());
    assert_eq!(fs::read(&glb_path).unwrap(), original);
    succeeds(import_command(&input, output.path()).arg("--force"));
    assert_eq!(fs::read(glb_path).unwrap(), original);
}

#[test]
fn static_import_keeps_cfg_material_associations_when_group_order_differs() {
    let output = tempfile::tempdir().unwrap();
    let input = output.path().join("palace.rdm");
    fs::copy(
        fixture("benchmark/12-palace_hall_tpart_building_lod3.rdm"),
        &input,
    )
    .unwrap();
    succeeds(&mut import_command(&input, output.path()));
    let (document, _, _) = gltf::import(input.with_extension("glb")).unwrap();
    assert_eq!(document.materials().count(), 2);
    let materials: Vec<_> = document
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .map(|primitive| primitive.material().index().unwrap())
        .collect();
    assert_eq!(materials, [1, 0]);
}

#[test]
fn animation_import_supports_sam_cluster_and_forced_out_glb() {
    let output = tempfile::Builder::new()
        .prefix("blender animation ")
        .tempdir()
        .unwrap();
    let input = output.path().join("model with spaces.rdm");
    let animation_path = output.path().join("idle animation.rdm");
    fs::copy(fixture("basalt_crusher_others_lod0.rdm"), &input).unwrap();
    fs::copy(fixture("basalt_crusher_others_idle01.rdm"), &animation_path).unwrap();
    let glb_path = output.path().join("out.glb");
    fs::write(&glb_path, b"old cached animation").unwrap();
    succeeds(
        converter()
            .arg("-i")
            .arg(&input)
            .arg("-sam")
            .arg(&animation_path)
            .arg("--force")
            .arg("--outdst")
            .arg(output.path()),
    );

    // import_animated_model_to_scene looks for out.glb, then renames it.
    let (document, buffers, _) = gltf::import(glb_path).unwrap();
    let skin = document.skins().next().unwrap();
    assert_eq!(skin.joints().count(), 9);
    let joint_names: BTreeSet<_> = skin.joints().map(|joint| joint.name().unwrap()).collect();
    let expected_names = BTreeSet::from([
        "SKN_building",
        "SKN_crusher_train",
        "SKN_fan",
        "SKN_drill_center",
        "SKN_top",
        "SKN_drill_element_E",
        "SKN_drill_element_N",
        "SKN_drill_element_S",
        "SKN_drill_element_W",
    ]);
    assert_eq!(joint_names, expected_names);
    let source = RdAnim::from(&animation_path);
    let animation = document.animations().next().unwrap();
    assert_eq!(animation.name(), Some("idle animation"));
    assert_eq!(animation.channels().count(), 18);
    let mut targets = BTreeSet::new();
    for channel in animation.channels() {
        let name = channel.target().node().name().unwrap();
        let property = channel.target().property();
        assert!(matches!(
            property,
            gltf::animation::Property::Rotation | gltf::animation::Property::Translation
        ));
        assert!(targets.insert((name, format!("{property:?}"))));
        let source_track = source
            .anim_vec
            .iter()
            .find(|track| track.name == name)
            .unwrap();
        let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
        let times: Vec<_> = reader.read_inputs().unwrap().collect();
        assert_eq!(times, [0.0, 0.033333335]);
        assert_eq!(
            times,
            source_track
                .frames
                .iter()
                .map(|frame| frame.time)
                .collect::<Vec<_>>()
        );
    }
    let primitive = document
        .meshes()
        .next()
        .unwrap()
        .primitives()
        .next()
        .unwrap();
    assert!(primitive.get(&gltf::Semantic::Joints(0)).is_some());
    assert!(primitive.get(&gltf::Semantic::Weights(0)).is_some());
    assert!(document
        .nodes()
        .any(|node| node.mesh().is_some() && node.skin().is_some()));
}

#[test]
fn rdm_export_supports_equals_format_and_preserves_basename_and_groups() {
    let output = tempfile::Builder::new()
        .prefix("blender export ")
        .tempdir()
        .unwrap();
    let input = output.path().join("exported mesh.rdm");
    fs::copy(fixture("fishery_others_lod2.rdm"), &input).unwrap();
    succeeds(&mut import_command(&input, output.path()));
    let read = |path: &Path| {
        RdmReader::new(std::io::BufReader::new(fs::File::open(path).unwrap()))
            .unwrap()
            .read_root::<RdmDocument>()
            .unwrap()
            .value
            .models
            .unwrap()
            .remove(0)
    };
    let expected = read(&input);
    // export_rdm deletes the old RDM before invoking the converter.
    fs::remove_file(&input).unwrap();
    succeeds(
        converter()
            .arg("--gltf=P4h_N4b_G4b_B4b_T2h")
            .arg("--input")
            .arg(input.with_extension("glb"))
            .arg("-n")
            .arg("--outdst")
            .arg(output.path()),
    );
    let actual = read(&input);
    assert_eq!(actual.vertices.count, expected.vertices.count);
    assert_eq!(actual.indices.values.len(), expected.indices.values.len());
    assert_eq!(actual.vertex_format.to_string(), "P4h_N4b_G4b_B4b_T2h");
    assert_eq!(actual.groups, expected.groups);
}
