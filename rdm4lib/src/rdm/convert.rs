//! Adapters to the existing glTF conversion types.

use std::error::Error;

use nalgebra::{Quaternion, UnitQuaternion, Vector3};

use super::{buffers::vertex_bytes, RdmDocument, RdmEndian, RdmJoint, RdmModel};
use crate::{
    rdm_anim::{FrameCollection, RdAnim},
    rdm_data_anim::Frame,
    rdm_data_main::MeshInfo,
    vertex::VertexFormat2,
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
