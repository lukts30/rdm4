//! glTF channel sampling into shared scene animation clips.
//! Baking follows https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html#animations-interpolations.

#![forbid(unsafe_code)]

use std::{collections::HashMap, error::Error};

use gltf::animation::{util::ReadOutputs, Interpolation};
use nalgebra::{Quaternion, UnitQuaternion};

use super::{node_get_local_transform, ImportedGltf};
use crate::{
    rdm_anim::{Frame, FrameCollection, RdAnim},
    scene::{Animation, Keyframe, Skeleton, Track as SceneTrack},
    RdJoint,
};

/// Controls conversion from independent glTF channels to RDM pose tracks.
#[derive(Clone, Copy)]
pub struct AnimationImportOptions {
    /// Bake glTF interpolation at the nominal rate, retaining every original key.
    /// STEP and cubic curves are approximated between the resulting RDM samples.
    pub interpolate: bool,
    /// Override `extras.rdm4.samplingRate`; absent metadata defaults to 15 Hz.
    pub sampling_rate: Option<u32>,
    /// Legacy interval count for constant tracks of joints without channels.
    pub idle_intervals: usize,
}

impl Default for AnimationImportOptions {
    fn default() -> Self {
        Self {
            interpolate: false,
            sampling_rate: None,
            idle_intervals: 6,
        }
    }
}

struct Curve<const N: usize> {
    times: Vec<f32>,
    values: Vec<[f32; N]>,
    interpolation: Interpolation,
}

impl<const N: usize> Curve<N> {
    fn new(
        times: Vec<f32>,
        values: Vec<[f32; N]>,
        interpolation: Interpolation,
    ) -> Result<Self, Box<dyn Error>> {
        let cubic = interpolation == Interpolation::CubicSpline;
        if times.is_empty()
            || (cubic && times.len() < 2)
            || values.len() != times.len() * if cubic { 3 } else { 1 }
            || times.iter().any(|t| !t.is_finite() || *t < 0.0)
            || times.windows(2).any(|t| t[0] >= t[1])
            || values.iter().flatten().any(|v| !v.is_finite())
        {
            return Err("invalid channel times or output count/values".into());
        }
        Ok(Self {
            times,
            values,
            interpolation,
        })
    }

    fn key(&self, index: usize) -> [f32; N] {
        self.values[if self.interpolation == Interpolation::CubicSpline {
            index * 3 + 1
        } else {
            index
        }]
    }

    fn requires_baking(&self) -> bool {
        self.interpolation == Interpolation::CubicSpline
            || (self.interpolation == Interpolation::Step
                && self.values.iter().any(|v| *v != self.key(0)))
    }

    fn sample(&self, time: f32, rotation: bool) -> Result<[f32; N], Box<dyn Error>> {
        let next = self.times.partition_point(|t| *t < time);
        // Exact keys and held endpoints must not pass through arithmetic or normalization.
        if next == self.times.len() {
            return Ok(self.key(next - 1));
        }
        if next == 0 || self.times[next] == time {
            return Ok(self.key(next));
        }
        let previous = next - 1;
        let a = self.key(previous);
        let b = self.key(next);
        let duration = self.times[next] - self.times[previous];
        let t = (time - self.times[previous]) / duration;
        let mut value = match self.interpolation {
            Interpolation::Step => return Ok(a),
            Interpolation::Linear if rotation => {
                let quaternion = |v: [f32; N]| {
                    UnitQuaternion::new_unchecked(Quaternion::new(v[3], v[0], v[1], v[2]))
                };
                let q = quaternion(a)
                    .try_slerp(&quaternion(b), t, f32::EPSILON)
                    .ok_or("cannot interpolate rotation quaternion")?;
                std::array::from_fn(|i| q.coords[i])
            }
            Interpolation::Linear => std::array::from_fn(|i| (1.0 - t) * a[i] + t * b[i]),
            Interpolation::CubicSpline => {
                let t2 = t * t;
                let t3 = t2 * t;
                let outgoing = self.values[previous * 3 + 2];
                let incoming = self.values[next * 3];
                std::array::from_fn(|i| {
                    (2.0 * t3 - 3.0 * t2 + 1.0) * a[i]
                        + duration * (t3 - 2.0 * t2 + t) * outgoing[i]
                        + (-2.0 * t3 + 3.0 * t2) * b[i]
                        + duration * (t3 - t2) * incoming[i]
                })
            }
        };
        if rotation && self.interpolation == Interpolation::CubicSpline {
            let length = value.iter().map(|v| v * v).sum::<f32>().sqrt();
            if length == 0.0 || !length.is_finite() {
                return Err("cubic interpolation produced an invalid quaternion length".into());
            }
            value = value.map(|v| v / length);
        }
        if value.iter().any(|v| !v.is_finite()) {
            return Err("interpolation produced a nonfinite value".into());
        }
        Ok(value)
    }
}

