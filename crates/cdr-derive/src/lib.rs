// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-cdr-derive`. Safety classification: **STANDARD**.
//!
//! `#[derive(DdsType)]` proc-macro — implements
//! `zerodds-xcdr2-rust-1.0` §11.1.
//!
//! Derives an `impl DdsType` from a plain `struct` that serializes via
//! the `zerodds_cdr::CdrEncode`/`CdrDecode` traits. Currently supports
//! final extensibility (no DHEADER) — appendable and mutable are
//! reserved for the `idl-rust` codegen because their per-field logic is
//! not trivially derive-only.
//!
//! Example:
//!
//! ```ignore
//! use zerodds_cdr_derive::DdsType;
//!
//! #[derive(DdsType, Debug, Clone, PartialEq)]
//! pub struct Sensor {
//!     #[dds(key)]
//!     pub id: i32,
//!     pub value: f64,
//! }
//! ```

#![allow(clippy::expect_used)]

extern crate proc_macro;

use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{Attribute, Data, DeriveInput, Fields, Lit, Meta, parse_macro_input};

/// Derives `DdsType` for a plain `struct`.
#[proc_macro_derive(DdsType, attributes(dds))]
pub fn derive_dds_type(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    expand(&ast).unwrap_or_else(|e| e.to_compile_error()).into()
}

fn expand(ast: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &ast.ident;
    let name_str = name.to_string();

    let Data::Struct(data) = &ast.data else {
        return Err(syn::Error::new_spanned(
            ast,
            "DdsType derive supports plain structs only",
        ));
    };

    let fields = match &data.fields {
        Fields::Named(named) => named.named.iter().collect::<Vec<_>>(),
        _ => {
            return Err(syn::Error::new_spanned(
                &data.fields,
                "DdsType derive supports named fields only",
            ));
        }
    };

    let opts = parse_struct_opts(&ast.attrs)?;
    let type_name = opts.type_name.unwrap_or(name_str);

    // Walk fields to find @key markers.
    let mut keyed_fields = Vec::new();
    let mut all_fields = Vec::new();
    for f in &fields {
        let opt = parse_field_opts(&f.attrs)?;
        let ident = f.ident.as_ref().expect("named field has ident");
        all_fields.push(ident.clone());
        if opt.key {
            keyed_fields.push(ident.clone());
        }
    }
    let is_keyed = !keyed_fields.is_empty();

    let encode_field_lines = all_fields.iter().map(|ident| {
        quote! {
            ::zerodds_cdr::CdrEncode::encode(&self.#ident, &mut writer)?;
        }
    });
    let decode_field_lines = all_fields.iter().map(|ident| {
        quote! {
            #ident: ::zerodds_cdr::CdrDecode::decode(&mut reader)?,
        }
    });
    // PlainCdr2BeKeyHolder has write_u8/u16/u32/u64/i8/.../bytes methods.
    // We delegate via CdrEncode to a temp BufferWriter and copy the bytes
    // into the holder buffer via `holder.write_bytes`.
    let key_field_lines = keyed_fields.iter().map(|ident| {
        quote! {
            {
                let mut __bw = ::zerodds_cdr::BufferWriter::new(
                    ::zerodds_cdr::Endianness::Big,
                );
                ::zerodds_cdr::CdrEncode::encode(&self.#ident, &mut __bw)?;
                holder.write_bytes(&__bw.into_bytes());
            }
        }
    });

    let key_holder_method = if is_keyed {
        quote! {
            fn encode_key_holder_be(
                &self,
                holder: &mut ::zerodds_cdr::PlainCdr2BeKeyHolder,
            ) {
                let _ = (|| -> ::core::result::Result<(), ::zerodds_cdr::EncodeError> {
                    #( #key_field_lines )*
                    ::core::result::Result::Ok(())
                })();
            }
        }
    } else {
        quote! {}
    };

    let extensibility_const = if is_keyed {
        quote! {
            const EXTENSIBILITY: ::zerodds_dcps::Extensibility =
                ::zerodds_dcps::Extensibility::Final;
            const HAS_KEY: bool = true;
        }
    } else {
        quote! {
            const EXTENSIBILITY: ::zerodds_dcps::Extensibility =
                ::zerodds_dcps::Extensibility::Final;
        }
    };

    let expanded = quote! {
        impl ::zerodds_dcps::DdsType for #name {
            const TYPE_NAME: &'static str = #type_name;
            #extensibility_const

            fn encode(&self, out: &mut ::std::vec::Vec<u8>)
                -> ::core::result::Result<(), ::zerodds_dcps::EncodeError>
            {
                let mut writer = ::zerodds_cdr::BufferWriter::new(
                    ::zerodds_cdr::Endianness::Little,
                );
                #( #encode_field_lines )*
                out.extend_from_slice(&writer.into_bytes());
                ::core::result::Result::Ok(())
            }

            fn decode(bytes: &[u8])
                -> ::core::result::Result<Self, ::zerodds_dcps::DecodeError>
            {
                let mut reader = ::zerodds_cdr::BufferReader::new(
                    bytes,
                    ::zerodds_cdr::Endianness::Little,
                );
                ::core::result::Result::Ok(Self {
                    #( #decode_field_lines )*
                })
            }

            #key_holder_method
        }
    };

    Ok(expanded)
}

#[derive(Default)]
struct StructOpts {
    type_name: Option<String>,
}

fn parse_struct_opts(attrs: &[Attribute]) -> syn::Result<StructOpts> {
    let mut out = StructOpts::default();
    for attr in attrs {
        if !attr.path().is_ident("dds") {
            continue;
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("type_name") {
                let v: Lit = meta.value()?.parse()?;
                if let Lit::Str(s) = v {
                    out.type_name = Some(s.value());
                }
            }
            Ok(())
        })?;
    }
    Ok(out)
}

#[derive(Default)]
struct FieldOpts {
    key: bool,
}

fn parse_field_opts(attrs: &[Attribute]) -> syn::Result<FieldOpts> {
    let mut out = FieldOpts::default();
    for attr in attrs {
        if !attr.path().is_ident("dds") {
            continue;
        }
        if let Meta::List(list) = &attr.meta {
            let toks = list.tokens.clone().into_token_stream().to_string();
            if toks.split(',').any(|t| t.trim() == "key") {
                out.key = true;
            }
        }
    }
    Ok(out)
}
