// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! C5.4-c §7.2.3 / §8.1.2 / §8.1.3 — AMQP bindings for IDL types in Java.
//!
//! Emits one `<TypeName>AmqpCodec.java` file per top-level struct/union
//! with static helpers:
//!
//! * `static org.zerodds.amqp.Value toAmqpValue(T v)`
//!   (Spec §8.1.3 AMQP-native composite).
//! * `static String toJsonString(T v)`
//!   (Spec §8.1.2 JSON type-reflection).
//!
//! For unions the emitted function additionally calls
//! `org.zerodds.amqp.Codec.makeUnionBody(disc, branch)` (Spec §7.2.3).
//!
//! # Runtime Surface
//!
//! The emitted calls expect a Java runtime package
//! `org.zerodds.amqp` (separate library crate). API sketch:
//!
//! ```java
//! package org.zerodds.amqp;
//!
//! public final class Value { /* opaque */ }
//!
//! public final class StructBuilder {
//!     public StructBuilder(String typeName);
//!     public StructBuilder fieldBool(String name, boolean v);
//!     public StructBuilder fieldInt8(String name, byte v);
//!     public StructBuilder fieldInt16(String name, short v);
//!     public StructBuilder fieldInt32(String name, int v);
//!     public StructBuilder fieldInt64(String name, long v);
//!     public StructBuilder fieldUint8(String name, byte v);
//!     public StructBuilder fieldUint16(String name, char v);
//!     public StructBuilder fieldUint32(String name, int v);
//!     public StructBuilder fieldUint64(String name, long v);
//!     public StructBuilder fieldFloat(String name, float v);
//!     public StructBuilder fieldDouble(String name, double v);
//!     public StructBuilder fieldString(String name, String v);
//!     public StructBuilder fieldValue(String name, Value v);
//!     public Value build();
//! }
//!
//! public final class Codec {
//!     public static Value makeUnionBody(Value discriminator);
//!     public static Value makeUnionBody(Value discriminator, Value branch);
//!     public static Value boolValue(boolean v);
//!     public static Value int32Value(int v);
//!     /* ... further factory methods ... */
//!     public static String toJson(Value v);
//! }
//! ```

use std::fmt::Write;

use zerodds_idl::ast::{
    CaseLabel, ConstrTypeDecl, Definition, FloatingType, IntegerType, PrimitiveType, Specification,
    StructDcl, StructDef, SwitchTypeSpec, TypeDecl, TypeSpec, UnionDcl, UnionDef,
};

use crate::JavaGenOptions;
use crate::emitter::{
    ImportSet, JavaFile, capitalize, const_expr_to_java, switch_type_to_java, type_for_declarator,
    wrap_compilation_unit,
};
use crate::error::JavaGenError;
use crate::keywords::sanitize_identifier;

/// Emits one `<TypeName>AmqpCodec.java` file per top-level struct/union.
///
/// # Errors
/// `JavaGenError::*` from the type-mapping helpers.
pub(crate) fn emit_amqp_codec_files(
    spec: &Specification,
    opts: &JavaGenOptions,
) -> Result<Vec<JavaFile>, JavaGenError> {
    let pkg = sanitize_package(&opts.root_package);
    let mut files: Vec<JavaFile> = Vec::new();
    walk(&spec.definitions, &pkg, &mut files)?;
    Ok(files)
}

fn sanitize_package(p: &str) -> String {
    p.split('.')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(".")
}

/// zerodds-lint: recursion-depth 32 (IDL-Module-Nesting)
fn walk(defs: &[Definition], pkg: &str, files: &mut Vec<JavaFile>) -> Result<(), JavaGenError> {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let inner_pkg = if pkg.is_empty() {
                    lower(&m.name.text)
                } else {
                    format!("{}.{}", pkg, lower(&m.name.text))
                };
                walk(&m.definitions, &inner_pkg, files)?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                files.push(emit_struct_codec(s, pkg)?);
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Union(UnionDcl::Def(u)))) => {
                files.push(emit_union_codec(u, pkg)?);
            }
            _ => {}
        }
    }
    Ok(())
}

fn lower(s: &str) -> String {
    s.to_lowercase()
}

fn fqn(pkg: &str, class: &str) -> String {
    if pkg.is_empty() {
        class.to_string()
    } else {
        format!("{pkg}.{class}")
    }
}

// ---------------------------------------------------------------------------
// Struct
// ---------------------------------------------------------------------------

