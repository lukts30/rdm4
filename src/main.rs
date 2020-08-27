extern crate rdm4lib;

use rdm4lib::RDModell;

use rdm4lib::gltf_export;
use rdm4lib::rdm_anim::RDAnim;
use rdm4lib::rdm_writer::RDWriter;

use rdm4lib::rdm_anim_writer::RDAnimWriter;

use rdm4lib::gltf_reader;

#[macro_use]
extern crate log;

use clap::Clap;
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

#[derive(Clap)]
#[clap(
    version = "v0.1-alpha",
    author = "lukts30 <https://github.com/lukts30/rdm4>"
)]
struct Opts {
    /// Convert from glTF to .rdm
    #[clap(
        short = "g",
        long = "gltf",
        conflicts_with("rdanimation"),
        display_order(1)
    )]
    gltf: bool,

    /// Export (available) skin
    #[clap(short = "s", long = "skeleton", display_order(2))]
    skeleton: bool,

    /// Export (available) animation. RDM to glTF needs external animation file (rdanimation)
    #[clap(
        short = "a",
        long = "animation",
        display_order(3),
        requires("skeleton")
    )]
    animation: bool,

    /// External animation file for rdm
    #[clap(
        short = "m",
        long = "rdanimation",
        display_order(4),
        value_name("anim/*.rdm"),
        validator_os(cli_in_is_file),
        parse(from_str),
        conflicts_with("gltf"),
        requires_all(&["skeleton", "animation"])
    )]
    rdanimation: Option<PathBuf>,

    /// Input file
    #[clap(
        short = "f",
        long = "file",
        value_name("glTF or rdm FILE"),
        validator_os(cli_in_is_file),
        parse(from_str)
    )]
    input: PathBuf,

    /// A level of verbosity, and can be used multiple times
    #[clap(short, long, parse(from_occurrences))]
    verbose: i32,
}

fn main() {
    let opts: Opts = Opts::parse();

    match opts.verbose {
        0 => env_logger::from_env(Env::default().default_filter_or("info")).init(),
        1 => env_logger::from_env(Env::default().default_filter_or("debug")).init(),
        2 => env_logger::from_env(Env::default().default_filter_or("trace")).init(),
        _ => warn!("Don't be crazy"),
    }

    // Gets a value for config if supplied by user, or defaults to "default.conf"
    info!("Using input file: {:?}", opts.input);
    info!("Export skeleton: {:?}", opts.skeleton);
    info!("Export rdanimation: {:?}", opts.rdanimation);
    if !opts.gltf {
        let mut rdm = RDModell::from(opts.input.as_path());
        if opts.skeleton && opts.rdanimation.is_none() {
            rdm.add_skin();
            info!("Skin added !");
        } else if opts.skeleton && opts.rdanimation.is_some() {
            rdm.add_skin();
            let anim = RDAnim::from(opts.rdanimation.unwrap().as_path());
            rdm.add_anim(anim);
            info!("Skin and anim added !");
        } else {
            warn!("No skin. No anim !");
        }

        info!("running gltf_export ...");
        gltf_export::build(rdm);
    } else {
        let f_path = opts.input.as_path();
        let rdm = gltf_reader::load_gltf(f_path, opts.skeleton);

        if opts.skeleton && opts.animation {
            let jj = rdm.joints.as_ref().unwrap();

            match gltf_reader::read_animation(&f_path, &jj, 6, 0.33333) {
                Some(anim) => {
                    let exp_rdm = RDAnimWriter::from(anim);
                    exp_rdm.write_anim_rdm();
                }
                None => error!("Could not read animation. Does glTF contain any animations ?"),
            }
        }

        let exp_rdm = RDWriter::from(rdm);
        exp_rdm.write_rdm();
    }
}
