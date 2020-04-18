use bytes::{Buf, Bytes};
use std::path::Path;

use std::fs::File;

use std::str;
use crate::RDModell;

#[derive(Debug)]
pub struct Frame {
    rotation: [f32;4],
    translation: [f32;3],
    time: f32,
}

#[derive(Debug)]
pub struct FrameCollection  {
    name: String,
    len: u32,
    frames: Vec<Frame>,
}

#[derive(Debug)]
pub struct RDAnim  {
    anim_vec: Vec<FrameCollection>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn read_anim_file_seq() {
        let mut f = File::open("basalt_crusher_others_idle01.rdm").unwrap();
        let mut buffer = Vec::new();
    
        // read the whole file
        std::io::Read::read_to_end(&mut f,&mut buffer);
        println!("anim buffer size: {}",buffer.len());

        let mut buffer = Bytes::from(buffer);
        let size = buffer.len();

        buffer.advance(44);

        let base_offset = buffer.get_u32_le() as usize;

        buffer.advance(base_offset - (size - buffer.remaining()));

        let model_str_ptr = buffer.get_u32_le() as usize;
        
        buffer.advance(model_str_ptr - RDModell::META_COUNT as usize - (size - buffer.remaining()));

        let model_str_len = buffer.get_u32_le() as usize;
        assert_eq!(model_str_len > 1,true);
        assert_eq!(buffer.get_u32_le(),1);

        let model_str = str::from_utf8(&buffer[..model_str_len]).unwrap();
        let model = String::from(model_str);
        
        println!("target model name: {}",model);
        buffer.advance(model_str_len);

        let joint_targets_num = buffer.get_u32_le() as usize;
        let joint_targets_tables_size = buffer.get_u32_le();
        assert_eq!(joint_targets_tables_size,24);
        println!("joint_targets: {}", joint_targets_num);
        
        let mut jtable: Vec<(usize,usize)> = Vec::with_capacity(joint_targets_num);
        for i in 0..joint_targets_num {

            jtable.push((
                buffer.get_u32_le() as usize,
                buffer.get_u32_le() as usize
                )
            );
            
            buffer.advance(16);
        }

        println!("jtable: {:?}", jtable);

        let mut anim_vec: Vec<FrameCollection> = Vec::with_capacity(joint_targets_num);

        for ent in &jtable {
            buffer.advance(ent.0 - RDModell::META_COUNT as usize - (size - buffer.remaining()));
            let ent_str_len = buffer.get_u32_le() as usize;
            assert_eq!(ent_str_len > 1,true);
            assert_eq!(buffer.get_u32_le(),1);

            let ent_model_str = str::from_utf8(&buffer[..ent_str_len]).unwrap();
            let ent_model = String::from(ent_model_str);
        
            println!("joint: {}",ent_model_str);
            buffer.advance(ent_str_len);

            let ent_child_count = buffer.get_u32_le();
            assert_eq!(buffer.get_u32_le(),32);

            let mut frame: Vec<Frame> =  Vec::new();
            

            for k in 0..ent_child_count {
                let kframe = Frame {
                    rotation: [buffer.get_f32_le(),buffer.get_f32_le(),buffer.get_f32_le(),buffer.get_f32_le()],
                    translation: [buffer.get_f32_le(),buffer.get_f32_le(),buffer.get_f32_le()],
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

        println!("anim: {:?}",anim_vec);
    }

}

