// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! `getTypeDependencies` between two runtimes (DDS-XTypes 1.3 §7.6.3.3.4.1).
//!
//! "The TypeIdentifiers shall be only direct HASH Identifiers. [...] The
//! TypeIdentifiers shall not include identifiers for individual types in
//! Strongly Connected Components". The reply names the direct hash
//! identifiers of the types the asked ones refer to; the requester gets the
//! objects with `getTypes`. What the tests measure is that a request over the
//! wire for a type that depends on another one makes the requester hold the
//! other one, and not the asked one: the reply carried the dependency.
//!
//! Linux-only: macOS multicast loopback is unreliable for SPDP. Run on the
//! Linux test host.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::from_ast::{
    Form, complete_type_object, minimal_type_object, type_component, type_identifier,
};
use zerodds_types::{
    EquivalenceHash, MinimalTypeObject, PrimitiveKind, TypeIdentifier, TypeObject,
};

#[path = "common/mod.rs"]
mod common;

const TIMEOUT: Duration = Duration::from_secs(15);

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
    let prefix_a = GuidPrefix::from_bytes([0x3A; 12]);
    let prefix_b = {
        let mut p = [0x4B; 12];
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

fn holds_minimal(rt: &Arc<DcpsRuntime>, hash: &EquivalenceHash) -> bool {
    rt.type_lookup_server
        .lock()
        .map(|s| s.registry.get_minimal(hash).is_some())
        .unwrap_or(false)
}

/// A type that depends on another one by hash: the request for the
/// dependencies of `Outer` is answered with `Inner`, which the requester
/// then fetches; `Outer` itself is not part of the answer.
#[test]
fn the_dependencies_of_a_hash_come_over_the_wire() {
    let (rt_a, rt_b) = two_runtimes(common::unique_domain());

    let inner = TypeObject::Minimal(MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::dep::Inner")
            .extensibility(Extensibility::Final)
            .member("x", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_minimal(),
    ));
    let inner_hash = zerodds_types::compute_hash(&inner).expect("hash");
    let outer = TypeObject::Minimal(MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::dep::Outer")
            .extensibility(Extensibility::Final)
            .member(
                "i",
                TypeIdentifier::EquivalenceHashMinimal(inner_hash),
                |m| m,
            )
            .build_minimal(),
    ));
    let outer_hash = zerodds_types::compute_hash(&outer).expect("hash");
    rt_a.register_type_object(inner).expect("register inner");
    rt_a.register_type_object(outer).expect("register outer");

    assert!(!holds_minimal(&rt_b, &inner_hash), "precondition");
    assert!(!holds_minimal(&rt_b, &outer_hash), "precondition");
    rt_b.send_type_dependencies_request(
        rt_a.guid_prefix,
        &[TypeIdentifier::EquivalenceHashMinimal(outer_hash)],
    )
    .expect("request")
    .expect("peer known");

    assert!(
        wait_until(TIMEOUT, || holds_minimal(&rt_b, &inner_hash)),
        "the reply to getTypeDependencies did not name the dependency, or the follow-up \
         getTypes did not bring it"
    );
    assert!(
        !holds_minimal(&rt_b, &outer_hash),
        "the asked type is not among its own dependencies"
    );
}

/// A strongly connected component is asked for as a whole: the request names
/// the identifier of the component, the reply the types its members refer to
/// outside the component.
#[test]
fn the_dependencies_of_a_component_come_over_the_wire() {
    let (rt_a, rt_b) = two_runtimes(common::unique_domain());

    let parsed = zerodds_idl_parse::parse(
        "module m {\
           @appendable struct Leaf { long x; };\
           struct B;\
           @appendable struct A { sequence<B> bs; Leaf leaf; };\
           @appendable struct B { sequence<A> as_; };\
         };",
    );
    assert!(!parsed.has_errors(), "{:?}", parsed.diagnostics());
    let mut ast = zerodds_idl_lower::lower(&parsed.syntax(), "component.idl").ast;
    ast.resolve_all_by_name().expect("resolve");
    let find = |name: &str| {
        ast.find_by_scoped_name(&zerodds_ast_core::ScopedName::absolute(vec![
            "m".to_string(),
            name.to_string(),
        ]))
        .ok()
        .and_then(zerodds_ast::AnyId::as_type)
        .unwrap_or_else(|| panic!("m::{name}"))
    };
    let (leaf, a) = (find("Leaf"), find("A"));

    let component = type_component(&ast, a)
        .expect("the component builds")
        .expect("A and B reach each other");
    rt_a.register_type_component(&component).expect("component");
    rt_a.register_type_object(TypeObject::Minimal(
        minimal_type_object(&ast, leaf).expect("leaf minimal"),
    ))
    .expect("leaf minimal registered");
    rt_a.register_type_object(TypeObject::Complete(
        complete_type_object(&ast, leaf).expect("leaf complete"),
    ))
    .expect("leaf complete registered");
    let TypeIdentifier::EquivalenceHashMinimal(leaf_hash) =
        type_identifier(&ast, leaf, Form::Minimal).expect("leaf identifier")
    else {
        panic!("a struct is identified by its hash");
    };
    let component_id = type_identifier(&ast, a, Form::Minimal).expect("component identifier");
    assert!(
        matches!(component_id, TypeIdentifier::StronglyConnectedComponent(_)),
        "{component_id:?}"
    );

    assert!(!holds_minimal(&rt_b, &leaf_hash), "precondition");
    rt_b.send_type_dependencies_request(rt_a.guid_prefix, &[component_id])
        .expect("request")
        .expect("peer known");
    assert!(
        wait_until(TIMEOUT, || holds_minimal(&rt_b, &leaf_hash)),
        "the component's dependency (Leaf, named by a member of A) did not arrive"
    );
}
