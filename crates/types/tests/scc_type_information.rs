// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! TypeInformation and assignability for a topic type that belongs to a
//! strongly connected component. XTypes 1.3 §7.6.3.2.1, §7.3.4.9.3.
//!
//! §7.6.3.2.1: `minimal.typeid_with_size` and `complete.typeid_with_size`
//! "shall contain the [...] Hash TypeIdentifier of the TopicType"; the
//! dependent identifiers "shall not contain individual type identifiers
//! for types belonging to Strongly Connected Component [...], instead it
//! shall include the identifier of the whole Strongly-Connected Component
//! (SCCIdentifier)". A recursive topic type used to announce no
//! TypeInformation at all, and a pair of recursive types the registry
//! held could not be compared: an identifier that is a component had no
//! arm in the assignability walk.
//!
//! What these tests do not establish: that another implementation
//! computes the same component hash. No vendor in the interop matrix
//! writes an SCC identifier (`strongly_connected_components.rs`).

#![allow(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_used,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::string::ToString;

use zerodds_ast::{Ast, TypeNode};
use zerodds_ast_core::Id;
use zerodds_types::TypeInformation;
use zerodds_types::assignability::{AssignabilityConfig, Assignable, is_assignable};
use zerodds_types::from_ast::{Form, register_all, type_identifier};
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_identifier::{StronglyConnectedComponentId, TypeIdentifier, kinds};

fn model(idl: &str) -> Ast {
    let parsed = zerodds_idl_parse::parse(idl);
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let lowered = zerodds_idl_lower::lower(&parsed.syntax(), "scc.idl");
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    let mut ast = lowered.ast;
    let report = ast.resolve_all_by_name().expect("resolve");
    assert!(report.is_clean(), "{:?}", report.broken);
    ast
}

fn type_of(ast: &Ast, name: &str) -> Id<TypeNode> {
    let segments: Vec<String> = name.split("::").map(ToString::to_string).collect();
    ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(segments))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("{name} is not in the model"))
}

fn component(ast: &Ast, name: &str, form: Form) -> StronglyConnectedComponentId {
    match type_identifier(ast, type_of(ast, name), form).expect("an identifier") {
        TypeIdentifier::StronglyConnectedComponent(scc) => scc,
        other => panic!("{name} is not a component member: {other:?}"),
    }
}

fn registry_of(models: &[&Ast]) -> TypeRegistry {
    let mut registry = TypeRegistry::new();
    for ast in models {
        register_all(ast, &mut registry).expect("the registry fills");
    }
    registry
}

const MUTUAL: &str = "
module m {
  struct B;
  @appendable struct A { long value; sequence<B> bs; };
  @appendable struct B { long weight; sequence<A> items; };
};
";

const NODE_V1: &str = "
module m {
  @appendable struct Node { long value; sequence<Node> children; };
};
";

const NODE_V2: &str = "
module m {
  @appendable struct Node { long value; sequence<Node> children; long extra; };
};
";

const NODE_V3: &str = "
module m {
  @appendable struct Node { string value; sequence<Node> children; };
};
";

/// A type of a component announces itself by its own component identifier
/// on both sides, the identifier of the whole component as the first
/// dependency, and a count that is the length of the list.
#[test]
fn a_component_member_announces_both_forms() {
    let ast = model(MUTUAL);
    let registry = registry_of(&[&ast]);
    let complete = component(&ast, "m::A", Form::Complete);
    let minimal = component(&ast, "m::A", Form::Minimal);
    assert_eq!(complete.scc_index, 1);

    let information = TypeInformation::from_component(&complete, &registry).expect("it builds");

    assert_eq!(
        information.complete.typeid_with_size.type_id,
        TypeIdentifier::StronglyConnectedComponent(complete)
    );
    assert_eq!(
        information.minimal.typeid_with_size.type_id,
        TypeIdentifier::StronglyConnectedComponent(minimal)
    );
    assert!(
        information
            .complete
            .typeid_with_size
            .typeobject_serialized_size
            > 0
    );
    assert!(
        information
            .minimal
            .typeid_with_size
            .typeobject_serialized_size
            > 0
    );

    for (side, kind, hash) in [
        (&information.complete, kinds::EK_COMPLETE, complete.hash),
        (&information.minimal, kinds::EK_MINIMAL, minimal.hash),
    ] {
        let first = &side.dependent_typeids[0];
        assert_eq!(
            first.type_id,
            TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
                equivalence_kind: kind,
                hash,
                scc_length: 2,
                scc_index: 0,
            }),
            "the SCCIdentifier of the component comes first"
        );
        assert_eq!(
            usize::try_from(side.dependent_typeid_count).expect("a count"),
            side.dependent_typeids.len()
        );
        assert!(
            side.dependent_typeids.iter().all(|dependent| !matches!(
                &dependent.type_id,
                TypeIdentifier::StronglyConnectedComponent(scc) if scc.scc_index != 0
            )),
            "no individual identifier of a type in a component"
        );
    }

    let bytes = information.to_bytes_le().expect("it encodes");
    assert_eq!(
        TypeInformation::from_bytes_le(&bytes).expect("it decodes"),
        information
    );
}

/// The other member of the same component is announced by the same
/// SCCIdentifier and its own position.
#[test]
fn the_other_member_differs_in_its_position_only() {
    let ast = model(MUTUAL);
    let registry = registry_of(&[&ast]);
    let a = component(&ast, "m::A", Form::Complete);
    let b = component(&ast, "m::B", Form::Complete);
    assert_eq!((a.scc_index, b.scc_index), (1, 2));
    let for_a = TypeInformation::from_component(&a, &registry).expect("A");
    let for_b = TypeInformation::from_component(&b, &registry).expect("B");
    assert_ne!(
        for_a.complete.typeid_with_size.type_id,
        for_b.complete.typeid_with_size.type_id
    );
    assert_eq!(
        for_a.complete.dependent_typeids[0],
        for_b.complete.dependent_typeids[0]
    );
}

