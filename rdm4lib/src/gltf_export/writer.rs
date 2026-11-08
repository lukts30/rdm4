//! Scene export directly from typed attributes and bind poses.

#![forbid(unsafe_code)]

use std::{collections::BTreeMap, env, error::Error, fs, path::PathBuf};

use gltf::json::{self, validation::Checked::Valid};

use super::{animation, mesh_node, skin, GltfExportFormat, RdGltf};
use crate::{
    gltf_buffers::{AccessorBuilder, BufferContainer, GltfBuffers},
    rdm_material::RdMaterial,
    scene::{Attribute, AttributeValues, Mesh, Scene},
    vertex::{StorageType, VertexSemantic},
};

/// Export meshes, skeletons and clips, retaining names, draw ranges and key values.
pub fn build_scene(
    scene: Scene,
    dir: Option<PathBuf>,
    create_new: bool,
    config: GltfExportFormat,
) -> Result<(), Box<dyn Error>> {
    GltfWriter::new(scene).write(dir, create_new, config)
}

/// Shared scene exporter with optional external textures and CLI compatibility.
pub struct GltfWriter {
    scene: Scene,
    textures: Option<RdMaterial>,
    legacy: bool,
}

impl GltfWriter {
    /// Retain typed geometry and explicit keyframes until final buffer placement.
    pub fn new(scene: Scene) -> Self {
        Self {
            scene,
            textures: None,
            legacy: false,
        }
    }

    /// Convert external DDS files with the existing Windows texture converter.
    pub fn textures(mut self, textures: RdMaterial) -> Self {
        self.textures = Some(textures);
        self
    }

    /// Preserve the old CLI's selected-model attributes, names and armature root.
    pub fn legacy(mut self) -> Self {
        self.legacy = true;
        self
    }

