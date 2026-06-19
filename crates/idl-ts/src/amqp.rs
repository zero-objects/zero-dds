// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Spec §7.2.3 / §8.1.2 / §8.1.3 — AMQP bindings for IDL types in TypeScript.
//!
//! Appends a section with `toAmqpValue<TypeName>(v)` and
//! `toJsonString<TypeName>(v)` to the end of the generated `.ts` file.
//!
//! # Runtime surface
//!
//! The emitted calls expect a TypeScript runtime module
//! `@zerodds/amqp/codec` (a separate library crate). API sketch:
//!
//! ```ts
//! export interface Value { /* opaque */ }
//!
//! export class StructBuilder {
//!     constructor(typeName: string);
//!     fieldBool(name: string, v: boolean): this;
//!     fieldInt8(name: string, v: number): this;
//!     fieldInt16(name: string, v: number): this;
//!     fieldInt32(name: string, v: number): this;
//!     fieldInt64(name: string, v: bigint): this;
//!     fieldUint8(name: string, v: number): this;
//!     fieldUint16(name: string, v: number): this;
//!     fieldUint32(name: string, v: number): this;
//!     fieldUint64(name: string, v: bigint): this;
//!     fieldFloat(name: string, v: number): this;
//!     fieldDouble(name: string, v: number): this;
//!     fieldString(name: string, v: string): this;
//!     fieldValue(name: string, v: Value): this;
//!     build(): Value;
//! }
//!
//! export function makeUnionBody(disc: Value): Value;
//! export function makeUnionBody(disc: Value, branch: Value): Value;
//! export function boolValue(v: boolean): Value;
//! export function int32Value(v: number): Value;
//! /* ... more factories ... */
//! export function toJson(v: Value): string;
//! ```

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use zerodds_idl::ast::{
    CaseLabel, ConstrTypeDecl, Definition, FloatingType, IntegerType, ModuleDef, PrimitiveType,
    Specification, StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, UnionDcl, UnionDef,
};

use crate::{IdlTsError, eval_const_int, typespec_to_ts};

