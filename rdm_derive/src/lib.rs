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
