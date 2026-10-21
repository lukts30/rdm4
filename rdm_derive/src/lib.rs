use proc_macro::{self, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{parse_macro_input, parse_quote, Attribute, Data, DeriveInput, FieldsNamed, Type};

/*
impl RDMStructSize for RdmHeader2 {}
*/

#[proc_macro_derive(RdmStructSize, attributes(br))]
pub fn duplicate_struct_replace_ptr_u32(input: proc_macro::TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input);
    let ident = &input.ident;

    match &input.data {
        Data::Struct(_) => {
            let mut new_struct: DeriveInput = input.clone();

            let struct_name = format_ident!("Raw_{}", new_struct.ident);
            new_struct.ident = struct_name.clone();

            let attr: Attribute = parse_quote! {
                #[repr(C, packed(1))]
            };
            new_struct.attrs = vec![attr];

            if let syn::Data::Struct(s) = &mut new_struct.data {
                if let syn::Fields::Named(FieldsNamed { named, .. }) = &mut s.fields {
                    for x in named.iter_mut() {
                        if let Type::Path(type_path) = &x.ty {
                            if type_path
                                .clone()
                                .into_token_stream()
                                .to_string()
                                .contains("AnnoPtr")
                            {
                                x.ty = parse_quote! { u32 };
                                x.attrs = vec![];
                            }
                        }
                    }
                }
            };

            let output = quote!(
                impl RDMStructSizeTr for #ident {
                    fn get_struct_byte_size() -> usize {
                         #new_struct
                         std::mem::size_of::<#struct_name>()
                    }
                }
            );
            // dbg!(&output.clone().into_token_stream().to_string());
            output.into()
        }
        _ => panic!("expected struct"),
    }
}

/// Apply RDM field conventions, then let binrw generate the reader.
///
/// Vec<T> is an offset to a record array (Vec<u8> uses a byte reader).
/// Option<T> is a nullable offset. Other fields remain inline unless marked
/// #[rdm(offset)]. #[rdm(inline)] opts collections out of these defaults.
/// Explicit br(parse_with = ...) overrides the conventions for special layouts.
/// Types are recognized syntactically; aliases need an explicit parser.
#[proc_macro_attribute]
pub fn rdm_read(args: TokenStream, input: TokenStream) -> TokenStream {
    if !args.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "rdm_read takes no arguments",
        )
        .to_compile_error()
        .into();
    }
    let mut input = parse_macro_input!(input as DeriveInput);
    match expand_rdm_read(&mut input) {
        Ok(()) => quote!(#[binrw::binread] #input).into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn type_argument<'a>(ty: &'a Type, name: &str) -> Option<&'a Type> {
    let Type::Path(path) = ty else { return None };
    let segment = path.path.segments.last()?;
    if segment.ident != name {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    match args.args.first()? {
        syn::GenericArgument::Type(inner) => Some(inner),
        _ => None,
    }
}

fn expand_rdm_read(input: &mut DeriveInput) -> syn::Result<()> {
    let Data::Struct(data) = &mut input.data else {
        return Err(syn::Error::new_spanned(input, "rdm_read expects a struct"));
    };
    for field in &mut data.fields {
        let mut storage = None;
        let mut custom_parser = false;
        for attr in &field.attrs {
            if attr.path().is_ident("rdm") {
                let mode: syn::Ident = attr.parse_args()?;
                if mode != "offset" && mode != "inline" {
                    return Err(syn::Error::new_spanned(mode, "expected offset or inline"));
                }
                if storage.replace(mode).is_some() {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "duplicate RDM storage attribute",
                    ));
                }
            } else if attr.path().is_ident("br") {
                let directives = attr.parse_args_with(
                    syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                )?;
                custom_parser |= directives
                    .iter()
                    .any(|meta| meta.path().is_ident("parse_with"));
            }
        }
        field.attrs.retain(|attr| !attr.path().is_ident("rdm"));
        if custom_parser {
            if let Some(mode) = storage {
                return Err(syn::Error::new_spanned(
                    mode,
                    "use either rdm storage or an explicit parser",
                ));
            }
            continue;
        }
        if storage.as_ref().is_some_and(|mode| mode == "inline") {
            continue;
        }
        let optional = type_argument(&field.ty, "Option");
        let value = optional.unwrap_or(&field.ty);
        let element = type_argument(value, "Vec");
        let bytes =
            element.is_some_and(|ty| matches!(ty, Type::Path(path) if path.path.is_ident("u8")));
        let parser = match (optional.is_some(), element.is_some(), bytes) {
            (true, true, true) => "read_optional_bytes",
            (true, true, false) => "read_optional_array",
            (true, false, _) => "read_optional",
            (false, true, true) => "read_bytes",
            (false, true, false) => "read_array",
            (false, false, _) if storage.is_some() => "read_one",
            _ => continue,
        };
        let parser = format_ident!("{parser}");
        field
            .attrs
            .push(parse_quote!(#[br(parse_with = crate::rdm_container::#parser)]));
    }
    Ok(())
}
