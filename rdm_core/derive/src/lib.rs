//! `#[derive(Record)]` for RDM record structs.
//!
//! Fields are stored on disk in declaration order, starting at the record's
//! first byte. Bytes between the last field and the stride are unknown and
//! preserved by the reader.
//!
//! Struct attributes:
//! - `#[rdm(strides(48, 52))]`: accepted on-disk record widths (required).
//! - `#[rdm(block_order(c, a, b))]`: child blocks of the listed fields are
//!   stored in this order instead of declaration order.
//! - `#[rdm(plain)]`: every field is a scalar, so blocks of this record are
//!   decoded with one infallible loop. Fails to compile otherwise.
//!
//! Field attribute:
//! - `#[rdm(args(a, b))]`: pass already decoded sibling fields to the field's
//!   reader and writer.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident, LitInt, LitStr, Token, Type};

struct Field {
    name: Ident,
    ty: Type,
    args: Vec<Ident>,
}

struct RecordAttributes {
    strides: Vec<LitInt>,
    block_order: Vec<Ident>,
    plain: bool,
}

#[proc_macro_derive(Record, attributes(rdm))]
pub fn derive_record(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let fields = parse_fields(input)?;
    let attributes = parse_record_attributes(input)?;
    let block_order = resolve_block_order(&fields, &attributes.block_order)?;

    let layout = expand_layout(name, &fields, &attributes.strides);
    let read = expand_read(name, &fields, &block_order, attributes.plain);
    let write = expand_write(name, &fields, &block_order);
    Ok(quote! {
        #layout
        #read
        #write
    })
}

fn parse_fields(input: &DeriveInput) -> syn::Result<Vec<Field>> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(input, "Record requires a struct"));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new_spanned(
            input,
            "Record requires named fields",
        ));
    };
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "Record does not support generics",
        ));
    }

    let mut fields = Vec::new();
    for field in &named.named {
        let mut args = Vec::new();
        for attribute in field.attrs.iter().filter(|a| a.path().is_ident("rdm")) {
            attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("args") {
                    meta.parse_nested_meta(|arg| {
                        args.push(arg.path.require_ident()?.clone());
                        Ok(())
                    })
                } else {
                    Err(meta.error("expected `args(...)`"))
                }
            })?;
        }
        fields.push(Field {
            name: field.ident.clone().expect("named field"),
            ty: field.ty.clone(),
            args,
        });
    }
    Ok(fields)
}

fn parse_record_attributes(input: &DeriveInput) -> syn::Result<RecordAttributes> {
    let mut attributes = RecordAttributes {
        strides: Vec::new(),
        block_order: Vec::new(),
        plain: false,
    };
    for attribute in input.attrs.iter().filter(|a| a.path().is_ident("rdm")) {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("strides") {
                let content;
                syn::parenthesized!(content in meta.input);
                let strides = Punctuated::<LitInt, Token![,]>::parse_terminated(&content)?;
                attributes.strides.extend(strides);
                Ok(())
            } else if meta.path.is_ident("block_order") {
                meta.parse_nested_meta(|field| {
                    attributes
                        .block_order
                        .push(field.path.require_ident()?.clone());
                    Ok(())
                })
            } else if meta.path.is_ident("plain") {
                attributes.plain = true;
                Ok(())
            } else {
                Err(meta.error("expected `strides(...)`, `block_order(...)` or `plain`"))
            }
        })?;
    }
    if attributes.strides.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "missing `#[rdm(strides(...))]`",
        ));
    }
    Ok(attributes)
}

/// Returns field indices in physical child-block order.
///
/// The fields named in `block_order` keep the set of positions they occupy
/// in declaration order but are visited in the listed order.
fn resolve_block_order(fields: &[Field], block_order: &[Ident]) -> syn::Result<Vec<usize>> {
    let mut listed = Vec::new();
    for name in block_order {
        let Some(index) = fields.iter().position(|f| &f.name == name) else {
            return Err(syn::Error::new_spanned(
                name,
                "unknown field in `block_order`",
            ));
        };
        if listed.contains(&index) {
            return Err(syn::Error::new_spanned(
                name,
                "field listed twice in `block_order`",
            ));
        }
        listed.push(index);
    }

    let mut slots = listed.clone();
    slots.sort_unstable();

    let mut order: Vec<usize> = (0..fields.len()).collect();
    for (slot, index) in slots.into_iter().zip(listed) {
        order[slot] = index;
    }
    Ok(order)
}

fn offset_ident(field: &Field) -> Ident {
    format_ident!("{}_OFFSET", field.name.to_string().to_uppercase())
}

/// Local constants for each field's byte offset within the record.
fn expand_offsets(fields: &[Field]) -> TokenStream {
    let mut previous = quote!(0usize);
    let mut constants = Vec::new();
    for field in fields {
        let ident = offset_ident(field);
        let ty = &field.ty;
        constants.push(quote! {
            const #ident: usize = #previous;
        });
        previous = quote!(#ident + <#ty as crate::wire::FieldLayout>::WIDTH);
    }
    quote!(#(#constants)*)
}

fn expand_layout(name: &Ident, fields: &[Field], strides: &[LitInt]) -> TokenStream {
    let types = fields.iter().map(|f| &f.ty);
    let name_text = LitStr::new(&name.to_string(), name.span());
    quote! {
        impl crate::wire::RecordLayout for #name {
            const NAME: &'static str = #name_text;
            const SIZE: usize = 0 #(+ <#types as crate::wire::FieldLayout>::WIDTH)*;
            const STRIDES: &'static [u32] = &[#(#strides),*];
        }

        impl crate::wire::FieldLayout for #name {
            const WIDTH: usize = crate::wire::REFERENCE_WIDTH;
        }

        #(
            const _: () = assert!(
                <#name as crate::wire::RecordLayout>::SIZE <= #strides,
                "record fields exceed a declared stride"
            );
        )*
    }
}

