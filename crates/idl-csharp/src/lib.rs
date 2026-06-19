// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL4 → C# 12.0 source code generator (OMG IDL4-CSharp mapping,
//! formal/2024-12-01).
//!
//! Crate `zerodds-idl-csharp` — foundation of the language binding (cluster C5.3-a).
//!
//! Safety classification: **SAFE (std-only)**. Pure build-time tool —
//! `forbid(unsafe_code)`, no no_std use case.
//!
//! # Scope (C5.3-a / phase 3.1-3.3)
//! - Phase 3.1: header layout (`#nullable enable`, `using`, `namespace`).
//! - Phase 3.2: primitive mapping
//!   (boolean → bool, octet → byte, char/wchar → char,
//!   short → short, ushort → ushort, long → int, ulong → uint,
//!   long long → long, ulong long → ulong, float/double, string → string).
//! - Phase 3.3: aggregate types — struct → `record class` with init-only
//!   properties, enum → `enum`, union → discriminated record,
//!   typedef → `record class` wrapper (spec-compliant file-scoped
//!   using-alias arrives with C5.3-b), sequence → `IList<T>`, array → `T[]`,
//!   inheritance → `record class : Parent`.
//!
//! # C5.3-b additions (this revision)
//! - `ISequence<T>` / `IBoundedSequence<T>` runtime contract
//!   (see `runtime/Omg.Types.cs`); codegen emits these instead of
//!   bare `IList<T>`.
//! - Annotation-Bridge `@key|@id|@optional|@must_understand|@external`
//!   on members, `@nested|@extensibility(...)` on types — emitted as
//!   C# attributes from `Omg.Types`.
//! - `ITopicType<T>` marker on every top-level (non-`@nested`) struct.
//!
//! # Deliberately not in this crate
//! - Phase 3.4: DDS-CSharp integration (P/Invoke to the Rust core).
//! - Time/Duration/Status/QoS/Listener codegen.
//! - File-scoped namespace + `using <Alias> = <Type>;` top-level form.
//! - Bitset/Bitmask/Map/Fixed/Any/Interface/Valuetype.
//!
//! # Example
//!
//! ```
//! use zerodds_idl::config::ParserConfig;
//! use zerodds_idl_csharp::{generate_csharp, CsGenOptions};
//!
//! let ast = zerodds_idl::parse(
//!     "module M { struct S { long x; }; };",
//!     &ParserConfig::default(),
//! )
//! .expect("parse");
//! let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("gen");
//! assert!(cs.contains("namespace M"));
//! assert!(cs.contains("record class S"));
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod annotations;
pub(crate) mod bitset;
pub(crate) mod corba_traits;
pub mod emitter;
pub mod error;
pub mod keywords;
pub mod type_map;
pub(crate) mod typesupport;
pub(crate) mod verbatim;

pub use error::CsGenError;

use zerodds_idl::ast::Specification;

/// Configuration of the C# code generator.
#[derive(Debug, Clone)]
pub struct CsGenOptions {
    /// Optional outer namespace that wraps the entire output. `None`
    /// or empty = no wrapper.
    pub root_namespace: Option<String>,
    /// Indent width in spaces. Default 4 (C# coding conventions).
    pub indent_width: usize,
    /// If `true`: struct mapping uses `record class` (spec-compliant).
    /// If `false`: struct mapping uses plain `class` (legacy CCM).
    /// Default `true`.
    pub use_records: bool,
    /// Annex A.1 (idl4-csharp-1.0) — opt-in: emits a CORBA marker
    /// (`Corba.ValueTypeAttribute`) plus a static `Corba.Traits` helper
    /// per top-level type. Default `false`.
    pub emit_corba_traits: bool,
}

impl Default for CsGenOptions {
    fn default() -> Self {
        Self {
            root_namespace: None,
            indent_width: 4,
            use_records: true,
            emit_corba_traits: false,
        }
    }
}

