// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `Config::rust_type` and `Config::rust_type_with`: which IDL declaration
//! the generated Rust holds as which Rust type.
//!
//! The paths a build script names are resolved against the composed model
//! here, once per unit, and handed to the emitter as nodes
//! (`zerodds_lang_rules::adapter::cst::RustTypeScope`). What the emitter
//! cannot know — whether the Rust type's `WireRepr::Idl` is the IDL type's
//! — is stated in the generated file as a compile-time assertion, so a
//! mismatch fails the consumer's build at a line that names both.

use core::fmt::Write as _;

use zerodds_ast::{Ast, CharWidth, PrimitiveKind, TypeKind, TypeNode};
use zerodds_ast_core::{Id, NodeId};
use zerodds_lang_cst::common::{Ident, Path};
use zerodds_lang_cst::rust;
use zerodds_lang_rules::adapter::cst::RustTypeMapping;

/// One `rust_type` or `rust_type_with` call, as given.
#[derive(Debug, Clone)]
pub(crate) struct RustTypeOverride {
    /// `app::T`, `app::S.m` or `app::U.branch`.
    pub(crate) idl_path: String,
    /// The Rust type, as a path with optional generic arguments.
    pub(crate) rust_type: String,
    /// The adapter value's path, for `rust_type_with`.
    pub(crate) adapter: Option<String>,
}

/// The overrides that named a declaration of one unit.
#[derive(Debug, Default)]
pub(crate) struct Resolved {
    /// Each declaration's node and the Rust type standing for it.
    pub(crate) mappings: Vec<(NodeId, RustTypeMapping)>,
    /// The compile-time assertions, one per mapping, as Rust source.
    pub(crate) assertions: String,
    /// Per override, in the order given: whether this unit declares it.
    pub(crate) found: Vec<bool>,
}

/// Resolve every override against one unit's model.
///
/// An override whose path this unit does not declare is not an error
/// here — another unit of the same call may declare it, and the caller
/// checks that one did. An override whose path the unit does declare, but
/// which no Rust type can stand for, is.
///
/// # Errors
/// The override's IDL path and what is wrong with it.
pub(crate) fn resolve(
    ast: &Ast,
    overrides: &[RustTypeOverride],
) -> Result<Resolved, (String, String)> {
    let mut resolved = Resolved {
        found: vec![false; overrides.len()],
        ..Resolved::default()
    };
    for (index, given) in overrides.iter().enumerate() {
        let fail = |detail: String| (given.idl_path.clone(), detail);
        let path = given.idl_path.trim();
        let path = path.strip_prefix("::").unwrap_or(path);
        let (type_path, member) = match path.rsplit_once('.') {
            Some((type_path, member)) => (type_path, Some(member)),
            None => (path, None),
        };
        let Some((type_id, type_node)) = ast
            .types()
            .find(|(id, node)| !node.name.is_empty() && scoped_name(ast, *id, node) == type_path)
        else {
            continue;
        };
        resolved.found[index] = true;
        let rust_type = parse_type(&given.rust_type).map_err(fail)?;
        let adapter = given
            .adapter
            .as_deref()
            .map(parse_path)
            .transpose()
            .map_err(fail)?;

        let (node_id, declared, name) = match member {
            None => {
                let TypeKind::Alias { aliased } = &type_node.kind else {
                    return Err(fail(format!(
                        "`{type_path}` is a {}; `rust_type` takes a typedef (`{type_path}`), a \
                         struct member (`Struct.member`) or a union branch (`Union.branch`)",
                        type_node.kind.as_str()
                    )));
                };
                let Some(aliased) = aliased.resolved() else {
                    return Err(fail(format!("the typedef `{type_path}` does not resolve")));
                };
                (type_node.node_id, aliased, type_node.name.as_str())
            }
            Some(member) => {
                let member_node = match &type_node.kind {
                    TypeKind::Structure { .. } => ast
                        .members_of(type_id)
                        .iter()
                        .filter_map(|id| ast.member(*id).ok())
                        .find(|node| node.name == member),
                    TypeKind::Union { .. } => ast
                        .cases_of(type_id)
                        .iter()
                        .filter_map(|id| ast.case(*id).ok())
                        .filter_map(|case| ast.member(case.member).ok())
                        .find(|node| node.name == member),
                    other => {
                        return Err(fail(format!(
                            "`{type_path}` is a {}; a member path names a struct member or a \
                             union branch",
                            other.as_str()
                        )));
                    }
                };
                let Some(member_node) = member_node else {
                    return Err(fail(format!(
                        "`{type_path}` declares no member `{member}` (an inherited member is \
                         named through the struct that declares it)"
                    )));
                };
                let Some(declared) = member_node.type_.resolved() else {
                    return Err(fail(format!("the type of `{path}` does not resolve")));
                };
                (member_node.node_id, declared, member_node.name.as_str())
            }
        };

        let idl = idl_rust_spelling(ast, declared).map_err(fail)?;
        // `pub type ItemId = ItemId;` inside the generated module names
        // the alias itself, and rustc reports a cycle at a line the user
        // did not write.
        if member.is_none()
            && let rust::Type::Named { path, args } = &rust_type
            && path.segments().len() == 1
            && args.is_empty()
            && path.segments()[0].as_str() == name
        {
            return Err(fail(format!(
                "the Rust type `{}` has the typedef's own name, and the generated `pub type \
                 {name} = {name};` would name itself; spell it as a path (`crate::…`)",
                given.rust_type.trim()
            )));
        }

        let type_text = zerodds_lang_render::rust::type_(&rust_type);
        let _ = writeln!(
            resolved.assertions,
            "\n// rust_type(\"{}\", \"{type_text}\"): the IDL type is held to `WireRepr::Idl = {idl}`.",
            path
        );
        match &adapter {
            None => {
                let _ = writeln!(
                    resolved.assertions,
                    "const _: () = zerodds_wire::repr::idl_type_is::<{type_text}, {idl}>();"
                );
            }
            Some(adapter) => {
                let adapter_text = adapter
                    .segments()
                    .iter()
                    .map(Ident::as_str)
                    .collect::<Vec<_>>()
                    .join("::");
                let _ = writeln!(
                    resolved.assertions,
                    "const _: () = zerodds_wire::repr::idl_type_with_is::<{adapter_text}, \
                     {type_text}, {idl}>();"
                );
            }
        }
        resolved
            .mappings
            .push((node_id, RustTypeMapping { rust_type, adapter }));
    }
    Ok(resolved)
}

