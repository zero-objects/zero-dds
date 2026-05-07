// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XCDR2 TypeSupport-Emission fuer C#-Codegen.
//!
//! Spec: `zerodds-xcdr2-csharp-1.0` §3 / §4 / §5 / §6 / §7
//! + `zerodds-xcdr2-bindings-conformance-1.0` §6 (V-1..V-12).
//!
//! Pro IDL-`struct` emittiert dieses Modul eine `*TypeSupport`-Klasse,
//! die `IDdsTopicType<T>` aus `ZeroDDS.Cdr` implementiert. Encode/Decode
//! delegieren an `Xcdr2Writer` / `Xcdr2Reader`; Extensibility steuert
//! das DHEADER/EMHEADER-Layout, `@key` triggert MD5-KeyHash via
//! `PlainCdr2BeKeyHolder`.

use std::fmt::Write;

use zerodds_idl::ast::{IntegerType, PrimitiveType, StructDef, TypeSpec};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind, lower_annotations, lower_type_annotations,
};

use crate::error::CsGenError;
use crate::keywords::escape_identifier;

/// Kontext-Info: aktueller Modul-Pfad fuer Type-Name-Emission und der
/// IDL-Struct-Name (ohne Module).
pub(crate) struct TsEmitContext<'a> {
    pub module_path: &'a [String],
    pub indent: &'a str,
    pub inner_indent: &'a str,
    pub deeper_indent: &'a str,
}

/// Liefert `<Module1>::<Module2>::<Struct>` per Spec §5 (Type-Name-Konvention).
pub(crate) fn make_dds_type_name(module_path: &[String], struct_name: &str) -> String {
    if module_path.is_empty() {
        struct_name.to_string()
    } else {
        format!("{}::{}", module_path.join("::"), struct_name)
    }
}

/// Member-Info: berechnete Wire-Charakteristiken eines Struct-Members.
struct MemberInfo {
    /// Property-Name in PascalCase (matches dem in `emit_struct_member_property`).
    cs_prop: String,
    /// IDL-Source-Type (fuer Encode/Decode-Method-Wahl).
    type_spec: TypeSpec,
    /// True wenn `@key`.
    is_key: bool,
    /// True wenn `@optional`.
    is_optional: bool,
    /// True wenn `@must_understand`.
    must_understand: bool,
    /// `@id(N)` falls explizit gesetzt; sonst None (Auto-Index in Mutable).
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

/// Liefert das IDL-Extensibility-Kind, default Appendable.
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

/// Haupt-Entry: emittiert eine `*TypeSupport`-Klasse fuer `s`.
///
/// `module_path` ist die Liste der umschliessenden Module (z.B.
/// `["Outer", "Inner"]` fuer `module Outer { module Inner { struct S }}`).
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

