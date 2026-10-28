// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-XTypes 1.3 §7.6.3.2.1: `dependent_typeid_count` is "the total number
//! of other […] Hash TypeIdentifiers that correspond to data-types the
//! TopicType depends on", or -1 when it is not provided.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_types::from_ast::{complete_type_object, dependency_type_objects};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::{TypeInformation, TypeObject, compute_hash};

const IDL: &str = r#"
module dep {
    enum Color { RED, GREEN };
    typedef long Meters;
    struct Inner { Color c; };
    struct Base { long a; Meters m; };
    struct Derived : Base { float angle; Inner nested; };
    @final struct Plain { long x; };
};
"#;

fn model() -> zerodds_ast::Ast {
    let parsed = zerodds_idl_parse::parse(IDL);
    assert!(!parsed.has_errors());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "dep.idl");
    let mut ast = lowered.ast;
    assert!(ast.resolve_all_by_name().expect("resolve").is_clean());
    ast
}

fn find(ast: &zerodds_ast::Ast, name: &str) -> zerodds_ast_core::Id<zerodds_ast::TypeNode> {
    let segments = name.split("::").map(ToString::to_string).collect();
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("type")
}

#[test]
fn the_count_is_the_number_of_types_the_topic_type_depends_on() {
    let ast = model();
    let id = find(&ast, "dep::Derived");
    let own = complete_type_object(&ast, id).expect("object");
    let mut registry = TypeRegistry::new();
    for object in dependency_type_objects(&ast, id).expect("closure") {
        let hash = compute_hash(&TypeObject::Complete(object.clone())).expect("hash");
        registry.insert_complete(hash, object);
    }
    registry.project_completes_to_minimal();

    let information = TypeInformation::from_registry(&own, &registry).expect("information");
    // Base, Color, Inner, Meters.
    assert_eq!(information.complete.dependent_typeid_count, 4);
    assert_eq!(information.complete.dependent_typeids.len(), 4);
    assert_eq!(information.minimal.dependent_typeid_count, 4);
    assert_eq!(information.minimal.dependent_typeids.len(), 4);
    assert!(
        information
            .complete
            .dependent_typeids
            .iter()
            .all(|d| d.typeobject_serialized_size > 0)
    );
}

#[test]
fn a_dependency_the_registry_lacks_gives_minus_one() {
    let ast = model();
    let id = find(&ast, "dep::Derived");
    let own = complete_type_object(&ast, id).expect("object");
    let registry = TypeRegistry::new();
    let information = TypeInformation::from_registry(&own, &registry);
    // The type itself does not project without its dependencies; either
    // that is refused or the count is -1, never a claimed 0.
    if let Ok(information) = information {
        assert_eq!(information.complete.dependent_typeid_count, -1);
        assert_eq!(information.minimal.dependent_typeid_count, -1);
    }
}

#[test]
fn a_type_without_dependencies_has_count_zero() {
    let ast = model();
    let id = find(&ast, "dep::Plain");
    let own = complete_type_object(&ast, id).expect("object");
    let information =
        TypeInformation::from_registry(&own, &TypeRegistry::new()).expect("information");
    assert_eq!(information.complete.dependent_typeid_count, 0);
    assert_eq!(information.minimal.dependent_typeid_count, 0);
}
