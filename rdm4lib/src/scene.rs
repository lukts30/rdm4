//! Typed conversion data without file framing, source placement or byte-order state.

#![forbid(unsafe_code)]

use std::{borrow::Cow, error::Error, ops::Range};

use crate::vertex::{StorageType, VertexIdentifier, VertexSemantic};

/// Static meshes in scene order; each currently maps to one node.
pub struct Scene {
    /// All meshes selected for export.
    pub meshes: Vec<Mesh>,
}

/// Geometry shared by scene consumers, retaining the source attribute encoding.
pub struct Mesh {
    /// Optional source model name.
    pub name: Option<String>,
    /// Number of vertices represented by every attribute.
    pub vertex_count: usize,
    /// Typed columns in source descriptor order.
    pub attributes: Vec<Attribute>,
    /// Triangle indices into the attribute columns.
    pub indices: Vec<u32>,
    /// Draw ranges and their material slots.
    pub primitives: Vec<Primitive>,
}

/// One indexed draw range.
pub struct Primitive {
    /// Half-open range of indices, covering complete triangles.
    pub indices: Range<usize>,
    /// Shared material slot.
    pub material: u32,
}

/// An attribute descriptor and its scalar column.
pub struct Attribute {
    /// Semantic, interpretation and component layout.
    pub descriptor: VertexIdentifier,
    /// Native scalar values; packed attributes retain whole numeric words.
    pub values: AttributeValues,
}

/// Scalar values of an attribute, in vertex order.
#[derive(Debug, Clone, PartialEq)]
pub enum AttributeValues {
    /// Unsigned 16-bit components.
    U16(Vec<u16>),
    /// Unsigned 32-bit components or packed words.
    U32(Vec<u32>),
    /// Half-precision components.
    F16(Vec<half::f16>),
    /// Single-precision components.
    F32(Vec<f32>),
}

impl AttributeValues {
    pub(crate) fn storage_and_len(&self) -> (StorageType, usize) {
        match self {
            Self::U16(v) => (StorageType::U16, v.len()),
            Self::U32(v) => (StorageType::U32, v.len()),
            Self::F16(v) => (StorageType::F16, v.len()),
            Self::F32(v) => (StorageType::F32, v.len()),
        }
    }
}

impl Mesh {
    pub(crate) fn validate(&self) -> Result<(), Box<dyn Error>> {
        if self.vertex_count == 0
            || self.indices.is_empty()
            || self.primitives.is_empty()
            || !self.indices.len().is_multiple_of(3)
            || self
                .indices
                .iter()
                .any(|&i| i as usize >= self.vertex_count)
        {
            return Err("scene mesh requires nonempty triangles with in-range indices".into());
        }
        for primitive in &self.primitives {
            let range = &primitive.indices;
            if range.start >= range.end
                || range.end > self.indices.len()
                || !range.start.is_multiple_of(3)
                || !range.end.is_multiple_of(3)
            {
                return Err(
                    "scene primitive must cover complete triangles within the index buffer".into(),
                );
            }
        }
        for attribute in &self.attributes {
            let d = &attribute.descriptor;
            let (storage, length) = attribute.values.storage_and_len();
            let expected = self.vertex_count.checked_mul(d.storage_count as usize);
            if d.storage_count == 0 || storage != d.storage_type || expected != Some(length) {
                return Err(format!(
                    "{:?}: attribute storage/count disagrees with its descriptor and {} vertices",
                    d.semantic, self.vertex_count
                )
                .into());
            }
        }
        Ok(())
    }
}

impl Attribute {
    /// Borrow unchanged F32 values; allocate only for conversion or dropped components.
    pub(crate) fn floats(&self, width: usize) -> Result<Cow<'_, [f32]>, Box<dyn Error>> {
        let count = self.descriptor.storage_count as usize;
        let result = match &self.values {
            AttributeValues::F32(values) if count == width => Cow::Borrowed(values.as_slice()),
            AttributeValues::F32(values) if count >= width => Cow::Owned(
                values
                    .chunks_exact(count)
                    .flat_map(|v| v[..width].iter().copied())
                    .collect(),
            ),
            AttributeValues::F16(values) if count >= width => Cow::Owned(
                values
                    .chunks_exact(count)
                    .flat_map(|v| v[..width].iter().map(|v| v.to_f32()))
                    .collect(),
            ),
            AttributeValues::U32(values)
                if count == 1
                    && width == 3
                    && self.descriptor.interpretation.0 == 6
                    && matches!(
                        self.descriptor.semantic,
                        VertexSemantic::Normal | VertexSemantic::Tangent
                    ) =>
            {
                Cow::Owned(
                    values
                        .iter()
                        .flat_map(|word| {
                            let bytes = word.to_le_bytes();
                            let vector = [bytes[0], bytes[1], bytes[2]]
                                .map(|v| 2.0 * f32::from(v) / 255.0 - 1.0);
                            let length = vector.iter().map(|v| v * v).sum::<f32>().sqrt();
                            vector.map(|v| v / length)
                        })
                        .collect(),
                )
            }
            _ => {
                return Err(format!(
                    "{:?}: unsupported {:?} x {count} encoding for {width} float components",
                    self.descriptor.semantic, self.descriptor.storage_type
                )
                .into())
            }
        };
        if result.iter().any(|value| !value.is_finite()) {
            return Err(format!(
                "{:?}: glTF requires finite attribute values",
                self.descriptor.semantic
            )
            .into());
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        gltf_export::{build_scene, GltfExportFormat},
        vertex::VertexIdentifier,
    };

    #[test]
    fn export_retains_float_attribute_bits_without_normalizing() {
        let positions = [[-0.0f32, 0.0, 0.0], [1.0, -0.0, 0.0], [0.0, 1.0, -0.0]];
        let normal = [f32::from_bits(0x3f800001), -0.0, 0.0];
        let tangent = [-0.0, f32::from_bits(0x3f800001), 0.0, -1.0];
        let mesh = Mesh {
            name: Some("precision".into()),
            vertex_count: 3,
            attributes: vec![
                Attribute {
                    descriptor: VertexIdentifier::p3f(),
                    values: AttributeValues::F32(positions.into_iter().flatten().collect()),
                },
                Attribute {
                    descriptor: VertexIdentifier::n3f(),
                    values: AttributeValues::F32(normal.repeat(3)),
                },
                Attribute {
                    descriptor: VertexIdentifier {
                        storage_count: 4,
                        ..VertexIdentifier::g3f()
                    },
                    values: AttributeValues::F32(tangent.repeat(3)),
                },
            ],
            indices: vec![0, 1, 2],
            primitives: vec![Primitive {
                indices: 0..3,
                material: 0,
            }],
        };
        let output = tempfile::tempdir().unwrap();
        build_scene(
            Scene { meshes: vec![mesh] },
            Some(output.path().into()),
            true,
            GltfExportFormat::Glb,
        )
        .unwrap();
        let (document, buffers, _) = gltf::import(output.path().join("out.glb")).unwrap();
        let primitive = document
            .meshes()
            .next()
            .unwrap()
            .primitives()
            .next()
            .unwrap();
        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
        assert_eq!(
            reader
                .read_positions()
                .unwrap()
                .map(|p| p.map(f32::to_bits))
                .collect::<Vec<_>>(),
            positions.map(|p| p.map(f32::to_bits))
        );
        for value in reader.read_normals().unwrap() {
            assert_eq!(value.map(f32::to_bits), normal.map(f32::to_bits));
        }
        for value in reader.read_tangents().unwrap() {
            assert_eq!(value.map(f32::to_bits), tangent.map(f32::to_bits));
        }
    }
}
