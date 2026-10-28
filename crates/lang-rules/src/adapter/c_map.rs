// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The C binding of a map. DDS-XTypes 1.3 §7.5.1.3.1 and §7.5.1.3.2.
//!
//! "Map types shall be represented as a collection of structures that
//! contain a member of the key type followed by a member of the element
//! type. A set of methods which create, delete and manipulate objects of
//! the map type shall also be generated."
//!
//! ## The name
//!
//! "For maps whose key type is a Primitive Type the name of the map type
//! shall be constructed by combining the key type name with the element
//! type name": `[key_type][fully_qualified_element_type]Map`, so
//! `UInt32MyModule_MyElementTypeMap` for `map<unsigned long,
//! MyModule::MyElementType>`. The key type name is the name the Type
//! System gives the primitive (`Int32`, `UInt64`, `Char8`, …) or `String`;
//! the element type name is that of a primitive or the flattened scoped
//! name of a declared type. A bounded map takes `_b<N>` after `Map`, and a
//! bounded string `_b<N>` after its name, because a name is also the name
//! of the routine that reads and writes the map, and two bounds are two
//! wire forms. The entry is `<map name>Element`.
//!
//! A map whose key is no primitive, string, enumeration or bitmask has no
//! name in the clause and keeps the structural name `zd_map…`; it has no
//! operations, because "implementations are only required to support keys
//! of types UInt32, UInt64, and `String<Char8>`" and a key of another type
//! cannot be compared with `==`.
//!
//! ## The type
//!
//! `{ uint32_t _maximum; uint32_t _length; Entry *_buffer; bool _release; }`
//! — [`crate::adapter::c`] writes it. `_maximum` is the capacity Table 47
//! speaks of (`get_max_size`, `set_max_size`), `_length` the size.
//! `_release` says who owns `_buffer`: the operations of
//! `runtime/c/include/zerodds_map.h` free and replace it when it is set,
//! and leave it alone otherwise (the arena of a generated reader).
//!
//! ## The operations, and the maps of every type
//!
//! [`maps`] writes one `ZD_MAP_DEFINE(…)` per map the model uses, and,
//! as the clause asks, "for any type T, the declaration and implementation
//! of the map types having element type T and key types UInt32, UInt64,
//! and String shall be generated alongside the implementation code for
//! element type T": the three maps of each declared structure, union,
//! enumeration, bitmask, bitset and typedef, unless the model has used
//! one of them.

use zerodds_ast::{Ast, Bound, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::c;
use zerodds_lang_cst::common::{Doc, Ident};

use super::c::{bound_tag, c_type_name, type_name_at, type_tag_at};
use super::cst::MaterialiseError;

/// The name the Type System gives a primitive. XTypes 1.3 §7.4.1.1.4:
/// `MapEntry_Int32_Float32`.
const fn type_system_name(kind: PrimitiveKind) -> &'static str {
    match kind {
        PrimitiveKind::Boolean => "Boolean",
        PrimitiveKind::Octet => "Byte",
        PrimitiveKind::Int8 => "Int8",
        PrimitiveKind::UInt8 => "UInt8",
        PrimitiveKind::Int16 => "Int16",
        PrimitiveKind::UInt16 => "UInt16",
        PrimitiveKind::Int32 => "Int32",
        PrimitiveKind::UInt32 => "UInt32",
        PrimitiveKind::Int64 => "Int64",
        PrimitiveKind::UInt64 => "UInt64",
        PrimitiveKind::Float32 => "Float32",
        PrimitiveKind::Float64 => "Float64",
        PrimitiveKind::Float128 => "Float128",
        PrimitiveKind::Char => "Char8",
        PrimitiveKind::WChar => "Char16",
    }
}

/// How two keys are compared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyKind {
    /// `==`.
    Scalar,
    /// By the characters.
    String,
}

impl KeyKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Scalar => "SCALAR",
            Self::String => "STRING",
        }
    }
}

/// What a key of this type is, or `None` for a type that is no key here.
fn key_kind(ast: &Ast, key: Id<TypeNode>) -> Option<KeyKind> {
    let mut current = key;
    for _ in 0..64 {
        let node = ast.type_(current).ok()?;
        match &node.kind {
            TypeKind::Primitive(PrimitiveKind::Float128) => return None,
            TypeKind::Primitive(_) | TypeKind::Enumeration { .. } | TypeKind::Bitmask { .. } => {
                return Some(KeyKind::Scalar);
            }
            TypeKind::String { .. } => return Some(KeyKind::String),
            TypeKind::Alias { aliased } => current = aliased.resolved()?,
            _ => return None,
        }
    }
    None
}

