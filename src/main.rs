use clap::{ArgAction, Parser};
use log::info;
use rdm4_bin::{
    cli::existing_file as cli_in_is_file,
    commands::{self, Files, GltfOptions, RdmOptions},
    init_logging,
};
use rdm4lib::{
    gltf_export::GltfExportFormat, gltf_reader::ResolveNodeName, vertex::TargetVertexFormat,
};
use std::path::PathBuf;

static HEADER_GLTF2RDM: &str = "GLTF TO RDM OPTIONS";
static HEADER_RDM2GLTF: &str = "RDM TO GLTF OPTIONS";

#[derive(Parser)]
#[command(
    version = env!("CARGO_PKG_VERSION"),
    author = "lukts30 <https://github.com/lukts30/rdm4>"
)]
struct Opts {
    // start of common options
    /// Input file
    #[arg(
        display_order(0),
        short = 'i',
        long = "input",
        value_name = "glTF or rdm FILE",
        value_parser = cli_in_is_file
    )]
    input: PathBuf,

    /// Output file or folder. If `--in-is-out-filename` is set this must be a folder!
    #[arg(display_order(1), short = 'o', long = "outdst")]
    out: Option<PathBuf>,

    /// Sets output to input file name
    #[arg(display_order(2), short = 'n', long)]
    in_is_out_filename: bool,

    /// Override existing files
    #[arg(display_order(4), long)]
    force: bool,

    /// Export (available) skin
    #[arg(display_order(5), short = 's', long = "skeleton")]
    skeleton: bool,

    /// Export (available) animation. RDM to glTF needs external animation file (rdanimation)
    #[arg(
        display_order(6),
        short = 'a',
        long = "animation",
        requires("skeleton")
    )]
    animation: bool,

    /// A level of verbosity, and can be used multiple times
    #[arg(display_order(7), short, long, action = ArgAction::Count)]
    verbose: u8,

    // end of common options
    // start of HEADER_GLTF2RDM
    /// VertexFormat for output rdm: P4h_N4b_G4b_B4b_T2h | P4h_N4b_G4b_B4b_T2h_I4b | P4h_N4b_G4b_B4b_T2h_I4b_W4b
    #[arg(
        display_order(0),
        short = 'g',
        long = "gltf",
        value_name = "VertexFormat",
        conflicts_with("rdanimation"),
        help_heading = HEADER_GLTF2RDM
    )]
    gltf: Option<TargetVertexFormat>,

    /// glTF mesh index to convert to rdm.
    #[arg(
        display_order(1),
        long,
        default_value = "0",
        help_heading = HEADER_GLTF2RDM
    )]
    gltf_mesh_index: u32,

    /// glTF to rdm: Do not apply node transforms. Recommended to use when working with animations.
    #[arg(
        display_order(2),
        long = "no_transform",
        requires("gltf"),
        help_heading = HEADER_GLTF2RDM
    )]
    no_transform: bool,

    /// Mirrors the object on the x axis.
    #[arg(display_order(3), long, conflicts_with_all = ["skeleton", "animation"], help_heading = HEADER_GLTF2RDM)]
    negative_x_and_v0v2v1: bool,

    /// Overrides MeshInstance mesh indcies. Useful to match the material order of an existing cfg.
    #[arg(display_order(4), long, help_heading = HEADER_GLTF2RDM)]
    overide_mesh_idx: Option<Vec<u32>>,

    /// For glTF joint to rdm bone: source for a unique identifier: "UnstableIndex" | "UniqueName"
    #[arg(
        display_order(5),
        long,
        short = 'u',
        default_value = "UniqueName",
        help_heading = HEADER_GLTF2RDM
    )]
    gltf_node_joint_name_src: ResolveNodeName,

    // end of HEADER_GLTF2RDM
    // start of HEADER_RDM2GLTF
    /// Export format to use for rdm to gltf: "glb", "gltf", "gltfmin"
    #[arg(
        display_order(0),
        short = 'e',
        long,
        default_value = "glb",
        help_heading = HEADER_RDM2GLTF
    )]
    gltf_export_format: GltfExportFormat,

    /// External animation file for rdm
    #[arg(
        short = 'm',
        long = "rdanimation",
        display_order(1),
        value_name = "anim/*.rdm",
        value_parser = cli_in_is_file,
        conflicts_with("gltf"),
        requires_all = ["skeleton", "animation"],
        help_heading = HEADER_RDM2GLTF
    )]
    rdanimation: Option<PathBuf>,

    /// DiffuseTextures.
    #[arg(
        short = 't',
        long = "diffusetexture",
        value_name = "*.dds",
        display_order(2),
        value_parser = cli_in_is_file,
        help_heading = HEADER_RDM2GLTF
    )]
    diffusetexture: Option<Vec<PathBuf>>,
    // end of HEADER_RDM2GLTF
}

fn main() {
    let opts = Opts::parse();
    init_logging(opts.verbose);
    entry_do_work(opts);
}

fn entry_do_work(opts: Opts) {
    info!("Material index overrides: {:?}", opts.overide_mesh_idx);
    info!("Using input file: {:?}", opts.input);
    info!("Export skeleton: {}", opts.skeleton);
    info!(
        "Export animation: {}, external animation: {:?}",
        opts.animation, opts.rdanimation
    );
    let files = Files {
        input: opts.input,
        output: opts.out,
        input_name: opts.in_is_out_filename,
        force: opts.force,
    };
    if let Some(format) = opts.gltf {
        commands::rdm(RdmOptions {
            files,
            format,
            mesh_index: Some(opts.gltf_mesh_index),
            skeleton: opts.skeleton,
            animation: opts.animation,
            sampling_rate: None,
            interpolate_animations: false,
            no_transform: opts.no_transform,
            mirror_x: opts.negative_x_and_v0v2v1,
            material_indices: opts.overide_mesh_idx,
            joint_names: opts.gltf_node_joint_name_src,
        })
        .expect("RDM conversion failed");
    } else {
        commands::gltf(
            GltfOptions {
                files,
                skeleton: opts.skeleton,
                animation: opts.rdanimation,
                format: opts.gltf_export_format,
                textures: opts.diffusetexture,
            },
            Some(0),
        )
        .unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_all_blender_addon_vertex_format_options() {
        use TargetVertexFormat::*;

        let input =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rdm4lib/rdm/gltf/stormtrooper.gltf");
        // Spellings emitted by the add-on's shell command, after tokenization.
        for (name, expected) in [
            ("P4h_N4b_G4b_B4b_T2h", P4h_N4b_G4b_B4b_T2h),
            ("P4h_N4b_G4b_B4b_T2h_I4b", P4h_N4b_G4b_B4b_T2h_I4b),
            ("P4h_N4b_G4b_B4b_T2h_I4b_W4b", P4h_N4b_G4b_B4b_T2h_I4b_W4b),
            ("P3f_N3f_G3f_B3f_T2f_C4b", P3f_N3f_G3f_B3f_T2f_C4b),
            ("P4h_N4b_G4b_B4b_T2h_C4b_C4b", P4h_N4b_G4b_B4b_T2h_C4b_C4b),
            ("P4h_T2h_C4b", P4h_T2h_C4b),
        ] {
            let options = Opts::try_parse_from([
                "rdm4-bin",
                "--input",
                input.to_str().unwrap(),
                &format!("--gltf={name}"),
                "-n",
                "--outdst",
                input.parent().unwrap().to_str().unwrap(),
            ])
            .unwrap();
            assert_eq!(options.gltf, Some(expected));
        }
    }
}
