// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL -> native endpoint SDK codegen (ADR 0013), C89 target.
//!
//! Parses an IDL file with the vendored ZeroDDS parser (`zerodds_idl`) and
//! emits the `wire-fixed` codec (C89 `<name>_encode`/`<name>_decode` over the
//! zdw wire-core primitives) per struct, byte-identical to the Rust core.
//!
//! Covered: primitives (int/float/bool/octet), string, sequence<octet>, nested
//! structs, sequence<struct>, and the three extensibility modes -- `@final`
//! (plain), `@appendable` (DHEADER), `@mutable` (DHEADER + per-member EMHEADER
//! LC4). The extensibility default for an unannotated struct is APPENDABLE
//! (OMG XTypes 1.3 section 7.3.3.1), matching the ZeroDDS Rust codegen.
//!
//! usage: zerodds-endpoint-codegen <input.idl> <out-dir>

#![allow(
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::panic
)]

use std::collections::BTreeSet;
use std::fmt::Write as _;

use zerodds_idl::ast::types::{
    Annotation, AnnotationParams, ConstExpr, ConstrTypeDecl, Declarator, Definition, FloatingType,
    IntegerType, LiteralKind, PrimitiveType, Specification, StructDcl, StructDef, TypeDecl,
    TypeSpec,
};
use zerodds_idl::config::ParserConfig;

const CAP: usize = 256;
const SEQ_STRUCT_CAP: usize = 16;

#[derive(Clone, Copy, PartialEq)]
enum Ext {
    Final,
    Appendable,
    Mutable,
}

fn last(name: &zerodds_idl::ast::types::ScopedName) -> String {
    name.parts
        .last()
        .map(|p| p.text.clone())
        .unwrap_or_default()
}

