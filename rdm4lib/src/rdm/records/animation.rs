use rdm_derive::RdmRecord;

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
