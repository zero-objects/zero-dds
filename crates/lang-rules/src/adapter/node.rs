// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The Node CST as the other end of a rule. Spec §3.2.
//!
//! Read [`crate::adapter::lua`] first: JavaScript's situation is Lua's —
//! "there is no type node" — with two consequences that follow from
//! JavaScript's own grammar and not from a simplification made here.
//!
//! ## Why this materialiser declares almost nothing
//!
//! [`crate::adapter::zig`] declares a struct type for every IDL
//! aggregate because Zig is statically typed and a codec function needs
//! a name to take. [`crate::adapter::lua`] still declares a *descriptor*
//! table for every aggregate, because `lua-language-server`'s `---@class`
//! reads one and because a Lua table has no order of its own, so a
//! member list has to be written down somewhere to be read back in
//! order. Neither reason holds here: this column has no LuaCATS-shaped
//! consumer to serve, and a JavaScript object literal's own property
//! order is insertion order (ECMA-262 §6.1.7.1) — the shared codec walk
//! already writes an aggregate's members in the model's declaration
//! order, so the order is carried by the generated code and needs no
//! separate table to state it again.
//!
//! What is left to declare is exactly what needs a **name at load
//! time**, because something outside the value it belongs to refers to
//! it by that name:
//!
//! - an **enumeration**, because [`crate::emit::CodecTarget::enumerator_constant`]
//!   writes `Colour.RED` and something has to bind `Colour`;
//! - a **bitmask**'s bits, because a sample or a test names one by IDL
//!   spelling and the value has to exist somewhere to be read; a
//!   bitmask's own "type" is nothing here — a member holding one is a
//!   plain `number` or `bigint`, exactly as [`crate::emit::node`]'s
//!   module documentation says;
//! - a **constant**, an IDL `const` declaration, because that is what it
//!   is for.
//!
//! A struct, a union, a bitset, a valuetype, an exception, an interface,
//! a native declaration and a `typedef` all declare **nothing**. Every
//! one of them is a plain object literal wherever the codec walk or
//! [`crate::default_node`] builds one, and "the runtime carries the
//! shape" is not a slogan here — it is the reason there is no
//! `node_member_type` in this file for [`crate::adapter::zig::zig_member_type`]
//! to answer to.
//!
//! ## This column does not join the seesaw rule catalogue
//!
//! [`crate::rule`] and [`crate::apply()`] exist to compare what different
//! backends declare for one construct, and every backend that has
//! nothing to compare has nothing to gain from joining. This column
//! declares nothing for six of the ten constructs the catalogue tracks
//! — struct, union, bitset, valuetype, exception, interface — because a
//! JavaScript object literal needs no declaration to exist. Registering
//! a `Language::Node` and a `rule::node::catalogue()` whose rules mostly
//! answer "nothing is created here" would be six rules whose only
//! purpose is to say so, in a system built to find the interesting cases
//! where two columns disagree about *something*. [`node_items`] is
//! called directly from the generator, the same as [`crate::adapter::c`]
//! is on the side that predates the catalogue.
//!
//! ## Where the IDL spelling of a name is kept
//!
//! Nothing is renamed for style: JavaScript has no export-by-case rule
//! and this column changes no letter's case. [`node::escape_member`] is
//! the identity always — [`zerodds_lang_cst::node`]'s module
//! documentation says why a property name is never renamed — so the
//! only escape in this file is [`node::escape_decl`], applied to a
//! **declaration**'s flattened name: an enumeration's own binding, a
//! bitmask bit's, a constant's. Every name it moves carries `IDL: `…`.`
//! in the comment above it, for the same reason [`crate::adapter::zig`]
//! keeps one: DDS-XTypes 1.3 §7.3.1.2.1.1 computes a member's ID under
//! `@autoid(HASH)` from the first four octets of the MD5 of its **IDL**
//! name, and the escape does not invert.

use crate::adapter::cst::{MaterialiseError, no_spelling_collision};
use crate::verbatim;
use zerodds_ast::{Ast, TypeKind, TypeNode};
use zerodds_ast_core::Id;
use zerodds_lang_cst::common::{Doc, Ident, Literal};
use zerodds_lang_cst::node::{self, Expr, Stat};

