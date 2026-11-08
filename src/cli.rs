//! Subcommand interface for `rdm4ng`.

use std::{io, path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand};
use rdm4lib::{
    gltf_export::GltfExportFormat, gltf_reader::ResolveNodeName, vertex::TargetVertexFormat,
};

use crate::commands::{self, Files, GltfOptions, InspectFormat, RdmOptions};

/// Parse an existing input file for either CLI.
pub fn existing_file(value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value);
    if path.is_file() {
        Ok(path)
    } else {
        Err(format!("No such file {value}"))
    }
}

/// Select all source models/mesh instances or one zero-based mesh index.
#[derive(Clone, Copy, Debug)]
pub enum MeshSelection {
    /// Every RDM model or mesh instance in the active glTF scene.
    All,
    /// One RDM model or glTF mesh definition.
    Index(u32),
}

impl std::str::FromStr for MeshSelection {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "all" {
            Ok(Self::All)
        } else {
            value
                .parse()
                .map(Self::Index)
                .map_err(|_| "expected 'all' or a non-negative mesh index".into())
        }
    }
}

impl MeshSelection {
    /// Return the selected index, or `None` for all models/instances.
    pub fn index(self) -> Option<u32> {
        match self {
            Self::All => None,
            Self::Index(index) => Some(index),
        }
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
    /// Suppress progress messages (warnings and errors remain).
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    quiet: bool,
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
    /// Output file or existing directory; defaults to INPUT with the output extension.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Use the input basename in the output directory (automatic for directories).
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
    /// RDM model index or all; DDS textures require one model.
    #[arg(long, default_value = "all", value_name = "N|all")]
    mesh_index: MeshSelection,
    /// Export the available skeleton.
    #[arg(short, long)]
    skeleton: bool,
    /// External animation RDM; includes the skeleton.
    #[arg(short, long, value_parser = existing_file)]
    animation: Option<PathBuf>,
    /// Output format: glb, gltf, or gltfmin; inferred from -o, otherwise glb.
    #[arg(short = 'e', long)]
    format: Option<GltfExportFormat>,
    /// Diffuse DDS texture; may be repeated (requires Windows and texconv).
    #[arg(short = 't', long = "diffuse-texture", value_parser = existing_file)]
    textures: Option<Vec<PathBuf>>,
}

#[derive(Args)]
struct RdmArgs {
    #[command(flatten)]
    files: FileArgs,
    /// Target vertex format; default means P4h_N4b_G4b_B4b_T2h.
    #[arg(short, long, default_value = "default")]
    format: TargetVertexFormat,
    /// glTF mesh definition index or all mesh instances in the active scene.
    #[arg(long, default_value = "all", value_name = "N|all")]
    mesh_index: MeshSelection,
    /// Export the available skeleton.
    #[arg(short, long)]
    skeleton: bool,
    /// Export animations alongside the mesh.
    #[arg(short, long, requires = "skeleton")]
    animation: bool,
    /// Nominal RDM samples/second; also the baking rate with --interpolate-animations.
    #[arg(long, requires = "animation", value_parser = clap::value_parser!(u32).range(1..))]
    sampling_rate: Option<u32>,
    /// Bake glTF interpolation, retaining original keys and adding a held pose at t=0.
    #[arg(long, requires = "animation")]
    interpolate_animations: bool,
    /// Do not apply node transforms; recommended for animated meshes.
    #[arg(long, alias = "no_transform")]
    no_transform: bool,
    /// Mirror X and reverse triangle winding.
    #[arg(long, conflicts_with_all = ["skeleton", "animation"])]
    mirror_x: bool,
    /// Override material slots in model and primitive order.
    #[arg(long, num_args = 1..)]
    material_indices: Option<Vec<u32>>,
    /// Joint identifiers: UniqueName or UnstableIndex.
    #[arg(long, default_value = "UniqueName")]
    joint_names: ResolveNodeName,
}

/// Parse and execute the `rdm4ng` command line.
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let level = match (cli.quiet, cli.verbose) {
        (true, _) => "warn",
        (false, 0) => "info",
        (false, 1) => "debug",
        _ => "trace",
    };
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(level))
        .format_timestamp(None)
        .format_target(false)
        .init();
    match cli.command {
        Command::Gltf(args) => {
            let format = args.format.unwrap_or_else(|| {
                match args
                    .files
                    .output
                    .as_ref()
                    .filter(|p| !p.is_dir())
                    .and_then(|p| p.extension())
                    .and_then(|e| e.to_str())
                {
                    Some(ext) if ext.eq_ignore_ascii_case("gltf") => GltfExportFormat::GltfSeparate,
                    _ => GltfExportFormat::Glb,
                }
            });
            let result = commands::gltf(
                GltfOptions {
                    files: args.files.into(),
                    legacy: false,
                    skeleton: args.skeleton || args.animation.is_some(),
                    animation: args.animation,
                    format,
                    textures: args.textures,
                },
                args.mesh_index.index().map(|index| index as usize),
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
                mesh_index: args.mesh_index.index(),
                legacy: false,
                skeleton: args.skeleton,
                animation: args.animation,
                sampling_rate: args.sampling_rate,
                interpolate_animations: args.interpolate_animations,
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
