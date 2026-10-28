// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-XTypes 1.3 §7.6.3.3: a getTypes request for an SCCIdentifier is
//! answered with all TypeObjects of the strongly connected component or
//! with none of them.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used, missing_docs)]

use zerodds_discovery::type_lookup::TypeLookupServer;
use zerodds_types::builder::TypeObjectBuilder;
use zerodds_types::resolve::TypeRegistry;
use zerodds_types::type_identifier::StronglyConnectedComponentId;
use zerodds_types::type_identifier::kinds::EK_COMPLETE;
use zerodds_types::type_lookup::GetTypesRequest;
use zerodds_types::{
    CompleteTypeObject, EquivalenceHash, PrimitiveKind, TypeIdentifier, TypeObject,
};

fn member(name: &str) -> CompleteTypeObject {
    CompleteTypeObject::Struct(
        TypeObjectBuilder::struct_type(name)
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_complete(),
    )
}

fn scc(index: i32, length: i32) -> TypeIdentifier {
    TypeIdentifier::StronglyConnectedComponent(StronglyConnectedComponentId {
        equivalence_kind: EK_COMPLETE,
        hash: EquivalenceHash([0x5C; 14]),
        scc_length: length,
        scc_index: index,
    })
}

fn server() -> TypeLookupServer {
    let mut registry = TypeRegistry::new();
    registry.insert_component(
        EquivalenceHash([0x5C; 14]),
        vec![member("::A"), member("::B")],
    );
    TypeLookupServer::with_registry(registry)
}

#[test]
fn an_scc_identifier_gets_every_member_of_the_component() {
    let reply = server().handle_get_types(&GetTypesRequest {
        type_ids: vec![scc(0, 2)],
    });
    let ids: Vec<&TypeIdentifier> = reply.types.iter().map(|p| &p.type_identifier).collect();
    assert_eq!(ids, [&scc(1, 2), &scc(2, 2)]);
    assert!(
        reply
            .types
            .iter()
            .all(|p| matches!(p.type_object, TypeObject::Complete(_)))
    );
}

#[test]
fn an_unknown_component_or_a_wrong_length_gets_none() {
    let server = server();
    let wrong_length = server.handle_get_types(&GetTypesRequest {
        type_ids: vec![scc(0, 3)],
    });
    assert!(wrong_length.types.is_empty());

    let mut other = scc(0, 2);
    if let TypeIdentifier::StronglyConnectedComponent(id) = &mut other {
        id.hash = EquivalenceHash([0x11; 14]);
    }
    let unknown = server.handle_get_types(&GetTypesRequest {
        type_ids: vec![other],
    });
    assert!(unknown.types.is_empty());
}
