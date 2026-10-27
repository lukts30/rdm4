//! Reads each file, writes it back and compares the bytes.
//!
//! `cargo run --release -p rdm_core --example roundtrip -- FILE...`
//! Set `RUST_LOG=debug` for log output.

use std::{fs, process::ExitCode};

use rdm_core::Document;

fn main() -> ExitCode {
    env_logger::init();
    let mut failures = 0;
    for path in std::env::args().skip(1) {
        match round_trip(&path) {
            Ok(summary) => println!("ok    {path}: {summary}"),
            Err(error) => {
                failures += 1;
                println!("FAIL  {path}: {error}");
            }
        }
    }
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn round_trip(path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let input = fs::read(path)?;
    let document = Document::from_bytes(&input)?;
    let output = document.to_bytes()?;
    if output != input {
        let position = input.iter().zip(&output).position(|(a, b)| a != b);
        let position = position.unwrap_or(input.len().min(output.len()));
        return Err(format!("output differs at {position:#x}").into());
    }

    let root = &document.root;
    let models = root.models.as_ref().map_or(0, Vec::len);
    let tracks: usize = root
        .animations
        .iter()
        .flatten()
        .map(|a| a.tracks.len())
        .sum();
    Ok(format!(
        "{} bytes, {:?}, {models} models, {tracks} animation tracks",
        input.len(),
        document.endian
    ))
}
