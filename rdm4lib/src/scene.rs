//! Typed conversion data without file framing, source placement or byte-order state.

#![forbid(unsafe_code)]

use std::{borrow::Cow, error::Error, ops::Range};

use crate::vertex::{StorageType, VertexIdentifier, VertexSemantic};

/// Meshes, skeletons and animation clips without file framing or source placement.
#[derive(Default)]
pub struct Scene {
    /// All meshes selected for export.
    pub meshes: Vec<Mesh>,
    /// Skeletons referenced by meshes.
    pub skeletons: Vec<Skeleton>,
    /// Clips bound to individual skeletons.
    pub animations: Vec<Animation>,
}

/// Local joint poses over time, independent of bind poses and file layout.
pub struct Animation {
    /// Clip name.
    pub name: String,
    /// Index into the scene's skeletons.
    pub skeleton: usize,
    /// Nominal source duration; explicit key times remain authoritative.
    pub duration_millis: u32,
    /// Nominal samples per second, independent of explicit key times.
    pub sampling_rate: u32,
    /// Joint tracks in source order.
    pub tracks: Vec<Track>,
}

/// A single joint's local poses.
pub struct Track {
    /// Joint index within the clip's skeleton.
    pub joint: usize,
    /// Explicit keys, retained without resampling.
    pub frames: Vec<Keyframe>,
}

/// One local pose; stored values are not implicitly normalized.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Keyframe {
    /// Rotation in XYZW order.
    pub rotation: [f32; 4],
    /// Translation relative to the parent joint.
    pub translation: [f32; 3],
    /// Time in seconds.
    pub time: f32,
}

/// Geometry shared by scene consumers, retaining the source attribute encoding.
pub struct Mesh {
    /// Optional source model name.
    pub name: Option<String>,
    /// Number of vertices represented by every attribute.
    pub vertex_count: usize,
    /// Typed columns in source descriptor order.
    pub attributes: Vec<Attribute>,
    /// Triangle indices into the attribute columns.
    pub indices: Vec<u32>,
    /// Draw ranges and their material slots.
    pub primitives: Vec<Primitive>,
    /// Optional world transform, in column-major order; geometry stays unbaked.
    pub transform: Option<[[f32; 4]; 4]>,
    /// Optional index into the scene's skeletons.
    pub skeleton: Option<usize>,
}

/// Joints in the order used by vertex joint indices.
pub struct Skeleton {
    /// Bind poses and hierarchy, without implicit normalization.
    pub joints: Vec<Joint>,
}

/// A joint's global inverse bind pose, stored as translation times rotation.
pub struct Joint {
    /// Source joint name.
    pub name: String,
    /// Parent joint index, or no parent for a root.
    pub parent: Option<usize>,
    /// Translation part of the inverse bind transform.
    pub inverse_bind_translation: [f32; 3],
    /// Inverse bind rotation in XYZW order; retained without normalization.
    pub inverse_bind_rotation: [f32; 4],
}

/// One indexed draw range.
pub struct Primitive {
    /// Half-open range of indices, covering complete triangles.
    pub indices: Range<usize>,
    /// Shared material slot.
    pub material: u32,
}

/// An attribute descriptor and its scalar column.
pub struct Attribute {
    /// Semantic, interpretation and component layout.
    pub descriptor: VertexIdentifier,
    /// Native scalar values; packed attributes retain whole numeric words.
    pub values: AttributeValues,
}

/// Scalar values of an attribute, in vertex order.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValues {
    /// Unsigned 16-bit components.
    U16(Vec<u16>),
    /// Unsigned 32-bit components or packed words.
    U32(Vec<u32>),
    /// Half-precision components.
    F16(Vec<half::f16>),
    /// Single-precision components.
    F32(Vec<f32>),
}

impl AttributeValues {
    pub(crate) fn storage_and_len(&self) -> (StorageType, usize) {
        match self {
            Self::U16(v) => (StorageType::U16, v.len()),
            Self::U32(v) => (StorageType::U32, v.len()),
            Self::F16(v) => (StorageType::F16, v.len()),
            Self::F32(v) => (StorageType::F32, v.len()),
        }
    }
}

impl Mesh {
    pub(crate) fn validate(&self) -> Result<(), Box<dyn Error>> {
        if self.vertex_count == 0
            || self.indices.is_empty()
            || self.primitives.is_empty()
            || !self.indices.len().is_multiple_of(3)
            || self
                .indices
                .iter()
                .any(|&i| i as usize >= self.vertex_count)
        {
            return Err("scene mesh requires nonempty triangles with in-range indices".into());
        }
        for primitive in &self.primitives {
            let range = &primitive.indices;
            if range.start >= range.end
                || range.end > self.indices.len()
                || !range.start.is_multiple_of(3)
                || !range.end.is_multiple_of(3)
            {
                return Err(
                    "scene primitive must cover complete triangles within the index buffer".into(),
                );
            }
        }
        for attribute in &self.attributes {
            let d = &attribute.descriptor;
            let (storage, length) = attribute.values.storage_and_len();
            let expected = self.vertex_count.checked_mul(d.storage_count as usize);
            if d.storage_count == 0 || storage != d.storage_type || expected != Some(length) {
                return Err(format!(
                    "{:?}: attribute storage/count disagrees with its descriptor and {} vertices",
                    d.semantic, self.vertex_count
                )
                .into());
            }
        }
        Ok(())
    }
}

