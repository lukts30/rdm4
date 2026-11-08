//! Scene/document conversion and RDM file output.

use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use super::{
    RdmAnimation, RdmDocument, RdmEndian, RdmExportInfo, RdmExportOptions, RdmFrame, RdmJoint,
    RdmMaterial, RdmMaterialEntry, RdmModel, RdmSkin, RdmTrack, RdmWriter,
};
use crate::scene::{
    Animation, Attribute, Joint, Keyframe, Mesh, Primitive, Scene, Skeleton, Track,
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
    let legacy = options.legacy;
    let mut document = RdmDocument::from_scene(scene, options)?;
    let mut file = output_path(dir)?;
    let directory = file.is_dir();
    for (index, animation) in document
        .animations
        .take()
        .unwrap_or_default()
        .into_iter()
        .enumerate()
    {
        let name = format!("anim_{index}.rdm");
        let path = if directory {
            file.join(name)
        } else {
            file.with_file_name(format!(
                "{}_{}",
                file.file_stem().unwrap().to_string_lossy(),
                name
            ))
        };
        write_document(&animation_document(animation, legacy), &path, create_new)?;
    }
    if directory {
        file.push("out.rdm");
    }
    write_document(&document, &file, create_new)?;
    Ok(file)
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
                name: if options.legacy {
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
        let mut document = mesh_document(models, skin, options.legacy)?;
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
}

impl Scene {
    /// Attach native RDM keys to a skeleton without resampling or normalizing them.
    /// Unmatched tracks are skipped; duplicate joint names use the last match.
    pub fn attach_rdm_animation(
        &mut self,
        animation: RdmAnimation,
        skeleton: usize,
    ) -> Result<(), Box<dyn Error>> {
        let joints = &self
            .skeletons
            .get(skeleton)
            .ok_or("animation skeleton index is out of range")?
            .joints;
        let mut tracks = Vec::new();
        for track in animation.tracks {
            let Some(joint) = joints
                .iter()
                .rposition(|joint| joint.name == track.joint_name)
            else {
                warn!(
                    "Could not find animation target {:?} in skeleton",
                    track.joint_name
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
            return Err(format!(
                "animation {:?} has no matching joint tracks",
                animation.name
            )
            .into());
        }
        self.animations.push(Animation {
            name: animation.name,
            skeleton,
            duration_millis: animation.time_max,
            sampling_rate: animation.sampling_rate,
            tracks,
        });
        Ok(())
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

#[cfg(test)]
mod tests {
    use crate::{
        rdm::*,
        vertex::{StorageType, TargetVertexFormat},
    };
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
                .as_chunks::<4>()
                .0
                .iter()
                .map(|raw| endian.decode(raw))
                .collect();
            assert_eq!(words, [1, 2, 3, 4]);
        }
    }

    #[test]
    fn converts_be_68_byte_metadata_to_le_92_byte_metadata() {
        let source = read_source("rdm/n_nature_small_stone_02_lod0.rdm");
        assert_eq!(source.layout.endian, RdmEndian::Big);
        let models = source
            .layout
            .blocks
            .iter()
            .find(|b| b.context == "RdmDocument.models")
            .unwrap();
        assert_eq!(models.source.stride, 68);
        // Check the packed BE word before direction normalization during glTF export.
        let RdmAttributeValues::U32(normals) =
            &source.value.models.as_ref().unwrap()[0].vertices.attributes[1]
        else {
            panic!("packed normals");
        };
        for (actual, expected) in normals[0]
            .to_le_bytes()
            .into_iter()
            .take(3)
            .map(|v| 2.0 * f32::from(v) / 255.0 - 1.0)
            .zip([0.90588236, 0.42745098, 0.03529412])
        {
            assert!((actual - expected).abs() < 0.000001);
        }
        let output = RdmWriter::new(Cursor::new(Vec::new()), RdmEndian::Little)
            .write_root(&source.value)
            .unwrap();
        let result = RdmReader::new(output)
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
        assert_eq!(result.value, source.value);
    }

    fn pack_static(document: RdmDocument) -> RdmDocument {
        RdmDocument::from_scene(
            document.into_static_scene().unwrap(),
            RdmExportOptions {
                format: TargetVertexFormat::P4h_N4b_G4b_B4b_T2h,
                no_transform: true,
                mirror_x: false,
                material_indices: None,
                legacy: false,
            },
        )
        .unwrap()
    }

    #[test]
    fn writes_bounds_from_quantized_vertices_instead_of_source_bounds() {
        let source = read_source("rdm/park_props_system_3x3_04_decal_detail04_lod0.rdm").value;
        let stored = stored_bounds(&source);
        assert_eq!(
            source.models.as_ref().unwrap()[0].vertex_format.attributes[0].storage_type,
            StorageType::F16
        );
        let packed = pack_static(source);
        let calculated = stored_bounds(&packed);
        assert_ne!(stored, calculated);
        let round = |value| half::f16::from_f32(value).to_f32();
        assert_eq!((stored.0.map(round), stored.1.map(round)), calculated);
    }

    #[test]
    fn preserves_f32_position_bounds_when_writing_rdm() {
        let source = read_source("rdm/eden_line_flagpole_variation_cloth_lod0.rdm").value;
        assert_eq!(
            source.models.as_ref().unwrap()[0].vertex_format.attributes[0].to_string(),
            "P3f"
        );
        let output = RdmWriter::new(Cursor::new(Vec::new()), RdmEndian::Little)
            .write_root(&source)
            .unwrap();
        let written = RdmReader::new(output)
            .unwrap()
            .read_root::<RdmDocument>()
            .unwrap()
            .value;
        let bounds = stored_bounds(&written);
        assert_eq!(
            bounds.0.map(f32::to_bits),
            [0x3c70ef5c, 0x3d073c7a, 0xbe9ba652]
        );
        assert_eq!(
            bounds.1.map(f32::to_bits),
            [0x3cf1c51e, 0x3fc6c946, 0x3e9ba652]
        );
        assert_eq!(written, source);
    }

    #[test]
    fn calculates_bounds_matching_existing_mesh_metadata() {
        for path in ["rdm/fishery_others_lod2.rdm", "rdm/ark_waterfall2.rdm"] {
            let source = read_source(path).value;
            let stored = stored_bounds(&source);
            assert_eq!(stored, stored_bounds(&pack_static(source)));
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
        let mut scene = read_source("rdm/basalt_crusher_others_lod0.rdm")
            .value
            .into_scene(true)
            .unwrap();
        for sampling_rate in [1u32, 30, 60, 78] {
            let mut data = include_bytes!("../../rdm/basalt_crusher_others_idle01.rdm").to_vec();
            let animation = u32::from_le_bytes(data[0x2c..0x30].try_into().unwrap()) as usize;
            data[animation + 12..animation + 16].copy_from_slice(&sampling_rate.to_le_bytes());
            let document = RdmReader::new(Cursor::new(data))
                .unwrap()
                .read_root::<RdmDocument>()
                .unwrap()
                .value;
            scene
                .attach_rdm_animation(document.animations.as_ref().unwrap()[0].clone(), 0)
                .unwrap();
            assert_eq!(
                scene.animations.last().unwrap().sampling_rate,
                sampling_rate
            );
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
}
