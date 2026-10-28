// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Crate `zerodds-corba-codegen`. Safety classification: **STANDARD**.
//!
//! The CORBA 3.3 Annex A.1 special types, the Annex A.1.1 type traits,
//! and the stub and skeleton mappings that go with them.
//! `no_std + alloc`, `forbid(unsafe_code)`.
//!
//! Annex A.1 lists thirteen IDL constructs whose language mapping is not
//! the mechanical one. This crate answers, for a type in the model:
//! which of the thirteen it is, if any, and what each language PSM calls
//! it — as a name, and as a node in that language's CST.
//!
//! ## What it is over
//!
//! `zerodds-ast` for the model, and `zerodds-lang-cst` plus
//! `zerodds-lang-render` for the output. It used to be over nothing: the
//! previous version was four tables of strings, and its consumers were
//! `zerodds-idl-cpp`, `-csharp` and `-java`, which this rebuild replaced
//! with `zerodds-lang-rules` and `zerodds-lang-render`. The tables held
//! up; the attachment did not.
//!
//! Three consequences, all of them visible in the public interface:
//!
//! 1. **[`special_type_of`] takes an [`Id<TypeNode>`].** A generator asks
//!    the model rather than matching on [`zerodds_ast::TypeKind`] itself,
//!    so the thirteen rows have one reader and a fourteenth construct
//!    cannot be answered twice differently.
//! 2. **[`repository_id_of`] reads the model's identifier.**
//!    `zerodds-idl-lower` already applies §10.7 including `#pragma
//!    prefix`, `#pragma version` and `#pragma ID`. A second derivation
//!    here would see none of them.
//! 3. **The renderers build CST nodes.** The previous stub and skeleton
//!    helpers concatenated strings and produced a comment; nothing in
//!    the workspace called them, which the crate's own audit recorded.
//!    They now build [`cpp::Function`], [`java::Method`] and
//!    [`rust::Function`], which `zerodds-lang-render` writes out.
//!
//! ## Where the tables were wrong, and how that was found
//!
//! Four rows of the Rust column named crates that no longer carry the
//! type, and one named a type that never carried the value:
//!
//! | row | was | is | why |
//! |---|---|---|---|
//! | `long double` | `f64` | `zerodds_float128::Float128` | `f64` put eight bytes of a value nobody wrote on the wire |
//! | `any` | `zerodds_dcps::DdsAny` | `zerodds_corba_rt::CorbaAny` | moved out of the wire crate in step 1 |
//! | `TypeCode` | `zerodds_corba_rust::TypeCode` | `zerodds_corba_rt::TypeCode` | same move |
//! | `wstring` | `String` | `zerodds_corba_rt::WString` | §15.3.2.7's byte-order mark has nowhere to live on a `String` |
//! | `fixed<P,S>` | `zerodds_cdr::fixed::Fixed` | `Fixed` | `zerodds-cdr` was demolished and `zerodds-wire` has no holder |
//!
//! The C++ and Java columns are the OMG's, and four of their answers
//! moved after `omniidl`, `tao_idl` and JacORB's `idl` were asked what
//! they actually generate — `tests/orb_agreement.rs` says which four and
//! `tests/orb-corpus/recorded/` holds what the three compilers wrote.
//! The C# column has an OMG mapping and no compiler on the measurement
//! host, so it is cited and not measured, and this says so rather than
//! letting it pass as checked.
//!
//! ## The two things this crate did not have
//!
//! **A C# column that renders.** [`TargetLanguage::CSharp`] had the
//! OMG's type table and no CST to write a declaration into, so
//! [`render_stub_op`] and [`render_skeleton_dispatch`] answered
//! [`NoRenderer`] for it. `zerodds_lang_cst::csharp` exists now:
//! [`csharp_stub_method`], [`csharp_param_modifier`] and
//! [`csharp_dispatch`] complete the column, and
//! [`TargetLanguage::RENDERABLE`] is all four.
//!
//! **The Annex A.1.1 type traits.** [`traits`] answers, for a type in
//! the model, which of Tables A.1 to A.12's members it has and what each
//! one is defined as, and renders that as `CORBA::traits<T>` in C++ and
//! as a companion table of constants in the three columns with no
//! templates. It is over [`stub`] rather than beside it: the passing
//! forms and the fixed-versus-variable split are [`shape_of`]'s answers,
//! so the two layers cannot disagree about what a type is.
//!
//! ## Layer position
//!
//! Over `zerodds-ast` and the backend arc's layers 0 and 1. Under the
//! CORBA emitters.
//!
//! ## Example
//!
//! ```
//! use zerodds_corba_codegen::{SpecialType, TargetLanguage, language_mapping};
//!
//! assert_eq!(
//!     language_mapping(SpecialType::String, TargetLanguage::Java),
//!     "java.lang.String"
//! );
//! ```
//!
//! [`Id<TypeNode>`]: zerodds_ast_core::Id
//! [`cpp::Function`]: zerodds_lang_cst::cpp::Function
//! [`java::Method`]: zerodds_lang_cst::java::Method
//! [`rust::Function`]: zerodds_lang_cst::rust::Function

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod repository_id;
pub mod skeleton;
pub mod special_types;
pub mod stub;
pub mod traits;

pub use repository_id::{
    ResolvedRepositoryId, build_repository_id, build_repository_id_with_prefix, repository_id_of,
    scoped_name_from_repository_id, split_repository_id,
};
pub use skeleton::{
    SkeletonOp, cpp_dispatch, csharp_dispatch, java_dispatch, render_skeleton_dispatch,
    rust_dispatch, skeleton_ops_of,
};
pub use special_types::{
    SpecialType, TargetLanguage, all_special_types, cpp_type, csharp_type, java_type,
    language_mapping, mapping_table, mapping_table_markdown, rust_type, special_type_of,
    special_type_of_scoped,
};
pub use stub::{
    NoRenderer, StubOp, StubParam, StubType, TypeShape, cpp_param_type, cpp_return_type,
    cpp_stub_function, csharp_param_modifier, csharp_param_type, csharp_return_type,
    csharp_stub_method, java_holder_type, java_param_type, java_return_type, java_stub_method,
    render_stub_op, rust_param_type, rust_stub_function, shape_of, stub_op_of, stub_type_of,
};
pub use traits::{
    DerivedForm, TraitDefinition, TraitMember, TypeTraits, cpp_traits_specialisation,
    csharp_traits_class, java_traits_class, render_type_traits, rust_traits_module, traits_of,
};
