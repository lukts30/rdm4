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
    RdmAnimation, RdmDocument, RdmEndian, RdmExportInfo, RdmExportOptions, RdmFrame,
    RdmIndexFormat, RdmIndices, RdmJoint, RdmMaterial, RdmMaterialEntry, RdmMeshGroup, RdmModel,
    RdmModelName, RdmSkin, RdmTrack, RdmVertexFormat, RdmWriter,
};
use crate::{
    rdm_anim::{FrameCollection, RdAnim},
    scene::{Animation, Attribute, Joint, Keyframe, Mesh, Primitive, Scene, Skeleton, Track},
    vertex::{StorageType, VertexFormat2},
    MeshInfo, RdJoint, RdModell, Triangle,
};

const LEGACY_NAME: &str = "basalt_crusher_others_lod0";

fn export_info(animation: bool, legacy: bool) -> RdmExportInfo {
    RdmExportInfo {
        source_path: legacy.then(|| if animation {
            r"G:\graphic\danny\Anno5\preproduction\buildings\others\basalt_crusher_others\scenes\basalt_crusher_others_idle01_01.max".into()
        } else {
            r"\\060.alpha\data\Art\graphic_backup\christian\#ANNO5\buildings\others\basalt_crusher_others\Lowpoly\basalt_crusher_others_low_05.max".into()
        }),
        rmp_name: Some(if animation { "Anno5_Building_Anim_UnCompressed.rmp" } else { "Anno5_Building_Skin_1Blend.rmp" }.into()),
    }
}

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
        let mesh = RdmModel {
            name: RdmModelName { name: model.name },
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
        mesh_document(
            vec![mesh],
            model.joints.map(|joints| RdmSkin {
                joints: joints.into_iter().map(RdmJoint::from).collect(),
            }),
            true,
        )
    }
}

fn mesh_document(
    mut models: Vec<RdmModel>,
    skin: Option<RdmSkin>,
    legacy: bool,
) -> Result<RdmDocument, Box<dyn Error>> {
    let max_material = models
        .iter()
        .flat_map(|model| &model.groups)
        .map(|group| group.material)
        .max()
        .ok_or("scene has no material groups")?;
    if legacy {
        for model in &mut models {
            model.name.name = Some(LEGACY_NAME.into());
        }
    }
    Ok(RdmDocument {
        export_info: Some(export_info(false, legacy)),
        models: Some(models),
        materials: Some((0..=max_material).map(|_| RdmMaterialEntry {
            material: RdmMaterial {
                name: legacy.then(|| "Default Standard12432142134".into()),
                texture: legacy.then(|| "d:/projekte/anno5/game/testdata/graphics/dummy_objects/dummy_christian/rdm/basalt_crusher_others/diffuse.png".into()),
            },
        }).collect()),
        skin, animations: None,
    })
}

impl From<RdAnim> for RdmDocument {
    /// Build a standalone animation document, retaining the legacy export defaults.
    fn from(animation: RdAnim) -> Self {
        animation_document(
            RdmAnimation {
                name: animation.name,
                time_max: animation.time_max,
                sampling_rate: animation.sampling_rate,
                tracks: animation
                    .anim_vec
                    .into_iter()
                    .map(|track| RdmTrack {
                        joint_name: track.name,
                        frames: track.frames,
                    })
                    .collect(),
            },
            true,
        )
    }
}

fn animation_document(mut animation: RdmAnimation, legacy: bool) -> RdmDocument {
    if legacy {
        animation.name = LEGACY_NAME.into();
    }
    RdmDocument {
        export_info: Some(export_info(true, legacy)),
        models: None,
        materials: None,
        skin: None,
        animations: Some(vec![animation]),
    }
}

