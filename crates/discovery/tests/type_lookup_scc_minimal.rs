// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XTypes 1.3 §7.6.3.3 and §7.6.3.3.4: a getTypes request for the
//! identifier of a strongly connected component is answered in the form it
//! asks for, atomically; a COMPLETE answer names each member's MINIMAL
//! counterpart; and the dependencies of a component are listed with the
//! component named as a whole.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_discovery::type_lookup::TypeLookupServer;
use zerodds_types::builder::TypeObjectBuilder;
use zerodds_types::resolve::{TypeComponent, TypeRegistry};
use zerodds_types::type_identifier::StronglyConnectedComponentId;
use zerodds_types::type_identifier::kinds::{EK_COMPLETE, EK_MINIMAL};
use zerodds_types::type_lookup::{GetTypeDependenciesRequest, GetTypesRequest};
use zerodds_types::{
    CompleteTypeObject, EquivalenceHash, MinimalTypeObject, PrimitiveKind, TypeIdentifier,
    TypeObject,
};

const COMPLETE: EquivalenceHash = EquivalenceHash([0x5C; 14]);
const MINIMAL: EquivalenceHash = EquivalenceHash([0x3D; 14]);

fn complete_member(name: &str, extra: Option<TypeIdentifier>) -> CompleteTypeObject {
    let mut builder = TypeObjectBuilder::struct_type(name).member(
        "a",
        TypeIdentifier::Primitive(PrimitiveKind::Int32),
        |m| m,
    );
    if let Some(identifier) = extra {
        builder = builder.member("e", identifier, |m| m);
    }
    CompleteTypeObject::Struct(builder.build_complete())
}

fn minimal_member(name: &str) -> MinimalTypeObject {
    MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type(name)
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_minimal(),
    )
}

fn scc(kind: u8, hash: EquivalenceHash, index: i32) -> TypeIdentifier {
    TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
        equivalence_kind: kind,
        hash,
        scc_length: 2,
        scc_index: index,
    })
}

fn server(first_extra: Option<TypeIdentifier>) -> TypeLookupServer {
    let mut registry = TypeRegistry::new();
    registry.insert_type_component(&TypeComponent {
        complete_hash: COMPLETE,
        minimal_hash: MINIMAL,
        complete: vec![
            complete_member("::A", first_extra),
            complete_member("::B", None),
        ],
        minimal: vec![minimal_member("::A"), minimal_member("::B")],
    });
    TypeLookupServer::with_registry(registry)
}

/// The MINIMAL identifier of a component gets its MINIMAL members, each
/// under the identifier of its own position.
#[test]
fn a_minimal_component_identifier_gets_the_minimal_members() {
    let reply = server(None).handle_get_types(&GetTypesRequest {
        type_ids: vec![scc(EK_MINIMAL, MINIMAL, 0)],
    });
    let ids: Vec<&TypeIdentifier> = reply.types.iter().map(|p| &p.type_identifier).collect();
    assert_eq!(
        ids,
        [&scc(EK_MINIMAL, MINIMAL, 1), &scc(EK_MINIMAL, MINIMAL, 2)]
    );
    assert!(
        reply
            .types
            .iter()
            .all(|pair| matches!(pair.type_object, TypeObject::Minimal(_)))
    );
}

/// A COMPLETE answer names the MINIMAL counterpart of each member
/// (§7.6.3.3.4.2), so that a requester which resolved the complete
/// identity reaches the minimal component assignability compares.
#[test]
fn a_complete_answer_pairs_each_member_with_its_minimal_counterpart() {
    let reply = server(None).handle_get_types(&GetTypesRequest {
        type_ids: vec![scc(EK_COMPLETE, COMPLETE, 0)],
    });
    assert_eq!(reply.types.len(), 2);
    let pairs: Vec<(&TypeIdentifier, &TypeIdentifier)> = reply
        .complete_to_minimal
        .iter()
        .map(|pair| (&pair.type_identifier1, &pair.type_identifier2))
        .collect();
    assert_eq!(
        pairs,
        [
            (&scc(EK_COMPLETE, COMPLETE, 1), &scc(EK_MINIMAL, MINIMAL, 1)),
            (&scc(EK_COMPLETE, COMPLETE, 2), &scc(EK_MINIMAL, MINIMAL, 2)),
        ]
    );
}

