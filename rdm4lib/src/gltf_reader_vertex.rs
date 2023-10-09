use bytes::{BufMut, BytesMut};
use half::f16;

use crate::vertex::AnnoData;

pub trait PutVertex<T, const I: u32, const N: usize> {
    fn put_vertex_data(&mut self, input: &AnnoData<T, I, N>);
}

impl<const I: u32, const N: usize> PutVertex<u8, I, N> for BytesMut {
    fn put_vertex_data(&mut self, input: &AnnoData<u8, I, N>) {
        self.put_slice(&input.data);
    }
}

impl<const I: u32, const N: usize> PutVertex<f16, I, N> for BytesMut {
    fn put_vertex_data(&mut self, input: &AnnoData<f16, I, N>) {
        for e in input.data.iter() {
            self.put_u16_le(e.to_bits());
        }
    }
}
