use rdm4lib::RdModell;

use rdm4lib::gltf_export;
use rdm4lib::rdm_anim::RdAnim;

use rdm4lib::gltf_reader;

use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::str;

#[cfg(test)]
mod tests {
    use super::*;
    use rdm4lib::rdm_data_anim::RdAnimWriter2;
    use rdm4lib::rdm_data_main::RdWriter2;
    #[cfg(target_os = "windows")]
    use rdm4lib::rdm_material::RdMaterial;
    use rdm4lib::{gltf_export::GltfExportFormat, vertex::TargetVertexFormat};
    use sha2::{Digest, Sha256};
    use std::convert::TryFrom;
    use std::fs;

    fn load_scene(path: &str, skin: bool) -> rdm4lib::scene::Scene {
        use rdm4lib::rdm::{RdmDocument, RdmReader};
        RdmReader::new(std::io::BufReader::new(File::open(path).unwrap()))
            .unwrap()
            .read_root::<RdmDocument>()
            .unwrap()
            .value
            .into_scene(skin)
            .unwrap()
    }

    fn add_animation(scene: &mut rdm4lib::scene::Scene, anim: RdAnim) {
        let names = scene.skeletons[0]
            .joints
            .iter()
            .map(|joint| joint.name.as_str())
            .collect::<Vec<_>>();
        scene
            .animations
            .push(anim.into_scene_animation(0, &names).unwrap());
    }

