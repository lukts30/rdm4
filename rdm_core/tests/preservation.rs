//! Unknown bytes survive round trips; value edits are written in place;
//! structural edits that would misplace preserved bytes are rejected.

use std::{fs, path::PathBuf};

use rdm_core::Document;

fn fixture(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../rdm4lib/rdm/benchmark")
        .join(name);
    fs::read(path).unwrap()
}

fn fill(input: &mut [u8], range: std::ops::Range<usize>, seed: u8) {
    for (i, byte) in input[range].iter_mut().enumerate() {
        *byte = seed.wrapping_add((i as u8).wrapping_mul(7));
    }
}

#[test]
fn preserves_unknown_record_bytes() {
    let original = fixture("03-impostor.rdm");
    let mut input = original.clone();
    fill(&mut input, 0x030..0x04c, 0x11); // Root bytes 20..48
    fill(&mut input, 0x11c..0x144, 0x22); // Model bytes 52..92
    fill(&mut input, 0x184..0x198, 0x33); // VertexFormat bytes 4..24
    fill(&mut input, 0x260..0x288, 0x44); // Material bytes 8..48

    let document = Document::from_bytes(&input).unwrap();
    assert_eq!(document.to_bytes().unwrap(), input);

    let unmodified = Document::from_bytes(&original).unwrap();
    assert_eq!(document.root, unmodified.root);
    assert_ne!(document, unmodified);
}

#[test]
fn preserves_unknown_animation_bytes() {
    let mut input = fixture("02-hook_idle_01.rdm");
    fill(&mut input, 0x126..0x146, 0x55); // Animation bytes 16..48
    fill(&mut input, 0x167..0x177, 0x66); // Track bytes 8..24
    let document = Document::from_bytes(&input).unwrap();
    assert_eq!(document.to_bytes().unwrap(), input);
}

#[test]
fn writes_edited_values_in_place() {
    let input = fixture("02-hook_idle_01.rdm");
    let mut document = Document::from_bytes(&input).unwrap();
    let frame = &mut document.root.animations.as_mut().unwrap()[0].tracks[0].frames[1];
    frame.time += 1.0;

    let output = document.to_bytes().unwrap();
    let changed: Vec<usize> = (0..input.len())
        .filter(|&i| input[i] != output[i])
        .collect();
    // Frames start at 0x190; frame 1's time is at 0x190 + 32 + 28.
    assert!(!changed.is_empty() && changed.iter().all(|i| (0x1cc..0x1d0).contains(i)));
    assert_eq!(Document::from_bytes(&output).unwrap(), document);
}

#[test]
fn relays_out_resized_strings_and_frame_arrays() {
    let input = fixture("09-hop_worker_stand_01.rdm");
    let mut document = Document::from_bytes(&input).unwrap();
    let track = &mut document.root.animations.as_mut().unwrap()[0].tracks[0];
    track.joint_name.push_str("_renamed");
    let last = *track.frames.last().unwrap();
    track.frames.push(last);

    let output = document.to_bytes().unwrap();
    assert_eq!(output.len(), input.len() + "_renamed".len() + 32);
    assert_eq!(Document::from_bytes(&output).unwrap(), document);
}

#[test]
fn rejects_structural_edits_that_misplace_unknown_bytes() {
    let input = fixture("09-hop_worker_stand_01.rdm");
    let document = Document::from_bytes(&input).unwrap();

    let mut added = document.clone();
    let tracks = &mut added.root.animations.as_mut().unwrap()[0].tracks;
    tracks.push(tracks[0].clone());
    let error = added.to_bytes().unwrap_err();
    assert_eq!(error.context(), "Animation.tracks", "{error}");
    assert!(
        error
            .expected()
            .contains("preserved unknown bytes per Track record"),
        "{error}"
    );

    let mut removed = document.clone();
    removed.root.animations.as_mut().unwrap()[0].tracks.pop();
    let error = removed.to_bytes().unwrap_err();
    assert_eq!(error.context(), "Document", "{error}");
}
