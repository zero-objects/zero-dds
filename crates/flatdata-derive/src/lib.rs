// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-flatdata-derive`. Safety classification: **STANDARD**
//! (the proc-macro generates `unsafe impl FlatStruct` — layout
//! guarantees are checked by the macro itself, not by a caller comment).
//!
//! `#[derive(FlatStruct)]` for
//! [`zerodds_flatdata::FlatStruct`](https://docs.rs/zerodds-flatdata).
//!
//! Spec: `docs/specs/zerodds-flatdata-1.0.md` §1.2 (derive macro).
//!
//! ## Layer position
//!
//! Layer 4 — Core Services (proc-macro for `zerodds-flatdata`).
//!
//! ## Public API
//!
//! - `#[derive(FlatStruct)]` — generates `unsafe impl FlatStruct for T`
//!   with `TYPE_HASH = sha256(type_name + field_layout)[..16]`.
//!
//! ## Compile-time checks
//!
//! The macro rejects with `compile_error!` when:
//! - `T` is an `enum` or `union` (layout not stable).
//! - `T` carries neither `#[repr(C)]` nor `#[repr(transparent)]`
//!   (the default repr is undefined).
//!
//! The `Copy + 'static + Send + Sync` bounds are enforced by the trait
//! itself — the compiler emits understandable errors when attempting to
//! derive on a non-`Copy` type.
//!
//! ## Example
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

/// `#[derive(FlatStruct)]` — generates `unsafe impl FlatStruct for T`.
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
                "FlatStruct cannot be derived on an enum — repr(C) enum layout is not stable",
            ));
        }
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                &input,
                "FlatStruct cannot be derived on a union",
            ));
        }
    };

    if !has_repr_c_or_transparent(&input.attrs) {
        return Err(syn::Error::new_spanned(
            &input,
            "FlatStruct requires #[repr(C)] or #[repr(transparent)] — \
             Default repr Rust has undefined field layout, hence \
             would be `as_bytes()`/`from_bytes_unchecked()` UB",
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
        // SAFETY: derive(FlatStruct) checks `repr(C)`/`repr(transparent)`
        // and rejects enum/union. The trait bounds `Copy + 'static + Send +
        // Sync` are enforced by the compiler via the trait definition.
        // TYPE_HASH is SHA-256 over `<TypeName>{<field>:<ty>,...}`,
        // detecting type rename / field add/remove / field reorder /
        // field type change.
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

/// Builds a layout string for SHA-256.
///
/// Format: `<TypeName>{<field-name>:<field-ty-string>,...}`. With this,
/// the hash detects:
/// - Type rename → new hash.
/// - Field add/remove → new hash.
/// - Field reorder → new hash.
/// - Field type change → new hash.
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

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::unreachable,
    clippy::panic
)]
mod tests {
    use super::{expand, has_repr_c_or_transparent, layout_signature};
    use syn::{DeriveInput, parse_str};

    fn parse(src: &str) -> DeriveInput {
        parse_str::<DeriveInput>(src).expect("parse")
    }

    fn reject_reason(src: &str) -> String {
        match expand(parse(src)) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("expansion must be refused: {src}"),
        }
    }

    /// Spec §1.2 — the macro refuses an `enum`: `repr(C)` enum layout is
    /// not the fixed byte layout `from_bytes_unchecked` relies on.
    #[test]
    fn derive_rejects_enum() {
        let msg = reject_reason("#[repr(C)] enum E { A, B }");
        assert!(msg.contains("enum"), "{msg}");
    }

    /// Spec §1.2 — the macro refuses a `union`.
    #[test]
    fn derive_rejects_union() {
        let msg = reject_reason("#[repr(C)] union U { a: u32, b: f32 }");
        assert!(msg.contains("union"), "{msg}");
    }

    /// Spec §1.2 — a struct without `#[repr(C)]` / `#[repr(transparent)]`
    /// is refused: the default Rust repr has no guaranteed field order, so
    /// the byte cast would be undefined behaviour.
    #[test]
    fn derive_rejects_struct_without_repr_c() {
        let msg = reject_reason("struct S { a: u32, b: u32 }");
        assert!(msg.contains("repr(C)"), "{msg}");
    }

    /// Spec §1.2 — `#[repr(transparent)]` and `#[repr(C)]` are the two
    /// accepted forms; `#[repr(packed)]` alone is not.
    #[test]
    fn repr_attribute_gate_accepts_c_and_transparent_only() {
        assert!(has_repr_c_or_transparent(
            &parse("#[repr(C)] struct S;").attrs
        ));
        assert!(has_repr_c_or_transparent(
            &parse("#[repr(transparent)] struct S(u32);").attrs
        ));
        assert!(has_repr_c_or_transparent(
            &parse("#[repr(C, packed)] struct S { a: u8 }").attrs
        ));
        assert!(!has_repr_c_or_transparent(
            &parse("#[repr(packed)] struct S { a: u8 }").attrs
        ));
        assert!(!has_repr_c_or_transparent(
            &parse("struct S { a: u8 }").attrs
        ));
    }

    /// Spec §1.2 — `TYPE_HASH` is taken over
    /// `<TypeName>{<field-name>:<field-ty>,...}`, so a rename, a reorder,
    /// a type change and an added field each produce a different string.
    #[test]
    fn layout_signature_covers_name_order_and_types() {
        let sig = |src: &str| {
            let input = parse(src);
            match &input.data {
                syn::Data::Struct(s) => layout_signature(&input.ident, &s.fields),
                _ => unreachable!("struct expected"),
            }
        };
        let base = sig("#[repr(C)] struct Pose { x: f64, y: f64 }");
        assert_eq!(base, "Pose{x:f64,y:f64}");
        assert_ne!(base, sig("#[repr(C)] struct Pose2 { x: f64, y: f64 }"));
        assert_ne!(base, sig("#[repr(C)] struct Pose { y: f64, x: f64 }"));
        assert_ne!(base, sig("#[repr(C)] struct Pose { x: f32, y: f64 }"));
        assert_ne!(
            base,
            sig("#[repr(C)] struct Pose { x: f64, y: f64, z: f64 }")
        );
        assert_eq!(sig("#[repr(C)] struct Unit;"), "Unit{()}");
        assert_eq!(sig("#[repr(transparent)] struct W(u32);"), "W{u32}");
    }
}