impl Attribute {
    /// Borrow unchanged F32 values; allocate only for conversion or dropped components.
    pub(crate) fn floats(&self, width: usize) -> Result<Cow<'_, [f32]>, Box<dyn Error>> {
        let count = self.descriptor.storage_count as usize;
        let result = match &self.values {
            AttributeValues::F32(values) if count == width => Cow::Borrowed(values.as_slice()),
            AttributeValues::F32(values) if count >= width => Cow::Owned(
                values
                    .chunks_exact(count)
                    .flat_map(|v| v[..width].iter().copied())
                    .collect(),
            ),
            AttributeValues::F16(values) if count >= width => Cow::Owned(
                values
                    .chunks_exact(count)
                    .flat_map(|v| v[..width].iter().map(|v| v.to_f32()))
                    .collect(),
            ),
            AttributeValues::U32(values)
                if count == 1
                    && width == 3
                    && self.descriptor.interpretation.0 == 6
                    && matches!(
                        self.descriptor.semantic,
                        VertexSemantic::Normal | VertexSemantic::Tangent
                    ) =>
            {
                Cow::Owned(
                    values
                        .iter()
                        .flat_map(|word| {
                            let bytes = word.to_le_bytes();
                            let vector = [bytes[0], bytes[1], bytes[2]]
                                .map(|v| 2.0 * f32::from(v) / 255.0 - 1.0);
                            let length = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
                            vector.map(|v| v / length)
                        })
                        .collect(),
                )
            }
            _ => {
                return Err(format!(
                    "{:?}: unsupported {:?} x {count} encoding for {width} float components",
                    self.descriptor.semantic, self.descriptor.storage_type
                )
                .into())
            }
        };
        if result.iter().any(|value| !value.is_finite()) {
            return Err(format!(
                "{:?}: glTF requires finite attribute values",
                self.descriptor.semantic
            )
            .into());
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        gltf_export::{build_scene, GltfExportFormat},
        vertex::VertexIdentifier,
    };

