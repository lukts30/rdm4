//! Binary payloads, views and accessors shared by glTF export paths.

#![forbid(unsafe_code)]

use std::io::{self, Write};

use bytes::Bytes;
use gltf::json::{self, validation::USize64};

/// Accessor metadata, separate from the binary payload and its placement.
pub(crate) struct AccessorBuilder {
    count: usize,
    shape: json::accessor::Type,
    component: json::accessor::ComponentType,
    min: Option<json::Value>,
    max: Option<json::Value>,
}

impl AccessorBuilder {
    pub fn new(
        count: usize,
        shape: json::accessor::Type,
        component: json::accessor::ComponentType,
    ) -> Self {
        Self {
            count,
            shape,
            component,
            min: None,
            max: None,
        }
    }

    pub fn bounds(mut self, min: Option<json::Value>, max: Option<json::Value>) -> Self {
        self.min = min;
        self.max = max;
        self
    }
}

#[derive(Default)]
pub(crate) struct GltfBuffers {
    pub buffers: Vec<json::Buffer>,
    pub views: Vec<json::buffer::View>,
    pub accessors: Vec<json::Accessor>,
    payloads: Vec<BufferContainer>,
}

impl GltfBuffers {
    pub fn push_buffer(&mut self, payload: BufferContainer) -> json::Index<json::Buffer> {
        let index = self.buffers.len();
        self.buffers.push(json::Buffer {
            byte_length: USize64(payload.get_bytes_len_padded() as u64),
            uri: Some(format!("buffer{index}.bin")),
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
        });
        self.payloads.push(payload);
        json::Index::new(index as u32)
    }

    pub fn push_buffer_view(
        &mut self,
        payload: BufferContainer,
        target: Option<json::validation::Checked<json::buffer::Target>>,
    ) -> json::Index<json::buffer::View> {
        let buffer = self.push_buffer(payload);
        let index = self.views.len();
        self.views.push(json::buffer::View {
            buffer,
            byte_length: self.buffers[buffer.value()].byte_length,
            target,
            byte_offset: None,
            byte_stride: None,
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
        });
        json::Index::new(index as u32)
    }

    pub fn push_accessor(&mut self, accessor: json::Accessor) -> json::Index<json::Accessor> {
        let index = self.accessors.len();
        self.accessors.push(accessor);
        json::Index::new(index as u32)
    }

    pub fn push_attribute(
        &mut self,
        payload: BufferContainer,
        accessor: AccessorBuilder,
        target: Option<json::validation::Checked<json::buffer::Target>>,
    ) -> json::Index<json::Accessor> {
        use json::validation::Checked::Valid;
        let view = self.push_buffer_view(payload, target);
        self.push_accessor(json::Accessor {
            buffer_view: Some(view),
            byte_offset: None,
            count: USize64(accessor.count as u64),
            component_type: Valid(json::accessor::GenericComponentType(accessor.component)),
            type_: Valid(accessor.shape),
            min: accessor.min,
            max: accessor.max,
            extensions: Default::default(),
            extras: Default::default(),
            name: None,
            normalized: false,
            sparse: None,
        })
    }

    /// Finalize placement without decoding or changing any payload values.
    pub fn finish(mut self, root: &mut json::Root, single_buffer: bool) -> Vec<BufferContainer> {
        if single_buffer {
            self.merge();
        }
        root.buffers = self.buffers;
        root.buffer_views = self.views;
        root.accessors = self.accessors;
        self.payloads
    }

    fn merge(&mut self) {
        let size = self.payloads.iter().map(|p| p.get_bytes_len_padded()).sum();
        let mut combined = Vec::with_capacity(size);
        let mut offsets = Vec::with_capacity(self.payloads.len());
        for payload in &self.payloads {
            offsets.push(combined.len() as u64);
            payload.to_writer(&mut combined).unwrap();
        }
        for view in &mut self.views {
            view.byte_offset = Some(USize64(
                view.byte_offset.map_or(0, |offset| offset.0) + offsets[view.buffer.value()],
            ));
            view.buffer = json::Index::new(0);
        }
        self.buffers[0].byte_length = USize64(combined.len() as u64);
        self.buffers.truncate(1);
        self.payloads = vec![BufferContainer::U8(combined)];
    }
}

pub(crate) enum BufferContainer {
    U8(Vec<u8>),
    Bytes(Bytes),
}

impl BufferContainer {
    fn padding(&self) -> usize {
        let length = self.get_bytes().len();
        assert_ne!(length, 0);
        (4 - length % 4) % 4
    }

    pub fn get_bytes_len_padded(&self) -> usize {
        self.get_bytes().len() + self.padding()
    }

    pub fn get_bytes(&self) -> &[u8] {
        match self {
            Self::U8(bytes) => bytes,
            Self::Bytes(bytes) => bytes,
        }
    }

    pub fn to_writer(&self, mut writer: impl Write) -> io::Result<()> {
        writer.write_all(self.get_bytes())?;
        writer.write_all(&[0; 3][..self.padding()])
    }
}