/// The name a key contributes: `UInt32`, `String`, the name of the
/// enumeration. `None` where [`key_kind`] is.
///
/// zerodds-lint: recursion-depth 64 (unnamed-typedef chains; bounded by
/// `MAX_DEPTH`, past which the key has no name here and the caller keeps
/// the structural one).
fn key_part(ast: &Ast, key: Id<TypeNode>, depth: usize) -> Option<String> {
    if depth > super::c::MAX_DEPTH {
        return None;
    }
    key_kind(ast, key)?;
    let node = ast.type_(key).ok()?;
    Some(match &node.kind {
        TypeKind::Primitive(kind) => type_system_name(*kind).to_string(),
        TypeKind::String { width, bound } => format!(
            "{}{}",
            if *width == CharWidth::Char {
                "String"
            } else {
                "WString"
            },
            bound_tag(ast, bound)
        ),
        TypeKind::Alias { aliased } if node.name.is_empty() => {
            return key_part(ast, aliased.resolved()?, depth + 1);
        }
        _ => type_name_at(ast, key, depth + 1),
    })
}

/// The name an element contributes: a primitive's, `String`, or the
/// fully qualified name of a declared type.
///
/// zerodds-lint: recursion-depth 64 (unnamed-typedef chains; bounded by
/// `MAX_DEPTH`, past which `type_tag_at` names the type by its arena slot).
fn element_part(ast: &Ast, element: Id<TypeNode>, depth: usize) -> String {
    if depth > super::c::MAX_DEPTH {
        return type_tag_at(ast, element, depth + 1);
    }
    let Ok(node) = ast.type_(element) else {
        return type_tag_at(ast, element, depth + 1);
    };
    match &node.kind {
        TypeKind::Primitive(kind) => type_system_name(*kind).to_string(),
        TypeKind::String { width, bound } => format!(
            "{}{}",
            if *width == CharWidth::Char {
                "String"
            } else {
                "WString"
            },
            bound_tag(ast, bound)
        ),
        TypeKind::Alias { aliased } if node.name.is_empty() => match aliased.resolved() {
            Some(target) => element_part(ast, target, depth + 1),
            None => type_tag_at(ast, element, depth + 1),
        },
        _ => type_tag_at(ast, element, depth + 1),
    }
}

/// The C name of a map type, where the clause names it.
///
/// `None` for a map whose key the clause gives no name for; the caller
/// keeps the structural name then.
pub(crate) fn map_type_name(
    ast: &Ast,
    key: Id<TypeNode>,
    value: Id<TypeNode>,
    bound: &Bound,
    depth: usize,
) -> Option<String> {
    let key = key_part(ast, key, depth)?;
    Some(format!(
        "{key}{}Map{}",
        element_part(ast, value, depth),
        bound_tag(ast, bound)
    ))
}

/// Whether an element is passed by value ("for elements with primitive
/// types, this argument shall be generated as the type and not as a
/// pointer to the primitive type", §7.5.1.3.2.11): a primitive, an
/// enumeration, a bitmask, a string and a `fixed` are one scalar in C.
fn passed_by_value(ast: &Ast, element: Id<TypeNode>) -> bool {
    let mut current = element;
    for _ in 0..64 {
        let Ok(node) = ast.type_(current) else {
            return false;
        };
        match &node.kind {
            TypeKind::Primitive(PrimitiveKind::Float128) => return false,
            TypeKind::Primitive(_)
            | TypeKind::Enumeration { .. }
            | TypeKind::Bitmask { .. }
            | TypeKind::String { .. }
            | TypeKind::Fixed { .. } => return true,
            TypeKind::Alias { aliased } => match aliased.resolved() {
                Some(next) => current = next,
                None => return false,
            },
            _ => return false,
        }
    }
    false
}

/// The entry struct of a map named `map`.
#[must_use]
pub(crate) fn entry_name(map: &str) -> String {
    format!("{map}Element")
}

/// The map struct, with its typedef.
pub(crate) fn map_decl(name: &str, entry: &str) -> Vec<c::Decl> {
    let member = |type_: c::Type, field: &str| c::Member {
        doc: Doc::none(),
        type_,
        name: Ident::new(field),
        dims: Vec::new(),
        bits: None,
    };
    vec![
        c::Decl::Typedef(c::Typedef {
            doc: Doc::none(),
            type_: c::Type::StructTag(Ident::new(name)),
            name: Ident::new(name),
            dims: Vec::new(),
        }),
        c::Decl::Struct(c::Struct {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::lines(&[
                "A map (XTypes 1.3 7.5.1.3.2): the entries in the order they were inserted,",
                "the capacity `_maximum` and the size `_length`. `_release` is set when",
                "`_buffer` is a heap block the operations of zerodds_map.h own.",
            ]),
            name: Some(Ident::new(name)),
            members: vec![
                member(c::Type::Primitive(c::Primitive::UInt32), "_maximum"),
                member(c::Type::Primitive(c::Primitive::UInt32), "_length"),
                member(
                    c::Type::Pointer(Box::new(c::Type::Named(Ident::new(entry)))),
                    "_buffer",
                ),
                member(c::Type::Primitive(c::Primitive::Bool), "_release"),
            ],
        }),
    ]
}

/// One map, as far as the operations need to know it.
struct Map {
    name: String,
    entry: String,
    key: String,
    element: String,
    kind: KeyKind,
    by_value: bool,
    bound: u64,
}

