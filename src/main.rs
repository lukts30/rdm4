extern crate rdm4lib;

use rdm4lib::gltf_reader::ResolveNodeName;
use rdm4lib::rdm_data_anim::RdAnimWriter2;
use rdm4lib::rdm_data_main::RdWriter2;
use rdm4lib::{gltf_export::GltfExportFormat, vertex::TargetVertexFormat, RdModell};

use rdm4lib::gltf_export;
use rdm4lib::rdm_anim::RdAnim;

use rdm4lib::{gltf_reader, rdm_material::RdMaterial};

#[macro_use]
extern crate log;

use clap::Parser;
use env_logger::Env;
use std::ffi::OsStr;
use std::path::PathBuf;

fn cli_in_is_file(v: &OsStr) -> Result<(), String> {
    let p = PathBuf::from(v);
    if p.is_file() {
        Ok(())
    } else {
        Err(format!("No such file {}", v.to_string_lossy()))
    }
}

static HEADER_GLTF2RDM: &str = "GLTF TO RDM OPTIONS";
static HEADER_RDM2GLTF: &str = "RDM TO GLTF OPTIONS";

#[derive(Parser)]
#[clap(
    version = env!("CARGO_PKG_VERSION"),
    author = "lukts30 <https://github.com/lukts30/rdm4>"
)]
struct Opts {
    // start of common options
    /// Input file
    #[clap(
        display_order(0),
        short = 'i',
        long = "input",
        value_name("glTF or rdm FILE"),
        validator_os(cli_in_is_file),
        parse(from_str)
    )]
    input: PathBuf,

    /// Output file or folder. If `--in-is-out-filename` is set this must be a folder!
    #[clap(display_order(1), short = 'o', long = "outdst", parse(from_str))]
    out: Option<PathBuf>,

    /// Sets output to input file name
    #[clap(display_order(2), short = 'n', long)]
    in_is_out_filename: bool,

    /// Override existing files
    #[clap(display_order(4), long)]
    force: bool,

    /// Export (available) skin
    #[clap(display_order(5), short = 's', long = "skeleton")]
    skeleton: bool,

    /// Export (available) animation. RDM to glTF needs external animation file (rdanimation)
    #[clap(
        display_order(6),
        short = 'a',
        long = "animation",
        requires("skeleton")
    )]
    animation: bool,

    /// A level of verbosity, and can be used multiple times
    #[clap(display_order(7), short, long, parse(from_occurrences))]
    verbose: i32,

    // end of common options
    // start of HEADER_GLTF2RDM
    /// VertexFormat for output rdm: P4h_N4b_G4b_B4b_T2h | P4h_N4b_G4b_B4b_T2h_I4b | P4h_N4b_G4b_B4b_T2h_I4b_W4b
    #[clap(
        display_order(0),
        short = 'g',
        long = "gltf",
        value_name("VertexFormat"),
        conflicts_with("rdanimation"),
        help_heading = HEADER_GLTF2RDM
    )]
    gltf: Option<TargetVertexFormat>,

    /// glTF mesh index to convert to rdm.
    #[clap(
        display_order(1),
        long,
        default_value = "0",
        help_heading = HEADER_GLTF2RDM
    )]
    gltf_mesh_index: u32,

    /// glTF to rdm: Do not apply node transforms. Recommended to use when working with animations.
    #[clap(
        display_order(2),
        long = "no_transform",
        requires("gltf"),
        help_heading = HEADER_GLTF2RDM
    )]
    no_transform: bool,

    /// Mirrors the object on the x axis.
    #[clap(display_order(3),long, conflicts_with_all(&["skeleton", "animation"]),help_heading = HEADER_GLTF2RDM)]
    negative_x_and_v0v2v1: bool,

    /// Overrides MeshInstance mesh indcies. Useful to match the material order of an existing cfg.
    #[clap(display_order(4),long, help_heading = HEADER_GLTF2RDM)]
    overide_mesh_idx: Option<Vec<u32>>,

    /// For glTF joint to rdm bone: source for a unique identifier: "UnstableIndex" | "UniqueName"
    #[clap(
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
    #[clap(
        display_order(0),
        short = 'e',
        long,
        default_value = "glb",
        help_heading = HEADER_RDM2GLTF
    )]
    gltf_export_format: GltfExportFormat,

    /// External animation file for rdm
    #[clap(
        short = 'm',
        long = "rdanimation",
        display_order(1),
        value_name("anim/*.rdm"),
        validator_os(cli_in_is_file),
        parse(from_str),
        conflicts_with("gltf"),
        requires_all(&["skeleton", "animation"]),
        help_heading = HEADER_RDM2GLTF
    )]
    rdanimation: Option<PathBuf>,

    /// DiffuseTextures.
    #[clap(
        short = 't',
        long = "diffusetexture",
        value_name("*.dds"),
        display_order(2),
        validator_os(cli_in_is_file),
        parse(from_str),
        help_heading = HEADER_RDM2GLTF
    )]
    diffusetexture: Option<Vec<PathBuf>>,
    // end of HEADER_RDM2GLTF
}

