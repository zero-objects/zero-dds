// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XCDR2 TypeSupport emission for the C# codegen.
//!
//! Spec: `zerodds-xcdr2-csharp-1.0` §3 / §4 / §5 / §6 / §7
//! + `zerodds-xcdr2-bindings-conformance-1.0` §6 (V-1..V-12).
//!
//! For each IDL `struct`, this module emits a `*TypeSupport` class that
//! implements `IDdsTopicType<T>` from `ZeroDDS.Cdr`. Encode/Decode
//! delegate to `Xcdr2Writer` / `Xcdr2Reader`; extensibility drives the
//! DHEADER/EMHEADER layout, and `@key` triggers an MD5 KeyHash via
//! `PlainCdr2BeKeyHolder`.

use std::fmt::Write;

use zerodds_idl::ast::{
    ConstExpr, Definition, IntegerType, PrimitiveType, ScopedName, Specification, StructDef,
    TypeSpec,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, lower_annotations, lower_type_annotations,
};

use crate::error::CsGenError;
use crate::keywords::escape_identifier;

/// Context info: the current module path for type-name emission and the
/// IDL struct name (without modules).
pub(crate) struct TsEmitContext<'a> {
    pub module_path: &'a [String],
    pub indent: &'a str,
    pub inner_indent: &'a str,
    pub deeper_indent: &'a str,
}

/// Returns `<Module1>::<Module2>::<Struct>` per spec §5 (type-name convention).
pub(crate) fn make_dds_type_name(module_path: &[String], struct_name: &str) -> String {
    if module_path.is_empty() {
        struct_name.to_string()
    } else {
        format!("{}::{}", module_path.join("::"), struct_name)
    }
}

/// Member info: computed wire characteristics of a struct member.
struct MemberInfo {
    /// Property name in PascalCase (matches the one in `emit_struct_member_property`).
    cs_prop: String,
    /// IDL source type (for Encode/Decode method selection).
    type_spec: TypeSpec,
    /// True if `@key`.
    is_key: bool,
    /// True if `@optional`.
    is_optional: bool,
    /// True if `@must_understand`.
    must_understand: bool,
    /// `@id(N)` if explicitly set; otherwise None (auto-index in mutable).
    explicit_id: Option<u32>,
}

fn pascal_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper_next = true;
    for c in s.chars() {
        if c == '_' {
            upper_next = true;
            continue;
        }
        if upper_next {
            for u in c.to_uppercase() {
                out.push(u);
            }
            upper_next = false;
        } else {
            out.push(c);
        }
    }
    if out.is_empty() {
        return s.to_string();
    }
    out
}

fn collect_member_info(s: &StructDef) -> Vec<MemberInfo> {
    let mut out = Vec::new();
    for m in &s.members {
        let mut is_key = false;
        let mut is_optional = false;
        let mut must_understand = false;
        let mut explicit_id: Option<u32> = None;
        if let Ok(lowered) = lower_annotations(&m.annotations) {
            for b in &lowered.builtins {
                match b {
                    BuiltinAnnotation::Key => is_key = true,
                    BuiltinAnnotation::Optional => is_optional = true,
                    BuiltinAnnotation::MustUnderstand => must_understand = true,
                    BuiltinAnnotation::Id(n) => explicit_id = Some(*n),
                    _ => {}
                }
            }
        }
        for decl in &m.declarators {
            let raw = &decl.name().text;
            let cs_prop = {
                let pas = pascal_case(raw);
                let escaped = escape_identifier(raw).unwrap_or_else(|_| raw.clone());
                if escaped == pas { escaped } else { pas }
            };
            out.push(MemberInfo {
                cs_prop,
                type_spec: m.type_spec.clone(),
                is_key,
                is_optional,
                must_understand,
                explicit_id,
            });
        }
        let _ = m; // silence unused on tail
    }
    out
}

/// Returns the IDL extensibility kind, default Appendable.
fn type_extensibility(s: &StructDef) -> ExtensibilityKind {
    if let Ok(lowered) = lower_type_annotations(&s.annotations) {
        for b in &lowered.builtins {
            if let Some(k) = match b {
                BuiltinAnnotation::Final => Some(ExtensibilityKind::Final),
                BuiltinAnnotation::Appendable => Some(ExtensibilityKind::Appendable),
                BuiltinAnnotation::Mutable => Some(ExtensibilityKind::Mutable),
                BuiltinAnnotation::Extensibility(k) => Some(*k),
                _ => None,
            } {
                return k;
            }
        }
    }
    ExtensibilityKind::Appendable
}

fn ext_to_cs(k: ExtensibilityKind) -> &'static str {
    match k {
        ExtensibilityKind::Final => "Final",
        ExtensibilityKind::Appendable => "Appendable",
        ExtensibilityKind::Mutable => "Mutable",
    }
}

fn fmt_err(_: core::fmt::Error) -> CsGenError {
    CsGenError::Internal("string formatting failed".into())
}

/// Main entry: emits a `*TypeSupport` class for `s`.
///
/// `module_path` is the list of enclosing modules (e.g.
/// `["Outer", "Inner"]` for `module Outer { module Inner { struct S }}`).
/// `true` if the XCDR2 codec can be generated for this type. `map`/`fixed`/`any`
/// have no wire codec in idl-csharp yet (cf. idl-java `typespec_supported`), so a
/// struct with such a member gets its data type but **no** TypeSupport — instead
/// of a codec that throws `XcdrException` at runtime.
/// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
fn typespec_xcdr2_codecable(t: &zerodds_idl::ast::TypeSpec) -> bool {
    use zerodds_idl::ast::TypeSpec;
    match t {
        TypeSpec::Map(_) | TypeSpec::Fixed(_) | TypeSpec::Any => false,
        TypeSpec::Sequence(s) => typespec_xcdr2_codecable(&s.elem),
        TypeSpec::Scoped(s) => match lookup_scoped_kind(s) {
            // Unions have no XCDR2 codec yet → a containing struct is gated.
            Some(ScopedKind::Union) => false,
            Some(ScopedKind::Typedef(inner)) => typespec_xcdr2_codecable(&inner),
            _ => true,
        },
        _ => true,
    }
}

