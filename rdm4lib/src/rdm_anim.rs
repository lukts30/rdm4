use bytes::{Buf, Bytes};
use std::path::Path;

use std::fs::File;

use crate::RDModell;
use std::str;

#[derive(Debug, Copy, Clone)]
pub struct Frame {
    pub rotation: [f32; 4],
    pub translation: [f32; 3],
    pub time: f32,
}

#[derive(Debug, Clone)]
pub struct FrameCollection {
    pub name: String,
    pub len: u32,
    pub frames: Vec<Frame>,
}

#[derive(Debug, Clone)]
pub struct RDAnim {
    pub time_max: u32,
    pub name: String,
    pub anim_vec: Vec<FrameCollection>,
}

impl RDAnim {
    pub fn new(buffer: Vec<u8>, name_anim: String) -> Self {
        let mut buffer = Bytes::from(buffer);
        let size = buffer.len();

        buffer.advance(44);

        let base_offset = buffer.get_u32_le() as usize;

        buffer.advance(base_offset - (size - buffer.remaining()));

        let model_str_ptr = buffer.get_u32_le() as usize;
        buffer.advance(4);
        let time_max = buffer.get_u32_le();

        buffer.advance(model_str_ptr - RDModell::META_COUNT as usize - (size - buffer.remaining()));

        let model_str_len = buffer.get_u32_le() as usize;
        assert_eq!(model_str_len > 1, true);
        assert_eq!(buffer.get_u32_le(), 1);

        let model_str = str::from_utf8(&buffer[..model_str_len]).unwrap();
        let model = String::from(model_str);

        info!("target model name: {}", model);
        buffer.advance(model_str_len);

        let joint_targets_num = buffer.get_u32_le() as usize;
        let joint_targets_tables_size = buffer.get_u32_le();
        assert_eq!(joint_targets_tables_size, 24);
        info!("joint_targets: {}", joint_targets_num);

        let mut jtable: Vec<(usize, usize)> = Vec::with_capacity(joint_targets_num);
        for _ in 0..joint_targets_num {
            jtable.push((buffer.get_u32_le() as usize, buffer.get_u32_le() as usize));

            buffer.advance(16);
        }

        info!("jtable: {:?}", jtable);

        let mut anim_vec: Vec<FrameCollection> = Vec::with_capacity(joint_targets_num);

        for ent in &jtable {
            error!("ent.0: {}",ent.0);
            error!("size: {}",size );
            error!("buffer.remaining(): {}",buffer.remaining());
            buffer.advance(ent.0 - RDModell::META_COUNT as usize - (size - buffer.remaining()));
            let ent_str_len = buffer.get_u32_le() as usize;
            assert_eq!(ent_str_len > 1, true);
            assert_eq!(buffer.get_u32_le(), 1);

            let ent_model_str = str::from_utf8(&buffer[..ent_str_len]).unwrap();
            let ent_model = String::from(ent_model_str);

            info!("joint: {}", ent_model_str);
            buffer.advance(ent_str_len);

            let ent_child_count = buffer.get_u32_le();
            assert_eq!(buffer.get_u32_le(), 32);

            let mut frame: Vec<Frame> = Vec::new();

            for _ in 0..ent_child_count {
                let kframe = Frame {
                    rotation: [
                        buffer.get_f32_le(),
                        buffer.get_f32_le(),
                        buffer.get_f32_le(),
                        buffer.get_f32_le(),
                    ],
                    translation: [
                        buffer.get_f32_le(),
                        buffer.get_f32_le(),
                        buffer.get_f32_le(),
                    ],
                    time: buffer.get_f32_le(),
                };
                frame.push(kframe);
            }

            let ent = FrameCollection {
                name: ent_model,
                len: ent_child_count,
                frames: frame,
            };

            anim_vec.push(ent);
        }

        debug!("anim: {:?}", anim_vec);

        RDAnim {
            anim_vec,
            name: name_anim,
            time_max,
        }
    }
}

impl From<&Path> for RDAnim {
    fn from(f_path: &Path) -> Self {
        let mut f = File::open(f_path).unwrap();
        let mut buffer = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut buffer).ok();

        let buffer_len = buffer.len();
        info!("loaded {:?} into buffer", f_path.to_str().unwrap());

        info!("buffer size: {}", buffer_len);

        RDAnim::new(
            buffer,
            String::from(f_path.file_stem().unwrap().to_str().unwrap()),
        )
    }
}

impl From<&str> for RDAnim {
    fn from(str_path: &str) -> Self {
        RDAnim::from(Path::new(str_path))
    }
}

impl From<&String> for RDAnim {
    fn from(string_path: &String) -> Self {
        RDAnim::from(Path::new(string_path))
    }
}
