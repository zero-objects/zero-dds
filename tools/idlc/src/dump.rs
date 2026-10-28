// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Showing what is in a unit, once it is a model.
//!
//! Two views, and both are read by people rather than parsed by
//! programs: the tree, for "what did the front end make of this", and
//! the dependency list, for "what does this file need in order to
//! compile". Neither invents a fact the model does not hold — an
//! unresolved reference prints as unresolved, because a dependency list
//! that quietly omits the name that failed is the list you consult right
//! before you find out it was missing.

use core::fmt::Write as _;

use zerodds_ast::{Ast, TypeKind};

/// The model as an indented tree, module by module.
#[must_use]
pub fn tree(ast: &Ast) -> String {
    let mut out = String::new();
    for (id, node) in ast.types() {
        if node.name.is_empty() {
            continue;
        }
        let path = scoped_name(ast, id, node);
        let _ = writeln!(out, "{path}  [{}]", kind_name(&node.kind));
        for member in ast.members_of(id) {
            let Ok(member_node) = ast.member(*member) else {
                continue;
            };
            let mut marks = String::new();
            if member_node.facets.is_key() {
                marks.push_str(" @key");
            }
            if member_node.facets.is_optional() {
                marks.push_str(" @optional");
            }
            if let Some(member_id) = member_node.facets.effective_member_id() {
                let _ = write!(marks, " id={member_id}");
            }
            let _ = writeln!(
                out,
                "    {}: {}{marks}",
                member_node.name,
                type_of(ast, member_node.type_.resolved())
            );
        }
    }
    out
}

/// Every type each declaration names, so a caller can see what a unit
/// pulls in.
#[must_use]
pub fn dependencies(ast: &Ast) -> String {
    let mut out = String::new();
    for (id, node) in ast.types() {
        if node.name.is_empty() {
            continue;
        }
        let mut names: Vec<String> = Vec::new();
        for member in ast.all_members_of(id) {
            let Ok(member_node) = ast.member(member) else {
                continue;
            };
            let name = type_of(ast, member_node.type_.resolved());
            if !names.contains(&name) {
                names.push(name);
            }
        }
        if names.is_empty() {
            continue;
        }
        names.sort();
        let _ = writeln!(
            out,
            "{} -> {}",
            scoped_name(ast, id, node),
            names.join(", ")
        );
    }
    out
}

/// A type's scoped name, `a::B::C`, falling back to the simple name for
/// a node the index does not carry.
fn scoped_name(
    ast: &Ast,
    id: zerodds_ast_core::Id<zerodds_ast::TypeNode>,
    node: &zerodds_ast::TypeNode,
) -> String {
    ast.scoped_name_of(zerodds_ast::AnyId::Type(id, node.node_id))
        .map_or_else(|| node.name.clone(), |name| name.segments().join("::"))
}

fn kind_name(kind: &TypeKind) -> &'static str {
    match kind {
        TypeKind::Structure { .. } => "struct",
        TypeKind::Union { .. } => "union",
        TypeKind::Enumeration { .. } => "enum",
        TypeKind::Bitmask { .. } => "bitmask",
        TypeKind::Bitset { .. } => "bitset",
        TypeKind::Alias { .. } => "typedef",
        TypeKind::Valuetype { .. } => "valuetype",
        TypeKind::Sequence { .. } => "sequence",
        TypeKind::Array { .. } => "array",
        TypeKind::Map { .. } => "map",
        TypeKind::String { .. } => "string",
        TypeKind::Primitive(_) => "primitive",
        _ => "other",
    }
}

/// A member's type, by name where it has one.
///
/// An unresolved reference says so rather than being left out: this is
/// the list somebody reads to find out why a file will not compile.
fn type_of(ast: &Ast, target: Option<zerodds_ast_core::Id<zerodds_ast::TypeNode>>) -> String {
    let Some(target) = target else {
        return String::from("<unresolved>");
    };
    ast.type_(target).map_or_else(
        |_| String::from("<unresolved>"),
        |node| {
            if node.name.is_empty() {
                String::from(kind_name(&node.kind))
            } else {
                node.name.clone()
            }
        },
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;

    fn model(source: &str) -> Ast {
        let parsed = zerodds_idl_parse::parse(source);
        zerodds_idl_lower::lower(&parsed.syntax(), "test.idl").ast
    }

    #[test]
    fn the_tree_names_every_type_and_its_members() {
        let out = tree(&model(
            "module m { struct Inner { long a; }; struct S { @key long id; Inner inner; }; };",
        ));
        assert!(out.contains("m::S  [struct]"), "{out}");
        assert!(out.contains("id: "), "{out}");
        assert!(out.contains("@key"), "{out}");
        assert!(out.contains("inner: Inner"), "{out}");
    }

    #[test]
    fn dependencies_list_each_named_type_once() {
        let out = dependencies(&model(
            "module m { struct Inner { long a; }; struct S { Inner one; Inner two; }; };",
        ));
        let line = out
            .lines()
            .find(|l| l.starts_with("m::S"))
            .expect("S has a line");
        assert_eq!(line.matches("Inner").count(), 1, "{line}");
    }

    /// A name that does not resolve appears as unresolved rather than
    /// being dropped — this is the list read to find out what is missing.
    #[test]
    fn an_unresolved_member_type_is_named_as_such() {
        let out = tree(&model("module m { struct S { Missing gap; }; };"));
        assert!(out.contains("<unresolved>"), "{out}");
    }
}