fn emit_struct_codec(s: &StructDef, pkg: &str) -> Result<JavaFile, JavaGenError> {
    let class = sanitize_identifier(&s.name.text)?;
    let codec_class = format!("{class}AmqpCodec");
    let spec_ty = fqn(pkg, &class);

    let mut body = String::new();
    writeln!(body, "// Spec §7.2.3 / §8.1.2 / §8.1.3 — AMQP-Bindings.").map_err(fmt_err)?;
    writeln!(body, "public final class {codec_class} {{").map_err(fmt_err)?;
    writeln!(body, "    private {codec_class}() {{}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;
    writeln!(
        body,
        "    public static org.zerodds.amqp.Value toAmqpValue({class} v_) {{",
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "        org.zerodds.amqp.StructBuilder b_ = new org.zerodds.amqp.StructBuilder(\"{spec_ty}\");",
    )
    .map_err(fmt_err)?;
    for m in &s.members {
        for decl in &m.declarators {
            let name = sanitize_identifier(&decl.name().text)?;
            let getter = format!("v_.{name}()");
            emit_field_call(&mut body, &m.type_spec, &name, &getter)?;
            // Validate type can be mapped (catches unsupported constructs).
            let _ = type_for_declarator(&m.type_spec, decl)?;
        }
    }
    writeln!(body, "        return b_.build();").map_err(fmt_err)?;
    writeln!(body, "    }}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;
    writeln!(body, "    public static String toJsonString({class} v_) {{",).map_err(fmt_err)?;
    writeln!(
        body,
        "        return org.zerodds.amqp.Codec.toJson(toAmqpValue(v_));",
    )
    .map_err(fmt_err)?;
    writeln!(body, "    }}").map_err(fmt_err)?;
    writeln!(body, "}}").map_err(fmt_err)?;

    let source = wrap_compilation_unit(pkg, &ImportSet::default(), &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: codec_class,
        source,
    })
}

fn emit_field_call(
    out: &mut String,
    ts: &TypeSpec,
    field_name: &str,
    getter: &str,
) -> Result<(), JavaGenError> {
    if let Some(setter) = primitive_setter_name(ts) {
        writeln!(out, "        b_.{setter}(\"{field_name}\", {getter});",).map_err(fmt_err)?;
        return Ok(());
    }
    if matches!(ts, TypeSpec::String(_)) {
        writeln!(out, "        b_.fieldString(\"{field_name}\", {getter});",).map_err(fmt_err)?;
        return Ok(());
    }
    // Fallback: recurse via <Type>AmqpCodec.toAmqpValue.
    let inner_ty = match ts {
        TypeSpec::Scoped(s) => s
            .parts
            .last()
            .map(|p| p.text.clone())
            .unwrap_or_else(|| "Object".to_string()),
        _ => "Object".to_string(),
    };
    writeln!(
        out,
        "        b_.fieldValue(\"{field_name}\", {inner_ty}AmqpCodec.toAmqpValue({getter}));",
    )
    .map_err(fmt_err)?;
    Ok(())
}

fn primitive_setter_name(ts: &TypeSpec) -> Option<&'static str> {
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

fn emit_union_codec(u: &UnionDef, pkg: &str) -> Result<JavaFile, JavaGenError> {
    let class = sanitize_identifier(&u.name.text)?;
    let codec_class = format!("{class}AmqpCodec");
    let _disc_ty = switch_type_to_java(&u.switch_type)?;

    let mut body = String::new();
    writeln!(body, "// Spec §7.2.3 — Union as AMQP list [disc, branch?].").map_err(fmt_err)?;
    writeln!(body, "public final class {codec_class} {{").map_err(fmt_err)?;
    writeln!(body, "    private {codec_class}() {{}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;
    writeln!(
        body,
        "    public static org.zerodds.amqp.Value toAmqpValue({class} u_) {{",
    )
    .map_err(fmt_err)?;

    // Per case: instanceof check + makeUnionBody call with IDL discriminator.
    let mut default_arm: Option<&zerodds_idl::ast::Case> = None;
    for c in &u.cases {
        let mut numeric_labels: Vec<String> = Vec::new();
        let mut is_default = false;
        for lbl in &c.labels {
            match lbl {
                CaseLabel::Default => is_default = true,
                CaseLabel::Value(expr) => numeric_labels.push(const_expr_to_java(expr)),
            }
        }
        if is_default {
            default_arm = Some(c);
            continue;
        }
        let field_name = sanitize_identifier(&c.element.declarator.name().text)?;
        let record_name = capitalize(&field_name);
        let disc_call = disc_factory_call(&u.switch_type, numeric_labels.first());
        let branch_call = branch_factory_call(&c.element.type_spec, &field_name);
        writeln!(
            body,
            "        if (u_ instanceof {class}.{record_name} rec_) {{",
        )
        .map_err(fmt_err)?;
        writeln!(
            body,
            "            return org.zerodds.amqp.Codec.makeUnionBody({disc_call}, {branch_call});",
        )
        .map_err(fmt_err)?;
        writeln!(body, "        }}").map_err(fmt_err)?;
    }

    if let Some(c) = default_arm {
        let field_name = sanitize_identifier(&c.element.declarator.name().text)?;
        let record_name = capitalize(&field_name);
        let disc_call = "org.zerodds.amqp.Codec.int32Value(0)".to_string();
        let branch_call = branch_factory_call(&c.element.type_spec, &field_name);
        writeln!(
            body,
            "        if (u_ instanceof {class}.{record_name} rec_) {{",
        )
        .map_err(fmt_err)?;
        writeln!(
            body,
            "            return org.zerodds.amqp.Codec.makeUnionBody({disc_call}, {branch_call});",
        )
        .map_err(fmt_err)?;
        writeln!(body, "        }}").map_err(fmt_err)?;
    }

    // Safety fallback: empty disc-only body (should never be reached).
    writeln!(
        body,
        "        return org.zerodds.amqp.Codec.makeUnionBody(org.zerodds.amqp.Codec.int32Value(0));",
    )
    .map_err(fmt_err)?;
    writeln!(body, "    }}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;
    writeln!(body, "    public static String toJsonString({class} u_) {{",).map_err(fmt_err)?;
    writeln!(
        body,
        "        return org.zerodds.amqp.Codec.toJson(toAmqpValue(u_));",
    )
    .map_err(fmt_err)?;
    writeln!(body, "    }}").map_err(fmt_err)?;
    writeln!(body, "}}").map_err(fmt_err)?;

    let source = wrap_compilation_unit(pkg, &ImportSet::default(), &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: codec_class,
        source,
    })
}

fn disc_factory_call(s: &SwitchTypeSpec, label: Option<&String>) -> String {
    let factory = match s {
        SwitchTypeSpec::Boolean => "boolValue",
        SwitchTypeSpec::Char => "int8Value",
        SwitchTypeSpec::Octet => "uint8Value",
        SwitchTypeSpec::Integer(i) => integer_factory(*i),
        SwitchTypeSpec::Scoped(_) => "int32Value",
    };
    let val = label.cloned().unwrap_or_else(|| "0".to_string());
    format!("org.zerodds.amqp.Codec.{factory}({val})")
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
    let getter = format!("rec_.{field_name}()");
    if let TypeSpec::Primitive(p) = ts {
        let factory = primitive_factory(*p);
        return format!("org.zerodds.amqp.Codec.{factory}({getter})");
    }
    if matches!(ts, TypeSpec::String(_)) {
        return format!("org.zerodds.amqp.Codec.stringValue({getter})");
    }
    let inner_ty = match ts {
        TypeSpec::Scoped(s) => s
            .parts
            .last()
            .map(|p| p.text.clone())
            .unwrap_or_else(|| "Object".to_string()),
        _ => "Object".to_string(),
    };
    format!("{inner_ty}AmqpCodec.toAmqpValue({getter})")
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

fn fmt_err(_: std::fmt::Error) -> JavaGenError {
    JavaGenError::Internal("fmt::Write into String failed".into())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::generate_java_files_with_amqp;
    use zerodds_idl::config::ParserConfig;

    fn gen_amqp(src: &str) -> Vec<JavaFile> {
        let ast = zerodds_idl::parse(src, &ParserConfig::default()).expect("parse");
        let opts = JavaGenOptions {
            emit_amqp_helpers: true,
            ..Default::default()
        };
        generate_java_files_with_amqp(&ast, &opts).expect("gen")
    }

    fn find_codec(files: &[JavaFile], class: &str) -> JavaFile {
        files
            .iter()
            .find(|f| f.class_name == class)
            .cloned()
            .unwrap_or_else(|| panic!("missing {class}"))
    }

    #[test]
    fn struct_emits_amqp_codec_file() {
        let files = gen_amqp("struct S { long x; };");
        let codec = find_codec(&files, "SAmqpCodec");
        assert!(codec.source.contains("public final class SAmqpCodec"));
        assert!(
            codec
                .source
                .contains("public static org.zerodds.amqp.Value toAmqpValue(S v_)")
        );
        assert!(
            codec
                .source
                .contains("new org.zerodds.amqp.StructBuilder(\"S\")")
        );
        assert!(codec.source.contains("b_.fieldInt32(\"x\", v_.x())"));
        assert!(codec.source.contains("return b_.build();"));
    }

    #[test]
    fn struct_emits_to_json_helper() {
        let files = gen_amqp("struct S { long x; };");
        let codec = find_codec(&files, "SAmqpCodec");
        assert!(
            codec
                .source
                .contains("public static String toJsonString(S v_)")
        );
        assert!(
            codec
                .source
                .contains("org.zerodds.amqp.Codec.toJson(toAmqpValue(v_))")
        );
    }

    #[test]
    fn primitive_field_setters_correct() {
        let files = gen_amqp(
            "struct S { boolean b; octet o; short s; long l; long long ll; \
             float f; double d; };",
        );
        let codec = find_codec(&files, "SAmqpCodec");
        let src = &codec.source;
        assert!(src.contains("b_.fieldBool(\"b\", v_.b())"));
        assert!(src.contains("b_.fieldUint8(\"o\", v_.o())"));
        assert!(src.contains("b_.fieldInt16(\"s\", v_.s())"));
        assert!(src.contains("b_.fieldInt32(\"l\", v_.l())"));
        assert!(src.contains("b_.fieldInt64(\"ll\", v_.ll())"));
        assert!(src.contains("b_.fieldFloat(\"f\", v_.f())"));
        assert!(src.contains("b_.fieldDouble(\"d\", v_.d())"));
    }

    #[test]
    fn string_field_uses_field_string() {
        let files = gen_amqp("struct S { string name; };");
        let codec = find_codec(&files, "SAmqpCodec");
        assert!(codec.source.contains("b_.fieldString(\"name\", v_.name())"));
    }

    #[test]
    fn module_qualifies_spec_type_name() {
        let files = gen_amqp("module M { struct S { long x; }; };");
        let codec = find_codec(&files, "SAmqpCodec");
        assert!(codec.source.contains("package m;"));
        assert!(
            codec
                .source
                .contains("new org.zerodds.amqp.StructBuilder(\"m.S\")")
        );
    }

    #[test]
    fn nested_modules_qualify_spec_type_name() {
        let files = gen_amqp("module A { module B { struct S { long x; }; }; };");
        let codec = find_codec(&files, "SAmqpCodec");
        assert!(codec.source.contains("package a.b;"));
        assert!(
            codec
                .source
                .contains("new org.zerodds.amqp.StructBuilder(\"a.b.S\")")
        );
    }

    #[test]
    fn union_emits_codec_with_make_union_body_calls() {
        let files = gen_amqp(
            "union U switch (long) { case 1: long a; case 2: double b; default: octet c; };",
        );
        let codec = find_codec(&files, "UAmqpCodec");
        let src = &codec.source;
        assert!(src.contains("public final class UAmqpCodec"));
        assert!(src.contains("if (u_ instanceof U.A rec_)"));
        assert!(src.contains("if (u_ instanceof U.B rec_)"));
        assert!(src.contains("if (u_ instanceof U.C rec_)"));
        assert!(src.contains("makeUnionBody(org.zerodds.amqp.Codec.int32Value(1)"));
        assert!(src.contains("makeUnionBody(org.zerodds.amqp.Codec.int32Value(2)"));
        assert!(src.contains("rec_.a()"));
        assert!(src.contains("rec_.b()"));
        assert!(src.contains("rec_.c()"));
    }

    #[test]
    fn union_disc_short_uses_int16_factory() {
        let files = gen_amqp("union U switch (short) { case 1: long a; };");
        let codec = find_codec(&files, "UAmqpCodec");
        assert!(
            codec
                .source
                .contains("org.zerodds.amqp.Codec.int16Value(1)")
        );
    }

    #[test]
    fn opt_in_disabled_by_default() {
        let ast =
            zerodds_idl::parse("struct S { long x; };", &ParserConfig::default()).expect("parse");
        let files = crate::generate_java_files(&ast, &JavaGenOptions::default()).expect("gen");
        assert!(files.iter().all(|f| !f.class_name.ends_with("AmqpCodec")));
    }

    #[test]
    fn empty_spec_emits_no_amqp_files() {
        let files = gen_amqp("");
        assert!(files.iter().all(|f| !f.class_name.ends_with("AmqpCodec")));
    }

    #[test]
    fn nested_struct_field_recurses_via_codec() {
        let files = gen_amqp("struct Inner { long x; }; struct Outer { Inner i; };");
        let codec = find_codec(&files, "OuterAmqpCodec");
        assert!(codec.source.contains("InnerAmqpCodec.toAmqpValue(v_.i())"));
    }

    #[test]
    fn json_helper_for_union_too() {
        let files = gen_amqp("union U switch (long) { case 1: long a; };");
        let codec = find_codec(&files, "UAmqpCodec");
        assert!(
            codec
                .source
                .contains("public static String toJsonString(U u_)")
        );
    }

    #[test]
    fn codec_files_appear_alongside_main_files() {
        let files = gen_amqp("struct S { long x; };");
        // There should be both S.java and SAmqpCodec.java.
        assert!(files.iter().any(|f| f.class_name == "S"));
        assert!(files.iter().any(|f| f.class_name == "SAmqpCodec"));
    }
}
