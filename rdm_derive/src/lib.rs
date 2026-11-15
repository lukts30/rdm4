use proc_macro::{self, TokenStream};
use syn::{parse_macro_input, DeriveInput};

mod record;

/// Derive layout and reading for named records in `crate::rdm`.
/// Use `#[rdm(strides(...))]` and `#[rdm(args(earlier_field, ...))]`.
/// Add `#[rdm(write)]` to also derive writing when the fields implement `RdmWriteField`.
#[proc_macro_derive(RdmRecord, attributes(rdm))]
pub fn derive_rdm_record(input: proc_macro::TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    record::expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
