//! Import typed scene data. RDM packing and coordinate conversion live in `rdm`.

#![forbid(unsafe_code)]

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    error::Error,
    path::Path,
    str::FromStr,
};

use gltf::{mesh::Semantic, Node};
use nalgebra::{Isometry3, Matrix4, Similarity3};

use crate::{
    scene::{Attribute, AttributeValues, Joint, Mesh, Primitive, Scene, Skeleton},
    vertex::{VertexIdentifier, VertexInterpretation, VertexSemantic},
};

mod animation;
pub use animation::AnimationImportOptions;

/// Naming policy retained for the CLI and Blender addon.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ResolveNodeName {
    UnstableIndex,
    UniqueName,
}

impl FromStr for ResolveNodeName {
    type Err = String;
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_ascii_lowercase().as_str() {
            "uniquename" | "n" => Ok(Self::UniqueName),
            "unstableindex" | "i" => Ok(Self::UnstableIndex),
            _ => Err(format!("Invalid value for ResolveNodeName: {input}")),
        }
    }
}

/// Loaded glTF document and accessor buffers, with optional mesh selection.
pub struct ImportedGltf {
    gltf: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    /// Policy for joint names.
    pub name_setting: ResolveNodeName,
    selected_node: Option<usize>,
}

impl TryFrom<&Path> for ImportedGltf {
    type Error = gltf::Error;
    fn try_from(path: &Path) -> Result<Self, Self::Error> {
        Self::try_import(path, 0, ResolveNodeName::UniqueName)
    }
}

impl ImportedGltf {
    /// Load a document and select the first node instantiating the requested mesh.
    pub fn try_import(
        path: &Path,
        mesh_idx: u32,
        joint_name_src: ResolveNodeName,
    ) -> Result<Self, gltf::Error> {
        let mut result = Self::load(path, joint_name_src)?;
        result.change_mesh_index(mesh_idx);
        Ok(result)
    }

    /// Load without selecting a mesh, allowing all mesh nodes in the active scene.
    pub fn load(path: &Path, joint_name_src: ResolveNodeName) -> Result<Self, gltf::Error> {
        let (gltf, buffers, _) = gltf::import(path)?;
        Ok(Self {
            gltf,
            buffers,
            name_setting: joint_name_src,
            selected_node: None,
        })
    }

    /// Select a mesh using the legacy first-instance behavior.
    pub fn change_mesh_index(&mut self, index: u32) {
        self.select_mesh(index).expect("selected mesh has no node");
    }

    /// Select a mesh definition's first instance, reporting absent indices.
    pub fn select_mesh(&mut self, index: u32) -> Result<(), Box<dyn Error>> {
        let node = self
            .gltf
            .nodes()
            .find(|node| {
                node.mesh()
                    .is_some_and(|mesh| mesh.index() == index as usize)
            })
            .ok_or_else(|| format!("mesh index {index} has no node"))?;
        self.selected_node = Some(node.index());
        Ok(())
    }

    fn check_node_name_uniqueness(&self, joints: &[usize]) {
        if self.name_setting == ResolveNodeName::UniqueName {
            let names: HashSet<_> = self
                .gltf
                .nodes()
                .filter(|node| joints.contains(&node.index()))
                .map(|node| {
                    node.name()
                        .expect("joint name missing; use --gltf-node-joint-name-src=UnstableIndex")
                })
                .collect();
            assert_eq!(
                names.len(),
                joints.len(),
                "node names must be unique; use --gltf-node-joint-name-src=UnstableIndex"
            );
        }
    }

    fn node_get_name(&self, node: &Node) -> String {
        match self.name_setting {
            ResolveNodeName::UnstableIndex => format!("UnnamedGltfNode{}", node.index()),
            ResolveNodeName::UniqueName => node.name().expect("node name was checked").into(),
        }
    }

    /// Import the selected mesh, optionally its skeleton and clips. Values remain
    /// unquantized; the world transform is stored separately from the attributes.
    pub fn read_scene(
        &self,
        load_skin: bool,
        animations: Option<AnimationImportOptions>,
    ) -> Result<Scene, Box<dyn Error>> {
        let node = self.selected_node.ok_or("no mesh selected")?;
        if load_skin && self.gltf.nodes().nth(node).unwrap().skin().is_none() {
            return Err("requested skeleton is absent".into());
        }
        self.read_nodes(&[node], load_skin, animations)
    }

