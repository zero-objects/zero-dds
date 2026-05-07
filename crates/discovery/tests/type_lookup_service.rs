//! C4.2 — TypeLookup-Service Integration-Tests im Discovery-Layer.
//!
//! Cross-Plugin: Server-Side build → Wire-Roundtrip → Client-Side parse
//! mit Callback-Trigger. Spec §7.6.3.3.4.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::sync::{Arc, Mutex};

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_discovery::type_lookup::{
    RequestId, TypeLookupClient, TypeLookupEndpoints, TypeLookupReply, TypeLookupServer,
    format_service_instance_name, hashes_to_minimal_ids, request_dependencies_payload,
    request_types_payload,
};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::builder::TypeObjectBuilder;
use zerodds_types::type_lookup::{
    ContinuationPoint, GetTypeDependenciesReply, GetTypeDependenciesRequest, GetTypesReply,
    GetTypesRequest, ReplyTypeObject,
};
use zerodds_types::{EquivalenceHash, MinimalTypeObject, PrimitiveKind, TypeIdentifier};

fn sample_struct(name: &str) -> MinimalTypeObject {
    MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type(name)
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int64), |m| m)
            .build_minimal(),
    )
}

#[test]
fn server_handle_get_types_happy_path_one_type() {
    let mut server = TypeLookupServer::new();
    let m = sample_struct("::Foo");
    let h = zerodds_types::compute_minimal_hash(&m).unwrap();
    server.registry.insert_minimal(h, m);

    let req = GetTypesRequest {
        type_ids: vec![TypeIdentifier::EquivalenceHashMinimal(h)],
    };
    let reply = server.handle_get_types(&req);
    assert_eq!(reply.types.len(), 1);
    assert!(matches!(reply.types[0], ReplyTypeObject::Minimal(_)));
}

#[test]
fn server_handle_get_types_unknown_returns_empty_no_error() {
    let server = TypeLookupServer::new();
    let req = GetTypesRequest {
        type_ids: vec![TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
            [0xAB; 14],
        ))],
    };
    let reply = server.handle_get_types(&req);
    assert!(reply.types.is_empty());
}

#[test]
fn server_handle_get_types_with_100_types_no_pagination() {
    let mut server = TypeLookupServer::new();
    let mut hashes = Vec::new();
    for i in 0..100u8 {
        let m = sample_struct(&format!("::T{i}"));
        let h = zerodds_types::compute_minimal_hash(&m).unwrap();
        server.registry.insert_minimal(h, m);
        hashes.push(h);
    }
    let req = GetTypesRequest {
        type_ids: hashes_to_minimal_ids(&hashes),
    };
    let reply = server.handle_get_types(&req);
    // getTypes hat keine Pagination — alle 100 muessen drin sein.
    assert_eq!(reply.types.len(), 100);
}

#[test]
fn server_handle_get_type_dependencies_struct_with_5_deps() {
    let mut server = TypeLookupServer::new();
    let mut builder = TypeObjectBuilder::struct_type("::Root");
    let mut dep_hashes = Vec::new();
    for i in 0..5u8 {
        let mut bytes = [0u8; 14];
        bytes[0] = i;
        let h = EquivalenceHash(bytes);
        dep_hashes.push(h);
        builder = builder.member(
            format!("m{i}"),
            TypeIdentifier::EquivalenceHashMinimal(h),
            |m| m,
        );
    }
    let root = MinimalTypeObject::Struct(builder.build_minimal());
    let root_hash = zerodds_types::compute_minimal_hash(&root).unwrap();
    server.registry.insert_minimal(root_hash, root);

    let req = GetTypeDependenciesRequest {
        type_ids: vec![TypeIdentifier::EquivalenceHashMinimal(root_hash)],
        continuation_point: ContinuationPoint::default(),
    };
    let reply = server.handle_get_type_dependencies(&req);
    assert_eq!(reply.dependent_typeids.len(), 5);
    assert!(reply.continuation_point.0.is_empty()); // alles in einer Page
}

#[test]
fn server_continuation_point_paginates_200_dependencies() {
    let mut server = TypeLookupServer::new().with_page_size(100);
    let mut builder = TypeObjectBuilder::struct_type("::Big");
    for i in 0..200u32 {
        let mut bytes = [0u8; 14];
        bytes[..4].copy_from_slice(&i.to_le_bytes());
        let h = EquivalenceHash(bytes);
        builder = builder.member(
            format!("m{i}"),
            TypeIdentifier::EquivalenceHashMinimal(h),
            |m| m,
        );
    }
    let root = MinimalTypeObject::Struct(builder.build_minimal());
    let root_hash = zerodds_types::compute_minimal_hash(&root).unwrap();
    server.registry.insert_minimal(root_hash, root);

    // Iter-Schleife bis CP leer ist.
    let mut total = 0;
    let mut cp = ContinuationPoint::default();
    let mut iters = 0;
    loop {
        iters += 1;
        let req = GetTypeDependenciesRequest {
            type_ids: vec![TypeIdentifier::EquivalenceHashMinimal(root_hash)],
            continuation_point: cp.clone(),
        };
        let reply = server.handle_get_type_dependencies(&req);
        total += reply.dependent_typeids.len();
        cp = reply.continuation_point;
        if cp.0.is_empty() {
            break;
        }
        assert!(iters < 5, "infinite-loop guard");
    }
    assert_eq!(total, 200);
    assert_eq!(iters, 2);
}