/// Appends an AMQP bindings block to the end of `out` if the spec
/// contains at least one top-level struct/union.
///
/// # Errors
/// `IdlTsError::Unsupported` aus `typespec_to_ts`-Fallthrough.
pub(crate) fn append_amqp_helpers(
    out: &mut String,
    spec: &Specification,
) -> Result<(), IdlTsError> {
    if !has_emittable_types(&spec.definitions) {
        return Ok(());
    }
    out.push_str("\n// Spec §7.2.3 / §8.1.2 / §8.1.3 — AMQP-Bindings.\n");
    out.push_str("import * as ddsAmqp from \"@zerodds/amqp/codec\";\n\n");
    walk(out, &spec.definitions, &mut Vec::new())?;
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
fn walk(out: &mut String, defs: &[Definition], scope: &mut Vec<String>) -> Result<(), IdlTsError> {
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
fn emit_module(out: &mut String, m: &ModuleDef, scope: &mut Vec<String>) -> Result<(), IdlTsError> {
    scope.push(m.name.text.clone());
    walk(out, &m.definitions, scope)?;
    scope.pop();
    Ok(())
}

fn qualified_ts_name(scope: &[String], name: &str) -> String {
    if scope.is_empty() {
        name.to_string()
    } else {
        format!("{}.{name}", scope.join("."))
    }
}

fn helper_suffix(scope: &[String], name: &str) -> String {
    let mut s = String::new();
    for p in scope {
        s.push_str(p);
        s.push('_');
    }
    s.push_str(name);
    s
}

// ---------------------------------------------------------------------------
// Struct
// ---------------------------------------------------------------------------

fn emit_struct_helpers(
    out: &mut String,
    scope: &[String],
    s: &StructDef,
) -> Result<(), IdlTsError> {
    let ts_ty = qualified_ts_name(scope, &s.name.text);
    let suffix = helper_suffix(scope, &s.name.text);
    let spec_ty = qualified_ts_name(scope, &s.name.text);

    out.push_str(&format!(
        "// {spec_ty} — Spec §8.1.3 composite + §8.1.2 JSON.\n",
    ));
    out.push_str(&format!(
        "export function toAmqpValue_{suffix}(v_: {ts_ty}): ddsAmqp.Value {{\n",
    ));
    out.push_str(&format!(
        "    const b_ = new ddsAmqp.StructBuilder(\"{spec_ty}\");\n",
    ));
    for m in &s.members {
        for decl in &m.declarators {
            let name = decl.name().text.clone();
            let getter = format!("v_.{name}");
            emit_field_call(out, &m.type_spec, &name, &getter, scope)?;
        }
    }
    out.push_str("    return b_.build();\n");
    out.push_str("}\n\n");

    // §8.1.2 JSON-Wrapper.
    out.push_str(&format!(
        "export function toJsonString_{suffix}(v_: {ts_ty}): string {{\n",
    ));
    out.push_str(&format!(
        "    return ddsAmqp.toJson(toAmqpValue_{suffix}(v_));\n",
    ));
    out.push_str("}\n\n");

    Ok(())
}

fn emit_field_call(
    out: &mut String,
    ts: &TypeSpec,
    field_name: &str,
    getter: &str,
    _scope: &[String],
) -> Result<(), IdlTsError> {
    if let Some(setter) = primitive_setter(ts) {
        out.push_str(&format!("    b_.{setter}(\"{field_name}\", {getter});\n",));
        return Ok(());
    }
    if matches!(ts, TypeSpec::String(_)) {
        out.push_str(&format!(
            "    b_.fieldString(\"{field_name}\", {getter});\n",
        ));
        return Ok(());
    }
    // Fallback: rekursiv via toAmqpValue_<TypeName>.
    let inner_suffix = match ts {
        TypeSpec::Scoped(s) => s
            .parts
            .iter()
            .map(|p| p.text.clone())
            .collect::<Vec<_>>()
            .join("_"),
        _ => "Unknown".to_string(),
    };
    out.push_str(&format!(
        "    b_.fieldValue(\"{field_name}\", toAmqpValue_{inner_suffix}({getter}));\n",
    ));
    Ok(())
}

fn primitive_setter(ts: &TypeSpec) -> Option<&'static str> {
    let TypeSpec::Primitive(p) = ts else {
        return None;
    };
    Some(prim_setter(*p))
}

fn prim_setter(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "fieldBool",
        PrimitiveType::Octet => "fieldUint8",
        PrimitiveType::Char => "fieldInt8",
        PrimitiveType::WideChar => "fieldUint16",
        PrimitiveType::Integer(i) => integer_setter(i),
        PrimitiveType::Floating(f) => floating_setter(f),
    }
}

fn integer_setter(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "fieldInt16",
        IntegerType::Long | IntegerType::Int32 => "fieldInt32",
        IntegerType::LongLong | IntegerType::Int64 => "fieldInt64",
        IntegerType::UShort | IntegerType::UInt16 => "fieldUint16",
        IntegerType::ULong | IntegerType::UInt32 => "fieldUint32",
        IntegerType::ULongLong | IntegerType::UInt64 => "fieldUint64",
        IntegerType::Int8 => "fieldInt8",
        IntegerType::UInt8 => "fieldUint8",
    }
}

fn floating_setter(f: FloatingType) -> &'static str {
    match f {
        FloatingType::Float => "fieldFloat",
        FloatingType::Double | FloatingType::LongDouble => "fieldDouble",
    }
}

// ---------------------------------------------------------------------------
// Union
// ---------------------------------------------------------------------------

