use binrw::BinReaderExt;
use std::path::Path;

use crate::{
    rdm_data_anim::Frame,
    rdm_data_main::{RdmFile, RdmKindAnim},
};
use std::fs::File;

#[derive(Debug, Clone)]
pub struct FrameCollection {
    pub name: String,
    pub frames: Vec<Frame>,
}

#[derive(Debug, Clone)]
pub struct RdAnim {
    pub time_max: u32,
    pub name: String,
    pub anim_vec: Vec<FrameCollection>,
}

impl RdAnim {
    pub fn new(buffer: Vec<u8>, name_anim: String) -> Self {
        let mut reader = std::io::Cursor::new(&buffer);
        let rdmm: RdmFile<RdmKindAnim> = reader.read_le().unwrap();
        let v = &rdmm.header1.meta_anim.anims;

        let time_max = rdmm.header1.meta_anim.time_max;
        // let name_anim = rdmm.header1.meta.name.as_ascii().into();

        let mut anim_vec: Vec<FrameCollection> = Vec::with_capacity(v.len());

        for x in v.iter() {
            let ent_model = x.j_name.as_ascii().into();
            let ent = FrameCollection {
                name: ent_model,
                frames: x.j_data.storage.items.clone(),
            };
            anim_vec.push(ent);
        }

        RdAnim {
            anim_vec,
            name: name_anim,
            time_max,
        }
    }
}

impl<P: AsRef<Path>> From<P> for RdAnim {
    fn from(f_path: P) -> Self {
        let mut f = File::open(&f_path).unwrap();
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut buffer).ok();

        let buffer_len = buffer.len();
        info!("loaded {:?} into buffer", f_path.as_ref().to_str().unwrap());
        info!("buffer size: {}", buffer_len);

        RdAnim::new(
            buffer,
            String::from(f_path.as_ref().file_stem().unwrap().to_str().unwrap()),
        )
    }
}