/// What the reply is filed from is what the registry can serve: a
/// requester that files the reply holds the component in both forms.
#[test]
fn a_reply_files_into_a_registry_as_the_component_it_asked_for() {
    let reply = server(None).handle_get_types(&GetTypesRequest {
        type_ids: vec![scc(EK_COMPLETE, COMPLETE, 0), scc(EK_MINIMAL, MINIMAL, 0)],
    });
    let mut requester = TypeRegistry::new();
    assert_eq!(requester.insert_components_of(&reply), 2);
    let complete = StronglyConnectedComponentId {
        equivalence_kind: EK_COMPLETE,
        hash: COMPLETE,
        scc_length: 2,
        scc_index: 1,
    };
    assert!(requester.holds_component(&complete));
    assert_eq!(requester.minimal_component_hash(&COMPLETE), Some(MINIMAL));
    assert!(requester.component_minimal_member(&complete).is_some());
}

/// A component that arrives with a member missing is not filed.
#[test]
fn a_reply_with_a_member_missing_files_nothing() {
    let mut reply = server(None).handle_get_types(&GetTypesRequest {
        type_ids: vec![scc(EK_COMPLETE, COMPLETE, 0)],
    });
    reply.types.pop();
    let mut requester = TypeRegistry::new();
    assert_eq!(requester.insert_components_of(&reply), 0);
}

/// The dependencies of a component are the hash identifiers of the types
/// its members name, in the form of the request; the component's own
/// SCCIdentifier is not one of them (§7.6.3.2.1).
#[test]
fn the_dependencies_of_a_component_are_the_types_its_members_name() {
    let enumeration = TypeObjectBuilder::enum_type("::Colour")
        .bit_bound(32)
        .literal("RED", 0)
        .literal("GREEN", 1)
        .build_complete();
    let enumeration = CompleteTypeObject::Enumerated(enumeration);
    let hash = zerodds_types::compute_complete_hash(&enumeration).expect("hash");
    let mut registry = TypeRegistry::new();
    registry.insert_type_component(&TypeComponent {
        complete_hash: COMPLETE,
        minimal_hash: MINIMAL,
        complete: vec![
            complete_member("::A", Some(TypeIdentifier::EquivalenceHashComplete(hash))),
            complete_member("::B", None),
        ],
        minimal: vec![minimal_member("::A"), minimal_member("::B")],
    });
    registry.insert_complete(hash, enumeration);
    let server = TypeLookupServer::with_registry(registry);

    let reply = server.handle_get_type_dependencies(&GetTypeDependenciesRequest {
        type_ids: vec![scc(EK_COMPLETE, COMPLETE, 0)],
        continuation_point: zerodds_types::type_lookup::ContinuationPoint::default(),
    });
    let ids: Vec<&TypeIdentifier> = reply
        .dependent_typeids
        .iter()
        .map(|dependent| &dependent.type_id)
        .collect();
    assert_eq!(ids, [&TypeIdentifier::EquivalenceHashComplete(hash)]);
    assert!(reply.dependent_typeids[0].typeobject_serialized_size > 0);

    // And the getTypes answer for the component carries the closure
    // beside the members.
    let types = server.handle_get_types(&GetTypesRequest {
        type_ids: vec![scc(EK_COMPLETE, COMPLETE, 0)],
    });
    assert!(
        types
            .types
            .iter()
            .any(|pair| pair.type_identifier == TypeIdentifier::EquivalenceHashComplete(hash)),
        "the enumeration travels with the component"
    );
}
