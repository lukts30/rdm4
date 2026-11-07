//! Explicit glTF-to-RDM vertex packing and coordinate conversion.

use std::{borrow::Cow, error::Error};

use nalgebra::{Matrix3, Matrix4};

use super::{
    RdmIndexFormat, RdmIndices, RdmMeshGroup, RdmModel, RdmModelName, RdmVertexFormat, RdmVertices,
};
use crate::{
    scene::{AttributeValues, Mesh},
    vertex::*,
    vertex_transform::*,
};

/// Target-specific decisions, applied after importing a scene.
pub struct RdmExportOptions {
    /// Destination vertex layout.
    pub format: TargetVertexFormat,
    /// Mirror X, adjusting winding and tangent handedness.
    pub mirror_x: bool,
    /// Leave the scene's mesh transform unapplied.
    pub no_transform: bool,
    /// Override material slots in model and primitive order.
    pub material_indices: Option<Vec<u32>>,
}

pub(super) fn pack_mesh(
    mut mesh: Mesh,
    options: &RdmExportOptions,
    material_indices: Option<&[u32]>,
) -> Result<RdmModel, Box<dyn Error>> {
    mesh.validate()?;
    if material_indices.is_some_and(|v| v.len() != mesh.primitives.len()) {
        return Err("material override count must match primitive count".into());
    }
    let format = match options.format {
        TargetVertexFormat::P4h_N4b_G4b_B4b_T2h => p4h_n4b_g4b_b4b_t2h().to_vec(),
        TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b => p4h_n4b_g4b_b4b_t2h_i4b().to_vec(),
        TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_I4b_W4b => p4h_n4b_g4b_b4b_t2h_i4b_w4b().to_vec(),
        TargetVertexFormat::P3f_N3f_G3f_B3f_T2f_C4b => p3f_n3f_g3f_b3f_t2f_c4b().to_vec(),
        TargetVertexFormat::P4h_T2h_C4b => p4h_t2h_c4c().to_vec(),
        TargetVertexFormat::P4h_N4b_G4b_B4b_T2h_C4b_C4b => p4h_n4b_g4b_b4b_t2h_c4b_c4b().to_vec(),
    };
    if !options.mirror_x
        && (options.no_transform || mesh.transform.is_none())
        && mesh
            .attributes
            .iter()
            .map(|a| &a.descriptor)
            .eq(format.iter())
    {
        let columns = mesh.attributes.drain(..).map(|a| a.values).collect();
        return finish_mesh(mesh, format, columns, material_indices, false);
    }
    let column =
        |semantic, set, width, default: &[f32]| -> Result<Cow<'_, [f32]>, Box<dyn Error>> {
            mesh.attributes
                .iter()
                .filter(|a| a.descriptor.semantic == semantic)
                .nth(set)
                .map_or_else(
                    || Ok(Cow::Owned(default.repeat(mesh.vertex_count))),
                    |a| a.floats(width),
                )
        };
    let positions = column(VertexSemantic::Position, 0, 3, &[])?;
    if positions.len() != mesh.vertex_count * 3 {
        return Err("mesh has no positions".into());
    }
    let normals = column(VertexSemantic::Normal, 0, 3, &[0.0, 0.0, 1.0])?;
    let tangents = column(VertexSemantic::Tangent, 0, 4, &[0.0, 0.0, 0.0, 1.0])?;
    let texcoords = column(VertexSemantic::Texcoord, 0, 2, &[0.0, 0.0])?;
    let weights = column(VertexSemantic::Weight, 0, 4, &[0.0; 4])?;
    let joints = mesh
        .attributes
        .iter()
        .find(|a| a.descriptor.semantic == VertexSemantic::JointIndex);
    let has_joints = joints.is_some();
    let joints = match joints.map(|a| &a.values) {
        Some(AttributeValues::U16(values)) => Cow::Borrowed(values.as_slice()),
        None => Cow::Owned(vec![0; mesh.vertex_count * 4]),
        _ => return Err("RDM packing requires unpacked joint indices".into()),
    };
    if mesh.skeleton.is_some()
        && (!has_joints
            || !TargetVertexFormat::has_joints(&options.format)
            || !mesh
                .attributes
                .iter()
                .any(|a| a.descriptor.semantic == VertexSemantic::Weight))
    {
        return Err(
            "skinned export requires a joint vertex format and joint/weight attributes".into(),
        );
    }
    let mut base = if options.no_transform {
        Matrix4::identity()
    } else {
        mesh.transform
            .map(Matrix4::from)
            .unwrap_or_else(Matrix4::identity)
    };
    if options.mirror_x {
        base *= Matrix3::new(-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0).to_homogeneous();
    }
    let reverse = base.determinant().is_sign_negative();
    let mut context = TransformContext {
        base,
        transpose_inv_transform_mat3: base
            .fixed_resize::<3, 3>(0.0)
            .try_inverse()
            .ok_or("mesh transform is singular")?
            .transpose(),
    };
    let positions: Vec<_> = positions
        .as_chunks::<3>()
        .0
        .iter()
        .map(|v| context.transform_position(*v))
        .collect();
    let normals: Vec<_> = normals
        .as_chunks::<3>()
        .0
        .iter()
        .map(|v| context.transform_normal(*v))
        .collect();
    let tangent_vectors: Vec<_> = tangents
        .as_chunks::<4>()
        .0
        .iter()
        .map(|v| context.transform_tangent(*v))
        .collect();
    let bitangents: Vec<_> = normals
        .iter()
        .zip(&tangent_vectors)
        .zip(tangents.as_chunks::<4>().0.iter())
        .map(|((normal, tangent), source)| {
            b3f(
                *tangent,
                *normal,
                if reverse { source[3] } else { -source[3] },
            )
            .data
        })
        .collect();
    let mut columns = Vec::new();
    let mut color_set = 0;
    for descriptor in &format {
        let values = match descriptor.semantic {
            VertexSemantic::Position if descriptor.storage_type == StorageType::F16 => {
                AttributeValues::F16(positions.iter().flat_map(|p| p4h(*p).data).collect())
            }
            VertexSemantic::Position => {
                AttributeValues::F32(positions.iter().flat_map(|p| p3f(*p).data).collect())
            }
            VertexSemantic::Normal if descriptor.storage_type == StorageType::F32 => {
                AttributeValues::F32(normals.iter().flat_map(|v| n3f(*v).data).collect())
            }
            VertexSemantic::Normal => AttributeValues::U32(
                normals
                    .iter()
                    .map(|v| u32::from_le_bytes(n4b(*v).data))
                    .collect(),
            ),
            VertexSemantic::Tangent if descriptor.storage_type == StorageType::F32 => {
                AttributeValues::F32(tangent_vectors.iter().flat_map(|v| g3f(*v).data).collect())
            }
            VertexSemantic::Tangent => AttributeValues::U32(
                tangent_vectors
                    .iter()
                    .map(|v| u32::from_le_bytes(g4b(*v).data))
                    .collect(),
            ),
            // Retain the legacy B3f descriptor's U32 storage and float bit patterns.
            VertexSemantic::Bitangent if descriptor.storage_count == 3 => {
                AttributeValues::U32(bitangents.iter().flatten().map(|v| v.to_bits()).collect())
            }
            VertexSemantic::Bitangent => AttributeValues::U32(
                bitangents
                    .iter()
                    .map(|v| {
                        let [x, y, z] = v.map(|v| (((v + 1.0) / 2.0) * 255.0).round() as u8);
                        u32::from_le_bytes([x, y, z, 0])
                    })
                    .collect(),
            ),
            VertexSemantic::Texcoord if descriptor.storage_type == StorageType::F16 => {
                AttributeValues::F16(
                    texcoords
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .flat_map(|v| t2h(*v).data)
                        .collect(),
                )
            }
            VertexSemantic::Texcoord => AttributeValues::F32(texcoords.to_vec()),
            VertexSemantic::JointIndex => {
                if joints.iter().any(|&joint| joint > 255) {
                    return Err("RDM joint indices must fit in one byte".into());
                }
                AttributeValues::U32(
                    joints
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|v| u32::from_le_bytes(i4b(*v).data))
                        .collect(),
                )
            }
            VertexSemantic::Weight => AttributeValues::U32(
                weights
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|v| u32::from_le_bytes(w4b(*v).data))
                    .collect(),
            ),
            VertexSemantic::Color => {
                let color = mesh
                    .attributes
                    .iter()
                    .filter(|a| a.descriptor.semantic == VertexSemantic::Color)
                    .nth(color_set);
                color_set += 1;
                let mut words = match color.map(|a| &a.values) {
                    Some(AttributeValues::U32(values)) => values.clone(),
                    Some(AttributeValues::U16(values)) => values
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|v| u32::from_le_bytes(std::array::from_fn(|i| (v[i] / 256) as u8)))
                        .collect(),
                    Some(AttributeValues::F32(values)) => values
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|v| u32::from_le_bytes(std::array::from_fn(|i| (v[i] * 255.0) as u8)))
                        .collect(),
                    None => vec![u32::from_le_bytes([255, 255, 255, 0]); mesh.vertex_count],
                    _ => return Err("unsupported color encoding for RDM packing".into()),
                };
                if options.format == TargetVertexFormat::P4h_T2h_C4b {
                    for word in &mut words {
                        *word = u32::from_le_bytes(word.to_le_bytes().map(|v| v.wrapping_sub(128)));
                    }
                }
                AttributeValues::U32(words)
            }
            _ => return Err("unsupported target vertex semantic".into()),
        };
        columns.push(values);
    }
    finish_mesh(mesh, format, columns, material_indices, reverse)
}

