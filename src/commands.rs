//! Command execution, independent of CLI argument parsing.

use std::{
    fs::File,
    io::{self, BufReader, Write},
    path::{Path, PathBuf},
};

use log::{error, info};
use rdm4lib::{
    gltf_export::{self, GltfExportFormat},
    gltf_reader::{ImportedGltf, ResolveNodeName},
    rdm::{RdmEndian, RdmScanner},
    rdm_anim::RdAnim,
    rdm_data_anim::RdAnimWriter2,
    rdm_data_main::RdWriter2,
    rdm_material::RdMaterial,
    vertex::TargetVertexFormat,
    RdModell,
};

/// Print physical framing, retaining partial results on format errors.
/// Returns true when framing is complete and no preamble issues were found;
/// record layouts and references are not validated.
pub fn inspect(input: &Path, mut output: impl Write) -> io::Result<bool> {
    let report = RdmScanner::new(BufReader::new(File::open(input)?)).scan()?;
    let order = match report.endian {
        Some(RdmEndian::Little) => "little",
        Some(RdmEndian::Big) => "big",
        None => "unknown",
    };
    writeln!(output, "File: {}", input.display())?;
    writeln!(output, "Size: {} bytes", report.source_len)?;
    writeln!(output, "Byte order: {order}")?;
    if let Some(words) = report.preamble {
        writeln!(output, "Preamble (0x04..0x14): {words:#010x?}")?;
    }
    writeln!(
        output,
        "Blocks: {} (physical framing only)",
        report.blocks.len()
    )?;
    writeln!(
        output,
        "  Header      Payload start..end        Count    Stride"
    )?;
    for block in &report.blocks {
        writeln!(
            output,
            "  {:#010x}  {:#010x}..{:#010x}  {:>8}  {:>8}",
            block.offset, block.payload.start, block.payload.end, block.count, block.stride,
        )?;
    }
    if let Some(range) = &report.unscanned {
        writeln!(output, "Unscanned: {:#x}..{:#x}", range.start, range.end)?;
    }
    writeln!(output, "Issues: {}", report.issues.len())?;
    for issue in &report.issues {
        writeln!(output, "  {issue}")?;
    }
    Ok(report.issues.is_empty() && report.unscanned.is_none())
}

/// Input and output settings common to both conversion directions.
pub struct Files {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub input_name: bool,
    pub force: bool,
}

impl Files {
    fn destination(&self, extension: &str) -> Option<PathBuf> {
        let mut output = self.output.clone();
        if let Some(ref mut path) = output {
            if self.input_name {
                assert!(
                    path.is_dir(),
                    "in_is_out_filename: output must not be a file!"
                );
                path.push(self.input.file_stem().unwrap());
                path.set_extension(extension);
            }
        }
        output
    }
}

/// Settings for RDM to glTF/GLB conversion.
pub struct GltfOptions {
    pub files: Files,
    pub skeleton: bool,
    pub animation: Option<PathBuf>,
    pub format: GltfExportFormat,
    pub textures: Option<Vec<PathBuf>>,
}

/// Settings for glTF to RDM conversion.
pub struct RdmOptions {
    pub files: Files,
    pub format: TargetVertexFormat,
    pub mesh_index: u32,
    pub skeleton: bool,
    pub animation: bool,
    pub no_transform: bool,
    pub mirror_x: bool,
    pub material_indices: Option<Vec<u32>>,
    pub joint_names: ResolveNodeName,
}

/// Convert an RDM mesh, optionally with skin and animation, to glTF/GLB.
pub fn gltf(opts: GltfOptions) {
    let output = opts.files.destination("gltf");
    let mut rdm = RdModell::from(opts.files.input.as_path());
    if opts.skeleton && opts.animation.is_none() {
        rdm.add_skin();
        info!("  Skin: {} joints", rdm.joints.as_ref().unwrap().len());
    } else if opts.skeleton && opts.animation.is_some() {
        rdm.add_skin();
        let anim = RdAnim::from(opts.animation.unwrap().as_path());
        rdm.add_anim(anim);
        info!(
            "  Skin: {} joints; animation added",
            rdm.joints.as_ref().unwrap().len()
        );
    } else {
        info!("  Skin and animation: not exported");
    }

    if let Some(textures) = opts.textures {
        rdm.mat = Some(RdMaterial::new(textures));
    }
    info!("Building glTF");
    gltf_export::build(rdm, output, !opts.files.force, opts.format);
}

/// Convert a selected glTF mesh, optionally with skin and animation, to RDM.
pub fn rdm(opts: RdmOptions) {
    let output = opts.files.destination("rdm");
    let gltf = ImportedGltf::try_import(
        opts.files.input.as_path(),
        opts.mesh_index,
        opts.joint_names,
    )
    .unwrap();
    let rdm = ImportedGltf::gltf_to_rdm(
        &gltf,
        opts.format,
        opts.skeleton,
        opts.mirror_x,
        opts.no_transform,
        opts.material_indices,
    );

    if opts.skeleton && opts.animation {
        let joints = rdm.joints.as_ref().unwrap();
        match ImportedGltf::read_animation(&gltf, joints, 6, 0.33333) {
            Some(mut animations) => {
                for animation in animations.drain(..) {
                    RdAnimWriter2::new(animation).write_anim_rdm(output.clone(), !opts.files.force);
                }
            }
            None => error!("Could not read animation. Does the glTF contain any animations ?"),
        }
    }

    RdWriter2::new(rdm).write_rdm(output, !opts.files.force);
    if opts.skeleton && !opts.no_transform {
        error!("glTF skeleton is set, but no_transform is not! Animation & Mesh might be severely deformed! Use --no_transform and apply rotation & translation in the cfg file.");
    }
}
