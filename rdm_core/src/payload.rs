//! Numeric payloads whose layout comes from format records.

use half::f16;

/// Vertex buffer decoded into one column per [`VertexDescriptor`].
///
/// `attributes[i]` holds `count * descriptors[i].count` values for the
/// descriptor at the same position in [`VertexFormat::attributes`], vertex
/// by vertex.
///
/// [`VertexDescriptor`]: crate::VertexDescriptor
/// [`VertexFormat::attributes`]: crate::VertexFormat::attributes
#[derive(Debug, Clone, PartialEq)]
pub struct Vertices {
    pub count: usize,
    pub attributes: Vec<AttributeData>,
}

/// Values of one vertex attribute. Packed `U32` attributes such as normals
/// keep each packed word as one number.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeData {
    U32(Vec<u32>),
    F16(Vec<f16>),
    F32(Vec<f32>),
}

/// Index buffer in the width given by [`IndexFormat::storage`].
///
/// [`IndexFormat::storage`]: crate::IndexFormat::storage
#[derive(Debug, Clone, PartialEq)]
pub enum Indices {
    U16(Vec<u16>),
    U32(Vec<u32>),
}

impl Indices {
    pub fn len(&self) -> usize {
        match self {
            Indices::U16(values) => values.len(),
            Indices::U32(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate over the indices widened to `u32`.
    pub fn iter(&self) -> Box<dyn Iterator<Item = u32> + '_> {
        match self {
            Indices::U16(values) => Box::new(values.iter().map(|&index| u32::from(index))),
            Indices::U32(values) => Box::new(values.iter().copied()),
        }
    }
}
