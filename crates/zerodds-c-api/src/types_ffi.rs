// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `dds::core::xtypes::TypeProvider` (DDS-PSM-Cxx 1.0 §7.5): the types a document defines.
//!
//! The document is IDL or a DDS-XML type library, read from a `file:`, `data:`, `http:` or
//! `https:` URI. An XML library is written as IDL first, so both go through the front end every
//! other column uses. The result is a description of every named structure, union, enumeration,
//! alias and bit set in JSON, which the C++ side turns into `DynamicType` trees.
//!
//! ```text
//! {"types":[{"name":"a::T","kind":"struct","base":null,"ext":"final","nested":false,
//!            "members":[{"name":"x","type":TYPE,"key":true,"optional":false,
//!                        "external":false,"must_understand":false,"id":null}]},
//!           {"name":"a::U","kind":"union","disc":TYPE,
//!            "cases":[{"labels":[1,2],"default":false,"name":"a","type":TYPE}]},
//!           {"name":"a::E","kind":"enum","enumerators":[{"name":"A","value":null}]},
//!           {"name":"a::L","kind":"alias","aliased":TYPE},
//!           {"name":"a::B","kind":"bitset","fields":[{"name":"f","bits":3}]},
//!           {"name":"a::M","kind":"bitmask","bit_bound":8,
//!            "flags":[{"name":"f","position":0}]}]}
//! TYPE = {"k":"prim","n":"long"} | {"k":"string","wide":false,"bound":null}
//!      | {"k":"seq","e":TYPE,"bound":null} | {"k":"map","key":TYPE,"v":TYPE,"bound":null}
//!      | {"k":"ref","n":"a::T"} | {"k":"unsupported","what":"..."}
//! ```
//!
//! A use of a construct the PSM has no class for (an array, `any`, a fixed-point number) is a
//! `unsupported` TYPE, which the C++ side refuses when the type that contains it is requested.

use core::ffi::{c_char, c_int};
use std::ffi::{CStr, CString};

