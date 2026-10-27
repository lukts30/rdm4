//! Encoding of vertex and index blocks.

use crate::byte_order::ByteOrder;
use crate::error::ErrorKind;
use crate::records::{AttributeValues, Indices, Vertices};

/// Appends a vertex block, interleaving the attribute columns.
pub(crate) fn encode_vertices(
    out: &mut Vec<u8>,
    vertices: &Vertices,
    stride: u32,
    order: ByteOrder,
) -> Result<(), ErrorKind> {
    out.extend_from_slice(&order.u32_bytes(vertices.count));
    out.extend_from_slice(&order.u32_bytes(stride));
    let count = vertices.count as usize;
    let start = out.len();
    out.resize(start + count * stride as usize, 0);
    if count == 0 {
        return Ok(());
    }

    let payload = &mut out[start..];
    let stride = stride as usize;
    let mut offset = 0;
    for values in &vertices.attributes {
        let column = Column {
            stride,
            offset,
            count,
        };
        offset += match values {
            AttributeValues::U16(values) => {
                column.encode(payload, values, |v| order.u16_bytes(*v))?
            }
            AttributeValues::U32(values) => {
                column.encode(payload, values, |v| order.u32_bytes(*v))?
            }
            AttributeValues::F16(values) => {
                column.encode(payload, values, |v| order.u16_bytes(v.0))?
            }
            AttributeValues::F32(values) => {
                column.encode(payload, values, |v| order.u32_bytes(v.to_bits()))?
            }
        };
    }
    if offset != stride {
        return Err(ErrorKind::mismatch(
            "vertex stride",
            stride,
            format_args!("{offset} (sum of attribute sizes)"),
        ));
    }
    Ok(())
}

/// Placement of one attribute inside interleaved vertices.
struct Column {
    stride: usize,
    offset: usize,
    count: usize,
}

impl Column {
    /// Writes the column and returns its width in bytes per vertex.
    fn encode<const N: usize, T>(
        &self,
        payload: &mut [u8],
        values: &[T],
        encode: impl Fn(&T) -> [u8; N],
    ) -> Result<usize, ErrorKind> {
        let lanes = values.len() / self.count;
        let width = lanes * N;
        if lanes * self.count != values.len() {
            return Err(ErrorKind::mismatch(
                "attribute value count",
                format_args!("a multiple of {} vertices", self.count),
                values.len(),
            ));
        }
        if self.offset + width > self.stride {
            return Err(ErrorKind::mismatch(
                "vertex stride",
                self.stride,
                format_args!("at least {}", self.offset + width),
            ));
        }
        if width == 0 {
            return Ok(0);
        }

        let vertices = payload.chunks_exact_mut(self.stride);
        for (vertex, values) in vertices.zip(values.chunks_exact(lanes)) {
            let column = &mut vertex[self.offset..self.offset + width];
            let (lanes, _) = column.as_chunks_mut::<N>();
            for (lane, value) in lanes.iter_mut().zip(values) {
                *lane = encode(value);
            }
        }
        Ok(width)
    }
}

/// Appends an index block.
pub(crate) fn encode_indices(
    out: &mut Vec<u8>,
    indices: &Indices,
    stride: u32,
    order: ByteOrder,
) -> Result<(), ErrorKind> {
    let (count, width) = match indices {
        Indices::U16(values) => (values.len(), 2),
        Indices::U32(values) => (values.len(), 4),
    };
    if stride != width {
        return Err(ErrorKind::mismatch("index stride", stride, width));
    }
    out.extend_from_slice(&order.u32_bytes(count as u32));
    out.extend_from_slice(&order.u32_bytes(stride));
    match indices {
        Indices::U16(values) => {
            for value in values {
                out.extend_from_slice(&order.u16_bytes(*value));
            }
        }
        Indices::U32(values) => {
            for value in values {
                out.extend_from_slice(&order.u32_bytes(*value));
            }
        }
    }
    Ok(())
}
