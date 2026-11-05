use std::{
    fs::File,
    io::{BufReader, Cursor},
    path::Path,
};

use crate::rdm::{RdmDocument, RdmReader};

/// Shared animation keyframe, retaining the conversion API name.
pub use crate::rdm::RdmFrame as Frame;

#[derive(Debug, Clone)]
pub struct FrameCollection {
    pub name: String,
    pub frames: Vec<Frame>,
}

#[derive(Debug, Clone)]
pub struct RdAnim {
    pub time_max: u32,
    /// Nominal samples per second.
    pub sampling_rate: u32,
    pub name: String,
    pub anim_vec: Vec<FrameCollection>,
}

impl RdAnim {
    pub fn new(buffer: Vec<u8>, name_anim: String) -> Self {
        RdmReader::new(Cursor::new(buffer))
            .expect("invalid RDM framing")
            .read_root::<RdmDocument>()
            .expect("invalid RDM records")
            .value
            .into_animation(name_anim)
            .expect("input has no usable animation")
    }
}

impl<P: AsRef<Path>> From<P> for RdAnim {
    fn from(f_path: P) -> Self {
        let path = f_path.as_ref();
        info!("Loaded animation {:?}", path);
        RdmReader::new(BufReader::new(
            File::open(path).expect("cannot open animation"),
        ))
        .expect("invalid RDM framing")
        .read_root::<RdmDocument>()
        .expect("invalid RDM records")
        .value
        .into_animation(path.file_stem().unwrap().to_str().unwrap().into())
        .expect("input has no usable animation")
    }
}
