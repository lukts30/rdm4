use rdm_derive::RdmRecord;

/// Named animation and its joint tracks.
#[derive(Debug, Clone, PartialEq, RdmRecord)]
#[rdm(strides(48))]
pub struct RdmAnimation {
    /// Name stored in the animation record.
    pub name: String,
    /// Per-joint keyframe sequences.
    pub tracks: Vec<RdmTrack>,
    /// Duration in the format's integer time units.
    pub time_max: u32,
    /// Nominal samples per second.
    pub sampling_rate: u32,
}

/// Keyframes affecting one named joint.
#[derive(Debug, Clone, PartialEq, RdmRecord)]
#[rdm(strides(24))]
pub struct RdmTrack {
    /// Joint name used to match the skeleton.
    pub joint_name: String,
    /// Rotation, translation and time samples.
    pub frames: Vec<RdmFrame>,
}

/// One animation keyframe, with quaternion components in `x, y, z, w` order.
#[derive(Debug, Clone, Copy, PartialEq, RdmRecord)]
#[rdm(strides(32))]
pub struct RdmFrame {
    /// Rotation quaternion.
    pub rotation: [f32; 4],
    /// Translation vector.
    pub translation: [f32; 3],
    /// Sample time in seconds.
    pub time: f32,
}