#[derive(Default)]
struct Track {
    rotation: Option<Curve<4>>,
    translation: Option<Curve<3>>,
}

impl Track {
    fn times(
        &self,
        duration: f32,
        rate: u32,
        interpolate: bool,
    ) -> Result<Vec<f32>, Box<dyn Error>> {
        let channels: Vec<_> = self
            .rotation
            .as_ref()
            .map(|c| c.times.as_slice())
            .into_iter()
            .chain(self.translation.as_ref().map(|c| c.times.as_slice()))
            .collect();
        if !interpolate {
            if let Some(start) = channels
                .iter()
                .map(|times| times[0])
                .find(|time| *time > 0.0)
            {
                return Err(format!("first key must be at t=0 (starts at {start}); add a zero-time key or use --interpolate-animations").into());
            }
            if channels.iter().any(|times| *times != channels[0])
                || self.rotation.as_ref().is_some_and(Curve::requires_baking)
                || self
                    .translation
                    .as_ref()
                    .is_some_and(Curve::requires_baking)
            {
                return Err("channels require interpolation; export from Blender with 'Always Sample Animations' or use --interpolate-animations".into());
            }
            return Ok(channels[0].to_vec());
        }
        let steps = (f64::from(duration) * f64::from(rate)).floor();
        if rate == 0 || steps >= 1_000_000.0 {
            return Err("animation baking requires a positive rate and at most 1,000,000 grid samples; reduce --sampling-rate".into());
        }
        let mut times: Vec<_> = (0..=steps as u32)
            .map(|i| (f64::from(i) / f64::from(rate)) as f32)
            .chain(std::iter::once(duration))
            .chain(channels.into_iter().flatten().copied())
            .collect();
        times.sort_by(f32::total_cmp);
        times.dedup_by(|a, b| *a == *b);
        Ok(times)
    }
}

impl ImportedGltf {
    /// Compatibility API: require zero-starting, sampled channels without interpolation.
    pub fn read_animation(
        &self,
        joints: &[RdJoint],
        frames: usize,
        _tmax: f32,
    ) -> Option<Vec<RdAnim>> {
        Some(
            self.read_animations(
                joints,
                AnimationImportOptions {
                    idle_intervals: frames,
                    ..Default::default()
                },
            )
            .expect("cannot import animation"),
        )
    }

    /// Convert channels to RDM tracks. Baking is opt-in; source key values are copied exactly.
    pub fn read_animations(
        &self,
        joints: &[RdJoint],
        options: AnimationImportOptions,
    ) -> Result<Vec<RdAnim>, Box<dyn Error>> {
        let names = joints
            .iter()
            .map(|joint| joint.name.as_str())
            .collect::<Vec<_>>();
        let nodes = self
            .read_bind_poses()?
            .into_iter()
            .map(|(node, _, _)| node)
            .collect::<Vec<_>>();
        Ok(self
            .read_clips(&names, 0, &nodes, options)?
            .into_iter()
            .map(|clip| RdAnim {
                name: clip.name,
                time_max: clip.duration_millis,
                sampling_rate: clip.sampling_rate,
                anim_vec: clip
                    .tracks
                    .into_iter()
                    .map(|track| FrameCollection {
                        name: names[track.joint].into(),
                        frames: track
                            .frames
                            .into_iter()
                            .map(|frame| {
                                let [x, y, z, w] = frame.rotation;
                                Frame {
                                    rotation: [x, y, z, -w],
                                    translation: frame.translation,
                                    time: frame.time,
                                }
                            })
                            .collect(),
                    })
                    .collect(),
            })
            .collect())
    }

    /// Read local pose clips bound to one scene skeleton. Baking remains opt-in.
    pub fn read_scene_animations(
        &self,
        skeleton: &Skeleton,
        skeleton_index: usize,
        options: AnimationImportOptions,
    ) -> Result<Vec<Animation>, Box<dyn Error>> {
        let names = skeleton
            .joints
            .iter()
            .map(|joint| joint.name.as_str())
            .collect::<Vec<_>>();
        let nodes = self
            .read_bind_poses()?
            .into_iter()
            .map(|(node, _, _)| node)
            .collect::<Vec<_>>();
        self.read_clips(&names, skeleton_index, &nodes, options)
    }

