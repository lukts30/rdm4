use rdm4lib::RDModell;

use rdm4lib::gltf_export;

use std::fs::File;
use std::process::Command;
use std::str;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fishery_others_lod2() {
        let rdm = RDModell::from("rdm/fishery_others_lod2.rdm");
        assert_eq!(rdm.vertices_count, 3291);
        assert_eq!(rdm.triangles_idx_count, 7473);

        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );
    }

    #[test]
    fn basalt_crusher_others_lod2() {
        let rdm = RDModell::from("rdm/basalt_crusher_others_lod2.rdm");
        assert_eq!(rdm.vertices_count, 2615);

        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );

        gltf_export::build(rdm);

        let output = if cfg!(target_os = "windows") {
            Command::new("..\\gltf_validator.exe")
                .args(&["-a", "triangle/triangle.gltf"])
                .output()
                .expect("failed to execute process")
        } else {
            Command::new("../gltf_validator")
                .args(&["-a", "triangle/triangle.gltf"])
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

        println!("gltf_validator: {:#?}", info);

        assert_eq!(r#"Errors: 0"#, info[0]);
        assert_eq!(r#"Warnings: 0"#, info[1]);

        let mut f = File::open("triangle/triangle.gltf.report.json").unwrap();
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
    }

    #[test]
    fn fishery_others_cutout_lod0() {
        let rdm = RDModell::from("rdm/fishery_others_cutout_lod0.rdm");
        assert_eq!(rdm.vertices_count, 32);
        assert_eq!(rdm.triangles_idx_count, 78);

        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );
    }

    #[test]
    fn ark_waterfall2() {
        let rdm = RDModell::from("rdm/ark_waterfall2.rdm");
        assert_eq!(rdm.vertices_count, 105);

        assert_eq!(
            rdm.triangles_idx_count as usize,
            rdm.triangle_indices.len() * 3
        );
    }
}
