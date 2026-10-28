// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! IDL -> native endpoint SDK codegen (ADR 0013), C89 / Python / Java.
//!
//! Parses an IDL file with `zerodds-idl-parse`, lowers it with
//! `zerodds-idl-lower`, and emits the wire-fixed codec (C89
//! `<name>_encode` / `<name>_decode` over the zdw wire-core primitives)
//! per struct, byte-identical to the Rust core.
//!
//! Covered: primitives (int/float/bool/octet), string, `sequence<octet>`,
//! nested structs, `sequence<struct>`, and the three extensibility modes —
//! `@final` (plain), `@appendable` (DHEADER), `@mutable` (DHEADER +
//! per-member EMHEADER LC4).
//!
//! The extensibility of an unannotated struct and the effective member id
//! of every member are read off the model rather than re-derived from
//! annotations here: `idl-lower`'s completion pass already applies
//! XTypes 1.3 §7.3.1.2.1.1's `@autoid`, §7.3.1.2.1.2's `@id` and the
//! previous-plus-one default, and a second implementation of those rules
//! is a second set of answers.
//!
//! usage: zerodds-endpoint-codegen <input.idl> `<out-dir>` [c|python|java]

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

use zerodds_ast::{Ast, Extensibility, MemberNode, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, Ref};

const CAP: usize = 256;
const SEQ_STRUCT_CAP: usize = 16;

/// One struct member, already resolved to what the emitters need.
struct Field {
    /// Member name.
    name: String,
    /// The member's declared type, after aliases are followed.
    kind: TypeKind,
    /// Effective XTypes member id.
    id: u32,
}

/// The types the emitters walk, in declaration order.
struct Model {
    ast: Ast,
    structs: Vec<Id<TypeNode>>,
    names: BTreeSet<String>,
}

impl Model {
    fn fields(&self, id: Id<TypeNode>) -> Vec<Field> {
        let TypeKind::Structure { members, .. } = &self.ast.type_(id).expect("struct").kind else {
            return Vec::new();
        };
        members
            .iter()
            .filter_map(|member| {
                let node = self.ast.member(*member).ok()?;
                Some(Field {
                    name: node.name.clone(),
                    kind: self.resolved_kind(&node.type_),
                    id: member_id(node),
                })
            })
            .collect()
    }

    /// The kind behind a reference, following `typedef` chains.
    ///
    /// A member declared through an alias encodes as what the alias
    /// names; the alias itself has no wire form.
    fn resolved_kind(&self, reference: &Ref<TypeNode>) -> TypeKind {
        let mut current = reference.resolved();
        for _ in 0..64 {
            let Some(id) = current else {
                panic!("a member's type never resolved");
            };
            let node = self.ast.type_(id).expect("member type");
            match &node.kind {
                TypeKind::Alias { aliased } => current = aliased.resolved(),
                other => return other.clone(),
            }
        }
        panic!("a typedef chain longer than 64 links");
    }

    /// The simple name of a type reference, for a nested struct.
    fn type_name(&self, reference: &Ref<TypeNode>) -> String {
        reference
            .resolved()
            .and_then(|id| self.ast.type_(id).ok())
            .map(|node| node.name.clone())
            .unwrap_or_default()
    }

    fn extensibility(&self, id: Id<TypeNode>) -> Extensibility {
        self.ast.effective_extensibility(id)
    }

    fn name(&self, id: Id<TypeNode>) -> String {
        self.ast.type_(id).expect("struct").name.clone()
    }
}

/// The effective member id the frontend assigned.
fn member_id(member: &MemberNode) -> u32 {
    member
        .facets
        .xtypes
        .as_ref()
        .and_then(|xtypes| xtypes.effective_member_id())
        .expect("the frontend assigns every member an effective id")
}

