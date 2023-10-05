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
    use rdm4lib::{gltf_export::GltfExportFormat, vertex::TargetVertexFormat};
    use std::convert::TryFrom;
    use std::fs;
    use std::path::PathBuf;

    #[cfg(target_os = "windows")]
    use rdm4lib::rdm_material::RdMaterial;

    #[test]
    #[cfg_attr(miri, ignore)]
    fn fishery_others_lod2() {
        let rdm = RdModell::from("rdm/fishery_others_lod2.rdm");
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h");
        assert_eq!(rdm.vertex.len(), 3291);
        assert_eq!(rdm.triangle_indices.len() * 3, 7473);
        assert_eq!(rdm.mesh_info.len(), 2);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn basalt_crusher_others_lod2() {
        let mut rdm = RdModell::from("rdm/basalt_crusher_others_lod2.rdm");
        assert_eq!(rdm.vertex.len(), 2615);
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h_I4b");
        assert_eq!(rdm.vertex.get_size(), 28);
        assert_eq!(rdm.mesh_info.len(), 1);

        rdm.add_skin();

        let anim = RdAnim::from("rdm/basalt_crusher_others_work01.rdm");
        rdm.add_anim(anim);

        gltf_export::build(rdm, None, false, GltfExportFormat::GltfSeparate);

        let output = if cfg!(target_os = "windows") {
            Command::new("..\\gltf_validator.exe")
                .args(&["-ar", "gltf_out/out.gltf"])
                .output()
                .expect("failed to execute process")
        } else {
            Command::new("../gltf_validator")
                .args(&["-ar", "gltf_out/out.gltf"])
                .output()
                .expect("failed to execute process")
        };

        let hello = String::from_utf8_lossy(&output.stderr);
        let info: Vec<&str> = hello
            .lines()
            .nth(1)
            .unwrap()
            .split_terminator(',')
            .map(|f| f.trim())
            .collect();

        assert_eq!(r#"Errors: 0"#, info[0]);
        assert_eq!(r#"Warnings: 0"#, info[1]);

        let mut f = File::open("gltf_out/out.gltf.report.json").unwrap();
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
    #[cfg(target_os = "windows")]
    fn excavator_tycoons_lod1() {
        let mut rdm = RdModell::from("rdm/excavator_tycoons_lod1.rdm");
        rdm.mat = Some(RdMaterial::from(r"rdm/excavator_tycoons_diff_0.dds"));
        assert_eq!(rdm.vertex.len(), 5225);
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h_I4b");
        assert_eq!(rdm.mesh_info.len(), 1);

        rdm.add_skin();

        let anim = RdAnim::from("rdm/excavator_tycoons_work02.rdm");
        rdm.add_anim(anim);

        if !Path::new("gltf_out1").exists() {
            fs::create_dir("gltf_out1").unwrap();
        }
        gltf_export::build(
            rdm,
            Some(Path::new("gltf_out1").into()),
            false,
            GltfExportFormat::GltfSeparate,
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    #[cfg(target_os = "windows")]
    fn residence_tier02_estate02() {
        let mut rdm = RdModell::from("rdm/residence_tier_02_estate_02_lod2.rdm");
        rdm.mat = Some(RdMaterial::new(vec![
            "rdm/residence_tier02_04_diff_0.dds",
            "rdm/residence_02_05_diff_0.dds",
            "rdm/brick_wall_white_estate_01_diff_0.dds",
        ]));
        assert_eq!(rdm.vertex.len(), 1965);
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h");
        assert_eq!(rdm.mesh_info.len(), 3);

        if !Path::new("gltf_out2").exists() {
            fs::create_dir("gltf_out2").unwrap();
        }
        gltf_export::build(
            rdm,
            Some(Path::new("gltf_out2").into()),
            false,
            GltfExportFormat::GltfSeparate,
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn fishery_others_cutout_lod0() {
        let rdm = RdModell::from("rdm/fishery_others_cutout_lod0.rdm");
        assert_eq!(rdm.vertex.len(), 32);
        assert_eq!(rdm.triangle_indices.len() * 3, 78);
        assert_eq!(rdm.vertex.to_string(), "P4h");
        assert_eq!(rdm.mesh_info.len(), 1);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn ark_waterfall2() {
        let rdm = RdModell::from("rdm/ark_waterfall2.rdm");
        assert_eq!(rdm.vertex.len(), 105);
        // TODO: cfg says P4h_N4b_T2h_C4c
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_T2h_C4b");
        assert_eq!(rdm.mesh_info.len(), 1);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn exp_rdm_inv_basalt_crusher_others_lod0() {
        let mut rdm = RdModell::from("rdm/basalt_crusher_others_lod2.rdm");
        rdm.add_skin();
        assert_eq!(rdm.vertex.len(), 2615);
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h_I4b");
        assert_eq!(rdm.mesh_info.len(), 1);

        let exp_rdm = RdWriter2::new(rdm);
        let dir_dst = PathBuf::from("rdm_out/basalt_crusher");
        std::fs::create_dir_all(&dir_dst).unwrap();
        exp_rdm.write_rdm(Some(dir_dst), false);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn read_gltf_skin_round_trip() {
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

        if !Path::new("gltf_out3").exists() {
            fs::create_dir("gltf_out3").unwrap();
        }
        gltf_export::build(
            rdm,
            Some(Path::new("gltf_out3").into()),
            false,
            GltfExportFormat::GltfSeparate,
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn read_gltf() {
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

        let exp_rdm = RdWriter2::new(rdm);
        let dir_dst = PathBuf::from("rdm_out/stormtrooper");
        std::fs::create_dir_all(&dir_dst).unwrap();
        exp_rdm.write_rdm(Some(dir_dst), false);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn read_gltf_anim() {
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
        let dir_dst = PathBuf::from("rdm_out/stormtrooper");
        std::fs::create_dir_all(&dir_dst).unwrap();
        exp_rdm.write_anim_rdm(Some(dir_dst), false);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn read_gltf_no_skin() {
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
        let dir_dst = PathBuf::from("rdm_out/read_gltf_no_skin");
        std::fs::create_dir_all(&dir_dst).unwrap();
        exp_rdm.write_rdm(Some(dir_dst), false);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn read_gltf_no_skin_rdw2() {
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
        let dir_dst = PathBuf::from("rdm_out/read_gltf_no_skin_rdw2");
        std::fs::create_dir_all(&dir_dst).unwrap();
        exp_rdm.write_rdm(Some(dir_dst), false);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    #[ignore]
    fn read_gltf_no_skin2() {
        // no normals so ignore it !
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

        let dir_dst = PathBuf::from("rdm_out/read_gltf_no_skin2");
        std::fs::create_dir_all(&dir_dst).unwrap();
        exp_rdm.write_rdm(Some(dir_dst), false);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    #[ignore]
    fn matrix_rel_calc() {
        use nalgebra::*;
        let global_parent_translation = Translation3::new(-1.0, 2.0, 0.0);
        let global_child_translation = Translation3::new(1.0, 2.0, 3.0);

        let c: Matrix4<f32> = global_child_translation.to_homogeneous();
        let p: Matrix4<f32> = global_parent_translation.to_homogeneous();

        let local: Matrix4<f32> = p.try_inverse().unwrap() * c;
        println!("{}", local);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    #[ignore]
    fn interpolation_test() {
        use nalgebra::Vector3;

        let input_time = vec![0.0f32, 0.8, 1.6, 2.4, 3.2];
        let output_values = vec![
            Vector3::new(10.0f32, 5.0, -5.0),
            Vector3::new(14.0f32, 3.0, -2.0),
            Vector3::new(18.0f32, 1.0, 1.0),
            Vector3::new(24.0f32, -1.0, 4.0),
            Vector3::new(31.0f32, -3.0, 7.0),
        ];

        assert_eq!(input_time.len(), output_values.len());

        fn interpolate(
            current_time: f32,
            input_time: &[f32],
            output_values: &[Vector3<f32>],
        ) -> Vector3<f32> {
            let next_idx = input_time.iter().position(|t| t > &current_time).unwrap();
            let previous_idx = next_idx - 1;

            let previous_time = input_time[previous_idx];
            let next_time = input_time[next_idx];

            let previous_translation = output_values[previous_idx];
            let next_translation = output_values[next_idx];

            let interpolation_value = (current_time - previous_time) / (next_time - previous_time);

            let current_translation = previous_translation
                + interpolation_value * (next_translation - previous_translation);
            current_translation
        }

        dbg!(interpolate(1.2f32, &&input_time, &output_values));
    }
}
