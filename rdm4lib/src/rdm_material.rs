use std::io::Error;
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

    pub fn run_dds_converter(&self, dst: &Path) {
        if cfg!(windows) {
            self.run_texconv(dst);
        } /*
            TODO: fix CI to download and test with compressonator since wine
            does not texconv (png) and wine does not work.
          else {
              self.run_compressonator(dst);
          }
          */
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

    #[allow(dead_code)]
    fn run_compressonator(&self, dst: &Path) {
        warn!("running compressonator ...");
        for p in self.c_model_diff_tex.iter() {
            let ab_path = p.canonicalize().unwrap();

            let file_name = format!("{}.png", ab_path.file_stem().unwrap().to_str().unwrap());
            let file_path = Path::new(&file_name);
            let ab_dst = RDMaterial::canonicalize_path(dst).unwrap();

            let output = Command::new("CompressonatorCLI.exe")
                .current_dir(ab_dst)
                .arg(&ab_path.as_os_str())
                .arg(&file_path.as_os_str())
                .output()
                .expect("failed to execute CompressonatorCLI");

            assert_eq!(output.status.success(), true);
            dbg!("{:?}", output);
        }
    }

    // Strips UNC from canonicalized paths.
    // See https://github.com/rust-lang/rust/issues/42869 for why this is needed.
    #[cfg(target_os = "windows")]
    fn canonicalize_path<'p, P>(path: P) -> Result<PathBuf, Error>
    where
        P: Into<&'p Path>,
    {
        let canonical = path.into().canonicalize()?;
        if cfg!(windows) {
            use std::ffi::OsString;
            use std::os::windows::prelude::*;
            let vec_chars = canonical.as_os_str().encode_wide().collect::<Vec<u16>>();
            if vec_chars[0..4] == [92, 92, 63, 92] {
                return Ok(Path::new(&OsString::from_wide(&vec_chars[4..])).to_owned());
            }
        }
        Ok(canonical)
    }

    #[cfg(not(target_os = "windows"))]
    fn canonicalize_path<'p, P>(path: P) -> Result<PathBuf, Error>
    where
        P: Into<&'p Path>,
    {
        let canonical = path.into().canonicalize()?;
        Ok(canonical)
    }
}
