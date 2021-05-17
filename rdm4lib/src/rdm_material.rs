use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug)]
pub struct RdMaterial {
    c_model_diff_tex: Vec<PathBuf>,
}

impl RdMaterial {
    pub fn new<P: AsRef<Path> + Into<PathBuf>>(paths: Vec<P>) -> Self {
        let mut v = Vec::with_capacity(paths.len());
        for p in paths {
            v.push(p.into());
        }
        RdMaterial {
            c_model_diff_tex: v,
        }
    }

    pub fn run_dds_converter(&self, dst: &Path) {
        if cfg!(windows) {
            self.run_texconv(dst);
        } else {
            unimplemented!("DDS convert needs Windows texconv!");
        }
    }

    fn run_texconv(&self, dst: &Path) {
        warn!("running texconv ...");
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
                .expect("failed to execute texconv.exe");
            trace!("{:?}", &ab_path.to_str().unwrap()[4..]);
            trace!("{:?}", &ab_dst.to_str().unwrap()[4..]);
            trace!("{:?}", output);
        }
    }
}

impl<P: AsRef<Path>> From<P> for RdMaterial
where
    PathBuf: From<P>,
{
    fn from(path: P) -> Self {
        RdMaterial::new(vec![PathBuf::from(path)])
    }
}

impl<'a> IntoIterator for &'a RdMaterial {
    type Item = &'a PathBuf;
    type IntoIter = std::slice::Iter<'a, PathBuf>;

    fn into_iter(self) -> Self::IntoIter {
        self.c_model_diff_tex.iter()
    }
}