    /// Import every mesh node in the default glTF scene, or the first scene if
    /// none is marked default. Repeated mesh instances retain their own transforms.
    pub fn read_active_scene(
        &self,
        load_skin: bool,
        animations: Option<AnimationImportOptions>,
    ) -> Result<Scene, Box<dyn Error>> {
        let active = self
            .gltf
            .default_scene()
            .or_else(|| self.gltf.scenes().next())
            .ok_or("input has no glTF scene")?;
        let mut pending = active.nodes().collect::<Vec<_>>();
        pending.reverse();
        let mut visited = HashSet::new();
        let mut meshes = Vec::new();
        while let Some(node) = pending.pop() {
            if !visited.insert(node.index()) {
                return Err("glTF scene contains repeated or cyclic node references".into());
            }
            if node.mesh().is_some() {
                meshes.push(node.index());
            }
            let mut children = node.children().collect::<Vec<_>>();
            children.reverse();
            pending.extend(children);
        }
        if meshes.is_empty() {
            return Err("active glTF scene contains no mesh nodes".into());
        }
        self.read_nodes(&meshes, load_skin, animations)
    }

    fn read_nodes(
        &self,
        nodes: &[usize],
        load_skin: bool,
        animations: Option<AnimationImportOptions>,
    ) -> Result<Scene, Box<dyn Error>> {
        let mut scene = Scene::default();
        let mut skins = HashMap::new();
        let mut joint_nodes = Vec::new();
        for &node_index in nodes {
            let node = self.gltf.nodes().nth(node_index).expect("mesh node exists");
            let skeleton = if let Some(skin) = node.skin().filter(|_| load_skin) {
                let index = if let Some(&index) = skins.get(&skin.index()) {
                    index
                } else {
                    let (skeleton, source_nodes) = self.read_skin(node_index)?;
                    let index = scene.skeletons.len();
                    scene.skeletons.push(skeleton);
                    joint_nodes.push(source_nodes);
                    skins.insert(skin.index(), index);
                    index
                };
                Some(index)
            } else {
                None
            };
            scene.meshes.push(self.read_mesh(node_index, skeleton)?);
        }
        if load_skin && scene.skeletons.is_empty() {
            return Err("requested skeleton is absent".into());
        }
        if let Some(options) = animations {
            if !load_skin {
                return Err("animation import requires a skeleton".into());
            }
            for (index, skeleton) in scene.skeletons.iter().enumerate() {
                let names = skeleton
                    .joints
                    .iter()
                    .map(|joint| joint.name.as_str())
                    .collect::<Vec<_>>();
                scene.animations.extend(self.read_clips(
                    &names,
                    index,
                    &joint_nodes[index],
                    options,
                )?);
            }
        }
        Ok(scene)
    }

