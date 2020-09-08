use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug)]
pub struct RDMaterial {
    pub c_model_diff_tex: Vec<PathBuf>,
}

impl RDMaterial {
    pub fn new(path: &Path) -> Self {
        RDMaterial {
            c_model_diff_tex: vec![PathBuf::from(path)],
        }
    }

    pub fn run_texconv(&self, dst: &Path) {
        info!("running texconv ...");
        for p in self.c_model_diff_tex.iter() {
            let ab_path = p.canonicalize().unwrap();
            let ab_dst = dst.canonicalize().unwrap();
            let output = Command::new("texconv.exe")
                .arg(&ab_path.as_os_str())
                .arg(r"-o")
                .arg(&ab_dst.as_os_str())
                .arg(r"-ft")
                .arg(r"png")
                .output()
                .expect("failed to execute process");
            trace!("{:?}", &ab_path.to_str().unwrap()[4..]);
            trace!("{:?}", &ab_dst.to_str().unwrap()[4..]);
            trace!("{:?}", output);
        }
    }
}
