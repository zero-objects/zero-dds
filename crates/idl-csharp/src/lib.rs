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
pub(crate) mod rpc;
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
        // F4/A4: explicit ordinals so `@value` gaps survive on the wire.
        assert!(cs.contains("RED = 0,"));
        assert!(cs.contains("BLUE = 2,"));
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
    fn keyhash_typedef_aliased_key_emits_real_bytes() {
        // Regression: `@key` whose type is a typedef alias to a primitive
        // used to emit NOTHING in `ComputeKeyHash` (a silent wrong-wire
        // KeyHash) instead of dealiasing to `MyId`'s underlying `long` and
        // writing it. `MyId` must be dealiased to a `WriteInt32` call, and
        // the old silent no-op comment must be gone.
        let cs = gen_cs("typedef long MyId;\n@final struct S { @key MyId id; long val; };");
        assert!(
            cs.contains("__kw.WriteInt32((sample.Id).Value);"),
            "typedef-aliased @key must dealias to the underlying primitive write:\n{cs}"
        );
        assert!(
            !cs.contains("nested key types delegate via TypeSupport"),
            "the silent zero-byte no-op for a Scoped @key must be gone:\n{cs}"
        );
    }

    #[test]
    fn keyhash_multi_level_typedef_chain_key_dealiases_to_terminal_primitive() {
        // typedef chain: Score -> Points -> long. The KeyHash must dealias
        // all the way to `long`, not stop at the first level.
        let cs = gen_cs(
            "typedef long Points;\ntypedef Points Score;\n@final struct S { @key Score s; };",
        );
        assert!(
            cs.contains("__kw.WriteInt32(((sample.S).Value).Value);"),
            "multi-level typedef chain must dealias through both levels to Int32:\n{cs}"
        );
    }

    #[test]
    fn keyhash_nested_struct_key_expands_inner_key_members() {
        // A `@key` member whose type is a nested struct with its own `@key`
        // members must expand into THOSE members (XTypes 1.3 §7.6.8), not
        // silently emit nothing.
        let cs = gen_cs(
            "@final struct Inner { @key long x; long ignored; @key long y; };\n\
             @final struct Outer { @key Inner i; long z; };",
        );
        assert!(
            cs.contains("__kw.WriteInt32(sample.I.X);")
                && cs.contains("__kw.WriteInt32(sample.I.Y);"),
            "nested-struct @key must expand into the inner struct's own @key members:\n{cs}"
        );
        assert!(
            !cs.contains("sample.I.Ignored"),
            "a non-@key inner member must NOT be included when the inner struct has explicit @key members:\n{cs}"
        );
    }

    #[test]
    fn keyhash_nested_struct_without_keys_uses_all_inner_members() {
        // XTypes 1.3 §7.6.8: an aggregate @key member with NO @key members of
        // its own is keyed in full (all its members).
        let cs = gen_cs(
            "@final struct Pair { long a; long b; };\n\
             @final struct Holder { @key Pair p; };",
        );
        assert!(
            cs.contains("__kw.WriteInt32(sample.P.A);")
                && cs.contains("__kw.WriteInt32(sample.P.B);"),
            "a keyless nested struct must key on ALL its members:\n{cs}"
        );
    }

    #[test]
    fn keyhash_multiple_keys_emit_in_member_id_order_not_declaration_order() {
        // Regression: the KeyHash loop walked `s.members` in DECLARATION
        // order, ignoring explicit `@id(N)`. XTypes 1.3 §7.6.8.3.1.b
        // mandates member-id order. `@id(1) octet a; @id(0) long b;` must
        // write `b` (id 0) before `a` (id 1).
        let cs = gen_cs("@final struct K { @id(1) @key octet a; @id(0) @key long b; };");
        let pos_a = cs.find("__kw.WriteOctet(sample.A);").expect("write a");
        let pos_b = cs.find("__kw.WriteInt32(sample.B);").expect("write b");
        assert!(
            pos_b < pos_a,
            "member-id order must place b (id 0) before a (id 1):\n{cs}"
        );
    }

    #[test]
    fn keyhash_nested_struct_key_expands_in_member_id_order() {
        // Same ordering rule applies to the expanded nested-struct @key
        // subset.
        let cs = gen_cs(
            "@final struct Inner { @id(1) @key octet a; @id(0) @key long b; };\n\
             @final struct Outer { @key Inner i; };",
        );
        let pos_a = cs.find("__kw.WriteOctet(sample.I.A);").expect("write i.a");
        let pos_b = cs.find("__kw.WriteInt32(sample.I.B);").expect("write i.b");
        assert!(
            pos_b < pos_a,
            "nested-struct @key subset must also expand in member-id order:\n{cs}"
        );
    }

    #[test]
    fn keyhash_branch_decision_is_static_not_runtime_length() {
        // Regression: the zero-pad/MD5 branch (XTypes 1.3 §7.6.8.4 step 5) is
        // decided STATICALLY per topic type from the KeyHolder's maximum
        // size, not from the actual runtime byte count of a given sample
        // (`crates/cdr/src/key_hash.rs`'s module doc is authoritative). A
        // single `@key long` has a static max of 4 bytes (<=16) -> the
        // generated KeyHash must unconditionally zero-pad, with NO runtime
        // `__kb.Length` branch at all.
        let cs = gen_cs("@final struct K { @key long id; };");
        assert!(
            !cs.contains("if (__kb.Length"),
            "a statically-small KeyHolder must not emit a runtime branch decision:\n{cs}"
        );
        assert!(
            cs.contains("var __h = new byte[16];"),
            "must zero-pad:\n{cs}"
        );

        // Four longs = exactly 16 (the zero-pad boundary) -> still zero-pad,
        // unconditionally.
        let cs16 =
            gen_cs("@final struct K { @key long a; @key long b; @key long c; @key long d; };");
        assert!(
            cs16.contains("var __h = new byte[16];") && !cs16.contains("Md5.Hash"),
            "16 bytes exactly is still the zero-pad branch:\n{cs16}"
        );

        // Five longs = 20 > 16 -> unconditionally MD5, no runtime check.
        let cs20 = gen_cs(
            "@final struct K { @key long a; @key long b; @key long c; @key long d; @key long e; };",
        );
        assert!(
            cs20.contains("return Md5.Hash(__kb);") && !cs20.contains("if (__kb.Length"),
            "over 16 bytes is unconditionally the MD5 branch, no runtime check:\n{cs20}"
        );

        // An unbounded string @key is dynamically sized -> unconditionally
        // MD5 (matches `idl-rust`'s `key_holder_atom_size`/`keyhash.rs`
        // reference: unbounded string has no static max).
        let cs_dyn = gen_cs("@final struct K { @key string s; };");
        assert!(
            cs_dyn.contains("return Md5.Hash(__kb);") && !cs_dyn.contains("if (__kb.Length"),
            "an unbounded-string key is dynamically sized -> always MD5:\n{cs_dyn}"
        );
    }

    #[test]
    fn keyhash_enum_key_is_a_loud_runtime_error_not_silent_wrong() {
        // Enum @key is not yet a supported shape (matches the idl-rust
        // reference, which also rejects it) — it must be a loud runtime
        // throw, not a silent no-op.
        let cs = gen_cs("enum Color { Red, Green, Blue };\n@final struct S { @key Color c; };");
        assert!(
            cs.contains("throw new XcdrException(\"unsupported key type"),
            "an unsupported enum @key must throw loudly, not silently emit nothing:\n{cs}"
        );
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