/// Pack and write a scene, retaining legacy CLI paths and separate animation files.
pub fn write_scene(
    scene: Scene,
    options: RdmExportOptions,
    dir: Option<PathBuf>,
    create_new: bool,
) -> Result<PathBuf, Box<dyn Error>> {
    // Keep stable CLI filenames while record names retain the source clip names.
    let names = (0..scene.animations.len())
        .map(|index| format!("anim_{index}"))
        .collect::<Vec<_>>();
    let legacy = options.legacy_metadata;
    let mut document = RdmDocument::from_scene(scene, options)?;
    for (name, animation) in names
        .into_iter()
        .zip(document.animations.take().unwrap_or_default())
    {
        RdAnimWriter2 {
            name,
            export: animation_document(animation, legacy),
        }
        .try_write_anim_rdm(dir.clone(), create_new)?;
    }
    RdWriter2 { inner: document }.try_write_rdm(dir, create_new)
}

impl RdmDocument {
    /// Pack typed scene values for RDM, applying only the explicit export decisions.
    pub fn from_scene(scene: Scene, options: RdmExportOptions) -> Result<Self, Box<dyn Error>> {
        if scene.skeletons.len() > 1 {
            return Err(format!(
                "RDM export supports one skeleton, found {}; select one mesh instead",
                scene.skeletons.len()
            )
            .into());
        }
        if scene.meshes.iter().any(|mesh| {
            mesh.skeleton
                .is_some_and(|index| index >= scene.skeletons.len())
        }) {
            return Err("mesh skeleton index is out of range".into());
        }
        let mut animations = Vec::new();
        for clip in scene.animations {
            let skeleton = scene
                .skeletons
                .get(clip.skeleton)
                .ok_or("animation skeleton index is out of range")?;
            let tracks = clip
                .tracks
                .into_iter()
                .map(|track| {
                    let joint = skeleton
                        .joints
                        .get(track.joint)
                        .ok_or("animation joint index is out of range")?;
                    Ok(RdmTrack {
                        joint_name: joint.name.clone(),
                        frames: track
                            .frames
                            .into_iter()
                            .map(|frame| {
                                let [x, y, z, w] = frame.rotation;
                                RdmFrame {
                                    rotation: [x, y, z, -w],
                                    translation: frame.translation,
                                    time: frame.time,
                                }
                            })
                            .collect(),
                    })
                })
                .collect::<Result<_, Box<dyn Error>>>()?;
            animations.push(RdmAnimation {
                name: if options.legacy_metadata {
                    LEGACY_NAME.into()
                } else {
                    clip.name
                },
                time_max: clip.duration_millis,
                sampling_rate: clip.sampling_rate,
                tracks,
            });
        }
        let primitive_count: usize = scene.meshes.iter().map(|mesh| mesh.primitives.len()).sum();
        if options
            .material_indices
            .as_ref()
            .is_some_and(|v| v.len() != primitive_count)
        {
            return Err(format!(
                "material override count must match {primitive_count} primitives across all models"
            )
            .into());
        }
        let mut offset = 0;
        let models = scene
            .meshes
            .into_iter()
            .map(|mesh| {
                let end = offset + mesh.primitives.len();
                let materials = options.material_indices.as_ref().map(|v| &v[offset..end]);
                offset = end;
                super::packing::pack_mesh(mesh, &options, materials)
            })
            .collect::<Result<_, _>>()?;
        let skin = scene.skeletons.into_iter().next().map(|skeleton| RdmSkin {
            joints: skeleton
                .joints
                .into_iter()
                .map(|joint| {
                    let [x, y, z, w] = joint.inverse_bind_rotation;
                    RdmJoint {
                        name: joint.name,
                        parent: joint.parent.map_or(u32::MAX, |i| i as u32),
                        translation: joint.inverse_bind_translation,
                        rotation: [-x, -y, -z, w],
                    }
                })
                .collect(),
        });
        let mut document = mesh_document(models, skin, options.legacy_metadata)?;
        document.animations = (!animations.is_empty()).then_some(animations);
        Ok(document)
    }

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
            transform: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{rdm::*, vertex::StorageType, RdModell};
    use std::{fs, io::Cursor};

    fn read_source(path: &str) -> RdmReadResult<RdmDocument> {
        RdmReader::new(std::io::BufReader::new(fs::File::open(path).unwrap()))
            .unwrap()
            .read_root()
            .unwrap()
    }

