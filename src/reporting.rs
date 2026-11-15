//! Human-facing progress and summaries for CLI commands, separate from codecs.

use log::{debug, info};
use rdm4lib::{rdm::RdmDocument, scene::Scene};
use std::path::Path;

pub(crate) fn input(path: &Path, bytes: u64) {
    info!("Reading {} ({} bytes)", path.display(), bytes);
}

pub(crate) fn written(path: &Path) {
    info!("Wrote {}", path.display());
}

pub(crate) fn available(document: &RdmDocument) {
    info!(
        "Available: {} models, {} materials, {} joints, {} animations",
        document.models.as_ref().map_or(0, Vec::len),
        document.materials.as_ref().map_or(0, Vec::len),
        document.skin.as_ref().map_or(0, |skin| skin.joints.len()),
        document.animations.as_ref().map_or(0, Vec::len)
    );
}

pub(crate) fn models(document: &RdmDocument, mesh_index: Option<usize>) {
    for (index, model) in document.models.iter().flatten().enumerate() {
        info!(
            "  Model #{} {:?}: {} vertices, {} triangles, {} groups",
            mesh_index.unwrap_or(index),
            model.name.name.as_deref().unwrap_or("<unnamed>"),
            model.vertices.count,
            model.indices.values.len() / 3,
            model.groups.len()
        );
        info!(
            "    Vertex format: {}; index type: {:?}",
            model.vertex_format, model.index_format.storage_type
        );
        debug!(
            "    Bounds: {:?} .. {:?}",
            model.bounds_min, model.bounds_max
        );
        for (index, group) in model.groups.iter().enumerate() {
            info!(
                "    Group #{index}: material {}, first index {}, index count {}",
                group.material, group.first_index, group.index_count
            );
        }
    }
}

pub(crate) fn meshes(scene: &Scene) {
    for (index, mesh) in scene.meshes.iter().enumerate() {
        info!(
            "  Model #{index} {:?}: {} vertices, {} triangles, {} groups",
            mesh.name.as_deref().unwrap_or("<unnamed>"),
            mesh.vertex_count,
            mesh.indices.len() / 3,
            mesh.primitives.len()
        );
        for (index, primitive) in mesh.primitives.iter().enumerate() {
            info!(
                "    Group #{index}: material {}, indices {:?}",
                primitive.material, primitive.indices
            );
        }
    }
}

pub(crate) fn scene(scene: &rdm4lib::scene::Scene) {
    info!(
        "Exporting {} models, {} skeletons ({} joints), {} animations",
        scene.meshes.len(),
        scene.skeletons.len(),
        scene
            .skeletons
            .iter()
            .map(|skin| skin.joints.len())
            .sum::<usize>(),
        scene.animations.len()
    );
    for animation in &scene.animations {
        info!(
            "  Animation {:?}: {} tracks, {} ms, {} samples/s",
            animation.name,
            animation.tracks.len(),
            animation.duration_millis,
            animation.sampling_rate
        );
    }
}