/// A type reached from a component by hash is a dependency by hash, in
/// both forms; a second component it reaches is a dependency by its
/// SCCIdentifier.
#[test]
fn what_the_component_reaches_is_listed_once_each() {
    let ast = model(
        "
module m {
  enum Colour { RED, GREEN };
  struct Inner;
  @appendable struct Outer { Colour c; sequence<Inner> xs; sequence<Outer> ys; };
  @appendable struct Inner { long v; sequence<Inner> more; };
};
",
    );
    let registry = registry_of(&[&ast]);
    let outer = component(&ast, "m::Outer", Form::Complete);
    let information = TypeInformation::from_component(&outer, &registry).expect("it builds");
    let complete = &information.complete;
    assert_eq!(
        complete.dependent_typeid_count,
        i32::try_from(complete.dependent_typeids.len()).unwrap()
    );
    assert!(complete.dependent_typeid_count >= 2, "{complete:?}");
    assert!(
        complete
            .dependent_typeids
            .iter()
            .any(|d| matches!(d.type_id, TypeIdentifier::EquivalenceHashComplete(_))),
        "the enumeration is a hash dependency: {complete:?}"
    );
    assert!(
        complete.dependent_typeids.iter().any(|d| matches!(
            &d.type_id,
            TypeIdentifier::StronglyConnectedComponent(s)
                if s.scc_index == 0 && s.hash != outer.hash
        )),
        "the second component is a dependency by its SCCIdentifier: {complete:?}"
    );
    let minimal = &information.minimal;
    assert_eq!(
        minimal.dependent_typeid_count,
        i32::try_from(minimal.dependent_typeids.len()).unwrap()
    );
}

/// A component the registry does not hold is an error, not a half-filled
/// announcement.
#[test]
fn an_unknown_component_is_an_error() {
    let ast = model(MUTUAL);
    let scc = component(&ast, "m::A", Form::Complete);
    assert!(TypeInformation::from_component(&scc, &TypeRegistry::new()).is_err());
}

/// Two evolutions of one recursive type, each a component of its own, are
/// compared member by member through the components: the writer's extra
/// trailing member is the APPENDABLE case, and the members that point
/// back at the type end the walk.
#[test]
fn an_evolved_recursive_type_is_assignable() {
    let v1 = model(NODE_V1);
    let v2 = model(NODE_V2);
    let registry = registry_of(&[&v1, &v2]);
    let reader =
        TypeIdentifier::StronglyConnectedComponent(component(&v1, "m::Node", Form::Minimal));
    let writer =
        TypeIdentifier::StronglyConnectedComponent(component(&v2, "m::Node", Form::Minimal));
    assert_ne!(reader, writer, "two components");
    let cfg = AssignabilityConfig::default();
    assert!(
        is_assignable(&writer, &reader, &registry, &cfg).is_yes(),
        "{:?}",
        is_assignable(&writer, &reader, &registry, &cfg)
    );
}

/// A member of another type is not assignable, and the verdict does not
/// depend on the cycle: it is found at the first level.
#[test]
fn a_recursive_type_with_another_member_type_is_not_assignable() {
    let v1 = model(NODE_V1);
    let v3 = model(NODE_V3);
    let registry = registry_of(&[&v1, &v3]);
    let reader =
        TypeIdentifier::StronglyConnectedComponent(component(&v1, "m::Node", Form::Minimal));
    let writer =
        TypeIdentifier::StronglyConnectedComponent(component(&v3, "m::Node", Form::Minimal));
    let cfg = AssignabilityConfig::default();
    assert!(
        matches!(
            is_assignable(&writer, &reader, &registry, &cfg),
            Assignable::No(_)
        ),
        "{:?}",
        is_assignable(&writer, &reader, &registry, &cfg)
    );
}

/// The COMPLETE identifiers of the same components give the same
/// verdicts: the registry maps a complete component onto its minimal one.
#[test]
fn the_complete_identifiers_give_the_same_verdict() {
    let v1 = model(NODE_V1);
    let v2 = model(NODE_V2);
    let registry = registry_of(&[&v1, &v2]);
    let reader =
        TypeIdentifier::StronglyConnectedComponent(component(&v1, "m::Node", Form::Complete));
    let writer =
        TypeIdentifier::StronglyConnectedComponent(component(&v2, "m::Node", Form::Complete));
    assert!(is_assignable(&writer, &reader, &registry, &AssignabilityConfig::default()).is_yes());
}

/// The identity short-circuit still answers `Yes` for one component with
/// itself, and the walk of a component against itself under a different
/// identifier (the same minimal component, asked for by its two forms)
/// ends in `Yes` as well.
#[test]
fn a_component_is_assignable_to_itself_in_either_form() {
    let ast = model(NODE_V1);
    let registry = registry_of(&[&ast]);
    let complete =
        TypeIdentifier::StronglyConnectedComponent(component(&ast, "m::Node", Form::Complete));
    let minimal =
        TypeIdentifier::StronglyConnectedComponent(component(&ast, "m::Node", Form::Minimal));
    let cfg = AssignabilityConfig::default();
    assert!(is_assignable(&complete, &minimal, &registry, &cfg).is_yes());
    assert!(is_assignable(&minimal, &complete, &registry, &cfg).is_yes());
    assert!(is_assignable(&minimal, &minimal, &registry, &cfg).is_yes());
}