    fn stored_bounds(document: &RdmDocument) -> ([f32; 3], [f32; 3]) {
        let model = &document.models.as_ref().unwrap()[0];
        (model.bounds_min, model.bounds_max)
    }

    #[test]
    fn reads_68_byte_metadata_tails_in_both_byte_orders() {
        for (source, endian) in [
            (
                include_bytes!("../../rdm/n_nature_small_stone_02_lod0.rdm").as_slice(),
                RdmEndian::Big,
            ),
            (
                include_bytes!("../../rdm/jellyfish_lod0.rdm").as_slice(),
                RdmEndian::Little,
            ),
        ] {
            let mut bytes = source.to_vec();
            let meta = endian.decode::<u32>(&bytes[0x20..0x24]) as usize;
            for i in 0..4 {
                endian.encode(
                    i as u32 + 1,
                    &mut bytes[meta + 52 + i * 4..meta + 56 + i * 4],
                );
            }
            let mut reader = RdmReader::new(Cursor::new(bytes)).unwrap();
            let block = reader.read_block::<RdmModel>(meta as u32).unwrap();
            assert_eq!(block.source.stride, 68);
            assert_eq!(block.source.payload.end, meta + 68);
            let words: Vec<u32> = block
                .tails
                .chunks_exact(4)
                .map(|raw| endian.decode(raw))
                .collect();
            assert_eq!(words, [1, 2, 3, 4]);
        }
    }

    #[test]
    fn converts_be_68_byte_metadata_to_le_92_byte_metadata() {
        use crate::vertex::{N3f, N4b};
        let model = crate::RdModell::from("rdm/n_nature_small_stone_02_lod0.rdm");
        let source = read_source("rdm/n_nature_small_stone_02_lod0.rdm");
        assert_eq!(source.layout.endian, RdmEndian::Big);
        let models = source
            .layout
            .blocks
            .iter()
            .find(|b| b.context == "RdmDocument.models")
            .unwrap();
        assert_eq!(models.source.stride, 68);
        // The packed normal must be decoded as a BE word, not four unchanged bytes.
        let normal = model.vertex.iter::<N4b, N3f>(0).unwrap().next().unwrap();
        for (actual, expected) in normal
            .data
            .into_iter()
            .zip([0.90588236, 0.42745098, 0.03529412])
        {
            assert!((actual - expected).abs() < 0.000001);
        }
        let bounds = model.vertex.calculate_bounds();
        let document = crate::rdm::RdmDocument::try_from(model).unwrap();
        let output = std::io::Cursor::new(Vec::new());
        let output = crate::rdm::RdmWriter::new(output, crate::rdm::RdmEndian::Little)
            .write_root(&document)
            .unwrap();
        let result = RdmReader::new(Cursor::new(output.get_ref()))
            .unwrap()
            .read_root::<RdmDocument>()
            .unwrap();
        assert_eq!(result.layout.endian, RdmEndian::Little);
        let models = result
            .layout
            .blocks
            .iter()
            .find(|b| b.context == "RdmDocument.models")
            .unwrap();
        assert_eq!(models.source.stride, 92);
        assert_eq!(models.tails, [0; 40]);
        assert_eq!(
            RdModell::new(output.into_inner()).vertex.calculate_bounds(),
            bounds
        );
    }

    #[test]
    fn writes_bounds_from_quantized_vertices_instead_of_source_bounds() {
        let model = crate::RdModell::from("rdm/park_props_system_3x3_04_decal_detail04_lod0.rdm");
        let stored = stored_bounds(
            &read_source("rdm/park_props_system_3x3_04_decal_detail04_lod0.rdm").value,
        );
        let calculated = model.vertex.calculate_bounds();
        assert_eq!(model.vertex.identifiers[0].storage_type, StorageType::F16);
        assert_ne!(stored, calculated);
        let round = |value| half::f16::from_f32(value).to_f32();
        assert_eq!((stored.0.map(round), stored.1.map(round)), calculated);
        assert_eq!(
            {
                let document = crate::rdm::RdmDocument::try_from(model).unwrap();
                let model = &document.models.as_ref().unwrap()[0];
                (model.bounds_min, model.bounds_max)
            },
            calculated
        );
    }

