//! Scene-based glTF export and final file packaging.

#![forbid(unsafe_code)]

use crate::{gltf_buffers::BufferContainer, rdm_material::RdMaterial};
use gltf::json;
use std::{
    borrow::Cow,
    env,
    error::Error,
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
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
        textures: Option<RdMaterial>,
        create_new: bool,
        config: GltfExportFormat,
    ) -> Result<(), Box<dyn Error>> {
        let mut file = match dir {
            Some(path) => path,
            None => {
                let path = PathBuf::from("gltf_out");
                fs::create_dir_all(&path)?;
                path
            }
        };
        if file.is_dir() {
            file.push("out");
        }
        file.set_extension(if config == GltfExportFormat::Glb {
            "glb"
        } else {
            "gltf"
        });
        let open = |path: &Path| {
            OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .create_new(create_new)
                .open(path)
                .map(BufWriter::new)
                .map_err(|error| format!("{}: {error}", path.display()))
        };
        let mut writer = open(&file)?;
        match config {
            GltfExportFormat::Glb => {
                self.root.buffers[0].uri = None;
                let json = json::serialize::to_vec(&self.root)?;
                let bin = self.buffers[0].get_bytes();
                let glb = gltf::Glb {
                    header: gltf::binary::Header {
                        magic: *b"glTF",
                        version: 2,
                        length: u32::try_from(
                            28 + json.len().next_multiple_of(4) + bin.len().next_multiple_of(4),
                        )?,
                    },
                    json: Cow::Borrowed(&json),
                    bin: Some(Cow::Borrowed(bin)),
                };
                glb.to_writer(&mut writer)?;
            }
            _ => {
                json::serialize::to_writer_pretty(&mut writer, &self.root)?;
                let directory = file.parent().expect("output has a parent directory");
                for (i, buffer) in self.buffers.into_iter().enumerate() {
                    let mut writer = open(&directory.join(format!("buffer{i}.bin")))?;
                    buffer.to_writer(&mut writer)?;
                    writer.flush()?;
                }
                if let Some(textures) = &textures {
                    for texture in textures {
                        let name =
                            PathBuf::from(texture.file_stem().expect("texture has a filename"))
                                .with_extension("PNG");
                        fs::copy(env::temp_dir().join(&name), directory.join(&name))?;
                    }
                }
            }
        }
        writer.flush()?;
        Ok(())
    }
}
