// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-flatdata-derive`. Safety classification: **STANDARD**
//! (proc-macro generiert `unsafe impl FlatStruct` — Layout-Garantien
//! werden vom Macro selbst geprueft, nicht vom Caller-Kommentar).
//!
//! `#[derive(FlatStruct)]` fuer
//! [`zerodds_flatdata::FlatStruct`](https://docs.rs/zerodds-flatdata).
//!
//! Spec: `docs/specs/zerodds-flatdata-1.0.md` §1.2 (Derive-Macro).
//!
//! ## Schichten-Position
//!
//! Layer 4 — Core Services (proc-macro fuer `zerodds-flatdata`).
//!
//! ## Public API (Stand 1.0.0-rc.1)
//!
//! - `#[derive(FlatStruct)]` — generiert `unsafe impl FlatStruct for T`
//!   mit `TYPE_HASH = sha256(type_name + field_layout)[..16]`.
//!
//! ## Compile-Time-Checks
//!
//! Der Macro lehnt mit `compile_error!` ab, wenn:
//! - `T` ist `enum` oder `union` (Layout nicht stable).
//! - `T` traegt weder `#[repr(C)]` noch `#[repr(transparent)]`
//!   (Default-Repr ist undefiniert).
//!
//! Die `Copy + 'static + Send + Sync`-Bounds werden vom Trait selbst
//! erzwungen — der Compiler emittiert verstaendliche Fehler beim
//! Versuch, ein Non-`Copy`-Type zu deriven.
//!
//! ## Beispiel
//!
//! ```ignore
//! use zerodds_flatdata_derive::FlatStruct;
//!
//! #[derive(Copy, Clone, FlatStruct)]
//! #[repr(C)]
//! struct Pose { x: f64, y: f64, z: f64 }
//! ```

#![warn(missing_docs)]

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use sha2::{Digest, Sha256};
use syn::{Attribute, Data, DeriveInput, Fields, parse_macro_input};

/// `#[derive(FlatStruct)]` — generiert `unsafe impl FlatStruct for T`.
#[proc_macro_derive(FlatStruct)]
pub fn derive_flat_struct(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(input).unwrap_or_else(|e| e.to_compile_error().into())
}

fn expand(input: DeriveInput) -> Result<TokenStream, syn::Error> {
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let layout_string = match &input.data {
        Data::Struct(s) => layout_signature(name, &s.fields),
        Data::Enum(_) => {
            return Err(syn::Error::new_spanned(
                &input,
                "FlatStruct kann nicht auf enum derive werden — repr(C)-Enum-Layout ist nicht stable",
            ));
        }
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                &input,
                "FlatStruct kann nicht auf union derive werden",
            ));
        }
    };

    if !has_repr_c_or_transparent(&input.attrs) {
        return Err(syn::Error::new_spanned(
            &input,
            "FlatStruct verlangt #[repr(C)] oder #[repr(transparent)] — \
             Default-repr-Rust hat undefiniertes Field-Layout, daher \
             waere `as_bytes()`/`from_bytes_unchecked()` UB",
        ));
    }

    let mut hasher = Sha256::new();
    hasher.update(layout_string.as_bytes());
    let digest = hasher.finalize();
    let hash_bytes: [u8; 16] = match digest[..16].try_into() {
        Ok(b) => b,
        Err(_) => {
            return Err(syn::Error::new_spanned(
                &input,
                "internal: sha256 truncate to 16 bytes failed",
            ));
        }
    };
    let hash_tokens = hash_bytes.iter().map(|b| quote! { #b });

    let expanded: TokenStream2 = quote! {
        // SAFETY: derive(FlatStruct) prueft `repr(C)`/`repr(transparent)`
        // und lehnt enum/union ab. Trait-Bounds `Copy + 'static + Send +
        // Sync` werden vom Compiler ueber die Trait-Definition erzwungen.
        // TYPE_HASH ist SHA-256 ueber `<TypeName>{<field>:<ty>,...}`,
        // erkennt Type-Rename / Field-add/remove / Field-reorder /
        // Field-Type-Change.
        #[automatically_derived]
        unsafe impl #impl_generics ::zerodds_flatdata::FlatStruct for #name #ty_generics #where_clause {
            const TYPE_HASH: [u8; 16] = [#( #hash_tokens ),*];
        }
    };
    Ok(expanded.into())
}

fn has_repr_c_or_transparent(attrs: &[Attribute]) -> bool {
    for attr in attrs {
        if !attr.path().is_ident("repr") {
            continue;
        }
        let mut found = false;
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("C") || meta.path.is_ident("transparent") {
                found = true;
            }
            Ok(())
        });
        if found {
            return true;
        }
    }
    false
}

/// Erzeugt einen Layout-String fuer SHA-256.
///
/// Format: `<TypeName>{<field-name>:<field-ty-string>,...}`. Damit
/// erkennt der Hash:
/// - Type-Rename → neuer Hash.
/// - Field-add/remove → neuer Hash.
/// - Field-reorder → neuer Hash.
/// - Field-Type-Change → neuer Hash.
fn layout_signature(name: &syn::Ident, fields: &Fields) -> String {
    let mut s = name.to_string();
    s.push('{');
    match fields {
        Fields::Named(f) => {
            for (i, field) in f.named.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                if let Some(id) = &field.ident {
                    s.push_str(&id.to_string());
                    s.push(':');
                }
                s.push_str(&type_signature(&field.ty));
            }
        }
        Fields::Unnamed(f) => {
            for (i, field) in f.unnamed.iter().enumerate() {
                if i > 0 {
                    s.push(',');
                }
                s.push_str(&type_signature(&field.ty));
            }
        }
        Fields::Unit => {
            s.push_str("()");
        }
    }
    s.push('}');
    s
}

fn type_signature(ty: &syn::Type) -> String {
    quote! { #ty }.to_string().replace(' ', "")
}