    #[test]
    fn preserves_f32_position_bounds_when_writing_rdm() {
        let model = crate::RdModell::from("rdm/eden_line_flagpole_variation_cloth_lod0.rdm");
        let position = &model.vertex.identifiers[0];
        assert_eq!(position.to_string(), "P3f");
        let calculated = model.vertex.calculate_bounds();
        assert_eq!(
            calculated.0.map(f32::to_bits),
            [0x3c70ef5c, 0x3d073c7a, 0xbe9ba652]
        );
        assert_eq!(
            calculated.1.map(f32::to_bits),
            [0x3cf1c51e, 0x3fc6c946, 0x3e9ba652]
        );
        assert_eq!(
            stored_bounds(&read_source("rdm/eden_line_flagpole_variation_cloth_lod0.rdm").value),
            calculated
        );
        assert_eq!(
            {
                let document = crate::rdm::RdmDocument::try_from(model).unwrap();
                let model = &document.models.as_ref().unwrap()[0];
                (model.bounds_min, model.bounds_max)
            },
            calculated
        );
    }

    #[test]
    fn calculates_bounds_matching_existing_mesh_metadata() {
        for data in [
            include_bytes!("../../rdm/fishery_others_lod2.rdm").as_slice(),
            include_bytes!("../../rdm/ark_waterfall2.rdm").as_slice(),
        ] {
            let rdm = RdmReader::new(Cursor::new(data))
                .unwrap()
                .read_root::<RdmDocument>()
                .unwrap();
            let model = RdModell::new(data.to_vec());
            assert_eq!(stored_bounds(&rdm.value), model.vertex.calculate_bounds());
        }
    }

    #[test]
    fn matches_serialized_record_strides() {
        assert_eq!(RdmMaterialEntry::STRIDES, [28]);
        assert_eq!(RdmSkin::STRIDES, [32]);
        assert_eq!(RdmModel::STRIDES, [92, 68]);
        assert_eq!(RdmModelName::STRIDES, [28]);
        assert_eq!(RdmVertexFormat::STRIDES, [24]);
        assert_eq!(RdmMeshGroup::STRIDES, [28]);
        assert_eq!(RdmJoint::STRIDES, [84]);
        assert_eq!(RdmExportInfo::STRIDES, [72]);
    }

    #[test]
    fn preserves_sampling_rate_when_loading_and_writing_animation() {
        for sampling_rate in [1u32, 30, 60, 78] {
            let mut data = include_bytes!("../../rdm/basalt_crusher_others_idle01.rdm").to_vec();
            let animation = u32::from_le_bytes(data[0x2c..0x30].try_into().unwrap()) as usize;
            data[animation + 12..animation + 16].copy_from_slice(&sampling_rate.to_le_bytes());
            let anim = crate::rdm_anim::RdAnim::new(data, "idle".into());
            assert_eq!(anim.sampling_rate, sampling_rate);
            let document = crate::rdm::RdmDocument::from(anim);
            let writer = std::io::Cursor::new(Vec::new());
            let writer = crate::rdm::RdmWriter::new(writer, crate::rdm::RdmEndian::Little)
                .write_root(&document)
                .unwrap();
            let written = RdmReader::new(writer)
                .unwrap()
                .read_root::<RdmDocument>()
                .unwrap();
            assert_eq!(
                written.value.animations.unwrap()[0].sampling_rate,
                sampling_rate
            );
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn writes_loaded_animation_without_errors() {
        let anim = crate::rdm_anim::RdAnim::from("rdm/basalt_crusher_others_work01.rdm");
        let rdaw = RdAnimWriter2::new(anim);
        let output_dir = tempfile::tempdir().unwrap();
        rdaw.write_anim_rdm(Some(output_dir.path().into()), false);
    }
}
