// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! XCDR2 TypeSupport code generation for IDL structs.
//!
//! For each `struct` this walker emits a singleton class
//! `<Name>TypeSupport` that implements `org.zerodds.cdr.TopicTypeSupport<T>`.
//! The class provides the §3 surface API
//! (`getTypeName`, `isKeyed`, `getExtensibility`, `encode/decode`,
//! `keyHash`).
//!
//! Spec anchor: zerodds-xcdr2-java-1.0 §3 + §4 + §5 + §6 + §7.
//!
//! Coverage:
//! - Primitive members: boolean/octet/char/wchar/short/long/long long/
//!   float/double + unsigned variants.
//! - String members (UTF-8 length+1+NUL).
//! - Sequence<T> members (uint32 count + element loop).
//! - Nested struct (delegates to `<Type>TypeSupport.INSTANCE`).
//! - Extensibility: @final/@appendable/@mutable.
//! - Annotations: @key, @id(N), @optional.
//!
//! Intentionally out of scope (grows in v1.x):
//! - Map<K,V> members.
//! - fixed/any/wstring (wstring is supported in Writer/Reader; codegen
//!   grows on demand).
//! - Bitset/Bitmask.
//! - Union members (unions are emitted separately as sealed interfaces
//!   by the codegen; TypeSupport for unions is v1.1).

use std::fmt::Write;

use zerodds_idl::ast::{
    ConstrTypeDecl, Declarator, Definition, FloatingType, IntegerType, Member, PrimitiveType,
    Specification, StructDcl, StructDef, TypeDecl, TypeSpec,
};
use zerodds_idl::semantics::annotations::{
    BuiltinAnnotation, ExtensibilityKind as IdlExtensibility, Lowered,
};

use crate::JavaGenOptions;
use crate::annotations::lower_or_empty;
use crate::emitter::{JavaFile, fmt_err, indent_unit, wrap_compilation_unit_default};
use crate::error::JavaGenError;
use crate::keywords::sanitize_identifier;
use crate::type_map::primitive_to_java;

/// Emits one TypeSupport file per top-level struct.
pub(crate) fn emit_typesupport_files(
    spec: &Specification,
    opts: &JavaGenOptions,
) -> Result<Vec<JavaFile>, JavaGenError> {
    let mut files = Vec::new();
    let pkg = sanitize_package(&opts.root_package);
    walk_defs(&spec.definitions, &pkg, &[], opts, &mut files)?;
    Ok(files)
}

fn sanitize_package(p: &str) -> String {
    p.trim_matches('.').to_string()
}

/// zerodds-lint: recursion-depth 64 (IDL module depth-bounded)
fn walk_defs(
    defs: &[Definition],
    pkg: &str,
    module_chain: &[String],
    opts: &JavaGenOptions,
    files: &mut Vec<JavaFile>,
) -> Result<(), JavaGenError> {
    for d in defs {
        match d {
            Definition::Module(m) => {
                let name = sanitize_identifier(&m.name.text)?;
                let lower = name.to_lowercase();
                let sub_pkg = if pkg.is_empty() {
                    lower
                } else {
                    format!("{pkg}.{lower}")
                };
                let mut chain = module_chain.to_vec();
                chain.push(m.name.text.clone());
                walk_defs(&m.definitions, &sub_pkg, &chain, opts, files)?;
            }
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                let lowered = lower_or_empty(&s.annotations);
                if has_nested_marker(&lowered) {
                    // @nested types are not Topic-Types; skip
                    // TypeSupport-Generation.
                    continue;
                }
                if !struct_is_typesupport_eligible(s) {
                    // Member types outside the current TypeSupport scope
                    // (e.g. `any`, `map<K,V>`, `fixed`,
                    // inheritance base with unsupported members) — the codegen
                    // silently skips the file and leaves the implementation
                    // to the user. This covers the idl4-java spec
                    // §7.7.4 reflection stretch goal.
                    continue;
                }
                let file = emit_typesupport_for_struct(s, pkg, module_chain, opts)?;
                files.push(file);
            }
            _ => {}
        }
    }
    Ok(())
}

fn has_nested_marker(lowered: &Lowered) -> bool {
    lowered
        .builtins
        .iter()
        .any(|b| matches!(b, BuiltinAnnotation::Nested))
}

/// Returns `true` if all member type specs are covered by the TypeSupport codegen.
/// Members with `any`, `Map<K,V>`, `fixed`, or arrays are excluded in v1.0
/// (stretch goal).
fn struct_is_typesupport_eligible(s: &StructDef) -> bool {
    s.members.iter().all(|m| {
        if !typespec_supported(&m.type_spec) {
            return false;
        }
        m.declarators
            .iter()
            .all(|d| matches!(d, Declarator::Simple(_)))
    })
}