fn snake(name: &str) -> String {
    let mut out = String::new();
    for (index, c) in name.chars().enumerate() {
        if c.is_uppercase() && index > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn cap_macro(prefix: &str, field: &str) -> String {
    format!("{}_{}_CAP", prefix.to_uppercase(), field.to_uppercase())
}

/// Whether a kind is a narrow string.
fn is_narrow_string(kind: &TypeKind) -> bool {
    matches!(
        kind,
        TypeKind::String {
            width: zerodds_ast::CharWidth::Char,
            ..
        }
    )
}

/// Whether a kind is an octet-like scalar, for `sequence<octet>`.
fn is_octet(kind: &TypeKind) -> bool {
    matches!(
        kind,
        TypeKind::Primitive(PrimitiveKind::Octet | PrimitiveKind::UInt8)
    )
}

/// The C field declaration and encode/decode statements for one field.
/// Returns false on a type outside the C89 subset.
fn emit_field(
    model: &Model,
    prefix: &str,
    kind: &TypeKind,
    field: &str,
    hdr: &mut String,
    fields: &mut String,
    enc: &mut String,
    dec: &mut String,
) -> bool {
    let cap = cap_macro(prefix, field);
    match kind {
        TypeKind::Primitive(p) => {
            let Some((cty, put, get)) = prim_calls(*p) else {
                return false;
            };
            let _ = writeln!(fields, "    {cty} {field};");
            let _ = writeln!(enc, "    {}", put.replace("{n}", field));
            let _ = writeln!(dec, "    {}", get.replace("{n}", field));
            true
        }
        kind if is_narrow_string(kind) => {
            let _ = writeln!(hdr, "#define {cap} {CAP}");
            let _ = writeln!(fields, "    char {field}[{cap}];");
            let _ = writeln!(enc, "    zdw_put_string(w, s->{field});");
            let _ = writeln!(dec, "    zdw_get_string(r, s->{field}, {cap});");
            true
        }
        TypeKind::Structure { .. } => false,
        TypeKind::Sequence { element, .. } => {
            let inner = model.resolved_kind(element);
            if is_octet(&inner) {
                let _ = writeln!(hdr, "#define {cap} {CAP}");
                let _ = writeln!(fields, "    unsigned char {field}[{cap}];");
                let _ = writeln!(fields, "    size_t {field}_len;");
                let _ = writeln!(enc, "    zdw_put_seq_u8(w, s->{field}, s->{field}_len);");
                let _ = writeln!(
                    dec,
                    "    zdw_get_seq_u8(r, s->{field}, {cap}, &s->{field}_len);"
                );
                return true;
            }
            let ref_ty = model.type_name(element);
            if !matches!(inner, TypeKind::Structure { .. }) || !model.names.contains(&ref_ty) {
                return false;
            }
            // sequence<struct>: collection DHEADER + u32 count + elements.
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
    }
}

/// A nested-struct member, which needs the reference rather than the
/// resolved kind to name its type.
fn emit_nested(
    model: &Model,
    reference: &Ref<TypeNode>,
    field: &str,
    fields: &mut String,
    enc: &mut String,
    dec: &mut String,
) -> bool {
    let ref_ty = model.type_name(reference);
    if !model.names.contains(&ref_ty) {
        return false;
    }
    let ref_pfx = snake(&ref_ty);
    let _ = writeln!(fields, "    {ref_ty} {field};");
    let _ = writeln!(enc, "    {ref_pfx}_encode(w, &s->{field});");
    let _ = writeln!(dec, "    {ref_pfx}_decode(r, &s->{field});");
    true
}

fn prim_calls(kind: PrimitiveKind) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match kind {
        PrimitiveKind::Octet | PrimitiveKind::Char | PrimitiveKind::Int8 | PrimitiveKind::UInt8 => {
            (
                "unsigned char",
                "zdw_put_u8(w, s->{n});",
                "zdw_get_u8(r, &s->{n});",
            )
        }
        PrimitiveKind::Int16 | PrimitiveKind::UInt16 => (
            "unsigned int",
            "zdw_put_u16(w, s->{n});",
            "zdw_get_u16(r, &s->{n});",
        ),
        PrimitiveKind::Int32 | PrimitiveKind::UInt32 => (
            "unsigned long",
            "zdw_put_u32(w, s->{n});",
            "zdw_get_u32(r, &s->{n});",
        ),
        PrimitiveKind::Int64 | PrimitiveKind::UInt64 => (
            "zdw_u64_t",
            "zdw_put_u64(w, s->{n});",
            "zdw_get_u64(r, &s->{n});",
        ),
        PrimitiveKind::Float32 => (
            "float",
            "zdw_put_f32(w, s->{n});",
            "zdw_get_f32(r, &s->{n});",
        ),
        PrimitiveKind::Float64 => (
            "double",
            "zdw_put_f64(w, s->{n});",
            "zdw_get_f64(r, &s->{n});",
        ),
        PrimitiveKind::Boolean => (
            "int",
            "zdw_put_bool(w, s->{n});",
            "zdw_get_bool(r, &s->{n});",
        ),
        PrimitiveKind::WChar | PrimitiveKind::Float128 => return None,
    })
}

fn emit_struct(model: &Model, hdr: &mut String, src: &mut String, id: Id<TypeNode>) {
    let name = model.name(id);
    let prefix = snake(&name);
    let ext = model.extensibility(id);

    let mut fields = String::new();
    let mut members: Vec<(u32, String, String)> = Vec::new();
    for field in model.fields(id) {
        let mut enc = String::new();
        let mut dec = String::new();
        let ok = match &field.kind {
            TypeKind::Structure { .. } => {
                let reference = member_type_ref(model, id, &field.name);
                emit_nested(
                    model,
                    &reference,
                    &field.name,
                    &mut fields,
                    &mut enc,
                    &mut dec,
                )
            }
            kind => emit_field(
                model,
                &prefix,
                kind,
                &field.name,
                hdr,
                &mut fields,
                &mut enc,
                &mut dec,
            ),
        };
        if !ok {
            panic!("unsupported member type for {name}.{}", field.name);
        }
        members.push((field.id, enc, dec));
    }

    let mut encode = String::new();
    let mut decode = String::new();
    match ext {
        Extensibility::Final => {
            for (_, e, d) in &members {
                encode.push_str(e);
                decode.push_str(d);
            }
        }
        Extensibility::Appendable => {
            let _ = writeln!(encode, "    {{ size_t _d = zdw_dheader_begin(w);");
            for (_, e, _) in &members {
                encode.push_str(e);
            }
            let _ = writeln!(encode, "    zdw_dheader_end(w, _d); }}");
            let _ = writeln!(
                decode,
                "    {{ unsigned long _dh = 0; zdw_dheader_read(r, &_dh);"
            );
            for (_, _, d) in &members {
                decode.push_str(d);
            }
            let _ = writeln!(decode, "    }}");
        }
        Extensibility::Mutable => {
            let _ = writeln!(
                encode,
                "    {{ size_t _d = zdw_dheader_begin(w); size_t _e;"
            );
            for (mid, e, _) in &members {
                let _ = writeln!(encode, "    _e = zdw_emheader_begin(w, {mid}uL, 0);");
                encode.push_str(e);
                let _ = writeln!(encode, "    zdw_emheader_end(w, _e);");
            }
            let _ = writeln!(encode, "    zdw_dheader_end(w, _d); }}");
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
            for (mid, _, d) in &members {
                let kw = if first { "if" } else { "else if" };
                first = false;
                let _ = writeln!(decode, "        {kw} (_id == {mid}uL) {{");
                decode.push_str(&d.replace('\n', "\n  "));
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

/// The declared reference of one member, by name.
fn member_type_ref(model: &Model, struct_: Id<TypeNode>, field: &str) -> Ref<TypeNode> {
    let TypeKind::Structure { members, .. } = &model.ast.type_(struct_).expect("struct").kind
    else {
        panic!("not a struct");
    };
    members
        .iter()
        .filter_map(|id| model.ast.member(*id).ok())
        .find(|node| node.name == field)
        .map(|node| node.type_.clone())
        .expect("the member exists")
}

/// The Python field encode/decode expressions for a member.
fn py_field(
    model: &Model,
    kind: &TypeKind,
    reference: &Ref<TypeNode>,
    field: &str,
    enc: &mut String,
    dec: &mut String,
) -> bool {
    let (e, d) = match kind {
        TypeKind::Primitive(p) => {
            let call = match p {
                PrimitiveKind::Octet
                | PrimitiveKind::Char
                | PrimitiveKind::Int8
                | PrimitiveKind::UInt8 => "u8",
                PrimitiveKind::Int16 | PrimitiveKind::UInt16 => "u16",
                PrimitiveKind::Int32 | PrimitiveKind::UInt32 => "u32",
                PrimitiveKind::Int64 | PrimitiveKind::UInt64 => "u64",
                PrimitiveKind::Float32 => "f32",
                PrimitiveKind::Float64 => "f64",
                PrimitiveKind::Boolean => "bool",
                PrimitiveKind::WChar | PrimitiveKind::Float128 => return false,
            };
            (
                format!("w.put_{call}(s['{field}'])"),
                format!("d['{field}'] = r.get_{call}()"),
            )
        }
        kind if is_narrow_string(kind) => (
            format!("w.put_string(s['{field}'])"),
            format!("d['{field}'] = r.get_string()"),
        ),
        TypeKind::Structure { .. } => {
            let ref_ty = model.type_name(reference);
            if !model.names.contains(&ref_ty) {
                return false;
            }
            let p = snake(&ref_ty);
            (
                format!("{p}_encode(w, s['{field}'])"),
                format!("d['{field}'] = {p}_decode(r)"),
            )
        }
        TypeKind::Sequence { element, .. } => {
            let inner = model.resolved_kind(element);
            if is_octet(&inner) {
                (
                    format!("w.put_seq_u8(s['{field}'])"),
                    format!("d['{field}'] = r.get_seq_u8()"),
                )
            } else {
                let ref_ty = model.type_name(element);
                if !matches!(inner, TypeKind::Structure { .. }) || !model.names.contains(&ref_ty) {
                    return false;
                }
                let p = snake(&ref_ty);
                (
                    format!(
                        "_b = w.dheader_begin(); w.put_u32(len(s['{field}']))\n        for _e in s['{field}']: {p}_encode(w, _e)\n        w.dheader_end(_b)"
                    ),
                    format!(
                        "r.dheader_read(); _n = r.get_u32(); d['{field}'] = [{p}_decode(r) for _ in range(_n)]"
                    ),
                )
            }
        }
        _ => return false,
    };
    enc.push_str(&format!("    {e}\n"));
    dec.push_str(&format!("    {d}\n"));
    true
}

fn emit_struct_py(model: &Model, out: &mut String, id: Id<TypeNode>) {
    let name = model.name(id);
    let prefix = snake(&name);
    let ext = model.extensibility(id);
    let mut enc = String::new();
    let mut dec = String::new();
    let mut ids: Vec<u32> = Vec::new();
    for field in model.fields(id) {
        let reference = member_type_ref(model, id, &field.name);
        if !py_field(
            model,
            &field.kind,
            &reference,
            &field.name,
            &mut enc,
            &mut dec,
        ) {
            panic!("unsupported type in {name}.{}", field.name);
        }
        ids.push(field.id);
    }
    let _ = writeln!(out, "\ndef {prefix}_encode(w, s):");
    match ext {
        Extensibility::Final => out.push_str(&enc),
        Extensibility::Appendable => {
            let _ = writeln!(out, "    _d = w.dheader_begin()");
            out.push_str(&enc);
            let _ = writeln!(out, "    w.dheader_end(_d)");
        }
        Extensibility::Mutable => {
            let _ = writeln!(out, "    _d = w.dheader_begin()");
            for (index, line) in enc.lines().enumerate() {
                let _ = writeln!(out, "    _e = w.emheader_begin({}, 0)", ids[index]);
                let _ = writeln!(out, "    {}", line.trim_start());
                let _ = writeln!(out, "    w.emheader_end(_e)");
            }
            let _ = writeln!(out, "    w.dheader_end(_d)");
        }
    }
    let _ = writeln!(out, "\ndef {prefix}_decode(r):");
    let _ = writeln!(out, "    d = {{}}");
    match ext {
        Extensibility::Final => out.push_str(&dec),
        Extensibility::Appendable => {
            let _ = writeln!(out, "    r.dheader_read()");
            out.push_str(&dec);
        }
        Extensibility::Mutable => {
            let _ = writeln!(out, "    _dh = r.dheader_read(); _start = r.pos");
            let _ = writeln!(out, "    while r.pos - _start < _dh:");
            let _ = writeln!(out, "        _id, _mu, _ni = r.emheader_read()");
            for (index, line) in dec.lines().enumerate() {
                let kw = if index == 0 { "if" } else { "elif" };
                let _ = writeln!(out, "        {kw} _id == {}:", ids[index]);
                let _ = writeln!(out, "            {}", line.trim_start());
            }
            let _ = writeln!(out, "        else:");
            let _ = writeln!(out, "            r.get_bytes(_ni)");
        }
    }
    let _ = writeln!(out, "    return d");
}

fn generate_py(model: &Model) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Generated by zerodds-endpoint-codegen (ADR 0013). DO NOT EDIT."
    );
    let _ = writeln!(out, "from __future__ import print_function");
    for id in &model.structs {
        emit_struct_py(model, &mut out, *id);
    }
    out
}

/// Java field encode/decode over a `Map<String,Object>`.
fn java_field(
    kind: &TypeKind,
    model: &Model,
    field: &str,
    enc: &mut String,
    dec: &mut String,
) -> bool {
    let (e, d) = match kind {
        TypeKind::Primitive(p) => {
            let (put, get) = match p {
                PrimitiveKind::Octet
                | PrimitiveKind::Char
                | PrimitiveKind::Int8
                | PrimitiveKind::UInt8 => (
                    "w.u8(n(s,\"F\").intValue());",
                    "d.put(\"F\", (long) r.u8());",
                ),
                PrimitiveKind::Int16 | PrimitiveKind::UInt16 => (
                    "w.u16(n(s,\"F\").intValue());",
                    "d.put(\"F\", (long) r.u16());",
                ),
                PrimitiveKind::Int32 | PrimitiveKind::UInt32 => {
                    ("w.u32(n(s,\"F\").longValue());", "d.put(\"F\", r.u32());")
                }
                PrimitiveKind::Int64 | PrimitiveKind::UInt64 => {
                    ("w.u64(n(s,\"F\").longValue());", "d.put(\"F\", r.u64());")
                }
                PrimitiveKind::Float32 => {
                    ("w.f32(n(s,\"F\").floatValue());", "d.put(\"F\", r.f32());")
                }
                PrimitiveKind::Float64 => {
                    ("w.f64(n(s,\"F\").doubleValue());", "d.put(\"F\", r.f64());")
                }
                PrimitiveKind::WChar | PrimitiveKind::Boolean | PrimitiveKind::Float128 => {
                    return false;
                }
            };
            (put.replace('F', field), get.replace('F', field))
        }
        kind if is_narrow_string(kind) => (
            "w.str((String) s.get(\"F\"));".replace('F', field),
            "d.put(\"F\", r.str());".replace('F', field),
        ),
        TypeKind::Sequence { element, .. } if is_octet(&model.resolved_kind(element)) => (
            "w.seqU8((byte[]) s.get(\"F\"));".replace('F', field),
            "d.put(\"F\", r.seqU8());".replace('F', field),
        ),
        _ => return false,
    };
    enc.push_str(&format!("        {e}\n"));
    dec.push_str(&format!("        {d}\n"));
    true
}

fn generate_java(model: &Model, base: &str) -> String {
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
    for id in &model.structs {
        let name = model.name(*id);
        let prefix = snake(&name);
        let ext = model.extensibility(*id);
        let mut encoders: Vec<String> = Vec::new();
        let mut decoders: Vec<String> = Vec::new();
        let mut ids: Vec<u32> = Vec::new();
        for field in model.fields(*id) {
            let mut e = String::new();
            let mut d = String::new();
            if !java_field(&field.kind, model, &field.name, &mut e, &mut d) {
                panic!("unsupported java type in {name}.{}", field.name);
            }
            encoders.push(e.trim().to_string());
            decoders.push(d.trim().to_string());
            ids.push(field.id);
        }

        let _ = writeln!(
            out,
            "\n    public static void {prefix}Encode(Zdw.Writer w, final Map<String,Object> s) {{"
        );
        match ext {
            Extensibility::Final => {
                for e in &encoders {
                    let _ = writeln!(out, "        {e}");
                }
            }
            Extensibility::Appendable => {
                let _ = writeln!(
                    out,
                    "        w.dheader(new Zdw.Body() {{ public void write(Zdw.Writer w) {{"
                );
                for e in &encoders {
                    let _ = writeln!(out, "            {e}");
                }
                let _ = writeln!(out, "        }} }});");
            }
            Extensibility::Mutable => {
                let _ = writeln!(
                    out,
                    "        w.dheader(new Zdw.Body() {{ public void write(final Zdw.Writer w) {{"
                );
                for (index, e) in encoders.iter().enumerate() {
                    let _ = writeln!(
                        out,
                        "            w.emheader({}, false, new Zdw.Body() {{ public void write(Zdw.Writer w) {{ {e} }} }});",
                        ids[index]
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
        if ext != Extensibility::Final {
            let _ = writeln!(out, "        r.dheaderRead();");
        }
        for d in &decoders {
            if ext == Extensibility::Mutable {
                let _ = writeln!(out, "        r.emheaderRead(); {d}");
            } else {
                let _ = writeln!(out, "        {d}");
            }
        }
        let _ = writeln!(out, "        return d;");
        let _ = writeln!(out, "    }}");
    }
    let _ = writeln!(out, "}}");
    out
}

fn generate(model: &Model, base: &str) -> (String, String) {
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

    for id in &model.structs {
        emit_struct(model, &mut hdr, &mut src, *id);
    }
    let _ = writeln!(hdr, "\n#endif");
    (hdr, src)
}

/// Parse and lower one file into a resolved model.
fn load(text: &str, path: &str) -> Model {
    let parsed = zerodds_idl_parse::parse(text);
    let errors: Vec<_> = parsed
        .diagnostics()
        .iter()
        .filter(|d| d.severity == zerodds_idl_parse::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "{path} does not parse: {errors:?}");

    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), path);
    assert!(
        lowered.diagnostics.is_empty(),
        "{path} does not lower: {:?}",
        lowered.diagnostics
    );
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("a resolve pass");
    assert!(
        report.is_clean(),
        "{path} names types it does not declare: {:?}",
        report.broken
    );

    // Declaration order is arena order, and a nested struct has to be
    // emitted before the one that holds it for the C89 output to compile.
    let structs: Vec<Id<TypeNode>> = ast
        .types()
        .filter(|(_, node)| matches!(node.kind, TypeKind::Structure { .. }))
        .map(|(id, _)| id)
        .collect();
    let names: BTreeSet<String> = structs
        .iter()
        .filter_map(|id| ast.type_(*id).ok().map(|node| node.name.clone()))
        .collect();
    Model {
        ast,
        structs,
        names,
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let input = args
        .next()
        .expect("usage: <input.idl> <out-dir> [c|python|java]");
    let out_dir = args
        .next()
        .expect("usage: <input.idl> <out-dir> [c|python|java]");
    let lang = args.next().unwrap_or_else(|| "c".to_string());
    let text = std::fs::read_to_string(&input).expect("read idl");
    let model = load(&text, &input);

    let base = std::path::Path::new(&input)
        .file_stem()
        .and_then(|s| s.to_str())
        .expect("stem");
    let dir = std::path::Path::new(&out_dir);
    match lang.as_str() {
        "python" | "py" => {
            std::fs::write(dir.join(format!("{base}_gen.py")), generate_py(&model))
                .expect("write py");
            eprintln!("generated {base}_gen.py in {out_dir}");
        }
        "java" => {
            std::fs::write(
                dir.join(format!("{base}_gen.java")),
                generate_java(&model, base),
            )
            .expect("write java");
            eprintln!("generated {base}_gen.java in {out_dir}");
        }
        _ => {
            let (hdr, src) = generate(&model, base);
            std::fs::write(dir.join(format!("{base}_gen.h")), hdr).expect("write h");
            std::fs::write(dir.join(format!("{base}_gen.c")), src).expect("write c");
            eprintln!("generated {base}_gen.h + {base}_gen.c in {out_dir}");
        }
    }
}