/// The language this column names in a diagnostic.
const LANGUAGE: &str = "JavaScript";

/// The marker [`crate::emit::CodecTarget::map_entry_name`] answers with.
///
/// Not a name this column declares: a `map<K,V>` entry is a plain
/// `{ key: …, value: … }` object literal wherever the codec walk builds
/// one, exactly as [`crate::adapter::lua`]'s `MAP_ENTRY` marker is for
/// Lua's tables and for the same reason.
pub const MAP_ENTRY: &str = "_zd_map_entry";

/// How far up a module chain the walk follows before it stops.
const MAX_MODULE_DEPTH: usize = 64;

/// Build the Node declarations a rule application produced.
///
/// Enumerations, bitmask bit constants and IDL constants only — the
/// module documentation says why nothing else declares. The order is
/// the model's own declaration order, exactly as
/// [`crate::adapter::zig::zig_items`] keeps it: nothing here depends on
/// where a name was written, because JavaScript resolves a top-level
/// `const` wherever it sits in one file scanned top to bottom before it
/// runs, same as every sibling.
///
/// # Errors
/// [`MaterialiseError`] when a value the output has to state does not
/// evaluate, or two declarations reach one JavaScript spelling — never a
/// silently dropped declaration.
pub fn node_items(ast: &Ast) -> Result<Vec<Stat>, MaterialiseError> {
    let mut out = Vec::new();
    let mut emitted: Vec<(String, String)> = Vec::new();

    for (id, _) in ast.types() {
        let start = out.len();
        declare(ast, id, &mut out, &mut emitted)?;
        verbatim::weave(
            verbatim::declaration_slots(ast, id, verbatim::aliases::NODE),
            "",
            start,
            &mut out,
        );
    }

    for (_, node) in ast.constants() {
        // `eval_const` and not `eval_integral`: `const string NAME` is a
        // constant that evaluates perfectly well and is not a number.
        let value = ast
            .eval_const(node.expr)
            .map_err(|_| MaterialiseError::Unevaluated {
                construct: "constant's value".to_string(),
                at: node.name.clone(),
            })?;
        let declared = node.type_.resolved();
        let is_wide = declared.is_some_and(|target| is_wide_integer(ast, target));
        let name = declared_name(ast, &node.name, node.module);
        remember(&mut emitted, &name, &node.name)?;
        out.push(Stat::Declare {
            doc: idl_note(&name, &node.name),
            kind: node::VarKind::Const,
            name: Ident::new(&name),
            value: Some(node_constant(&value, is_wide, &node.name)?),
        });
    }

    Ok(verbatim::weave_file(ast, verbatim::aliases::NODE, out))
}

/// `@verbatim` in the Node column. A declaration is an object literal or
/// a frozen table, which holds properties and not statements, so
/// `BEGIN_DECLARATION` and `END_DECLARATION` land adjacent to it.
impl verbatim::Carrier for Stat {
    fn verbatim(text: String) -> Self {
        Self::Verbatim(text)
    }

    fn take_into_body(&mut self, _: &str, _: &mut Vec<String>, _: &mut Vec<String>) -> bool {
        false
    }
}

/// The Node declarations, in a file with its own header.
///
/// A caller that only wants the declarations uses [`node_items`]; this
/// is what a generator writes to disk.
///
/// # Errors
/// As [`node_items`].
pub fn node_file(ast: &Ast) -> Result<node::File, MaterialiseError> {
    let body = node_items(ast)?;
    Ok(node::File {
        doc: Doc::lines(&[
            "Generated by ZeroDDS from an IDL source. Do not edit.",
            "",
            "The IDL spelling of every renamed name is in the comment above it.",
            "Nothing is renamed for style: a rename here is always an escape, of a",
            "JavaScript reserved word used as a top-level declaration. A member's",
            "ID under @autoid(HASH) is the MD5 of its IDL name (DDS-XTypes 1.3",
            "7.3.1.2.1.1) and the escape does not invert, which is why the",
            "comment is there.",
            "",
            "A struct, a union, a bitset, a valuetype, an exception, an",
            "interface, a native declaration and a typedef declare nothing at",
            "all here: every one of them is a plain object literal wherever the",
            "codec below or its Table 9 defaulter builds one.",
        ]),
        body,
    })
}