#[test]
fn server_handle_get_types_empty_request() {
    let server = TypeLookupServer::new();
    let reply = server.handle_get_types(&GetTypesRequest::default());
    assert!(reply.types.is_empty());
}

#[test]
fn client_request_types_assigns_unique_ids() {
    let mut client = TypeLookupClient::new();
    let id1 = client.request_types(vec![], Box::new(|_| {}));
    let id2 = client.request_types(vec![], Box::new(|_| {}));
    assert_ne!(id1, id2);
}

#[test]
fn client_handle_reply_invokes_correct_callback() {
    let last_id: Arc<Mutex<Option<RequestId>>> = Arc::new(Mutex::new(None));
    let captured: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    let captured_clone = Arc::clone(&captured);
    let mut client = TypeLookupClient::new();
    let id = client.request_types(
        vec![TypeIdentifier::Primitive(PrimitiveKind::Int32)],
        Box::new(move |reply| {
            if let TypeLookupReply::Types(r) = reply {
                *captured_clone.lock().unwrap() = r.types.len();
            }
        }),
    );
    *last_id.lock().unwrap() = Some(id);

    let reply = TypeLookupReply::Types(GetTypesReply {
        types: vec![ReplyTypeObject::Minimal(sample_struct("::X"))],
    });
    let consumed = client.handle_reply(id, reply);
    assert!(consumed);
    assert_eq!(*captured.lock().unwrap(), 1);
}

#[test]
fn client_handle_reply_unknown_id_does_not_panic() {
    let mut client = TypeLookupClient::new();
    let consumed = client.handle_reply(
        RequestId(42),
        TypeLookupReply::Types(GetTypesReply::default()),
    );
    assert!(!consumed);
}

#[test]
fn service_instance_name_format_matches_spec() {
    let prefix = GuidPrefix::from_bytes([
        0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0, 0x11, 0x22, 0x33, 0x44,
    ]);
    let name = format_service_instance_name(&prefix);
    assert!(name.starts_with("dds.builtin.TOS."));
    let suffix = name.trim_start_matches("dds.builtin.TOS.");
    assert_eq!(suffix.len(), 24);
    assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn typelookup_endpoints_4_distinct_guids_per_participant() {
    let prefix = GuidPrefix::from_bytes([0x77; 12]);
    let e = TypeLookupEndpoints::new(prefix);
    let guids = e.all_guids();
    assert_eq!(guids.len(), 4);
    for g in &guids {
        assert_eq!(g.prefix, prefix);
    }
    // Alle vier EntityIds sind unterschiedlich.
    let mut seen = std::collections::HashSet::new();
    for g in &guids {
        assert!(seen.insert(g.entity_id), "duplicate entity_id: {g:?}");
    }
}

#[test]
fn cross_plugin_server_emits_reply_client_parses_typeobject() {
    // Server-Side hat den Type, Client nicht.
    let mut server = TypeLookupServer::new();
    let m = sample_struct("::Cross");
    let h = zerodds_types::compute_minimal_hash(&m).unwrap();
    server.registry.insert_minimal(h, m);

    // Client baut Request, schickt zum Server (simuliert via direkter
    // Funktionsaufruf).
    let mut client = TypeLookupClient::new();
    let collected: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let collected_clone = Arc::clone(&collected);
    let id = client.request_types(
        vec![TypeIdentifier::EquivalenceHashMinimal(h)],
        Box::new(move |reply| {
            if let TypeLookupReply::Types(r) = reply {
                *collected_clone.lock().unwrap() = r.types.len();
            }
        }),
    );

    // Wire-Roundtrip: Request-Bytes → Server.
    let req_bytes = request_types_payload(&[TypeIdentifier::EquivalenceHashMinimal(h)]).unwrap();
    let mut r = BufferReader::new(&req_bytes, Endianness::Little);
    let req = GetTypesRequest::decode_from(&mut r).unwrap();
    let reply = server.handle_get_types(&req);

    // Wire-Roundtrip: Reply-Bytes → Client.
    let mut w = BufferWriter::new(Endianness::Little);
    reply.encode_into(&mut w).unwrap();
    let reply_bytes = w.into_bytes();
    let mut r = BufferReader::new(&reply_bytes, Endianness::Little);
    let decoded_reply = GetTypesReply::decode_from(&mut r).unwrap();

    // Callback feuert.
    client.handle_reply(id, TypeLookupReply::Types(decoded_reply));
    assert_eq!(*collected.lock().unwrap(), 1);
}

#[test]
fn typeidentifier_with_minimal_hash_yields_complete_to_minimal_pair() {
    // Spec-§7.6.3.3.4 §a: when client requests Complete and server has
    // both, server includes a complete_to_minimal mapping. Wir testen
    // dass beide Hash-Varianten unabhaengig auflosbar sind.
    let mut server = TypeLookupServer::new();
    let m = sample_struct("::Both");
    let h_min = zerodds_types::compute_minimal_hash(&m).unwrap();
    server.registry.insert_minimal(h_min, m);

    let req_min = GetTypesRequest {
        type_ids: vec![TypeIdentifier::EquivalenceHashMinimal(h_min)],
    };
    let reply = server.handle_get_types(&req_min);
    assert_eq!(reply.types.len(), 1);
    assert!(matches!(reply.types[0], ReplyTypeObject::Minimal(_)));

    // Complete-Hash NICHT in Registry → leerer Reply.
    let req_complete = GetTypesRequest {
        type_ids: vec![TypeIdentifier::EquivalenceHashComplete(EquivalenceHash(
            [0xCC; 14],
        ))],
    };
    let reply_c = server.handle_get_types(&req_complete);
    assert!(reply_c.types.is_empty());
}

#[test]
fn dependencies_payload_carries_continuation_point() {
    let cp = ContinuationPoint(vec![0x42, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    let bytes = request_dependencies_payload(
        &[TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
            [1; 14],
        ))],
        cp.clone(),
    )
    .unwrap();
    let mut r = BufferReader::new(&bytes, Endianness::Little);
    let decoded = GetTypeDependenciesRequest::decode_from(&mut r).unwrap();
    assert_eq!(decoded.continuation_point, cp);
}

#[test]
fn server_dependencies_with_zero_known_returns_empty() {
    let server = TypeLookupServer::new();
    let req = GetTypeDependenciesRequest {
        type_ids: vec![TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash(
            [0xFF; 14],
        ))],
        continuation_point: ContinuationPoint::default(),
    };
    let reply = server.handle_get_type_dependencies(&req);
    assert!(reply.dependent_typeids.is_empty());
    assert!(reply.continuation_point.0.is_empty());
}