/// A declaration's scoped name without the leading `::`.
fn scoped_name(ast: &Ast, id: Id<TypeNode>, node: &TypeNode) -> String {
    ast.scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))
        .map(|scoped| {
            let text = scoped.to_string();
            text.strip_prefix("::").unwrap_or(&text).to_string()
        })
        .unwrap_or_else(|| node.name.clone())
}

/// The Rust spelling of `WireRepr::Idl` for the IDL type behind every
/// typedef from `type_`: `str` for a `string`, the primitive's Rust type
/// for a primitive.
///
/// # Errors
/// A type no `WireRepr` stands for, named.
fn idl_rust_spelling(ast: &Ast, start: Id<TypeNode>) -> Result<&'static str, String> {
    let mut type_ = start;
    for _ in 0..64 {
        let node = ast
            .type_(type_)
            .map_err(|_| String::from("its IDL type is not in the model"))?;
        match &node.kind {
            TypeKind::Alias { aliased } => {
                type_ = aliased
                    .resolved()
                    .ok_or_else(|| String::from("its IDL type does not resolve"))?;
            }
            TypeKind::String {
                width: CharWidth::Char,
                ..
            } => return Ok("str"),
            TypeKind::String {
                width: CharWidth::WChar,
                ..
            } => return Err(not_mappable("wstring")),
            TypeKind::Primitive(kind) => {
                return match kind {
                    PrimitiveKind::Boolean => Ok("bool"),
                    PrimitiveKind::Octet | PrimitiveKind::UInt8 => Ok("u8"),
                    PrimitiveKind::Int8 => Ok("i8"),
                    PrimitiveKind::Int16 => Ok("i16"),
                    PrimitiveKind::UInt16 => Ok("u16"),
                    PrimitiveKind::Int32 => Ok("i32"),
                    PrimitiveKind::UInt32 => Ok("u32"),
                    PrimitiveKind::Int64 => Ok("i64"),
                    PrimitiveKind::UInt64 => Ok("u64"),
                    PrimitiveKind::Float32 => Ok("f32"),
                    PrimitiveKind::Float64 => Ok("f64"),
                    PrimitiveKind::Char => Ok("char"),
                    PrimitiveKind::WChar | PrimitiveKind::Float128 => {
                        Err(not_mappable(kind.as_str()))
                    }
                };
            }
            other => return Err(not_mappable(other.as_str())),
        }
    }
    Err(String::from("its typedef chain is longer than 64"))
}

fn not_mappable(kind: &str) -> String {
    format!(
        "its IDL type is `{kind}`; a Rust type stands for a `string` or a primitive other than \
         `wchar` and `long double` (`WireRepr::Idl` is `str` or the primitive's Rust type)"
    )
}

/// A Rust type path with optional generic arguments:
/// `crate::time::UtcTimestamp`, `core::num::NonZeroU32`,
/// `chrono::DateTime<chrono::Utc>`, `std::borrow::Cow<'static, str>`.
///
/// # Errors
/// Text that is no such path, with what was expected where.
pub(crate) fn parse_type(text: &str) -> Result<rust::Type, String> {
    let tokens = tokens(text)?;
    let mut at = 0;
    let parsed = type_at(&tokens, &mut at, 0)?;
    if at != tokens.len() {
        return Err(format!(
            "the Rust type `{}` is no type path: `{}` after the type",
            text.trim(),
            tokens[at]
        ));
    }
    Ok(parsed)
}

