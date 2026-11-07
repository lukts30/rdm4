//! Subcommand interface for `rdm4ng`.

use std::{io, path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand};
use rdm4lib::{
    gltf_export::GltfExportFormat, gltf_reader::ResolveNodeName, vertex::TargetVertexFormat,
};

use crate::{
    commands::{self, Files, GltfOptions, InspectFormat, RdmOptions},
    init_logging,
};

/// Parse an existing input file for either CLI.
pub fn existing_file(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("No such file {value}"))
    }
}

#[derive(Parser)]
#[command(
    name = "rdm4ng",
    version,
    about = "Convert, inspect and split RDM files"
)]
struct Cli {
    /// Increase logging verbosity.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Convert RDM to glTF/GLB.
    #[command(visible_alias = "rdm2gltf")]
    Gltf(GltfArgs),
    /// Convert glTF/GLB to RDM.
    #[command(visible_alias = "gltf2rdm")]
    Rdm(RdmArgs),
    /// Show physical blocks and optionally decode known records.
    Inspect(InspectArgs),
    /// Write one RDM per model record, retaining shared file records.
    Split(SplitArgs),
}

#[derive(Args)]
struct SplitArgs {
    /// Input RDM file.
    #[arg(value_parser = existing_file)]
    input: PathBuf,
    /// Output directory; created when needed.
    #[arg(short, long)]
    output: PathBuf,
    /// Replace existing split files.
    #[arg(long)]
    force: bool,
}

#[derive(Args)]
struct InspectArgs {
    /// Input RDM file. Framing issues are reported with partial results and exit status 1.
    #[arg(value_parser = existing_file)]
    input: PathBuf,
    /// Report format.
    #[arg(long, value_enum, default_value = "text")]
    format: InspectFormat,
    /// Decode known records and buffers, checking references and block ownership.
    #[arg(long)]
    records: bool,
}

#[derive(Args)]
struct FileArgs {
    /// Input file.
    #[arg(value_parser = existing_file)]
    input: PathBuf,
    /// Output file or existing directory.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Use the input basename in the output directory.
    #[arg(short = 'n', long, requires = "output")]
    input_name: bool,
    /// Overwrite existing output files.
    #[arg(long)]
    force: bool,
}

impl From<FileArgs> for Files {
    fn from(args: FileArgs) -> Self {
        Self {
            input: args.input,
            output: args.output,
            input_name: args.input_name,
            force: args.force,
        }
    }
}

#[derive(Args)]
struct GltfArgs {
    #[command(flatten)]
    files: FileArgs,
    /// Export one RDM model; defaults to all static models.
    #[arg(long)]
    mesh_index: Option<usize>,
    /// Export the available skeleton.
    #[arg(short, long)]
    skeleton: bool,
    /// External animation RDM; includes the skeleton.
    #[arg(short, long, value_parser = existing_file)]
    animation: Option<PathBuf>,
    /// Output format: glb, gltf, or gltfmin.
    #[arg(short = 'e', long, default_value = "glb")]
    format: GltfExportFormat,
    /// Diffuse DDS texture; may be repeated (requires Windows and texconv).
    #[arg(short = 't', long = "diffuse-texture", value_parser = existing_file)]
    textures: Option<Vec<PathBuf>>,
}

#[derive(Args)]
struct RdmArgs {
    #[command(flatten)]
    files: FileArgs,
    /// Target vertex format, e.g. P4h_N4b_G4b_B4b_T2h.
    #[arg(short, long)]
    format: TargetVertexFormat,
    /// glTF mesh index.
    #[arg(long, default_value = "0")]
    mesh_index: u32,
    /// Export the available skeleton.
    #[arg(short, long)]
    skeleton: bool,
    /// Export animations alongside the mesh.
    #[arg(short, long, requires = "skeleton")]
    animation: bool,
    /// Override nominal RDM samples/second; source key times are retained, without resampling.
    #[arg(long, requires = "animation", value_parser = clap::value_parser!(u32).range(1..))]
    sampling_rate: Option<u32>,
    /// Do not apply node transforms; recommended for animated meshes.
    #[arg(long, alias = "no_transform")]
    no_transform: bool,
    /// Mirror X and reverse triangle winding.
    #[arg(long, conflicts_with_all = ["skeleton", "animation"])]
    mirror_x: bool,
    /// Override material indices in primitive order.
    #[arg(long, num_args = 1..)]
    material_indices: Option<Vec<u32>>,
    /// Joint identifiers: UniqueName or UnstableIndex.
    #[arg(long, default_value = "UniqueName")]
    joint_names: ResolveNodeName,
}

/// Parse and execute the `rdm4ng` command line.
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    init_logging(cli.verbose);
    match cli.command {
        Command::Gltf(args) => {
            let result = commands::gltf(
                GltfOptions {
                    files: args.files.into(),
                    skeleton: args.skeleton || args.animation.is_some(),
                    animation: args.animation,
                    format: args.format,
                    textures: args.textures,
                },
                args.mesh_index,
            );
            if let Err(error) = result {
                eprintln!("gltf: {error}");
                return ExitCode::FAILURE;
            }
        }
        Command::Rdm(args) => {
            if let Err(error) = commands::rdm(RdmOptions {
                files: args.files.into(),
                format: args.format,
                mesh_index: args.mesh_index,
                skeleton: args.skeleton,
                animation: args.animation,
                sampling_rate: args.sampling_rate,
                no_transform: args.no_transform,
                mirror_x: args.mirror_x,
                material_indices: args.material_indices,
                joint_names: args.joint_names,
            }) {
                eprintln!("rdm: {error}");
                return ExitCode::FAILURE;
            }
        }
        Command::Inspect(args) => {
            return match commands::inspect(
                &args.input,
                args.format,
                args.records,
                io::stdout().lock(),
            ) {
                Ok(true) => ExitCode::SUCCESS,
                Ok(false) => ExitCode::FAILURE,
                Err(error) => {
                    eprintln!("inspect {}: {error}", args.input.display());
                    ExitCode::FAILURE
                }
            };
        }
        Command::Split(args) => {
            if let Err(error) =
                commands::split(&args.input, &args.output, args.force, io::stdout().lock())
            {
                eprintln!("split: {error}");
                return ExitCode::FAILURE;
            }
        }
    }
    ExitCode::SUCCESS
}