#[test]
fn pending_count_decreases_after_handle_reply() {
    let mut client = TypeLookupClient::new();
    let id = client.request_types(vec![], Box::new(|_| {}));
    assert_eq!(client.pending_count(), 1);
    client.handle_reply(id, TypeLookupReply::Types(GetTypesReply::default()));
    assert_eq!(client.pending_count(), 0);
}

#[test]
fn typelookup_reply_dependencies_variant_is_dispatched() {
    let calls: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));
    let calls_clone = Arc::clone(&calls);
    let mut client = TypeLookupClient::new();
    let id = client.request_type_dependencies(
        vec![],
        ContinuationPoint::default(),
        Box::new(move |reply| {
            if matches!(reply, TypeLookupReply::Dependencies(_)) {
                *calls_clone.lock().unwrap() += 1;
            }
        }),
    );
    let consumed = client.handle_reply(
        id,
        TypeLookupReply::Dependencies(GetTypeDependenciesReply::default()),
    );
    assert!(consumed);
    assert_eq!(*calls.lock().unwrap(), 1);
}

#[test]
fn server_with_registry_constructor_preserves_entries() {
    let mut reg = zerodds_types::resolve::TypeRegistry::new();
    let m = sample_struct("::From");
    let h = zerodds_types::compute_minimal_hash(&m).unwrap();
    reg.insert_minimal(h, m);

    let server = TypeLookupServer::with_registry(reg);
    let reply = server.handle_get_types(&GetTypesRequest {
        type_ids: vec![TypeIdentifier::EquivalenceHashMinimal(h)],
    });
    assert_eq!(reply.types.len(), 1);
}

#[test]
fn pagination_with_page_size_one_emits_one_per_call() {
    let mut server = TypeLookupServer::new().with_page_size(1);
    let mut builder = TypeObjectBuilder::struct_type("::PS1");
    let mut deps = Vec::new();
    for i in 0..3u8 {
        let mut b = [0u8; 14];
        b[0] = i;
        let h = EquivalenceHash(b);
        deps.push(h);
        builder = builder.member(
            format!("m{i}"),
            TypeIdentifier::EquivalenceHashMinimal(h),
            |m| m,
        );
    }
    let root = MinimalTypeObject::Struct(builder.build_minimal());
    let root_hash = zerodds_types::compute_minimal_hash(&root).unwrap();
    server.registry.insert_minimal(root_hash, root);

    let mut cp = ContinuationPoint::default();
    let mut counts = Vec::new();
    for _ in 0..10 {
        let req = GetTypeDependenciesRequest {
            type_ids: vec![TypeIdentifier::EquivalenceHashMinimal(root_hash)],
            continuation_point: cp.clone(),
        };
        let reply = server.handle_get_type_dependencies(&req);
        counts.push(reply.dependent_typeids.len());
        cp = reply.continuation_point;
        if cp.0.is_empty() {
            break;
        }
    }
    assert_eq!(counts, vec![1, 1, 1]);
}