fn main() {
    let opts: Opts = Opts::parse();
    match opts.verbose {
        0 => env_logger::Builder::from_env(Env::default().default_filter_or("info")).init(),
        1 => env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init(),
        2 => env_logger::Builder::from_env(Env::default().default_filter_or("trace")).init(),
        _ => warn!("Don't be crazy"),
    }
    entry_do_work(opts);
}

fn entry_do_work(mut opts: Opts) {
    if let Some(ref mut out) = opts.out {
        if opts.in_is_out_filename {
            let k = opts.input.file_stem().unwrap();
            assert!(
                out.is_dir(),
                "in_is_out_filename: output must not be a file!"
            );
            out.push(k);

            if opts.gltf.is_some() {
                out.set_extension("rdm");
            } else {
                out.set_extension("gltf");
            }
        }
    }

    info!("overide_mesh_idx: {:?}", &opts.overide_mesh_idx);
    // Gets a value for config if supplied by user, or defaults to "default.conf"
    info!("Using input file: {:?}", opts.input);
    info!("Export skeleton: {:?}", opts.skeleton);
    info!("Export rdanimation: {:?}", opts.rdanimation);
    if opts.gltf.is_none() {
        convert_rdm_to_gltf(opts);
    } else {
        convert_gltf_to_rdm(opts);
    }
}

fn convert_rdm_to_gltf(opts: Opts) {
    let mut rdm = RdModell::from(opts.input.as_path());
    if opts.skeleton && opts.rdanimation.is_none() {
        rdm.add_skin();
        info!("Skin added !");
    } else if opts.skeleton && opts.rdanimation.is_some() {
        rdm.add_skin();
        let anim = RdAnim::from(opts.rdanimation.unwrap().as_path());
        rdm.add_anim(anim);
        info!("Skin and anim added !");
    } else {
        warn!("No skin. No anim !");
    }

    if let Some(diffusetexture) = opts.diffusetexture {
        rdm.mat = Some(RdMaterial::new(diffusetexture));
    }
    info!("running gltf_export ...");

    gltf_export::build(rdm, opts.out, !opts.force, opts.gltf_export_format);
}

fn convert_gltf_to_rdm(opts: Opts) {
    let f_path = opts.input.as_path();
    let i_gltf = gltf_reader::ImportedGltf::try_import(
        f_path,
        opts.gltf_mesh_index,
        opts.gltf_node_joint_name_src,
    )
    .unwrap();

    let rdm = gltf_reader::ImportedGltf::gltf_to_rdm(
        &i_gltf,
        opts.gltf.unwrap(),
        opts.skeleton,
        opts.negative_x_and_v0v2v1,
        opts.no_transform,
        opts.overide_mesh_idx,
    );

    if opts.skeleton && opts.animation {
        let jj = rdm.joints.as_ref().unwrap();

        match gltf_reader::ImportedGltf::read_animation(&i_gltf, jj, 6, 0.33333) {
            Some(mut anims) => {
                for anim in anims.drain(..) {
                    let exp_rdm = RdAnimWriter2::new(anim);
                    exp_rdm.write_anim_rdm(opts.out.clone(), !opts.force);
                }
            }
            None => error!("Could not read animation. Does the glTF contain any animations ?"),
        }
    }

    let exp_rdm = RdWriter2::new(rdm);
    exp_rdm.write_rdm(opts.out, !opts.force);
    if opts.skeleton && !opts.no_transform {
        error!("glTF skeleton is set, but no_transform is not! Animation & Mesh might be severely deformed! Use --no_transform and apply rotation & translation in the cfg file.");
    }
}
