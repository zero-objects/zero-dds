// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! End-to-end for the Gap 2 (#24) TypeLookup resolution-miss path.
//!
//! Two real `DcpsRuntime` on loopback SEDP. The writer registers its
//! `TypeObject` (V2) locally; a reader on the second runtime carries only
//! the writer type's *identifier* (no V2 object). On the SEDP match the
//! reader detects the resolution-miss and issues a `getTypes` request; the
//! reply populates the reader's TypeLookup registry, and the subsequent
//! matching pass resolves the structural (appendable, widening-safe) match.
//!
//! Evolution direction: writer carries the SUPERSET (V2 = V1 + a trailing
//! @optional member), reader the PREFIX (V1) — the direction the appendable
//! assignability rule supports (`assignability.rs`, XTypes §7.2.4.4.4.4).
//! The reader registers its OWN V1 object so only the writer's V2 object is
//! missing and must be fetched.
//!
//! Two variants:
//!   * non-strict (default TCE): match resolves (optimistic + confirming
//!     fetch, and the structural check once V2 is present);
//!   * `force_type_validation` (strict): the reader DEFERS the match on the
//!     miss (no optimistic match) and resolves only after the getTypes reply
//!     populates the registry (real async resolution).
//!
//! Linux-only: macOS multicast loopback is unreliable for SPDP (see
//! `same_host_e2e.rs`). Run on codepit.

#![cfg(all(target_os = "linux", feature = "std"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::field_reassign_with_default
)]

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_qos::{
    DeadlineQosPolicy, DurabilityKind, LifespanQosPolicy, LivelinessQosPolicy, OwnershipKind,
};
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};
use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::qos::TypeConsistencyEnforcement;
use zerodds_types::{
    EquivalenceHash, MinimalTypeObject, PrimitiveKind, TypeIdentifier, TypeObject,
};

const TOPIC: &str = "Gap2Evo";

fn i32_id() -> TypeIdentifier {
    TypeIdentifier::Primitive(PrimitiveKind::Int32)
}

/// V1 = {a: i32, b: i32}, @appendable (the reader/prefix type).
fn v1_object() -> TypeObject {
    TypeObject::Minimal(MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::gap2::Evo")
            .extensibility(Extensibility::Appendable)
            .member("a", i32_id(), |m| m)
            .member("b", i32_id(), |m| m)
            .build_minimal(),
    ))
}

/// V2 = V1 + trailing @optional c: i32, @appendable (the writer/superset).
fn v2_object() -> TypeObject {
    TypeObject::Minimal(MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::gap2::Evo")
            .extensibility(Extensibility::Appendable)
            .member("a", i32_id(), |m| m)
            .member("b", i32_id(), |m| m)
            .member("c", i32_id(), |m| m.optional())
            .build_minimal(),
    ))
}

fn hash_of(obj: &TypeObject) -> EquivalenceHash {
    zerodds_types::compute_hash(obj).expect("hash")
}

fn writer_config(type_id: TypeIdentifier) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: TOPIC.into(),
        type_name: "gap2::Evo".into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        lifespan: LifespanQosPolicy::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        ownership_strength: 0,
        presentation: Default::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: type_id,
        data_representation_offer: None,
    }
}

fn reader_config(type_id: TypeIdentifier, tce: TypeConsistencyEnforcement) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: TOPIC.into(),
        type_name: "gap2::Evo".into(),
        reliable: true,
        durability: DurabilityKind::Volatile,
        deadline: DeadlineQosPolicy::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        liveliness: LivelinessQosPolicy::default(),
        ownership: OwnershipKind::Shared,
        presentation: Default::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: type_id,
        type_consistency: tce,
        data_representation_offer: None,
    }
}