fn emit_union_helpers(out: &mut String, scope: &[String], u: &UnionDef) -> Result<(), IdlTsError> {
    let ts_ty = qualified_ts_name(scope, &u.name.text);
    let suffix = helper_suffix(scope, &u.name.text);
    let spec_ty = ts_ty.clone();

    out.push_str(&format!(
        "// {spec_ty} — Spec §7.2.3 union (list of [disc, branch?]).\n",
    ));
    out.push_str(&format!(
        "export function toAmqpValue_{suffix}(u_: {ts_ty}): ddsAmqp.Value {{\n",
    ));

    let disc_factory = switch_disc_factory(&u.switch_type);

    let mut default_arm: Option<&zerodds_idl::ast::Case> = None;
    for c in &u.cases {
        let mut numeric_labels: Vec<String> = Vec::new();
        let mut is_default = false;
        for lbl in &c.labels {
            match lbl {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(expr) => {
                    if let Some(n) = eval_const_int(expr) {
                        numeric_labels.push(format!("{n}"));
                    }
                }
            }
        }
        if is_default {
            default_arm = Some(c);
            continue;
        }
        if numeric_labels.is_empty() {
            continue;
        }
        let field_name = c.element.declarator.name().text.clone();
        let cond = numeric_labels
            .iter()
            .map(|l| format!("u_.discriminator === {l}"))
            .collect::<Vec<_>>()
            .join(" || ");
        let disc_value = numeric_labels
            .first()
            .cloned()
            .unwrap_or_else(|| "0".into());
        let branch_call = branch_factory_call(&c.element.type_spec, &field_name);
        out.push_str(&format!("    if ({cond}) {{\n"));
        out.push_str(&format!(
            "        return ddsAmqp.makeUnionBody(ddsAmqp.{disc_factory}({disc_value}), {branch_call});\n",
        ));
        out.push_str("    }\n");
    }

    if let Some(c) = default_arm {
        // Default branch: reaches all discriminator values not hit
        // explicitly. `u_` is still in the broad union type here, so
        // TypeScript reads `u_.discriminator` without never-narrowing.
        let field_name = c.element.declarator.name().text.clone();
        let branch_call = branch_factory_call(&c.element.type_spec, &field_name);
        out.push_str(&format!(
            "    return ddsAmqp.makeUnionBody(ddsAmqp.{disc_factory}((u_ as {{ discriminator: number }}).discriminator), {branch_call});\n",
        ));
    } else {
        // No default branch: TypeScript narrows `u_` to `never` after
        // the exhaustive case checks. Hence no `u_.discriminator` access
        // in the fallback — we throw instead, which at runtime means
        // "unreachable". TS-3 finding 6 (fixed 2026-05-01).
        out.push_str(
            "    throw new Error(\"toAmqpValue: union value did not match any explicit case\");\n",
        );
    }
    out.push_str("}\n\n");

    // §8.1.2 JSON-Wrapper.
    out.push_str(&format!(
        "export function toJsonString_{suffix}(u_: {ts_ty}): string {{\n",
    ));
    out.push_str(&format!(
        "    return ddsAmqp.toJson(toAmqpValue_{suffix}(u_));\n",
    ));
    out.push_str("}\n\n");

    Ok(())
}

fn switch_disc_factory(s: &SwitchTypeSpec) -> &'static str {
    match s {
        SwitchTypeSpec::Boolean => "boolValue",
        SwitchTypeSpec::Char => "int8Value",
        SwitchTypeSpec::Octet => "uint8Value",
        SwitchTypeSpec::Integer(i) => integer_factory(*i),
        SwitchTypeSpec::Scoped(_) => "int32Value",
    }
}

fn integer_factory(i: IntegerType) -> &'static str {
    match i {
        IntegerType::Short | IntegerType::Int16 => "int16Value",
        IntegerType::Long | IntegerType::Int32 => "int32Value",
        IntegerType::LongLong | IntegerType::Int64 => "int64Value",
        IntegerType::UShort | IntegerType::UInt16 => "uint16Value",
        IntegerType::ULong | IntegerType::UInt32 => "uint32Value",
        IntegerType::ULongLong | IntegerType::UInt64 => "uint64Value",
        IntegerType::Int8 => "int8Value",
        IntegerType::UInt8 => "uint8Value",
    }
}