fn context(name: &Ident, field: &Field) -> LitStr {
    LitStr::new(&format!("{}.{}", name, field.name), field.name.span())
}

fn expand_read(name: &Ident, fields: &[Field], block_order: &[usize], plain: bool) -> TokenStream {
    let offsets = expand_offsets(fields);
    let read_block = if plain {
        expand_read_plain_block(name, fields)
    } else {
        quote!()
    };
    let names: Vec<&Ident> = fields.iter().map(|f| &f.name).collect();
    let reads = block_order.iter().map(|&index| {
        let field = &fields[index];
        let field_name = &field.name;
        let ty = &field.ty;
        let offset = offset_ident(field);
        let context = context(name, field);
        let args = match field.args.as_slice() {
            [] => quote!(()),
            [single] => quote!(&#single),
            many => quote!((#(&#many),*)),
        };
        quote! {
            let #field_name = <#ty as crate::read::ReadField>::read_field(
                reader,
                &bytes[#offset..#offset + <#ty as crate::wire::FieldLayout>::WIDTH],
                at + #offset,
                #context,
                #args,
            )?;
        }
    });

    quote! {
        impl crate::read::ReadRecord for #name {
            fn read<E: crate::wire::ByteOrder>(
                reader: &mut crate::read::Reader<'_, E>,
                at: usize,
            ) -> Result<Self, crate::Error> {
                #offsets
                let bytes: &[u8; <#name as crate::wire::RecordLayout>::SIZE] = reader.record_bytes(at);
                #(#reads)*
                Ok(Self { #(#names),* })
            }

            #read_block
        }

        impl crate::read::ReadField for #name {
            type Args<'a> = ();

            fn read_field<E: crate::wire::ByteOrder>(
                reader: &mut crate::read::Reader<'_, E>,
                raw: &[u8],
                at: usize,
                context: &'static str,
                _args: (),
            ) -> Result<Self, crate::Error> {
                reader.read_single(raw, at, context)
            }
        }
    }
}

/// Decodes every record of a block without per-record error handling.
fn expand_read_plain_block(name: &Ident, fields: &[Field]) -> TokenStream {
    let offsets = expand_offsets(fields);
    let decodes = fields.iter().map(|field| {
        let field_name = &field.name;
        let ty = &field.ty;
        let offset = offset_ident(field);
        quote! {
            #field_name: <#ty as crate::wire::Scalar>::decode::<E>(
                &bytes[#offset..#offset + <#ty as crate::wire::FieldLayout>::WIDTH],
            )
        }
    });

    quote! {
        fn read_block<E: crate::wire::ByteOrder>(
            reader: &mut crate::read::Reader<'_, E>,
            block: &crate::read::Block,
        ) -> Result<Vec<Self>, crate::Error> {
            #offsets
            const SIZE: usize = <#name as crate::wire::RecordLayout>::SIZE;
            let records = reader.block_bytes(block).chunks_exact(block.stride);
            let decoded = records.map(|record| {
                let bytes: &[u8; SIZE] = record[..SIZE].try_into().unwrap();
                Self { #(#decodes),* }
            });
            Ok(decoded.collect())
        }
    }
}

fn expand_write(name: &Ident, fields: &[Field], block_order: &[usize]) -> TokenStream {
    let offsets = expand_offsets(fields);
    let names: Vec<&Ident> = fields.iter().map(|f| &f.name).collect();
    let writes = block_order.iter().map(|&index| {
        let field = &fields[index];
        let field_name = &field.name;
        let ty = &field.ty;
        let offset = offset_ident(field);
        let context = context(name, field);
        let args = match field.args.as_slice() {
            [] => quote!(()),
            [single] => quote!(#single),
            many => quote!((#(#many),*)),
        };
        quote! {
            <#ty as crate::write::WriteField>::write_field(
                #field_name,
                writer,
                at + #offset,
                #context,
                #args,
            )?;
        }
    });

    quote! {
        impl crate::write::WriteRecord for #name {
            fn write<E: crate::wire::ByteOrder>(
                &self,
                writer: &mut crate::write::Writer<'_, E>,
                at: usize,
            ) -> Result<(), crate::Error> {
                #offsets
                let Self { #(#names),* } = self;
                #(#writes)*
                Ok(())
            }
        }

        impl crate::write::WriteField for #name {
            type Args<'a> = ();

            fn write_field<E: crate::wire::ByteOrder>(
                &self,
                writer: &mut crate::write::Writer<'_, E>,
                at: usize,
                context: &'static str,
                _args: (),
            ) -> Result<(), crate::Error> {
                writer.write_single(self, at, context)
            }
        }
    }
}