use serde_json::{Value, json};
use zerodds_ast::{Ast, Bound, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;

use crate::ZeroDdsStatus;

const MAX_DEPTH: usize = 64;

fn scoped_name(ast: &Ast, node: &TypeNode) -> Result<String, String> {
    let mut parts = vec![node.name.clone()];
    if let Some(scope) = &node.scope {
        parts.insert(0, scope.clone());
    }
    let mut module = Some(node.module);
    let mut depth = 0;
    while let Some(id) = module {
        depth += 1;
        if depth > MAX_DEPTH {
            return Err("module nesting too deep".into());
        }
        let m = ast.module(id).map_err(|e| format!("{e:?}"))?;
        if !m.name.is_empty() {
            parts.insert(0, m.name.clone());
        }
        module = m.parent;
    }
    Ok(parts.join("::"))
}

fn bound_of(ast: &Ast, bound: Bound) -> Result<Value, String> {
    match bound.expr() {
        None => Ok(Value::Null),
        Some(expr) => {
            let n = ast
                .eval_integral(expr)
                .map_err(|e| format!("bound: {e:?}"))?;
            Ok(json!(n))
        }
    }
}

fn unsupported(what: &str) -> Value {
    json!({"k": "unsupported", "what": what})
}

/// zerodds-lint: recursion-depth 64 (type nesting; `depth` is checked against `MAX_DEPTH`)
fn type_spec(ast: &Ast, id: Id<TypeNode>, depth: usize) -> Result<Value, String> {
    if depth > MAX_DEPTH {
        return Err("type nesting too deep".into());
    }
    let node = ast.type_(id).map_err(|e| format!("{e:?}"))?;
    Ok(match &node.kind {
        TypeKind::Primitive(kind) => match kind {
            PrimitiveKind::Int8 | PrimitiveKind::UInt8 => unsupported("8-bit integer"),
            other => json!({"k": "prim", "n": other.as_str()}),
        },
        TypeKind::String { width, bound } => json!({
            "k": "string",
            "wide": matches!(width, CharWidth::WChar),
            "bound": bound_of(ast, *bound)?,
        }),
        TypeKind::Sequence { element, bound } => {
            let element = element.resolved().ok_or("unresolved sequence element")?;
            json!({"k": "seq", "e": type_spec(ast, element, depth + 1)?, "bound": bound_of(ast, *bound)?})
        }
        TypeKind::Map { key, value, bound } => {
            let key = key.resolved().ok_or("unresolved map key")?;
            let value = value.resolved().ok_or("unresolved map value")?;
            json!({
                "k": "map",
                "key": type_spec(ast, key, depth + 1)?,
                "v": type_spec(ast, value, depth + 1)?,
                "bound": bound_of(ast, *bound)?,
            })
        }
        TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Enumeration { .. }
        | TypeKind::Bitmask { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Alias { .. } => json!({"k": "ref", "n": scoped_name(ast, node)?}),
        TypeKind::Array { element, dims } => {
            let element = element.resolved().ok_or("unresolved array element")?;
            let mut extents = Vec::with_capacity(dims.len());
            for dim in dims {
                extents.push(
                    ast.eval_integral(*dim)
                        .map_err(|e| format!("array dimension: {e:?}"))?,
                );
            }
            json!({"k": "array", "e": type_spec(ast, element, depth + 1)?, "dims": extents})
        }
        TypeKind::Fixed { .. } => unsupported("fixed"),
        TypeKind::Any => unsupported("any"),
        _ => unsupported(node.kind.as_str()),
    })
}

fn extensibility_name(e: zerodds_ast::Extensibility) -> &'static str {
    match e {
        zerodds_ast::Extensibility::Final => "final",
        zerodds_ast::Extensibility::Appendable => "appendable",
        zerodds_ast::Extensibility::Mutable => "mutable",
    }
}

fn member_json(ast: &Ast, id: Id<zerodds_ast::MemberNode>) -> Result<Value, String> {
    let m = ast.member(id).map_err(|e| format!("{e:?}"))?;
    let t = m.type_.resolved().ok_or("unresolved member type")?;
    let external = m.facets.xtypes.as_ref().is_some_and(|x| x.is_external);
    Ok(json!({
        "name": m.name,
        "type": type_spec(ast, t, 0)?,
        "key": m.facets.is_key(),
        "optional": m.facets.is_optional(),
        "external": external,
        "must_understand": m.facets.is_must_understand(),
        "id": m.facets.effective_member_id(),
    }))
}

fn declaration(ast: &Ast, id: Id<TypeNode>, node: &TypeNode) -> Result<Option<Value>, String> {
    let name = scoped_name(ast, node)?;
    let nested = node.facets.xtypes.as_ref().is_some_and(|x| x.is_nested);
    Ok(Some(match &node.kind {
        TypeKind::Structure { base, members } => {
            let base = match base {
                Some(b) => {
                    let b = b.resolved().ok_or("unresolved base type")?;
                    Value::String(scoped_name(
                        ast,
                        ast.type_(b).map_err(|e| format!("{e:?}"))?,
                    )?)
                }
                None => Value::Null,
            };
            let mut list = Vec::new();
            for m in members {
                list.push(member_json(ast, *m)?);
            }
            json!({
                "name": name, "kind": "struct", "base": base,
                "ext": node.facets.extensibility().map(extensibility_name),
                "nested": nested, "members": list,
            })
        }
        TypeKind::Union {
            discriminator,
            cases,
            ..
        } => {
            let disc = discriminator.resolved().ok_or("unresolved discriminator")?;
            let mut list = Vec::new();
            for c in cases {
                let case = ast.case(*c).map_err(|e| format!("{e:?}"))?;
                let member = ast.member(case.member).map_err(|e| format!("{e:?}"))?;
                let t = member.type_.resolved().ok_or("unresolved case type")?;
                let mut labels = Vec::new();
                let mut default = false;
                for label in &case.labels {
                    match label {
                        zerodds_ast::CaseLabel::Value(expr) => labels.push(
                            ast.eval_integral(*expr)
                                .map_err(|e| format!("case label: {e:?}"))?
                                .to_string(),
                        ),
                        zerodds_ast::CaseLabel::Default => default = true,
                    }
                }
                list.push(json!({
                    "labels": labels, "default": default, "name": member.name,
                    "type": type_spec(ast, t, 0)?,
                }));
            }
            json!({
                "name": name, "kind": "union", "disc": type_spec(ast, disc, 0)?,
                "ext": node.facets.extensibility().map(extensibility_name),
                "nested": nested, "cases": list,
            })
        }
        TypeKind::Enumeration { enumerators } => {
            let mut list = Vec::new();
            for e in enumerators {
                let e = ast.enumerator(*e).map_err(|e| format!("{e:?}"))?;
                let value = match e.value {
                    Some(expr) => json!(
                        ast.eval_integral(expr)
                            .map_err(|e| format!("enumerator value: {e:?}"))?
                    ),
                    None => Value::Null,
                };
                list.push(json!({"name": e.name, "value": value}));
            }
            json!({"name": name, "kind": "enum", "enumerators": list})
        }
        TypeKind::Alias { aliased } => {
            let aliased = aliased.resolved().ok_or("unresolved alias")?;
            json!({"name": name, "kind": "alias", "aliased": type_spec(ast, aliased, 0)?})
        }
        TypeKind::Bitset { base, fields } => {
            if base.is_some() {
                return Ok(Some(
                    json!({"name": name, "kind": "unsupported", "what": "bitset with a base"}),
                ));
            }
            let mut list = Vec::new();
            for f in fields {
                let f = ast.bitfield(*f).map_err(|e| format!("{e:?}"))?;
                let Some(field_name) = &f.name else {
                    return Ok(Some(
                        json!({"name": name, "kind": "unsupported", "what": "anonymous bit field"}),
                    ));
                };
                list.push(json!({"name": field_name, "bits": f.bits}));
            }
            json!({"name": name, "kind": "bitset", "fields": list})
        }
        TypeKind::Bitmask { bits } => {
            let mut flags = Vec::new();
            let mut next: u32 = 0;
            for bit in bits {
                let bit = ast.bit_value(*bit).map_err(|e| format!("{e:?}"))?;
                let position = bit.position.map_or(next, u32::from);
                next = position + 1;
                flags.push(json!({"name": bit.name, "position": position}));
            }
            json!({
                "name": name, "kind": "bitmask",
                "bit_bound": ast.effective_bit_bound(id), "flags": flags,
            })
        }
        // Not a type of data; named so a caller asking for it gets an answer.
        TypeKind::AnnotationType { .. } => json!({"name": name, "kind": "annotation"}),
        // Anonymous constructs, `any`, interfaces, valuetypes: not types a document defines for
        // the PSM; a type that uses one says so in its own TYPE.
        _ => {
            let _ = id;
            return Ok(None);
        }
    }))
}

/// The description of every named type of `idl`.
pub(crate) fn describe_idl(idl: &str) -> Result<String, String> {
    let parsed = zerodds_idl_parse::parse(idl);
    if parsed.has_errors() {
        return Err("the IDL does not parse".to_string());
    }
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "types.idl");
    if !lowered.diagnostics.is_empty() {
        return Err(format!("the IDL does not lower: {:?}", lowered.diagnostics));
    }
    let mut ast = lowered.ast;
    // A name written with a scope (`m::Color`) or declared further down is pending until the
    // resolve pass has run.
    ast.resolve_all_by_name()
        .map_err(|e| format!("the IDL does not resolve: {e:?}"))?;
    let mut types = Vec::new();
    for (id, node) in ast.types() {
        if node.name.is_empty() {
            continue;
        }
        if let Some(d) = declaration(&ast, id, node)? {
            types.push(d);
        }
    }
    serde_json::to_string(&json!({"types": types})).map_err(|e| e.to_string())
}