/// Extensibility from annotations; default APPENDABLE (spec SX2).
fn extensibility(anns: &[Annotation]) -> Ext {
    for a in anns {
        match last(&a.name).as_str() {
            "final" | "Final" => return Ext::Final,
            "appendable" | "Appendable" => return Ext::Appendable,
            "mutable" | "Mutable" => return Ext::Mutable,
            "extensibility" | "Extensibility" => {
                if let AnnotationParams::Single(ConstExpr::Scoped(sn)) = &a.params {
                    match last(sn).to_ascii_uppercase().as_str() {
                        "FINAL" => return Ext::Final,
                        "APPENDABLE" => return Ext::Appendable,
                        "MUTABLE" => return Ext::Mutable,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ext::Appendable
}

/// Explicit @id(N) on a member, if any.
fn member_id(anns: &[Annotation]) -> Option<u32> {
    for a in anns {
        if last(&a.name) == "id" {
            if let AnnotationParams::Single(ConstExpr::Literal(lit)) = &a.params {
                if lit.kind == LiteralKind::Integer {
                    return lit.raw.trim().parse::<u32>().ok();
                }
            }
        }
    }
    None
}

fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn cap_macro(prefix: &str, field: &str) -> String {
    format!("{}_{}_CAP", prefix.to_uppercase(), field.to_uppercase())
}

/// The C field declaration + encode/decode statements for one field, appended
/// to the given buffers. Returns false on an unsupported type.
fn emit_field(
    structs: &BTreeSet<String>,
    prefix: &str,
    ts: &TypeSpec,
    field: &str,
    hdr: &mut String,
    fields: &mut String,
    enc: &mut String,
    dec: &mut String,
) -> bool {
    let cap = cap_macro(prefix, field);
    match ts {
        TypeSpec::Primitive(p) => {
            let (cty, put, get) = match prim_calls(*p) {
                Some(t) => t,
                None => return false,
            };
            let _ = writeln!(fields, "    {cty} {field};");
            let _ = writeln!(enc, "    {}", put.replace("{n}", field));
            let _ = writeln!(dec, "    {}", get.replace("{n}", field));
            true
        }
        TypeSpec::String(s) if !s.wide => {
            let _ = writeln!(hdr, "#define {cap} {CAP}");
            let _ = writeln!(fields, "    char {field}[{cap}];");
            let _ = writeln!(enc, "    zdw_put_string(w, s->{field});");
            let _ = writeln!(dec, "    zdw_get_string(r, s->{field}, {cap});");
            true
        }
        TypeSpec::Scoped(sn) if structs.contains(&last(sn)) => {
            let ref_ty = last(sn);
            let ref_pfx = snake(&ref_ty);
            let _ = writeln!(fields, "    {ref_ty} {field};");
            let _ = writeln!(enc, "    {ref_pfx}_encode(w, &s->{field});");
            let _ = writeln!(dec, "    {ref_pfx}_decode(r, &s->{field});");
            true
        }
        TypeSpec::Sequence(seq) => match seq.elem.as_ref() {
            TypeSpec::Primitive(PrimitiveType::Octet)
            | TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::UInt8)) => {
                let _ = writeln!(hdr, "#define {cap} {CAP}");
                let _ = writeln!(fields, "    unsigned char {field}[{cap}];");
                let _ = writeln!(fields, "    size_t {field}_len;");
                let _ = writeln!(enc, "    zdw_put_seq_u8(w, s->{field}, s->{field}_len);");
                let _ = writeln!(
                    dec,
                    "    zdw_get_seq_u8(r, s->{field}, {cap}, &s->{field}_len);"
                );
                true
            }
            TypeSpec::Scoped(sn) if structs.contains(&last(sn)) => {
                // sequence<struct>: collection DHEADER + u32 count + elements.
                let ref_ty = last(sn);
                let ref_pfx = snake(&ref_ty);
                let _ = writeln!(hdr, "#define {cap} {SEQ_STRUCT_CAP}");
                let _ = writeln!(fields, "    {ref_ty} {field}[{cap}];");
                let _ = writeln!(fields, "    size_t {field}_len;");
                let _ = writeln!(enc, "    {{ size_t _i; size_t _d = zdw_dheader_begin(w);");
                let _ = writeln!(enc, "      zdw_put_u32(w, (unsigned long)s->{field}_len);");
                let _ = writeln!(enc, "      for (_i = 0; _i < s->{field}_len; _i++)");
                let _ = writeln!(enc, "        {ref_pfx}_encode(w, &s->{field}[_i]);");
                let _ = writeln!(enc, "      zdw_dheader_end(w, _d); }}");
                let _ = writeln!(dec, "    {{ size_t _i; unsigned long _dh = 0, _c = 0;");
                let _ = writeln!(dec, "      zdw_dheader_read(r, &_dh); zdw_get_u32(r, &_c);");
                let _ = writeln!(dec, "      s->{field}_len = (size_t)_c;");
                let _ = writeln!(
                    dec,
                    "      for (_i = 0; _i < (size_t)_c && _i < {cap}; _i++)"
                );
                let _ = writeln!(dec, "        {ref_pfx}_decode(r, &s->{field}[_i]); }}");
                true
            }
            _ => false,
        },
        _ => false,
    }
}

fn prim_calls(p: PrimitiveType) -> Option<(&'static str, &'static str, &'static str)> {
    use IntegerType::*;
    Some(match p {
        PrimitiveType::Octet | PrimitiveType::Char | PrimitiveType::Integer(Int8 | UInt8) => (
            "unsigned char",
            "zdw_put_u8(w, s->{n});",
            "zdw_get_u8(r, &s->{n});",
        ),
        PrimitiveType::Integer(Short | UShort | Int16 | UInt16) => (
            "unsigned int",
            "zdw_put_u16(w, s->{n});",
            "zdw_get_u16(r, &s->{n});",
        ),
        PrimitiveType::Integer(Long | ULong | Int32 | UInt32) => (
            "unsigned long",
            "zdw_put_u32(w, s->{n});",
            "zdw_get_u32(r, &s->{n});",
        ),
        PrimitiveType::Integer(LongLong | ULongLong | Int64 | UInt64) => (
            "zdw_u64_t",
            "zdw_put_u64(w, s->{n});",
            "zdw_get_u64(r, &s->{n});",
        ),
        PrimitiveType::Floating(FloatingType::Float) => (
            "float",
            "zdw_put_f32(w, s->{n});",
            "zdw_get_f32(r, &s->{n});",
        ),
        PrimitiveType::Floating(FloatingType::Double) => (
            "double",
            "zdw_put_f64(w, s->{n});",
            "zdw_get_f64(r, &s->{n});",
        ),
        PrimitiveType::Boolean => (
            "int",
            "zdw_put_bool(w, s->{n});",
            "zdw_get_bool(r, &s->{n});",
        ),
        _ => return None,
    })
}

fn emit_struct(structs: &BTreeSet<String>, hdr: &mut String, src: &mut String, s: &StructDef) {
    let name = &s.name.text;
    let prefix = snake(name);
    let ext = extensibility(&s.annotations);

    let mut fields = String::new();
    // Per-member encode/decode fragments (so @mutable can wrap each in an EMHEADER).
    let mut members: Vec<(Option<u32>, bool, String, String)> = Vec::new();
    let mut seq_index: u32 = 0;
    for member in &s.members {
        let explicit_id = member_id(&member.annotations);
        for decl in &member.declarators {
            let Declarator::Simple(id) = decl else {
                panic!("array declarators not in the C89 subset yet");
            };
            let field = &id.text;
            let mut enc = String::new();
            let mut dec = String::new();
            if !emit_field(
                structs,
                &prefix,
                &member.type_spec,
                field,
                hdr,
                &mut fields,
                &mut enc,
                &mut dec,
            ) {
                panic!("unsupported member type for {name}.{field}");
            }
            let mid = explicit_id.unwrap_or(seq_index);
            members.push((Some(mid), false, enc, dec));
            seq_index += 1;
        }
    }

    // Assemble encode/decode bodies per extensibility mode.
    let mut encode = String::new();
    let mut decode = String::new();
    match ext {
        Ext::Final => {
            for (_, _, e, d) in &members {
                encode.push_str(e);
                decode.push_str(d);
            }
        }
        Ext::Appendable => {
            let _ = writeln!(encode, "    {{ size_t _d = zdw_dheader_begin(w);");
            for (_, _, e, _) in &members {
                encode.push_str(e);
            }
            let _ = writeln!(encode, "    zdw_dheader_end(w, _d); }}");
            let _ = writeln!(
                decode,
                "    {{ unsigned long _dh = 0; zdw_dheader_read(r, &_dh);"
            );
            for (_, _, _, d) in &members {
                decode.push_str(d);
            }
            let _ = writeln!(decode, "    }}");
        }
        Ext::Mutable => {
            let _ = writeln!(
                encode,
                "    {{ size_t _d = zdw_dheader_begin(w); size_t _e;"
            );
            for (mid, mu, e, _) in &members {
                let id = mid.unwrap_or(0);
                let muf = if *mu { 1 } else { 0 };
                let _ = writeln!(encode, "    _e = zdw_emheader_begin(w, {id}uL, {muf});");
                encode.push_str(e);
                let _ = writeln!(encode, "    zdw_emheader_end(w, _e);");
            }
            let _ = writeln!(encode, "    zdw_dheader_end(w, _d); }}");
            // Decode: @mutable members in any order, dispatch by member-ID.
            let _ = writeln!(
                decode,
                "    {{ unsigned long _dh = 0, _id = 0, _ni = 0; int _mu = 0; size_t _start;"
            );
            let _ = writeln!(decode, "      zdw_dheader_read(r, &_dh); _start = r->pos;");
            let _ = writeln!(
                decode,
                "      while (r->pos - _start < (size_t)_dh && r->error == ZDW_OK) {{"
            );
            let _ = writeln!(
                decode,
                "        if (zdw_emheader_read(r, &_id, &_mu, &_ni) != ZDW_OK) break;"
            );
            let mut first = true;
            for (mid, _, _, d) in &members {
                let id = mid.unwrap_or(0);
                let kw = if first { "if" } else { "else if" };
                first = false;
                let _ = writeln!(decode, "        {kw} (_id == {id}uL) {{");
                decode.push_str(&d.replace("\n", "\n  "));
                let _ = writeln!(decode, "        }}");
            }
            let _ = writeln!(
                decode,
                "        else {{ unsigned long _j; unsigned char _sk;"
            );
            let _ = writeln!(
                decode,
                "               for (_j = 0; _j < _ni; _j++) zdw_get_u8(r, &_sk); }}"
            );
            let _ = writeln!(decode, "      }} }}");
        }
    }

    let _ = writeln!(hdr, "\ntypedef struct {{\n{fields}}} {name};\n");
    let _ = writeln!(hdr, "int {prefix}_encode(zdw_writer *w, const {name} *s);");
    let _ = writeln!(hdr, "int {prefix}_decode(zdw_reader *r, {name} *s);");

    let _ = writeln!(
        src,
        "int {prefix}_encode(zdw_writer *w, const {name} *s)\n{{\n{encode}    return w->error;\n}}\n"
    );
    let _ = writeln!(
        src,
        "int {prefix}_decode(zdw_reader *r, {name} *s)\n{{\n{decode}    return r->error;\n}}"
    );
}

/// The Python field encode/decode expressions for a member (mirrors the C
/// emitter; uses the zerodds_wire.py + collection-DHEADER helpers).
fn py_field(
    structs: &BTreeSet<String>,
    ts: &TypeSpec,
    field: &str,
    enc: &mut String,
    dec: &mut String,
) -> bool {
    let (e, d) = match ts {
        TypeSpec::Primitive(p) => {
            let call = match p {
                PrimitiveType::Octet
                | PrimitiveType::Char
                | PrimitiveType::Integer(IntegerType::Int8 | IntegerType::UInt8) => "u8",
                PrimitiveType::Integer(
                    IntegerType::Short
                    | IntegerType::UShort
                    | IntegerType::Int16
                    | IntegerType::UInt16,
                ) => "u16",
                PrimitiveType::Integer(
                    IntegerType::Long
                    | IntegerType::ULong
                    | IntegerType::Int32
                    | IntegerType::UInt32,
                ) => "u32",
                PrimitiveType::Integer(
                    IntegerType::LongLong
                    | IntegerType::ULongLong
                    | IntegerType::Int64
                    | IntegerType::UInt64,
                ) => "u64",
                PrimitiveType::Floating(FloatingType::Float) => "f32",
                PrimitiveType::Floating(FloatingType::Double) => "f64",
                PrimitiveType::Boolean => "bool",
                _ => return false,
            };
            (
                format!("w.put_{call}(s['{field}'])"),
                format!("d['{field}'] = r.get_{call}()"),
            )
        }
        TypeSpec::String(st) if !st.wide => (
            format!("w.put_string(s['{field}'])"),
            format!("d['{field}'] = r.get_string()"),
        ),
        TypeSpec::Scoped(sn) if structs.contains(&last(sn)) => {
            let p = snake(&last(sn));
            (
                format!("{p}_encode(w, s['{field}'])"),
                format!("d['{field}'] = {p}_decode(r)"),
            )
        }
        TypeSpec::Sequence(seq) => match seq.elem.as_ref() {
            TypeSpec::Primitive(PrimitiveType::Octet)
            | TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::UInt8)) => (
                format!("w.put_seq_u8(s['{field}'])"),
                format!("d['{field}'] = r.get_seq_u8()"),
            ),
            TypeSpec::Scoped(sn) if structs.contains(&last(sn)) => {
                let p = snake(&last(sn));
                (
                    format!(
                        "_b = w.dheader_begin(); w.put_u32(len(s['{field}']))\n        for _e in s['{field}']: {p}_encode(w, _e)\n        w.dheader_end(_b)"
                    ),
                    format!(
                        "r.dheader_read(); _n = r.get_u32(); d['{field}'] = [{p}_decode(r) for _ in range(_n)]"
                    ),
                )
            }
            _ => return false,
        },
        _ => return false,
    };
    enc.push_str(&format!("    {e}\n"));
    dec.push_str(&format!("    {d}\n"));
    true
}

fn emit_struct_py(structs: &BTreeSet<String>, out: &mut String, s: &StructDef) {
    let name = &s.name.text;
    let prefix = snake(name);
    let ext = extensibility(&s.annotations);
    let mut enc = String::new();
    let mut dec = String::new();
    let mut mids: Vec<u32> = Vec::new();
    let mut idx = 0u32;
    for m in &s.members {
        let mid = member_id(&m.annotations).unwrap_or(idx);
        for decl in &m.declarators {
            let Declarator::Simple(id) = decl else {
                panic!("array declarator")
            };
            if !py_field(structs, &m.type_spec, &id.text, &mut enc, &mut dec) {
                panic!("unsupported type in {name}.{}", id.text);
            }
            mids.push(mid);
            idx += 1;
        }
    }
    let _ = writeln!(out, "\ndef {prefix}_encode(w, s):");
    match ext {
        Ext::Final => {
            out.push_str(&enc);
        }
        Ext::Appendable => {
            let _ = writeln!(out, "    _d = w.dheader_begin()");
            out.push_str(&enc);
            let _ = writeln!(out, "    w.dheader_end(_d)");
        }
        Ext::Mutable => {
            let _ = writeln!(out, "    _d = w.dheader_begin()");
            // one emheader per member: re-emit fields wrapped
            for (i, line) in enc.lines().enumerate() {
                let _ = writeln!(out, "    _e = w.emheader_begin({}, 0)", mids[i]);
                let _ = writeln!(out, "    {}", line.trim_start());
                let _ = writeln!(out, "    w.emheader_end(_e)");
            }
            let _ = writeln!(out, "    w.dheader_end(_d)");
        }
    }
    let _ = writeln!(out, "\ndef {prefix}_decode(r):");
    let _ = writeln!(out, "    d = {{}}");
    match ext {
        Ext::Final => {
            out.push_str(&dec);
        }
        Ext::Appendable => {
            let _ = writeln!(out, "    r.dheader_read()");
            out.push_str(&dec);
        }
        Ext::Mutable => {
            let _ = writeln!(out, "    _dh = r.dheader_read(); _start = r.pos");
            let _ = writeln!(out, "    while r.pos - _start < _dh:");
            let _ = writeln!(out, "        _id, _mu, _ni = r.emheader_read()");
            for (i, line) in dec.lines().enumerate() {
                let kw = if i == 0 { "if" } else { "elif" };
                let _ = writeln!(out, "        {kw} _id == {}:", mids[i]);
                let _ = writeln!(out, "            {}", line.trim_start());
            }
            let _ = writeln!(out, "        else:");
            let _ = writeln!(out, "            r.get_bytes(_ni)");
        }
    }
    let _ = writeln!(out, "    return d");
}

fn generate_py(spec: &Specification) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Generated by zerodds-endpoint-codegen (ADR 0013). DO NOT EDIT."
    );
    let _ = writeln!(out, "from __future__ import print_function");
    let mut structs = Vec::new();
    collect_structs(&spec.definitions, &mut structs);
    let names: BTreeSet<String> = structs.iter().map(|s| s.name.text.clone()).collect();
    for s in structs {
        emit_struct_py(&names, &mut out, s);
    }
    out
}