/// One declared type: an enumeration or a bitmask's bits. Every other
/// kind declares nothing — the module documentation says why.
fn declare(
    ast: &Ast,
    id: Id<TypeNode>,
    out: &mut Vec<Stat>,
    emitted: &mut Vec<(String, String)>,
) -> Result<(), MaterialiseError> {
    let Ok(type_node) = ast.type_(id) else {
        return Ok(());
    };

    match &type_node.kind {
        TypeKind::Enumeration { enumerators } => {
            let name = node_type_name(ast, id);
            remember(emitted, &name, &type_node.name)?;
            let mut pairs = Vec::new();
            for enumerator in enumerators {
                if let Ok(enum_node) = ast.enumerator(*enumerator) {
                    pairs.push((field_name(&enum_node.name), enum_node.name.clone()));
                }
            }
            no_spelling_collision(as_pairs(&pairs), LANGUAGE, &name)?;
            let mut fields = Vec::new();
            for enumerator in enumerators {
                let Ok(enum_node) = ast.enumerator(*enumerator) else {
                    return Err(MaterialiseError::NoNode {
                        kind: "enumerator",
                        at: crate::adapter::cst::handle_of(*enumerator),
                    });
                };
                let value = ast.effective_enumerator_value(*enumerator).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "enumerator value".to_string(),
                        at: enum_node.name.clone(),
                    }
                })?;
                fields.push((Ident::new(&field_name(&enum_node.name)), Expr::int(value)));
            }
            out.push(Stat::Declare {
                doc: with_idl_note(
                    Doc::lines(&[
                        "An enumeration, as a frozen object of its enumerator names.",
                        "`Object.freeze` is not an encoding decision — nothing here",
                        "reads a value off the wire this way — it only refuses an",
                        "accidental write to a shared constant.",
                    ]),
                    &name,
                    &type_node.name,
                ),
                kind: node::VarKind::Const,
                name: Ident::new(&name),
                value: Some(Expr::Call {
                    callee: Box::new(Expr::member(Expr::name("Object"), "freeze")),
                    args: vec![Expr::Object(fields)],
                }),
            });
        }

        TypeKind::Bitmask { bits } => {
            let bound = ast.effective_bit_bound(id);
            let wide = bound > 32;
            let prefix = module_prefix(ast, id);
            for bit in bits {
                let Ok(bit_node) = ast.bit_value(*bit) else {
                    return Err(MaterialiseError::NoNode {
                        kind: "bit",
                        at: crate::adapter::cst::handle_of(*bit),
                    });
                };
                let position = ast.effective_bit_position(*bit).ok_or_else(|| {
                    MaterialiseError::Unevaluated {
                        construct: "bit position".to_string(),
                        at: bit_node.name.clone(),
                    }
                })?;
                let constant = node::escape_decl(&format!("{prefix}{}", bit_node.name));
                remember(emitted, &constant, &bit_node.name)?;
                let value = if wide {
                    Expr::Binary {
                        op: "<<",
                        lhs: Box::new(Expr::Literal(Literal::Int("1n".to_string()))),
                        rhs: Box::new(Expr::Literal(Literal::Int(format!("{position}n")))),
                    }
                } else {
                    Expr::Binary {
                        op: "<<",
                        lhs: Box::new(Expr::int(1)),
                        rhs: Box::new(Expr::int(i128::from(position))),
                    }
                };
                out.push(Stat::Declare {
                    doc: idl_note(&constant, &bit_node.name),
                    kind: node::VarKind::Const,
                    name: Ident::new(&constant),
                    value: Some(value),
                });
            }
        }

        // Everything else is a plain object literal wherever it is
        // built and declares nothing here.
        TypeKind::Primitive(_)
        | TypeKind::String { .. }
        | TypeKind::Fixed { .. }
        | TypeKind::Sequence { .. }
        | TypeKind::Array { .. }
        | TypeKind::Map { .. }
        | TypeKind::Alias { .. }
        | TypeKind::Bitset { .. }
        | TypeKind::Structure { .. }
        | TypeKind::Union { .. }
        | TypeKind::Native
        | TypeKind::ValueBox { .. }
        | TypeKind::Valuetype { .. }
        | TypeKind::ObjectRef { .. }
        | TypeKind::AnnotationType { .. }
        | TypeKind::Any
        | TypeKind::ValueBase => {}
    }
    Ok(())
}

