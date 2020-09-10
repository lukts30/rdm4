#[cfg(test)]
mod cfg_xml_tests {
    use cfg::AnnoCfg;
    use cfghelper::cfghelper as cfg;
    use quick_xml::*;
    use std::{fs, path::Path};

    #[test]
    #[cfg_attr(miri, ignore)]
    fn battle_cruiser() {
        let battle_cruiser_cfg: AnnoCfg =
            cfg::parse_cfg(Path::new("tests/cfgs/battle_cruiser.cfg")).unwrap();

        dbg!(&battle_cruiser_cfg);
        assert_eq!(
            &battle_cruiser_cfg.models.models_vec[0].file_name,
            "data\\graphics\\vehicle\\battle_cruiser\\rdm\\battle_cruiser_lod0.rdm"
        );
        assert_eq!(
            &battle_cruiser_cfg.models.models_vec[0]
                .materials
                .materials_vec[0]
                .cModelDiffTex,
            "data/graphics/vehicle/battle_cruiser/maps/battle_cruiser_diff.psd"
        );
        assert_eq!(
            &battle_cruiser_cfg.models.models_vec[1]
                .materials
                .materials_vec[0]
                .VertexFormat,
            "P4h_N4b_G4b_B4b_T2h_I4b"
        );
        let out = se::to_string(&battle_cruiser_cfg).unwrap();

        let expected = fs::read_to_string("tests/cfgs/expected/battle_cruiser.cfgn").unwrap();
        assert_eq!(out, expected);
        // https://github.com/tafia/quick-xml/issues/187
        // notice the serialized xml gets spammed with <$value> & </$value> tags.
    }
}