// ---- DDS-XML type library -> IDL ----

fn xml_primitive(p: zerodds_xml::xtypes_def::PrimitiveType) -> &'static str {
    use zerodds_xml::xtypes_def::PrimitiveType as P;
    match p {
        P::Boolean => "boolean",
        P::Octet => "octet",
        P::Int8 => "int8",
        P::UInt8 => "uint8",
        P::Char => "char",
        P::WChar => "wchar",
        P::Short => "short",
        P::UShort => "unsigned short",
        P::Long => "long",
        P::ULong => "unsigned long",
        P::LongLong => "long long",
        P::ULongLong => "unsigned long long",
        P::Float => "float",
        P::Double => "double",
        P::LongDouble => "long double",
        P::String => "string",
        P::WString => "wstring",
    }
}

fn xml_type_ref(
    r: &zerodds_xml::xtypes_def::TypeRef,
    string_bound: Option<u32>,
    sequence_bound: Option<u32>,
) -> String {
    use zerodds_xml::xtypes_def::{PrimitiveType as P, TypeRef};
    let base = match r {
        TypeRef::Primitive(p @ (P::String | P::WString)) => match string_bound {
            Some(n) if n > 0 => format!("{}<{n}>", xml_primitive(*p)),
            _ => xml_primitive(*p).to_string(),
        },
        TypeRef::Primitive(p) => xml_primitive(*p).to_string(),
        TypeRef::Named(n) => n.clone(),
    };
    match sequence_bound {
        Some(n) if n > 0 => format!("sequence<{base}, {n}>"),
        Some(_) => format!("sequence<{base}>"),
        None => base,
    }
}

