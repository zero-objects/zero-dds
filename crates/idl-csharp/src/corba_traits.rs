// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL-to-C# 1.0 Annex A.1 — CORBA-specific type mappings.
//!
//! Spec: `idl4-csharp-1.0` Annex A.1. The C# PSM has no direct
//! type traits like the C++ PSM; CORBA mappings for C# are primarily:
//!
//! 1. **Parameter-direction convention** (in/out/inout) as
//!    language modifiers `in`/`out`/`ref` on the call side.
//! 2. **Marker attribute** `[Corba.ValueTypeAttribute]` per
//!    top-level type, for ORB-side stub/skeleton discovery
//!    (analogous to `IIOP.NET`).
//! 3. **Static helper** `Corba.Traits` with per-type constants as
//!    doc markers, so tooling can locate the in/out/inout direction.
//!
//! The A.1 layer is emitted opt-in as an additional codegen block
//! (`CsGenOptions::emit_corba_traits = true`).

use std::fmt::Write;

use zerodds_idl::ast::{
    ConstrTypeDecl, Definition, ModuleDef, Specification, StructDcl, StructDef, TypeDecl, TypeSpec,
    UnionDcl, UnionDef,
};

use crate::error::CsGenError;

/// Emits Annex A.1 CORBA markers (ValueType attribute + Trait helper).
///
/// # Errors
/// `CsGenError::Internal` on `fmt::Write` failure.
pub(crate) fn emit_corba_traits(out: &mut String, spec: &Specification) -> Result<(), CsGenError> {
    let mut emitted = Vec::new();
    collect_top_level(&spec.definitions, &mut Vec::new(), &mut emitted);
    if emitted.is_empty() {
        return Ok(());
    }

    writeln!(out, "// Annex A.1 — CORBA-specific type mappings.").map_err(fmt_err)?;
    writeln!(
        out,
        "// Requires CORBA runtime: IIOP.NET, Remoting.Corba, or equivalent."
    )
    .map_err(fmt_err)?;
    writeln!(out, "namespace Corba").map_err(fmt_err)?;
    writeln!(out, "{{").map_err(fmt_err)?;
    writeln!(
        out,
        "    /// Marker attribute for Annex A.1 CORBA value types."
    )
    .map_err(fmt_err)?;
    writeln!(out, "    [System.AttributeUsage(System.AttributeTargets.Class | System.AttributeTargets.Struct | System.AttributeTargets.Enum)]").map_err(fmt_err)?;
    writeln!(
        out,
        "    public sealed class ValueTypeAttribute : System.Attribute"
    )
    .map_err(fmt_err)?;
    writeln!(out, "    {{").map_err(fmt_err)?;
    writeln!(out, "        public string FullyQualifiedName {{ get; }}").map_err(fmt_err)?;
    writeln!(out, "        public bool IsLocal {{ get; init; }} = false;").map_err(fmt_err)?;
    writeln!(
        out,
        "        public bool IsVariableSize {{ get; init; }} = false;"
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "        public ValueTypeAttribute(string fullyQualifiedName)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "        {{").map_err(fmt_err)?;
    writeln!(out, "            FullyQualifiedName = fullyQualifiedName;").map_err(fmt_err)?;
    writeln!(out, "        }}").map_err(fmt_err)?;
    writeln!(out, "    }}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    writeln!(
        out,
        "    /// Per-type traits helper analogous to CORBA::traits<T> in C++."
    )
    .map_err(fmt_err)?;
    writeln!(out, "    public static class Traits").map_err(fmt_err)?;
    writeln!(out, "    {{").map_err(fmt_err)?;
    for entry in &emitted {
        emit_traits_entry(out, entry)?;
    }
    writeln!(out, "    }}").map_err(fmt_err)?;
    writeln!(out, "}} // namespace Corba").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

#[derive(Debug)]
struct TraitEntry {
    cs_qualified: String,
    method_name: String,
    variable_size: bool,
}

/// zerodds-lint: recursion-depth 32 (IDL-Module-Nesting)
fn collect_top_level(defs: &[Definition], scope: &mut Vec<String>, out: &mut Vec<TraitEntry>) {
    for d in defs {
        match d {
            Definition::Module(m) => collect_module(m, scope, out),
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                out.push(TraitEntry {
                    cs_qualified: qualified(scope, &s.name.text),
                    method_name: scoped_method(scope, &s.name.text),
                    variable_size: struct_is_variable(s),
                });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                out.push(TraitEntry {
                    cs_qualified: qualified(scope, &u.name.text),
                    method_name: scoped_method(scope, &u.name.text),
                    variable_size: union_is_variable(u),
                });
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Enum(e))) => {
                out.push(TraitEntry {
                    cs_qualified: qualified(scope, &e.name.text),
                    method_name: scoped_method(scope, &e.name.text),
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
        format!("global::{name}")
    } else {
        format!("global::{}.{name}", scope.join("."))
    }
}

/// Injectively flattens the module path + type name into a single C# method
/// identifier (F35/A35). A C# identifier cannot carry `.`, so the scope is
/// joined with `_`; to keep the mapping injective, each segment's own
/// underscores are doubled first. Thus `A::B::C` (`A_B_C`) never collides with
/// `module A { struct B_C }` (`A_B__C`) — the previous plain `join("_")` mapped
/// both to `A_B_C`. Global scope returns the bare name (existing output
/// unchanged).
fn scoped_method(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        return name.to_string();
    }
    let mut parts: Vec<String> = scope.iter().map(|s| s.replace('_', "__")).collect();
    parts.push(name.replace('_', "__"));
    parts.join("_")
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
        TypeSpec::Scoped(_) => true,
        TypeSpec::Any => true,
    }
}

fn emit_traits_entry(out: &mut String, entry: &TraitEntry) -> Result<(), CsGenError> {
    let q = &entry.cs_qualified;
    let m = &entry.method_name;
    writeln!(
        out,
        "        /// {q} — variable_size={var}.",
        var = entry.variable_size
    )
    .map_err(fmt_err)?;
    writeln!(out, "        public const string {m}_FullName = \"{q}\";").map_err(fmt_err)?;
    writeln!(
        out,
        "        public const bool {m}_IsVariableSize = {};",
        if entry.variable_size { "true" } else { "false" }
    )
    .map_err(fmt_err)?;
    writeln!(out, "        public const bool {m}_IsLocal = false;").map_err(fmt_err)?;
    Ok(())
}

fn fmt_err(_: std::fmt::Error) -> CsGenError {
    CsGenError::Internal("string formatting failed".into())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use crate::{CsGenOptions, generate_csharp_with_corba_traits};
    use zerodds_idl::config::ParserConfig;

    fn render(src: &str) -> String {
        let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
        let opts = CsGenOptions::default();
        generate_csharp_with_corba_traits(&ast, &opts).expect("gen")
    }

    #[test]
    fn empty_source_emits_no_traits_block() {
        let out = render("");
        assert!(!out.contains("namespace Corba"));
    }

    #[test]
    fn struct_emits_traits_constants() {
        let out = render("struct S { long x; };");
        assert!(out.contains("namespace Corba"));
        assert!(out.contains("public const string S_FullName = \"global::S\";"));
        assert!(out.contains("public const bool S_IsVariableSize = false;"));
        assert!(out.contains("public const bool S_IsLocal = false;"));
    }

    #[test]
    fn variable_size_struct_marked_correctly() {
        let out = render("struct S { string name; };");
        assert!(out.contains("public const bool S_IsVariableSize = true;"));
    }

    #[test]
    fn enum_marked_fixed_size() {
        let out = render("enum Color { RED, GREEN };");
        assert!(out.contains("public const string Color_FullName = \"global::Color\";"));
        assert!(out.contains("public const bool Color_IsVariableSize = false;"));
    }

    #[test]
    fn nested_module_qualifies() {
        let out = render("module A { module B { struct S { long x; }; }; };");
        assert!(out.contains("public const string A_B_S_FullName = \"global::A.B.S\";"));
    }

    #[test]
    fn value_type_attribute_emitted() {
        let out = render("struct S { long x; };");
        assert!(out.contains("public sealed class ValueTypeAttribute"));
        assert!(out.contains("public string FullyQualifiedName"));
    }

    #[test]
    fn union_with_string_branch_is_variable() {
        let out = render("union U switch (long) { case 1: string s; case 2: long n; };");
        assert!(out.contains("public const bool U_IsVariableSize = true;"));
    }

    #[test]
    fn sequence_member_marks_struct_variable() {
        let out = render("struct S { sequence<long> data; };");
        assert!(out.contains("public const bool S_IsVariableSize = true;"));
    }

    #[test]
    fn no_local_default_set_to_false() {
        let out = render("struct S { long x; };");
        assert!(out.contains("S_IsLocal = false"));
    }
}
