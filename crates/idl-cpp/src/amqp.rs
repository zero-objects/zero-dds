// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C5.1-c §7.2.3 / §8.1.2 / §8.1.3 — AMQP-Bindings für IDL-Types.
//!
//! Emittiert pro Top-Level-Struct/Union eine inline Free-Function:
//!
//! * `to_amqp_value(const T&) -> ::zerodds::amqp::Value`
//!   (Spec §8.1.3 AMQP-Native Composite — `composite-from-described`).
//! * `to_json_string(const T&) -> std::string`
//!   (Spec §8.1.2 JSON Type-Reflection).
//!
//! Für Unions ruft die emittierte Funktion zusätzlich
//! `make_union_body(disc, branch)` (Spec §7.2.3) — der zentrale
//! Spec-Helper. Discriminator-Werte und Branch-Mapping kommen aus
//! der IDL-Quelle, nicht hardcoded.
//!
//! # Runtime-Surface
//!
//! Die emittierten Helper rufen in einen kleinen C++-Runtime-Header
//! `<zerodds/amqp/codec.hpp>` (separates Library-Crate, nicht Teil
//! dieses Codegens). API-Skizze:
//!
//! ```cpp
//! namespace zerodds::amqp {
//!     class Value;
//!     class StructBuilder {
//!     public:
//!         explicit StructBuilder(const char* type_name);
//!         StructBuilder& field_int8(const char*, int8_t);
//!         StructBuilder& field_int16(const char*, int16_t);
//!         StructBuilder& field_int32(const char*, int32_t);
//!         StructBuilder& field_int64(const char*, int64_t);
//!         StructBuilder& field_uint8(const char*, uint8_t);
//!         StructBuilder& field_uint16(const char*, uint16_t);
//!         StructBuilder& field_uint32(const char*, uint32_t);
//!         StructBuilder& field_uint64(const char*, uint64_t);
//!         StructBuilder& field_float(const char*, float);
//!         StructBuilder& field_double(const char*, double);
//!         StructBuilder& field_bool(const char*, bool);
//!         StructBuilder& field_string(const char*, const std::string&);
//!         StructBuilder& field_value(const char*, Value);
//!         Value build();
//!     };
//!     Value make_union_body(Value discriminator);
//!     Value make_union_body(Value discriminator, Value branch);
//!     Value int8_value(int8_t); /* ... weitere Faktories ... */
//!     std::string to_json(const Value&);
//! }
//! ```

use std::fmt::Write;

use zerodds_idl::ast::{
    CaseLabel, ConstrTypeDecl, Definition, FloatingType, IntegerType, ModuleDef, PrimitiveType,
    Specification, StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, UnionDcl, UnionDef,
};

use crate::emitter::{const_expr_to_cpp, declarator_name, switch_type_to_cpp, type_for_declarator};
use crate::error::CppGenError;

