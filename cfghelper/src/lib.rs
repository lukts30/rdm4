pub mod cfghelper {

    use quick_xml::{de::from_str, events::Event, Reader};
    use regex::Regex;
    use serde::{Deserialize, Serialize};
    use std::{fs, path::Path};

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
    pub struct AnnoCfg {
        #[serde(rename = "RenderPropertyFlags", default)]
        render_property_flags: String,

        #[serde(rename = "Models", default)]
        pub models: Models,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
    pub struct Models {
        #[serde(rename = "$value")]
        pub models_vec: Vec<Model>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
    pub struct Model {
        #[serde(rename = "Materials")]
        pub materials: Materials,

        #[serde(rename = "Animations")]
        animations: Option<Animations>,

        #[serde(rename = "FileName", default)]
        pub file_name: String,
        #[serde(rename = "IgnoreRuinState", default)]
        ignore_ruin_state: Option<bool>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
    pub struct Materials {
        #[serde(rename = "$value")]
        pub materials_vec: Vec<Material>,
    }

    #[allow(non_snake_case)]
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
    pub struct Material {
        Name: String,
        ShaderID: u32,
        pub VertexFormat: String,
        NumBonesPerVertex: u32,
        METALLIC_TEX_ENABLED: Option<bool>,
        cModelMetallicTex: Option<String>,
        cUseTerrainTinting: Option<String>,
        SEPARATE_AO_TEXTURE: Option<String>,
        cSeparateAOTex: Option<String>,
        Common: Option<String>,
        DIFFUSE_ENABLED: Option<bool>,
        pub cModelDiffTex: String,
        NORMAL_ENABLED: Option<bool>,
        cModelNormalTex: Option<String>,
        #[serde(rename = "cDiffuseColor.r")]
        cDiffuseColor_r: f32,
        #[serde(rename = "cDiffuseColor.g")]
        cDiffuseColor_g: f32,
        #[serde(rename = "cDiffuseColor.b")]
        cDiffuseColor_b: f32,
        ALPHA_BLEND_ENABLED: Option<bool>,
        cTexScrollSpeed: String,
        DYE_MASK_ENABLED: Option<bool>,
        WATER_CUTOUT_ENABLED: Option<bool>,
        TerrainAdaption: String,
        ADJUST_TO_TERRAIN_HEIGHT: Option<bool>,
        VERTEX_COLORED_TERRAIN_ADAPTION: String,
        ABSOLUTE_TERRAIN_ADAPTION: Option<bool>,
        Environment: String,
        cUseLocalEnvironmentBox: String,
        #[serde(rename = "cEnvironmentBoundingBox.x")]
        cEnvironmentBoundingBox_x: f32,
        #[serde(rename = "cEnvironmentBoundingBox.y")]
        cEnvironmentBoundingBox_y: f32,
        #[serde(rename = "cEnvironmentBoundingBox.z")]
        cEnvironmentBoundingBox_z: f32,
        #[serde(rename = "cEnvironmentBoundingBox.w")]
        cEnvironmentBoundingBox_w: f32,
        Glow: String,
        GLOW_ENABLED: Option<bool>,
        #[serde(rename = "cEmissiveColor.r")]
        cEmissiveColor_r: f32,
        #[serde(rename = "cEmissiveColor.g")]
        cEmissiveColor_g: f32,
        #[serde(rename = "cEmissiveColor.b")]
        cEmissiveColor_b: f32,
        NIGHT_GLOW_ENABLED: Option<bool>,
    }

    #[allow(non_snake_case)]
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
    pub struct Animations {
        #[serde(rename = "$value")]
        animations_vec: Vec<Animation>,
    }

    #[allow(non_snake_case)]
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
    pub struct Animation {
        FileName: String,
        LoopCount: u32,
        Scale: f32,
    }

    pub fn parse_cfg(path: &Path) -> Result<AnnoCfg, Box<dyn std::error::Error + 'static>> {
        let cfg = fs::read_to_string(path)?;

        let mut reader = Reader::from_str(&cfg);
        reader.trim_text(true);

        let mut buf = Vec::new();

        let mut tmp_stack = Vec::new();
        let mut dst_ordered = Vec::new();

        loop {
            match reader.read_event(&mut buf) {
                Ok(Event::Start(ref e)) if e.name() == b"ConfigType" => {
                    let cfg_type = reader.read_text(e.name(), &mut Vec::new()).unwrap();
                    tmp_stack.push(cfg_type);
                }
                Ok(Event::End(ref e)) if e.name() == b"Config" => {
                    dst_ordered.push(tmp_stack.pop().unwrap());
                }
                Ok(Event::Eof) => break,
                Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
                _ => (),
            }
            buf.clear();
        }
        let cfgn = replace_tags(&cfg, dst_ordered);
        Ok(from_str(&cfgn)?)
    }

    fn replace_tags(cfg: &str, types: Vec<String>) -> String {
        let remove_config_open = Regex::new(r"<Config>").unwrap();
        let p0 = remove_config_open.replace_all(cfg, "");

        let match_config_type_to_tag_open =
            Regex::new(r"ConfigType[^>]*>(.*?)</ConfigType").unwrap();
        let p = match_config_type_to_tag_open
            .replace_all(&p0, |captures: &regex::Captures| captures[1].to_string());

        //dbg!(&types);

        let match_config_close_to_tag_close = Regex::new(r"</Config>").unwrap();
        let mut i = 0;
        let p2 = match_config_close_to_tag_close.replace_all(&p, |_captures: &regex::Captures| {
            let tag = format!("</{}>", types[i]);
            i += 1;
            tag
        });
        p2.into_owned()
    }

    #[cfg(test)]
    mod tests {}
}
