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

fn storage_mode(field: &syn::Field) -> syn::Result<Option<syn::Ident>> {
    let mut mode = None;
    for attr in field
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("rdm"))
    {
        let value: syn::Ident = attr.parse_args()?;
        if value != "offset" && value != "inline" && value != "skip" {
            return Err(syn::Error::new_spanned(
                value,
                "expected offset, inline, or skip",
            ));
        }
        if mode.replace(value).is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "duplicate RDM storage attribute",
            ));
        }
    }
    Ok(mode)
}

fn byte_sequence(ty: &Type) -> bool {
    let element = match ty {
        Type::Array(array) => Some(array.elem.as_ref()),
        _ => type_argument(ty, "Vec"),
    };
    element.is_some_and(|ty| matches!(ty, Type::Path(path) if path.path.is_ident("u8")))
}

fn expand_rdm_read(input: &mut DeriveInput) -> syn::Result<()> {
    // binrw removes temporary fields before the writer derive runs. Preserve shared
    // storage attributes for that derive on the remaining native struct fields.
    let derives_writer = input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("derive"))
        .map(|attr| {
            attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            )
        })
        .collect::<syn::Result<Vec<_>>>()?
        .iter()
        .flatten()
        .any(|path| {
            path.segments
                .last()
                .is_some_and(|part| part.ident == "RdmDescribe")
        });

    let Data::Struct(data) = &mut input.data else {
        return Err(syn::Error::new_spanned(input, "rdm_read expects a struct"));
    };
    for field in &mut data.fields {
        let mut storage = storage_mode(field)?;
        let mut custom_parser = false;
        for attr in field.attrs.iter().filter(|attr| attr.path().is_ident("br")) {
            let directives = attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )?;
            custom_parser |= directives
                .iter()
                .any(|meta| meta.path().is_ident("parse_with"));
        }
        if !derives_writer {
            field.attrs.retain(|attr| !attr.path().is_ident("rdm"));
        }
        // skip affects writing only; retain the normal reading convention.
        if storage.as_ref().is_some_and(|mode| mode == "skip") {
            storage = None;
        }
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

/// Describe a native record using the same Vec/Option/inline conventions as rdm_read.
/// Use after #[rdm_read] so binrw's temporary fields have already been removed.
/// This derive generates writing only; it neither invokes nor interprets binrw.
#[proc_macro_derive(RdmDescribe, attributes(rdm))]
pub fn derive_rdm_describe(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand_rdm_describe(&input) {
        Ok(output) => output.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn block_expression(value: proc_macro2::TokenStream, ty: &Type) -> proc_macro2::TokenStream {
    match type_argument(ty, "Vec") {
        Some(Type::Path(path)) if path.path.is_ident("u8") => quote!(plan.add_byte_block(#value)),
        Some(_) => quote!(plan.add_record_array(#value)),
        None => quote!(plan.add_record(#value)),
    }
}

fn expand_rdm_describe(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "RdmDescribe expects a struct",
        ));
    };
    let mut statements = Vec::new();
    for (index, field) in data.fields.iter().enumerate() {
        let mode = storage_mode(field)?;
        if mode.as_ref().is_some_and(|mode| mode == "skip") {
            continue;
        }
        let member = field
            .ident
            .clone()
            .map(syn::Member::Named)
            .unwrap_or_else(|| syn::Member::Unnamed(syn::Index::from(index)));
        let inline = mode.as_ref().is_some_and(|mode| mode == "inline");
        let optional = type_argument(&field.ty, "Option");
        let statement = if let Some(inner) = optional.filter(|_| !inline) {
            let block = block_expression(quote!(value), inner);
            quote!(record.append_offset(self.#member.as_ref().map(|value| #block));)
        } else if !inline && (mode.is_some() || type_argument(&field.ty, "Vec").is_some()) {
            let block = block_expression(quote!(&self.#member), &field.ty);
            quote!(record.append_offset(Some(#block));)
        } else if byte_sequence(&field.ty) {
            quote!(record.append_bytes(&self.#member);)
        } else {
            quote!(crate::rdm_writer::Inline::append_inline(&self.#member, plan, &mut record);)
        };
        statements.push(statement);
    }
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics crate::rdm_writer::Describe for #name #ty_generics #where_clause {
            fn describe<'__rdm>(&'__rdm self, plan: &mut crate::rdm_writer::Plan<'__rdm>) -> crate::rdm_writer::Record {
                let mut record = crate::rdm_writer::Record::new(plan);
                #(#statements)*
                record
            }
        }
        impl #impl_generics crate::rdm_writer::Inline for #name #ty_generics #where_clause {
            fn append_inline<'__rdm>(&'__rdm self, plan: &mut crate::rdm_writer::Plan<'__rdm>, record: &mut crate::rdm_writer::Record) {
                record.append_record(crate::rdm_writer::Describe::describe(self, plan));
            }
        }
    })
}
