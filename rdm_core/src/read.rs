//! Shape-driven RDM reader.
//!
//! The reader walks the reference tree in field order, building records with
//! Facet's `Partial`. Every block reports the end of its subtree; a parent
//! then requires its children, sorted by offset, to follow its payload
//! back to back. That single check rejects gaps, trailing bytes, overlaps,
//! shared blocks and non-preorder layouts. Backward offsets are rejected
//! before recursing, which also rules out cycles.

use facet::{Facet, Field, Shape};
use facet_reflect::{Partial, ReflectError};

use crate::byte_order::{ByteOrder, RecordInput};
use crate::error::{Error, ErrorKind, Path, Segment};
use crate::layout::BlockLayout;
use crate::read_payload::{Payload, PayloadContext};
use crate::records::{Preamble, Root};
use crate::wire::{self, Code, Reference, Scalar, Target, Wire};

type Builder = Partial<'static, false>;

pub(crate) struct ReadOutput {
    pub root: Root,
    pub byte_order: ByteOrder,
    pub layout: BlockLayout,
}

pub(crate) fn read(file: &[u8]) -> Result<ReadOutput, Error> {
    let byte_order = detect_byte_order(file)?;
    let mut reader = Reader {
        file,
        byte_order,
        path: Path::default(),
        payloads: PayloadContext::default(),
    };

    let preamble = reader.read_preamble()?;
    let root_block = reader.block_at(
        preamble.root_offset,
        Preamble::ROOT_OFFSET_AT,
        Preamble::SIZE,
    )?;
    let builder = Partial::alloc_owned::<Root>().map_err(|e| reflect_error(0, e))?;
    let (builder, subtree) = reader.read_records(builder, Root::SHAPE, root_block, Count::One)?;
    if subtree.end != file.len() as u64 {
        let kind = ErrorKind::mismatch(
            "end of referenced data",
            format_args!("{:#x} (input length)", file.len()),
            format_args!("{:#x}", subtree.end),
        );
        return Err(reader.path.error(subtree.end, kind));
    }

    let root = builder
        .build()
        .map_err(|e| reflect_error(0, e))?
        .materialize::<Root>()
        .map_err(|e| reflect_error(0, e))?;
    log::debug!("read {} bytes ({byte_order:?})", file.len());
    Ok(ReadOutput {
        root,
        byte_order,
        layout: subtree.layout,
    })
}

fn detect_byte_order(file: &[u8]) -> Result<ByteOrder, Error> {
    if (file.len() as u64) < Preamble::SIZE {
        let kind = ErrorKind::Truncated {
            end: Preamble::SIZE,
            len: file.len() as u64,
        };
        return Err(Error::new(0, "preamble".into(), kind));
    }
    ByteOrder::from_magic_byte(file[3]).ok_or_else(|| {
        let kind = ErrorKind::mismatch("byte order marker", "0 or 1", file[3]);
        Error::new(3, "preamble.byte_order".into(), kind)
    })
}

fn reflect_error(offset: u64, error: impl std::fmt::Display) -> Error {
    Error::new(offset, String::new(), ErrorKind::Reflect(error.to_string()))
}

/// A validated block header; the payload lies within the input.
#[derive(Clone, Copy)]
struct Block {
    header: u64,
    payload: u64,
    count: u32,
    stride: u32,
}

impl Block {
    fn end(&self) -> u64 {
        self.payload + self.count as u64 * self.stride as u64
    }
}

/// A fully read block together with everything it references.
struct Subtree {
    header: u64,
    end: u64,
    layout: BlockLayout,
}

#[derive(Clone, Copy, PartialEq)]
enum Count {
    One,
    Many,
}

/// The known bytes of one record and their file position.
struct Record<'a> {
    input: RecordInput<'a>,
    offset: u64,
}

impl Record<'_> {
    fn position(&self) -> u64 {
        self.offset + self.input.pos() as u64
    }
}

/// Children collected while reading the records of one block.
struct Children {
    block_end: u64,
    subtrees: Vec<Subtree>,
}

struct Reader<'a> {
    file: &'a [u8],
    byte_order: ByteOrder,
    path: Path,
    payloads: PayloadContext,
}

