//! Layout and field decoding adapted from `feat/rdm-core-derive-reader`.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{punctuated::Punctuated, Data, DeriveInput, Fields, Ident, LitInt, Token, Type};

struct Field {
    name: Ident,
    ty: Type,
    args: Vec<usize>,
}

pub(super) fn expand(input: &DeriveInput) -> syn::Result<TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(
            input,
            "RdmRecord requires a struct",
        ));
    };
    let Fields::Named(named) = &data.fields else {
        return Err(syn::Error::new_spanned(
            input,
            "RdmRecord requires named fields",
        ));
    };
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "RdmRecord does not support generics",
        ));
    }
    let mut strides = Vec::<LitInt>::new();
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("rdm"))
    {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("strides") {
                return Err(meta.error("expected `strides(...)`"));
            }
            let content;
            syn::parenthesized!(content in meta.input);
            strides.extend(Punctuated::<LitInt, Token![,]>::parse_terminated(&content)?);
            Ok(())
        })?;
    }
    if strides.is_empty() {
        return Err(syn::Error::new_spanned(
            input,
            "missing `#[rdm(strides(...))]`",
        ));
    }
    for stride in &strides {
        if stride.base10_parse::<u32>()? == 0 {
            return Err(syn::Error::new_spanned(stride, "stride must be nonzero"));
        }
    }
    let mut fields = Vec::<Field>::new();
    for field in &named.named {
        let mut args = Vec::new();
        for attr in field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident("rdm"))
        {
            attr.parse_nested_meta(|meta| {
                if !meta.path.is_ident("args") {
                    return Err(meta.error("expected `args(...)`"));
                }
                meta.parse_nested_meta(|arg| {
                    let name = arg.path.require_ident()?;
                    let index = fields
                        .iter()
                        .position(|field| field.name == *name)
                        .ok_or_else(|| arg.error("args must name an earlier field"))?;
                    args.push(index);
                    Ok(())
                })
            })?;
        }
        fields.push(Field {
            name: field.ident.clone().expect("named field"),
            ty: field.ty.clone(),
            args,
        });
    }

    let name = &input.ident;
    let types = fields.iter().map(|field| &field.ty);
    let names: Vec<_> = fields.iter().map(|field| &field.name).collect();
    let locals: Vec<_> = (0..fields.len())
        .map(|i| format_ident!("__rdm_field_{i}"))
        .collect();
    let mut previous = quote!(0usize);
    let mut reads = Vec::new();
    for (i, field) in fields.iter().enumerate() {
        let offset = format_ident!("__RDM_OFFSET_{i}");
        let ty = &field.ty;
        let local = &locals[i];
        let context = format!("{name}.{}", field.name);
        let args: Vec<_> = field.args.iter().map(|&index| &locals[index]).collect();
        let args = match args.as_slice() {
            [] => quote!(()),
            [single] => quote!(&#single),
            _ => quote!((#(&#args),*)),
        };
        reads.push(quote! {
            const #offset: usize = #previous;
            let #local = <#ty as crate::rdm::RdmReadField>::read_field(
                reader, &bytes[#offset..#offset + <#ty as crate::rdm::RdmFieldLayout>::WIDTH],
                at + #offset, #context, #args,
            )?;
        });
        previous = quote!(#offset + <#ty as crate::rdm::RdmFieldLayout>::WIDTH);
    }
    Ok(quote! {
        impl crate::rdm::RdmRecordLayout for #name {
            const NAME: &'static str = stringify!(#name);
            const SIZE: usize = 0 #(+ <#types as crate::rdm::RdmFieldLayout>::WIDTH)*;
            const STRIDES: &'static [u32] = &[#(#strides),*];
        }
        #(
            const _: () = assert!(
                <#name as crate::rdm::RdmRecordLayout>::SIZE <= #strides as usize,
                "record fields exceed a declared stride"
            );
        )*
        impl crate::rdm::RdmReadRecord for #name {
            fn read<R: std::io::Read + std::io::Seek>(
                reader: &mut crate::rdm::RdmReader<R>, at: usize,
            ) -> Result<Self, crate::rdm::RdmError> {
                let bytes = reader.record_bytes::<{ <Self as crate::rdm::RdmRecordLayout>::SIZE }>(
                    at, <Self as crate::rdm::RdmRecordLayout>::NAME,
                )?;
                #(#reads)*
                Ok(Self { #(#names: #locals),* })
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn rejects_unsupported_shapes_and_layout_attributes() {
        for (input, message) in [
            (
                parse_quote!(
                    enum Bad {
                        A,
                    }
                ),
                "RdmRecord requires a struct",
            ),
            (
                parse_quote!(
                    struct Bad(u32);
                ),
                "RdmRecord requires named fields",
            ),
            (
                parse_quote!(
                    struct Bad<T> {
                        value: T,
                    }
                ),
                "RdmRecord does not support generics",
            ),
            (
                parse_quote!(
                    struct Bad {
                        value: u32,
                    }
                ),
                "missing `#[rdm(strides(...))]`",
            ),
            (
                parse_quote!(
                    #[rdm(strides(0))]
                    struct Bad {
                        value: u32,
                    }
                ),
                "stride must be nonzero",
            ),
            (
                parse_quote!(
                    #[rdm(strides(4), typo)]
                    struct Bad {
                        value: u32,
                    }
                ),
                "expected `strides(...)`",
            ),
            (
                parse_quote!(
                    #[rdm(strides(4))]
                    struct Bad {
                        #[rdm(args(later))]
                        value: u32,
                        later: u32,
                    }
                ),
                "args must name an earlier field",
            ),
            (
                parse_quote!(
                    #[rdm(strides(4))]
                    struct Bad {
                        #[rdm(typo)]
                        value: u32,
                    }
                ),
                "expected `args(...)`",
            ),
        ] {
            assert_eq!(expand(&input).unwrap_err().to_string(), message);
        }
    }
}
