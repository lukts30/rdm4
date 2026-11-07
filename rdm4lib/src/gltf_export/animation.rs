//! Shared animation serialization; no interpolation or quaternion normalization.

#![forbid(unsafe_code)]

use std::{collections::HashSet, error::Error};

use gltf::json::{
    self,
    accessor::{ComponentType, Type},
    validation::Checked::Valid,
};

use crate::{
    gltf_buffers::{AccessorBuilder, BufferContainer, GltfBuffers},
    scene::Animation,
};

pub(super) fn build(
    animation: &Animation,
    joint_count: usize,
    first_node: usize,
    binary: &mut GltfBuffers,
) -> Result<json::Animation, Box<dyn Error>> {
    let mut targets = HashSet::new();
    for track in &animation.tracks {
        if track.joint >= joint_count || !targets.insert(track.joint) {
            return Err(format!(
                "animation {:?}: invalid or repeated joint {}",
                animation.name, track.joint
            )
            .into());
        }
        if track.frames.is_empty()
            || track.frames.iter().any(|frame| {
                !frame.time.is_finite()
                    || frame.time < 0.0
                    || frame
                        .rotation
                        .iter()
                        .chain(&frame.translation)
                        .any(|v| !v.is_finite())
            })
            || track
                .frames
                .windows(2)
                .any(|pair| pair[0].time >= pair[1].time)
        {
            return Err(format!("animation {:?}, joint {}: expected finite poses and strictly increasing, nonnegative key times", animation.name, track.joint).into());
        }
    }
    let count: usize = animation
        .tracks
        .iter()
        .map(|track| track.frames.len())
        .sum();
    if count == 0 {
        return Err(format!("animation {:?} has no joint tracks", animation.name).into());
    }
    // Keep rotations, translations and times in one payload, matching legacy placement.
    let mut payload = Vec::with_capacity(count * 32);
    for frame in animation.tracks.iter().flat_map(|track| &track.frames) {
        payload.extend(frame.rotation.into_iter().flat_map(f32::to_le_bytes));
    }
    for frame in animation.tracks.iter().flat_map(|track| &track.frames) {
        payload.extend(frame.translation.into_iter().flat_map(f32::to_le_bytes));
    }
    for frame in animation.tracks.iter().flat_map(|track| &track.frames) {
        payload.extend(frame.time.to_le_bytes());
    }
    let buffer = binary.push_buffer(BufferContainer::U8(payload));
    let mut samplers = Vec::new();
    let mut channels = Vec::new();
    let mut start = 0;
    for track in &animation.tracks {
        let end = start + track.frames.len();
        let rotation = binary.push_accessor_range(
            buffer,
            start * 16..end * 16,
            AccessorBuilder::new(track.frames.len(), Type::Vec4, ComponentType::F32),
        );
        let translation = binary.push_accessor_range(
            buffer,
            count * 16 + start * 12..count * 16 + end * 12,
            AccessorBuilder::new(track.frames.len(), Type::Vec3, ComponentType::F32),
        );
        let time = binary.push_accessor_range(
            buffer,
            count * 28 + start * 4..count * 28 + end * 4,
            AccessorBuilder::new(track.frames.len(), Type::Scalar, ComponentType::F32).bounds(
                Some(json::Value::from(vec![track.frames[0].time])),
                Some(json::Value::from(vec![
                    track.frames.last().expect("validated frames").time,
                ])),
            ),
        );
        for (output, property) in [
            (rotation, json::animation::Property::Rotation),
            (translation, json::animation::Property::Translation),
        ] {
            channels.push(json::animation::Channel {
                sampler: json::Index::new(samplers.len() as u32),
                target: json::animation::Target {
                    node: json::Index::new((first_node + track.joint) as u32),
                    path: Valid(property),
                    extensions: None,
                    extras: None,
                },
                extensions: None,
                extras: None,
            });
            samplers.push(json::animation::Sampler {
                input: time,
                output,
                interpolation: Valid(json::animation::Interpolation::Linear),
                extensions: None,
                extras: None,
            });
        }
        start = end;
    }
    Ok(json::Animation {
        name: Some(animation.name.clone()),
        samplers,
        channels,
        extensions: None,
        // glTF key times do not encode the independent nominal RDM sample rate.
        extras: Some(serde_json::value::to_raw_value(&serde_json::json!({
            "rdm4": { "samplingRate": animation.sampling_rate }
        }))?),
    })
}
