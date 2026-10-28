// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! A recursive topic type between two runtimes (DDS-XTypes 1.3 §7.3.4.9,
//! §7.6.3.2.1, §7.6.3.3).
//!
//! `Node` contains a `sequence<Node>`; it lies on a cycle and is identified by
//! the `SCCIdentifier` of its component, not by a hash. Runtime A announces a
//! writer of that type with `PID_TYPE_INFORMATION` naming the component; runtime
//! B holds a reader of the same type. What is measured over the wire:
//! the announcement reaches B with the component in both forms, the reader and
//! the writer match on it, and B fetches the component whole with `getTypes`
//! for either identifier (§7.6.3.3.4.2: "the TypeObjects within the Strongly
//! Connected Components" atomically).
//!
//! Linux-only: macOS multicast loopback is unreliable for SPDP. Run on the
//! Linux test host.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserWriterConfig};
use zerodds_rtps::wire_types::{Guid, GuidPrefix};
use zerodds_types::from_ast::{Form, type_component, type_identifier};
use zerodds_types::type_identifier::{StronglyConnectedComponentId, kinds};
use zerodds_types::{TypeIdentifier, TypeInformation};

#[path = "common/mod.rs"]
mod common;

const TIMEOUT: Duration = Duration::from_secs(20);
const TOPIC: &str = "RecursiveWire";

fn wait_until(timeout: Duration, mut pred: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if pred() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn two_runtimes(domain: i32) -> (Arc<DcpsRuntime>, Arc<DcpsRuntime>) {
    let prefix_a = GuidPrefix::from_bytes([0x5A; 12]);
    let prefix_b = {
        let mut p = [0x6B; 12];
        p[..6].copy_from_slice(&prefix_a.to_bytes()[..6]);
        GuidPrefix::from_bytes(p)
    };
    let cfg = RuntimeConfig::default();
    let rt_a = DcpsRuntime::start(domain, prefix_a, cfg.clone()).expect("rt_a");
    let rt_b = DcpsRuntime::start(domain, prefix_b, cfg).expect("rt_b");
    for rt in [&rt_a, &rt_b] {
        assert!(
            wait_until(TIMEOUT, || !rt.discovered_participants().is_empty()),
            "the runtimes did not discover each other"
        );
    }
    (rt_a, rt_b)
}

fn writer_cfg(type_identifier: TypeIdentifier) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: TOPIC.into(),
        type_name: "m::Node".into(),
        reliable: true,
        type_identifier,
        ..UserWriterConfig::default()
    }
}

fn holds(rt: &Arc<DcpsRuntime>, scc: &StronglyConnectedComponentId) -> bool {
    rt.type_lookup_server
        .lock()
        .map(|s| s.registry.holds_component(scc))
        .unwrap_or(false)
}

#[test]
fn a_recursive_topic_type_is_announced_matched_and_fetched_as_a_component() {
    let (rt_a, rt_b) = two_runtimes(common::unique_domain());

    let parsed = zerodds_idl_parse::parse(
        "module m { @appendable struct Node { long value; sequence<Node> children; }; };",
    );
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "recursive.idl").ast;
    ast.resolve_all_by_name().expect("resolve");
    let node = ast
        .find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
            "m".to_string(),
            "Node".to_string(),
        ]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .expect("m::Node");
    let component = type_component(&ast, node)
        .expect("the component builds")
        .expect("Node reaches itself");
    let complete = type_identifier(&ast, node, Form::Complete).expect("complete identifier");
    let minimal = type_identifier(&ast, node, Form::Minimal).expect("minimal identifier");
    let (
        TypeIdentifier::StronglyConnectedComponent(complete_scc),
        TypeIdentifier::StronglyConnectedComponent(minimal_scc),
    ) = (complete.clone(), minimal.clone())
    else {
        panic!("a recursive type is identified by its component: {complete:?} {minimal:?}");
    };
    assert_eq!(complete_scc.equivalence_kind, kinds::EK_COMPLETE);
    assert_eq!(minimal_scc.equivalence_kind, kinds::EK_MINIMAL);

    // A serves the component; B knows nothing of it.
    rt_a.register_type_component(&component).expect("component");
    assert!(!holds(&rt_b, &complete_scc), "precondition");
    assert!(!holds(&rt_b, &minimal_scc), "precondition");

    let _writer = rt_a
        .register_user_writer(writer_cfg(complete.clone()))
        .expect("writer");
    let (reader, _rx) = rt_b
        .register_user_reader(UserReaderConfig {
            topic_name: TOPIC.into(),
            type_name: "m::Node".into(),
            reliable: true,
            type_identifier: complete.clone(),
            ..UserReaderConfig::default()
        })
        .expect("reader");

    // 1. The announcement arrives at B with the TypeInformation of the
    //    component, on both sides.
    let announced = |rt: &Arc<DcpsRuntime>| {
        rt.sedp
            .lock()
            .unwrap()
            .cache()
            .publications_for(rt_a.guid_prefix)
            .filter(|p| p.data.topic_name == TOPIC)
            .filter_map(|p| p.data.type_information.clone())
            .next()
    };
    assert!(
        wait_until(TIMEOUT, || announced(&rt_b).is_some()),
        "B received no PID_TYPE_INFORMATION for the recursive type"
    );
    let ti = TypeInformation::from_bytes_le(&announced(&rt_b).unwrap()).expect("spec-framed");
    assert_eq!(ti.complete.typeid_with_size.type_id, complete);
    assert_eq!(ti.minimal.typeid_with_size.type_id, minimal);

    // 2. The reader and the writer match on it.
    assert!(
        wait_until(TIMEOUT, || !rt_b
            .user_reader_matched_writer_guids(reader)
            .is_empty()),
        "the reader did not match the writer of the recursive type"
    );
    let matched = rt_b.user_reader_matched_writer_guids(reader);
    assert!(
        matched
            .iter()
            .any(|g| Guid::from_bytes(*g).prefix == rt_a.guid_prefix),
        "{matched:?}"
    );

    // 3. B fetches the component with getTypes for the COMPLETE identifier and
    //    holds its members, with the pairs that name their MINIMAL
    //    counterparts (§7.6.3.3.4.2); the MINIMAL identifier brings the
    //    minimal members, and then B holds the component in both forms.
    let members_of_complete = |rt: &Arc<DcpsRuntime>| {
        rt.type_lookup_server
            .lock()
            .map(|s| {
                s.registry
                    .component_members(&complete_scc.hash)
                    .map_or(0, <[_]>::len)
            })
            .unwrap_or(0)
    };
    assert_eq!(members_of_complete(&rt_b), 0, "precondition");
    rt_b.send_type_lookup_request(rt_a.guid_prefix, &[complete])
        .expect("request")
        .expect("peer known");
    assert!(
        wait_until(TIMEOUT, || members_of_complete(&rt_b) == 1),
        "getTypes for the COMPLETE component identifier did not bring its member"
    );
    rt_b.send_type_lookup_request(rt_a.guid_prefix, &[minimal])
        .expect("request")
        .expect("peer known");
    assert!(
        wait_until(TIMEOUT, || holds(&rt_b, &minimal_scc)
            && holds(&rt_b, &complete_scc)),
        "complete form held: {}, minimal form held: {}",
        holds(&rt_b, &complete_scc),
        holds(&rt_b, &minimal_scc)
    );
}
