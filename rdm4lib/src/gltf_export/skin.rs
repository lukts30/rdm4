//! Bind-matrix construction shared by scene and compatibility exports.

#![forbid(unsafe_code)]

use std::error::Error;

use gltf::json;
use nalgebra::{Matrix4, Quaternion, Similarity3, Translation3, UnitQuaternion, Vector3};

use crate::{
    gltf_buffers::{AccessorBuilder, BufferContainer, GltfBuffers},
    scene::{AttributeValues, Joint, Mesh},
    vertex::VertexSemantic,
};

/// A calculated global bind pose; the source scene remains unchanged.
pub(super) struct GlobalJoint<'a> {
    pub name: &'a str,
    pub parent: Option<usize>,
    pub translation: [f32; 3],
    pub rotation: [f32; 4],
}

impl<'a> GlobalJoint<'a> {
    pub fn from_inverse_bind(joint: &'a Joint) -> Result<Self, Box<dyn Error>> {
        let [x, y, z, w] = joint.inverse_bind_rotation;
        let rotation = rotation([-x, -y, -z, w])?;
        let translation = rotation
            .transform_vector(&Vector3::from(joint.inverse_bind_translation))
            .scale(-1.0);
        let q = rotation.coords;
        Ok(Self {
            name: &joint.name,
            parent: joint.parent,
            translation: [translation.x, translation.y, translation.z],
            rotation: [q.x, q.y, q.z, q.w],
        })
    }
}

fn rotation(value: [f32; 4]) -> Result<UnitQuaternion<f32>, Box<dyn Error>> {
    let [x, y, z, w] = value;
    let q = Quaternion::new(w, x, y, z);
    if !q.norm_squared().is_finite() || q.norm_squared() == 0.0 {
        return Err("joint has an invalid rotation quaternion".into());
    }
    Ok(UnitQuaternion::from_quaternion(q))
}

pub(super) struct SkinOutput {
    pub nodes: Vec<json::Node>,
    pub roots: Vec<json::Index<json::Node>>,
    pub skin: json::Skin,
}