    fn read_mesh(
        &self,
        node_index: usize,
        skeleton: Option<usize>,
    ) -> Result<Mesh, Box<dyn Error>> {
        let node = self.gltf.nodes().nth(node_index).expect("mesh node exists");
        let mesh = node.mesh().ok_or("node has no mesh")?;
        let mut semantics: BTreeSet<_> = mesh
            .primitives()
            .flat_map(|primitive| primitive.attributes().map(|(semantic, _)| semantic))
            .collect();
        // Scene columns use dense set order; fill absent lower sets without renumbering.
        for semantic in semantics.clone() {
            let (set, constructor): (u32, fn(u32) -> Semantic) = match semantic {
                Semantic::TexCoords(set) => (set, Semantic::TexCoords),
                Semantic::Colors(set) => (set, Semantic::Colors),
                Semantic::Joints(set) => (set, Semantic::Joints),
                Semantic::Weights(set) => (set, Semantic::Weights),
                _ => continue,
            };
            for lower in 0..set {
                semantics.insert(constructor(lower));
            }
        }
        let mut attributes: Vec<Attribute> = Vec::new();
        let mut indices = Vec::new();
        let mut primitives = Vec::new();
        let mut shared = HashMap::new();
        let mut vertex_count = 0;
        for primitive in mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                return Err("only triangle primitives are supported".into());
            }
            let reader = primitive.reader(|buffer| Some(&self.buffers[buffer.index()]));
            let count = reader
                .read_positions()
                .ok_or("mesh has no positions")?
                .len();
            let key: Vec<_> = primitive
                .attributes()
                .map(|(semantic, accessor)| (semantic, accessor.index()))
                .collect();
            let offset = if let Some(&offset) = shared.get(&key) {
                offset
            } else {
                let mut columns = Vec::new();
                for semantic in &semantics {
                    let (role, width, values) = match *semantic {
                        Semantic::Extras(_) => continue,
                        Semantic::Positions => (
                            VertexSemantic::Position,
                            3,
                            AttributeValues::F32(
                                reader.read_positions().unwrap().flatten().collect(),
                            ),
                        ),
                        Semantic::Normals => (
                            VertexSemantic::Normal,
                            3,
                            AttributeValues::F32(
                                reader
                                    .read_normals()
                                    .map(|v| v.flatten().collect())
                                    .unwrap_or_else(|| [0.0, 0.0, 1.0].repeat(count)),
                            ),
                        ),
                        Semantic::Tangents => (
                            VertexSemantic::Tangent,
                            4,
                            AttributeValues::F32(
                                reader
                                    .read_tangents()
                                    .map(|v| v.flatten().collect())
                                    .unwrap_or_else(|| [0.0, 0.0, 0.0, 1.0].repeat(count)),
                            ),
                        ),
                        Semantic::TexCoords(set) => (
                            VertexSemantic::Texcoord,
                            2,
                            AttributeValues::F32(
                                reader
                                    .read_tex_coords(set)
                                    .map(|v| v.into_f32().flatten().collect())
                                    .unwrap_or_else(|| vec![0.0; count * 2]),
                            ),
                        ),
                        Semantic::Colors(set) => {
                            let types = mesh
                                .primitives()
                                .filter_map(|p| p.get(semantic))
                                .map(|a| a.data_type())
                                .collect::<Vec<_>>();
                            let component = types
                                .first()
                                .copied()
                                .filter(|first| types.iter().all(|value| value == first))
                                // Match legacy per-primitive byte conversion for mixed
                                // sources; U16 -> F32 -> U8 changes truncation results.
                                .unwrap_or(gltf::accessor::DataType::U8);
                            match component {
                                gltf::accessor::DataType::U8 => (
                                    VertexSemantic::Color,
                                    1,
                                    AttributeValues::U32(
                                        reader
                                            .read_colors(set)
                                            .map(|v| {
                                                v.into_rgba_u8().map(u32::from_le_bytes).collect()
                                            })
                                            .unwrap_or_else(|| {
                                                vec![u32::from_le_bytes([255, 255, 255, 0]); count]
                                            }),
                                    ),
                                ),
                                gltf::accessor::DataType::U16 => (
                                    VertexSemantic::Color,
                                    4,
                                    AttributeValues::U16(
                                        reader
                                            .read_colors(set)
                                            .map(|v| v.into_rgba_u16().flatten().collect())
                                            .unwrap_or_else(|| {
                                                [u16::MAX, u16::MAX, u16::MAX, 0].repeat(count)
                                            }),
                                    ),
                                ),
                                _ => (
                                    VertexSemantic::Color,
                                    4,
                                    AttributeValues::F32(
                                        reader
                                            .read_colors(set)
                                            .map(|v| v.into_rgba_f32().flatten().collect())
                                            .unwrap_or_else(|| [1.0, 1.0, 1.0, 0.0].repeat(count)),
                                    ),
                                ),
                            }
                        }
                        Semantic::Joints(set) => (
                            VertexSemantic::JointIndex,
                            4,
                            AttributeValues::U16(
                                reader
                                    .read_joints(set)
                                    .map(|v| v.into_u16().flatten().collect())
                                    .unwrap_or_else(|| vec![0; count * 4]),
                            ),
                        ),
                        Semantic::Weights(set) => (
                            VertexSemantic::Weight,
                            4,
                            AttributeValues::F32(
                                reader
                                    .read_weights(set)
                                    .map(|v| v.into_f32().flatten().collect())
                                    .unwrap_or_else(|| vec![0.0; count * 4]),
                            ),
                        ),
                    };
                    let storage = values.storage_and_len().0;
                    columns.push(Attribute {
                        descriptor: VertexIdentifier {
                            semantic: role.clone(),
                            storage_type: storage,
                            storage_count: width,
                            interpretation: VertexInterpretation(
                                if role == VertexSemantic::Color
                                    && storage == crate::vertex::StorageType::U32
                                {
                                    4
                                } else {
                                    0
                                },
                            ),
                        },
                        values,
                    });
                }
                if attributes.is_empty() {
                    attributes = columns;
                } else {
                    for (column, source) in attributes.iter_mut().zip(columns) {
                        match (&mut column.values, source.values) {
                            (AttributeValues::F32(a), AttributeValues::F32(mut b)) => {
                                a.append(&mut b)
                            }
                            (AttributeValues::U16(a), AttributeValues::U16(mut b)) => {
                                a.append(&mut b)
                            }
                            (AttributeValues::U32(a), AttributeValues::U32(mut b)) => {
                                a.append(&mut b)
                            }
                            _ => return Err("attribute encodings differ between primitives".into()),
                        }
                    }
                }
                shared.insert(key, vertex_count);
                let offset = vertex_count;
                vertex_count += count;
                offset
            };
            let start = indices.len();
            let local: Vec<_> = reader
                .read_indices()
                .map(|v| v.into_u32().collect())
                .unwrap_or_else(|| (0..count as u32).collect());
            if local.iter().any(|&index| index as usize >= count) {
                return Err("primitive index exceeds its vertex count".into());
            }
            indices.extend(local.into_iter().map(|index| index + offset as u32));
            primitives.push(Primitive {
                indices: start..indices.len(),
                material: primitive.material().index().unwrap_or(0) as u32,
            });
        }
        let transform = build_transform2(&self.gltf, node_index);
        let result = Mesh {
            name: node.name().or_else(|| mesh.name()).map(str::to_owned),
            vertex_count,
            attributes,
            indices,
            primitives,
            skeleton,
            transform: Some(transform.into()),
        };
        result.validate()?;
        Ok(result)
    }

    fn read_skin(&self, node_index: usize) -> Result<(Skeleton, Vec<usize>), Box<dyn Error>> {
        let poses = self.read_bind_poses_for(node_index)?;
        let source_nodes = poses.iter().map(|(node, _, _)| *node).collect();
        let joints = poses
            .into_iter()
            .map(|(node, parent, pose)| {
                let rotation =
                    nalgebra::UnitQuaternion::from_quaternion(*pose.rotation.quaternion());
                let translation = rotation
                    .inverse_transform_vector(&pose.translation.vector)
                    .scale(-1.0);
                let q = rotation.coords;
                Joint {
                    name: self.node_get_name(&self.gltf.nodes().nth(node).unwrap()),
                    parent,
                    inverse_bind_translation: translation.into(),
                    inverse_bind_rotation: [-q.x, -q.y, -q.z, q.w],
                }
            })
            .collect();
        Ok((Skeleton { joints }, source_nodes))
    }

    fn read_bind_poses_for(&self, node_index: usize) -> Result<Vec<BindPose>, Box<dyn Error>> {
        let node = self.gltf.nodes().nth(node_index).expect("mesh node exists");
        let skin = node.skin().ok_or("requested skeleton is absent")?;
        let mut nodes: Vec<_> = skin.joints().map(|node| node.index()).collect();
        let mut parents = vec![None; self.gltf.nodes().len()];
        for node in self.gltf.nodes() {
            for child in node.children() {
                parents[child.index()] = Some(node.index());
            }
        }
        let reader = skin.reader(|buffer| Some(&self.buffers[buffer.index()]));
        let matrices: Vec<_> = reader
            .read_inverse_bind_matrices()
            .map(|v| v.collect())
            .unwrap_or_else(|| vec![Matrix4::identity().into(); nodes.len()]);
        if matrices.len() < nodes.len() {
            return Err("inverse bind matrix count disagrees with skin joints".into());
        }
        let mut binds = matrices
            .into_iter()
            .take(nodes.len())
            .map(|matrix| {
                Matrix4::from(matrix)
                    .try_inverse()
                    .ok_or_else(|| "singular inverse bind matrix".into())
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        // Keep skin indices stable and append ancestors in the legacy promotion order.
        let mut cursor = 0;
        while cursor < nodes.len() {
            let end = nodes.len();
            for i in cursor..end {
                if let Some(parent) = parents[nodes[i]] {
                    if !nodes.contains(&parent) {
                        nodes.push(parent);
                        binds.push(build_transform2(&self.gltf, parent));
                    }
                }
            }
            cursor = end;
        }
        self.check_node_name_uniqueness(&nodes);
        nodes
            .iter()
            .zip(binds)
            .map(|(&node, mut bind)| {
                bind.m44 = 1.0;
                let similarity: Similarity3<f32> =
                    nalgebra::try_convert(bind).ok_or("joint transform is not rigid")?;
                Ok((
                    node,
                    parents[node].and_then(|parent| nodes.iter().position(|&node| node == parent)),
                    similarity.isometry,
                ))
            })
            .collect()
    }
}

type BindPose = (usize, Option<usize>, Isometry3<f32>);

fn node_get_local_transform(node: &Node) -> Isometry3<f32> {
    let mut matrix = Matrix4::from(node.transform().matrix());
    matrix.m44 = 1.0;
    let similarity: Similarity3<f32> =
        nalgebra::try_convert(matrix).expect("joint local transform is not rigid");
    let mut pose = similarity.isometry;
    pose.rotation = pose.rotation.inverse();
    pose
}

fn build_transform2(document: &gltf::Document, node: usize) -> Matrix4<f32> {
    let mut chain = Vec::new();
    let mut current = node;
    while let Some(parent) = document
        .nodes()
        .find(|parent| parent.children().any(|child| child.index() == current))
    {
        chain.push(parent.index());
        current = parent.index();
        assert!(
            chain.len() <= document.nodes().len(),
            "cyclic node hierarchy"
        );
    }
    let mut matrix = Matrix4::<f32>::identity();
    for parent in chain.into_iter().rev() {
        matrix *= Matrix4::from(document.nodes().nth(parent).unwrap().transform().matrix());
    }
    matrix * Matrix4::from(document.nodes().nth(node).unwrap().transform().matrix())
}
