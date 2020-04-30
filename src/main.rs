extern crate rdm4lib;

use rdm4lib::RDModell;

use rdm4lib::gltf_export;
use rdm4lib::rdm_anim::RDAnim;

#[macro_use]
extern crate log;

use env_logger::Env;
use std::env;

fn main() {
    env_logger::from_env(Env::default().default_filter_or("info")).init();
    let args: Vec<String> = env::args().collect();

    if args.len() > 2 {
        let mut rdm = RDModell::from(&args[2]);

        match args[1].as_ref() {
            "-fs" => {
                rdm.add_skin();
                warn!("Skin added !");
            }
            "-fsm" if args.len() > 3 => {
                rdm.add_skin();

                let anim = RDAnim::from(&args[3]);
                rdm.add_anim(anim);

                warn!("Skin and anim added !");
            }
            _ => {
                warn!("No skin. No anim !");
            }
        }
        info!("running gltf_export ...");
        gltf_export::build(rdm);
    } else {
        error!("Not enough arguments provided");
    }
}