/// Java field encode/decode over a Map<String,Object> (mirrors the C/Python
/// emitters; uses the Zdw.Writer/Reader primitives).
fn java_field(ts: &TypeSpec, field: &str, enc: &mut String, dec: &mut String) -> bool {
    let (e, d) = match ts {
        TypeSpec::Primitive(p) => {
            let (put, get) = match p {
                PrimitiveType::Octet
                | PrimitiveType::Char
                | PrimitiveType::Integer(IntegerType::Int8 | IntegerType::UInt8) => (
                    "w.u8(n(s,\"F\").intValue());",
                    "d.put(\"F\", (long) r.u8());",
                ),
                PrimitiveType::Integer(
                    IntegerType::Short
                    | IntegerType::UShort
                    | IntegerType::Int16
                    | IntegerType::UInt16,
                ) => (
                    "w.u16(n(s,\"F\").intValue());",
                    "d.put(\"F\", (long) r.u16());",
                ),
                PrimitiveType::Integer(
                    IntegerType::Long
                    | IntegerType::ULong
                    | IntegerType::Int32
                    | IntegerType::UInt32,
                ) => ("w.u32(n(s,\"F\").longValue());", "d.put(\"F\", r.u32());"),
                PrimitiveType::Integer(
                    IntegerType::LongLong
                    | IntegerType::ULongLong
                    | IntegerType::Int64
                    | IntegerType::UInt64,
                ) => ("w.u64(n(s,\"F\").longValue());", "d.put(\"F\", r.u64());"),
                PrimitiveType::Floating(FloatingType::Float) => {
                    ("w.f32(n(s,\"F\").floatValue());", "d.put(\"F\", r.f32());")
                }
                PrimitiveType::Floating(FloatingType::Double) => {
                    ("w.f64(n(s,\"F\").doubleValue());", "d.put(\"F\", r.f64());")
                }
                _ => return false,
            };
            (put.replace('F', field), get.replace('F', field))
        }
        TypeSpec::String(st) if !st.wide => (
            "w.str((String) s.get(\"F\"));".replace('F', field),
            "d.put(\"F\", r.str());".replace('F', field),
        ),
        TypeSpec::Sequence(seq) => match seq.elem.as_ref() {
            TypeSpec::Primitive(PrimitiveType::Octet)
            | TypeSpec::Primitive(PrimitiveType::Integer(IntegerType::UInt8)) => (
                "w.seqU8((byte[]) s.get(\"F\"));".replace('F', field),
                "d.put(\"F\", r.seqU8());".replace('F', field),
            ),
            _ => return false,
        },
        _ => return false,
    };
    enc.push_str(&format!("        {e}\n"));
    dec.push_str(&format!("        {d}\n"));
    true
}

