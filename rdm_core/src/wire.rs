//! Maps Facet shapes to their RDM wire representation.

use facet::{Def, EnumRepr, EnumType, Facet, Field, ScalarType, Shape, StructKind, Type, UserType};

use crate::records::{Indices, Vertices};

/// How a value of some shape is stored on disk.
#[derive(Clone, Copy)]
pub(crate) enum Wire {
    Scalar(Scalar),
    Array { element: &'static Shape, len: usize },
    Struct(&'static [Field]),
    Code(Code),
    Reference(Reference),
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum Scalar {
    U8,
    U16,
    U32,
    I32,
    F32,
}

impl Scalar {
    pub(crate) fn size(self) -> u32 {
        match self {
            Scalar::U8 => 1,
            Scalar::U16 => 2,
            Scalar::U32 | Scalar::I32 | Scalar::F32 => 4,
        }
    }
}

/// A field-less enum stored as its integer discriminant.
#[derive(Clone, Copy)]
pub(crate) struct Code {
    pub enum_type: &'static EnumType,
    pub width: Scalar,
}

/// A 32-bit absolute offset to another block.
#[derive(Clone, Copy)]
pub(crate) struct Reference {
    pub nullable: bool,
    pub target: Target,
}

/// What the referenced block contains.
#[derive(Clone, Copy)]
pub(crate) enum Target {
    /// Exactly one record (`Box<T>`).
    One(&'static Shape),
    /// Any number of records (`Vec<T>`).
    Many(&'static Shape),
    Vertices,
    Indices,
}

/// Classifies a shape. Panics on shapes no record uses, which is a bug in
/// the record definitions rather than bad input.
pub(crate) fn classify(shape: &'static Shape) -> Wire {
    if let Some(target) = payload_target(shape) {
        return Wire::Reference(Reference {
            nullable: false,
            target,
        });
    }
    match shape.def {
        Def::Option(option) => Wire::Reference(Reference {
            nullable: true,
            target: reference_target(option.t),
        }),
        Def::Pointer(_) | Def::List(_) => Wire::Reference(Reference {
            nullable: false,
            target: reference_target(shape),
        }),
        Def::Array(array) => Wire::Array {
            element: array.t,
            len: array.n,
        },
        Def::Scalar => Wire::Scalar(scalar(shape)),
        _ => classify_user_type(shape),
    }
}

fn classify_user_type(shape: &'static Shape) -> Wire {
    match shape.ty {
        Type::User(UserType::Struct(struct_type)) => Wire::Struct(struct_type.fields),
        Type::User(UserType::Enum(ref enum_type)) => Wire::Code(code(shape, enum_type)),
        _ => panic!("unsupported record field type {shape}"),
    }
}

fn payload_target(shape: &'static Shape) -> Option<Target> {
    if shape.id == Vertices::SHAPE.id {
        Some(Target::Vertices)
    } else if shape.id == Indices::SHAPE.id {
        Some(Target::Indices)
    } else {
        None
    }
}

fn reference_target(shape: &'static Shape) -> Target {
    if let Some(target) = payload_target(shape) {
        return target;
    }
    match shape.def {
        Def::Pointer(pointer) => Target::One(
            pointer
                .pointee()
                .unwrap_or_else(|| panic!("pointer {shape} has no pointee")),
        ),
        Def::List(list) => Target::Many(list.t),
        _ => panic!("{shape} cannot be the target of an offset"),
    }
}

fn scalar(shape: &'static Shape) -> Scalar {
    match shape.scalar_type() {
        Some(ScalarType::U8) => Scalar::U8,
        Some(ScalarType::U16) => Scalar::U16,
        Some(ScalarType::U32) => Scalar::U32,
        Some(ScalarType::I32) => Scalar::I32,
        Some(ScalarType::F32) => Scalar::F32,
        _ => panic!("unsupported scalar {shape}"),
    }
}

fn code(shape: &'static Shape, enum_type: &'static EnumType) -> Code {
    let unit_only = enum_type
        .variants
        .iter()
        .all(|variant| variant.data.kind == StructKind::Unit);
    assert!(unit_only, "enum {shape} in a record must be field-less");
    let width = match enum_type.enum_repr {
        EnumRepr::U8 => Scalar::U8,
        EnumRepr::U16 => Scalar::U16,
        EnumRepr::U32 => Scalar::U32,
        _ => panic!("enum {shape} needs #[repr(u8)], #[repr(u16)] or #[repr(u32)]"),
    };
    Code { enum_type, width }
}

/// Number of known bytes a value of this shape occupies inline.
pub(crate) fn known_size(shape: &'static Shape) -> u32 {
    match classify(shape) {
        Wire::Scalar(scalar) => scalar.size(),
        Wire::Array { element, len } => known_size(element) * len as u32,
        Wire::Struct(fields) => fields.iter().map(|field| known_size(field.shape())).sum(),
        Wire::Code(code) => code.width.size(),
        Wire::Reference(_) => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::*;

    #[test]
    fn known_sizes_match_shortest_observed_strides() {
        assert_eq!(known_size(Preamble::SHAPE) as u64, Preamble::SIZE);
        assert_eq!(known_size(Root::SHAPE), 20);
        assert_eq!(known_size(Model::SHAPE), 52);
        assert_eq!(known_size(VertexAttribute::SHAPE), 16);
        assert_eq!(known_size(Frame::SHAPE), 32);
        assert_eq!(known_size(Joint::SHAPE), 36);
        assert_eq!(known_size(MeshGroup::SHAPE), 12);
    }
}
