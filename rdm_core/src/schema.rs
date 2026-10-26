//! Declarative record layouts.
//!
//! `record!` declares a record struct together with its wire layout and
//! generates the layout table plus the reader and writer glue. `wire_enum!`
//! declares a `u32`-coded enum.

use std::ops::Range;

/// Inline width of a block reference.
pub(crate) const REFERENCE_SIZE: usize = 4;

/// Location and inline width of one known record field.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FieldSlot {
    pub(crate) offset: usize,
    pub(crate) size: usize,
}

/// Wire layout of a record type.
pub(crate) trait RecordLayout {
    const NAME: &'static str;
    /// Record widths observed for this type.
    const STRIDES: &'static [u32];
    /// Known fields, in physical child order.
    const FIELDS: &'static [FieldSlot];
}

/// Compile-time check that fields fit every stride and do not overlap.
pub(crate) const fn check_layout(fields: &[FieldSlot], strides: &[u32]) {
    let mut i = 0;
    while i < fields.len() {
        let end = fields[i].offset + fields[i].size;
        let mut s = 0;
        while s < strides.len() {
            assert!(end <= strides[s] as usize, "field exceeds record stride");
            s += 1;
        }
        let mut j = i + 1;
        while j < fields.len() {
            let other_end = fields[j].offset + fields[j].size;
            assert!(
                end <= fields[j].offset || other_end <= fields[i].offset,
                "record fields overlap"
            );
            j += 1;
        }
        i += 1;
    }
}

/// Byte ranges of a record that no known field covers.
pub(crate) fn unknown_ranges(fields: &[FieldSlot], stride: usize) -> Vec<Range<usize>> {
    let mut slots = fields.to_vec();
    slots.sort_by_key(|slot| slot.offset);

    let mut ranges = Vec::new();
    let mut position = 0;
    for slot in slots {
        if slot.offset > position {
            ranges.push(position..slot.offset);
        }
        position = slot.offset + slot.size;
    }
    if stride > position {
        ranges.push(position..stride);
    }
    ranges
}

