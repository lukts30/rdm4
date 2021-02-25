use rdm4lib::RdModell;

use rdm4lib::gltf_export;
use rdm4lib::rdm_anim::RdAnim;
use rdm4lib::rdm_writer::RdWriter;

use rdm4lib::rdm_anim_writer::RdAnimWriter;

use rdm4lib::gltf_reader;

use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::str;

#[cfg(test)]
mod tests {
    use super::*;
    use rdm4lib::{
        gltf_export::GltfExportFormat, rdm_material::RdMaterial, vertex::TargetVertexFormat,
    };
    use std::{fs, path::PathBuf};

    #[test]
    #[cfg_attr(miri, ignore)]
    fn fishery_others_lod2() {
        let rdm = RdModell::from("rdm/fishery_others_lod2.rdm");
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h");
        assert_eq!(rdm.vertex.len(), 3291);
        assert_eq!(rdm.triangles_idx_count, 7473);
        assert_eq!(rdm.mesh_info.len(), 2);

        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn basalt_crusher_others_lod2() {
        let mut rdm = RdModell::from("rdm/basalt_crusher_others_lod2.rdm");
        assert_eq!(rdm.vertex.len(), 2615);
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h_I4b");
        assert_eq!(rdm.vertex.get_size(), 28);
        assert_eq!(rdm.mesh_info.len(), 1);

        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );

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
        // assert_eq!(r#"Warnings: 0"#, info[1]);
        assert_eq!(
            true,
            r#"Warnings: 0"# == info[1] || r#"Warnings: 1"# == info[1]
        );

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
            true,
            v["issues"]["numWarnings"]
                .to_string()
                .parse::<u32>()
                .unwrap()
                == 0
                || v["issues"]["numWarnings"]
                    .to_string()
                    .parse::<u32>()
                    .unwrap()
                    == 1
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    #[cfg(target_os = "windows")]
    fn excavator_tycoons_lod1() {
        let mut rdm = RdModell::from("rdm/excavator_tycoons_lod1.rdm");
        rdm.mat = Some(RdMaterial::new(Path::new(
            r"rdm/excavator_tycoons_diff_0.dds",
        )));
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
        rdm.mat = Some(RdMaterial {
            c_model_diff_tex: vec![
                PathBuf::from("rdm/residence_tier02_04_diff_0.dds"),
                PathBuf::from("rdm/residence_02_05_diff_0.dds"),
                PathBuf::from("rdm/brick_wall_white_estate_01_diff_0.dds"),
            ],
        });
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
        assert_eq!(rdm.triangles_idx_count, 78);
        assert_eq!(rdm.vertex.to_string(), "P4h");
        assert_eq!(rdm.mesh_info.len(), 1);

        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn ark_waterfall2() {
        let rdm = RdModell::from("rdm/ark_waterfall2.rdm");
        assert_eq!(rdm.vertex.len(), 105);
        // TODO: cfg says P4h_N4b_T2h_C4c
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_T2h_C4b");
        assert_eq!(rdm.mesh_info.len(), 1);

        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn exp_rdm_inv_basalt_crusher_others_lod0() {
        let mut rdm = RdModell::from("rdm/basalt_crusher_others_lod2.rdm");
        rdm.add_skin();
        assert_eq!(rdm.vertex.len(), 2615);
        assert_eq!(rdm.vertex.to_string(), "P4h_N4b_G4b_B4b_T2h_I4b");
        assert_eq!(rdm.mesh_info.len(), 1);

        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );

        let exp_rdm = RdWriter::from(rdm);
        let dir_dst = PathBuf::from("rdm_out/basalt_crusher");
        std::fs::create_dir_all(&dir_dst).unwrap();
        exp_rdm.write_rdm(Some(dir_dst), false);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn read_gltf() {
        let rdm = gltf_reader::load_gltf(
            Path::new("rdm/gltf/stormtrooper_with_tangent.gltf"),
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b,
            true,
            false,
            false,
            None,
        );
        assert_eq!(rdm.vertex.len(), 5184);
        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );

        let exp_rdm = RdWriter::from(rdm);
        let dir_dst = PathBuf::from("rdm_out/stormtrooper");
        std::fs::create_dir_all(&dir_dst).unwrap();
        exp_rdm.write_rdm(Some(dir_dst), false);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn read_gltf_anim() {
        let f_path = Path::new("rdm/gltf/stormtrooper_with_tangent.gltf");
        let rdm = gltf_reader::load_gltf(
            &f_path,
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b,
            true,
            false,
            false,
            None,
        );
        assert_eq!(rdm.vertex.len(), 5184);
        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );

        let jj = &rdm.joints.unwrap();
        let mut anims = gltf_reader::read_animation(&f_path, &jj, 6, 0.33333).unwrap();

        assert_eq!(anims.len(), 1);
        let anim = anims.pop().unwrap();
        let exp_rdm = RdAnimWriter::from(anim);
        let dir_dst = PathBuf::from("rdm_out/stormtrooper");
        std::fs::create_dir_all(&dir_dst).unwrap();
        exp_rdm.write_anim_rdm(Some(dir_dst), false);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn read_gltf_no_skin() {
        let rdm = gltf_reader::load_gltf(
            Path::new("rdm/gltf/stormtrooper.gltf"),
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h,
            false,
            false,
            false,
            None,
        );
        assert_eq!(rdm.vertex.len(), 5184);
        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );

        let exp_rdm = RdWriter::from(rdm);
        let dir_dst = PathBuf::from("rdm_out/read_gltf_no_skin");
        std::fs::create_dir_all(&dir_dst).unwrap();
        exp_rdm.write_rdm(Some(dir_dst), false);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    #[ignore]
    fn read_gltf_no_skin2() {
        // no normals so ignore it !
        let rdm = gltf_reader::load_gltf(
            Path::new("rdm/gltf/triangle.gltf"),
            TargetVertexFormat::P4h_N4b_G4b_B4b_T2h,
            false,
            false,
            false,
            None,
        );
        assert_eq!(rdm.vertex.len(), 3);
        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );

        let exp_rdm = RdWriter::from(rdm);

        let dir_dst = PathBuf::from("rdm_out/read_gltf_no_skin2");
        std::fs::create_dir_all(&dir_dst).unwrap();
        exp_rdm.write_rdm(Some(dir_dst), false);
    }
}
