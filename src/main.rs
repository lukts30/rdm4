extern crate rdm4lib;

use rdm4lib::{vertex::TargetVertexFormat, RDModell};

use rdm4lib::gltf_export;
use rdm4lib::rdm_anim::RDAnim;
use rdm4lib::rdm_writer::RDWriter;

use rdm4lib::rdm_anim_writer::RDAnimWriter;

use rdm4lib::{gltf_reader, rdm_material::RDMaterial};

#[macro_use]
extern crate log;

use clap::Clap;
use env_logger::Env;
use std::{ffi::OsStr, panic};
use walkdir::WalkDir;

use std::path::PathBuf;

fn cli_in_is_file(v: &OsStr) -> Result<(), String> {
    let p = PathBuf::from(v);
    if p.is_file() {
        Ok(())
    } else {
        Err(format!("No such file {}", v.to_string_lossy()))
    }
}

fn cli_in_is_file_or_dir(v: &OsStr) -> Result<(), String> {
    let p = PathBuf::from(v);
    if p.is_file() || p.is_dir() {
        Ok(())
    } else {
        Err(format!("No such file or directory {}", v.to_string_lossy()))
    }
}

#[derive(Clap)]
#[clap(
    version = "v0.4-alpha",
    author = "lukts30 <https://github.com/lukts30/rdm4>"
)]
struct Opts {
    /// Convert from glTF to .rdm
    /// Possible VertexFormat values are: P4h_N4b_G4b_B4b_T2h | P4h_N4b_G4b_B4b_T2h_I4b | P4h_N4b_G4b_B4b_T2h_I4b_W4b
    #[clap(
        short = 'g',
        long = "gltf",
        conflicts_with("rdanimation"),
        display_order(2)
    )]
    gltf: Option<TargetVertexFormat>,

    /// Export (available) skin
    #[clap(short = 's', long = "skeleton", display_order(5))]
    skeleton: bool,

    /// Export (available) animation. RDM to glTF needs external animation file (rdanimation)
    #[clap(
        short = 'a',
        long = "animation",
        display_order(6),
        requires("skeleton")
    )]
    animation: bool,

    /// External animation file for rdm
    #[clap(
        short = 'm',
        long = "rdanimation",
        display_order(7),
        value_name("anim/*.rdm"),
        validator_os(cli_in_is_file),
        parse(from_str),
        conflicts_with("gltf"),
        requires_all(&["skeleton", "animation"])
    )]
    rdanimation: Option<PathBuf>,

    /// Input file or folder (see --batch)
    #[clap(
        short = 'i',
        long = "input",
        display_order(0),
        value_name("glTF or rdm FILE(s)"),
        validator_os(cli_in_is_file_or_dir),
        parse(from_str)
    )]
    input: PathBuf,

    /// Output file or folder. If 'in_is_out_filename' is set this must be a folder!
    #[clap(
        short = 'o',
        long = "outdst",
        display_order(1),
        parse(from_str),
        validator_os(cli_in_is_file_or_dir)
    )]
    out: Option<PathBuf>,

    /// Sets output to input file name
    #[clap(long = "in_is_out_filename", display_order(2), short = 'n')]
    in_is_out_filename: bool,

    /// Batch process recursively
    #[clap(short = 'b',long = "batch", display_order(3),conflicts_with_all(&["diffusetexture", "rdanimation"]))]
    batch: bool,

    /// glTF to rdm: Do not apply node transforms.
    #[clap(long = "no_transform", display_order(3), requires("gltf"))]
    no_transform: bool,

    /// DiffuseTextures.
    #[clap(
        short = 't',
        long = "diffusetexture",
        value_name("*.dds"),
        display_order(5),
        validator_os(cli_in_is_file),
        parse(from_str)
    )]
    diffusetexture: Option<Vec<PathBuf>>,

    /// Mirrors the object on the x axis.
    #[clap(long, display_order(4),conflicts_with_all(&["skeleton", "animation"]))]
    negative_x_and_v0v2v1: bool,

    /// A level of verbosity, and can be used multiple times
    #[clap(short, long, parse(from_occurrences))]
    verbose: i32,
}

fn main() {
    let opts: Opts = Opts::parse();
    match opts.verbose {
        0 => env_logger::Builder::from_env(Env::default().default_filter_or("info")).init(),
        1 => env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init(),
        2 => env_logger::Builder::from_env(Env::default().default_filter_or("trace")).init(),
        _ => warn!("Don't be crazy"),
    }
    if opts.batch {
        batch(opts).unwrap();
    } else {
        entry_do_work(opts);
    }
}