fn generate_java(spec: &Specification, base: &str) -> String {
    let class = format!("{base}_gen");
    let mut out = String::new();
    let _ = writeln!(
        out,
        "// Generated by zerodds-endpoint-codegen (ADR 0013). DO NOT EDIT."
    );
    let _ = writeln!(out, "import java.util.Map;\nimport java.util.HashMap;\n");
    let _ = writeln!(out, "public final class {class} {{");
    let _ = writeln!(out, "    private {class}() {{}}");
    let _ = writeln!(
        out,
        "    private static Number n(Map<String,Object> s, String k) {{ return (Number) s.get(k); }}"
    );
    let mut structs = Vec::new();
    collect_structs(&spec.definitions, &mut structs);
    for s in structs {
        let prefix = snake(&s.name.text);
        let ext = extensibility(&s.annotations);
        // Per-field encode/decode statements + member ids (scalar members only:
        // the Java emitter covers primitives/string/seq<octet>, all three
        // extensibility modes; nested/seq<struct> extend via the same lambdas).
        let mut ef: Vec<String> = Vec::new();
        let mut df: Vec<String> = Vec::new();
        let mut ids: Vec<u32> = Vec::new();
        let mut idx = 0u32;
        for m in &s.members {
            let mid = member_id(&m.annotations).unwrap_or(idx);
            for decl in &m.declarators {
                let Declarator::Simple(id) = decl else {
                    panic!("array decl")
                };
                let mut e = String::new();
                let mut d = String::new();
                if !java_field(&m.type_spec, &id.text, &mut e, &mut d) {
                    panic!("unsupported java type in {}.{}", s.name.text, id.text);
                }
                ef.push(e.trim().to_string());
                df.push(d.trim().to_string());
                ids.push(mid);
                idx += 1;
            }
        }

        let _ = writeln!(
            out,
            "\n    public static void {prefix}Encode(Zdw.Writer w, final Map<String,Object> s) {{"
        );
        match ext {
            Ext::Final => {
                for e in &ef {
                    let _ = writeln!(out, "        {e}");
                }
            }
            Ext::Appendable => {
                let _ = writeln!(
                    out,
                    "        w.dheader(new Zdw.Body() {{ public void write(Zdw.Writer w) {{"
                );
                for e in &ef {
                    let _ = writeln!(out, "            {e}");
                }
                let _ = writeln!(out, "        }} }});");
            }
            Ext::Mutable => {
                let _ = writeln!(
                    out,
                    "        w.dheader(new Zdw.Body() {{ public void write(final Zdw.Writer w) {{"
                );
                for (i, e) in ef.iter().enumerate() {
                    let _ = writeln!(
                        out,
                        "            w.emheader({}, false, new Zdw.Body() {{ public void write(Zdw.Writer w) {{ {e} }} }});",
                        ids[i]
                    );
                }
                let _ = writeln!(out, "        }} }});");
            }
        }
        let _ = writeln!(out, "    }}");

        let _ = writeln!(
            out,
            "\n    public static Map<String,Object> {prefix}Decode(Zdw.Reader r) {{"
        );
        let _ = writeln!(
            out,
            "        Map<String,Object> d = new HashMap<String,Object>();"
        );
        match ext {
            Ext::Final => {}
            Ext::Appendable => {
                let _ = writeln!(out, "        r.dheaderRead();");
            }
            Ext::Mutable => {
                let _ = writeln!(out, "        r.dheaderRead();");
            }
        }
        for (i, d) in df.iter().enumerate() {
            if ext == Ext::Mutable {
                let _ = writeln!(out, "        r.emheaderRead(); {d}");
            } else {
                let _ = writeln!(out, "        {d}");
            }
            let _ = i;
        }
        let _ = writeln!(out, "        return d;");
        let _ = writeln!(out, "    }}");
    }
    let _ = writeln!(out, "}}");
    out
}

