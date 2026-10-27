//! Reads RDM files, prints a summary and checks the byte-identical round trip.
//!
//! Usage: cargo run -p rdm_core --example rdm_roundtrip -- FILE...

use std::process::ExitCode;

use rdm_core::records::{AttributeValues, Indices, Root};
use rdm_core::Document;

fn main() -> ExitCode {
    let mut failed = false;
    for path in std::env::args().skip(1) {
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                eprintln!("{path}: {error}");
                failed = true;
                continue;
            }
        };
        let document = match Document::from_bytes(&bytes) {
            Ok(document) => document,
            Err(error) => {
                eprintln!("{path}: {error}");
                failed = true;
                continue;
            }
        };
        let identical = document.to_bytes().is_ok_and(|written| written == bytes);
        failed |= !identical;
        let status = if identical { "identical" } else { "DIFFERS" };
        println!("{path}: {:?}, round trip {status}", document.byte_order());
        print_summary(document.root());
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_summary(root: &Root) {
    for (index, model) in root.models.iter().flatten().enumerate() {
        let format: Vec<String> = model
            .vertex_format
            .attributes
            .iter()
            .map(|a| format!("{:?}:{:?}x{}", a.semantic, a.storage_type, a.storage_count))
            .collect();
        let index_count = match &model.indices {
            Indices::U16(values) => values.len(),
            Indices::U32(values) => values.len(),
        };
        println!(
            "  model[{index}] {} vertices [{}], {index_count} indices, {} groups",
            model.vertices.count,
            format.join(" "),
            model.groups.len()
        );
        let columns = model.vertices.attributes.iter().map(|values| match values {
            AttributeValues::U16(v) => v.len(),
            AttributeValues::U32(v) => v.len(),
            AttributeValues::F16(v) => v.len(),
            AttributeValues::F32(v) => v.len(),
        });
        println!("    column lengths {:?}", columns.collect::<Vec<_>>());
    }
    let materials = root.materials.as_ref().map_or(0, Vec::len);
    let joints = root.skin.as_ref().map_or(0, |skin| skin.joints.len());
    println!("  {materials} materials, {joints} joints");
    if let Some(animation) = &root.animation {
        let frames: usize = animation.tracks.iter().map(|t| t.frames.len()).sum();
        println!(
            "  animation {:?}: {} tracks, {frames} frames, {} Hz",
            String::from_utf8_lossy(&animation.name),
            animation.tracks.len(),
            animation.sampling_rate
        );
    }
}