/// Emittiert AMQP-Helper für alle Top-Level-Struct/Union-Definitionen
/// der Spec.
///
/// # Errors
/// `CppGenError::Internal` bei `std::fmt::Write`-Fehler;
/// `CppGenError::*` aus den Type-Mapping-Helpern.
pub(crate) fn emit_amqp_helpers(out: &mut String, spec: &Specification) -> Result<(), CppGenError> {
    if !has_emittable_types(&spec.definitions) {
        return Ok(());
    }

    writeln!(out, "// Spec §7.2.3 / §8.1.2 / §8.1.3 — AMQP-Bindings.").map_err(fmt_err)?;
    writeln!(out, "// Runtime-Header: <zerodds/amqp/codec.hpp>").map_err(fmt_err)?;
    writeln!(out, "namespace zerodds_amqp_helpers {{").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    emit_definitions(out, &spec.definitions, &mut Vec::new())?;
    writeln!(out, "}} // namespace zerodds_amqp_helpers").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

/// zerodds-lint: recursion-depth 32 (IDL-Module-Nesting)
fn has_emittable_types(defs: &[Definition]) -> bool {
    for d in defs {
        match d {
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(_)))) => {
                return true;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(_)))) => {
                return true;
            }
            Definition::Module(m) => {
                if has_emittable_types(&m.definitions) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// zerodds-lint: recursion-depth 32 (IDL-Module-Nesting)
fn emit_definitions(
    out: &mut String,
    defs: &[Definition],
    scope: &mut Vec<String>,
) -> Result<(), CppGenError> {
    for d in defs {
        match d {
            Definition::Module(m) => emit_module(out, m, scope)?,
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                emit_struct_helpers(out, scope, s)?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                emit_union_helpers(out, scope, u)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// zerodds-lint: recursion-depth 32 (IDL-Module-Nesting)
fn emit_module(
    out: &mut String,
    m: &ModuleDef,
    scope: &mut Vec<String>,
) -> Result<(), CppGenError> {
    scope.push(m.name.text.clone());
    emit_definitions(out, &m.definitions, scope)?;
    scope.pop();
    Ok(())
}

/// Vollqualifizierter C++-Name eines Top-Level-Typs im aktuellen Scope.
fn qualified(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        format!("::{name}")
    } else {
        format!("::{}::{name}", scope.join("::"))
    }
}

/// Spec-Type-Name (für composite-symbol "dds:type:<scoped>").
fn spec_type_name(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{}::{name}", scope.join("::"))
    }
}

// ---------------------------------------------------------------------------
// §8.1.3 + §8.1.2 — Struct
// ---------------------------------------------------------------------------

fn emit_struct_helpers(
    out: &mut String,
    scope: &[String],
    s: &StructDef,
) -> Result<(), CppGenError> {
    let cpp_ty = qualified(scope, &s.name.text);
    let spec_ty = spec_type_name(scope, &s.name.text);

    writeln!(out, "// {spec_ty} — Spec §8.1.3 composite + §8.1.2 JSON.",).map_err(fmt_err)?;
    writeln!(
        out,
        "inline ::zerodds::amqp::Value to_amqp_value(const {cpp_ty}& v_) {{",
    )
    .map_err(fmt_err)?;
    writeln!(out, "    ::zerodds::amqp::StructBuilder b_(\"{spec_ty}\");",).map_err(fmt_err)?;

    for m in &s.members {
        for decl in &m.declarators {
            let name = declarator_name(decl).to_string();
            let getter = format!("v_.{name}()");
            emit_field_call(out, &m.type_spec, decl, &name, &getter)?;
        }
    }

    writeln!(out, "    return b_.build();").map_err(fmt_err)?;
    writeln!(out, "}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // §8.1.2 JSON-Wrapper.
    writeln!(
        out,
        "inline std::string to_json_string(const {cpp_ty}& v_) {{",
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    return ::zerodds::amqp::to_json(to_amqp_value(v_));",
    )
    .map_err(fmt_err)?;
    writeln!(out, "}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    Ok(())
}

/// Emittiert einen `b_.field_*` Call für ein Struct-Member.
fn emit_field_call(
    out: &mut String,
    ts: &TypeSpec,
    decl: &zerodds_idl::ast::Declarator,
    field_name: &str,
    getter: &str,
) -> Result<(), CppGenError> {
    let _ = type_for_declarator(ts, decl)?; // Validierung des Felds.
    if let Some(prim_setter) = primitive_field_setter(ts) {
        writeln!(out, "    b_.{prim_setter}(\"{field_name}\", {getter});",).map_err(fmt_err)?;
        return Ok(());
    }
    if matches!(ts, TypeSpec::String(_)) {
        writeln!(out, "    b_.field_string(\"{field_name}\", {getter});",).map_err(fmt_err)?;
        return Ok(());
    }
    // Fallback: rekursiv via to_amqp_value.
    writeln!(
        out,
        "    b_.field_value(\"{field_name}\", to_amqp_value({getter}));",
    )
    .map_err(fmt_err)?;
    Ok(())
}

fn primitive_field_setter(ts: &TypeSpec) -> Option<&'static str> {
    let TypeSpec::Primitive(p) = ts else {
        return None;
    };
    Some(primitive_setter_name(*p))
}

fn primitive_setter_name(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "field_bool",
        PrimitiveType::Octet => "field_uint8",
        PrimitiveType::Char => "field_int8",
        PrimitiveType::WideChar => "field_uint16",
        PrimitiveType::Integer(i) => integer_setter_name(i),
        PrimitiveType::Floating(f) => floating_setter_name(f),
    }
}

fn integer_setter_name(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "field_int16",
        IntegerType::Long | IntegerType::Int32 => "field_int32",
        IntegerType::LongLong | IntegerType::Int64 => "field_int64",
        IntegerType::UShort | IntegerType::UInt16 => "field_uint16",
        IntegerType::ULong | IntegerType::UInt32 => "field_uint32",
        IntegerType::ULongLong | IntegerType::UInt64 => "field_uint64",
        IntegerType::Int8 => "field_int8",
        IntegerType::UInt8 => "field_uint8",
    }
}

fn floating_setter_name(f: FloatingType) -> &'static str {
    match f {
        FloatingType::Float => "field_float",
        // long double → narrowed to double per §7.1.2.
        FloatingType::Double | FloatingType::LongDouble => "field_double",
    }
}