    /// Build accessors and nodes, then package separate buffers or a GLB.
    pub fn write(
        self,
        dir: Option<PathBuf>,
        create_new: bool,
        config: GltfExportFormat,
    ) -> Result<(), Box<dyn Error>> {
        let Self {
            scene,
            textures,
            legacy,
        } = self;
        if legacy && scene.meshes.len() != 1 {
            return Err("legacy glTF export requires one selected model".into());
        }
        let material_count = scene
            .meshes
            .iter()
            .flat_map(|mesh| &mesh.primitives)
            .map(|primitive| primitive.material)
            .max()
            .ok_or("scene contains no primitives")?;
        let mut binary = GltfBuffers::default();
        let mut meshes = Vec::new();
        let mut nodes = Vec::new();
        let mut roots = Vec::new();
        let mut skins = Vec::new();
        let mut skeleton_nodes = Vec::new();
        for mesh in &scene.meshes {
            mesh.validate()?;
            meshes.push(export_mesh(mesh, &mut binary, legacy)?);
        }
        for skeleton in &scene.skeletons {
            let joints = skeleton
                .joints
                .iter()
                .map(skin::GlobalJoint::from_inverse_bind)
                .collect::<Result<Vec<_>, _>>()?;
            skeleton_nodes.push(nodes.len());
            let output = skin::build(&joints, &mut binary, nodes.len())?;
            nodes.extend(output.nodes);
            roots.extend(output.roots);
            skins.push(output.skin);
        }
        for (mesh_index, mesh) in scene.meshes.into_iter().enumerate() {
            let joint_count = mesh
                .skeleton
                .map(|index| {
                    scene
                        .skeletons
                        .get(index)
                        .map(|skeleton| skeleton.joints.len())
                        .ok_or("mesh skeleton index is out of range")
                })
                .transpose()?;
            let index = json::Index::new(mesh_index as u32);
            if let Some(count) = joint_count {
                let influences = skin::influences(&mesh, count, &mut binary)?;
                for primitive in &mut meshes[index.value()].primitives {
                    primitive.attributes.extend(
                        influences
                            .iter()
                            .map(|(semantic, accessor)| (Valid(semantic.clone()), *accessor)),
                    );
                }
            }
            let mut node = mesh_node(index, if legacy { None } else { mesh.name });
            if legacy && mesh.skeleton.is_some() {
                node.name = Some("armature".into());
                node.children = Some(std::mem::take(&mut roots));
            }
            node.matrix = mesh.transform.map(|matrix| {
                matrix
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .try_into()
                    .expect("4x4 matrix")
            });
            node.skin = mesh.skeleton.map(|i| json::Index::new(i as u32));
            roots.push(json::Index::new(nodes.len() as u32));
            nodes.push(node);
        }
        let animations = scene
            .animations
            .iter()
            .map(|clip| {
                let skeleton = scene
                    .skeletons
                    .get(clip.skeleton)
                    .ok_or("animation skeleton index is out of range")?;
                animation::build(
                    clip,
                    skeleton.joints.len(),
                    skeleton_nodes[clip.skeleton],
                    &mut binary,
                )
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        let mut root = json::Root {
            scenes: vec![json::Scene {
                nodes: roots,
                extensions: Default::default(),
                extras: Default::default(),
                name: None,
            }],
            scene: Some(json::Index::new(0)),
            nodes,
            meshes,
            skins,
            animations,
            materials: (0..=material_count)
                .map(|_| json::Material::default())
                .collect(),
            ..Default::default()
        };
        if let Some(textures) = &textures {
            put_textures(
                textures,
                &mut root,
                &mut binary,
                config == GltfExportFormat::Glb,
            )?;
        }
        let buffers = binary.finish(&mut root, config != GltfExportFormat::GltfSeparate);
        RdGltf { root, buffers }.write_gltf(dir, textures, create_new, config)
    }
}

fn put_textures(
    textures: &RdMaterial,
    root: &mut json::Root,
    binary: &mut GltfBuffers,
    embed: bool,
) -> Result<(), Box<dyn Error>> {
    textures.run_dds_converter(&env::temp_dir());
    for (i, (path, material)) in textures.into_iter().zip(&mut root.materials).enumerate() {
        let name = format!("{}.PNG", path.file_stem().unwrap().to_str().unwrap());
        let mut image = json::Image {
            uri: Some(name.clone()),
            buffer_view: None,
            mime_type: None,
            name: None,
            extensions: None,
            extras: None,
        };
        if embed {
            image.uri = None;
            image.mime_type = Some(json::image::MimeType("image/png".into()));
            image.buffer_view = Some(
                binary
                    .push_buffer_view(BufferContainer(fs::read(env::temp_dir().join(name))?), None),
            );
        }
        root.images.push(image);
        root.samplers.push(Default::default());
        root.textures.push(json::Texture {
            source: json::Index::new(i as u32),
            sampler: Some(json::Index::new(i as u32)),
            name: None,
            extensions: None,
            extras: None,
        });
        material.pbr_metallic_roughness.base_color_texture = Some(json::texture::Info {
            index: json::Index::new(i as u32),
            tex_coord: 0,
            extensions: None,
            extras: None,
        });
    }
    Ok(())
}

fn export_mesh(
    mesh: &Mesh,
    binary: &mut GltfBuffers,
    legacy: bool,
) -> Result<json::Mesh, Box<dyn Error>> {
    use json::{
        accessor::{ComponentType, Type},
        mesh::Semantic,
    };

    let find = |semantic| {
        mesh.attributes
            .iter()
            .find(|attribute| attribute.descriptor.semantic == semantic)
    };
    let positions = find(VertexSemantic::Position)
        .ok_or("mesh has no position attribute")?
        .floats(3)?;
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for position in positions.as_chunks::<3>().0 {
        for axis in 0..3 {
            min[axis] = min[axis].min(position[axis]);
            max[axis] = max[axis].max(position[axis]);
        }
    }
    let position = binary.push_attribute(
        floats(&positions),
        AccessorBuilder::new(mesh.vertex_count, Type::Vec3, ComponentType::F32)
            .bounds(Some(json::Value::from(min)), Some(json::Value::from(max))),
        Some(Valid(json::buffer::Target::ArrayBuffer)),
    );
    let mut attributes = BTreeMap::from([(Valid(Semantic::Positions), position)]);
    let mut indices = Vec::new();
    for primitive in &mesh.primitives {
        let values = &mesh.indices[primitive.indices.clone()];
        let bytes = values
            .iter()
            .flat_map(|index| index.to_le_bytes())
            .collect();
        indices.push(binary.push_attribute(
            BufferContainer(bytes),
            AccessorBuilder::new(values.len(), Type::Scalar, ComponentType::U32),
            Some(Valid(json::buffer::Target::ElementArrayBuffer)),
        ));
    }
    for (set, attribute) in mesh
        .attributes
        .iter()
        .filter(|attribute| attribute.descriptor.semantic == VertexSemantic::Texcoord)
        .take(if legacy { 1 } else { usize::MAX })
        .enumerate()
    {
        let values = attribute.floats(2)?;
        let accessor = binary.push_attribute(
            floats(&values),
            AccessorBuilder::new(mesh.vertex_count, Type::Vec2, ComponentType::F32),
            Some(Valid(json::buffer::Target::ArrayBuffer)),
        );
        attributes.insert(Valid(Semantic::TexCoords(set as u32)), accessor);
    }
    for (source, semantic, width, shape) in [
        (VertexSemantic::Normal, Semantic::Normals, 3, Type::Vec3),
        (VertexSemantic::Tangent, Semantic::Tangents, 3, Type::Vec4),
    ] {
        let Some(attribute) = find(source) else {
            continue;
        };
        let tangent = semantic == Semantic::Tangents;
        let width = if tangent && !legacy && attribute.descriptor.storage_count == 4 {
            4
        } else {
            width
        };
        let mut values = attribute.floats(width)?;
        // Packed vectors already require normalization when decoded. The old CLI
        // also normalized float vectors; direct scene exports keep their bits.
        if legacy && attribute.descriptor.storage_type != StorageType::U32 {
            for vector in values.to_mut().chunks_exact_mut(width) {
                let length = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
                for value in vector {
                    *value /= length;
                }
            }
        }
        let payload = if tangent && width == 3 {
            BufferContainer(
                values
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .flat_map(|xyz| {
                        xyz.iter()
                            .copied()
                            .chain(std::iter::once(1.0))
                            .flat_map(f32::to_le_bytes)
                    })
                    .collect(),
            )
        } else {
            floats(&values)
        };
        let accessor = binary.push_attribute(
            payload,
            AccessorBuilder::new(mesh.vertex_count, shape, ComponentType::F32),
            Some(Valid(json::buffer::Target::ArrayBuffer)),
        );
        attributes.insert(Valid(semantic), accessor);
    }
    for (set, attribute) in mesh
        .attributes
        .iter()
        .filter(|attribute| !legacy && attribute.descriptor.semantic == VertexSemantic::Color)
        .enumerate()
    {
        let accessor = color(attribute, mesh.vertex_count, binary)?;
        attributes.insert(Valid(Semantic::Colors(set as u32)), accessor);
    }
    let primitives = mesh
        .primitives
        .iter()
        .zip(indices)
        .map(|(primitive, indices)| json::mesh::Primitive {
            attributes: attributes.clone(),
            indices: Some(indices),
            material: Some(json::Index::new(primitive.material)),
            mode: Valid(json::mesh::Mode::Triangles),
            extensions: Default::default(),
            extras: Default::default(),
            targets: None,
        })
        .collect();
    Ok(json::Mesh {
        name: if legacy { None } else { mesh.name.clone() },
        primitives,
        extensions: Default::default(),
        extras: Default::default(),
        weights: None,
    })
}

fn color(
    attribute: &Attribute,
    count: usize,
    binary: &mut GltfBuffers,
) -> Result<json::Index<json::Accessor>, Box<dyn Error>> {
    use json::accessor::{ComponentType, Type};
    let components = attribute.descriptor.storage_count;
    let (payload, accessor) = match &attribute.values {
        AttributeValues::U32(values)
            if components == 1 && matches!(attribute.descriptor.interpretation.0, 2 | 4 | 6) =>
        {
            (
                BufferContainer(values.iter().flat_map(|v| v.to_le_bytes()).collect()),
                AccessorBuilder::new(count, Type::Vec4, ComponentType::U8).normalized(),
            )
        }
        AttributeValues::F16(_) | AttributeValues::F32(_) if matches!(components, 3 | 4) => (
            floats(&attribute.floats(components as usize)?),
            AccessorBuilder::new(
                count,
                if components == 3 {
                    Type::Vec3
                } else {
                    Type::Vec4
                },
                ComponentType::F32,
            ),
        ),
        AttributeValues::U16(values) if components == 4 => (
            BufferContainer(values.iter().flat_map(|v| v.to_le_bytes()).collect()),
            AccessorBuilder::new(count, Type::Vec4, ComponentType::U16).normalized(),
        ),
        _ => return Err(format!("unsupported color encoding: {:?}", attribute.descriptor).into()),
    };
    Ok(binary.push_attribute(
        payload,
        accessor,
        Some(Valid(json::buffer::Target::ArrayBuffer)),
    ))
}

fn floats(values: &[f32]) -> BufferContainer {
    BufferContainer(
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect(),
    )
}
