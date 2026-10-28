// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The rule catalogues. Spec §2.
//!
//! A rule is a value. It names an L-side pattern over the model, an
//! R-side pattern over a language CST, and the correspondences between
//! them — a `RuleDecl`, which serialises, so two languages' treatment of one
//! construct is a diff rather than a reading of two emitters.
//!
//! Rules are minimal and positive. One rule maps a struct to a type
//! declaration; a second maps a member to a field. A single rule doing
//! both could not be reused for the members of a union.

pub mod ada;
pub mod ada83;
pub mod c;
pub mod cpp;
pub mod csharp;
pub mod d;
pub mod elixir;
pub mod fsharp;
pub mod go;
pub mod java;
pub mod julia;
pub mod kotlin;
pub mod lua;
pub mod nim;
pub mod ocaml;
pub mod python;
pub mod rust;
pub mod shape;
pub mod swift;
pub mod typescript;
pub mod zig;

use seesaw_tgg::rules::format::RuleFile;
use zerodds_lang_cst::Language;

/// The catalogue for a language.
///
/// Exhaustive over `Language`, so a language added to the trees without a
/// catalogue is a compile error rather than a backend that produces
/// nothing — with one named exception, and it names itself: `Language::Node`
/// answers with a `RuleFile` of zero rules rather than joining the other
/// seventeen with real ones.
///
/// Node's materialiser (`crates/lang-rules/src/adapter/node.rs`) declares
/// almost nothing — a struct, a union, a bitset, a valuetype, an
/// exception and an interface are all plain object literals wherever the
/// codec walk or the Table 9 defaulter builds one, needing no named
/// declaration this system's ghost graph could track. A `rule::node`
/// catalogue of rules whose only content is "nothing is created here"
/// would be noise in a system built to compare what different backends
/// *do* declare, so there is no `rule::node` module and none of the
/// seventeen `.catalogue()` calls above is made for it.
/// `crates/lang-rules/tests/apply.rs` excludes `Language::Node` from the
/// loops that would otherwise apply this empty result and find it fires
/// zero times.
///
/// zerodds-lint: recursion-depth 1 — this does not recurse. The callee
/// is `the per-language ``<lang>`::catalogue``; `dds_bounded_recursion` compares only the last
/// segment of a call path, so it reads as a self-call. Each arm names
/// a different module's function.
#[must_use]
pub fn catalogue(language: Language) -> RuleFile {
    match language {
        Language::Rust => rust::catalogue(),
        Language::Cpp => cpp::catalogue(),
        Language::C => c::catalogue(),
        Language::Java => java::catalogue(),
        Language::CSharp => csharp::catalogue(),
        Language::Python => python::catalogue(),
        Language::TypeScript => typescript::catalogue(),
        Language::Go => go::catalogue(),
        Language::OCaml => ocaml::catalogue(),
        Language::Zig => zig::catalogue(),
        Language::Swift => swift::catalogue(),
        Language::Nim => nim::catalogue(),
        Language::Lua => lua::catalogue(),
        Language::Julia => julia::catalogue(),
        Language::Elixir => elixir::catalogue(),
        Language::D => d::catalogue(),
        Language::Ada => ada::catalogue(),
        Language::Ada83 => ada83::catalogue(),
        Language::FSharp => fsharp::catalogue(),
        Language::Kotlin => kotlin::catalogue(),
        Language::Node => RuleFile {
            format: 3,
            name: "node".to_string(),
            rules: Vec::new(),
        },
    }
}