fn xml_member(m: &zerodds_xml::xtypes_def::StructMember) -> String {
    let mut annotations = String::new();
    if m.key {
        annotations.push_str("@key ");
    }
    if m.optional {
        annotations.push_str("@optional ");
    }
    if m.must_understand {
        annotations.push_str("@must_understand ");
    }
    if let Some(id) = m.id {
        annotations.push_str(&format!("@id({id}) "));
    }
    let dims: String = m
        .array_dimensions
        .iter()
        .map(|d| format!("[{d}]"))
        .collect();
    format!(
        "{annotations}{} {}{dims};",
        xml_type_ref(&m.type_ref, m.string_max_length, m.sequence_max_length),
        m.name
    )
}

fn xml_extensibility(e: Option<zerodds_xml::xtypes_def::Extensibility>) -> &'static str {
    use zerodds_xml::xtypes_def::Extensibility as E;
    match e {
        Some(E::Final) => "@final ",
        Some(E::Appendable) => "@appendable ",
        Some(E::Mutable) => "@mutable ",
        None => "",
    }
}

fn xml_scoped(name: &str) -> (Vec<&str>, &str) {
    let mut parts: Vec<&str> = name.split("::").filter(|p| !p.is_empty()).collect();
    let simple = parts.pop().unwrap_or("");
    (parts, simple)
}

