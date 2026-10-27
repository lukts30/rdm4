//! Decoded numeric payloads whose layout comes from format records.

use half::f16;

/// Decoded vertex block of a `Model`.
///
/// `attributes[i]` holds the values of `vertex_format.attributes[i]` for all
/// vertices: `count * components` values in vertex order. Packed attributes
/// are `U32` words decoded in the file's byte order.
#[derive(Debug, Clone, PartialEq)]
pub struct Vertices {
    pub count: usize,
    pub attributes: Vec<AttributeValues>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValues {
    U16(Vec<u16>),
    U32(Vec<u32>),
    F16(Vec<f16>),
    F32(Vec<f32>),
}

impl AttributeValues {
    pub fn len(&self) -> usize {
        match self {
            Self::U16(values) => values.len(),
            Self::U32(values) => values.len(),
            Self::F16(values) => values.len(),
            Self::F32(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Decoded index block of a `Model`, typed by its `IndexFormat`.
#[derive(Debug, Clone, PartialEq)]
pub enum Indices {
    U16(Vec<u16>),
    U32(Vec<u32>),
}

impl Indices {
    pub fn len(&self) -> usize {
        match self {
            Self::U16(values) => values.len(),
            Self::U32(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<u32> {
        match self {
            Self::U16(values) => values.get(index).map(|&v| u32::from(v)),
            Self::U32(values) => values.get(index).copied(),
        }
    }
}