/// zerodds-lint: recursion-depth 32 (TypeSpec recursive via Sequence)
fn typespec_supported(ts: &TypeSpec) -> bool {
    match ts {
        TypeSpec::Primitive(_) | TypeSpec::String(_) | TypeSpec::Scoped(_) => true,
        TypeSpec::Sequence(seq) => typespec_supported(&seq.elem),
        TypeSpec::Map(_) | TypeSpec::Fixed(_) | TypeSpec::Any => false,
    }
}

fn emit_typesupport_for_struct(
    s: &StructDef,
    pkg: &str,
    module_chain: &[String],
    opts: &JavaGenOptions,
) -> Result<JavaFile, JavaGenError> {
    let class = sanitize_identifier(&s.name.text)?;
    let support_class = format!("{class}TypeSupport");
    let ind = indent_unit(opts);
    let lowered = lower_or_empty(&s.annotations);
    let extensibility = lowered
        .extensibility()
        .unwrap_or(IdlExtensibility::Appendable);

    let type_name = build_type_name(module_chain, &class);
    let is_keyed = struct_has_key(s);

    let mut body = String::new();
    writeln!(
        body,
        "/** XCDR2 TypeSupport for {class}. Generated by zerodds idl-java. */"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "public final class {support_class} implements org.zerodds.cdr.TopicTypeSupport<{class}> {{"
    )
    .map_err(fmt_err)?;

    writeln!(
        body,
        "{ind}public static final {support_class} INSTANCE = new {support_class}();",
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    writeln!(body, "{ind}private {support_class}() {{}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // getTypeName
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public String getTypeName() {{ return \"{type_name}\"; }}"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // isKeyed
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public boolean isKeyed() {{ return {is_keyed}; }}"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // getExtensibility
    let ext_lit = match extensibility {
        IdlExtensibility::Final => "FINAL",
        IdlExtensibility::Appendable => "APPENDABLE",
        IdlExtensibility::Mutable => "MUTABLE",
    };
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public org.zerodds.cdr.ExtensibilityKind getExtensibility() {{ \
         return org.zerodds.cdr.ExtensibilityKind.{ext_lit}; }}"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // encode(T)
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public byte[] encode({class} sample) {{ return encode(sample, org.zerodds.cdr.EndianMode.LITTLE_ENDIAN); }}"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // encode(T, EndianMode)
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public byte[] encode({class} sample, org.zerodds.cdr.EndianMode endian) {{"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}{ind}org.zerodds.cdr.Xcdr2Writer w = new org.zerodds.cdr.Xcdr2Writer(endian);"
    )
    .map_err(fmt_err)?;
    emit_encode_body(&mut body, s, &ind, extensibility)?;
    writeln!(body, "{ind}{ind}return w.toByteArray();").map_err(fmt_err)?;
    writeln!(body, "{ind}}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // decode(byte[])
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public {class} decode(byte[] bytes) {{ return decode(bytes, 0, bytes.length); }}"
    )
    .map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // decode(byte[], offset, length)
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}public {class} decode(byte[] bytes, int offset, int length) {{"
    )
    .map_err(fmt_err)?;
    writeln!(
        body,
        "{ind}{ind}org.zerodds.cdr.Xcdr2Reader r = new org.zerodds.cdr.Xcdr2Reader(bytes, offset, length, org.zerodds.cdr.EndianMode.LITTLE_ENDIAN);"
    )
    .map_err(fmt_err)?;
    writeln!(body, "{ind}{ind}{class} v = new {class}();").map_err(fmt_err)?;
    emit_decode_body(&mut body, s, &ind, extensibility)?;
    writeln!(body, "{ind}{ind}return v;").map_err(fmt_err)?;
    writeln!(body, "{ind}}}").map_err(fmt_err)?;
    writeln!(body).map_err(fmt_err)?;

    // keyHash
    writeln!(body, "{ind}@Override").map_err(fmt_err)?;
    writeln!(body, "{ind}public byte[] keyHash({class} sample) {{").map_err(fmt_err)?;
    if is_keyed {
        writeln!(
            body,
            "{ind}{ind}org.zerodds.cdr.Xcdr2Writer w = new org.zerodds.cdr.Xcdr2Writer(org.zerodds.cdr.EndianMode.BIG_ENDIAN);"
        )
        .map_err(fmt_err)?;
        emit_key_extraction(&mut body, s, &ind)?;
        // XTypes 1.3 §7.6.8.4: holder ≤ 16 octets -> zero-pad; otherwise MD5.
        writeln!(body, "{ind}{ind}byte[] holder = w.toByteArray();").map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}if (holder.length <= 16) {{").map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}{ind}byte[] h = new byte[16];").map_err(fmt_err)?;
        writeln!(
            body,
            "{ind}{ind}{ind}System.arraycopy(holder, 0, h, 0, holder.length);"
        )
        .map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}{ind}return h;").map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}}}").map_err(fmt_err)?;
        writeln!(body, "{ind}{ind}return org.zerodds.cdr.Md5.hash(holder);").map_err(fmt_err)?;
    } else {
        writeln!(body, "{ind}{ind}return new byte[16];").map_err(fmt_err)?;
    }
    writeln!(body, "{ind}}}").map_err(fmt_err)?;

    writeln!(body, "}}").map_err(fmt_err)?;

    let source = wrap_compilation_unit_default(pkg, &body);
    Ok(JavaFile {
        package_path: pkg.to_string(),
        class_name: support_class,
        source,
    })
}

fn build_type_name(module_chain: &[String], class: &str) -> String {
    if module_chain.is_empty() {
        class.to_string()
    } else {
        let mut parts: Vec<String> = module_chain.to_vec();
        parts.push(class.to_string());
        parts.join("::")
    }
}

fn struct_has_key(s: &StructDef) -> bool {
    s.members.iter().any(member_has_key)
}

fn member_has_key(m: &Member) -> bool {
    let lowered = lower_or_empty(&m.annotations);
    lowered
        .builtins
        .iter()
        .any(|b| matches!(b, BuiltinAnnotation::Key))
}

fn member_is_optional(m: &Member) -> bool {
    let lowered = lower_or_empty(&m.annotations);
    lowered
        .builtins
        .iter()
        .any(|b| matches!(b, BuiltinAnnotation::Optional))
}

fn member_explicit_id(m: &Member) -> Option<u32> {
    let lowered = lower_or_empty(&m.annotations);
    lowered.builtins.iter().find_map(|b| match b {
        BuiltinAnnotation::Id(n) => Some(*n),
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// Encode body emitter
// ---------------------------------------------------------------------------

fn emit_encode_body(
    out: &mut String,
    s: &StructDef,
    ind: &str,
    ext: IdlExtensibility,
) -> Result<(), JavaGenError> {
    let inner = format!("{ind}{ind}");
    match ext {
        IdlExtensibility::Final => {
            for m in &s.members {
                emit_member_encode_inline(out, m, &inner)?;
            }
        }
        IdlExtensibility::Appendable => {
            writeln!(out, "{inner}int __dh = w.beginAppendable();").map_err(fmt_err)?;
            for m in &s.members {
                emit_member_encode_inline(out, m, &inner)?;
            }
            writeln!(out, "{inner}w.endDelimited(__dh);").map_err(fmt_err)?;
        }
        IdlExtensibility::Mutable => {
            writeln!(out, "{inner}int __dh = w.beginMutable();").map_err(fmt_err)?;
            // Auto-id: 1, 2, 3, ... if no @id; honor explicit @id(N).
            let mut auto_id: u32 = 1;
            for m in &s.members {
                let mid = member_explicit_id(m).unwrap_or(auto_id);
                auto_id = mid + 1;
                emit_member_encode_mutable(out, m, &inner, mid)?;
            }
            writeln!(out, "{inner}w.endDelimited(__dh);").map_err(fmt_err)?;
        }
    }
    Ok(())
}

fn emit_member_encode_inline(
    out: &mut String,
    m: &Member,
    inner: &str,
) -> Result<(), JavaGenError> {
    let optional = member_is_optional(m);
    for decl in &m.declarators {
        let name = sanitize_identifier(&decl.name().text)?;
        let getter = format!("sample.get{}()", capitalize(&name));
        if optional {
            writeln!(
                out,
                "{inner}if ({getter} != null && {getter}.isPresent()) {{"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{inner}    w.writePresenceFlag(true);").map_err(fmt_err)?;
            let val_expr = format!("{getter}.get()");
            emit_typespec_encode(out, &m.type_spec, &val_expr, &format!("{inner}    "))?;
            writeln!(out, "{inner}}} else {{").map_err(fmt_err)?;
            writeln!(out, "{inner}    w.writePresenceFlag(false);").map_err(fmt_err)?;
            writeln!(out, "{inner}}}").map_err(fmt_err)?;
        } else {
            emit_typespec_encode(out, &m.type_spec, &getter, inner)?;
        }
    }
    Ok(())
}

fn emit_member_encode_mutable(
    out: &mut String,
    m: &Member,
    inner: &str,
    member_id: u32,
) -> Result<(), JavaGenError> {
    let optional = member_is_optional(m);
    let lc = lc_for_typespec(&m.type_spec);
    for decl in &m.declarators {
        let name = sanitize_identifier(&decl.name().text)?;
        let getter = format!("sample.get{}()", capitalize(&name));
        if optional {
            writeln!(
                out,
                "{inner}if ({getter} != null && {getter}.isPresent()) {{"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{inner}    w.writeEmHeader({member_id}, {lc}, false);")
                .map_err(fmt_err)?;
            let val_expr = format!("{getter}.get()");
            emit_typespec_encode(out, &m.type_spec, &val_expr, &format!("{inner}    "))?;
            writeln!(out, "{inner}}}").map_err(fmt_err)?;
        } else {
            writeln!(out, "{inner}w.writeEmHeader({member_id}, {lc}, false);").map_err(fmt_err)?;
            emit_typespec_encode(out, &m.type_spec, &getter, inner)?;
        }
    }
    Ok(())
}

fn lc_for_typespec(ts: &TypeSpec) -> &'static str {
    match ts {
        TypeSpec::Primitive(p) => match p {
            PrimitiveType::Boolean | PrimitiveType::Octet | PrimitiveType::Char => {
                "org.zerodds.cdr.Xcdr2Writer.LC_BYTE"
            }
            PrimitiveType::WideChar => "org.zerodds.cdr.Xcdr2Writer.LC_SHORT",
            PrimitiveType::Integer(i) => match i {
                IntegerType::Short
                | IntegerType::Int16
                | IntegerType::UShort
                | IntegerType::UInt16 => "org.zerodds.cdr.Xcdr2Writer.LC_SHORT",
                IntegerType::Long
                | IntegerType::Int32
                | IntegerType::ULong
                | IntegerType::UInt32 => "org.zerodds.cdr.Xcdr2Writer.LC_INT32",
                IntegerType::LongLong
                | IntegerType::Int64
                | IntegerType::ULongLong
                | IntegerType::UInt64 => "org.zerodds.cdr.Xcdr2Writer.LC_INT64",
                IntegerType::Int8 | IntegerType::UInt8 => "org.zerodds.cdr.Xcdr2Writer.LC_BYTE",
            },
            PrimitiveType::Floating(f) => match f {
                FloatingType::Float => "org.zerodds.cdr.Xcdr2Writer.LC_INT32",
                FloatingType::Double | FloatingType::LongDouble => {
                    "org.zerodds.cdr.Xcdr2Writer.LC_INT64"
                }
            },
        },
        // String / Sequence / Scoped → variable, NEXTINT-aware.
        _ => "org.zerodds.cdr.Xcdr2Writer.LC_NEXTINT",
    }
}

fn emit_typespec_encode(
    out: &mut String,
    ts: &TypeSpec,
    expr: &str,
    ind: &str,
) -> Result<(), JavaGenError> {
    match ts {
        TypeSpec::Primitive(p) => match p {
            PrimitiveType::Boolean => {
                writeln!(out, "{ind}w.writeBoolean({expr});").map_err(fmt_err)?;
            }
            PrimitiveType::Octet => {
                writeln!(out, "{ind}w.writeOctet({expr});").map_err(fmt_err)?;
            }
            PrimitiveType::Char => {
                writeln!(out, "{ind}w.writeChar({expr});").map_err(fmt_err)?;
            }
            PrimitiveType::WideChar => {
                writeln!(out, "{ind}w.writeWChar({expr});").map_err(fmt_err)?;
            }
            PrimitiveType::Integer(i) => match i {
                IntegerType::Short | IntegerType::Int16 => {
                    writeln!(out, "{ind}w.writeInt16({expr});").map_err(fmt_err)?;
                }
                IntegerType::UShort | IntegerType::UInt16 => {
                    writeln!(out, "{ind}w.writeUInt16({expr});").map_err(fmt_err)?;
                }
                IntegerType::Long | IntegerType::Int32 => {
                    writeln!(out, "{ind}w.writeInt32({expr});").map_err(fmt_err)?;
                }
                IntegerType::ULong | IntegerType::UInt32 => {
                    writeln!(out, "{ind}w.writeUInt32({expr});").map_err(fmt_err)?;
                }
                IntegerType::LongLong | IntegerType::Int64 => {
                    writeln!(out, "{ind}w.writeInt64({expr});").map_err(fmt_err)?;
                }
                IntegerType::ULongLong | IntegerType::UInt64 => {
                    writeln!(out, "{ind}w.writeUInt64({expr});").map_err(fmt_err)?;
                }
                IntegerType::Int8 => {
                    writeln!(out, "{ind}w.writeOctet({expr});").map_err(fmt_err)?;
                }
                IntegerType::UInt8 => {
                    writeln!(out, "{ind}w.writeUInt8({expr});").map_err(fmt_err)?;
                }
            },
            PrimitiveType::Floating(f) => match f {
                FloatingType::Float => {
                    writeln!(out, "{ind}w.writeFloat32({expr});").map_err(fmt_err)?;
                }
                FloatingType::Double | FloatingType::LongDouble => {
                    writeln!(out, "{ind}w.writeFloat64({expr});").map_err(fmt_err)?;
                }
            },
        },
        TypeSpec::String(s) => {
            // Bounded `string<N>` (DDS-XTypes §7.4.3): reject over-bound on
            // encode like strict vendors do. Narrow → UTF-8 byte length (matches
            // the CDR wire); wide `wstring<N>` → UTF-16 unit count (String.length).
            if let Some(b) = &s.bound {
                let bv = crate::emitter::const_expr_to_java(b);
                if s.wide {
                    writeln!(
                        out,
                        "{ind}if ({expr} != null && {expr}.length() > {bv}) throw new IllegalArgumentException(\"bounded wstring length exceeds its IDL bound ({bv})\");"
                    )
                    .map_err(fmt_err)?;
                } else {
                    writeln!(
                        out,
                        "{ind}if ({expr} != null && {expr}.getBytes(java.nio.charset.StandardCharsets.UTF_8).length > {bv}) throw new IllegalArgumentException(\"bounded string length exceeds its IDL bound ({bv})\");"
                    )
                    .map_err(fmt_err)?;
                }
            }
            writeln!(out, "{ind}w.writeString({expr});").map_err(fmt_err)?;
        }
        TypeSpec::Sequence(seq) => {
            // XCDR2 §7.4.3.5: non-primitive elements (string, struct, …)
            // → DHEADER (uint32 = byte length of [count + elements]) prepended;
            // primitives do not get one. Verified against CycloneDDS (V-5 without, V-6 with).
            let non_primitive = !matches!(&*seq.elem, TypeSpec::Primitive(_));
            writeln!(out, "{ind}{{").map_err(fmt_err)?;
            writeln!(
                out,
                "{ind}    java.util.List<?> __seq = ({expr} == null) ? java.util.Collections.emptyList() : ({expr});"
            )
            .map_err(fmt_err)?;
            // Bounded `sequence<T, N>` (DDS-XTypes §7.4.3): over-bound = encode error.
            if let Some(b) = &seq.bound {
                let bv = crate::emitter::const_expr_to_java(b);
                writeln!(
                    out,
                    "{ind}    if (__seq.size() > {bv}) throw new IllegalArgumentException(\"bounded sequence length exceeds its IDL bound ({bv})\");"
                )
                .map_err(fmt_err)?;
            }
            if non_primitive {
                writeln!(out, "{ind}    int __seqdh = w.beginAppendable();").map_err(fmt_err)?;
            }
            writeln!(out, "{ind}    w.writeSequenceCount(__seq.size());").map_err(fmt_err)?;
            writeln!(out, "{ind}    for (Object __el : __seq) {{").map_err(fmt_err)?;
            let inner_indent = format!("{ind}        ");
            // Element encode in two steps: cast to element type,
            // then encode.
            let elem_expr = "__el";
            emit_seq_element_encode(out, &seq.elem, elem_expr, &inner_indent)?;
            writeln!(out, "{ind}    }}").map_err(fmt_err)?;
            if non_primitive {
                writeln!(out, "{ind}    w.endDelimited(__seqdh);").map_err(fmt_err)?;
            }
            writeln!(out, "{ind}}}").map_err(fmt_err)?;
        }
        TypeSpec::Scoped(_) => {
            // Nested struct: delegate.
            // We import TypeSupport via FQN. The Java code calls
            // <Type>TypeSupport.INSTANCE.encode(expr) and writes the
            // bytes as a raw payload.
            // Simplified form: write the bytes directly.
            let scoped_str = match ts {
                TypeSpec::Scoped(s) => scoped_to_short(s),
                _ => {
                    return Err(JavaGenError::UnsupportedConstruct {
                        construct: "non-scoped nested type in TypeSupport encode".into(),
                        context: None,
                    });
                }
            };
            writeln!(
                out,
                "{ind}w.writeBytes({scoped_str}TypeSupport.INSTANCE.encode({expr}, endian));"
            )
            .map_err(fmt_err)?;
        }
        _ => {
            return Err(JavaGenError::UnsupportedConstruct {
                construct: "typesupport-encode for this TypeSpec".into(),
                context: None,
            });
        }
    }
    Ok(())
}

fn emit_seq_element_encode(
    out: &mut String,
    elem: &TypeSpec,
    expr: &str,
    ind: &str,
) -> Result<(), JavaGenError> {
    match elem {
        TypeSpec::Primitive(p) => match p {
            PrimitiveType::Boolean => {
                writeln!(out, "{ind}w.writeBoolean((Boolean) {expr});").map_err(fmt_err)?;
            }
            PrimitiveType::Octet => {
                writeln!(out, "{ind}w.writeOctet((Byte) {expr});").map_err(fmt_err)?;
            }
            PrimitiveType::Char | PrimitiveType::WideChar => {
                writeln!(out, "{ind}w.writeChar((Character) {expr});").map_err(fmt_err)?;
            }
            PrimitiveType::Integer(i) => match i {
                IntegerType::Short | IntegerType::Int16 => {
                    writeln!(out, "{ind}w.writeInt16((Short) {expr});").map_err(fmt_err)?;
                }
                IntegerType::UShort | IntegerType::UInt16 => {
                    writeln!(out, "{ind}w.writeUInt16((Integer) {expr});").map_err(fmt_err)?;
                }
                IntegerType::Long | IntegerType::Int32 => {
                    writeln!(out, "{ind}w.writeInt32((Integer) {expr});").map_err(fmt_err)?;
                }
                IntegerType::ULong | IntegerType::UInt32 => {
                    writeln!(out, "{ind}w.writeUInt32((Long) {expr});").map_err(fmt_err)?;
                }
                IntegerType::LongLong | IntegerType::Int64 => {
                    writeln!(out, "{ind}w.writeInt64((Long) {expr});").map_err(fmt_err)?;
                }
                IntegerType::ULongLong | IntegerType::UInt64 => {
                    writeln!(out, "{ind}w.writeUInt64((Long) {expr});").map_err(fmt_err)?;
                }
                IntegerType::Int8 => {
                    writeln!(out, "{ind}w.writeOctet((Byte) {expr});").map_err(fmt_err)?;
                }
                IntegerType::UInt8 => {
                    writeln!(out, "{ind}w.writeUInt8((Short) {expr});").map_err(fmt_err)?;
                }
            },
            PrimitiveType::Floating(f) => match f {
                FloatingType::Float => {
                    writeln!(out, "{ind}w.writeFloat32((Float) {expr});").map_err(fmt_err)?;
                }
                FloatingType::Double | FloatingType::LongDouble => {
                    writeln!(out, "{ind}w.writeFloat64((Double) {expr});").map_err(fmt_err)?;
                }
            },
        },
        TypeSpec::String(_) => {
            writeln!(out, "{ind}w.writeString((String) {expr});").map_err(fmt_err)?;
        }
        TypeSpec::Scoped(s) => {
            let cls = scoped_to_short(s);
            writeln!(
                out,
                "{ind}w.writeBytes({cls}TypeSupport.INSTANCE.encode(({cls}) {expr}, endian));"
            )
            .map_err(fmt_err)?;
        }
        _ => {
            return Err(JavaGenError::UnsupportedConstruct {
                construct: "typesupport-encode element-type".into(),
                context: None,
            });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Decode body emitter
// ---------------------------------------------------------------------------

fn emit_decode_body(
    out: &mut String,
    s: &StructDef,
    ind: &str,
    ext: IdlExtensibility,
) -> Result<(), JavaGenError> {
    let inner = format!("{ind}{ind}");
    match ext {
        IdlExtensibility::Final => {
            for m in &s.members {
                emit_member_decode_inline(out, m, &inner)?;
            }
        }
        IdlExtensibility::Appendable => {
            writeln!(out, "{inner}int __dhSize = r.readDHeader();").map_err(fmt_err)?;
            writeln!(out, "{inner}int __endPos = r.position() + __dhSize;").map_err(fmt_err)?;
            for m in &s.members {
                emit_member_decode_inline(out, m, &inner)?;
            }
            writeln!(
                out,
                "{inner}while (r.position() < __endPos) {{ r.skip(1); }}"
            )
            .map_err(fmt_err)?;
        }
        IdlExtensibility::Mutable => {
            writeln!(out, "{inner}int __dhSize = r.readDHeader();").map_err(fmt_err)?;
            writeln!(out, "{inner}int __endPos = r.position() + __dhSize;").map_err(fmt_err)?;
            writeln!(out, "{inner}while (r.position() < __endPos) {{").map_err(fmt_err)?;
            writeln!(
                out,
                "{inner}    org.zerodds.cdr.Xcdr2Reader.EmHeader __em = r.readEmHeader();"
            )
            .map_err(fmt_err)?;
            // Auto-id: 1, 2, 3, ... if no @id.
            let mut auto_id: u32 = 1;
            let mut first = true;
            for m in &s.members {
                let mid = member_explicit_id(m).unwrap_or(auto_id);
                auto_id = mid + 1;
                emit_member_decode_mutable_branch(out, m, &inner, mid, first)?;
                first = false;
            }
            // Default branch: skip-by-LC.
            writeln!(out, "{inner}    else {{").map_err(fmt_err)?;
            writeln!(out, "{inner}        skipByLc(r, __em.lc);").map_err(fmt_err)?;
            writeln!(out, "{inner}    }}").map_err(fmt_err)?;
            writeln!(out, "{inner}}}").map_err(fmt_err)?;
        }
    }
    Ok(())
}

fn emit_member_decode_inline(
    out: &mut String,
    m: &Member,
    inner: &str,
) -> Result<(), JavaGenError> {
    let optional = member_is_optional(m);
    for decl in &m.declarators {
        let name = sanitize_identifier(&decl.name().text)?;
        let setter = format!("v.set{}", capitalize(&name));
        if optional {
            writeln!(out, "{inner}if (r.readPresenceFlag()) {{").map_err(fmt_err)?;
            let read_expr = read_expr_for_typespec(&m.type_spec)?;
            writeln!(
                out,
                "{inner}    {setter}(java.util.Optional.of({read_expr}));"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{inner}}} else {{").map_err(fmt_err)?;
            writeln!(out, "{inner}    {setter}(java.util.Optional.empty());").map_err(fmt_err)?;
            writeln!(out, "{inner}}}").map_err(fmt_err)?;
        } else {
            emit_typespec_decode(out, &m.type_spec, &setter, inner)?;
        }
    }
    Ok(())
}

fn emit_member_decode_mutable_branch(
    out: &mut String,
    m: &Member,
    inner: &str,
    member_id: u32,
    first: bool,
) -> Result<(), JavaGenError> {
    let optional = member_is_optional(m);
    let kw = if first { "if" } else { "else if" };
    for decl in &m.declarators {
        let name = sanitize_identifier(&decl.name().text)?;
        let setter = format!("v.set{}", capitalize(&name));
        writeln!(out, "{inner}    {kw} (__em.memberId == {member_id}) {{").map_err(fmt_err)?;
        if optional {
            let read_expr = read_expr_for_typespec(&m.type_spec)?;
            writeln!(
                out,
                "{inner}        {setter}(java.util.Optional.of({read_expr}));"
            )
            .map_err(fmt_err)?;
        } else {
            emit_typespec_decode(out, &m.type_spec, &setter, &format!("{inner}        "))?;
        }
        writeln!(out, "{inner}    }}").map_err(fmt_err)?;
    }
    Ok(())
}

fn emit_typespec_decode(
    out: &mut String,
    ts: &TypeSpec,
    setter: &str,
    ind: &str,
) -> Result<(), JavaGenError> {
    match ts {
        TypeSpec::Primitive(_) | TypeSpec::String(_) => {
            let read = read_expr_for_typespec(ts)?;
            writeln!(out, "{ind}{setter}({read});").map_err(fmt_err)?;
        }
        TypeSpec::Sequence(seq) => {
            let elem_ty = boxed_for_seq_elem(&seq.elem);
            // XCDR2 §7.4.3.5: skip DHEADER for non-primitive elements.
            let non_primitive = !matches!(&*seq.elem, TypeSpec::Primitive(_));
            writeln!(out, "{ind}{{").map_err(fmt_err)?;
            if non_primitive {
                writeln!(out, "{ind}    r.readDHeader();").map_err(fmt_err)?;
            }
            writeln!(out, "{ind}    int __cnt = r.readSequenceCount();").map_err(fmt_err)?;
            writeln!(
                out,
                "{ind}    java.util.List<{elem_ty}> __out = new java.util.ArrayList<>(__cnt);"
            )
            .map_err(fmt_err)?;
            writeln!(out, "{ind}    for (int __i = 0; __i < __cnt; __i++) {{").map_err(fmt_err)?;
            emit_seq_element_decode(out, &seq.elem, &format!("{ind}        "))?;
            writeln!(out, "{ind}    }}").map_err(fmt_err)?;
            writeln!(out, "{ind}    {setter}(__out);").map_err(fmt_err)?;
            writeln!(out, "{ind}}}").map_err(fmt_err)?;
        }
        TypeSpec::Scoped(s) => {
            let cls = scoped_to_short(s);
            writeln!(
                out,
                "{ind}{setter}({cls}TypeSupport.INSTANCE.decode(r.readBytes(r.remaining())));"
            )
            .map_err(fmt_err)?;
        }
        _ => {
            return Err(JavaGenError::UnsupportedConstruct {
                construct: "typesupport-decode for this TypeSpec".into(),
                context: None,
            });
        }
    }
    Ok(())
}

fn read_expr_for_typespec(ts: &TypeSpec) -> Result<String, JavaGenError> {
    Ok(match ts {
        TypeSpec::Primitive(p) => match p {
            PrimitiveType::Boolean => "r.readBoolean()".into(),
            PrimitiveType::Octet => "r.readOctet()".into(),
            PrimitiveType::Char => "r.readChar()".into(),
            PrimitiveType::WideChar => "r.readWChar()".into(),
            PrimitiveType::Integer(i) => match i {
                IntegerType::Short | IntegerType::Int16 => "r.readInt16()".into(),
                IntegerType::UShort | IntegerType::UInt16 => "r.readUInt16()".into(),
                IntegerType::Long | IntegerType::Int32 => "r.readInt32()".into(),
                IntegerType::ULong | IntegerType::UInt32 => "r.readUInt32()".into(),
                IntegerType::LongLong | IntegerType::Int64 => "r.readInt64()".into(),
                IntegerType::ULongLong | IntegerType::UInt64 => "r.readUInt64()".into(),
                IntegerType::Int8 => "r.readOctet()".into(),
                IntegerType::UInt8 => "(short) r.readUInt8()".into(),
            },
            PrimitiveType::Floating(f) => match f {
                FloatingType::Float => "r.readFloat32()".into(),
                FloatingType::Double | FloatingType::LongDouble => "r.readFloat64()".into(),
            },
        },
        TypeSpec::String(_) => "r.readString()".into(),
        _ => {
            return Err(JavaGenError::UnsupportedConstruct {
                construct: "typesupport-read inline expr".into(),
                context: None,
            });
        }
    })
}

fn boxed_for_seq_elem(elem: &TypeSpec) -> String {
    match elem {
        TypeSpec::Primitive(p) => match p {
            PrimitiveType::Boolean => "Boolean".into(),
            PrimitiveType::Octet => "Byte".into(),
            PrimitiveType::Char | PrimitiveType::WideChar => "Character".into(),
            PrimitiveType::Integer(i) => match i {
                IntegerType::Short | IntegerType::Int16 => "Short".into(),
                IntegerType::UShort | IntegerType::UInt16 => "Integer".into(),
                IntegerType::Long | IntegerType::Int32 => "Integer".into(),
                IntegerType::ULong | IntegerType::UInt32 => "Long".into(),
                IntegerType::LongLong | IntegerType::Int64 => "Long".into(),
                IntegerType::ULongLong | IntegerType::UInt64 => "Long".into(),
                IntegerType::Int8 => "Byte".into(),
                IntegerType::UInt8 => "Short".into(),
            },
            PrimitiveType::Floating(f) => match f {
                FloatingType::Float => "Float".into(),
                FloatingType::Double | FloatingType::LongDouble => "Double".into(),
            },
        },
        TypeSpec::String(_) => "String".into(),
        TypeSpec::Scoped(s) => scoped_to_short(s),
        _ => "Object".into(),
    }
}

fn emit_seq_element_decode(
    out: &mut String,
    elem: &TypeSpec,
    ind: &str,
) -> Result<(), JavaGenError> {
    match elem {
        TypeSpec::Primitive(_) | TypeSpec::String(_) => {
            let read = read_expr_for_typespec(elem)?;
            writeln!(out, "{ind}__out.add({read});").map_err(fmt_err)?;
        }
        TypeSpec::Scoped(s) => {
            let cls = scoped_to_short(s);
            writeln!(
                out,
                "{ind}__out.add({cls}TypeSupport.INSTANCE.decode(r.readBytes(r.remaining())));"
            )
            .map_err(fmt_err)?;
        }
        _ => {
            return Err(JavaGenError::UnsupportedConstruct {
                construct: "typesupport-decode seq-element".into(),
                context: None,
            });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Key-Hash extraction
// ---------------------------------------------------------------------------

fn emit_key_extraction(out: &mut String, s: &StructDef, ind: &str) -> Result<(), JavaGenError> {
    let inner = format!("{ind}{ind}");
    for m in &s.members {
        if member_has_key(m) {
            for decl in &m.declarators {
                let name = sanitize_identifier(&decl.name().text)?;
                let getter = format!("sample.get{}()", capitalize(&name));
                emit_typespec_encode(out, &m.type_spec, &getter, &inner)?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn scoped_to_short(s: &zerodds_idl::ast::ScopedName) -> String {
    s.parts.last().map(|p| p.text.clone()).unwrap_or_default()
}

// Suppress warnings for the unused helper.
#[allow(dead_code)]
fn _unused(p: PrimitiveType) -> &'static str {
    primitive_to_java(p)
}

#[allow(dead_code)]
fn _unused_decl(_d: &Declarator) {}
