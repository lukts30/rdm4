//! Adapters to the existing glTF conversion types.

use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use nalgebra::{Quaternion, UnitQuaternion, Vector3};

use super::{
    buffers::{vertex_bytes, vertices_from_bytes},
    RdmAnimation, RdmDocument, RdmEndian, RdmExportInfo, RdmIndexFormat, RdmIndices, RdmJoint,
    RdmMaterial, RdmMaterialEntry, RdmMeshGroup, RdmModel, RdmModelName, RdmSkin, RdmTrack,
    RdmVertexFormat, RdmWriter,
};
use crate::{
    rdm_anim::{FrameCollection, RdAnim},
    scene::{Animation, Attribute, Joint, Keyframe, Mesh, Primitive, Scene, Skeleton, Track},
    vertex::{StorageType, VertexFormat2},
    MeshInfo, RdJoint, RdModell, Triangle,
};

/// Compatibility entrypoint backed by the current document writer.
pub struct RdWriter2 {
    inner: RdmDocument,
}

impl RdWriter2 {
    /// Write a mesh, retaining the existing output path and overwrite behavior.
    pub fn write_rdm(self, dir: Option<PathBuf>, create_new: bool) -> PathBuf {
        self.try_write_rdm(dir, create_new)
            .expect("cannot write RDM output")
    }

    /// Write a mesh, returning its output path or a file/record error.
    pub fn try_write_rdm(
        self,
        dir: Option<PathBuf>,
        create_new: bool,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let mut file = output_path(dir)?;
        if file.is_dir() {
            file.push("out.rdm");
        }
        write_document(&self.inner, &file, create_new)?;
        Ok(file)
    }

    /// Adapt the model for the current record writer.
    pub fn new(rdm_in: RdModell) -> Self {
        Self {
            inner: RdmDocument::try_from(rdm_in).expect("invalid export model"),
        }
    }
}

/// Compatibility entrypoint backed by the current document writer.
pub struct RdAnimWriter2 {
    name: String,
    export: RdmDocument,
}

impl RdAnimWriter2 {
    /// Write an animation, retaining filename-based naming and overwrite behavior.
    pub fn write_anim_rdm(self, dir: Option<PathBuf>, create_new: bool) {
        self.try_write_anim_rdm(dir, create_new)
            .expect("cannot write animation output");
    }

    /// Write an animation, returning its output path or a file/record error.
    pub fn try_write_anim_rdm(
        self,
        dir: Option<PathBuf>,
        create_new: bool,
    ) -> Result<PathBuf, Box<dyn Error>> {
        let mut file = output_path(dir)?;
        if file.is_dir() {
            file.push(self.name);
        } else {
            let n = file.file_stem().unwrap();
            let anim_name = format!("{}_{}", n.to_string_lossy(), self.name);
            file.set_file_name(anim_name);
        }
        file.set_extension("rdm");
        write_document(&self.export, &file, create_new)?;
        Ok(file)
    }

    /// Adapt the animation for the current record writer.
    pub fn new(anim_input: RdAnim) -> Self {
        Self {
            name: anim_input.name.clone(),
            export: anim_input.into(),
        }
    }
}