/// Produces a complete C# 12.0 source text from an IDL specification.
///
/// # Errors
/// - [`CsGenError::UnsupportedConstruct`]: IDL construct outside the current scope
///   (e.g. `interface`, `valuetype`, `fixed`, `any`, `bitset`, `bitmask`).
/// - [`CsGenError::InvalidName`]: an identifier is empty or already
///   `@`-prefixed (double-escape).
/// - [`CsGenError::InheritanceCycle`]: direct or indirect
///   self-inheritance in the struct graph.
pub fn generate_csharp(ast: &Specification, opts: &CsGenOptions) -> Result<String, CsGenError> {
    let mut out = emitter::emit_source(ast, opts)?;
    if opts.emit_corba_traits {
        corba_traits::emit_corba_traits(&mut out, ast)?;
    }
    Ok(out)
}

/// Convenience variant with the `emit_corba_traits` flag enabled.
///
/// Cross-ref: `idl4-csharp-1.0` Annex A.1.
///
/// # Errors
/// As for [`generate_csharp`].
pub fn generate_csharp_with_corba_traits(
    ast: &Specification,
    opts: &CsGenOptions,
) -> Result<String, CsGenError> {
    let opts = CsGenOptions {
        emit_corba_traits: true,
        ..opts.clone()
    };
    generate_csharp(ast, &opts)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic)]
    use super::*;
    use zerodds_idl::config::ParserConfig;

    fn gen_cs(src: &str) -> String {
        let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse must succeed");
        generate_csharp(&ast, &CsGenOptions::default()).expect("gen must succeed")
    }

    #[test]
    fn empty_source_emits_only_preamble() {
        let cs = gen_cs("");
        assert!(cs.contains("// Generated by zerodds idl-csharp"));
        assert!(cs.contains("#nullable enable"));
        assert!(cs.contains("using System;"));
        assert!(!cs.contains("namespace M"));
    }

    #[test]
    fn empty_module_emits_namespace() {
        let cs = gen_cs("module M {};");
        assert!(cs.contains("namespace M"));
        assert!(cs.contains("} // namespace M"));
    }

    #[test]
    fn three_level_modules_nest() {
        let cs = gen_cs("module A { module B { module C {}; }; };");
        assert!(cs.contains("namespace A"));
        assert!(cs.contains("namespace B"));
        assert!(cs.contains("namespace C"));
    }

    #[test]
    fn primitive_struct_member_uses_correct_cs_types() {
        let cs = gen_cs(
            "struct S { boolean b; octet o; short s; long l; long long ll; \
             unsigned short us; unsigned long ul; unsigned long long ull; \
             float f; double d; };",
        );
        assert!(cs.contains("public bool B"));
        assert!(cs.contains("public byte O"));
        assert!(cs.contains("public short S"));
        assert!(cs.contains("public int L "));
        assert!(cs.contains("public long Ll"));
        assert!(cs.contains("public ushort Us"));
        assert!(cs.contains("public uint Ul "));
        assert!(cs.contains("public ulong Ull"));
        assert!(cs.contains("public float F"));
        assert!(cs.contains("public double D"));
    }

    #[test]
    fn string_member_uses_string() {
        let cs = gen_cs("struct S { string name; };");
        assert!(cs.contains("public string Name"));
    }

    #[test]
    fn sequence_member_uses_isequence() {
        // C5.3-b: unbounded sequence → `ISequence<T>` from Omg.Types.
        let cs = gen_cs("struct S { sequence<long> data; };");
        assert!(cs.contains("using System.Collections.Generic;"));
        assert!(cs.contains("using Omg.Types;"));
        assert!(cs.contains("ISequence<int>"));
    }

    #[test]
    fn bounded_sequence_member_uses_ibounded_sequence() {
        // C5.3-b: bounded sequence → `IBoundedSequence<T>` from Omg.Types.
        let cs = gen_cs("struct S { sequence<long, 100> data; };");
        assert!(cs.contains("using Omg.Types;"));
        assert!(cs.contains("IBoundedSequence<int>"));
    }

    #[test]
    fn array_member_uses_jagged_array() {
        let cs = gen_cs("struct S { long matrix[3][4]; };");
        // Jagged: int[][] (the spec allows both variants).
        assert!(cs.contains("int[][]"));
    }

    #[test]
    fn enum_emits_int_backed_enum() {
        let cs = gen_cs("enum Color { RED, GREEN, BLUE };");
        assert!(cs.contains("public enum Color : int"));
        assert!(cs.contains("RED,"));
        assert!(cs.contains("BLUE,"));
    }

    #[test]
    fn typedef_emits_alias_record() {
        let cs = gen_cs("typedef long MyInt;");
        assert!(cs.contains("public sealed record class MyInt(int Value);"));
    }

    #[test]
    fn inheritance_emits_record_inheritance() {
        let cs = gen_cs("struct Parent { long x; }; struct Child : Parent { long y; };");
        assert!(cs.contains("record class Child : Parent"));
    }

    #[test]
    fn keyed_struct_marker_appears() {
        let cs = gen_cs("struct S { @key long id; long val; };");
        assert!(cs.contains("[Key]"));
    }

    #[test]
    fn optional_member_uses_nullable() {
        let cs = gen_cs("struct S { @optional long maybe; };");
        assert!(cs.contains("int? Maybe"));
    }

    #[test]
    fn exception_inherits_exception() {
        let cs = gen_cs("exception NotFound { string what_; };");
        assert!(cs.contains("class NotFound : Exception"));
    }

    #[test]
    fn union_uses_discriminator_record() {
        let cs = gen_cs(
            "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
        );
        assert!(cs.contains("record class U"));
        assert!(cs.contains("public int Discriminator"));
        assert!(cs.contains("public object? Value"));
        assert!(cs.contains("// case default"));
    }

    #[test]
    fn header_starts_with_generated_marker() {
        let cs = gen_cs("");
        assert!(cs.starts_with("// Generated by zerodds idl-csharp."));
    }

    #[test]
    fn nullable_enable_appears_exactly_once() {
        let cs = gen_cs("module M { struct S { long x; }; };");
        let count = cs.matches("#nullable enable").count();
        assert_eq!(count, 1);
    }

    #[test]
    fn record_class_is_init_only() {
        let cs = gen_cs("struct S { long x; };");
        assert!(cs.contains("get; init;"));
    }

    #[test]
    fn root_namespace_option_wraps_output() {
        let ast =
            zerodds_idl::parse("struct S { long x; };", &ParserConfig::default()).expect("parse");
        let opts = CsGenOptions {
            root_namespace: Some("Zerodds.Generated".into()),
            ..Default::default()
        };
        let cs = generate_csharp(&ast, &opts).expect("gen");
        assert!(cs.contains("namespace Zerodds.Generated"));
        assert!(cs.contains("} // namespace Zerodds.Generated"));
    }

    #[test]
    fn non_service_interface_emits_csharp_interface() {
        let ast = zerodds_idl::parse("interface I { void op(); };", &ParserConfig::default())
            .expect("parse");
        let cs = generate_csharp(&ast, &CsGenOptions::default()).expect("ok");
        assert!(cs.contains("public interface I"));
    }

    #[test]
    fn const_decl_emits_const() {
        let cs = gen_cs("const long MAX = 100;");
        assert!(cs.contains("public const int MAX = 100;"));
    }

    #[test]
    fn options_have_sensible_defaults() {
        let o = CsGenOptions::default();
        assert_eq!(o.indent_width, 4);
        assert!(o.root_namespace.is_none());
        assert!(o.use_records);
    }

    #[test]
    fn options_clone_works() {
        let o = CsGenOptions {
            root_namespace: Some("Foo".into()),
            indent_width: 2,
            use_records: false,
            emit_corba_traits: false,
        };
        let cloned = o.clone();
        assert_eq!(cloned.indent_width, 2);
        assert_eq!(cloned.root_namespace.as_deref(), Some("Foo"));
        assert!(!cloned.use_records);
    }
}