// ---------------------------------------------------------------------------
// §7.2.3 — Union
// ---------------------------------------------------------------------------

fn emit_union_helpers(out: &mut String, scope: &[String], u: &UnionDef) -> Result<(), CppGenError> {
    let cpp_ty = qualified(scope, &u.name.text);
    let spec_ty = spec_type_name(scope, &u.name.text);
    let disc_ty = switch_type_to_cpp(&u.switch_type)?;
    let disc_setter = switch_disc_factory(&u.switch_type);

    writeln!(
        out,
        "// {spec_ty} — Spec §7.2.3 union (list of [disc, branch?])."
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "inline ::zerodds::amqp::Value to_amqp_value(const {cpp_ty}& u_) {{",
    )
    .map_err(fmt_err)?;
    writeln!(out, "    {disc_ty} d_ = u_._d();").map_err(fmt_err)?;
    writeln!(
        out,
        "    ::zerodds::amqp::Value disc_ = ::zerodds::amqp::{disc_setter}(d_);",
    )
    .map_err(fmt_err)?;
    writeln!(out, "    switch (d_) {{").map_err(fmt_err)?;

    let mut default_arm: Option<&zerodds_idl::ast::Case> = None;

    for c in &u.cases {
        let mut numeric_labels: Vec<String> = Vec::new();
        let mut is_default = false;
        for label in &c.labels {
            match label {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(expr) => numeric_labels.push(const_expr_to_cpp(expr)),
            }
        }
        if is_default {
            default_arm = Some(c);
            continue;
        }
        let element_cpp_ty = type_for_declarator(&c.element.type_spec, &c.element.declarator)?;
        let branch_factory = branch_value_factory(&c.element.type_spec);
        for lbl in &numeric_labels {
            writeln!(out, "        case {lbl}:").map_err(fmt_err)?;
        }
        writeln!(
            out,
            "            return ::zerodds::amqp::make_union_body(disc_, {});",
            branch_call(branch_factory, &element_cpp_ty),
        )
        .map_err(fmt_err)?;
    }

    if let Some(c) = default_arm {
        let element_cpp_ty = type_for_declarator(&c.element.type_spec, &c.element.declarator)?;
        let branch_factory = branch_value_factory(&c.element.type_spec);
        writeln!(out, "        default:").map_err(fmt_err)?;
        writeln!(
            out,
            "            return ::zerodds::amqp::make_union_body(disc_, {});",
            branch_call(branch_factory, &element_cpp_ty),
        )
        .map_err(fmt_err)?;
    } else {
        // §7.2.3 — kein active branch → list mit nur disc.
        writeln!(out, "        default:").map_err(fmt_err)?;
        writeln!(
            out,
            "            return ::zerodds::amqp::make_union_body(disc_);"
        )
        .map_err(fmt_err)?;
    }

    writeln!(out, "    }}").map_err(fmt_err)?;
    writeln!(out, "}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // §8.1.2 JSON-Wrapper für Union.
    writeln!(
        out,
        "inline std::string to_json_string(const {cpp_ty}& u_) {{",
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "    return ::zerodds::amqp::to_json(to_amqp_value(u_));",
    )
    .map_err(fmt_err)?;
    writeln!(out, "}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    Ok(())
}

fn switch_disc_factory(s: &SwitchTypeSpec) -> &'static str {
    match s {
        SwitchTypeSpec::Boolean => "bool_value",
        SwitchTypeSpec::Char => "int8_value",
        SwitchTypeSpec::Octet => "uint8_value",
        SwitchTypeSpec::Integer(i) => integer_factory_name(*i),
        // Scoped (z.B. enum oder typedef): konservativ als int32.
        SwitchTypeSpec::Scoped(_) => "int32_value",
    }
}

fn integer_factory_name(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "int16_value",
        IntegerType::Long | IntegerType::Int32 => "int32_value",
        IntegerType::LongLong | IntegerType::Int64 => "int64_value",
        IntegerType::UShort | IntegerType::UInt16 => "uint16_value",
        IntegerType::ULong | IntegerType::UInt32 => "uint32_value",
        IntegerType::ULongLong | IntegerType::UInt64 => "uint64_value",
        IntegerType::Int8 => "int8_value",
        IntegerType::UInt8 => "uint8_value",
    }
}