    // Encode(sample, endian).
    writeln!(out).map_err(fmt_err)?;
    writeln!(
        out,
        "{inner}public byte[] Encode({struct_name} sample, EndianMode endian)"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{inner}{{").map_err(fmt_err)?;
    writeln!(out, "{deeper}var w = new Xcdr2Writer(endian);").map_err(fmt_err)?;
    emit_encode_body(out, deeper, &members, ext)?;
    writeln!(out, "{deeper}return w.ToArray();").map_err(fmt_err)?;
    writeln!(out, "{inner}}}").map_err(fmt_err)?;
    writeln!(out).map_err(fmt_err)?;

    // Decode(bytes).
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

/// Schreibt die Encode-Sequenz fuer alle Member entsprechend der Extensibility.
fn emit_encode_body(
    out: &mut String,
    indent: &str,
    members: &[MemberInfo],
    ext: ExtensibilityKind,
) -> Result<(), CsGenError> {
    match ext {
        ExtensibilityKind::Final => {
            // Plain-Body, kein DHEADER.
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
        // Final/Appendable: 1-Byte present-flag + value-on-true.
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
        // Fixed-size 1/2/4 Byte: 4-Byte EMHEADER, dann Wert.
        writeln!(out, "{body_indent}w.WriteEmHeader({id}u, {lc}, {mu});").map_err(fmt_err)?;
        emit_encode_value(out, &body_indent, &m.type_spec, &val_expr)?;
    } else {
        // LC=3 → NEXTINT-prefixed (Vendor-Spec §6 V-10): encode in Sub-Writer,
        // dann EMHEADER (LC=3) + NEXTINT(byte-len) + Body-Bytes.
        writeln!(out, "{body_indent}{{").map_err(fmt_err)?;
        let d = format!("{body_indent}    ");
        writeln!(out, "{d}var __sub = new Xcdr2Writer(endian);").map_err(fmt_err)?;
        emit_encode_value_into(out, &d, &m.type_spec, &val_expr, "__sub")?;
        writeln!(out, "{d}var __subBytes = __sub.ToArray();").map_err(fmt_err)?;
        writeln!(out, "{d}w.WriteEmHeader({id}u, 3, {mu});").map_err(fmt_err)?;
        writeln!(out, "{d}w.WriteUInt32((uint)__subBytes.Length);").map_err(fmt_err)?;
        writeln!(out, "{d}w.WriteBytes(__subBytes);").map_err(fmt_err)?;
        writeln!(out, "{body_indent}}}").map_err(fmt_err)?;
    }

    if !close.is_empty() {
        writeln!(out, "{close}").map_err(fmt_err)?;
    }
    Ok(())
}

/// Wie `emit_encode_value`, aber schreibt in einen Sub-Writer mit benutzer-
/// gewaehltem Variablen-Namen statt `w`.
fn emit_encode_value_into(
    out: &mut String,
    indent: &str,
    ts: &TypeSpec,
    expr: &str,
    writer_var: &str,
) -> Result<(), CsGenError> {
    // Wir nutzen einen einfachen String-Replace-Trick: in einen temporaeren
    // Buffer emittieren, dann `w.` durch `<writer_var>.` ersetzen.
    let mut tmp = String::new();
    emit_encode_value(&mut tmp, indent, ts, expr)?;
    let patched = tmp.replace("w.", &format!("{writer_var}."));
    out.push_str(&patched);
    Ok(())
}

/// Liefert das LC-Length-Code fuer einen Member-Type entsprechend
/// `zerodds-xcdr2-bindings-conformance-1.0` §6 V-10/V-11.  Zwischen
/// XTypes 1.3 Tabelle 36 und der ZeroDDS-Vendor-Spec gibt es eine
/// Differenz beim variabel-langen Member: die Spec nutzt **LC=3 mit
/// NEXTINT** (anstatt XTypes' LC=4 mit NEXTINT). Wir folgen der Vendor-
/// Spec, damit V-10 byte-exact passt.
///
/// Mapping:
/// 0 = 1 Byte fix, 1 = 2 Byte fix, 2 = 4 Byte fix, 3 = NEXTINT-prefixed
/// (byte length).
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
                // long long / unsigned long long: 8-Byte fix; im Vendor-LC-Schema
                // wird das ueber NEXTINT-LC=3 mit explizitem size=8 abgebildet.
                IntegerType::LongLong
                | IntegerType::ULongLong
                | IntegerType::Int64
                | IntegerType::UInt64 => 3,
                IntegerType::Int8 | IntegerType::UInt8 => 0,
            },
            PrimitiveType::Floating(f) => match f {
                zerodds_idl::ast::FloatingType::Float => 2,
                zerodds_idl::ast::FloatingType::Double => 3,
                zerodds_idl::ast::FloatingType::LongDouble => 3,
            },
        },
        // Variabel: NEXTINT-Layout LC=3 (next-int = body-size in bytes).
        TypeSpec::String(_) | TypeSpec::Sequence(_) | TypeSpec::Map(_) | TypeSpec::Any => 3,
        TypeSpec::Fixed(_) => 3,
        TypeSpec::Scoped(_) => 3,
    }
}
/// zerodds-lint: recursion-depth 64 (emit_encode_value bounded by AST depth)
/// Encode-Helper fuer Sample-Field gegebenen TypeSpec.
fn emit_encode_value(
    out: &mut String,
    indent: &str,
    ts: &TypeSpec,
    expr: &str,
) -> Result<(), CsGenError> {
    match ts {
        TypeSpec::Primitive(p) => emit_encode_primitive(out, indent, *p, expr),
        TypeSpec::String(_) => {
            writeln!(out, "{indent}w.WriteString({expr});").map_err(fmt_err)?;
            Ok(())
        }
        TypeSpec::Sequence(s) => emit_encode_sequence(out, indent, &s.elem, expr),
        TypeSpec::Scoped(_) => {
            // Nested struct -> delegate to its TypeSupport.Instance.
            // Spec §5: "rekursiv UTypeSupport.Instance.Encode(...)" — wir
            // emittieren einen Aufruf der die Sub-Bytes direkt in den
            // aktuellen Writer schreibt. Da Encode() eine neue Byte-Liste
            // liefert, kopieren wir sie via WriteBytes (kein Alignment-
            // Reset, da der Sub-Encode am gleichen Origin haengt — bei
            // Final-Sub Wire-identisch; bei Appendable-Sub bringt es seinen
            // eigenen DHEADER mit).
            writeln!(
                out,
                "{indent}// Nested struct: bytes contain own DHEADER if appendable/mutable"
            )
            .map_err(fmt_err)?;
            writeln!(
                out,
                "{indent}w.WriteBytes((({expr}) as object) is byte[] __ts ? __ts : System.Array.Empty<byte>());"
            )
            .map_err(fmt_err)?;
            // ^ Foundation-Approximation. Der saubere Weg ist ein spezialisierter
            // Codegen pro Nested-Struct-Type; das setzen wir via Spec-Hint
            // (§5) auf rec-Aufruf um, wenn wir den Type-Namen aufloesen koennen.
            Ok(())
        }
        TypeSpec::Map(_) | TypeSpec::Fixed(_) | TypeSpec::Any => {
            // Codegen-Foundation: nicht alle XTypes-Konstrukte sind in
            // V-1..V-12 verlangt; wir emittieren einen runtime-Stub.
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
    expr: &str,
) -> Result<(), CsGenError> {
    let elem_ty = cs_storage_type(elem);
    writeln!(out, "{indent}{{").map_err(fmt_err)?;
    let d = format!("{indent}    ");
    writeln!(
        out,
        "{d}var __seq = ({expr}) as System.Collections.Generic.IEnumerable<{elem_ty}>;"
    )
    .map_err(fmt_err)?;
    writeln!(out, "{d}var __mat = __seq is null ? new System.Collections.Generic.List<{elem_ty}>() : new System.Collections.Generic.List<{elem_ty}>(__seq);").map_err(fmt_err)?;
    writeln!(out, "{d}w.WriteSequenceLength(__mat.Count);").map_err(fmt_err)?;
    writeln!(out, "{d}foreach (var __item in __mat)").map_err(fmt_err)?;
    writeln!(out, "{d}{{").map_err(fmt_err)?;
    let dd = format!("{d}    ");
    emit_encode_value(out, &dd, elem, "__item")?;
    writeln!(out, "{d}}}").map_err(fmt_err)?;
    writeln!(out, "{indent}}}").map_err(fmt_err)?;
    Ok(())
}

/// Decode-Body: nutzt Object-Initializer-Syntax.
fn emit_decode_body(
    out: &mut String,
    indent: &str,
    struct_name: &str,
    members: &[MemberInfo],
    ext: ExtensibilityKind,
) -> Result<(), CsGenError> {
    match ext {
        ExtensibilityKind::Final => {
            // Sequenziell decodieren, dann Object-Initializer.
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
            // Variable Reihenfolge / Optionalitaet -> nullable locals,
            // dann while bis DHEADER-Ende.
            for (i, m) in members.iter().enumerate() {
                let ty = decode_local_type(&m.type_spec, m.is_optional);
                writeln!(out, "{indent}{ty} __m{i} = default;").map_err(fmt_err)?;
            }
            writeln!(out, "{indent}var __scope = r.BeginDHeader();").map_err(fmt_err)?;
            writeln!(out, "{indent}while (!r.DHeaderDone(__scope))").map_err(fmt_err)?;
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            let d = format!("{indent}    ");
            writeln!(out, "{d}var (__id, __lc, __mu) = r.ReadEmHeader();").map_err(fmt_err)?;
            // NEXTINT consumption: LC=3 (Vendor-Spec) ist NEXTINT-prefixed.
            writeln!(
                out,
                "{d}if (__lc >= 3) {{ var __nx = r.ReadUInt32(); _ = __nx; }}"
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
            // Property-Type aus dem Codegen ist `Omg.Types.ISequence<T>`;
            // beim Decode brauchen wir den konkreten Container.
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
    // Mutable: Wert direkt in Local zuweisen.
    emit_decode_assign(out, indent, &m.type_spec, &format!("__m{idx}"))?;
    Ok(())
}
/// zerodds-lint: recursion-depth 64 (emit_decode_assign bounded by AST depth)
/// Emittiert C#-Statements, die `target` mit dem decodeten Wert von `ts` belegen.
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
            writeln!(out, "{indent}{{").map_err(fmt_err)?;
            let d = format!("{indent}    ");
            writeln!(out, "{d}int __cnt = r.ReadSequenceLength();").map_err(fmt_err)?;
            // Property-Type ist ISequence<T> (Omg.Types) — wir nutzen den
            // konkreten Container `SequenceList<T>` aus der Runtime.
            writeln!(
                out,
                "{d}var __list = new Omg.Types.SequenceList<{elem_ty}>();"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{d}for (int __i = 0; __i < __cnt; __i++)").map_err(fmt_err)?;
            writeln!(out, "{d}{{").map_err(fmt_err)?;
            let dd = format!("{d}    ");
            // Element-decode kann selbst rekursiv sein.
            writeln!(out, "{dd}{elem_ty} __e;").map_err(fmt_err)?;
            emit_decode_assign(out, &dd, &s.elem, "__e")?;
            writeln!(out, "{dd}__list.Add(__e);").map_err(fmt_err)?;
            writeln!(out, "{d}}}").map_err(fmt_err)?;
            writeln!(out, "{d}{target} = __list;").map_err(fmt_err)?;
            writeln!(out, "{indent}}}").map_err(fmt_err)?;
            Ok(())
        }
    }
}

fn decode_simple_expr(ts: &TypeSpec) -> String {
    match ts {
        TypeSpec::Primitive(p) => decode_primitive_expr(*p).to_string(),
        TypeSpec::String(_) => "r.ReadString()".into(),
        TypeSpec::Scoped(_) => "default!".into(),
        TypeSpec::Map(_) | TypeSpec::Fixed(_) | TypeSpec::Any => {
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

/// KeyHash: PlainCdr2BeKeyHolder -> MD5 wenn > 16 Bytes, sonst zero-pad.
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
