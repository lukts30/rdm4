//! Shape-driven RDM writer.
//!
//! Each block is emitted as header and records, then its children in the
//! recorded physical order. Offsets are patched once a child's position is
//! known. Error offsets refer to the output.

use facet::{Facet, Shape};
use facet_reflect::Peek;

use crate::byte_order::{ByteOrder, RecordOutput};
use crate::error::{Error, ErrorKind, Path, Segment};
use crate::layout::BlockLayout;
use crate::records::{Indices, Preamble, Root, Vertices};
use crate::wire::{self, Scalar, Target, Wire};
use crate::write_payload;

pub(crate) fn write(
    root: &Root,
    byte_order: ByteOrder,
    layout: &BlockLayout,
) -> Result<Vec<u8>, Error> {
    let mut writer = Writer {
        out: Vec::new(),
        byte_order,
        path: Path::default(),
    };
    writer.write_preamble(&Preamble::expected(byte_order))?;
    writer.write_records(&[Peek::new(root)], Root::SHAPE, layout, false)?;
    Ok(writer.out)
}

/// An offset slot waiting for its target block to be placed.
struct Pending<'mem> {
    slot: usize,
    value: Peek<'mem, 'static>,
    target: Target,
    field: &'static str,
    record: Option<u32>,
}

struct Writer {
    out: Vec<u8>,
    byte_order: ByteOrder,
    path: Path,
}

impl Writer {
    fn error(&self, kind: ErrorKind) -> Error {
        self.path.error(self.out.len() as u64, kind)
    }

    fn write_preamble(&mut self, preamble: &Preamble) -> Result<(), Error> {
        self.out.resize(Preamble::SIZE as usize, 0);
        let mut output = RecordOutput::new(&mut self.out, self.byte_order);
        let mut pending = Vec::new();
        encode_value(
            Peek::new(preamble),
            Preamble::SHAPE,
            &mut output,
            "preamble",
            &mut pending,
        )
        .map_err(|kind| self.error(kind))?;
        Ok(())
    }

    fn write_records(
        &mut self,
        records: &[Peek<'_, 'static>],
        element: &'static Shape,
        layout: &BlockLayout,
        indexed: bool,
    ) -> Result<(), Error> {
        let known = wire::known_size(element);
        if layout.stride < known {
            let kind = ErrorKind::mismatch("stride", format_args!(">= {known}"), layout.stride);
            return Err(self.error(kind));
        }
        let stride = layout.stride as usize;
        self.write_header(records.len() as u32, layout.stride);
        let payload = self.out.len();
        self.out.resize(payload + records.len() * stride, 0);

        let mut pending = Vec::new();
        for (index, record) in records.iter().enumerate() {
            let start = payload + index * stride;
            let bytes = &mut self.out[start..start + known as usize];
            let mut output = RecordOutput::new(bytes, self.byte_order);
            let mut record_pending = Vec::new();
            let encoded = encode_value(*record, element, &mut output, "", &mut record_pending);
            encoded.map_err(|kind| self.error(kind))?;
            for mut child in record_pending {
                child.slot += start;
                child.record = indexed.then_some(index as u32);
                pending.push(child);
            }
            if let Some(unknown) = layout.unknown_of(index, known) {
                self.out[start + known as usize..start + stride].copy_from_slice(unknown);
            }
        }

        if pending.len() != layout.children.len() {
            let kind = ErrorKind::mismatch(
                "referenced block count",
                layout.children.len(),
                pending.len(),
            );
            return Err(self.error(kind));
        }
        for index in layout.emission_order() {
            self.write_child(&pending[index], &layout.children[index])?;
        }
        Ok(())
    }

    fn write_header(&mut self, count: u32, stride: u32) {
        let count = self.byte_order.u32_bytes(count);
        let stride = self.byte_order.u32_bytes(stride);
        self.out.extend_from_slice(&count);
        self.out.extend_from_slice(&stride);
    }

    fn write_child(&mut self, child: &Pending, layout: &BlockLayout) -> Result<(), Error> {
        let Ok(offset) = u32::try_from(self.out.len() + 8) else {
            let kind = ErrorKind::mismatch("output size", "below 4 GiB", self.out.len());
            return Err(self.error(kind));
        };
        let slot = &mut self.out[child.slot..child.slot + 4];
        slot.copy_from_slice(&self.byte_order.u32_bytes(offset));

        if let Some(record) = child.record {
            self.path.push(Segment::Index(record));
        }
        self.path.push(Segment::Field(child.field));
        self.write_target(child.value, child.target, layout)?;
        self.path.pop();
        if child.record.is_some() {
            self.path.pop();
        }
        Ok(())
    }