/// zerodds-lint: recursion-depth 64 (IDL module/struct nesting; bounded by type depth)
fn collect_structs<'a>(defs: &'a [Definition], out: &mut Vec<&'a StructDef>) {
    for d in defs {
        match d {
            Definition::Module(m) => collect_structs(&m.definitions, out),
            Definition::Type(TypeDecl::Constr(ConstrTypeDecl::Struct(StructDcl::Def(s)))) => {
                out.push(s);
            }
            _ => {}
        }
    }
}

fn generate(spec: &Specification, base: &str) -> (String, String) {
    let guard = format!("{}_GEN_H", base.to_uppercase());
    let mut hdr = String::new();
    let _ = writeln!(
        hdr,
        "/* Generated by zerodds-endpoint-codegen (ADR 0013). DO NOT EDIT. */"
    );
    let _ = writeln!(
        hdr,
        "#ifndef {guard}\n#define {guard}\n\n#include \"zerodds_wire.h\""
    );
    let mut src = String::new();
    let _ = writeln!(
        src,
        "/* Generated by zerodds-endpoint-codegen (ADR 0013). DO NOT EDIT. */"
    );
    let _ = writeln!(src, "#include \"{base}_gen.h\"\n");

    let mut structs = Vec::new();
    collect_structs(&spec.definitions, &mut structs);
    let names: BTreeSet<String> = structs.iter().map(|s| s.name.text.clone()).collect();
    // Forward-declare so a nested reference resolves regardless of order.
    for s in &structs {
        emit_struct(&names, &mut hdr, &mut src, s);
    }
    let _ = writeln!(hdr, "\n#endif");
    (hdr, src)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .expect("usage: <input.idl> <out-dir> [c|python]");
    let out_dir = args
        .next()
        .expect("usage: <input.idl> <out-dir> [c|python]");
    let lang = args.next().unwrap_or_else(|| "c".to_string());
    let text = std::fs::read_to_string(&input).expect("read idl");
    let spec = zerodds_idl::parse(&text, &ParserConfig::default()).expect("parse idl");

    let base = std::path::Path::new(&input)
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("stem");
    let dir = std::path::Path::new(&out_dir);
    match lang.as_str() {
        "python" | "py" => {
            std::fs::write(dir.join(format!("{base}_gen.py")), generate_py(&spec))
                .expect("write py");
            eprintln!("generated {base}_gen.py in {out_dir}");
        }
        "java" => {
            std::fs::write(
                dir.join(format!("{base}_gen.java")),
                generate_java(&spec, base),
            )
            .expect("write java");
            eprintln!("generated {base}_gen.java in {out_dir}");
        }
        _ => {
            let (hdr, src) = generate(&spec, base);
            std::fs::write(dir.join(format!("{base}_gen.h")), hdr).expect("write h");
            std::fs::write(dir.join(format!("{base}_gen.c")), src).expect("write c");
            eprintln!("generated {base}_gen.h + {base}_gen.c in {out_dir}");
        }
    }
}
