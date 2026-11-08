use rdm4lib::gltf_export;

use rdm4lib::gltf_reader;

use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::str;

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "windows")]
    use rdm4lib::rdm_material::RdMaterial;
    use rdm4lib::{gltf_export::GltfExportFormat, vertex::TargetVertexFormat};
    use sha2::{Digest, Sha256};
    use std::convert::TryFrom;
    use std::fs;

    use rdm4lib::rdm::{
        write_scene, RdmAnimation, RdmDocument, RdmEndian, RdmExportOptions, RdmReader, RdmWriter,
    };

    fn read_document(path: &str) -> RdmDocument {
        RdmReader::new(std::io::BufReader::new(File::open(path).unwrap()))
            .unwrap()
            .read_root::<RdmDocument>()
            .unwrap()
            .value
    }

    fn options(format: TargetVertexFormat) -> RdmExportOptions {
        RdmExportOptions {
            format,
            mirror_x: false,
            no_transform: false,
            material_indices: None,
            legacy: true,
        }
    }

    fn load_scene(path: &str, skin: bool) -> rdm4lib::scene::Scene {
        read_document(path).into_scene(skin).unwrap()
    }

    fn read_animation(path: &str) -> RdmAnimation {
        let mut animation = read_document(path).animations.unwrap().remove(0);
        animation.name = Path::new(path)
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .into();
        animation
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
        let rdm = read_document("rdm/fishery_others_lod2.rdm");
        let model = &rdm.models.as_ref().unwrap()[0];
        assert_eq!(model.vertex_format.to_string(), "P4h_N4b_G4b_B4b_T2h");
        assert_eq!(model.vertices.count, 3291);
        assert_eq!(model.indices.values.len(), 7473);
        assert_eq!(model.groups.len(), 2);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn exports_animated_mesh_without_gltf_validation_errors() {
        let mut scene = load_scene("rdm/basalt_crusher_others_lod2.rdm", true);
        assert_eq!(scene.meshes[0].vertex_count, 2615);
        scene
            .attach_rdm_animation(read_animation("rdm/basalt_crusher_others_work01.rdm"), 0)
            .unwrap();

        let output_dir = tempfile::tempdir().unwrap();
        gltf_export::build_scene(
            scene,
            Some(output_dir.path().into()),
            false,
            GltfExportFormat::GltfSeparate,
        )
        .unwrap();

        let gltf_path = output_dir.path().join("out.gltf");
        let (document, _, _) = gltf::import(&gltf_path).unwrap();
        for primitive in document.meshes().flat_map(|mesh| mesh.primitives()) {
            for (_, attribute) in primitive.attributes() {
                assert_eq!(
                    attribute.view().unwrap().target(),
                    Some(gltf::buffer::Target::ArrayBuffer)
                );
            }
            assert_eq!(
                primitive.indices().unwrap().view().unwrap().target(),
                Some(gltf::buffer::Target::ElementArrayBuffer)
            );
        }
        for skin in document.skins() {
            assert_eq!(
                skin.inverse_bind_matrices()
                    .unwrap()
                    .view()
                    .unwrap()
                    .target(),
                None
            );
        }
        for sampler in document
            .animations()
            .flat_map(|animation| animation.samplers())
        {
            assert_eq!(sampler.input().view().unwrap().target(), None);
            assert_eq!(sampler.output().view().unwrap().target(), None);
        }

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
        assert!(
            !v["issues"]["messages"]
                .as_array()
                .unwrap()
                .iter()
                .any(|message| message["code"] == "BUFFER_VIEW_TARGET_MISSING"),
            "{report}"
        );

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
        let mut anim = read_animation("rdm/jellyfish_idle01.rdm");
        assert_eq!(anim.sampling_rate, 60);
        // Exercise #176's late start and a joint with no animation channels.
        for frame in &mut anim.tracks[0].frames {
            frame.time += 1.0 / 24.0;
        }
        anim.time_max += 42;
        let idle_joint = anim.tracks.pop().unwrap().joint_name;
        let tracks = anim.tracks.clone();

        let mut scene = source.value.into_scene(true).unwrap();
        scene.attach_rdm_animation(anim, 0).unwrap();
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
                .find(|track| Some(track.joint_name.as_str()) == node.name())
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
                track.joint_name,
            );
        }
        let input = output_dir.path().join("out.gltf");
        let imported = gltf_reader::ImportedGltf::try_from(input.as_path()).unwrap();
        let error = imported
            .read_scene(true, Some(Default::default()))
            .err()
            .unwrap()
            .to_string();
        assert!(
            error.contains("t=0") && error.contains(&tracks[0].joint_name),
            "{error}"
        );
        let imported_scene = imported
            .read_scene(
                true,
                Some(gltf_reader::AnimationImportOptions {
                    interpolate: true,
                    ..Default::default()
                }),
            )
            .unwrap();
        let animation = &imported_scene.animations[0];
        assert_eq!(animation.sampling_rate, 60);
        for source in tracks {
            let track = animation
                .tracks
                .iter()
                .find(|t| imported_scene.skeletons[0].joints[t.joint].name == source.joint_name)
                .unwrap();
            assert_eq!(track.frames[0].time.to_bits(), 0.0f32.to_bits());
            let bits = |rotation: [f32; 4], translation: [f32; 3]| {
                (rotation.map(f32::to_bits), translation.map(f32::to_bits))
            };
            let rdm_rotation = |[x, y, z, w]: [f32; 4]| [x, y, z, -w];
            assert_eq!(
                bits(
                    rdm_rotation(track.frames[0].rotation),
                    track.frames[0].translation
                ),
                bits(source.frames[0].rotation, source.frames[0].translation)
            );
            for expected in source.frames {
                let index = track
                    .frames
                    .binary_search_by(|frame| frame.time.total_cmp(&expected.time))
                    .unwrap();
                let actual = &track.frames[index];
                assert_eq!(actual.time.to_bits(), expected.time.to_bits());
                assert_eq!(
                    bits(rdm_rotation(actual.rotation), actual.translation),
                    bits(expected.rotation, expected.translation)
                );
            }
        }
        let idle = animation
            .tracks
            .iter()
            .find(|t| imported_scene.skeletons[0].joints[t.joint].name == idle_joint)
            .unwrap();
        assert_eq!(idle.frames[0].time, 0.0);
        assert!(idle.frames.iter().all(|frame| {
            frame.rotation == idle.frames[0].rotation
                && frame.translation == idle.frames[0].translation
        }));
        let minimal = imported
            .read_scene(
                true,
                Some(gltf_reader::AnimationImportOptions {
                    interpolate: true,
                    idle_intervals: 0,
                    ..Default::default()
                }),
            )
            .unwrap();
        let idle = minimal.animations[0]
            .tracks
            .iter()
            .find(|t| minimal.skeletons[0].joints[t.joint].name == idle_joint)
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
            scene
                .attach_rdm_animation(read_animation("rdm/excavator_tycoons_work02.rdm"), 0)
                .unwrap();
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
        let rdm = read_document("rdm/fishery_others_cutout_lod0.rdm");
        let model = &rdm.models.as_ref().unwrap()[0];
        assert_eq!(model.vertices.count, 32);
        assert_eq!(model.indices.values.len(), 78);
        assert_eq!(model.vertex_format.to_string(), "P4h");
        assert_eq!(model.groups.len(), 1);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn formats_packed_colors_as_c4c_to_match_game_cfg() {
        let rdm = read_document("rdm/ark_waterfall2.rdm");
        let model = &rdm.models.as_ref().unwrap()[0];
        assert_eq!(model.vertices.count, 105);
        // Matches the vertex format declared in the game CFG.
        assert_eq!(model.vertex_format.to_string(), "P4h_N4b_T2h_C4c");
        assert_eq!(model.groups.len(), 1);
    }

    #[test]
    fn imports_mixed_color_storage_with_legacy_byte_conversion() {
        use rdm4lib::{rdm::RdmAttributeValues, vertex::VertexSemantic};
        use serde_json::json;

        let output = tempfile::tempdir().unwrap();
        let mut bytes: Vec<_> = [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        let mut views = vec![json!({"buffer": 0, "byteLength": bytes.len()})];
        let mut accessors = vec![json!({
            "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3",
            "min": [0, 0, 0], "max": [1, 1, 0]
        })];
        let mut primitives = Vec::new();
        for (component_type, color) in [
            (5121, vec![11u8, 77, 193, 255]),
            (
                5123,
                [256u16, 512, 32768, 65535]
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
            ),
            (
                5126,
                [0.25f32, 0.5, 0.75, 1.0]
                    .into_iter()
                    .flat_map(f32::to_le_bytes)
                    .collect(),
            ),
            (5121, vec![37, 88, 199, 128]),
        ] {
            views.push(json!({
                "buffer": 0, "byteOffset": bytes.len(), "byteLength": color.len() * 3
            }));
            bytes.extend(color.repeat(3));
            accessors.push(json!({
                "bufferView": views.len() - 1, "componentType": component_type,
                "count": 3, "type": "VEC4", "normalized": component_type != 5126
            }));
            primitives.push(json!({"attributes": {
                "POSITION": 0, "COLOR_0": accessors.len() - 1
            }}));
        }
        fs::write(output.path().join("colors.bin"), &bytes).unwrap();
        let input = output.path().join("colors.gltf");
        fs::write(
            &input,
            serde_json::to_vec(&json!({
                "asset": {"version": "2.0"},
                "buffers": [{"uri": "colors.bin", "byteLength": bytes.len()}],
                "bufferViews": views, "accessors": accessors,
                "meshes": [{"primitives": primitives}], "nodes": [{"mesh": 0}],
                "scenes": [{"nodes": [0]}], "scene": 0
            }))
            .unwrap(),
        )
        .unwrap();
        let scene = gltf_reader::ImportedGltf::try_from(input.as_path())
            .unwrap()
            .read_scene(false, None)
            .unwrap();
        let document =
            RdmDocument::from_scene(scene, options(TargetVertexFormat::P3f_N3f_G3f_B3f_T2f_C4b))
                .unwrap();
        let model = &document.models.as_ref().unwrap()[0];
        let color = model
            .vertex_format
            .attributes
            .iter()
            .position(|attribute| attribute.semantic == VertexSemantic::Color)
            .unwrap();
        // U16 uses the high byte; floats truncate after multiplication by 255.
        let expected: Vec<_> = [
            [11, 77, 193, 255],
            [1, 2, 128, 255],
            [63, 127, 191, 255],
            [37, 88, 199, 128],
        ]
        .into_iter()
        .flat_map(|color| [u32::from_le_bytes(color); 3])
        .collect();
        assert_eq!(model.groups.len(), 4);
        assert_eq!(
            model.vertices.attributes[color],
            RdmAttributeValues::U32(expected)
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn preserves_skinned_rdm_bytes_and_layout() {
        let source = fs::read("rdm/basalt_crusher_others_lod2.rdm").unwrap();
        let decoded = RdmReader::new(std::io::Cursor::new(&source))
            .unwrap()
            .read_root::<RdmDocument>()
            .unwrap();
        let output = RdmWriter::new(std::io::Cursor::new(Vec::new()), RdmEndian::Little)
            .write_preserving(&decoded.value, decoded.layout)
            .unwrap()
            .into_inner();
        assert_eq!(output, source);
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
        let imported = gltf_reader::ImportedGltf::try_from(Path::new(
            "rdm/gltf/stormtrooper_with_tangent.gltf",
        ))
        .unwrap();
        let scene = imported.read_scene(true, None).unwrap();
        assert_eq!(scene.meshes[0].vertex_count, 5184);
        assert_eq!(scene.skeletons[0].joints.len(), 72);
        let output = tempfile::tempdir().unwrap();
        let path = write_scene(
            scene,
            options(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b),
            Some(output.path().into()),
            false,
        )
        .unwrap();
        check_hash(
            &path,
            "f7612fc465e1a0f27687ed7371c88f945b71858bddbe963bb39389644e41b206",
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn converts_gltf_animation_to_expected_rdm_bytes() {
        let imported = gltf_reader::ImportedGltf::try_from(Path::new(
            "rdm/gltf/stormtrooper_with_tangent.gltf",
        ))
        .unwrap();
        let scene = imported.read_scene(true, Some(Default::default())).unwrap();
        assert_eq!(scene.animations.len(), 1);
        let output = tempfile::tempdir().unwrap();
        write_scene(
            scene,
            options(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b),
            Some(output.path().into()),
            false,
        )
        .unwrap();
        check_hash(
            &output.path().join("anim_0.rdm"),
            "c2183cd7d5c342a6e6e1c8260b5ab93503fe2ad282d851bbee791bf3a1d6c3f1",
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn converts_unskinned_gltf_to_expected_rdm_bytes() {
        let scene = gltf_reader::ImportedGltf::try_from(Path::new("rdm/gltf/stormtrooper.gltf"))
            .unwrap()
            .read_scene(false, None)
            .unwrap();
        assert_eq!(scene.meshes[0].vertex_count, 5184);
        let output = tempfile::tempdir().unwrap();
        let path = write_scene(
            scene,
            options(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h),
            Some(output.path().into()),
            false,
        )
        .unwrap();
        check_hash(
            &path,
            "48a52be827288189f8ec8bbf28d7c0cb50305b51c7513285472c5fcc8c7b9714",
        );
    }
    #[test]
    #[cfg_attr(miri, ignore)]
    #[ignore = "Triangle fixture has no normals"]
    fn converts_triangle_gltf_without_normals_to_rdm() {
        let scene = gltf_reader::ImportedGltf::try_from(Path::new("rdm/gltf/triangle.gltf"))
            .unwrap()
            .read_scene(false, None)
            .unwrap();
        assert_eq!(scene.meshes[0].vertex_count, 3);
        let output = tempfile::tempdir().unwrap();
        let path = write_scene(
            scene,
            options(TargetVertexFormat::P4h_N4b_G4b_B4b_T2h),
            Some(output.path().into()),
            false,
        )
        .unwrap();
        check_hash(
            &path,
            "f456d4418387e5bcbe4522064f5fdb75bd67b762793489ad6220a9718d431e3c",
        );
    }
}