fn branch_value_factory(ts: &TypeSpec) -> Option<&'static str> {
    let TypeSpec::Primitive(p) = ts else {
        return None;
    };
    Some(match p {
        PrimitiveType::Boolean => "bool_value",
        PrimitiveType::Octet => "uint8_value",
        PrimitiveType::Char => "int8_value",
        PrimitiveType::WideChar => "uint16_value",
        PrimitiveType::Integer(i) => integer_factory_name(*i),
        PrimitiveType::Floating(FloatingType::Float) => "float_value",
        PrimitiveType::Floating(FloatingType::Double) => "double_value",
        PrimitiveType::Floating(FloatingType::LongDouble) => "double_value",
    })
}

fn branch_call(factory: Option<&'static str>, cpp_ty: &str) -> String {
    if let Some(f) = factory {
        format!("::zerodds::amqp::{f}(std::get<{cpp_ty}>(u_.value()))")
    } else {
        // String/Sequence/Struct: rekursiv to_amqp_value.
        format!("to_amqp_value(std::get<{cpp_ty}>(u_.value()))")
    }
}

fn fmt_err(_: std::fmt::Error) -> CppGenError {
    CppGenError::Internal("fmt::Write into String failed".into())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::{CppGenOptions, generate_cpp_header_with_amqp};
    use zerodds_idl::config::ParserConfig;

    fn gen_amqp(src: &str) -> String {
        let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
        let opts = CppGenOptions {
            emit_amqp_helpers: true,
            ..Default::default()
        };
        generate_cpp_header_with_amqp(&ast, &opts).expect("gen")
    }

    #[test]
    fn struct_emits_to_amqp_value_function() {
        let cpp = gen_amqp("struct S { long x; };");
        assert!(cpp.contains("inline ::zerodds::amqp::Value to_amqp_value(const ::S& v_)"));
        assert!(cpp.contains("StructBuilder b_(\"S\")"));
        assert!(cpp.contains("b_.field_int32(\"x\", v_.x())"));
        assert!(cpp.contains("return b_.build();"));
    }

    #[test]
    fn struct_emits_to_json_wrapper() {
        let cpp = gen_amqp("struct S { long x; };");
        assert!(cpp.contains("inline std::string to_json_string(const ::S& v_)"));
        assert!(cpp.contains("::zerodds::amqp::to_json(to_amqp_value(v_))"));
    }

    #[test]
    fn primitive_field_types_map_correctly() {
        let cpp = gen_amqp(
            "struct S { boolean b; octet o; short s; long l; long long ll; \
             unsigned short us; unsigned long ul; unsigned long long ull; \
             float f; double d; };",
        );
        assert!(cpp.contains("b_.field_bool(\"b\", v_.b())"));
        assert!(cpp.contains("b_.field_uint8(\"o\", v_.o())"));
        assert!(cpp.contains("b_.field_int16(\"s\", v_.s())"));
        assert!(cpp.contains("b_.field_int32(\"l\", v_.l())"));
        assert!(cpp.contains("b_.field_int64(\"ll\", v_.ll())"));
        assert!(cpp.contains("b_.field_uint16(\"us\", v_.us())"));
        assert!(cpp.contains("b_.field_uint32(\"ul\", v_.ul())"));
        assert!(cpp.contains("b_.field_uint64(\"ull\", v_.ull())"));
        assert!(cpp.contains("b_.field_float(\"f\", v_.f())"));
        assert!(cpp.contains("b_.field_double(\"d\", v_.d())"));
    }

    #[test]
    fn string_field_uses_field_string() {
        let cpp = gen_amqp("struct S { string name; };");
        assert!(cpp.contains("b_.field_string(\"name\", v_.name())"));
    }

    #[test]
    fn nested_struct_recurses_via_field_value() {
        let cpp = gen_amqp("struct Inner { long x; }; struct Outer { Inner i; };");
        assert!(cpp.contains("b_.field_value(\"i\", to_amqp_value(v_.i()))"));
    }

    #[test]
    fn module_scoping_qualifies_type_name() {
        let cpp = gen_amqp("module M { struct S { long x; }; };");
        assert!(cpp.contains("to_amqp_value(const ::M::S& v_)"));
        assert!(cpp.contains("StructBuilder b_(\"M::S\")"));
    }

    #[test]
    fn nested_module_scoping_qualifies_type_name() {
        let cpp = gen_amqp("module A { module B { struct S { long x; }; }; };");
        assert!(cpp.contains("to_amqp_value(const ::A::B::S& v_)"));
        assert!(cpp.contains("StructBuilder b_(\"A::B::S\")"));
    }

    #[test]
    fn union_emits_make_union_body_call() {
        let cpp = gen_amqp(
            "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
        );
        assert!(cpp.contains("to_amqp_value(const ::U& u_)"));
        assert!(cpp.contains("::zerodds::amqp::int32_value(d_)"));
        assert!(cpp.contains("case 1:"));
        assert!(cpp.contains("case 2:"));
        assert!(cpp.contains("default:"));
        assert!(cpp.contains("make_union_body(disc_,"));
        assert!(cpp.contains("std::get<int32_t>(u_.value())"));
        assert!(cpp.contains("std::get<double>(u_.value())"));
        assert!(cpp.contains("std::get<uint8_t>(u_.value())"));
    }

    #[test]
    fn union_without_default_emits_disc_only_fallback() {
        let cpp = gen_amqp("union U switch (long) { case 1: long a; };");
        assert!(cpp.contains("default:"));
        assert!(cpp.contains("make_union_body(disc_)"));
    }

    #[test]
    fn union_disc_type_short_uses_int16_value_factory() {
        let cpp = gen_amqp("union U switch (short) { case 1: long a; };");
        assert!(cpp.contains("::zerodds::amqp::int16_value(d_)"));
    }

    #[test]
    fn union_disc_type_boolean_uses_bool_value_factory() {
        let cpp = gen_amqp("union U switch (boolean) { case TRUE: long a; case FALSE: short b; };");
        assert!(cpp.contains("::zerodds::amqp::bool_value(d_)"));
    }

    #[test]
    fn opt_in_disabled_by_default() {
        let ast =
            zerodds_idl::parse("struct S { long x; };", &ParserConfig::default()).expect("parse");
        let cpp = crate::generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen");
        assert!(!cpp.contains("to_amqp_value"));
        assert!(!cpp.contains("zerodds_amqp_helpers"));
    }

    #[test]
    fn empty_spec_emits_no_amqp_namespace() {
        let cpp = gen_amqp("");
        assert!(!cpp.contains("zerodds_amqp_helpers"));
    }

    #[test]
    fn helpers_appear_inside_dedicated_namespace() {
        let cpp = gen_amqp("struct S { long x; };");
        assert!(cpp.contains("namespace zerodds_amqp_helpers"));
        assert!(cpp.contains("} // namespace zerodds_amqp_helpers"));
    }

    #[test]
    fn header_contains_codec_runtime_reference() {
        let cpp = gen_amqp("struct S { long x; };");
        assert!(cpp.contains("Runtime-Header: <zerodds/amqp/codec.hpp>"));
    }

    #[test]
    fn json_wrapper_for_union_too() {
        let cpp = gen_amqp("union U switch (long) { case 1: long a; };");
        assert!(cpp.contains("to_json_string(const ::U& u_)"));
    }

    #[test]
    fn _scope_helper_qualifies_top_level() {
        assert_eq!(qualified(&[], "S"), "::S");
        let scope = vec!["A".to_string(), "B".to_string()];
        assert_eq!(qualified(&scope, "S"), "::A::B::S");
    }

    #[test]
    fn _spec_name_helper() {
        assert_eq!(spec_type_name(&[], "S"), "S");
        let scope = vec!["A".to_string()];
        assert_eq!(spec_type_name(&scope, "S"), "A::S");
    }
}