    pub fn check_hash(path: &Path, expected: &str) {
        let hash = Sha256::new()
            .chain_update(fs::read(path).unwrap())
            .finalize();

        let calc_hash = base16ct::lower::encode_string(&hash);
        assert_eq!(expected, calc_hash);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn reads_mesh_with_multiple_material_groups() {
        let rdm = RdModell::from("rdm/fishery_others_lod2.rdm");
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h");
        assert_eq!(rdm.vertex.len(), 3291);
        assert_eq!(rdm.triangle_indices.len() * 3, 7473);
        assert_eq!(rdm.mesh_info.len(), 2);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn exports_animated_mesh_without_gltf_validation_errors() {
        let mut scene = load_scene("rdm/basalt_crusher_others_lod2.rdm", true);
        assert_eq!(scene.meshes[0].vertex_count, 2615);
        add_animation(
            &mut scene,
            RdAnim::from("rdm/basalt_crusher_others_work01.rdm"),
        );

        let output_dir = tempfile::tempdir().unwrap();
        gltf_export::build_scene(
            scene,
            Some(output_dir.path().into()),
            false,
            GltfExportFormat::GltfSeparate,
        )
        .unwrap();

        let gltf_path = output_dir.path().join("out.gltf");
        if cfg!(target_os = "windows") {
            Command::new("gltf_validator.exe")
                .arg("-ar")
                .arg(&gltf_path)
                .output()
                .or_else(|_| {
                    Command::new("..\\gltf_validator.exe")
                        .arg("-ar")
                        .arg(&gltf_path)
                        .output()
                })
                .expect("failed to execute process")
        } else {
            Command::new("gltf_validator")
                .arg("-ar")
                .arg(&gltf_path)
                .output()
                .or_else(|_| {
                    Command::new("../gltf_validator")
                        .arg("-ar")
                        .arg(&gltf_path)
                        .output()
                })
                .expect("failed to execute process")
        };

        let mut f = File::open(output_dir.path().join("out.gltf.report.json")).unwrap();
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut buffer).ok();

        let report = str::from_utf8(&buffer).unwrap();
        let v: serde_json::Value = serde_json::from_str(report).unwrap();

        assert_eq!(
            2615,
            v["info"]["totalVertexCount"]
                .to_string()
                .parse::<u32>()
                .unwrap()
        );

        assert_eq!(
            1,
            v["info"]["animationCount"]
                .to_string()
                .parse::<u32>()
                .unwrap()
        );

        assert_eq!(
            0,
            v["issues"]["numErrors"].to_string().parse::<u32>().unwrap()
        );

        assert_eq!(
            0,
            v["issues"]["numWarnings"]
                .to_string()
                .parse::<u32>()
                .unwrap()
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn exports_68_byte_mesh_with_60_hz_animation_preserving_keyframe_times() {
        use rdm4lib::rdm::{RdmDocument, RdmReader};

        // Original Anno 2070 jellyfish mesh and animation.
        let mesh_path = "rdm/jellyfish_lod0.rdm";
        let source = RdmReader::new(std::io::BufReader::new(File::open(mesh_path).unwrap()))
            .unwrap()
            .read_root::<RdmDocument>()
            .unwrap();
        let models = source
            .layout
            .blocks
            .iter()
            .find(|block| block.context == "RdmDocument.models")
            .unwrap();
        assert_eq!(models.source.stride, 68);
        let mut anim = RdAnim::from("rdm/jellyfish_idle01.rdm");
        assert_eq!(anim.sampling_rate, 60);
        // Exercise #176's late start and a joint with no animation channels.
        for frame in &mut anim.anim_vec[0].frames {
            frame.time += 1.0 / 24.0;
        }
        anim.time_max += 42;
        let idle_joint = anim.anim_vec.pop().unwrap().name;
        let tracks = anim.anim_vec.clone();

        let mut rdm = RdModell::from(mesh_path);
        rdm.add_skin();
        let joints = rdm.joints.clone().unwrap();
        let mut scene = source.value.into_scene(true).unwrap();
        add_animation(&mut scene, anim);
        let output_dir = tempfile::tempdir().unwrap();
        gltf_export::build_scene(
            scene,
            Some(output_dir.path().into()),
            false,
            GltfExportFormat::GltfSeparate,
        )
        .unwrap();

        let (document, buffers, _) = gltf::import(output_dir.path().join("out.gltf")).unwrap();
        let animation = document.animations().next().unwrap();
        assert_eq!(animation.channels().count(), tracks.len() * 2);
        for channel in animation.channels() {
            let node = channel.target().node();
            let track = tracks
                .iter()
                .find(|track| Some(track.name.as_str()) == node.name())
                .unwrap();
            let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
            assert_eq!(
                channel.sampler().input().min().unwrap()[0]
                    .as_f64()
                    .unwrap() as f32,
                track.frames[0].time,
            );
            assert_eq!(
                reader.read_inputs().unwrap().collect::<Vec<_>>(),
                track
                    .frames
                    .iter()
                    .map(|frame| frame.time)
                    .collect::<Vec<_>>(),
                "keyframe times for {}",
                track.name,
            );
        }
        let input = output_dir.path().join("out.gltf");
        let imported = gltf_reader::ImportedGltf::try_from(input.as_path()).unwrap();
        let error = imported
            .read_animations(&joints, Default::default())
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("t=0") && error.contains(&tracks[0].name),
            "{error}"
        );
        let animations = imported
            .read_animations(
                &joints,
                gltf_reader::AnimationImportOptions {
                    interpolate: true,
                    ..Default::default()
                },
            )
            .unwrap();
        let animation = &animations[0];
        assert_eq!(animation.sampling_rate, 60);
        for source in tracks {
            let track = animation
                .anim_vec
                .iter()
                .find(|t| t.name == source.name)
                .unwrap();
            assert_eq!(track.frames[0].time.to_bits(), 0.0f32.to_bits());
            let bits = |frame: &rdm4lib::rdm_anim::Frame| {
                (
                    frame.rotation.map(f32::to_bits),
                    frame.translation.map(f32::to_bits),
                )
            };
            assert_eq!(bits(&track.frames[0]), bits(&source.frames[0]));
            for expected in source.frames {
                let index = track
                    .frames
                    .binary_search_by(|frame| frame.time.total_cmp(&expected.time))
                    .unwrap();
                let actual = &track.frames[index];
                assert_eq!(actual.time.to_bits(), expected.time.to_bits());
                assert_eq!(bits(actual), bits(&expected));
            }
        }
        let idle = animation
            .anim_vec
            .iter()
            .find(|t| t.name == idle_joint)
            .unwrap();
        assert_eq!(idle.frames[0].time, 0.0);
        assert!(idle.frames.iter().all(|frame| {
            frame.rotation == idle.frames[0].rotation
                && frame.translation == idle.frames[0].translation
        }));
        let minimal = imported
            .read_animations(
                &joints,
                gltf_reader::AnimationImportOptions {
                    interpolate: true,
                    idle_intervals: 0,
                    ..Default::default()
                },
            )
            .unwrap();
        let idle = minimal[0]
            .anim_vec
            .iter()
            .find(|t| t.name == idle_joint)
            .unwrap();
        assert_eq!(idle.frames.len(), 2);
        assert!(idle.frames.iter().all(|frame| frame.time.is_finite()));
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    #[cfg(target_os = "windows")]
    fn exports_animated_mesh_with_dds_texture() {
        // Exercise both linked PNGs and embedded image buffer views.
        for format in [GltfExportFormat::GltfSeparate, GltfExportFormat::Glb] {
            let mut scene = load_scene("rdm/excavator_tycoons_lod1.rdm", true);
            add_animation(&mut scene, RdAnim::from("rdm/excavator_tycoons_work02.rdm"));
            let output_dir = tempfile::tempdir().unwrap();
            gltf_export::GltfWriter::new(scene)
                .textures(RdMaterial::from(r"rdm/excavator_tycoons_diff_0.dds"))
                .write(Some(output_dir.path().into()), false, format)
                .unwrap();
            let extension = if format == GltfExportFormat::Glb {
                "glb"
            } else {
                "gltf"
            };
            let (document, _, images) =
                gltf::import(output_dir.path().join(format!("out.{extension}"))).unwrap();
            assert_eq!(images.len(), 1);
            assert!(document
                .materials()
                .next()
                .unwrap()
                .pbr_metallic_roughness()
                .base_color_texture()
                .is_some());
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    #[cfg(target_os = "windows")]
    fn exports_mesh_with_multiple_dds_materials() {
        let scene = load_scene("rdm/residence_tier_02_estate_02_lod2.rdm", false);
        assert_eq!(scene.meshes[0].vertex_count, 1965);
        assert_eq!(scene.meshes[0].primitives.len(), 3);
        let output_dir = tempfile::tempdir().unwrap();
        gltf_export::GltfWriter::new(scene)
            .textures(RdMaterial::new(vec![
                "rdm/residence_tier02_04_diff_0.dds",
                "rdm/residence_02_05_diff_0.dds",
                "rdm/brick_wall_white_estate_01_diff_0.dds",
            ]))
            .write(
                Some(output_dir.path().into()),
                false,
                GltfExportFormat::GltfSeparate,
            )
            .unwrap();
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn reads_position_only_mesh_from_file() {
        let rdm = RdModell::from("rdm/fishery_others_cutout_lod0.rdm");
        assert_eq!(rdm.vertex.len(), 32);
        assert_eq!(rdm.triangle_indices.len() * 3, 78);
        assert_eq!(rdm.vertex.to_string(), "P4h");
        assert_eq!(rdm.mesh_info.len(), 1);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn formats_packed_colors_as_c4c_to_match_game_cfg() {
        let rdm = RdModell::from("rdm/ark_waterfall2.rdm");
        assert_eq!(rdm.vertex.len(), 105);
        // Matches the vertex format declared in the game CFG.
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_T2h_C4c");
        assert_eq!(rdm.mesh_info.len(), 1);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn rewrites_skinned_rdm_with_expected_bytes() {
        let mut rdm = RdModell::from("rdm/basalt_crusher_others_lod2.rdm");
        rdm.add_skin();
        assert_eq!(rdm.vertex.len(), 2615);
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h_I4b");
        assert_eq!(rdm.mesh_info.len(), 1);

        let exp_rdm = RdWriter2::new(rdm);
        let output_dir = tempfile::tempdir().unwrap();
        let dest_path = exp_rdm.write_rdm(Some(output_dir.path().into()), false);
        check_hash(
            &dest_path,
            "8f048b28e18b12aa1d078e42eb3fe7dcc8bb5017b529c09b8bfb6eb11973fc75",
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn exports_imported_gltf_with_skin_and_animation() {
        let f_path = Path::new("rdm/gltf/stormtrooper_with_tangent.gltf");
        let i_gltf = gltf_reader::ImportedGltf::try_from(f_path).unwrap();
        let scene = i_gltf.read_scene(true, Some(Default::default())).unwrap();
        assert_eq!(scene.meshes[0].vertex_count, 5184);
        assert_eq!(scene.animations.len(), 1);

        let output_dir = tempfile::tempdir().unwrap();
        gltf_export::build_scene(
            scene,
            Some(output_dir.path().into()),
            false,
            GltfExportFormat::GltfSeparate,
        )
        .unwrap();
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn converts_skinned_gltf_to_expected_rdm_bytes() {
        let rdm = gltf_reader::ImportedGltf::gltf_to_rdm(
            &gltf_reader::ImportedGltf::try_from(Path::new(
                "rdm/gltf/stormtrooper_with_tangent.gltf",
            ))
            .unwrap(),
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b,
            true,
            false,
            false,
            None,
        );
        assert_eq!(rdm.vertex.len(), 5184);
        assert_eq!(rdm.joints.as_ref().unwrap().len(), 72);

        let exp_rdm = RdWriter2::new(rdm);
        let output_dir = tempfile::tempdir().unwrap();
        let dest_path = exp_rdm.write_rdm(Some(output_dir.path().into()), false);
        check_hash(
            &dest_path,
            "f7612fc465e1a0f27687ed7371c88f945b71858bddbe963bb39389644e41b206",
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn converts_gltf_animation_to_expected_rdm_bytes() {
        let f_path = Path::new("rdm/gltf/stormtrooper_with_tangent.gltf");
        let i_gltf = gltf_reader::ImportedGltf::try_from(f_path).unwrap();
        let rdm = gltf_reader::ImportedGltf::gltf_to_rdm(
            &i_gltf,
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b,
            true,
            false,
            false,
            None,
        );
        assert_eq!(rdm.vertex.len(), 5184);

        let jj = &rdm.joints.unwrap();
        let mut anims = gltf_reader::ImportedGltf::read_animation(&i_gltf, jj, 6, 0.33333).unwrap();

        assert_eq!(anims.len(), 1);
        let anim = anims.pop().unwrap();
        let exp_rdm = RdAnimWriter2::new(anim);
        let output_dir = tempfile::tempdir().unwrap();
        exp_rdm.write_anim_rdm(Some(output_dir.path().into()), false);

        check_hash(
            &output_dir.path().join("anim_0.rdm"),
            "c2183cd7d5c342a6e6e1c8260b5ab93503fe2ad282d851bbee791bf3a1d6c3f1",
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn converts_unskinned_gltf_to_expected_rdm_bytes() {
        let rdm = gltf_reader::ImportedGltf::gltf_to_rdm(
            &gltf_reader::ImportedGltf::try_from(Path::new("rdm/gltf/stormtrooper.gltf")).unwrap(),
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h,
            false,
            false,
            false,
            None,
        );
        assert_eq!(rdm.vertex.len(), 5184);

        let exp_rdm = RdWriter2::new(rdm);
        let output_dir = tempfile::tempdir().unwrap();
        let dest_path = exp_rdm.write_rdm(Some(output_dir.path().into()), false);
        check_hash(
            &dest_path,
            "48a52be827288189f8ec8bbf28d7c0cb50305b51c7513285472c5fcc8c7b9714",
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    #[ignore = "Triangle fixture has no normals"]
    fn converts_triangle_gltf_without_normals_to_rdm() {
        let rdm = gltf_reader::ImportedGltf::gltf_to_rdm(
            &gltf_reader::ImportedGltf::try_from(Path::new("rdm/gltf/triangle.gltf")).unwrap(),
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h,
            false,
            false,
            false,
            None,
        );
        assert_eq!(rdm.vertex.len(), 3);

        let exp_rdm = RdWriter2::new(rdm);

        let output_dir = tempfile::tempdir().unwrap();
        let dest_path = exp_rdm.write_rdm(Some(output_dir.path().into()), false);
        check_hash(
            &dest_path,
            "f456d4418387e5bcbe4522064f5fdb75bd67b762793489ad6220a9718d431e3c",
        );
    }
}