/// Whether a resolved type is the 64-bit primitive pair — the one
/// boundary where a constant needs a `bigint` literal rather than a
/// `number` one, exactly as [`crate::emit::node::Node::scalar`] draws it
/// for a codec local.
fn is_wide_integer(ast: &Ast, id: Id<TypeNode>) -> bool {
    matches!(
        ast.type_(id).map(|node| node.kind.clone()),
        Ok(TypeKind::Primitive(
            zerodds_ast::PrimitiveKind::Int64 | zerodds_ast::PrimitiveKind::UInt64
        ))
    )
}

/// A declared type's flattened JavaScript identifier.
///
/// The scoped name with the module separators joined by `_`, exactly as
/// [`crate::adapter::zig::zig_type_name`] flattens: `::common::util::Colour`
/// is `common_util_Colour`. No case is changed. This is also the tag
/// `crate::emit::node::Node::type_name` falls back to for a construct
/// that declares nothing here — a codec function's name is built from
/// it whether or not the type it codes has a declaration of its own.
#[must_use]
pub fn node_type_name(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return "ZdUnnamed".to_string();
    };
    match ast.scoped_name_of(any) {
        Some(scoped) => flatten(scoped.segments()),
        None => "ZdUnnamed".to_string(),
    }
}

/// The flattened module path a file-scope declaration carries: a
/// bitmask's bit constants, which sit outside the type their mask
/// belongs to and need the whole scope in their own name to keep two
/// modules' `READY` apart.
fn module_prefix(ast: &Ast, id: Id<TypeNode>) -> String {
    let Ok(any) = ast.any_of_type(id) else {
        return String::new();
    };
    let Some(scoped) = ast.scoped_name_of(any) else {
        return String::new();
    };
    let segments = scoped.segments();
    if segments.len() <= 1 {
        return String::new();
    }
    let mut prefix = segments[..segments.len() - 1].join("_");
    prefix.push('_');
    prefix
}

/// The Node name a declaration outside the type arena takes — a
/// constant, whose scope comes from the module it was declared in
/// rather than from [`Ast::scoped_name_of`].
fn declared_name(ast: &Ast, name: &str, module: Id<zerodds_ast::ModuleNode>) -> String {
    let mut segments: Vec<String> = Vec::new();
    let mut current = Some(module);
    let mut depth = 0;
    while let Some(id) = current {
        if depth > MAX_MODULE_DEPTH {
            break;
        }
        depth += 1;
        let Ok(node) = ast.module(id) else { break };
        if node.name.is_empty() {
            break;
        }
        segments.push(node.name.clone());
        current = node.parent;
    }
    segments.reverse();
    segments.push(name.to_string());
    node::escape_decl(&segments.join("_"))
}

/// Join scoped segments with `_` and escape the result — the identity
/// unless the joined name lands on a JavaScript reserved word.
fn flatten(segments: &[String]) -> String {
    node::escape_decl(&segments.join("_"))
}

/// The JavaScript spelling of a member, an enumerator, a bitfield or a
/// map-entry half. The identity, always: [`zerodds_lang_cst::node`]'s
/// module documentation says why a property name never collides with
/// anything JavaScript reserves.
#[must_use]
pub fn field_name(name: &str) -> String {
    node::escape_member(name)
}

/// The declaration's own IDL name, where the Node spelling moved it.
fn idl_note(spelled: &str, declared: &str) -> Doc {
    if spelled == declared {
        return Doc::none();
    }
    Doc::line(&format!("IDL: `{declared}`."))
}