    pub(super) fn read_clips(
        &self,
        joints: &[&str],
        skeleton_index: usize,
        joint_nodes: &[usize],
        options: AnimationImportOptions,
    ) -> Result<Vec<Animation>, Box<dyn Error>> {
        let mut result = Vec::new();
        for (index, animation) in self.gltf.animations().enumerate() {
            let rate = options
                .sampling_rate
                .or_else(|| {
                    animation
                        .extras()
                        .as_ref()
                        .and_then(|extra| {
                            serde_json::from_str::<serde_json::Value>(extra.get()).ok()
                        })
                        .and_then(|extra| extra["rdm4"]["samplingRate"].as_u64())
                        .and_then(|rate| u32::try_from(rate).ok())
                })
                .unwrap_or(15);
            let mut tracks: HashMap<String, Track> = HashMap::new();
            let mut duration = 0.0f64;
            for channel in animation.channels() {
                duration = duration.max(
                    channel.sampler().input().max().unwrap()[0]
                        .as_f64()
                        .unwrap(),
                );
                let node = channel.target().node();
                if !joint_nodes.contains(&node.index()) {
                    warn!(
                        "animation target node #{} is outside the selected skeleton",
                        node.index()
                    );
                    continue;
                }
                let name = self.node_get_name(&node);
                if !joints.contains(&name.as_str()) {
                    warn!("animation target {name:?} is not in the selected skeleton");
                    continue;
                }
                let reader = channel.reader(|buffer| Some(&self.buffers[buffer.index()]));
                let times: Vec<_> = reader
                    .read_inputs()
                    .ok_or("animation channel has no times")?
                    .collect();
                let interpolation = channel.sampler().interpolation();
                let context = |error| {
                    format!(
                        "animation #{index}, joint {name:?}, {:?}: {error}",
                        channel.target().property()
                    )
                };
                match reader
                    .read_outputs()
                    .ok_or("animation channel has no output")?
                {
                    ReadOutputs::Rotations(values) => {
                        let curve = Curve::new(times, values.into_f32().collect(), interpolation)
                            .map_err(context)?;
                        tracks.entry(name).or_default().rotation = Some(curve);
                    }
                    ReadOutputs::Translations(values) => {
                        let curve =
                            Curve::new(times, values.collect(), interpolation).map_err(context)?;
                        tracks.entry(name).or_default().translation = Some(curve);
                    }
                    _ => warn!(
                        "animation property {:?} is not supported",
                        channel.target().property()
                    ),
                }
            }
            let mut collections = Vec::new();
            for (joint_index, &name) in joints.iter().enumerate() {
                let node = self
                    .gltf
                    .nodes()
                    .filter(|node| joint_nodes.contains(&node.index()))
                    .find(|node| self.node_get_name(node) == name)
                    .ok_or_else(|| format!("joint {:?} has no glTF node", name))?;
                let origin = node_get_local_transform(&node);
                let translation = [
                    origin.translation.x,
                    origin.translation.y,
                    origin.translation.z,
                ];
                let q = origin.rotation.coords;
                let rotation = [q.x, q.y, q.z, -q.w];
                let frames = if let Some(track) = tracks.remove(name) {
                    let times = track
                        .times(duration as f32, rate, options.interpolate)
                        .map_err(|error| {
                            format!("animation #{index}, joint {:?}: {error}", name)
                        })?;
                    times
                        .into_iter()
                        .map(|time| {
                            let rotation = match &track.rotation {
                                Some(curve) => curve.sample(time, true)?,
                                None => rotation,
                            };
                            let translation = match &track.translation {
                                Some(curve) => curve.sample(time, false)?,
                                None => translation,
                            };
                            Ok(Keyframe {
                                time,
                                rotation,
                                translation,
                            })
                        })
                        .collect::<Result<_, Box<dyn Error>>>()
                        .map_err(|error| format!("animation #{index}, joint {:?}: {error}", name))?
                } else {
                    let intervals = if duration > 0.0 {
                        options.idle_intervals.max(1)
                    } else {
                        0
                    };
                    let interval = duration as f32 / intervals.max(1) as f32;
                    (0..=intervals)
                        .map(|i| Keyframe {
                            time: i as f32 * interval,
                            rotation,
                            translation,
                        })
                        .collect()
                };
                collections.push(SceneTrack {
                    joint: joint_index,
                    frames,
                });
            }
            collections.sort_by(|a, b| joints[a.joint].cmp(joints[b.joint]));
            result.push(Animation {
                name: format!("anim_{index}"),
                sampling_rate: rate,
                duration_millis: (duration * 1000.0) as u32,
                skeleton: skeleton_index,
                tracks: collections,
            });
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_spec_modes_and_copies_exact_keys_and_endpoints() {
        let linear = Curve::new(
            vec![1.0, 3.0],
            vec![[-0.0, 0.0, 0.0], [2.0, 4.0, 6.0]],
            Interpolation::Linear,
        )
        .unwrap();
        for time in [0.0, 1.0] {
            assert_eq!(
                linear.sample(time, false).unwrap().map(f32::to_bits),
                [-0.0f32, 0.0, 0.0].map(f32::to_bits)
            );
        }
        assert_eq!(linear.sample(2.0, false).unwrap(), [1.0, 2.0, 3.0]);
        assert_eq!(linear.sample(4.0, false).unwrap(), [2.0, 4.0, 6.0]);
        let step = Curve::new(vec![0.0, 2.0], vec![[1.0], [3.0]], Interpolation::Step).unwrap();
        assert_eq!(step.sample(1.0, false).unwrap(), [1.0]);
        assert_eq!(step.sample(2.0, false).unwrap(), [3.0]);
        let cubic = Curve::new(
            vec![0.0, 2.0],
            vec![[0.0], [0.0], [2.0], [0.0], [2.0], [0.0]],
            Interpolation::CubicSpline,
        )
        .unwrap();
        assert_eq!(cubic.sample(1.0, false).unwrap(), [1.5]);
        let s = std::f32::consts::FRAC_1_SQRT_2;
        let rotation = Curve::new(
            vec![0.0, 2.0],
            vec![[0.0, 0.0, 0.0, 1.0], [0.0, 0.0, -s, -s]],
            Interpolation::Linear,
        )
        .unwrap();
        let q = rotation.sample(1.0, true).unwrap();
        assert!((q[2] - (std::f32::consts::PI / 8.0).sin()).abs() < 1e-6);
        assert!((q[3] - (std::f32::consts::PI / 8.0).cos()).abs() < 1e-6);
        let cubic = Curve::new(
            vec![0.0, 2.0],
            vec![
                [0.0; 4],
                [0.0, -0.0, 0.0, 1.0],
                [0.0; 4],
                [0.0; 4],
                [0.0, 0.0, 1.0, 0.0],
                [0.0; 4],
            ],
            Interpolation::CubicSpline,
        )
        .unwrap();
        assert_eq!(
            cubic.sample(0.0, true).unwrap()[1].to_bits(),
            (-0.0f32).to_bits()
        );
        let q = cubic.sample(1.0, true).unwrap();
        assert!((q[2] - s).abs() < 1e-6 && (q[3] - s).abs() < 1e-6);
    }

    #[test]
    fn default_rejects_late_keys_and_unequal_grids_baking_retains_both() {
        let mut track = Track {
            rotation: None,
            translation: Some(
                Curve::new(
                    vec![0.25, 1.0],
                    vec![[0.0; 3], [1.0; 3]],
                    Interpolation::Linear,
                )
                .unwrap(),
            ),
        };
        assert!(track
            .times(1.0, 2, false)
            .unwrap_err()
            .to_string()
            .contains("t=0"));
        assert_eq!(track.times(1.0, 2, true).unwrap(), [0.0, 0.25, 0.5, 1.0]);
        track.translation.as_mut().unwrap().times[0] = 0.0;
        track.rotation = Some(
            Curve::new(
                vec![0.0, 0.5, 1.0],
                vec![[0.0, 0.0, 0.0, 1.0]; 3],
                Interpolation::Linear,
            )
            .unwrap(),
        );
        assert!(track
            .times(1.0, 2, false)
            .unwrap_err()
            .to_string()
            .contains("Always Sample"));
        assert_eq!(track.times(1.0, 2, true).unwrap(), [0.0, 0.5, 1.0]);
        assert!(track.times(1.0, 0, true).is_err());
        assert!(Curve::<1>::new(vec![0.0, 0.0], vec![[1.0]; 2], Interpolation::Linear).is_err());
    }
}
