// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL-to-C++ Annex A.1 — CORBA-specific type traits.
//!
//! Spec: `idl4-cpp-1.0` Annex A.1. Emits CORBA trait specializations
//! for each top-level struct/union/typedef/enum that a CORBA C++ mapping
//! expects:
//!
//! ```cpp
//! namespace CORBA {
//!     template<> struct traits<::M::T> {
//!         using value_type   = ::M::T;
//!         using in_type      = const ::M::T&;
//!         using out_type     = ::M::T&;
//!         using inout_type   = ::M::T&;
//!         static constexpr bool is_local = false;
//!     };
//! }
//! ```
//!
//! Annex A.1 distinguishes fixed-size vs. variable-size:
//!
//! - **fixed-size** (only primitive members, no string/sequence):
//!   `out_type = T&`.
//! - **variable-size** (at least one string/sequence/scoped member):
//!   `out_type = T*&` (heap-pointer reference).
//!
//! Cross-ref dds-psm-cxx §6.4.6.1 (layout classification).

use std::fmt::Write;

use zerodds_idl::ast::{
    ConstrTypeDecl, Definition, ModuleDef, Specification, StructDcl, StructDef, TypeDecl, TypeSpec,
    UnionDcl, UnionDef,
};

use crate::error::CppGenError;

/// Emits Annex A.1 CORBA trait specializations.
///
/// # Errors
/// `CppGenError::Internal` on `fmt::Write` error.
pub(crate) fn emit_corba_traits(out: &mut String, spec: &Specification) -> Result<(), CppGenError> {
    let mut emitted = Vec::new();
    collect_top_level(&spec.definitions, &mut Vec::new(), &mut emitted);
    if emitted.is_empty() {
        return Ok(());
    }

    writeln!(out, "// Annex A.1 — CORBA-spezifische Type-Traits.").map_err(fmt_err)?;
    writeln!(out, "// Requires a CORBA header: <CORBA.h> or equivalent.").map_err(fmt_err)?;
    writeln!(out, "namespace CORBA {{").map_err(fmt_err)?;
    for entry in emitted {
        emit_one(out, &entry)?;
    }
    writeln!(out, "}} // namespace CORBA").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

#[derive(Debug)]
struct TraitEntry {
    cpp_qualified: String,
    variable_size: bool,
}

/// zerodds-lint: recursion-depth 32 (IDL-Module-Nesting)
fn collect_top_level(defs: &[Definition], scope: &mut Vec<String>, out: &mut Vec<TraitEntry>) {
    for d in defs {
        match d {
            Definition::Module(m) => collect_module(m, scope, out),
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                out.push(TraitEntry {
                    cpp_qualified: qualified(scope, &s.name.text),
                    variable_size: struct_is_variable(s),
                });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                out.push(TraitEntry {
                    cpp_qualified: qualified(scope, &u.name.text),
                    variable_size: union_is_variable(u),
                });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                out.push(TraitEntry {
                    cpp_qualified: qualified(scope, &e.name.text),
                    variable_size: false,
                });
            }
            _ => {}
        }
    }
}

/// zerodds-lint: recursion-depth 32 (IDL-Module-Nesting)
fn collect_module(m: &ModuleDef, scope: &mut Vec<String>, out: &mut Vec<TraitEntry>) {
    scope.push(m.name.text.clone());
    collect_top_level(&m.definitions, scope, out);
    scope.pop();
}

fn qualified(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        format!("::{name}")
    } else {
        format!("::{}::{name}", scope.join("::"))
    }
}

fn struct_is_variable(s: &StructDef) -> bool {
    s.members.iter().any(|m| type_is_variable(&m.type_spec))
}

fn union_is_variable(u: &UnionDef) -> bool {
    u.cases
        .iter()
        .any(|c| type_is_variable(&c.element.type_spec))
}