fn finish_mesh(
    mesh: Mesh,
    format: Vec<VertexIdentifier>,
    columns: Vec<AttributeValues>,
    material_indices: Option<&[u32]>,
    reverse: bool,
) -> Result<RdmModel, Box<dyn Error>> {
    let mut bounds_min = [f32::INFINITY; 3];
    let mut bounds_max = [f32::NEG_INFINITY; 3];
    let stored = match &columns[0] {
        AttributeValues::F16(values) => values
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|v| v[..3].iter().map(|v| v.to_f32()))
            .collect(),
        AttributeValues::F32(values) => values.clone(),
        _ => unreachable!(),
    };
    for position in stored.as_chunks::<3>().0.iter() {
        for axis in 0..3 {
            if !position[axis].is_finite() {
                return Err("nonfinite packed vertex position".into());
            }
            bounds_min[axis] = bounds_min[axis].min(position[axis]);
            bounds_max[axis] = bounds_max[axis].max(position[axis]);
        }
    }
    let mut indices = mesh.indices;
    if reverse {
        for triangle in indices.as_chunks_mut::<3>().0 {
            triangle.swap(1, 2);
        }
    }
    let storage_type = if indices.iter().any(|&index| index > u16::MAX as u32) {
        StorageType::U32
    } else {
        StorageType::U16
    };
    Ok(RdmModel {
        name: RdmModelName {
            name: Some("basalt_crusher_others_lod0".into()),
        },
        vertex_format: RdmVertexFormat {
            attributes: format,
            selector: u32::from(mesh.skeleton.is_some()),
        },
        index_format: RdmIndexFormat { storage_type },
        vertices: RdmVertices {
            count: mesh.vertex_count,
            attributes: columns,
        },
        indices: RdmIndices { values: indices },
        groups: mesh
            .primitives
            .into_iter()
            .enumerate()
            .map(|(i, primitive)| {
                Ok(RdmMeshGroup {
                    first_index: u32::try_from(primitive.indices.start)?,
                    index_count: u32::try_from(primitive.indices.len())?,
                    material: material_indices.map_or(primitive.material, |v| v[i]),
                })
            })
            .collect::<Result<_, Box<dyn Error>>>()?,
        skeleton_index: mesh.skeleton.map_or(-1, |i| i as i32),
        bounds_min,
        bounds_max,
    })
}