    #[test]
    fn exports_two_meshes_with_shared_skeleton_and_retains_bind_values() {
        use crate::rdm::{RdmDocument, RdmReader};
        let mut document = RdmReader::new(std::io::Cursor::new(include_bytes!(
            "../rdm/basalt_crusher_others_lod0.rdm"
        )))
        .unwrap()
        .read_root::<RdmDocument>()
        .unwrap()
        .value;
        let joint = &mut document.skin.as_mut().unwrap().joints[0];
        joint.rotation = [-0.0, 0.0, -0.0, f32::from_bits(0x3f800001)];
        joint.translation = [-0.0, 0.0, -0.0];
        let source_rotation = joint.rotation;
        let source_translation = joint.translation;
        let models = document.models.as_mut().unwrap();
        models.push(models[0].clone());
        let native = RdmDocument::from_scene(
            document.clone().into_scene(true).unwrap(),
            crate::rdm::RdmExportOptions {
                format: crate::vertex::TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b,
                mirror_x: false,
                no_transform: true,
                material_indices: None,
                legacy_metadata: false,
            },
        )
        .unwrap();
        for (actual, expected) in native
            .models
            .as_ref()
            .unwrap()
            .iter()
            .zip(document.models.as_ref().unwrap())
        {
            assert_eq!(actual.vertices, expected.vertices);
            assert_eq!(actual.indices, expected.indices);
            assert_eq!(actual.groups, expected.groups);
        }
        assert_eq!(
            native.skin.unwrap().joints[0].rotation.map(f32::to_bits),
            source_rotation.map(f32::to_bits)
        );
        let mut scene = document.into_scene(true).unwrap();
        assert_eq!(scene.skeletons.len(), 1);
        assert!(scene.meshes.iter().all(|mesh| mesh.skeleton == Some(0)));
        let joint = &scene.skeletons[0].joints[0];
        assert_eq!(
            joint.inverse_bind_translation.map(f32::to_bits),
            source_translation.map(f32::to_bits)
        );
        let [x, y, z, w] = source_rotation;
        assert_eq!(
            joint.inverse_bind_rotation.map(f32::to_bits),
            [-x, -y, -z, w].map(f32::to_bits)
        );
        let joint_count = scene.skeletons[0].joints.len();
        let mut animation = crate::rdm_anim::RdAnim::new(
            include_bytes!("../rdm/basalt_crusher_others_idle01.rdm").to_vec(),
            "working".into(),
        );
        let first = &mut animation.anim_vec[0].frames[0];
        first.rotation = [-0.0, 0.0, -0.0, f32::from_bits(0x3f800001)];
        first.translation = [-0.0, 0.0, -0.0];
        let rotation = [
            first.rotation[0],
            first.rotation[1],
            first.rotation[2],
            -first.rotation[3],
        ];
        let translation = first.translation;
        let animated_joint = animation.anim_vec[0].name.clone();
        let times = animation.anim_vec[0]
            .frames
            .iter()
            .map(|frame| frame.time.to_bits())
            .collect::<Vec<_>>();
        let rate = animation.sampling_rate;
        let names = scene.skeletons[0]
            .joints
            .iter()
            .map(|joint| joint.name.as_str())
            .collect::<Vec<_>>();
        scene
            .animations
            .push(animation.into_scene_animation(0, &names).unwrap());
        let output = tempfile::tempdir().unwrap();
        build_scene(
            scene,
            Some(output.path().into()),
            true,
            GltfExportFormat::Glb,
        )
        .unwrap();
        let (document, buffers, _) = gltf::import(output.path().join("out.glb")).unwrap();
        assert_eq!(document.meshes().count(), 2);
        assert_eq!(document.skins().count(), 1);
        let animation = document.animations().next().unwrap();
        assert_eq!(animation.name(), Some("working"));
        let extras: serde_json::Value =
            serde_json::from_str(animation.extras().as_ref().unwrap().get()).unwrap();
        assert_eq!(extras["rdm4"]["samplingRate"], rate);
        for (channel, expected) in animation
            .channels()
            .take(2)
            .zip([rotation.to_vec(), translation.to_vec()])
        {
            let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
            assert_eq!(
                reader
                    .read_inputs()
                    .unwrap()
                    .map(f32::to_bits)
                    .collect::<Vec<_>>(),
                times
            );
            let values = match reader.read_outputs().unwrap() {
                gltf::animation::util::ReadOutputs::Rotations(values) => {
                    values.into_f32().next().unwrap().to_vec()
                }
                gltf::animation::util::ReadOutputs::Translations(mut values) => {
                    values.next().unwrap().to_vec()
                }
                _ => unreachable!(),
            };
            assert_eq!(
                values.into_iter().map(f32::to_bits).collect::<Vec<_>>(),
                expected.into_iter().map(f32::to_bits).collect::<Vec<_>>()
            );
        }
        let skin = document.skins().next().unwrap();
        assert_eq!(skin.joints().count(), joint_count);
        assert_eq!(
            document
                .nodes()
                .filter(|node| node.skin().is_some())
                .count(),
            2
        );
        for primitive in document.meshes().flat_map(|mesh| mesh.primitives()) {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
            assert!(reader
                .read_weights(0)
                .unwrap()
                .into_f32()
                .all(|w| w == [1.0, 0.0, 0.0, 0.0]));
            assert!(reader
                .read_joints(0)
                .unwrap()
                .into_u16()
                .flatten()
                .all(|i| usize::from(i) < joint_count));
        }
        let imported = crate::gltf_reader::ImportedGltf::load(
            output.path().join("out.glb").as_path(),
            crate::gltf_reader::ResolveNodeName::UniqueName,
        )
        .unwrap()
        .read_active_scene(true, Some(Default::default()))
        .unwrap();
        assert_eq!(imported.meshes.len(), 2);
        assert_eq!(imported.skeletons.len(), 1);
        assert_eq!(imported.animations.len(), 1);
        assert_eq!(imported.animations[0].name, "working");
        assert!(imported.meshes.iter().all(|mesh| mesh.skeleton == Some(0)));
        let target = imported.skeletons[0]
            .joints
            .iter()
            .position(|joint| joint.name == animated_joint)
            .unwrap();
        let track = imported.animations[0]
            .tracks
            .iter()
            .find(|track| track.joint == target)
            .unwrap();
        assert_eq!(
            track.frames[0].rotation.map(f32::to_bits),
            rotation.map(f32::to_bits)
        );
        assert_eq!(
            track.frames[0].translation.map(f32::to_bits),
            translation.map(f32::to_bits)
        );
        assert_eq!(
            track
                .frames
                .iter()
                .map(|frame| frame.time.to_bits())
                .collect::<Vec<_>>(),
            times
        );
        build_scene(
            imported,
            Some(output.path().join("imported.glb")),
            true,
            GltfExportFormat::Glb,
        )
        .unwrap();
        gltf::import(output.path().join("imported.glb")).unwrap();
    }