    fn write_target(
        &mut self,
        value: Peek<'_, 'static>,
        target: Target,
        layout: &BlockLayout,
    ) -> Result<(), Error> {
        match target {
            Target::One(element) => {
                let pointer = value.into_pointer().map_err(|e| self.reflect(e))?;
                let Some(inner) = pointer.borrow_inner() else {
                    return Err(self.error(ErrorKind::Reflect("empty pointer".into())));
                };
                self.write_records(&[inner], element, layout, false)
            }
            Target::Many(element) => {
                let list = value.into_list().map_err(|e| self.reflect(e))?;
                let records: Vec<Peek> = list.iter().collect();
                self.write_records(&records, element, layout, true)
            }
            Target::Vertices => {
                let vertices = value.get::<Vertices>().map_err(|e| self.reflect(e))?;
                let encoded = write_payload::encode_vertices(
                    &mut self.out,
                    vertices,
                    layout.stride,
                    self.byte_order,
                );
                encoded.map_err(|kind| self.error(kind))
            }
            Target::Indices => {
                let indices = value.get::<Indices>().map_err(|e| self.reflect(e))?;
                let encoded = write_payload::encode_indices(
                    &mut self.out,
                    indices,
                    layout.stride,
                    self.byte_order,
                );
                encoded.map_err(|kind| self.error(kind))
            }
        }
    }

    fn reflect(&self, error: impl std::fmt::Display) -> Error {
        self.error(ErrorKind::Reflect(error.to_string()))
    }
}

fn reflect_kind(error: impl std::fmt::Display) -> ErrorKind {
    ErrorKind::Reflect(error.to_string())
}

/// Encodes the inline part of a value; offsets become pending slots
/// relative to the start of `output`.
fn encode_value<'mem>(
    value: Peek<'mem, 'static>,
    shape: &'static Shape,
    output: &mut RecordOutput,
    field: &'static str,
    pending: &mut Vec<Pending<'mem>>,
) -> Result<(), ErrorKind> {
    match wire::classify(shape) {
        Wire::Scalar(scalar) => encode_scalar(value, scalar, output),
        Wire::Array { element, .. } => {
            let items = value.into_list_like().map_err(reflect_kind)?;
            for item in items.iter() {
                encode_value(item, element, output, field, pending)?;
            }
            Ok(())
        }
        Wire::Struct(fields) => {
            let record = value.into_struct().map_err(reflect_kind)?;
            for (index, field) in fields.iter().enumerate() {
                let item = record.field(index).map_err(reflect_kind)?;
                encode_value(item, field.shape(), output, field.name, pending)?;
            }
            Ok(())
        }
        Wire::Code(code) => {
            let discriminant = value.into_enum().map_err(reflect_kind)?.discriminant();
            match code.width {
                Scalar::U8 => output.u8(discriminant as u8),
                Scalar::U16 => output.u16(discriminant as u16),
                _ => output.u32(discriminant as u32),
            }
            Ok(())
        }
        Wire::Reference(reference) => {
            let slot = output.pos();
            output.u32(0);
            let target = if reference.nullable {
                let option = value.into_option().map_err(reflect_kind)?;
                match option.value() {
                    Some(inner) => inner,
                    None => return Ok(()),
                }
            } else {
                value
            };
            pending.push(Pending {
                slot,
                value: target,
                target: reference.target,
                field,
                record: None,
            });
            Ok(())
        }
    }
}

fn encode_scalar(value: Peek, scalar: Scalar, output: &mut RecordOutput) -> Result<(), ErrorKind> {
    match scalar {
        Scalar::U8 => output.u8(*value.get::<u8>().map_err(reflect_kind)?),
        Scalar::U16 => output.u16(*value.get::<u16>().map_err(reflect_kind)?),
        Scalar::U32 => output.u32(*value.get::<u32>().map_err(reflect_kind)?),
        Scalar::I32 => output.u32(*value.get::<i32>().map_err(reflect_kind)? as u32),
        Scalar::F32 => output.u32(value.get::<f32>().map_err(reflect_kind)?.to_bits()),
    }
    Ok(())
}