fn entry_do_work(mut opts: Opts) {
    assert_eq!(
        opts.input.is_file(),
        true,
        "Input must be a file! Missing --batch / -b ?"
    );
    if let Some(ref mut out) = opts.out {
        if opts.in_is_out_filename {
            let k = opts.input.file_stem().unwrap();
            assert_eq!(
                out.is_dir(),
                true,
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

    // Gets a value for config if supplied by user, or defaults to "default.conf"
    info!("Using input file: {:?}", opts.input);
    info!("Export skeleton: {:?}", opts.skeleton);
    info!("Export rdanimation: {:?}", opts.rdanimation);
    if opts.gltf.is_none() {
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

        if let Some(diffusetexture) = opts.diffusetexture {
            rdm.mat = Some(RDMaterial {
                c_model_diff_tex: diffusetexture,
            });
        }
        info!("running gltf_export ...");
        gltf_export::build(rdm, opts.out);
    } else {
        let f_path = opts.input.as_path();
        let rdm = gltf_reader::load_gltf(
            f_path,
            opts.gltf.unwrap(),
            opts.skeleton,
            opts.negative_x_and_v0v2v1,
            opts.no_transform,
        );

        if opts.skeleton && opts.animation {
            let jj = rdm.joints.as_ref().unwrap();

            match gltf_reader::read_animation(&f_path, &jj, 6, 0.33333) {
                Some(anim) => {
                    let exp_rdm = RDAnimWriter::from(anim);
                    exp_rdm.write_anim_rdm(opts.out.clone());
                }
                None => error!("Could not read animation. Does glTF contain any animations ?"),
            }
        }

        let exp_rdm = RDWriter::from(rdm);
        exp_rdm.write_rdm(opts.out);
    }
}

#[ignore]
#[test]
fn test_batch() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let input = PathBuf::from(r"C:\Users\lukas\Desktop\anno7\data");
    let dst = PathBuf::from(r"C:\Users\lukas\Desktop\anno7\dst\final");
    let opt = Opts {
        batch: true,
        gltf: None,
        animation: false,
        diffusetexture: None,
        in_is_out_filename: true,
        input,
        negative_x_and_v0v2v1: false,
        no_transform: false,
        rdanimation: None,
        skeleton: false,
        verbose: 0,
        out: Some(dst),
    };
    batch(opt).expect("msg");
}

fn batch(defopt: Opts) -> std::result::Result<(), Box<dyn std::error::Error + 'static>> {
    let input = defopt.input;
    assert_eq!(input.is_dir(), true, "Batch: input must be a folder!");

    let mut dst = defopt.out.unwrap();
    let dst_clone = dst.clone();
    assert_eq!(dst.is_dir(), true, "Batch: dst must be a folder!");

    let rext = if defopt.gltf.is_some() {
        OsStr::new("gltf")
    } else {
        OsStr::new("rdm")
    };
    let rext2 = if defopt.gltf.is_some() {
        OsStr::new("glb")
    } else {
        OsStr::new("rdm")
    };

    let anim = OsStr::new("anim");
    let anims = OsStr::new("anims");
    for entry in WalkDir::new(&input).min_depth(1) {
        let rel_enty = entry?;
        let path = rel_enty.path();

        match path.extension() {
            Some(ext) => {
                if ext.eq(rext) || ext.eq(rext2) {
                    let parent = path.parent().unwrap();

                    if (!parent.ends_with(anim) && !parent.ends_with(anims))
                        || defopt.gltf.is_some()
                    {
                        let base = input.parent().unwrap();
                        dst.push(path.strip_prefix(base).unwrap());

                        if defopt.gltf.is_some() && (defopt.in_is_out_filename || !ext.eq("glb")) {
                            dbg!(&dst);
                            dst.pop();
                            dbg!(&dst);
                            if defopt.in_is_out_filename && !ext.eq(rext2) {
                                dst.pop();
                            }
                            dbg!(&dst);
                        }
                        std::fs::create_dir_all(&dst)?;
                        let input_final = PathBuf::from(path);
                        let opt = Opts {
                            batch: false,
                            gltf: defopt.gltf.clone(),
                            animation: false,
                            diffusetexture: None,
                            in_is_out_filename: defopt.in_is_out_filename,
                            input: input_final.clone(),
                            negative_x_and_v0v2v1: defopt.negative_x_and_v0v2v1,
                            no_transform: defopt.no_transform,
                            rdanimation: None,
                            skeleton: defopt.skeleton,
                            verbose: defopt.verbose,
                            out: Some(dst),
                        };
                        let result = panic::catch_unwind(|| {
                            entry_do_work(opt);
                        });
                        match result {
                            Ok(_) => {}
                            Err(_) => {
                                error!("Thread panicked !");
                                error!("Could not convert: {}", &input_final.display());
                            }
                        }
                        dst = dst_clone.clone();
                    }
                }
            }
            None => continue,
        }
    }

    Ok(())
}
