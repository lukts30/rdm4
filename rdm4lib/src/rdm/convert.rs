//! Adapters to the existing glTF conversion types.

use std::error::Error;

use nalgebra::{Quaternion, UnitQuaternion, Vector3};

use super::{wire::Scalar, RdmAttributeValues, RdmDocument, RdmEndian, RdmJoint, RdmModel};
use crate::{
    rdm_anim::{FrameCollection, RdAnim},
    rdm_data_anim::Frame,
    rdm_data_main::MeshInfo,
    vertex::{StorageType, VertexFormat2},
    RdJoint, RdModell, Triangle,
};

impl RdmDocument {
    /// Adapt one selected model for the existing exporter, optionally with its skeleton.
    pub fn into_model(self, index: usize, with_skin: bool) -> Result<RdModell, Box<dyn Error>> {
        let models = self.models.unwrap_or_default();
        let count = models.len();
        let model = models
            .into_iter()
            .nth(index)
            .ok_or_else(|| format!("model index {index} out of range for {count} models"))?;
        let mut model = RdModell::try_from(model)?;
        if with_skin {
            let skin = self.skin.ok_or("requested skeleton is absent")?;
            model.joints = Some(skin.joints.into_iter().map(RdJoint::from).collect());
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
                    frames: track
                        .frames
                        .into_iter()
                        .map(|frame| Frame {
                            rotation: frame.rotation,
                            translation: frame.translation,
                            time: frame.time,
                        })
                        .collect(),
                })
                .collect(),
        })
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
        let identifiers = model.vertex_format.attributes;
        let count = model.vertices.count;
        if identifiers.len() != model.vertices.attributes.len() {
            return Err("vertex descriptors and attribute columns disagree".into());
        }
        let stride = identifiers.iter().try_fold(0usize, |stride, id| {
            (id.storage_type.byte_size() as usize)
                .checked_mul(id.storage_count as usize)
                .and_then(|size| stride.checked_add(size))
                .ok_or("vertex stride overflow")
        })?;
        let mut bytes = vec![
            0;
            stride
                .checked_mul(count)
                .ok_or("vertex buffer size overflow")?
        ];
        let mut offset = 0;
        for (id, values) in identifiers.iter().zip(&model.vertices.attributes) {
            let components = id.storage_count as usize;
            match (id.storage_type, values) {
                (StorageType::U16, RdmAttributeValues::U16(v)) => {
                    interleave(&mut bytes, v, stride, offset, components, count)?
                }
                (StorageType::U32, RdmAttributeValues::U32(v)) => {
                    interleave(&mut bytes, v, stride, offset, components, count)?
                }
                (StorageType::F16, RdmAttributeValues::F16(v)) => {
                    interleave(&mut bytes, v, stride, offset, components, count)?
                }
                (StorageType::F32, RdmAttributeValues::F32(v)) => {
                    interleave(&mut bytes, v, stride, offset, components, count)?
                }
                _ => {
                    return Err(
                        "vertex attribute storage type disagrees with its descriptor".into(),
                    )
                }
            }
            offset += id.storage_type.byte_size() as usize * components;
        }
        Ok(Self {
            rdmf: None,
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
                u32::try_from(stride)?,
                bytes.into(),
            ),
        })
    }
}

fn interleave<T: Scalar>(
    bytes: &mut [u8],
    values: &[T],
    stride: usize,
    offset: usize,
    components: usize,
    count: usize,
) -> Result<(), Box<dyn Error>> {
    if values.len()
        != count
            .checked_mul(components)
            .ok_or("vertex column size overflow")?
    {
        return Err("vertex attribute count disagrees with its descriptor".into());
    }
    for i in 0..count {
        for component in 0..components {
            let at = i * stride + offset + component * T::WIDTH;
            RdmEndian::Little.encode(
                values[i * components + component],
                &mut bytes[at..at + T::WIDTH],
            );
        }
    }
    Ok(())
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
            transition: [translation.x, translation.y, translation.z],
            quaternion: [q.x, q.y, q.z, q.w],
            parent: joint.parent,
        }
    }
}