impl<'a> Reader<'a> {
    fn reflect(&self, offset: u64) -> impl FnOnce(ReflectError) -> Error + '_ {
        move |error| {
            self.path
                .error(offset, ErrorKind::Reflect(error.to_string()))
        }
    }

    fn read_preamble(&mut self) -> Result<Preamble, Error> {
        self.path.push(Segment::Field("preamble"));
        let builder = Partial::alloc_owned::<Preamble>().map_err(|e| reflect_error(0, e))?;
        let bytes = &self.file[..Preamble::SIZE as usize];
        let mut record = Record {
            input: RecordInput::new(bytes, self.byte_order),
            offset: 0,
        };
        let mut children = Children {
            block_end: Preamble::SIZE,
            subtrees: Vec::new(),
        };
        let builder = self.read_value(builder, Preamble::SHAPE, &mut record, &mut children)?;
        let preamble = builder
            .build()
            .map_err(|e| reflect_error(0, e))?
            .materialize::<Preamble>()
            .map_err(|e| reflect_error(0, e))?;

        let expected = Preamble::expected(self.byte_order);
        if preamble != expected {
            let kind = ErrorKind::mismatch(
                "preamble",
                format_args!("{expected:?}"),
                format_args!("{preamble:?}"),
            );
            return Err(self.path.error(0, kind));
        }
        self.path.pop();
        Ok(preamble)
    }

    /// Resolves an offset found at `at` to a block starting at or after `min_header`.
    fn block_at(&self, offset: u32, at: u64, min_header: u64) -> Result<Block, Error> {
        let payload = offset as u64;
        if payload < min_header + 8 {
            let kind = ErrorKind::mismatch(
                "offset",
                format_args!("forward offset >= {:#x}", min_header + 8),
                format_args!("{payload:#x}"),
            );
            return Err(self.path.error(at, kind));
        }
        let header = payload - 8;
        let header_bytes = self.slice(header, 8)?;
        let count = self.byte_order.u32(header_bytes[0..4].try_into().unwrap());
        let stride = self.byte_order.u32(header_bytes[4..8].try_into().unwrap());
        let block = Block {
            header,
            payload,
            count,
            stride,
        };
        self.slice(payload, block.end() - payload)?;
        Ok(block)
    }

    fn slice(&self, start: u64, len: u64) -> Result<&'a [u8], Error> {
        let end = start + len;
        if end > self.file.len() as u64 {
            let kind = ErrorKind::Truncated {
                end,
                len: self.file.len() as u64,
            };
            return Err(self.path.error(start, kind));
        }
        Ok(&self.file[start as usize..end as usize])
    }

    /// Reads all records of `block` into the current frame, which is either
    /// the element itself (`Count::One`) or a list of elements.
    fn read_records(
        &mut self,
        builder: Builder,
        element: &'static Shape,
        block: Block,
        count: Count,
    ) -> Result<(Builder, Subtree), Error> {
        let known = wire::known_size(element);
        self.check_block(block, known, count)?;

        let payload = self.slice(block.payload, block.end() - block.payload)?;
        let mut children = Children {
            block_end: block.end(),
            subtrees: Vec::new(),
        };
        let mut builder = builder;
        if count == Count::Many {
            builder = builder
                .init_list_with_capacity(block.count as usize)
                .map_err(self.reflect(block.header))?;
        }

        for index in 0..block.count {
            let start = index as usize * block.stride as usize;
            let mut record = Record {
                input: RecordInput::new(&payload[start..start + known as usize], self.byte_order),
                offset: block.payload + start as u64,
            };
            if count == Count::One {
                builder = self.read_value(builder, element, &mut record, &mut children)?;
                continue;
            }
            self.path.push(Segment::Index(index));
            builder = builder
                .begin_list_item()
                .map_err(self.reflect(record.offset))?;
            builder = self.read_value(builder, element, &mut record, &mut children)?;
            builder = builder.end().map_err(self.reflect(record.offset))?;
            self.path.pop();
        }
        self.payloads.observe(&mut builder);

        let unknown = unknown_bytes(payload, block.stride, known);
        let subtree = self.chain_children(block, unknown, children.subtrees)?;
        Ok((builder, subtree))
    }

    fn check_block(&self, block: Block, known: u32, count: Count) -> Result<(), Error> {
        if count == Count::One && block.count != 1 {
            let kind = ErrorKind::mismatch("record count", 1, block.count);
            return Err(self.path.error(block.header, kind));
        }
        if block.stride < known {
            let kind = ErrorKind::mismatch("stride", format_args!(">= {known}"), block.stride);
            return Err(self.path.error(block.header + 4, kind));
        }
        Ok(())
    }

    /// Requires child subtrees to follow the block contiguously, in any order.
    fn chain_children(
        &self,
        block: Block,
        unknown: Option<Box<[u8]>>,
        subtrees: Vec<Subtree>,
    ) -> Result<Subtree, Error> {
        let mut order: Vec<u32> = (0..subtrees.len() as u32).collect();
        order.sort_by_key(|&index| subtrees[index as usize].header);

        let mut cursor = block.end();
        for &index in &order {
            let child = &subtrees[index as usize];
            if child.header != cursor {
                let what = if child.header > cursor {
                    "gap before block"
                } else {
                    "overlapping or shared block"
                };
                let kind = ErrorKind::mismatch(
                    what,
                    format_args!("block at {cursor:#x}"),
                    format_args!("block at {:#x}", child.header),
                );
                return Err(self.path.error(child.header, kind));
            }
            cursor = child.end;
        }

        let in_field_order = order
            .iter()
            .enumerate()
            .all(|(position, &index)| position as u32 == index);
        if !in_field_order {
            log::debug!("`{}` stores child blocks in order {order:?}", self.path);
        }
        let layout = BlockLayout {
            stride: block.stride,
            unknown,
            children: subtrees.into_iter().map(|child| child.layout).collect(),
            physical_order: (!in_field_order).then(|| order.into_boxed_slice()),
        };
        Ok(Subtree {
            header: block.header,
            end: cursor,
            layout,
        })
    }

    fn read_value(
        &mut self,
        builder: Builder,
        shape: &'static Shape,
        record: &mut Record,
        children: &mut Children,
    ) -> Result<Builder, Error> {
        match wire::classify(shape) {
            Wire::Scalar(scalar) => self.read_scalar(builder, scalar, record),
            Wire::Array { element, len } => {
                self.read_array(builder, element, len, record, children)
            }
            Wire::Struct(fields) => self.read_struct(builder, fields, record, children),
            Wire::Code(code) => self.read_code(builder, code, record),
            Wire::Reference(reference) => self.read_reference(builder, reference, record, children),
        }
    }

    fn read_scalar(
        &self,
        builder: Builder,
        scalar: Scalar,
        record: &mut Record,
    ) -> Result<Builder, Error> {
        let at = record.position();
        let input = &mut record.input;
        let result = match scalar {
            Scalar::U8 => builder.set(input.u8()),
            Scalar::U16 => builder.set(input.u16()),
            Scalar::U32 => builder.set(input.u32()),
            Scalar::I32 => builder.set(input.u32() as i32),
            Scalar::F32 => builder.set(f32::from_bits(input.u32())),
        };
        result.map_err(self.reflect(at))
    }

    fn read_array(
        &mut self,
        builder: Builder,
        element: &'static Shape,
        len: usize,
        record: &mut Record,
        children: &mut Children,
    ) -> Result<Builder, Error> {
        let at = record.position();
        if let (Wire::Scalar(Scalar::F32), 3 | 4) = (wire::classify(element), len) {
            return self.read_f32_array(builder, len, record);
        }
        let mut builder = builder.init_array().map_err(self.reflect(at))?;
        for index in 0..len {
            builder = builder.begin_nth_field(index).map_err(self.reflect(at))?;
            builder = self.read_value(builder, element, record, children)?;
            builder = builder.end().map_err(self.reflect(at))?;
        }
        Ok(builder)
    }

    /// Sets a whole float array at once. Per-element `Partial` steps made up
    /// most of the time spent on animation frames and joints.
    fn read_f32_array(
        &self,
        builder: Builder,
        len: usize,
        record: &mut Record,
    ) -> Result<Builder, Error> {
        let at = record.position();
        let result = match len {
            3 => builder.set(record.input.f32_array::<3>()),
            _ => builder.set(record.input.f32_array::<4>()),
        };
        result.map_err(self.reflect(at))
    }

    fn read_struct(
        &mut self,
        builder: Builder,
        fields: &'static [Field],
        record: &mut Record,
        children: &mut Children,
    ) -> Result<Builder, Error> {
        let mut builder = builder;
        for (index, field) in fields.iter().enumerate() {
            self.path.push(Segment::Field(field.name));
            let at = record.position();
            builder = builder.begin_nth_field(index).map_err(self.reflect(at))?;
            builder = self.read_value(builder, field.shape(), record, children)?;
            builder = builder.end().map_err(self.reflect(at))?;
            self.path.pop();
        }
        Ok(builder)
    }

    fn read_code(
        &self,
        builder: Builder,
        code: Code,
        record: &mut Record,
    ) -> Result<Builder, Error> {
        let at = record.position();
        let value = match code.width {
            Scalar::U8 => record.input.u8() as i64,
            Scalar::U16 => record.input.u16() as i64,
            _ => record.input.u32() as i64,
        };
        let variants = code.enum_type.variants;
        if !variants.iter().any(|v| v.discriminant == Some(value)) {
            let names: Vec<String> = variants
                .iter()
                .map(|v| format!("{}={}", v.name, v.discriminant.unwrap_or_default()))
                .collect();
            let kind = ErrorKind::mismatch(
                "enum code",
                format_args!("one of {}", names.join(", ")),
                value,
            );
            return Err(self.path.error(at, kind));
        }
        builder.select_variant(value).map_err(self.reflect(at))
    }

    fn read_reference(
        &mut self,
        builder: Builder,
        reference: Reference,
        record: &mut Record,
        children: &mut Children,
    ) -> Result<Builder, Error> {
        let at = record.position();
        let offset = record.input.u32();
        if offset == 0 {
            if reference.nullable {
                return builder.set_default().map_err(self.reflect(at));
            }
            let kind = ErrorKind::mismatch("offset", "non-null", "null");
            return Err(self.path.error(at, kind));
        }

        let block = self.block_at(offset, at, children.block_end)?;
        let mut builder = builder;
        if reference.nullable {
            builder = builder.begin_some().map_err(self.reflect(at))?;
        }
        let (mut builder, subtree) = match reference.target {
            Target::One(element) => {
                let inner = builder.begin_smart_ptr().map_err(self.reflect(at))?;
                let (inner, subtree) = self.read_records(inner, element, block, Count::One)?;
                (inner.end().map_err(self.reflect(at))?, subtree)
            }
            Target::Many(element) => self.read_records(builder, element, block, Count::Many)?,
            Target::Vertices | Target::Indices => {
                self.read_payload(builder, reference.target, block)?
            }
        };
        if reference.nullable {
            builder = builder.end().map_err(self.reflect(at))?;
        }
        children.subtrees.push(subtree);
        Ok(builder)
    }

    /// Hands a numeric block to its codec; the codec owns stride validation.
    fn read_payload(
        &mut self,
        builder: Builder,
        target: Target,
        block: Block,
    ) -> Result<(Builder, Subtree), Error> {
        let payload = Payload {
            bytes: self.slice(block.payload, block.end() - block.payload)?,
            count: block.count,
            stride: block.stride,
            byte_order: self.byte_order,
        };
        let result = match target {
            Target::Vertices => self
                .payloads
                .decode_vertices(&payload)
                .map(|vertices| builder.set(vertices)),
            _ => self
                .payloads
                .decode_indices(&payload)
                .map(|indices| builder.set(indices)),
        };
        let builder = result
            .map_err(|kind| self.path.error(block.header, kind))?
            .map_err(self.reflect(block.header))?;
        let subtree = Subtree {
            header: block.header,
            end: block.end(),
            layout: BlockLayout::leaf(block.stride),
        };
        Ok((builder, subtree))
    }
}

/// Copies the bytes after each record's known fields, unless all are zero.
fn unknown_bytes(payload: &[u8], stride: u32, known: u32) -> Option<Box<[u8]>> {
    if stride == known {
        return None;
    }
    let tails = payload
        .chunks_exact(stride as usize)
        .map(|record| &record[known as usize..]);
    if tails.clone().all(|tail| tail.iter().all(|&byte| byte == 0)) {
        return None;
    }
    Some(tails.flatten().copied().collect())
}