pub(super) fn build(
    joints: &[GlobalJoint<'_>],
    binary: &mut GltfBuffers,
    first_node: usize,
) -> Result<SkinOutput, Box<dyn Error>> {
    if joints.is_empty() {
        return Err("skeleton contains no joints".into());
    }
    let mut states = vec![0u8; joints.len()];
    for joint in joints {
        if joint.parent.is_some_and(|parent| parent >= joints.len()) {
            return Err(format!("joint {:?} has an out-of-range parent", joint.name).into());
        }
    }
    for start in 0..joints.len() {
        let mut current = Some(start);
        while let Some(i) = current {
            match states[i] {
                1 => {
                    return Err(
                        format!("joint {:?} is in a hierarchy cycle", joints[i].name).into(),
                    )
                }
                2 => break,
                _ => {
                    states[i] = 1;
                    current = joints[i].parent;
                }
            }
        }
        current = Some(start);
        while let Some(i) = current {
            if states[i] == 2 {
                break;
            }
            states[i] = 2;
            current = joints[i].parent;
        }
    }
    let mut binds = Vec::new();
    let mut inverses = Vec::new();
    let mut bytes = Vec::new();
    for joint in joints {
        if joint.translation.iter().any(|v| !v.is_finite()) {
            return Err("joint has a nonfinite translation".into());
        }
        let bind: Matrix4<f32> = Translation3::from(Vector3::from(joint.translation))
            .to_homogeneous()
            * rotation(joint.rotation)?.to_homogeneous();
        let mut inverse = bind.try_inverse().ok_or("joint bind matrix is singular")?;
        inverse.m44 = 1.0;
        bytes.extend(inverse.iter().flat_map(|v| v.to_le_bytes()));
        binds.push(bind);
        inverses.push(inverse);
    }
    let accessor = binary.push_attribute(
        BufferContainer(bytes),
        AccessorBuilder::new(
            joints.len(),
            json::accessor::Type::Mat4,
            json::accessor::ComponentType::F32,
        ),
        None,
    );
    let index = |i: usize| json::Index::new((first_node + i) as u32);
    let mut children = vec![Vec::new(); joints.len()];
    let mut roots = Vec::new();
    for (i, joint) in joints.iter().enumerate() {
        match joint.parent {
            Some(parent) => children[parent].push(index(i)),
            None => roots.push(index(i)),
        }
    }
    let mut nodes = Vec::new();
    for (i, joint) in joints.iter().enumerate() {
        let (translation, rotation) = if let Some(parent) = joint.parent {
            let mut local = inverses[parent] * binds[i];
            local.m44 = 1.0;
            let similarity: Similarity3<f32> =
                nalgebra::try_convert(local).ok_or("joint local transform is not rigid")?;
            let pose = similarity.isometry;
            let t = pose.translation;
            let q = pose.rotation.coords;
            ([t.x, t.y, t.z], [q.x, q.y, q.z, q.w])
        } else {
            (joint.translation, joint.rotation)
        };
        nodes.push(json::Node {
            name: Some(joint.name.into()),
            translation: Some(translation),
            rotation: Some(json::scene::UnitQuaternion(rotation)),
            children: if children[i].is_empty() {
                None
            } else {
                Some(std::mem::take(&mut children[i]))
            },
            ..Default::default()
        });
    }
    Ok(SkinOutput {
        nodes,
        roots,
        skin: json::Skin {
            joints: (0..joints.len()).map(index).collect(),
            inverse_bind_matrices: Some(accessor),
            skeleton: None,
            name: None,
            extensions: None,
            extras: None,
        },
    })
}

type AttributeAccessor = (json::mesh::Semantic, json::Index<json::Accessor>);

pub(super) fn influences(
    mesh: &Mesh,
    joint_count: usize,
    binary: &mut GltfBuffers,
) -> Result<Vec<AttributeAccessor>, Box<dyn Error>> {
    if mesh.attributes.iter().any(|a| {
        a.descriptor.semantic == VertexSemantic::JointIndex
            && matches!(a.values, AttributeValues::U16(_))
    }) {
        return typed_influences(mesh, joint_count, binary);
    }
    let packed = |semantic, interpretation| {
        mesh.attributes
            .iter()
            .filter(|a| a.descriptor.semantic == semantic)
            .map(|a| match &a.values {
                AttributeValues::U32(values)
                    if a.descriptor.storage_count == 1
                        && a.descriptor.interpretation.0 == interpretation =>
                {
                    Ok(values.iter().map(|v| v.to_le_bytes()).collect::<Vec<_>>())
                }
                _ => Err(format!("unsupported skin attribute: {:?}", a.descriptor).into()),
            })
            .collect::<Result<Vec<_>, Box<dyn Error>>>()
    };
    let joints = packed(VertexSemantic::JointIndex, 0)?;
    let mut weights = packed(VertexSemantic::Weight, 2)?;
    if joints.is_empty() {
        return Err("skinned mesh has no joint indices".into());
    }
    if weights.is_empty() && joints.len() == 1 {
        weights.push(vec![[255u8, 0, 0, 0]; mesh.vertex_count]);
    }
    if joints.len() != weights.len() {
        return Err("joint and weight set counts disagree".into());
    }
    let mut sums = vec![0u32; mesh.vertex_count];
    for set in &weights {
        for (sum, w) in sums.iter_mut().zip(set) {
            *sum += w.iter().map(|v| u32::from(*v)).sum::<u32>();
        }
    }
    if sums.contains(&0) {
        return Err("vertex has zero total joint weight".into());
    }
    let mut result = Vec::new();
    let mut joint_payloads = Vec::new();
    for (set, (joints, weights)) in joints.into_iter().zip(weights).enumerate() {
        let mut js = Vec::new();
        let mut ws = Vec::new();
        for ((joints, weights), sum) in joints.into_iter().zip(weights).zip(&sums) {
            for (joint, weight) in joints.into_iter().zip(weights) {
                let joint = if weight == 0 { 0 } else { joint };
                if joint as usize >= joint_count {
                    return Err(format!(
                        "vertex joint index {joint} exceeds skeleton size {joint_count}"
                    )
                    .into());
                }
                js.push(joint);
                ws.extend((f32::from(weight) / 255.0 * (255.0 / *sum as f32)).to_le_bytes());
            }
        }
        let accessor = binary.push_attribute(
            BufferContainer(ws),
            AccessorBuilder::new(
                mesh.vertex_count,
                json::accessor::Type::Vec4,
                json::accessor::ComponentType::F32,
            ),
            None,
        );
        result.push((json::mesh::Semantic::Weights(set as u32), accessor));
        joint_payloads.push(js);
    }
    for (set, bytes) in joint_payloads.into_iter().enumerate() {
        let accessor = binary.push_attribute(
            BufferContainer(bytes),
            AccessorBuilder::new(
                mesh.vertex_count,
                json::accessor::Type::Vec4,
                json::accessor::ComponentType::U8,
            ),
            None,
        );
        result.push((json::mesh::Semantic::Joints(set as u32), accessor));
    }
    Ok(result)
}

fn typed_influences(
    mesh: &Mesh,
    joint_count: usize,
    binary: &mut GltfBuffers,
) -> Result<Vec<AttributeAccessor>, Box<dyn Error>> {
    use json::{
        accessor::{ComponentType, Type},
        mesh::Semantic,
    };
    let joints: Vec<_> = mesh
        .attributes
        .iter()
        .filter(|a| a.descriptor.semantic == VertexSemantic::JointIndex)
        .collect();
    let weights: Vec<_> = mesh
        .attributes
        .iter()
        .filter(|a| a.descriptor.semantic == VertexSemantic::Weight)
        .collect();
    if joints.len() != weights.len() {
        return Err("joint and weight set counts disagree".into());
    }
    let mut sums = vec![0.0; mesh.vertex_count];
    let mut result = Vec::new();
    for (set, (joint, weight)) in joints.into_iter().zip(weights).enumerate() {
        let (AttributeValues::U16(js), AttributeValues::F32(ws)) = (&joint.values, &weight.values)
        else {
            return Err("expected U16 joints and F32 weights from glTF".into());
        };
        if joint.descriptor.storage_count != 4
            || weight.descriptor.storage_count != 4
            || js.iter().any(|&j| j as usize >= joint_count)
            || ws
                .iter()
                .any(|&w| !w.is_finite() || !(0.0..=1.0).contains(&w))
        {
            return Err("invalid typed joint indices or weights".into());
        }
        for (sum, weights) in sums.iter_mut().zip(ws.as_chunks::<4>().0) {
            *sum += weights.iter().sum::<f32>();
        }
        for (semantic, bytes, component) in [
            (
                Semantic::Joints(set as u32),
                js.iter().flat_map(|v| v.to_le_bytes()).collect(),
                ComponentType::U16,
            ),
            (
                Semantic::Weights(set as u32),
                ws.iter().flat_map(|v| v.to_le_bytes()).collect(),
                ComponentType::F32,
            ),
        ] {
            let accessor = binary.push_attribute(
                BufferContainer(bytes),
                AccessorBuilder::new(mesh.vertex_count, Type::Vec4, component),
                None,
            );
            result.push((semantic, accessor));
        }
    }
    if sums.contains(&0.0) {
        return Err("vertex has zero total joint weight".into());
    }
    Ok(result)
}