fn wait_for_peers(rt: &Arc<DcpsRuntime>, n: usize, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if rt.discovered_participants().len() >= n {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn wait_for_reader_matched(rt: &Arc<DcpsRuntime>, eid: EntityId, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if rt.user_reader_matched_count(eid) >= 1 {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn registry_has(rt: &Arc<DcpsRuntime>, hash: &EquivalenceHash) -> bool {
    rt.type_lookup_server
        .lock()
        .map(|s| s.registry.get_minimal(hash).is_some())
        .unwrap_or(false)
}

fn wait_for_registry(rt: &Arc<DcpsRuntime>, hash: &EquivalenceHash, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if registry_has(rt, hash) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

fn two_runtimes(domain: i32) -> (Arc<DcpsRuntime>, Arc<DcpsRuntime>) {
    let prefix_a = GuidPrefix::from_bytes([0xA7; 12]);
    let prefix_b = {
        let mut p = [0xB8; 12];
        p[..4].copy_from_slice(&prefix_a.to_bytes()[..4]); // same host-id
        GuidPrefix::from_bytes(p)
    };
    let rt_a = DcpsRuntime::start(domain, prefix_a, RuntimeConfig::default()).expect("rt_a");
    let rt_b = DcpsRuntime::start(domain, prefix_b, RuntimeConfig::default()).expect("rt_b");
    assert!(
        wait_for_peers(&rt_a, 1, Duration::from_secs(10)),
        "rt_a !see rt_b"
    );
    assert!(
        wait_for_peers(&rt_b, 1, Duration::from_secs(10)),
        "rt_b !see rt_a"
    );
    (rt_a, rt_b)
}

/// Runs the fetch-and-resolve scenario with the reader's TCE. Returns after
/// asserting the reader matched and the writer's V2 object was fetched into
/// the reader-runtime registry.
fn run_fetch_and_resolve(domain: i32, tce: TypeConsistencyEnforcement) {
    let (rt_a, rt_b) = two_runtimes(domain);

    let v1 = v1_object();
    let v2 = v2_object();
    let v2_hash = hash_of(&v2);
    let v1_hash = hash_of(&v1);
    assert_ne!(v1_hash, v2_hash, "V1/V2 must have distinct hashes");

    // Writer side: registers V2 locally so it can answer a getTypes for V2.
    let w_id = rt_a.register_type_object(v2).expect("register v2 on rt_a");
    assert_eq!(w_id, v2_hash);
    let writer_eid = rt_a
        .register_user_writer(writer_config(TypeIdentifier::EquivalenceHashMinimal(
            v2_hash,
        )))
        .expect("writer");

    // Reader side: registers only its OWN V1 object — the writer's V2 object
    // is ABSENT here and must be fetched via getTypes.
    let r_id = rt_b.register_type_object(v1).expect("register v1 on rt_b");
    assert_eq!(r_id, v1_hash);
    assert!(
        !registry_has(&rt_b, &v2_hash),
        "precondition: rt_b must NOT hold the writer's V2 object yet"
    );
    let (reader_eid, rx) = rt_b
        .register_user_reader(reader_config(
            TypeIdentifier::EquivalenceHashMinimal(v1_hash),
            tce,
        ))
        .expect("reader");

    // The reader issues getTypes(V2) on the resolution-miss; the reply
    // populates rt_b's registry.
    assert!(
        wait_for_registry(&rt_b, &v2_hash, Duration::from_secs(10)),
        "reader did not fetch the writer's V2 TypeObject via getTypes"
    );
    // ...and the (deferred / optimistic) match resolves.
    assert!(
        wait_for_reader_matched(&rt_b, reader_eid, Duration::from_secs(10)),
        "typed match did not resolve after the TypeLookup reply"
    );

    // Sample flows end-to-end.
    let mut received = false;
    for _ in 0..5u8 {
        rt_a.write_user_sample(writer_eid, b"evolved".to_vec())
            .expect("write");
        if let Ok(UserSample::Alive { payload, .. }) = rx.recv_timeout(Duration::from_secs(2)) {
            assert_eq!(payload.as_slice(), b"evolved");
            received = true;
            break;
        }
    }
    assert!(
        received,
        "reader did not receive a sample after the match resolved"
    );
}

/// Non-strict (default TCE): the resolution-miss resolves via getTypes.
#[test]
fn gap2_typelookup_fetch_resolves_match_nonstrict() {
    run_fetch_and_resolve(151, TypeConsistencyEnforcement::default());
}

/// Strict (`force_type_validation`): the reader DEFERS on the miss and
/// resolves only once the getTypes reply lands (real async resolution). The
/// appendable prefix match needs no coercion, so it still resolves under
/// strict validation.
#[test]
fn gap2_typelookup_fetch_resolves_match_strict() {
    let mut tce = TypeConsistencyEnforcement::default();
    tce.force_type_validation = true;
    run_fetch_and_resolve(152, tce);
}

/// Strict fail-closed: an UNRESOLVED writer type that is structurally
/// INCOMPATIBLE (a @final narrowing) must NEVER match under
/// `force_type_validation`, even after the object is fetched — the reader
/// must not optimistically match on the miss.
#[test]
fn gap2_typelookup_strict_incompatible_never_matches() {
    let (rt_a, rt_b) = two_runtimes(153);

    // Writer @final {a: i16}; reader @final {a: i32}. Under strict TCE
    // (no coercion) i16↛i32 → structurally incompatible.
    let writer_obj = TypeObject::Minimal(MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::gap2::Narrow")
            .extensibility(Extensibility::Final)
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int16), |m| m)
            .build_minimal(),
    ));
    let reader_obj = TypeObject::Minimal(MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::gap2::Narrow")
            .extensibility(Extensibility::Final)
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int32), |m| m)
            .build_minimal(),
    ));
    let w_hash = hash_of(&writer_obj);
    let r_hash = hash_of(&reader_obj);

    rt_a.register_type_object(writer_obj)
        .expect("register writer obj");
    let mut w_cfg = writer_config(TypeIdentifier::EquivalenceHashMinimal(w_hash));
    w_cfg.type_name = "gap2::Narrow".into();
    w_cfg.topic_name = "Gap2Narrow".into();
    let _writer_eid = rt_a.register_user_writer(w_cfg).expect("writer");

    rt_b.register_type_object(reader_obj)
        .expect("register reader obj");
    let mut tce = TypeConsistencyEnforcement::default();
    tce.force_type_validation = true;
    let mut r_cfg = reader_config(TypeIdentifier::EquivalenceHashMinimal(r_hash), tce);
    r_cfg.type_name = "gap2::Narrow".into();
    r_cfg.topic_name = "Gap2Narrow".into();
    let (reader_eid, _rx) = rt_b.register_user_reader(r_cfg).expect("reader");

    // Allow the fetch + re-evaluation to run.
    let _ = wait_for_registry(&rt_b, &w_hash, Duration::from_secs(5));
    std::thread::sleep(Duration::from_secs(1));

    assert_eq!(
        rt_b.user_reader_matched_count(reader_eid),
        0,
        "strict reader must NOT match a structurally-incompatible writer type"
    );
}
