//! Scene-based glTF export and final file packaging.

#![forbid(unsafe_code)]

use crate::{gltf_buffers::BufferContainer, rdm_material::RdMaterial};
use gltf::json;
use std::{
    borrow::Cow,
    env,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    str::FromStr,
};

mod animation;
mod skin;
mod writer;
pub use writer::{build_scene, GltfWriter};

fn mesh_node(mesh: json::Index<json::Mesh>, name: Option<String>) -> json::Node {
    json::Node {
        camera: None,
        children: None,
        extensions: Default::default(),
        extras: Default::default(),
        matrix: None,
        mesh: Some(mesh),
        name,
        rotation: None,
        scale: None,
        translation: None,
        skin: None,
        weights: None,
    }
}

struct RdGltf {
    buffers: Vec<BufferContainer>,
    root: json::Root,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GltfExportFormat {
    GltfSeparate,
    GltfSeparateMinimise,
    Glb,
}

impl FromStr for GltfExportFormat {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_ascii_lowercase().as_str() {
            "gltf" => Ok(GltfExportFormat::GltfSeparate),
            "gltfm" | "gltfmi" | "gltfmin" => Ok(GltfExportFormat::GltfSeparateMinimise),
            "glb" => Ok(GltfExportFormat::Glb),
            _ => Err(format!(
                "Invalid value for GltfExportFormat: {}, Only gltf/gltfmin/glb are allowed value",
                input
            )),
        }
    }
}

impl RdGltf {
    fn write_gltf(
        mut self,
        dir: Option<PathBuf>,
        optmat: Option<RdMaterial>,
        create_new: bool,
        config: GltfExportFormat,
    ) {
        if config == GltfExportFormat::Glb {
            self.root.buffers[0].uri = None;
        }
        let mut file = dir.unwrap_or_else(|| {
            let f = PathBuf::from("gltf_out");
            let _ = fs::create_dir(&f);
            f
        });

        if file.is_dir() {
            file.push("out");
        }
        if config == GltfExportFormat::Glb {
            file.set_extension("glb");
        } else {
            file.set_extension("gltf");
        }
        info!("Writing glTF {:?}", file);
        let output_path = file.clone();

        let mut writer = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .create_new(create_new)
            .open(&file)
            .expect("I/O error");

        match config {
            GltfExportFormat::Glb => {
                //TODO fix this. Currently Glb writer ignores these values otherwise this would not work.
                let header: gltf::binary::Header = gltf::binary::Header {
                    magic: Default::default(),
                    version: 2,
                    length: 0xDEAD_BEEF,
                };
                let j = json::serialize::to_vec(&self.root).expect("Serialization error");
                let glb = gltf::Glb {
                    header,
                    json: Cow::from(&j),
                    bin: Some(Cow::from(self.buffers[0].get_bytes())),
                };
                glb.to_writer(writer).expect("I/O error");
                debug!("json: {}", glb.json.len());
                debug!("bin: {}", self.buffers[0].get_bytes_len_padded());
            }
            _ => {
                let vjson =
                    json::serialize::to_vec_pretty(&self.root).expect("Serialization error");
                writer.write_all(&vjson).expect("I/O error");

                debug!("wrote json to disk!");

                file.pop();
                let udir = file;

                for (i, bin) in self.buffers.into_iter().enumerate() {
                    let mut file_path = udir.clone();
                    file_path.push(format!("buffer{}.bin", i));
                    debug!("write_all {:?}", file_path);
                    let mut writer = OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .create_new(create_new)
                        .open(&file_path)
                        .expect("I/O error");
                    bin.to_writer(&mut writer).unwrap();
                }

                // copy converted png from tmp to dest
                if let Some(mat) = optmat.as_ref() {
                    let tmp_dir = env::temp_dir();
                    for e in mat.into_iter() {
                        let mut src = tmp_dir.join(e.file_stem().unwrap());
                        src.set_extension("PNG");
                        let mut dst_file = udir.join(e.file_stem().unwrap());
                        dst_file.set_extension("PNG");
                        debug!("copy: {:?} to {:?}", src, dst_file);
                        fs::copy(src, &dst_file).unwrap();
                    }
                }
            }
        }
        info!("Wrote glTF {:?}", output_path);
    }
}