/// An adapter's path: a unit struct or a constant, no generic arguments.
///
/// # Errors
/// Text that is no such path.
fn parse_path(text: &str) -> Result<Path, String> {
    match parse_type(text) {
        Ok(rust::Type::Named { path, args }) if args.is_empty() => Ok(path),
        _ => Err(format!(
            "the adapter `{}` is no path to a value (a unit struct or a constant, no generic \
             arguments)",
            text.trim()
        )),
    }
}

/// The tokens of a type path: identifiers, lifetimes, `::`, `<`, `>`, `,`.
fn tokens(text: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        match c {
            c if c.is_whitespace() => {}
            '<' | '>' | ',' => out.push(c.to_string()),
            ':' => {
                if chars.next_if(|(_, next)| *next == ':').is_none() {
                    return Err(format!("the Rust type `{}` has a lone `:`", text.trim()));
                }
                out.push(String::from("::"));
            }
            c if c == '\'' || c == '_' || c.is_ascii_alphabetic() => {
                let mut end = start + c.len_utf8();
                while let Some((at, next)) =
                    chars.next_if(|(_, next)| *next == '_' || next.is_ascii_alphanumeric())
                {
                    end = at + next.len_utf8();
                }
                out.push(text[start..end].to_string());
            }
            other => {
                return Err(format!(
                    "the Rust type `{}` is no type path: `{other}` (a path with optional generic \
                     arguments, such as `chrono::DateTime<chrono::Utc>`)",
                    text.trim()
                ));
            }
        }
    }
    if out.is_empty() {
        return Err(String::from("the Rust type is empty"));
    }
    Ok(out)
}

/// One type at `tokens[*at]`.
///
/// zerodds-lint: recursion-depth 16 (one level per nested generic
/// argument list, refused past `depth` 16)
fn type_at(tokens: &[String], at: &mut usize, depth: usize) -> Result<rust::Type, String> {
    if depth > 16 {
        return Err(String::from(
            "the Rust type nests its generic arguments past 16",
        ));
    }
    let mut segments: Vec<String> = Vec::new();
    // A leading `::` names an extern crate; kept as an empty first segment,
    // which renders as written.
    if tokens.get(*at).is_some_and(|token| token == "::") {
        segments.push(String::new());
        *at += 1;
    }
    loop {
        let Some(token) = tokens.get(*at) else {
            return Err(String::from("the Rust type ends where a name is expected"));
        };
        if !is_identifier(token) {
            return Err(format!("`{token}` where a name is expected"));
        }
        segments.push(token.clone());
        *at += 1;
        if tokens.get(*at).is_some_and(|token| token == "::") {
            *at += 1;
            continue;
        }
        break;
    }
    let mut args = Vec::new();
    if tokens.get(*at).is_some_and(|token| token == "<") {
        *at += 1;
        loop {
            let Some(token) = tokens.get(*at) else {
                return Err(String::from("an unclosed `<`"));
            };
            if token == ">" {
                *at += 1;
                break;
            }
            if token.starts_with('\'') {
                // A lifetime argument, `'static`: written as it is.
                args.push(rust::Type::named(Path::single(token)));
                *at += 1;
            } else {
                args.push(type_at(tokens, at, depth + 1)?);
            }
            match tokens.get(*at).map(String::as_str) {
                Some(",") => *at += 1,
                Some(">") => {}
                Some(other) => return Err(format!("`{other}` inside the generic arguments")),
                None => return Err(String::from("an unclosed `<`")),
            }
        }
    }
    let borrowed: Vec<&str> = segments.iter().map(String::as_str).collect();
    Ok(rust::Type::generic(Path::new(&borrowed), args))
}

fn is_identifier(token: &str) -> bool {
    token
        .chars()
        .next()
        .is_some_and(|c| c == '_' || c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;

    fn spelled(text: &str) -> String {
        zerodds_lang_render::rust::type_(&parse_type(text).expect("a type path"))
    }

    #[test]
    fn a_type_path_with_generic_arguments_is_written_back_as_given() {
        assert_eq!(
            spelled("crate::time::UtcTimestamp"),
            "crate::time::UtcTimestamp"
        );
        assert_eq!(
            spelled(" chrono::DateTime < chrono::Utc > "),
            "chrono::DateTime<chrono::Utc>"
        );
        assert_eq!(
            spelled("std::borrow::Cow<'static, str>"),
            "std::borrow::Cow<'static, str>"
        );
        assert_eq!(
            spelled("::core::num::NonZeroU32"),
            "::core::num::NonZeroU32"
        );
        assert_eq!(spelled("a::B<c::D<E, F>, G>"), "a::B<c::D<E, F>, G>");
    }

    #[test]
    fn text_that_is_no_type_path_is_refused_by_name() {
        for text in [
            "", "&str", "[u8; 4]", "a::", "a<b", "a b", "a:b", "(u8, u8)",
        ] {
            assert!(parse_type(text).is_err(), "{text:?} is no type path");
        }
        assert!(parse_path("zerodds_wire::DisplayFromStr").is_ok());
        assert!(parse_path("a::B<C>").is_err());
    }
}