fn type_is_variable(ts: &TypeSpec) -> bool {
    match ts {
        TypeSpec::Primitive(_) => false,
        TypeSpec::Fixed(_) => false,
        TypeSpec::String(_) => true,
        TypeSpec::Sequence(_) => true,
        TypeSpec::Map(_) => true,
        // Scoped refs can be anything — conservatively classified as variable
        // (Annex A.1 allows over-classification because
        // const T& and T*& both compile in either case).
        TypeSpec::Scoped(_) => true,
        TypeSpec::Any => true,
    }
}

fn emit_one(out: &mut String, entry: &TraitEntry) -> Result<(), CppGenError> {
    let q = &entry.cpp_qualified;
    writeln!(out, "    template<> struct traits<{q}> {{").map_err(fmt_err)?;
    writeln!(out, "        using value_type = {q};").map_err(fmt_err)?;
    writeln!(out, "        using in_type    = const {q}&;").map_err(fmt_err)?;
    if entry.variable_size {
        writeln!(out, "        using out_type   = {q}*&;").map_err(fmt_err)?;
    } else {
        writeln!(out, "        using out_type   = {q}&;").map_err(fmt_err)?;
    }
    writeln!(out, "        using inout_type = {q}&;").map_err(fmt_err)?;
    writeln!(out, "        static constexpr bool is_local = false;").map_err(fmt_err)?;
    writeln!(out, "    }};").map_err(fmt_err)?;
    Ok(())
}

fn fmt_err(_: std::fmt::Error) -> CppGenError {
    CppGenError::Internal("string formatting failed".into())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use crate::{CppGenOptions, generate_cpp_header_with_corba_traits};
    use zerodds_idl::config::ParserConfig;

    fn render(src: &str) -> String {
        let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
        let opts = CppGenOptions::default();
        generate_cpp_header_with_corba_traits(&ast, &opts).expect("gen")
    }

    #[test]
    fn empty_source_emits_no_traits_block() {
        let out = render("");
        assert!(!out.contains("namespace CORBA"));
    }

    #[test]
    fn fixed_size_struct_uses_value_out_type() {
        let out = render("struct S { long x; };");
        assert!(out.contains("namespace CORBA {"));
        assert!(out.contains("template<> struct traits<::S>"));
        assert!(out.contains("using out_type   = ::S&;"));
        assert!(!out.contains("using out_type   = ::S*&;"));
    }

    #[test]
    fn variable_size_struct_uses_pointer_out_type() {
        let out = render("struct S { string name; };");
        assert!(out.contains("template<> struct traits<::S>"));
        assert!(out.contains("using out_type   = ::S*&;"));
    }

    #[test]
    fn enum_traits_is_fixed_size() {
        let out = render("enum Color { RED, GREEN, BLUE };");
        assert!(out.contains("template<> struct traits<::Color>"));
        assert!(out.contains("using out_type   = ::Color&;"));
    }

    #[test]
    fn nested_module_qualifies_correctly() {
        let out = render("module A { module B { struct S { long x; }; }; };");
        assert!(out.contains("template<> struct traits<::A::B::S>"));
    }

    #[test]
    fn union_with_string_branch_is_variable() {
        let out = render("union U switch (long) { case 1: string s; case 2: long n; };");
        assert!(out.contains("template<> struct traits<::U>"));
        assert!(out.contains("using out_type   = ::U*&;"));
    }

    #[test]
    fn is_local_default_false_for_value_types() {
        let out = render("struct S { long x; };");
        assert!(out.contains("static constexpr bool is_local = false;"));
    }

    #[test]
    fn in_type_is_always_const_ref() {
        let out = render("struct S { long x; };");
        assert!(out.contains("using in_type    = const ::S&;"));
    }

    #[test]
    fn inout_type_is_always_mut_ref() {
        let out = render("struct S { string s; };");
        assert!(out.contains("using inout_type = ::S&;"));
    }

    #[test]
    fn sequence_member_marks_struct_as_variable() {
        let out = render("struct S { sequence<long> data; };");
        assert!(out.contains("using out_type   = ::S*&;"));
    }
}
