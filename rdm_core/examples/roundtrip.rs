//! Read RDM files, print a summary and check that writing reproduces them.
//!
//! cargo run -p rdm_core --example roundtrip -- FILE... [--out DIR]
//!
//! Set `RUST_LOG=rdm_core=trace` to log every block.

use std::path::PathBuf;
use std::process::ExitCode;

use rdm_core::Document;

fn main() -> ExitCode {
    env_logger::init();
    let mut args = std::env::args().skip(1);
    let mut files = Vec::new();
    let mut out_dir = None;
    while let Some(arg) = args.next() {
        if arg == "--out" {
            out_dir = args.next().map(PathBuf::from);
        } else {
            files.push(PathBuf::from(arg));
        }
    }

    let mut failures = 0;
    for path in &files {
        match check(path, out_dir.as_ref()) {
            Ok(summary) => println!("ok   {}: {summary}", path.display()),
            Err(error) => {
                failures += 1;
                println!("FAIL {}: {error}", path.display());
            }
        }
    }
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn check(path: &PathBuf, out_dir: Option<&PathBuf>) -> Result<String, Box<dyn std::error::Error>> {
    let input = std::fs::read(path)?;
    let document = Document::from_bytes(&input)?;
    let output = document.to_bytes()?;
    if output != input {
        return Err("output differs from input".into());
    }
    if let Some(dir) = out_dir {
        std::fs::write(dir.join(path.file_name().unwrap()), &output)?;
    }

    let root = document.root();
    let mut summary = format!("{:?} endian, {} bytes", document.byte_order(), input.len());
    if let Some(model) = &root.model {
        summary += &format!(
            ", {} vertices, {} indices, {} groups",
            model.vertices.count,
            model.indices.len(),
            model.groups.len()
        );
    }
    if let Some(skin) = &root.skin {
        summary += &format!(", {} joints", skin.joints.len());
    }
    if let Some(animation) = &root.animation {
        summary += &format!(", {} tracks", animation.tracks.len());
    }
    Ok(summary)
}
