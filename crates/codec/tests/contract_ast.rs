// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Contract of `zerodds-codec` with `zerodds-ast`
//! (internal/release/rc8-test-suite.md, "Crate tier").
//!
//! What this crate uses of `zerodds-ast` (`src/model.rs`): `Ast::types`
//! and `Ast::scoped_name_of(AnyId::Type(..))` to find the described type by
//! its scoped name; `Ast::effective_extensibility`; `Ast::members_of`,
//! `Ast::member` (`name`, `node_id`, `type_.resolved()`,
//! `facets.is_optional()`); `Ast::type_` with the `TypeKind` variants
//! `Alias`, `Primitive`, `String`, `Sequence`, `Array`, `Enumeration` and
//! `Structure`; `Bound::expr` and `Ast::eval_integral` for bounds and
//! array lengths; `PrimitiveKind`.
//!
//! Oracle: IDL 4.2 §7.4.1.4 (the declarations), §7.5 (scoped names),
//! DDS-XTypes 1.3 §7.2.2.4.4.4 (an unannotated struct is `@appendable`).

#![allow(clippy::unwrap_used, clippy::panic, missing_docs)]

use zerodds_ast::facet::Extensibility;
use zerodds_ast::{AnyId, Ast, CharWidth, PrimitiveKind, TypeKind};

const IDL: &str = "\
module a { module b {
    const long N = 3;
    typedef sequence<short, N> Shorts;
    struct Shape { @optional long x; string<N> s; Shorts v; double grid[2][N]; };
}; };";

fn compose() -> Ast {
    let composed = zerodds_idl_compose::compose_source(
        "contract.idl",
        IDL,
        &zerodds_idl_compose::PathLoader::bare(),
    );
    assert!(!composed.failed());
    composed.ast
}

#[test]
fn idl_4_2_7_5_a_type_is_found_by_its_scoped_name() {
    let ast = compose();
    let found: Vec<String> = ast
        .types()
        .filter(|(_, node)| node.name == "Shape")
        .filter_map(|(id, node)| ast.scoped_name_of(AnyId::Type(id, node.node_id)))
        .map(|name| name.to_string())
        .collect();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].trim_start_matches("::"), "a::b::Shape");
}

#[test]
fn idl_4_2_7_4_1_4_members_their_types_bounds_and_optionality() {
    let ast = compose();
    let (s, _) = ast.types().find(|(_, node)| node.name == "Shape").unwrap();
    assert_eq!(ast.effective_extensibility(s), Extensibility::Appendable);
    let members: Vec<_> = ast
        .members_of(s)
        .iter()
        .map(|id| ast.member(*id).unwrap())
        .collect();
    let names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["x", "s", "v", "grid"]);
    assert!(members[0].facets.is_optional());
    assert!(!members[1].facets.is_optional());
    let kind = |index: usize| {
        &ast.type_(members[index].type_.resolved().unwrap())
            .unwrap()
            .kind
    };
    assert!(matches!(kind(0), TypeKind::Primitive(PrimitiveKind::Int32)));
    match kind(1) {
        TypeKind::String {
            width: CharWidth::Char,
            bound,
        } => {
            assert_eq!(ast.eval_integral(bound.expr().unwrap()).unwrap(), 3);
        }
        other => panic!("a string, got {other:?}"),
    }
    match kind(2) {
        TypeKind::Alias { aliased } => {
            match &ast.type_(aliased.resolved().unwrap()).unwrap().kind {
                TypeKind::Sequence { bound, .. } => {
                    assert_eq!(ast.eval_integral(bound.expr().unwrap()).unwrap(), 3);
                }
                other => panic!("a sequence, got {other:?}"),
            }
        }
        other => panic!("a typedef, got {other:?}"),
    }
    match kind(3) {
        TypeKind::Array { dims, .. } => {
            let lengths: Vec<i128> = dims
                .iter()
                .map(|d| ast.eval_integral(*d).unwrap())
                .collect();
            assert_eq!(lengths, [2, 3]);
        }
        other => panic!("an array, got {other:?}"),
    }
}
