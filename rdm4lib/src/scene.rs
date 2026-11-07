//! Conversion data without RDM framing, source placements or byte-order state.

#![forbid(unsafe_code)]

use crate::{
    rdm_anim::RdAnim, rdm_material::RdMaterial, vertex::VertexFormat2, MeshInfo, RdJoint, Triangle,
};

/// Meshes in scene order. Static RDM models currently map to one node each.
pub struct Scene {
    /// All meshes selected for export.
    pub meshes: Vec<Mesh>,
}

/// Shared conversion mesh; `RdModell` remains a compatibility name.
/// Packed vertex storage is retained while attribute handling is migrated.
pub struct Mesh {
    /// Optional source model name.
    pub name: Option<String>,
    pub(crate) pending_joints: Option<Vec<RdJoint>>,
    /// Draw ranges and material slots.
    pub mesh_info: Vec<MeshInfo>,
    /// Activated skeleton.
    pub joints: Option<Vec<RdJoint>>,
    /// Triangle indices into the vertex buffer.
    pub triangle_indices: Vec<Triangle>,
    /// Vertex attributes and their current packed storage.
    pub vertex: VertexFormat2,
    pub(crate) anim: Option<RdAnim>,
    /// Optional external texture inputs.
    pub mat: Option<RdMaterial>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        gltf_export::{build_scene, GltfExportFormat},
        vertex::VertexIdentifier,
        RdModell,
    };

    #[test]
    fn export_retains_float_attribute_bits_without_normalizing() {
        let positions = [[-0.0f32, 0.0, 0.0], [1.0, -0.0, 0.0], [0.0, 1.0, -0.0]];
        let normal = [f32::from_bits(0x3f800001), -0.0, 0.0];
        let tangent = [-0.0, f32::from_bits(0x3f800001), 0.0];
        let bytes: Vec<_> = positions
            .into_iter()
            .flat_map(|position| {
                position
                    .into_iter()
                    .chain(normal)
                    .chain(tangent)
                    .flat_map(f32::to_le_bytes)
            })
            .collect();
        let mut mesh = RdModell::from("rdm/fishery_others_lod2.rdm");
        mesh.vertex = VertexFormat2::new(
            vec![
                VertexIdentifier::p3f(),
                VertexIdentifier::n3f(),
                VertexIdentifier::g3f(),
            ]
            .into_boxed_slice(),
            3,
            36,
            bytes.into(),
        );
        mesh.triangle_indices = vec![Triangle { indices: [0, 1, 2] }];
        mesh.mesh_info = vec![MeshInfo {
            start_index_location: 0,
            index_count: 3,
            material: 0,
            _padding: [0; 16],
        }];
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
            assert_eq!(
                value.map(f32::to_bits),
                [tangent[0], tangent[1], tangent[2], 1.0].map(f32::to_bits)
            );
        }
    }
}