fn output_path(dir: Option<PathBuf>) -> std::io::Result<PathBuf> {
    if let Some(path) = dir {
        return Ok(path);
    }
    let path = PathBuf::from("rdm_out");
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn write_document(
    document: &RdmDocument,
    file: &Path,
    create_new: bool,
) -> Result<(), Box<dyn Error>> {
    info!("Writing RDM {:?}", file);
    let writer = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .create_new(create_new)
        .open(file)
        .map_err(|error| format!("{}: {error}", file.display()))?;
    RdmWriter::new(BufWriter::new(writer), RdmEndian::Little)
        .write_root(document)?
        .flush()?;
    info!("Wrote RDM {:?}", file);
    Ok(())
}

impl TryFrom<RdModell> for RdmDocument {
    type Error = Box<dyn Error>;

    /// Build a fresh mesh document with the existing exporter's metadata defaults.
    fn try_from(model: RdModell) -> Result<Self, Self::Error> {
        let with_skin = model.has_skin();
        let format = RdmVertexFormat {
            attributes: model.vertex.identifiers.to_vec(),
            selector: u32::from(with_skin),
        };
        let vertices = vertices_from_bytes(
            model.vertex.as_bytes(),
            model.vertex.len() as usize,
            &format,
            RdmEndian::Little,
            0,
            "RdmModel.vertices",
        )?;
        let indices: Vec<_> = model
            .triangle_indices
            .iter()
            .flat_map(|triangle| triangle.indices)
            .collect();
        let storage_type = if indices.iter().any(|&index| index > u32::from(u16::MAX)) {
            StorageType::U32
        } else {
            StorageType::U16
        };
        let (bounds_min, bounds_max) = model.vertex.calculate_bounds();
        let max_material = model
            .mesh_info
            .iter()
            .map(|group| group.material)
            .max()
            .ok_or("mesh has no material groups")?;
        let mesh = RdmModel {
            name: RdmModelName {
                name: Some("basalt_crusher_others_lod0".into()),
            },
            vertex_format: format,
            index_format: RdmIndexFormat { storage_type },
            vertices,
            indices: RdmIndices { values: indices },
            groups: model
                .mesh_info
                .into_iter()
                .map(|group| RdmMeshGroup {
                    first_index: group.start_index_location,
                    index_count: group.index_count,
                    material: group.material,
                })
                .collect(),
            skeleton_index: if with_skin { 0 } else { -1 },
            bounds_min,
            bounds_max,
        };
        Ok(Self {
            export_info: Some(RdmExportInfo {
                source_path: Some(r"\\060.alpha\data\Art\graphic_backup\christian\#ANNO5\buildings\others\basalt_crusher_others\Lowpoly\basalt_crusher_others_low_05.max".into()),
                rmp_name: Some("Anno5_Building_Skin_1Blend.rmp".into()),
            }),
            models: Some(vec![mesh]),
            materials: Some((0..=max_material).map(|_| RdmMaterialEntry {
                material: RdmMaterial {
                    name: Some("Default Standard12432142134".into()),
                    texture: Some("d:/projekte/anno5/game/testdata/graphics/dummy_objects/dummy_christian/rdm/basalt_crusher_others/diffuse.png".into()),
                },
            }).collect()),
            skin: model.joints.map(|joints| RdmSkin {
                joints: joints.into_iter().map(RdmJoint::from).collect(),
            }),
            animations: None,
        })
    }
}

impl From<RdAnim> for RdmDocument {
    /// Build a standalone animation document, retaining the legacy export defaults.
    fn from(animation: RdAnim) -> Self {
        Self {
            export_info: Some(RdmExportInfo {
                source_path: Some(r"G:\graphic\danny\Anno5\preproduction\buildings\others\basalt_crusher_others\scenes\basalt_crusher_others_idle01_01.max".into()),
                rmp_name: Some("Anno5_Building_Anim_UnCompressed.rmp".into()),
            }),
            models: None,
            materials: None,
            skin: None,
            animations: Some(vec![RdmAnimation {
                name: "basalt_crusher_others_lod0".into(),
                time_max: animation.time_max,
                sampling_rate: animation.sampling_rate,
                tracks: animation.anim_vec.into_iter().map(|track| RdmTrack {
                    joint_name: track.name,
                    frames: track.frames,
                }).collect(),
            }]),
        }
    }
}

impl RdmDocument {
    /// Adapt all model geometry, without activating skeletons or animations.
    /// Source layout and opaque data remain outside the scene representation.
    pub fn into_static_scene(self) -> Result<Scene, Box<dyn Error>> {
        self.into_scene(false)
    }

    /// Move geometry and optionally bind poses into a scene, without normalizing joints.
    pub fn into_scene(self, with_skin: bool) -> Result<Scene, Box<dyn Error>> {
        let mut meshes = self
            .models
            .unwrap_or_default()
            .into_iter()
            .map(Mesh::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        if meshes.is_empty() {
            return Err("input contains no models".into());
        }
        let skeletons = if with_skin {
            let skin = self.skin.ok_or("requested skeleton is absent")?;
            vec![Skeleton {
                joints: skin
                    .joints
                    .into_iter()
                    .map(|joint| {
                        let [x, y, z, w] = joint.rotation;
                        Joint {
                            name: joint.name,
                            parent: (joint.parent != u32::MAX).then_some(joint.parent as usize),
                            inverse_bind_translation: joint.translation,
                            // RDM stores the conjugate rotation alongside inverse-bind translation.
                            inverse_bind_rotation: [-x, -y, -z, w],
                        }
                    })
                    .collect(),
            }]
        } else {
            for mesh in &mut meshes {
                mesh.skeleton = None;
            }
            Vec::new()
        };
        Ok(Scene {
            meshes,
            skeletons,
            ..Default::default()
        })
    }

    /// Adapt one selected model for the existing exporter, optionally with its skeleton.
    pub fn into_model(self, index: usize, with_skin: bool) -> Result<RdModell, Box<dyn Error>> {
        let models = self.models.unwrap_or_default();
        let count = models.len();
        let model = models
            .into_iter()
            .nth(index)
            .ok_or_else(|| format!("model index {index} out of range for {count} models"))?;
        let mut model = RdModell::try_from(model)?;
        model.pending_joints = self
            .skin
            .map(|skin| skin.joints.into_iter().map(RdJoint::from).collect());
        if with_skin {
            model.joints = Some(
                model
                    .pending_joints
                    .take()
                    .ok_or("requested skeleton is absent")?,
            );
        }
        Ok(model)
    }

    /// Adapt one animation, retaining the filename-based name used by the legacy CLI.
    pub fn into_animation(self, name: String) -> Result<RdAnim, Box<dyn Error>> {
        let mut animations = self.animations.unwrap_or_default();
        if animations.len() != 1 {
            return Err(format!(
                "animation export requires one animation, found {}",
                animations.len()
            )
            .into());
        }
        let animation = animations.pop().expect("one animation was checked");
        Ok(RdAnim {
            name,
            time_max: animation.time_max,
            sampling_rate: animation.sampling_rate,
            anim_vec: animation
                .tracks
                .into_iter()
                .map(|track| FrameCollection {
                    name: track.joint_name,
                    frames: track.frames,
                })
                .collect(),
        })
    }
}

impl RdAnim {
    /// Move RDM keys into a scene clip, resolving names within one skeleton.
    /// Unmatched tracks are skipped, as in the legacy exporter.
    pub fn into_scene_animation(
        self,
        skeleton: usize,
        joint_names: &[&str],
    ) -> Result<Animation, Box<dyn Error>> {
        let mut tracks = Vec::new();
        for track in self.anim_vec {
            let Some(joint) = joint_names.iter().rposition(|name| *name == track.name) else {
                warn!(
                    "Could not find animation target {:?} in skeleton",
                    track.name
                );
                continue;
            };
            tracks.push(Track {
                joint,
                frames: track
                    .frames
                    .into_iter()
                    .map(|frame| {
                        let [x, y, z, w] = frame.rotation;
                        Keyframe {
                            rotation: [x, y, z, -w],
                            translation: frame.translation,
                            time: frame.time,
                        }
                    })
                    .collect(),
            });
        }
        if tracks.is_empty() {
            return Err(format!("animation {:?} has no matching joint tracks", self.name).into());
        }
        Ok(Animation {
            name: self.name,
            skeleton,
            duration_millis: self.time_max,
            sampling_rate: self.sampling_rate,
            tracks,
        })
    }
}

impl TryFrom<RdmModel> for Mesh {
    type Error = Box<dyn Error>;

    fn try_from(model: RdmModel) -> Result<Self, Self::Error> {
        if model.vertex_format.attributes.len() != model.vertices.attributes.len() {
            return Err("vertex descriptors and attribute columns disagree".into());
        }
        let mesh = Self {
            name: model.name.name,
            skeleton: usize::try_from(model.skeleton_index).ok(),
            vertex_count: model.vertices.count,
            attributes: model
                .vertex_format
                .attributes
                .into_iter()
                .zip(model.vertices.attributes)
                .map(|(descriptor, values)| Attribute { descriptor, values })
                .collect(),
            indices: model.indices.values,
            primitives: model
                .groups
                .into_iter()
                .map(|group| {
                    Ok(Primitive {
                        indices: group.first_index as usize
                            ..usize::try_from(
                                u64::from(group.first_index) + u64::from(group.index_count),
                            )?,
                        material: group.material,
                    })
                })
                .collect::<Result<_, Box<dyn Error>>>()?,
        };
        mesh.validate()?;
        Ok(mesh)
    }
}

impl TryFrom<RdmModel> for RdModell {
    type Error = Box<dyn Error>;

    fn try_from(model: RdmModel) -> Result<Self, Self::Error> {
        let (triangles, rest) = model.indices.values.as_chunks::<3>();
        if !rest.is_empty() {
            return Err("triangle index count must be divisible by three".into());
        }
        for group in &model.groups {
            if !group.first_index.is_multiple_of(3)
                || !group.index_count.is_multiple_of(3)
                || u64::from(group.first_index) + u64::from(group.index_count)
                    > model.indices.values.len() as u64
            {
                return Err(
                    "mesh group must cover complete triangles within the index buffer".into(),
                );
            }
        }
        let triangle_indices = triangles
            .iter()
            .map(|&indices| Triangle { indices })
            .collect();
        let (stride, bytes) = vertex_bytes(
            &model.vertices,
            &model.vertex_format,
            RdmEndian::Little,
            0,
            "RdmModel.vertices",
        )?;
        let identifiers = model.vertex_format.attributes;
        let count = model.vertices.count;
        Ok(Self {
            name: model.name.name,
            pending_joints: None,
            joints: None,
            anim: None,
            mat: None,
            triangle_indices,
            mesh_info: model
                .groups
                .into_iter()
                .map(|group| MeshInfo {
                    start_index_location: group.first_index,
                    index_count: group.index_count,
                    material: group.material,
                    _padding: [0; 16],
                })
                .collect(),
            vertex: VertexFormat2::new(
                identifiers.into_boxed_slice(),
                u32::try_from(count)?,
                stride,
                bytes.into(),
            ),
        })
    }
}

impl From<RdmJoint> for RdJoint {
    fn from(joint: RdmJoint) -> Self {
        let [rx, ry, rz, rw] = joint.rotation;
        let rotation = UnitQuaternion::from_quaternion(Quaternion::new(rw, rx, ry, rz));
        let [tx, ty, tz] = joint.translation;
        let translation = rotation
            .transform_vector(&Vector3::new(tx, ty, tz))
            .scale(-1.0);
        let q = rotation.quaternion().coords;
        Self {
            name: joint.name,
            translation: [translation.x, translation.y, translation.z],
            quaternion: [q.x, q.y, q.z, q.w],
            parent: joint.parent,
        }
    }
}

impl From<RdJoint> for RdmJoint {
    fn from(joint: RdJoint) -> Self {
        let [rx, ry, rz, rw] = joint.quaternion;
        let rotation = UnitQuaternion::from_quaternion(Quaternion::new(rw, rx, ry, rz));
        let [tx, ty, tz] = joint.translation;
        let translation = rotation
            .inverse_transform_vector(&Vector3::new(tx, ty, tz))
            .scale(-1.0);
        let q = rotation.quaternion().coords;
        Self {
            name: joint.name,
            translation: [translation.x, translation.y, translation.z],
            rotation: [q.x, q.y, q.z, q.w],
            parent: joint.parent,
        }
    }
}