/// Codec kind of a scoped type reference (for the nested XCDR2 dispatch).
#[derive(Clone)]
enum ScopedKind {
    /// struct — encoded via `<Name>TypeSupport.Instance.EncodeInto`/`DecodeFrom`.
    Struct,
    /// union — no XCDR2 codec yet; a struct containing one is gated.
    Union,
    /// enum — int32 representation.
    Enum,
    /// typedef — resolves to the aliased type.
    Typedef(TypeSpec),
}

std::thread_local! {
    /// Type-name → kind, built once per `generate_csharp` call (keyed by simple
    /// name). Lets the codec tell a nested struct from an enum/union and resolve
    /// typedefs. Without it every scoped member was encoded as empty bytes and
    /// decoded as `default!` (silent corruption).
    static TYPE_REG: core::cell::RefCell<std::collections::BTreeMap<String, ScopedKind>> =
        const { core::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// Populates [`TYPE_REG`] from the spec (recursing modules).
pub(crate) fn build_type_registry(spec: &Specification) {
    use zerodds_idl::ast::{ConstrTypeDecl, StructDcl, TypeDecl, UnionDcl};
    /// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
    fn walk(defs: &[Definition], reg: &mut std::collections::BTreeMap<String, ScopedKind>) {
        for def in defs {
            match def {
                Definition::Module(m) => walk(&m.definitions, reg),
                Definition::Type(TypeDecl::Constr(c)) => match c {
                    ConstrTypeDecl::Struct(StructDcl::Def(s)) => {
                        reg.insert(s.name.text.clone(), ScopedKind::Struct);
                    }
                    ConstrTypeDecl::Union(UnionDcl::Def(u)) => {
                        reg.insert(u.name.text.clone(), ScopedKind::Union);
                    }
                    ConstrTypeDecl::Enum(e) => {
                        reg.insert(e.name.text.clone(), ScopedKind::Enum);
                    }
                    _ => {}
                },
                Definition::Type(TypeDecl::Typedef(t)) => {
                    for d in &t.declarators {
                        reg.insert(
                            d.name().text.clone(),
                            ScopedKind::Typedef(t.type_spec.clone()),
                        );
                    }
                }
                _ => {}
            }
        }
    }
    let mut reg = std::collections::BTreeMap::new();
    walk(&spec.definitions, &mut reg);
    TYPE_REG.with(|r| *r.borrow_mut() = reg);
}

fn lookup_scoped_kind(s: &ScopedName) -> Option<ScopedKind> {
    let key = &s.parts.last()?.text;
    TYPE_REG.with(|r| r.borrow().get(key).cloned())
}

/// Dotted C# reference for a scoped name (`Module.Sub.Name`), each part escaped.
fn scoped_dotted_cs(s: &ScopedName) -> String {
    s.parts
        .iter()
        .map(|p| escape_identifier(&p.text).unwrap_or_else(|_| p.text.clone()))
        .collect::<Vec<_>>()
        .join(".")
}

/// Whether a struct's XCDR2 TypeSupport can be generated (all members codecable).
pub(crate) fn struct_xcdr2_codecable(s: &StructDef) -> bool {
    s.members
        .iter()
        .all(|m| typespec_xcdr2_codecable(&m.type_spec))
}

pub(crate) fn emit_typesupport_class(
    out: &mut String,
    ctx: &TsEmitContext<'_>,
    s: &StructDef,
) -> Result<(), CsGenError> {
    let struct_name = escape_identifier(&s.name.text)?;
    let ts_name = format!("{struct_name}TypeSupport");
    let ext = type_extensibility(s);
    let members = collect_member_info(s);
    let is_keyed = members.iter().any(|m| m.is_key);
    let dds_type_name = make_dds_type_name(ctx.module_path, &s.name.text);

    let ind = ctx.indent;
    let inner = ctx.inner_indent;
    let deeper = ctx.deeper_indent;

    writeln!(
        out,
        "{ind}public sealed class {ts_name} : IDdsTopicType<{struct_name}>"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{ind}{{").map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public static readonly {ts_name} Instance = new();"
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    writeln!(out, "{inner}public string TypeName => \"{dds_type_name}\";").map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public bool IsKeyed => {};",
        if is_keyed { "true" } else { "false" }
    )
    .map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public ExtensibilityKind Extensibility => ExtensibilityKind.{};",
        ext_to_cs(ext)
    )
    .map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // Encode(sample) - LE Default.
    writeln!(
        out,
        "{inner}public byte[] Encode({struct_name} sample) => Encode(sample, EndianMode.LittleEndian);"
    )
    .map_err(fmt_err)?;

    // Encode(sample, endian) — delegates to EncodeInto on a fresh writer.
    writeln!(out).map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public byte[] Encode({struct_name} sample, EndianMode endian)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    writeln!(out, "{deeper}var w = new Xcdr2Writer(endian);").map_err(fmt_err)?;
    writeln!(out, "{deeper}EncodeInto(w, sample);").map_err(fmt_err)?;
    writeln!(out, "{deeper}return w.ToArray();").map_err(fmt_err)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // EncodeInto(w, sample) — writes the body into a shared writer so this type
    // can be embedded as a nested member (alignment stays relative to the outer
    // CDR stream).
    writeln!(
        out,
        "{inner}public void EncodeInto(Xcdr2Writer w, {struct_name} sample)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    emit_encode_body(out, deeper, &members, ext)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // Decode(bytes) — delegates to DecodeFrom on a fresh reader.
    writeln!(
        out,
        "{inner}public {struct_name} Decode(ReadOnlySpan<byte> bytes)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    writeln!(
        out,
        "{deeper}var r = new Xcdr2Reader(bytes, EndianMode.LittleEndian);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{deeper}return DecodeFrom(r);").map_err(fmt_err)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // DecodeFrom(r) — reads the body from a shared reader (nested-member entry).
    writeln!(out, "{inner}public {struct_name} DecodeFrom(Xcdr2Reader r)").map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    emit_decode_body(out, deeper, &struct_name, &members, ext)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // KeyHash(sample).
    writeln!(out, "{inner}public byte[] KeyHash({struct_name} sample)").map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    if !is_keyed {
        writeln!(out, "{deeper}return new byte[16];").map_err(fmt_err)?;
    } else {
        emit_key_hash_body(out, deeper, &members)?;
    }
    writeln!(out, "{inner}}}").map_err(fmt_err)?;

    writeln!(out, "{ind}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;
    Ok(())
}

/// Writes the encode sequence for all members according to the extensibility.
fn emit_encode_body(
    out: &mut String,
    indent: &str,
    members: &[MemberInfo],
    ext: ExtensibilityKind,
) -> Result<(), CsGenError> {
    match ext {
        ExtensibilityKind::Final => {
            // Plain body, no DHEADER.
            for m in members {
                emit_encode_member_plain(out, indent, m)?;
            }
        }
        ExtensibilityKind::Appendable => {
            writeln!(out, "{indent}using (var __scope = w.BeginAppendable())").map_err(fmt_err)?;
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            let deeper = format!("{indent}    ");
            for m in members {
                emit_encode_member_plain(out, &deeper, m)?;
            }
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
        }
        ExtensibilityKind::Mutable => {
            writeln!(out, "{indent}using (var __scope = w.BeginMutable())").map_err(fmt_err)?;
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            let deeper = format!("{indent}    ");
            for (idx, m) in members.iter().enumerate() {
                emit_encode_member_mutable(out, &deeper, m, idx)?;
            }
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
        }
    }
    Ok(())
}

fn emit_encode_member_plain(
    out: &mut String,
    indent: &str,
    m: &MemberInfo,
) -> Result<(), CsGenError> {
    if m.is_optional {
        // Final/Appendable: 1-byte present-flag + value-on-true.
        writeln!(
            out,
            "{indent}if (sample.{prop} is null) {{ w.WriteOctet(0); }}",
            prop = m.cs_prop
        )
        .map_err(fmt_err)?;
        writeln!(out, "{indent}else").map_err(fmt_err)?;
        writeln!(out, "{indent}{{").map_err(fmt_err)?;
        let deeper = format!("{indent}    ");
        writeln!(out, "{deeper}w.WriteOctet(1);").map_err(fmt_err)?;
        emit_encode_value(
            out,
            &deeper,
            &m.type_spec,
            &format!("sample.{}.Value", m.cs_prop),
        )?;
        writeln!(out, "{indent}}}").map_err(fmt_err)?;
    } else {
        emit_encode_value(out, indent, &m.type_spec, &format!("sample.{}", m.cs_prop))?;
    }
    Ok(())
}

fn emit_encode_member_mutable(
    out: &mut String,
    indent: &str,
    m: &MemberInfo,
    idx: usize,
) -> Result<(), CsGenError> {
    let id = m.explicit_id.unwrap_or(idx as u32);
    let mu = if m.must_understand { "true" } else { "false" };
    let lc = lc_for_type(&m.type_spec);

    let (open, close, val_expr) = if m.is_optional {
        let prop = &m.cs_prop;
        (
            format!("{indent}if (sample.{prop} is not null)\n{indent}{{"),
            format!("{indent}}}"),
            format!("sample.{prop}.Value"),
        )
    } else {
        (
            String::new(),
            String::new(),
            format!("sample.{}", m.cs_prop),
        )
    };

    if !open.is_empty() {
        writeln!(out, "{open}").map_err(fmt_err)?;
    }
    let body_indent = if m.is_optional {
        format!("{indent}    ")
    } else {
        indent.to_string()
    };

    if lc <= 2 {
        // Fixed-size 1/2/4 bytes: 4-byte EMHEADER, then value.
        writeln!(out, "{body_indent}w.WriteEmHeader({id}u, {lc}, {mu});").map_err(fmt_err)?;
        emit_encode_value(out, &body_indent, &m.type_spec, &val_expr)?;
    } else {
        // LC=4 (variable / 8-byte): encode the body in a sub-writer, then
        // EMHEADER(LC=4) + NEXTINT(byte-len) + body bytes. (LC=3 would mean a
        // fixed 8-byte body with no NEXTINT and desyncs spec-compliant readers.)
        writeln!(out, "{body_indent}{{").map_err(fmt_err)?;
        let d = format!("{body_indent}    ");
        writeln!(out, "{d}var __sub = new Xcdr2Writer(endian);").map_err(fmt_err)?;
        emit_encode_value_into(out, &d, &m.type_spec, &val_expr, "__sub")?;
        writeln!(out, "{d}var __subBytes = __sub.ToArray();").map_err(fmt_err)?;
        writeln!(out, "{d}w.WriteEmHeader({id}u, {lc}, {mu});").map_err(fmt_err)?;
        writeln!(out, "{d}w.WriteUInt32((uint)__subBytes.Length);").map_err(fmt_err)?;
        writeln!(out, "{d}w.WriteBytes(__subBytes);").map_err(fmt_err)?;
        writeln!(out, "{body_indent}}}").map_err(fmt_err)?;
    }

    if !close.is_empty() {
        writeln!(out, "{close}").map_err(fmt_err)?;
    }
    Ok(())
}

/// Like `emit_encode_value`, but writes into a sub-writer with a
/// user-chosen variable name instead of `w`.
fn emit_encode_value_into(
    out: &mut String,
    indent: &str,
    ts: &TypeSpec,
    expr: &str,
    writer_var: &str,
) -> Result<(), CsGenError> {
    // We use a simple string-replace trick: emit into a temporary
    // buffer, then replace `w.` with `<writer_var>.`.
    let mut tmp = String::new();
    emit_encode_value(&mut tmp, indent, ts, expr)?;
    let patched = tmp.replace("w.", &format!("{writer_var}."));
    out.push_str(&patched);
    Ok(())
}

/// Returns the EMHEADER LengthCode for a member type per OMG XTypes 1.3
/// §7.4.3.4.2 (= the `zerodds-cdr` `LengthCode` enum + `org.zerodds.cdr`
/// Java constants):
///   0 = 1-byte body (no NEXTINT), 1 = 2-byte, 2 = 4-byte, 3 = 8-byte,
///   4 = variable body with NEXTINT (= byte length), 5/6/7 = NEXTINT variants.
///
/// IMPORTANT: variable-length members (string, sequence, map, nested struct)
/// MUST use LC=4 — NOT LC=3. LC=3 tells a spec-compliant reader "read exactly
/// 8 bytes, no NEXTINT"; emitting LC=3+NEXTINT for a string desyncs Rust/
/// Cyclone/FastDDS. 8-byte primitives also take LC=4 here (NEXTINT=8) to match
/// the Rust reference encoder's universal-LC4-NEXTINT style and avoid a raw
/// unaligned 8-byte write path; an LC=4 8-byte member is fully spec-valid.
/// (Prior bug: variable AND 8-byte members were emitted as LC=3 with NEXTINT.)
fn lc_for_type(ts: &TypeSpec) -> i32 {
    match ts {
        TypeSpec::Primitive(p) => match p {
            PrimitiveType::Boolean | PrimitiveType::Octet | PrimitiveType::Char => 0,
            PrimitiveType::WideChar => 1,
            PrimitiveType::Integer(i) => match i {
                IntegerType::Short
                | IntegerType::UShort
                | IntegerType::Int16
                | IntegerType::UInt16 => 1,
                IntegerType::Long
                | IntegerType::ULong
                | IntegerType::Int32
                | IntegerType::UInt32 => 2,
                // long long / unsigned long long: 8-byte body via LC=4 NEXTINT(=8).
                IntegerType::LongLong
                | IntegerType::ULongLong
                | IntegerType::Int64
                | IntegerType::UInt64 => 4,
                IntegerType::Int8 | IntegerType::UInt8 => 0,
            },
            PrimitiveType::Floating(f) => match f {
                zerodds_idl::ast::FloatingType::Float => 2,
                // double / long double: 8-byte body via LC=4 NEXTINT(=8).
                zerodds_idl::ast::FloatingType::Double => 4,
                zerodds_idl::ast::FloatingType::LongDouble => 4,
            },
        },
        // Variable-length: LC=4, NEXTINT = body byte length.
        TypeSpec::String(_) | TypeSpec::Sequence(_) | TypeSpec::Map(_) | TypeSpec::Any => 4,
        TypeSpec::Fixed(_) => 4,
        TypeSpec::Scoped(_) => 4,
    }
}
/// zerodds-lint: recursion-depth 64 (emit_encode_value bounded by AST depth)
/// Encode helper for a sample field of the given TypeSpec.
fn emit_encode_value(
    out: &mut String,
    indent: &str,
    ts: &TypeSpec,
    expr: &str,
) -> Result<(), CsGenError> {
    match ts {
        TypeSpec::Primitive(p) => emit_encode_primitive(out, indent, *p, expr),
        TypeSpec::String(s) => {
            // Bounded `string<N>` (DDS-XTypes §7.4.3): reject over-bound on
            // encode. Narrow → UTF-8 byte length (matches the CDR wire); wide
            // `wstring<N>` → UTF-16 unit count (C# string.Length).
            if let Some(b) = &s.bound {
                let bv = crate::emitter::const_expr_to_cs(b);
                if s.wide {
                    writeln!(
                        out,
                        "{indent}if (({expr}) != null && ({expr}).Length > {bv}) throw new System.ArgumentException(\"bounded wstring length exceeds its IDL bound ({bv})\");"
                    )
                    .map_err(fmt_err)?;
                } else {
                    writeln!(
                        out,
                        "{indent}if (({expr}) != null && System.Text.Encoding.UTF8.GetByteCount({expr}) > {bv}) throw new System.ArgumentException(\"bounded string length exceeds its IDL bound ({bv})\");"
                    )
                    .map_err(fmt_err)?;
                }
            }
            writeln!(out, "{indent}w.WriteString({expr});").map_err(fmt_err)?;
            Ok(())
        }
        TypeSpec::Sequence(s) => emit_encode_sequence(out, indent, &s.elem, s.bound.as_ref(), expr),
        TypeSpec::Scoped(sc) => match lookup_scoped_kind(sc) {
            // Nested struct → encode its body into the shared writer (alignment
            // stays relative to the outer CDR stream).
            Some(ScopedKind::Struct) => {
                writeln!(
                    out,
                    "{indent}{}TypeSupport.Instance.EncodeInto(w, {expr});",
                    scoped_dotted_cs(sc)
                )
                .map_err(fmt_err)?;
                Ok(())
            }
            // Enum → int32 representation.
            Some(ScopedKind::Enum) => {
                writeln!(out, "{indent}w.WriteInt32((int){expr});").map_err(fmt_err)?;
                Ok(())
            }
            // Typedef → encode the aliased type.
            Some(ScopedKind::Typedef(inner)) => emit_encode_value(out, indent, &inner, expr),
            // Union has no codec; the containing struct is gated, so this is
            // unreachable — guard it rather than emit a broken reference.
            Some(ScopedKind::Union) => Err(CsGenError::UnsupportedConstruct {
                construct: "XCDR2 codec for union member".into(),
                context: None,
            }),
            // Unresolved scoped ref: treat as an enum-like int32 (best effort).
            None => {
                writeln!(out, "{indent}w.WriteInt32((int){expr});").map_err(fmt_err)?;
                Ok(())
            }
        },
        TypeSpec::Map(_) | TypeSpec::Fixed(_) | TypeSpec::Any => {
            // Unreachable: a struct with a map/fixed/any member is gated in
            // `emitter.rs` (no TypeSupport emitted), so this codec is never
            // generated. Defensive safety-net only.
            writeln!(
                out,
                "{indent}throw new XcdrException(\"unsupported codegen TypeSpec for member {expr}\");"
            )
            .map_err(fmt_err)?;
            Ok(())
        }
    }
}

fn emit_encode_primitive(
    out: &mut String,
    indent: &str,
    p: PrimitiveType,
    expr: &str,
) -> Result<(), CsGenError> {
    match p {
        PrimitiveType::Boolean => {
            writeln!(out, "{indent}w.WriteBool({expr});").map_err(fmt_err)?;
        }
        PrimitiveType::Octet => {
            writeln!(out, "{indent}w.WriteOctet({expr});").map_err(fmt_err)?;
        }
        PrimitiveType::Char => {
            writeln!(out, "{indent}w.WriteOctet((byte)({expr}));").map_err(fmt_err)?;
        }
        PrimitiveType::WideChar => {
            writeln!(out, "{indent}w.WriteWChar({expr});").map_err(fmt_err)?;
        }
        PrimitiveType::Integer(i) => match i {
            IntegerType::Short | IntegerType::Int16 => {
                writeln!(out, "{indent}w.WriteInt16({expr});").map_err(fmt_err)?;
            }
            IntegerType::UShort | IntegerType::UInt16 => {
                writeln!(out, "{indent}w.WriteUInt16({expr});").map_err(fmt_err)?;
            }
            IntegerType::Long | IntegerType::Int32 => {
                writeln!(out, "{indent}w.WriteInt32({expr});").map_err(fmt_err)?;
            }
            IntegerType::ULong | IntegerType::UInt32 => {
                writeln!(out, "{indent}w.WriteUInt32({expr});").map_err(fmt_err)?;
            }
            IntegerType::LongLong | IntegerType::Int64 => {
                writeln!(out, "{indent}w.WriteInt64({expr});").map_err(fmt_err)?;
            }
            IntegerType::ULongLong | IntegerType::UInt64 => {
                writeln!(out, "{indent}w.WriteUInt64({expr});").map_err(fmt_err)?;
            }
            IntegerType::Int8 => {
                writeln!(out, "{indent}w.WriteOctet((byte)({expr}));").map_err(fmt_err)?;
            }
            IntegerType::UInt8 => {
                writeln!(out, "{indent}w.WriteOctet({expr});").map_err(fmt_err)?;
            }
        },
        PrimitiveType::Floating(f) => match f {
            zerodds_idl::ast::FloatingType::Float => {
                writeln!(out, "{indent}w.WriteFloat32({expr});").map_err(fmt_err)?;
            }
            zerodds_idl::ast::FloatingType::Double => {
                writeln!(out, "{indent}w.WriteFloat64({expr});").map_err(fmt_err)?;
            }
            zerodds_idl::ast::FloatingType::LongDouble => {
                writeln!(
                    out,
                    "{indent}throw new XcdrException(\"long double not in v1.0 codegen surface\");"
                )
                .map_err(fmt_err)?;
            }
        },
    }
    Ok(())
}
/// zerodds-lint: recursion-depth 64 (emit_encode_sequence bounded by AST depth)
fn emit_encode_sequence(
    out: &mut String,
    indent: &str,
    elem: &TypeSpec,
    bound: Option<&ConstExpr>,
    expr: &str,
) -> Result<(), CsGenError> {
    let elem_ty = cs_storage_type(elem);
    // XCDR2 §7.4.3.5: non-primitive elements (string, struct, …) →
    // DHEADER (uint32 = byte length of [count + elements]) prepended;
    // primitives not. Verified against Cyclone DDS (V-5 without, V-6 with).
    let non_primitive = !matches!(elem, TypeSpec::Primitive(_));
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let d = format!("{indent}    ");
    writeln!(
        out,
        "{d}var __seq = ({expr}) as System.Collections.Generic.IEnumerable<{elem_ty}>;"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{d}var __mat = __seq is null ? new System.Collections.Generic.List<{elem_ty}>() : new System.Collections.Generic.List<{elem_ty}>(__seq);").map_err(fmt_err)?;
    if non_primitive {
        writeln!(out, "{d}using (var __seqdh = w.BeginAppendable())").map_err(fmt_err)?;
        writeln!(out, "{d}{{").map_err(fmt_err)?;
    }
    let seqd = if non_primitive {
        format!("{d}    ")
    } else {
        d.clone()
    };
    // Bounded `sequence<T, N>` (DDS-XTypes §7.4.3): over-bound = encode error.
    if let Some(b) = bound {
        let bv = crate::emitter::const_expr_to_cs(b);
        writeln!(
            out,
            "{seqd}if (__mat.Count > {bv}) throw new System.ArgumentException(\"bounded sequence length exceeds its IDL bound ({bv})\");"
        )
        .map_err(fmt_err)?;
    }
    writeln!(out, "{seqd}w.WriteSequenceLength(__mat.Count);").map_err(fmt_err)?;
    writeln!(out, "{seqd}foreach (var __item in __mat)").map_err(fmt_err)?;
    writeln!(out, "{seqd}{{").map_err(fmt_err)?;
    let dd = format!("{seqd}    ");
    emit_encode_value(out, &dd, elem, "__item")?;
    writeln!(out, "{seqd}}}").map_err(fmt_err)?;
    if non_primitive {
        writeln!(out, "{d}}}").map_err(fmt_err)?;
    }
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    Ok(())
}

/// Decode body: uses object-initializer syntax.
fn emit_decode_body(
    out: &mut String,
    indent: &str,
    struct_name: &str,
    members: &[MemberInfo],
    ext: ExtensibilityKind,
) -> Result<(), CsGenError> {
    match ext {
        ExtensibilityKind::Final => {
            // Decode sequentially, then object initializer.
            for (i, m) in members.iter().enumerate() {
                emit_decode_member_to_var(out, indent, m, i)?;
            }
            emit_decode_return(out, indent, struct_name, members)?;
        }
        ExtensibilityKind::Appendable => {
            writeln!(out, "{indent}var __scope = r.BeginDHeader();").map_err(fmt_err)?;
            for (i, m) in members.iter().enumerate() {
                emit_decode_member_to_var(out, indent, m, i)?;
            }
            writeln!(out, "{indent}r.EndDHeader(__scope);").map_err(fmt_err)?;
            emit_decode_return(out, indent, struct_name, members)?;
        }
        ExtensibilityKind::Mutable => {
            // Variable order / optionality -> nullable locals,
            // then while loop until the DHEADER end.
            for (i, m) in members.iter().enumerate() {
                let ty = decode_local_type(&m.type_spec, m.is_optional);
                writeln!(out, "{indent}{ty} __m{i} = default;").map_err(fmt_err)?;
            }
            writeln!(out, "{indent}var __scope = r.BeginDHeader();").map_err(fmt_err)?;
            writeln!(out, "{indent}while (!r.DHeaderDone(__scope))").map_err(fmt_err)?;
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            let d = format!("{indent}    ");
            writeln!(out, "{d}var (__id, __lc, __mu) = r.ReadEmHeader();").map_err(fmt_err)?;
            // NEXTINT consumption: only LC>=4 carries a NEXTINT (XTypes
            // §7.4.3.4.2). LC0..3 are fixed 1/2/4/8-byte bodies with no NEXTINT
            // — including a cross-vendor LC=3 8-byte primitive.
            writeln!(
                out,
                "{d}if (__lc >= 4) {{ var __nx = r.ReadUInt32(); _ = __nx; }}"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{d}switch (__id)").map_err(fmt_err)?;
            writeln!(out, "{d}{{").map_err(fmt_err)?;
            let dd = format!("{d}    ");
            for (i, m) in members.iter().enumerate() {
                let id = m.explicit_id.unwrap_or(i as u32);
                writeln!(out, "{dd}case {id}u:").map_err(fmt_err)?;
                let ddd = format!("{dd}    ");
                emit_decode_member_assign(out, &ddd, m, i)?;
                writeln!(out, "{ddd}break;").map_err(fmt_err)?;
            }
            writeln!(out, "{dd}default:").map_err(fmt_err)?;
            writeln!(
                out,
                "{dd}    throw new XcdrException($\"unknown member id {{__id}}\");"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{d}}}").map_err(fmt_err)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
            writeln!(out, "{indent}r.EndDHeader(__scope);").map_err(fmt_err)?;
            emit_decode_return_mutable(out, indent, struct_name, members)?;
        }
    }
    Ok(())
}

fn decode_local_type(ts: &TypeSpec, optional: bool) -> String {
    let base = cs_storage_type(ts);
    if optional && !is_reference_type(ts) {
        format!("{base}?")
    } else {
        base
    }
}
/// zerodds-lint: recursion-depth 64 (cs_storage_type bounded by AST depth)
fn cs_storage_type(ts: &TypeSpec) -> String {
    match ts {
        TypeSpec::Primitive(p) => prim_to_cs_type(*p).to_string(),
        TypeSpec::String(_) => "string".into(),
        TypeSpec::Sequence(s) => {
            let inner = cs_storage_type(&s.elem);
            // The property type from the codegen is `Omg.Types.ISequence<T>`;
            // for decode we need the concrete container.
            format!("Omg.Types.ISequence<{inner}>")
        }
        TypeSpec::Scoped(s) => s
            .parts
            .iter()
            .map(|p| p.text.clone())
            .collect::<Vec<_>>()
            .join("."),
        TypeSpec::Map(_) => "object".into(),
        TypeSpec::Fixed(_) => "decimal".into(),
        TypeSpec::Any => "object".into(),
    }
}

fn prim_to_cs_type(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "bool",
        PrimitiveType::Octet => "byte",
        PrimitiveType::Char => "byte",
        PrimitiveType::WideChar => "char",
        PrimitiveType::Integer(i) => match i {
            IntegerType::Short | IntegerType::Int16 => "short",
            IntegerType::UShort | IntegerType::UInt16 => "ushort",
            IntegerType::Long | IntegerType::Int32 => "int",
            IntegerType::ULong | IntegerType::UInt32 => "uint",
            IntegerType::LongLong | IntegerType::Int64 => "long",
            IntegerType::ULongLong | IntegerType::UInt64 => "ulong",
            IntegerType::Int8 => "sbyte",
            IntegerType::UInt8 => "byte",
        },
        PrimitiveType::Floating(f) => match f {
            zerodds_idl::ast::FloatingType::Float => "float",
            zerodds_idl::ast::FloatingType::Double => "double",
            zerodds_idl::ast::FloatingType::LongDouble => "decimal",
        },
    }
}

fn is_reference_type(ts: &TypeSpec) -> bool {
    matches!(
        ts,
        TypeSpec::String(_) | TypeSpec::Sequence(_) | TypeSpec::Map(_) | TypeSpec::Any
    )
}

fn emit_decode_member_to_var(
    out: &mut String,
    indent: &str,
    m: &MemberInfo,
    idx: usize,
) -> Result<(), CsGenError> {
    let ty = decode_local_type(&m.type_spec, m.is_optional);
    writeln!(out, "{indent}{ty} __m{idx} = default;").map_err(fmt_err)?;
    if m.is_optional {
        // Final/Appendable: present-flag + value.
        writeln!(out, "{indent}{{").map_err(fmt_err)?;
        let d = format!("{indent}    ");
        writeln!(out, "{d}byte __present = r.ReadOctet();").map_err(fmt_err)?;
        writeln!(out, "{d}if (__present != 0)").map_err(fmt_err)?;
        writeln!(out, "{d}{{").map_err(fmt_err)?;
        let dd = format!("{d}    ");
        emit_decode_assign(out, &dd, &m.type_spec, &format!("__m{idx}"))?;
        writeln!(out, "{d}}}").map_err(fmt_err)?;
        writeln!(out, "{indent}}}").map_err(fmt_err)?;
    } else {
        emit_decode_assign(out, indent, &m.type_spec, &format!("__m{idx}"))?;
    }
    Ok(())
}

fn emit_decode_member_assign(
    out: &mut String,
    indent: &str,
    m: &MemberInfo,
    idx: usize,
) -> Result<(), CsGenError> {
    // Mutable: assign the value directly into the local.
    emit_decode_assign(out, indent, &m.type_spec, &format!("__m{idx}"))?;
    Ok(())
}
/// zerodds-lint: recursion-depth 64 (emit_decode_assign bounded by AST depth)
/// Emits C# statements that assign `target` the decoded value of `ts`.
fn emit_decode_assign(
    out: &mut String,
    indent: &str,
    ts: &TypeSpec,
    target: &str,
) -> Result<(), CsGenError> {
    match ts {
        TypeSpec::Primitive(_)
        | TypeSpec::String(_)
        | TypeSpec::Scoped(_)
        | TypeSpec::Map(_)
        | TypeSpec::Fixed(_)
        | TypeSpec::Any => {
            writeln!(
                out,
                "{indent}{target} = {expr};",
                target = target,
                expr = decode_simple_expr(ts)
            )
            .map_err(fmt_err)?;
            Ok(())
        }
        TypeSpec::Sequence(s) => {
            let elem_ty = cs_storage_type(&s.elem);
            // XCDR2 §7.4.3.5: for non-primitive elements, skip over the DHEADER.
            let non_primitive = !matches!(&*s.elem, TypeSpec::Primitive(_));
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            let d = format!("{indent}    ");
            if non_primitive {
                writeln!(out, "{d}var __seqdh = r.BeginDHeader();").map_err(fmt_err)?;
            }
            writeln!(out, "{d}int __cnt = r.ReadSequenceLength();").map_err(fmt_err)?;
            // The property type is ISequence<T> (Omg.Types) — we use the
            // concrete container `SequenceList<T>` from the runtime.
            writeln!(
                out,
                "{d}var __list = new Omg.Types.SequenceList<{elem_ty}>();"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{d}for (int __i = 0; __i < __cnt; __i++)").map_err(fmt_err)?;
            writeln!(out, "{d}{{").map_err(fmt_err)?;
            let dd = format!("{d}    ");
            // Element decode can itself be recursive.
            writeln!(out, "{dd}{elem_ty} __e;").map_err(fmt_err)?;
            emit_decode_assign(out, &dd, &s.elem, "__e")?;
            writeln!(out, "{dd}__list.Add(__e);").map_err(fmt_err)?;
            writeln!(out, "{d}}}").map_err(fmt_err)?;
            if non_primitive {
                writeln!(out, "{d}r.EndDHeader(__seqdh);").map_err(fmt_err)?;
            }
            writeln!(out, "{d}{target} = __list;").map_err(fmt_err)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
            Ok(())
        }
    }
}

/// zerodds-lint: recursion-depth 64 (bounded by IDL nesting)
fn decode_simple_expr(ts: &TypeSpec) -> String {
    match ts {
        TypeSpec::Primitive(p) => decode_primitive_expr(*p).to_string(),
        TypeSpec::String(_) => "r.ReadString()".into(),
        TypeSpec::Scoped(sc) => match lookup_scoped_kind(sc) {
            // Nested struct → decode its body from the shared reader.
            Some(ScopedKind::Struct) => {
                format!("{}TypeSupport.Instance.DecodeFrom(r)", scoped_dotted_cs(sc))
            }
            // Enum → cast from the int32 representation.
            Some(ScopedKind::Enum) => format!("({})r.ReadInt32()", scoped_dotted_cs(sc)),
            // Typedef → decode the aliased type.
            Some(ScopedKind::Typedef(inner)) => decode_simple_expr(&inner),
            // Union: unreachable (containing struct gated); unresolved: best effort.
            _ => "default!".into(),
        },
        TypeSpec::Map(_) | TypeSpec::Fixed(_) | TypeSpec::Any => {
            // Unreachable: the containing struct is gated (no TypeSupport).
            "throw new XcdrException(\"decode unsupported type\")".into()
        }
        TypeSpec::Sequence(_) => "default!".into(),
    }
}

fn decode_primitive_expr(p: PrimitiveType) -> &'static str {
    match p {
        PrimitiveType::Boolean => "r.ReadBool()",
        PrimitiveType::Octet => "r.ReadOctet()",
        PrimitiveType::Char => "r.ReadOctet()",
        PrimitiveType::WideChar => "r.ReadWChar()",
        PrimitiveType::Integer(i) => match i {
            IntegerType::Short | IntegerType::Int16 => "r.ReadInt16()",
            IntegerType::UShort | IntegerType::UInt16 => "r.ReadUInt16()",
            IntegerType::Long | IntegerType::Int32 => "r.ReadInt32()",
            IntegerType::ULong | IntegerType::UInt32 => "r.ReadUInt32()",
            IntegerType::LongLong | IntegerType::Int64 => "r.ReadInt64()",
            IntegerType::ULongLong | IntegerType::UInt64 => "r.ReadUInt64()",
            IntegerType::Int8 => "(sbyte)r.ReadOctet()",
            IntegerType::UInt8 => "r.ReadOctet()",
        },
        PrimitiveType::Floating(f) => match f {
            zerodds_idl::ast::FloatingType::Float => "r.ReadFloat32()",
            zerodds_idl::ast::FloatingType::Double => "r.ReadFloat64()",
            zerodds_idl::ast::FloatingType::LongDouble => "default(decimal)",
        },
    }
}

fn emit_decode_return(
    out: &mut String,
    indent: &str,
    struct_name: &str,
    members: &[MemberInfo],
) -> Result<(), CsGenError> {
    writeln!(out, "{indent}return new {struct_name}").map_err(fmt_err)?;
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let d = format!("{indent}    ");
    for (i, m) in members.iter().enumerate() {
        writeln!(out, "{d}{prop} = __m{i}!,", prop = m.cs_prop, i = i).map_err(fmt_err)?;
    }
    writeln!(out, "{indent}}};").map_err(fmt_err)?;
    Ok(())
}

fn emit_decode_return_mutable(
    out: &mut String,
    indent: &str,
    struct_name: &str,
    members: &[MemberInfo],
) -> Result<(), CsGenError> {
    writeln!(out, "{indent}return new {struct_name}").map_err(fmt_err)?;
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let d = format!("{indent}    ");
    for (i, m) in members.iter().enumerate() {
        writeln!(out, "{d}{prop} = __m{i}!,", prop = m.cs_prop, i = i).map_err(fmt_err)?;
    }
    writeln!(out, "{indent}}};").map_err(fmt_err)?;
    Ok(())
}

/// KeyHash: PlainCdr2BeKeyHolder -> MD5 if > 16 bytes, otherwise zero-pad.
/// XTypes 1.3 §7.6.8.
fn emit_key_hash_body(
    out: &mut String,
    indent: &str,
    members: &[MemberInfo],
) -> Result<(), CsGenError> {
    writeln!(
        out,
        "{indent}var __kw = new Xcdr2Writer(EndianMode.BigEndian);"
    )
    .map_err(fmt_err)?;
    for m in members {
        if !m.is_key {
            continue;
        }
        emit_key_encode_value(out, indent, &m.type_spec, &format!("sample.{}", m.cs_prop))?;
    }
    writeln!(out, "{indent}var __kb = __kw.ToArray();").map_err(fmt_err)?;
    // XTypes §7.6.8: when keyholder size > 16 -> MD5; else zero-pad to 16.
    writeln!(
        out,
        "{indent}if (__kb.Length > 16) {{ return Md5.Hash(__kb); }}"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{indent}var __h = new byte[16];").map_err(fmt_err)?;
    writeln!(
        out,
        "{indent}System.Array.Copy(__kb, 0, __h, 0, __kb.Length);"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{indent}return __h;").map_err(fmt_err)?;
    Ok(())
}

fn emit_key_encode_value(
    out: &mut String,
    indent: &str,
    ts: &TypeSpec,
    expr: &str,
) -> Result<(), CsGenError> {
    match ts {
        TypeSpec::Primitive(p) => emit_key_encode_primitive(out, indent, *p, expr),
        TypeSpec::String(_) => {
            writeln!(out, "{indent}__kw.WriteString({expr});").map_err(fmt_err)?;
            Ok(())
        }
        TypeSpec::Scoped(_) => {
            writeln!(
                out,
                "{indent}// nested key types delegate via TypeSupport (Phase 6+)"
            )
            .map_err(fmt_err)?;
            Ok(())
        }
        _ => {
            writeln!(
                out,
                "{indent}throw new XcdrException(\"unsupported key type for member {expr}\");"
            )
            .map_err(fmt_err)?;
            Ok(())
        }
    }
}

fn emit_key_encode_primitive(
    out: &mut String,
    indent: &str,
    p: PrimitiveType,
    expr: &str,
) -> Result<(), CsGenError> {
    let stmt = match p {
        PrimitiveType::Boolean => format!("__kw.WriteBool({expr});"),
        PrimitiveType::Octet => format!("__kw.WriteOctet({expr});"),
        PrimitiveType::Char => format!("__kw.WriteOctet((byte)({expr}));"),
        PrimitiveType::WideChar => format!("__kw.WriteWChar({expr});"),
        PrimitiveType::Integer(i) => match i {
            IntegerType::Short | IntegerType::Int16 => format!("__kw.WriteInt16({expr});"),
            IntegerType::UShort | IntegerType::UInt16 => format!("__kw.WriteUInt16({expr});"),
            IntegerType::Long | IntegerType::Int32 => format!("__kw.WriteInt32({expr});"),
            IntegerType::ULong | IntegerType::UInt32 => format!("__kw.WriteUInt32({expr});"),
            IntegerType::LongLong | IntegerType::Int64 => format!("__kw.WriteInt64({expr});"),
            IntegerType::ULongLong | IntegerType::UInt64 => format!("__kw.WriteUInt64({expr});"),
            IntegerType::Int8 => format!("__kw.WriteOctet((byte)({expr}));"),
            IntegerType::UInt8 => format!("__kw.WriteOctet({expr});"),
        },
        PrimitiveType::Floating(f) => match f {
            zerodds_idl::ast::FloatingType::Float => format!("__kw.WriteFloat32({expr});"),
            zerodds_idl::ast::FloatingType::Double => format!("__kw.WriteFloat64({expr});"),
            zerodds_idl::ast::FloatingType::LongDouble => {
                "throw new XcdrException(\"long double key not supported\");".into()
            }
        },
    };
    writeln!(out, "{indent}{stmt}").map_err(fmt_err)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    #[test]
    fn type_name_root_struct() {
        assert_eq!(make_dds_type_name(&[], "Point"), "Point");
    }

    #[test]
    fn type_name_one_module() {
        assert_eq!(make_dds_type_name(&["Outer".to_string()], "S"), "Outer::S");
    }

    #[test]
    fn type_name_nested_modules() {
        assert_eq!(
            make_dds_type_name(&["Outer".to_string(), "Inner".to_string()], "S"),
            "Outer::Inner::S"
        );
    }
}