fn branch_factory_call(ts: &TypeSpec, field_name: &str) -> String {
    let getter = format!("u_.{field_name}");
    if let TypeSpec::Primitive(p) = ts {
        let factory = primitive_factory(*p);
        return format!("ddsAmqp.{factory}({getter})");
    }
    if matches!(ts, TypeSpec::String(_)) {
        return format!("ddsAmqp.stringValue({getter})");
    }
    let inner_suffix = match ts {
        TypeSpec::Scoped(s) => s
            .parts
            .iter()
            .map(|p| p.text.clone())
            .collect::<Vec<_>>()
            .join("_"),
        _ => "Unknown".to_string(),
    };
    format!("toAmqpValue_{inner_suffix}({getter})")
}

fn primitive_factory(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "boolValue",
        PrimitiveType::Octet => "uint8Value",
        PrimitiveType::Char => "int8Value",
        PrimitiveType::WideChar => "uint16Value",
        PrimitiveType::Integer(i) => integer_factory(i),
        PrimitiveType::Floating(FloatingType::Float) => "floatValue",
        PrimitiveType::Floating(FloatingType::Double) => "doubleValue",
        PrimitiveType::Floating(FloatingType::LongDouble) => "doubleValue",
    }
}

// Re-export for the unused-imports diagnostic in the code path.
const _: fn(&TypeSpec) -> Result<String, IdlTsError> = typespec_to_ts;

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::generate_ts_source_with_amqp;
    use zerodds_idl::config::ParserConfig;

    fn gen_amqp(src: &str) -> String {
        let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
        generate_ts_source_with_amqp(&ast).expect("gen")
    }

    #[test]
    fn struct_emits_to_amqp_value_function() {
        let ts = gen_amqp("struct S { long x; };");
        assert!(ts.contains("export function toAmqpValue_S(v_: S): ddsAmqp.Value"));
        assert!(ts.contains("new ddsAmqp.StructBuilder(\"S\")"));
        assert!(ts.contains("b_.fieldInt32(\"x\", v_.x);"));
        assert!(ts.contains("return b_.build();"));
    }

    #[test]
    fn struct_emits_to_json_wrapper() {
        let ts = gen_amqp("struct S { long x; };");
        assert!(ts.contains("export function toJsonString_S(v_: S): string"));
        assert!(ts.contains("ddsAmqp.toJson(toAmqpValue_S(v_))"));
    }

    #[test]
    fn primitive_field_setters_correct() {
        let ts = gen_amqp(
            "struct S { boolean b; octet o; short s; long l; long long ll; \
             float f; double d; };",
        );
        assert!(ts.contains("b_.fieldBool(\"b\", v_.b);"));
        assert!(ts.contains("b_.fieldUint8(\"o\", v_.o);"));
        assert!(ts.contains("b_.fieldInt16(\"s\", v_.s);"));
        assert!(ts.contains("b_.fieldInt32(\"l\", v_.l);"));
        assert!(ts.contains("b_.fieldInt64(\"ll\", v_.ll);"));
        assert!(ts.contains("b_.fieldFloat(\"f\", v_.f);"));
        assert!(ts.contains("b_.fieldDouble(\"d\", v_.d);"));
    }

    #[test]
    fn string_field_uses_field_string() {
        let ts = gen_amqp("struct S { string name; };");
        assert!(ts.contains("b_.fieldString(\"name\", v_.name);"));
    }

    #[test]
    fn nested_struct_recurses() {
        let ts = gen_amqp("struct Inner { long x; }; struct Outer { Inner i; };");
        assert!(ts.contains("b_.fieldValue(\"i\", toAmqpValue_Inner(v_.i));"));
    }

    #[test]
    fn module_qualifies_spec_type_name() {
        let ts = gen_amqp("module M { struct S { long x; }; };");
        assert!(ts.contains("export function toAmqpValue_M_S(v_: M.S)"));
        assert!(ts.contains("new ddsAmqp.StructBuilder(\"M.S\")"));
    }

    #[test]
    fn nested_modules_qualify_spec_type_name() {
        let ts = gen_amqp("module A { module B { struct S { long x; }; }; };");
        assert!(ts.contains("export function toAmqpValue_A_B_S(v_: A.B.S)"));
        assert!(ts.contains("new ddsAmqp.StructBuilder(\"A.B.S\")"));
    }

    #[test]
    fn union_emits_make_union_body_calls() {
        let ts = gen_amqp(
            "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
        );
        assert!(ts.contains("export function toAmqpValue_U(u_: U)"));
        assert!(ts.contains("u_.discriminator === 1"));
        assert!(ts.contains("u_.discriminator === 2"));
        assert!(ts.contains("ddsAmqp.makeUnionBody("));
        assert!(ts.contains("ddsAmqp.int32Value(1)"));
        assert!(ts.contains("ddsAmqp.int32Value(2)"));
        // TS-3 finding 6 (fixed 2026-05-01): the default path casts via
        // the broad discriminator type, not the `u_.discriminator` typed
        // to `never` after exhaustive narrowing.
        assert!(ts.contains("(u_ as { discriminator: number }).discriminator"));
        assert!(ts.contains("ddsAmqp.int32Value(u_.a)"));
        assert!(ts.contains("ddsAmqp.doubleValue(u_.b)"));
        assert!(ts.contains("ddsAmqp.uint8Value(u_.c)"));
    }

    #[test]
    fn union_disc_short_uses_int16_factory() {
        let ts = gen_amqp("union U switch (short) { case 1: long a; };");
        assert!(ts.contains("ddsAmqp.int16Value(1)"));
    }

    #[test]
    fn opt_in_disabled_by_default() {
        let ast =
            zerodds_idl::parse("struct S { long x; };", &ParserConfig::default()).expect("parse");
        let ts = crate::generate_ts_source(&ast).expect("gen");
        assert!(!ts.contains("toAmqpValue_S"));
        assert!(!ts.contains("@zerodds/amqp/codec"));
    }

    #[test]
    fn empty_spec_emits_no_amqp_section() {
        let ts = gen_amqp("");
        assert!(!ts.contains("toAmqpValue_"));
        assert!(!ts.contains("@zerodds/amqp/codec"));
    }

    #[test]
    fn helpers_appear_after_main_definitions() {
        let ts = gen_amqp("struct S { long x; };");
        let main_pos = ts.find("export interface S").unwrap_or(0);
        let codec_pos = ts.find("toAmqpValue_S").unwrap_or(0);
        assert!(codec_pos > main_pos, "codec must come after main type");
    }

    #[test]
    fn json_helper_for_union_too() {
        let ts = gen_amqp("union U switch (long) { case 1: long a; };");
        assert!(ts.contains("export function toJsonString_U(u_: U)"));
    }

    #[test]
    fn union_without_default_emits_unreachable_throw() {
        // TS-3 finding 6 (fixed 2026-05-01): with a missing default
        // branch, TypeScript narrows `u_` to `never` after exhaustive
        // case handling. The codegen throws at this point instead of
        // producing an unreachable `u_.discriminator` fallback.
        let ts = gen_amqp("union U switch (long) { case 1: long a; };");
        assert!(ts.contains("throw new Error("));
        assert!(ts.contains("union value did not match any explicit case"));
    }

    #[test]
    fn _qualified_ts_name_helper() {
        assert_eq!(qualified_ts_name(&[], "S"), "S");
        let scope = vec!["A".to_string(), "B".to_string()];
        assert_eq!(qualified_ts_name(&scope, "S"), "A.B.S");
    }

    #[test]
    fn _helper_suffix_uses_underscore_separator() {
        assert_eq!(helper_suffix(&[], "S"), "S");
        let scope = vec!["A".to_string(), "B".to_string()];
        assert_eq!(helper_suffix(&scope, "S"), "A_B_S");
    }
}