    #[test]
    fn export_retains_float_attribute_bits_without_normalizing() {
        let positions = [[-0.0f32, 0.0, 0.0], [1.0, -0.0, 0.0], [0.0, 1.0, -0.0]];
        let normal = [f32::from_bits(0x3f800001), -0.0, 0.0];
        let tangent = [-0.0, f32::from_bits(0x3f800001), 0.0, -1.0];
        let texcoords = [[-0.0f32, 0.0], [1.0, -0.0], [0.25, 1.0]];
        let colors = [[0u8, 255, 1, 128], [255, 0, 127, 64], [8, 16, 32, 0]];
        let float_color = [-0.0f32, 0.5, 1.0];
        let mesh = Mesh {
            name: Some("precision".into()),
            vertex_count: 3,
            attributes: vec![
                Attribute {
                    descriptor: VertexIdentifier::p3f(),
                    values: AttributeValues::F32(positions.into_iter().flatten().collect()),
                },
                Attribute {
                    descriptor: VertexIdentifier::n3f(),
                    values: AttributeValues::F32(normal.repeat(3)),
                },
                Attribute {
                    descriptor: VertexIdentifier {
                        storage_count: 4,
                        ..VertexIdentifier::g3f()
                    },
                    values: AttributeValues::F32(tangent.repeat(3)),
                },
                Attribute {
                    descriptor: VertexIdentifier::t2f(),
                    values: AttributeValues::F32(texcoords.into_iter().flatten().collect()),
                },
                Attribute {
                    descriptor: VertexIdentifier::t2h(),
                    values: AttributeValues::F16(
                        texcoords
                            .into_iter()
                            .flatten()
                            .map(half::f16::from_f32)
                            .collect(),
                    ),
                },
                Attribute {
                    descriptor: VertexIdentifier::c4b(),
                    values: AttributeValues::U32(colors.map(u32::from_le_bytes).to_vec()),
                },
                Attribute {
                    descriptor: VertexIdentifier {
                        semantic: VertexSemantic::Color,
                        ..VertexIdentifier::p3f()
                    },
                    values: AttributeValues::F32(float_color.repeat(3)),
                },
            ],
            indices: vec![0, 1, 2],
            primitives: vec![Primitive {
                indices: 0..3,
                material: 0,
            }],
            skeleton: None,
            transform: None,
        };
        let output = tempfile::tempdir().unwrap();
        build_scene(
            Scene {
                meshes: vec![mesh],
                ..Default::default()
            },
            Some(output.path().into()),
            true,
            GltfExportFormat::Glb,
        )
        .unwrap();
        let (document, buffers, _) = gltf::import(output.path().join("out.glb")).unwrap();
        let primitive = document
            .meshes()
            .next()
            .unwrap()
            .primitives()
            .next()
            .unwrap();
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
        assert_eq!(
            reader
                .read_positions()
                .unwrap()
                .map(|p| p.map(f32::to_bits))
                .collect::<Vec<_>>(),
            positions.map(|p| p.map(f32::to_bits))
        );
        for value in reader.read_normals().unwrap() {
            assert_eq!(value.map(f32::to_bits), normal.map(f32::to_bits));
        }
        for value in reader.read_tangents().unwrap() {
            assert_eq!(value.map(f32::to_bits), tangent.map(f32::to_bits));
        }
        for set in 0..2 {
            assert_eq!(
                reader
                    .read_tex_coords(set)
                    .unwrap()
                    .into_f32()
                    .map(|v| v.map(f32::to_bits))
                    .collect::<Vec<_>>(),
                texcoords.map(|v| v.map(f32::to_bits))
            );
        }
        assert!(primitive
            .get(&gltf::Semantic::Colors(0))
            .unwrap()
            .normalized());
        assert_eq!(
            reader
                .read_colors(0)
                .unwrap()
                .into_rgba_u8()
                .collect::<Vec<_>>(),
            colors
        );
        for value in reader.read_colors(1).unwrap().into_rgb_f32() {
            assert_eq!(value.map(f32::to_bits), float_color.map(f32::to_bits));
        }
        let imported =
            crate::gltf_reader::ImportedGltf::try_from(output.path().join("out.glb").as_path())
                .unwrap()
                .read_scene(false, None)
                .unwrap();
        let mesh = &imported.meshes[0];
        for (semantic, width, expected) in [
            (
                VertexSemantic::Position,
                3,
                positions.into_iter().flatten().collect::<Vec<_>>(),
            ),
            (VertexSemantic::Normal, 3, normal.repeat(3)),
            (VertexSemantic::Tangent, 4, tangent.repeat(3)),
        ] {
            let values = mesh
                .attributes
                .iter()
                .find(|attribute| attribute.descriptor.semantic == semantic)
                .unwrap()
                .floats(width)
                .unwrap();
            assert_eq!(
                values.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                expected.into_iter().map(f32::to_bits).collect::<Vec<_>>()
            );
        }
    }
}