impl Map {
    fn define(&self) -> c::Decl {
        c::Decl::Verbatim(format!(
            "ZD_MAP_DEFINE({}, {}, {}, {}, {}, {}, {}u)\n",
            self.name,
            self.entry,
            self.key,
            self.element,
            self.kind.as_str(),
            if self.by_value { "VAL" } else { "PTR" },
            self.bound
        ))
    }
}

/// The operations of every map of the model, and the maps of every type.
///
/// # Errors
/// [`MaterialiseError::Unevaluated`] for a bound that does not evaluate.
///
/// `emitted` are the C names of the types the header declares.
pub fn maps(
    ast: &Ast,
    order: &[Id<TypeNode>],
    emitted: &[String],
) -> Result<Vec<c::Decl>, MaterialiseError> {
    let mut used: Vec<Map> = Vec::new();
    for id in order {
        let Ok(node) = ast.type_(*id) else { continue };
        let TypeKind::Map { key, value, bound } = &node.kind else {
            continue;
        };
        let (Some(key), Some(value)) = (key.resolved(), value.resolved()) else {
            continue;
        };
        let Some(kind) = key_kind(ast, key) else {
            continue;
        };
        let name = c_type_name(ast, *id);
        if used.iter().any(|map| map.name == name) {
            continue;
        }
        used.push(Map {
            entry: entry_name(&name),
            name,
            key: c_type_name(ast, key),
            element: c_type_name(ast, value),
            kind,
            by_value: passed_by_value(ast, value),
            bound: bound_value(ast, bound)?,
        });
    }

    // The maps of every declared type, with the three mandatory keys.
    let mut alongside: Vec<Map> = Vec::new();
    for id in order {
        let Ok(node) = ast.type_(*id) else { continue };
        let declared = match &node.kind {
            TypeKind::Structure { .. }
            | TypeKind::Union { .. }
            | TypeKind::Enumeration { .. }
            | TypeKind::Bitmask { .. }
            | TypeKind::Bitset { .. } => true,
            TypeKind::Alias { .. } => !node.name.is_empty(),
            _ => false,
        };
        if !declared {
            continue;
        }
        let element_name = c_type_name(ast, *id);
        // Only a type this header declares can be the element of a map.
        if !emitted.contains(&element_name) {
            continue;
        }
        for (key_name, key_c, kind) in [
            ("UInt32", "uint32_t", KeyKind::Scalar),
            ("UInt64", "uint64_t", KeyKind::Scalar),
            ("String", "zd_string", KeyKind::String),
        ] {
            let name = format!("{key_name}{element_name}Map");
            if used.iter().any(|map| map.name == name)
                || alongside.iter().any(|map| map.name == name)
            {
                continue;
            }
            alongside.push(Map {
                entry: entry_name(&name),
                name,
                key: key_c.to_string(),
                element: element_name.clone(),
                kind,
                by_value: passed_by_value(ast, *id),
                bound: 0,
            });
        }
    }

    if used.is_empty() && alongside.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = vec![c::Decl::Verbatim(
        "#include \"zerodds_map.h\"\n".to_string(),
    )];
    // The declarations of the maps the model did not use; the others were
    // declared where the model put them.
    for map in &alongside {
        out.push(c::Decl::Typedef(c::Typedef {
            doc: Doc::none(),
            type_: c::Type::StructTag(Ident::new(&map.entry)),
            name: Ident::new(&map.entry),
            dims: Vec::new(),
        }));
        out.extend(map_decl(&map.name, &map.entry));
        out.push(c::Decl::Struct(c::Struct {
            body_text: zerodds_lang_cst::common::BodyText::none(),
            doc: Doc::line("One entry of a map (XTypes 1.3 7.5.1.3.2): the key, then the element."),
            name: Some(Ident::new(&map.entry)),
            members: vec![
                c::Member {
                    doc: Doc::none(),
                    type_: c::Type::Named(Ident::new(&map.key)),
                    name: Ident::new("key"),
                    dims: Vec::new(),
                    bits: None,
                },
                c::Member {
                    doc: Doc::none(),
                    type_: c::Type::Named(Ident::new(&map.element)),
                    name: Ident::new("value"),
                    dims: Vec::new(),
                    bits: None,
                },
            ],
        }));
    }
    for map in used.iter().chain(alongside.iter()) {
        out.push(map.define());
    }
    Ok(out)
}

fn bound_value(ast: &Ast, bound: &Bound) -> Result<u64, MaterialiseError> {
    match bound.expr() {
        None => Ok(0),
        Some(expr) => {
            let value = ast
                .eval_integral(expr)
                .map_err(|_| MaterialiseError::Unevaluated {
                    construct: "map bound".to_string(),
                    at: "map".to_string(),
                })?;
            u64::try_from(value).map_err(|_| MaterialiseError::Unevaluated {
                construct: format!("map bound — it evaluates to {value}"),
                at: "map".to_string(),
            })
        }
    }
}