/// zerodds-lint: recursion-depth 64 (module nesting; `depth` is checked against `MAX_DEPTH`)
fn xml_types_idl(
    types: &[zerodds_xml::xtypes_def::TypeDef],
    out: &mut String,
    depth: usize,
) -> Result<(), String> {
    use zerodds_xml::xtypes_def::{TypeDef, UnionDiscriminator};
    if depth > MAX_DEPTH {
        return Err("module nesting too deep".into());
    }
    for t in types {
        match t {
            TypeDef::Module(m) => {
                out.push_str(&format!("module {} {{\n", m.name));
                xml_types_idl(&m.types, out, depth + 1)?;
                out.push_str("};\n");
            }
            TypeDef::Struct(s) => {
                let (modules, simple) = xml_scoped(&s.name);
                for m in &modules {
                    out.push_str(&format!("module {m} {{ "));
                }
                let base = s
                    .base_type
                    .as_ref()
                    .map_or(String::new(), |b| format!(" : {b}"));
                out.push_str(&format!(
                    "{}struct {simple}{base} {{\n{}\n}};",
                    xml_extensibility(s.extensibility),
                    s.members
                        .iter()
                        .map(xml_member)
                        .collect::<Vec<_>>()
                        .join("\n")
                ));
                for _ in &modules {
                    out.push_str(" };");
                }
                out.push('\n');
            }
            TypeDef::Enum(e) => {
                let list = e
                    .enumerators
                    .iter()
                    .map(|l| match l.value {
                        Some(v) => format!("@value({v}) {}", l.name),
                        None => l.name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("enum {} {{ {list} }};\n", e.name));
            }
            TypeDef::Union(u) => {
                let mut cases = String::new();
                for c in &u.cases {
                    for d in &c.discriminators {
                        match d {
                            UnionDiscriminator::Value(v) => cases.push_str(&format!("case {v}: ")),
                            UnionDiscriminator::Default => cases.push_str("default: "),
                        }
                    }
                    cases.push_str(&xml_member(&c.member));
                    cases.push('\n');
                }
                out.push_str(&format!(
                    "union {} switch ({}) {{\n{cases}}};\n",
                    u.name,
                    xml_type_ref(&u.discriminator, None, None)
                ));
            }
            TypeDef::Typedef(d) => {
                let dims: String = d
                    .array_dimensions
                    .iter()
                    .map(|n| format!("[{n}]"))
                    .collect();
                out.push_str(&format!(
                    "typedef {} {}{dims};\n",
                    xml_type_ref(&d.type_ref, d.string_max_length, d.sequence_max_length),
                    d.name
                ));
            }
            TypeDef::Bitmask(b) => {
                let list = b
                    .bit_values
                    .iter()
                    .map(|v| match v.position {
                        Some(p) => format!("@position({p}) {}", v.name),
                        None => v.name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!("bitmask {} {{ {list} }};\n", b.name));
            }
            TypeDef::Bitset(b) => {
                return Err(format!(
                    "the bit set `{}` of an XML library has masks, not widths; write it as IDL",
                    b.name
                ));
            }
            TypeDef::Annotation(a) => {
                let base = a
                    .base_type
                    .as_ref()
                    .map_or(String::new(), |b| format!(" : {b}"));
                let members: String = a
                    .members
                    .iter()
                    .map(|m| {
                        format!(
                            "    {} {};\n",
                            xml_type_ref(&m.type_ref, None, None),
                            m.name
                        )
                    })
                    .collect();
                out.push_str(&format!("@annotation {}{base} {{\n{members}}};\n", a.name));
            }
            TypeDef::Include(_) | TypeDef::ForwardDcl(_) | TypeDef::Const(_) => {}
        }
    }
    Ok(())
}

fn xml_to_idl(xml: &str) -> Result<String, String> {
    let libraries = zerodds_xml::xtypes_parser::parse_type_libraries(xml)
        .map_err(|e| format!("DDS-XML types: {e:?}"))?;
    let mut idl = String::new();
    for library in &libraries {
        xml_types_idl(&library.types, &mut idl, 0)?;
    }
    Ok(idl)
}

/// Loads the document `uri` names and describes the types it defines. IDL or DDS-XML types;
/// the text decides (an XML document starts with `<`). Writes JSON (see the module note) to
/// `*out`; free it with `zerodds_string_free`.
///
/// Returns `ZERODDS_UNSUPPORTED` for a scheme no specification defines,
/// `ZERODDS_BAD_PARAMETER` for a document that cannot be read or does not parse.
///
/// # Safety
/// `uri` is a NUL-terminated string, `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_types_load(uri: *const c_char, out: *mut *mut c_char) -> c_int {
    if uri.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: NUL-terminated per the caller's contract.
    let Ok(uri) = unsafe { CStr::from_ptr(uri) }.to_str() else {
        return ZeroDdsStatus::InvalidUtf8 as c_int;
    };
    let document = match crate::qos_provider_ffi::load_document(uri) {
        Ok(d) => d,
        Err(status) => return status as c_int,
    };
    let idl = if document.trim_start().starts_with('<') {
        match xml_to_idl(&document) {
            Ok(idl) => idl,
            Err(_) => return ZeroDdsStatus::BadParameter as c_int,
        }
    } else {
        document
    };
    match describe_idl(&idl) {
        Ok(json) => match CString::new(json) {
            Ok(c) => {
                // SAFETY: `out` is writable per the caller's contract.
                unsafe { *out = c.into_raw() };
                ZeroDdsStatus::Ok as c_int
            }
            Err(_) => ZeroDdsStatus::Error as c_int,
        },
        Err(_) => ZeroDdsStatus::BadParameter as c_int,
    }
}

/// [`zerodds_types_load`] for IDL text instead of a URI.
///
/// # Safety
/// `idl` is a NUL-terminated string, `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zerodds_types_describe_idl(
    idl: *const c_char,
    out: *mut *mut c_char,
) -> c_int {
    if idl.is_null() || out.is_null() {
        return ZeroDdsStatus::BadParameter as c_int;
    }
    // SAFETY: NUL-terminated per the caller's contract.
    let Ok(idl) = unsafe { CStr::from_ptr(idl) }.to_str() else {
        return ZeroDdsStatus::InvalidUtf8 as c_int;
    };
    match describe_idl(idl) {
        Ok(json) => match CString::new(json) {
            Ok(c) => {
                // SAFETY: `out` is writable per the caller's contract.
                unsafe { *out = c.into_raw() };
                ZeroDdsStatus::Ok as c_int
            }
            Err(_) => ZeroDdsStatus::Error as c_int,
        },
        Err(_) => ZeroDdsStatus::BadParameter as c_int,
    }
}