/// Declare a record struct and its wire layout.
///
/// ```text
/// record! {
///     pub struct Track: stride 24 {
///         0x00 name: ref Vec<u8>,          // reference to a child block
///         0x04 frames: ref Vec<Frame>,
///     }
/// }
/// ```
///
/// Each field is `offset name: type` for an inline value or
/// `offset name: ref type` for a reference to a child block. A reference
/// may pass `[args]` to its payload decoder, naming earlier fields.
///
/// Fields are declared in physical block order: children are read and
/// written in declaration order, independent of their offsets. Bytes not
/// covered by a field are preserved by the document, not the struct.
macro_rules! record {
    // Reference field.
    (@fields $head:tt [$($done:tt)*]
        $(#[$fmeta:meta])* $offset:literal $field:ident : ref $ty:ty $([$args:expr])?
        $(, $($rest:tt)*)?
    ) => {
        record!(@fields $head
            [$($done)* [reference [$(#[$fmeta])*] $offset $field $ty [$($args)?]]]
            $($($rest)*)?);
    };
    // Inline value field.
    (@fields $head:tt [$($done:tt)*]
        $(#[$fmeta:meta])* $offset:literal $field:ident : $ty:ty
        $(, $($rest:tt)*)?
    ) => {
        record!(@fields $head
            [$($done)* [value [$(#[$fmeta])*] $offset $field $ty []]]
            $($($rest)*)?);
    };
    // All fields collected: generate the struct and its impls.
    (@fields [$(#[$meta:meta])* $vis:vis $name:ident $($stride:literal)|+]
        [$([$kind:ident [$(#[$fmeta:meta])*] $offset:literal $field:ident $ty:ty [$($args:expr)?]])*]
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq)]
        $vis struct $name {
            $(
                $(#[$fmeta])*
                pub $field: $ty,
            )*
        }

        impl $crate::schema::RecordLayout for $name {
            const NAME: &'static str = stringify!($name);
            const STRIDES: &'static [u32] = &[$($stride),+];
            const FIELDS: &'static [$crate::schema::FieldSlot] = &[$(
                $crate::schema::FieldSlot {
                    offset: $offset,
                    size: record!(@size $kind $ty),
                },
            )*];
        }

        const _: () = $crate::schema::check_layout(
            <$name as $crate::schema::RecordLayout>::FIELDS,
            <$name as $crate::schema::RecordLayout>::STRIDES,
        );

        impl $crate::reader::ReadRecord for $name {
            fn read_record<'d>(
                r: &mut $crate::reader::Reader<'d>,
                rec: $crate::reader::RecordAt<'d>,
            ) -> $crate::error::Result<Self> {
                $(
                    let $field = record!(@read $kind r rec $offset $ty [$($args)?]
                        concat!(stringify!($name), ".", stringify!($field)));
                )*
                Ok($name { $($field),* })
            }
        }

        impl $crate::writer::WriteRecord for $name {
            fn write_record(
                &self,
                w: &mut $crate::writer::Writer<'_>,
                pos: usize,
            ) -> $crate::error::Result<()> {
                let $name { $($field),* } = self;
                $(
                    record!(@write $kind w pos $offset $field [$($args)?]
                        concat!(stringify!($name), ".", stringify!($field)));
                )*
                Ok(())
            }
        }
    };

    (@size value $ty:ty) => { <$ty as $crate::codec::Value>::SIZE };
    (@size reference $ty:ty) => { $crate::schema::REFERENCE_SIZE };

    (@read value $r:ident $rec:ident $offset:literal $ty:ty [] $context:expr) => {
        $r.value::<$ty>(&$rec, $offset, $context)?
    };
    (@read reference $r:ident $rec:ident $offset:literal $ty:ty [$($args:expr)?] $context:expr) => {
        $r.reference::<$ty>(&$rec, $offset, $context, record!(@args $($args)?))?
    };

    (@write value $w:ident $pos:ident $offset:literal $field:ident [] $context:expr) => {
        $w.value($pos + $offset, $field)
    };
    (@write reference $w:ident $pos:ident $offset:literal $field:ident [$($args:expr)?] $context:expr) => {
        $w.reference($pos + $offset, $field, $context, record!(@args $($args)?))?
    };

    (@args) => { () };
    (@args $args:expr) => { $args };

    // Entry point.
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident : stride $($stride:literal)|+ { $($body:tt)* }
    ) => {
        record!(@fields [$(#[$meta])* $vis $name $($stride)|+] [] $($body)*);
    };
}

/// Declare an enum stored as a `u32` code.
macro_rules! wire_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($(#[$vmeta:meta])* $variant:ident = $code:literal),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis enum $name {
            $($(#[$vmeta])* $variant),+
        }

        impl $name {
            const CODES: &'static [u32] = &[$($code),+];

            /// The stored code.
            pub const fn code(self) -> u32 {
                match self {
                    $($name::$variant => $code),+
                }
            }
        }

        impl $crate::codec::Value for $name {
            const SIZE: usize = 4;

            fn decode(
                bytes: &[u8],
                order: $crate::codec::ByteOrder,
            ) -> Result<Self, $crate::error::ErrorKind> {
                match <u32 as $crate::codec::Number>::from_bytes(bytes, order) {
                    $($code => Ok($name::$variant),)+
                    actual => Err($crate::error::ErrorKind::UnknownCode {
                        expected: $name::CODES,
                        actual,
                    }),
                }
            }

            fn encode(&self, out: &mut [u8], order: $crate::codec::ByteOrder) {
                $crate::codec::Number::to_bytes(self.code(), out, order);
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_unknown_ranges_between_unsorted_fields() {
        let fields = [
            FieldSlot { offset: 8, size: 4 },
            FieldSlot { offset: 0, size: 4 },
        ];
        assert_eq!(unknown_ranges(&fields, 16), vec![4..8, 12..16]);
        assert_eq!(unknown_ranges(&fields, 12), vec![4..8]);
    }
}
