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
        let mut rdm = RdModell::from("rdm/basalt_crusher_others_lod2.rdm");
        assert_eq!(rdm.vertex.len(), 2615);
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h_I4b");
        assert_eq!(rdm.vertex.get_size(), 28);
        assert_eq!(rdm.mesh_info.len(), 1);

        rdm.add_skin();

        let anim = RdAnim::from("rdm/basalt_crusher_others_work01.rdm");
        rdm.add_anim(anim);

        let output_dir = tempfile::tempdir().unwrap();
        gltf_export::build(
            rdm,
            Some(output_dir.path().into()),
            false,
            GltfExportFormat::GltfSeparate,
        );

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
        use binrw::BinReaderExt;
        use rdm4lib::rdm_data_main::{RdmFile, RdmKindMesh};

        // Original Anno 2070 jellyfish mesh and animation.
        let mesh_path = "rdm/jellyfish_lod0.rdm";
        let source: RdmFile<RdmKindMesh> = File::open(mesh_path).unwrap().read_le().unwrap();
        assert_eq!(source.header1.meta.info.part_size, 68);
        let anim = RdAnim::from("rdm/jellyfish_idle01.rdm");
        assert_eq!(anim.sampling_rate, 60);
        let tracks = anim.anim_vec.clone();

        let mut rdm = RdModell::from(mesh_path);
        rdm.add_skin();
        rdm.add_anim(anim);
        let output_dir = tempfile::tempdir().unwrap();
        gltf_export::build(
            rdm,
            Some(output_dir.path().into()),
            false,
            GltfExportFormat::GltfSeparate,
        );

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
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    #[cfg(target_os = "windows")]
    fn exports_animated_mesh_with_dds_texture() {
        let mut rdm = RdModell::from("rdm/excavator_tycoons_lod1.rdm");
        rdm.mat = Some(RdMaterial::from(r"rdm/excavator_tycoons_diff_0.dds"));
        assert_eq!(rdm.vertex.len(), 5225);
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h_I4b");
        assert_eq!(rdm.mesh_info.len(), 1);

        rdm.add_skin();

        let anim = RdAnim::from("rdm/excavator_tycoons_work02.rdm");
        rdm.add_anim(anim);

        let output_dir = tempfile::tempdir().unwrap();
        gltf_export::build(
            rdm,
            Some(output_dir.path().into()),
            false,
            GltfExportFormat::GltfSeparate,
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    #[cfg(target_os = "windows")]
    fn exports_mesh_with_multiple_dds_materials() {
        let mut rdm = RdModell::from("rdm/residence_tier_02_estate_02_lod2.rdm");
        rdm.mat = Some(RdMaterial::new(vec![
            "rdm/residence_tier02_04_diff_0.dds",
            "rdm/residence_02_05_diff_0.dds",
            "rdm/brick_wall_white_estate_01_diff_0.dds",
        ]));
        assert_eq!(rdm.vertex.len(), 1965);
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h");
        assert_eq!(rdm.mesh_info.len(), 3);

        let output_dir = tempfile::tempdir().unwrap();
        gltf_export::build(
            rdm,
            Some(output_dir.path().into()),
            false,
            GltfExportFormat::GltfSeparate,
        );
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
            "2f6993eb99b4a0c89ee9723c822d18034cc772900331d1cb80e403b96339d398",
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn exports_imported_gltf_with_skin_and_animation() {
        let f_path = Path::new("rdm/gltf/stormtrooper_with_tangent.gltf");
        let i_gltf = gltf_reader::ImportedGltf::try_from(f_path).unwrap();
        let mut rdm = gltf_reader::ImportedGltf::gltf_to_rdm(
            &i_gltf,
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b,
            true,
            false,
            true,
            None,
        );
        assert_eq!(rdm.vertex.len(), 5184);

        let jj = rdm.joints.clone().unwrap();
        let mut anims =
            gltf_reader::ImportedGltf::read_animation(&i_gltf, &jj, 6, 0.33333).unwrap();

        assert_eq!(anims.len(), 1);
        let anim = anims.pop().unwrap();

        rdm.add_anim(anim);

        let output_dir = tempfile::tempdir().unwrap();
        gltf_export::build(
            rdm,
            Some(output_dir.path().into()),
            false,
            GltfExportFormat::GltfSeparate,
        );
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
            "f11a0a18cd89df95799719dc9b77cfda0d2b87d5080b506f9a257c0d51acda88",
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
        let mut anims =
            gltf_reader::ImportedGltf::read_animation(&i_gltf, &jj, 6, 0.33333).unwrap();

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
            "188da4dc618ab2c80c88386d5272071a1999ccb524f72c05b22748ce21bd74df",
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