/// The same, appended to a documentation comment the caller wrote.
fn with_idl_note(mut doc: Doc, spelled: &str, declared: &str) -> Doc {
    if spelled != declared {
        doc.push(&format!("IDL: `{declared}`."));
    }
    doc
}

/// Record a declared name, refusing a second declaration that reaches
/// the same JavaScript spelling.
///
/// One file scope holds every top-level `const`, so this is a
/// whole-file check and not a per-scope one — which is the difference
/// from [`no_spelling_collision`], used for the members of one
/// enumeration.
fn remember(
    emitted: &mut Vec<(String, String)>,
    spelled: &str,
    declared: &str,
) -> Result<(), MaterialiseError> {
    if let Some((_, first)) = emitted.iter().find(|(other, _)| other == spelled) {
        return Err(MaterialiseError::SpellingCollision {
            first: first.clone(),
            second: declared.to_string(),
            spelled: spelled.to_string(),
            language: LANGUAGE,
            at: "the generated file".to_string(),
        });
    }
    emitted.push((spelled.to_string(), declared.to_string()));
    Ok(())
}

/// Borrow a list of owned name pairs for [`no_spelling_collision`].
fn as_pairs(pairs: &[(String, String)]) -> Vec<(&str, &str)> {
    pairs
        .iter()
        .map(|(spelled, declared)| (spelled.as_str(), declared.as_str()))
        .collect()
}

/// The Node expression a top-level constant's value takes.
///
/// `is_wide` says the declared type is a 64-bit primitive, so the
/// literal needs the trailing `n` [`zerodds_lang_cst::node::Expr::Literal`]'s
/// documentation carries — a `number` cannot hold every `i64` or `u64`
/// exactly, and a bare decimal here would already have lost precision
/// before anything read it.
///
/// # Errors
/// [`MaterialiseError::NoLanguageForm`] for a constant form this column
/// has no literal for.
fn node_constant(
    value: &zerodds_ast::ConstValue,
    is_wide: bool,
    at: &str,
) -> Result<Expr, MaterialiseError> {
    use zerodds_ast::ConstValue;
    let refuse = |construct: &str| MaterialiseError::NoLanguageForm {
        construct: construct.to_string(),
        language: LANGUAGE,
        at: at.to_string(),
    };
    Ok(match value {
        ConstValue::Boolean(flag) => Expr::bool(*flag),
        ConstValue::Integer(number) => {
            if is_wide {
                Expr::Literal(Literal::Int(format!("{number}n")))
            } else {
                Expr::int(*number)
            }
        }
        // The code point as a one-character string: an IDL `char` and
        // an IDL `wchar` are both held that way here.
        ConstValue::Char(character) => Expr::Literal(Literal::Char(*character)),
        // Refused rather than folded, the same answer the `WString` arm
        // below gives for the plural case: `Literal::Char` holds a Rust
        // `char`, which a lone surrogate is not, and substituting NUL
        // would put a `const wchar` in the generated file that no reader
        // could tell from one the source declared as `'\0'`.
        ConstValue::WChar(point) => {
            Expr::Literal(Literal::Char(char::from_u32(*point).ok_or_else(|| {
                refuse("a `wchar` constant that is not a Unicode scalar value")
            })?))
        }
        ConstValue::Float { mantissa, exponent } => Expr::Literal(Literal::Float(
            crate::adapter::constant::float_text(mantissa, exponent.as_deref()),
        )),
        ConstValue::Fixed { digits, scale } => {
            Expr::string(&crate::adapter::constant::fixed_decimal(digits, *scale))
        }
        ConstValue::String(text) => Expr::string(text),
        ConstValue::WString(points) => Expr::string(
            &crate::adapter::constant::wstring_utf8(points)
                .ok_or_else(|| refuse("a `wstring` constant holding a lone surrogate"))?,
        ),
        _ => return Err(refuse("a constant form this binding has no literal for")),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
    use super::*;

    #[test]
    fn a_member_name_never_moves() {
        for name in ["class", "new", "constructor", "x"] {
            assert_eq!(field_name(name), name);
        }
    }

    #[test]
    fn a_map_entry_has_no_declared_name() {
        assert_eq!(MAP_ENTRY, "_zd_map_entry");
    }
}
