// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! Cross-participant type resolution: the three things that stood
//! between a peer announcing a type this participant does not hold and
//! the verdict XTypes 1.3 §7.6.3.4.2 gives for it.
//!
//! The path is the ordinary one in a mixed installation — a peer names a
//! type that is not in the local registry — and it was the one the suite
//! never reached: of 227 case blocks of the OMG `dds-xtypes` suite only
//! two pair types out of two different files and expect a refusal, so
//! every other refusal is decided on a registry that already holds both
//! types and none of them exercises resolution at all.
//!
//! One test per layer, each red under one mutation:
//!
//! * `get_types_answers_the_complete_form_of_a_hash` — the request asks
//!   for both hash forms. Mutation: ask only for
//!   `EquivalenceHashMinimal` in `send_type_lookup_request`, and the
//!   reply is `GetTypesReply { types: [] }`.
//! * `a_pair_is_not_wired_before_its_type_resolves_and_is_refused_after`
//!   — the miss path defers instead of wiring, and the reply decides.
//!   Mutation: wire on `Unresolved` at either first-pass site, and a
//!   sample reaches the reader before the verdict; drop the
//!   `revisit_wired_{writer,reader}_match` call at either gate, and a
//!   pair wired on a stale verdict stays matched for ever.
//! * `an_unanswered_lookup_falls_back_to_the_type_names_after_the_grace`
//!   and its twin `_and_refuses_differing_names` — a peer that announces
//!   a hash and never serves it. Mutation: never expire
//!   `TYPE_RESOLUTION_GRACE`, and the pair is deferred for ever, which
//!   is what the strict path used to do to every miss.
//! * `a_pair_whose_dependency_must_be_fetched_still_matches` — the
//!   counter-test, and the one that must not be traded for the other
//!   two: an unresolvable *dependency* is not a refusal. Mutation:
//!   probe only the top-level identifier in `unresolved_type_hash`
//!   (`reg.get_minimal(h)`) instead of the closure, and the walk answers
//!   "not assignable" for a type it simply has not been given, which
//!   layer 2 then acts on by separating a pair that belongs together.
//!
//! Linux-only: macOS multicast loopback is unreliable for SPDP (see
//! `same_host_e2e.rs`). Run on the Linux test host.

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
    DcpsRuntime, RuntimeConfig, TYPE_RESOLUTION_GRACE, UserReaderConfig, UserSample,
    UserWriterConfig,
};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::builder::{Extensibility, TypeObjectBuilder};
use zerodds_types::{
    CompleteTypeObject, EquivalenceHash, MinimalTypeObject, PrimitiveKind, TypeIdentifier,
    TypeObject,
};

#[path = "common/mod.rs"]
mod common;

const RESOLVE_TIMEOUT: Duration = Duration::from_secs(15);

fn i32_id() -> TypeIdentifier {
    TypeIdentifier::Primitive(PrimitiveKind::Int32)
}

fn hash_of(obj: &TypeObject) -> EquivalenceHash {
    zerodds_types::compute_hash(obj).expect("hash")
}

fn writer_config(topic: &str, type_name: &str, type_id: TypeIdentifier) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.into(),
        type_name: type_name.into(),
        type_identifier: type_id,
        ..Default::default()
    }
}

fn reader_config(topic: &str, type_name: &str, type_id: TypeIdentifier) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.into(),
        type_name: type_name.into(),
        type_identifier: type_id,
        ..Default::default()
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

/// `(minimals, completes)` currently in a runtime's TypeLookup registry.
fn registry_len(rt: &Arc<DcpsRuntime>) -> (usize, usize) {
    rt.type_lookup_server
        .lock()
        .map(|s| s.registry.len())
        .unwrap_or((0, 0))
}

fn two_runtimes(domain: i32) -> (Arc<DcpsRuntime>, Arc<DcpsRuntime>) {
    let prefix_a = GuidPrefix::from_bytes([0x5A; 12]);
    let prefix_b = {
        let mut p = [0x6B; 12];
        // Same host-id, so the same-host paths run as they do in a real
        // two-process installation on one machine.
        p[..6].copy_from_slice(&prefix_a.to_bytes()[..6]);
        GuidPrefix::from_bytes(p)
    };
    let cfg = RuntimeConfig::default();
    let rt_a = DcpsRuntime::start(domain, prefix_a, cfg.clone()).expect("rt_a");
    let rt_b = DcpsRuntime::start(domain, prefix_b, cfg).expect("rt_b");
    assert!(
        wait_for_peers(&rt_a, 1, RESOLVE_TIMEOUT),
        "rt_a did not discover rt_b"
    );
    assert!(
        wait_for_peers(&rt_b, 1, RESOLVE_TIMEOUT),
        "rt_b did not discover rt_a"
    );
    (rt_a, rt_b)
}

// ---------------------------------------------------------------------
// Layer 1 — the request asks for the form the peer announced
// ---------------------------------------------------------------------

/// A `CompleteTypeObject` is filed under the hash of the *complete*
/// object, and `TypeLookupServer::handle_get_types` dispatches on the
/// requested identifier's variant: `EquivalenceHashMinimal(h)` is looked
/// up in the minimal map and finds nothing for a complete `h`.
///
/// So a participant announcing `Form::Complete` was unanswerable while
/// the request named only the minimal form — the reply came back
/// `GetTypesReply { types: [] }`. What this test measures is the count:
/// the requesting runtime's registry gains **exactly one** complete
/// entry, filed under the hash that was asked for.
#[test]
fn get_types_answers_the_complete_form_of_a_hash() {
    let (rt_a, rt_b) = two_runtimes(common::unique_domain());

    // A complete object with no dependencies: the whole answer is the
    // one object, so the count below is unambiguous.
    let complete = TypeObject::Complete(CompleteTypeObject::Struct(
        TypeObjectBuilder::struct_type("::tl::CompleteOnly")
            .extensibility(Extensibility::Final)
            .member("a", i32_id(), |m| m)
            .build_complete(),
    ));
    let complete_hash = hash_of(&complete);
    let registered = rt_a.register_type_object(complete).expect("register");
    assert_eq!(registered, complete_hash);

    assert!(
        rt_b.type_lookup_server
            .lock()
            .map(|s| s.registry.get_complete(&complete_hash).is_none())
            .unwrap_or(false),
        "precondition: rt_b must not hold the complete object yet"
    );

    let before = registry_len(&rt_b);
    rt_b.send_type_lookup_request(
        rt_a.guid_prefix,
        &[zerodds_types::TypeIdentifier::EquivalenceHashComplete(
            complete_hash,
        )],
    )
    .expect("request")
    .expect("peer known");

    assert!(
        wait_until(RESOLVE_TIMEOUT, || {
            rt_b.type_lookup_server
                .lock()
                .map(|s| s.registry.get_complete(&complete_hash).is_some())
                .unwrap_or(false)
        }),
        "getTypes for a complete hash returned nothing — the reply was \
         GetTypesReply {{ types: [] }} (n=0)"
    );
    let after = registry_len(&rt_b);
    assert_eq!(
        after.1 - before.1,
        1,
        "expected exactly one complete TypeObject out of the reply (n=1), \
         registry went {before:?} -> {after:?}"
    );
    // A fetched complete object is filed the way a locally registered
    // one is: with its minimal projection beside it. The assignability
    // walk projects a complete pair down and then compares minimal
    // objects, so a registry holding only the complete half reports a
    // type it was just given as unknown.
    assert_eq!(
        after.0 - before.0,
        1,
        "the fetched complete object was not projected to its minimal \
         form, registry went {before:?} -> {after:?}"
    );
}

/// One participant writes the TypeLookup service in CDR_LE
/// (`wire.type_lookup_encoding = xcdr1`), the other in CDR2_LE (XCDR2, the
/// default).
/// Each reads the other's samples in the encoding their identifier names,
/// so a complete object with member annotations — `@optional` members,
/// which the two encodings frame differently, and an `int64` bound, which
/// they align differently — crosses in both directions.
#[test]
fn a_complete_object_crosses_between_an_xcdr1_and_an_xcdr2_participant() {
    use zerodds_types::type_lookup::TypeLookupEncoding;
    use zerodds_types::type_object::common::AnnotationParameterValue;
    let domain = common::unique_domain();
    let xcdr1 = RuntimeConfig {
        type_lookup_encoding: TypeLookupEncoding::Xcdr1,
        ..RuntimeConfig::default()
    };
    let xcdr2 = RuntimeConfig::default();
    assert_eq!(xcdr2.type_lookup_encoding, TypeLookupEncoding::Xcdr2);
    let rt_a = DcpsRuntime::start(domain, GuidPrefix::from_bytes([0x5A; 12]), xcdr1).expect("a");
    let rt_b = DcpsRuntime::start(domain, GuidPrefix::from_bytes([0x6B; 12]), xcdr2).expect("b");
    assert!(
        wait_for_peers(&rt_a, 1, RESOLVE_TIMEOUT),
        "a did not discover b"
    );
    assert!(
        wait_for_peers(&rt_b, 1, RESOLVE_TIMEOUT),
        "b did not discover a"
    );

    let annotated = |name: &str| {
        TypeObject::Complete(CompleteTypeObject::Struct(
            TypeObjectBuilder::struct_type(name)
                .extensibility(Extensibility::Appendable)
                .member("id", i32_id(), |m| m.key())
                .member(
                    "range",
                    zerodds_types::TypeIdentifier::Primitive(zerodds_types::PrimitiveKind::Int64),
                    |m| {
                        m.unit("m")
                            .min(AnnotationParameterValue::Int64(-7))
                            .max(AnnotationParameterValue::Int64(1 << 40))
                    },
                )
                .member("note", i32_id(), |m| m.optional())
                .build_complete(),
        ))
    };
    for (server, client, name) in [
        (&rt_a, &rt_b, "::tl::FromXcdr1"),
        (&rt_b, &rt_a, "::tl::FromXcdr2"),
    ] {
        let object = annotated(name);
        let hash = hash_of(&object);
        server
            .register_type_object(object.clone())
            .expect("register");
        client
            .send_type_lookup_request(
                server.guid_prefix,
                &[zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash)],
            )
            .expect("request")
            .expect("peer known");
        assert!(
            wait_until(RESOLVE_TIMEOUT, || {
                client
                    .type_lookup_server
                    .lock()
                    .map(|s| s.registry.get_complete(&hash).is_some())
                    .unwrap_or(false)
            }),
            "{name}: the reply did not arrive or did not decode"
        );
        let fetched = client
            .type_lookup_server
            .lock()
            .ok()
            .and_then(|s| s.registry.get_complete(&hash).cloned())
            .map(TypeObject::Complete);
        assert_eq!(
            fetched,
            Some(object),
            "{name}: the object changed on the way"
        );
    }
    rt_a.shutdown();
    rt_b.shutdown();
}

// ---------------------------------------------------------------------
// Layer 2 — the verdict is revisited after the reply
// ---------------------------------------------------------------------

/// Writer `@final { a: int16 }` against reader `@final { a: int32 }`.
/// XTypes 1.3 §7.2.4.4.4 Table 15 gives a primitive exactly one
/// assignable source — the same primitive — so the pair is inconsistent
/// on any TCE, and the suite's expectation for such a pair is
/// `INCONSISTENT_TOPIC` on **both** endpoints (§7.6.3.4.2).
///
/// Neither participant holds the other's `TypeObject` at discovery time,
/// so both wire the pair over the resolution-miss path and both fetch.
/// The reply is what turns the verdict into a refusal, and it arrives
/// after the proxy exists. This test measures that the refusal still
/// takes effect: the proxies go away on both sides and both Topics
/// record the status change.
#[test]
fn a_pair_is_not_wired_before_its_type_resolves_and_is_refused_after() {
    const TOPIC: &str = "TlNarrow";
    const TYPE_NAME: &str = "tl::Narrow";
    let (rt_a, rt_b) = two_runtimes(common::unique_domain());

    let writer_obj = TypeObject::Minimal(MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::tl::Narrow")
            .extensibility(Extensibility::Final)
            .member("a", TypeIdentifier::Primitive(PrimitiveKind::Int16), |m| m)
            .build_minimal(),
    ));
    let reader_obj = TypeObject::Minimal(MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::tl::Narrow")
            .extensibility(Extensibility::Final)
            .member("a", i32_id(), |m| m)
            .build_minimal(),
    ));
    let w_hash = hash_of(&writer_obj);
    let r_hash = hash_of(&reader_obj);
    assert_ne!(w_hash, r_hash);

    // Each side registers only its own type — the mixed-installation
    // case, and the only one in which the verdict can change late.
    rt_a.register_type_object(writer_obj).expect("register w");
    rt_b.register_type_object(reader_obj).expect("register r");

    let writer_eid = rt_a
        .register_user_writer(writer_config(
            TOPIC,
            TYPE_NAME,
            TypeIdentifier::EquivalenceHashMinimal(w_hash),
        ))
        .expect("writer");
    let (reader_eid, rx) = rt_b
        .register_user_reader(reader_config(
            TOPIC,
            TYPE_NAME,
            TypeIdentifier::EquivalenceHashMinimal(r_hash),
        ))
        .expect("reader");

    // Both sides fetch the peer's object, so both registries fill. While
    // that round trip is out, the writer writes and the reader must not
    // be matched and must not receive: "the DataWriter and DataReader
    // shall not communicate" is not a verdict that can be revised after
    // a sample went through.
    let mut wired_early = false;
    assert!(
        wait_until(RESOLVE_TIMEOUT, || {
            let _ = rt_a.write_user_sample(writer_eid, b"early".to_vec());
            if rt_b.user_reader_matched_count(reader_eid) > 0
                || rt_a.user_writer_matched_count(writer_eid) > 0
            {
                wired_early = true;
            }
            rt_b.type_lookup_server
                .lock()
                .map(|s| s.registry.get_minimal(&w_hash).is_some())
                .unwrap_or(false)
        }),
        "rt_b never fetched the writer's TypeObject"
    );
    assert!(!wired_early, "the pair was wired before its type resolved");
    assert!(
        rx.try_recv().is_err(),
        "a sample reached the reader before the verdict"
    );

    // §7.6.3.4.2: "the DataWriter and DataReader shall not communicate
    // with each other" — on both endpoints.
    assert!(
        wait_until(RESOLVE_TIMEOUT, || {
            rt_b.user_reader_matched_count(reader_eid) == 0
                && rt_b.inconsistent_topic_count_for(TOPIC) >= 1
        }),
        "reader stayed matched on a verdict that had become a refusal: \
         matched={}, inconsistent_topic={}",
        rt_b.user_reader_matched_count(reader_eid),
        rt_b.inconsistent_topic_count_for(TOPIC)
    );
    // "...and the Service shall trigger an INCONSISTENT_TOPIC status
    // change for both the DataReader's Topic and the DataWriter's
    // Topic" — the writer's half.
    assert!(
        wait_until(RESOLVE_TIMEOUT, || {
            rt_a.user_writer_matched_count(writer_eid) == 0
                && rt_a.inconsistent_topic_count_for(TOPIC) >= 1
        }),
        "writer stayed matched / raised no INCONSISTENT_TOPIC: \
         matched={}, inconsistent_topic={}",
        rt_a.user_writer_matched_count(writer_eid),
        rt_a.inconsistent_topic_count_for(TOPIC)
    );
}

// ---------------------------------------------------------------------
// Layer 3 — an unresolvable dependency is not a refusal
// ---------------------------------------------------------------------

/// The counter-test. A pair that belongs together, whose types are
/// **both** unknown to the peer and whose top-level objects reference a
/// nested type that is unknown too, must still match.
///
/// The layout is the one that separates the two verdicts:
///
/// * writer `@appendable Outer { a: int32, ext: Nested_w }`,
///   `Nested_w = @appendable { x: int32, y: @optional int32 }`;
/// * reader `@appendable Outer { a: int32, ext: Nested_r }`,
///   `Nested_r = @appendable { x: int32 }`.
///
/// Writer-superset against reader-prefix in both aggregates: assignable
/// under §7.2.4.4.4.4, and four distinct hashes, of which the reader's
/// participant holds two.
///
/// The first `getTypes` reply brings `Outer_w` and nothing else — a
/// reply carries the objects that were asked for, and the dependency's
/// hash is only readable *out of* `Outer_w`. A registry that then holds
/// `Outer_w` but not `Nested_w` answers the top-level lookup, and the
/// assignability walk falls over inside it with the only answer it has a
/// shape for: `No("unknown type objects for hash comparison")`. Layer 2
/// acts on that, so the pair that belongs together is the one that gets
/// separated. Hence: a missing dependency is `Unresolved` and another
/// round trip, never `Inconsistent`.
#[test]
fn a_pair_whose_dependency_must_be_fetched_still_matches() {
    const TOPIC: &str = "TlNested";
    const TYPE_NAME: &str = "tl::Outer";
    let (rt_a, rt_b) = two_runtimes(common::unique_domain());

    let nested_w = TypeObject::Minimal(MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::tl::Nested")
            .extensibility(Extensibility::Appendable)
            .member("x", i32_id(), |m| m)
            .member("y", i32_id(), |m| m.optional())
            .build_minimal(),
    ));
    let nested_r = TypeObject::Minimal(MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::tl::Nested")
            .extensibility(Extensibility::Appendable)
            .member("x", i32_id(), |m| m)
            .build_minimal(),
    ));
    let nested_w_hash = hash_of(&nested_w);
    let nested_r_hash = hash_of(&nested_r);
    assert_ne!(nested_w_hash, nested_r_hash);

    let outer = |nested: EquivalenceHash| {
        TypeObject::Minimal(MinimalTypeObject::Struct(
            TypeObjectBuilder::struct_type("::tl::Outer")
                .extensibility(Extensibility::Appendable)
                .member("a", i32_id(), |m| m)
                .member("ext", TypeIdentifier::EquivalenceHashMinimal(nested), |m| m)
                .build_minimal(),
        ))
    };
    let outer_w = outer(nested_w_hash);
    let outer_r = outer(nested_r_hash);
    let outer_w_hash = hash_of(&outer_w);
    let outer_r_hash = hash_of(&outer_r);
    assert_ne!(outer_w_hash, outer_r_hash);

    rt_a.register_type_object(nested_w).expect("register n_w");
    rt_a.register_type_object(outer_w).expect("register o_w");
    rt_b.register_type_object(nested_r).expect("register n_r");
    rt_b.register_type_object(outer_r).expect("register o_r");
    assert!(
        rt_b.type_lookup_server
            .lock()
            .map(|s| {
                s.registry.get_minimal(&outer_w_hash).is_none()
                    && s.registry.get_minimal(&nested_w_hash).is_none()
            })
            .unwrap_or(false),
        "precondition: rt_b holds neither the writer's aggregate nor its nested type"
    );

    let writer_eid = rt_a
        .register_user_writer(writer_config(
            TOPIC,
            TYPE_NAME,
            TypeIdentifier::EquivalenceHashMinimal(outer_w_hash),
        ))
        .expect("writer");
    let (reader_eid, rx) = rt_b
        .register_user_reader(reader_config(
            TOPIC,
            TYPE_NAME,
            TypeIdentifier::EquivalenceHashMinimal(outer_r_hash),
        ))
        .expect("reader");

    // The dependency is fetched, not guessed: the nested type reaches
    // the reader's registry on its own round trip.
    assert!(
        wait_until(RESOLVE_TIMEOUT, || {
            rt_b.type_lookup_server
                .lock()
                .map(|s| s.registry.get_minimal(&nested_w_hash).is_some())
                .unwrap_or(false)
        }),
        "the writer type's nested dependency was never fetched"
    );
    assert!(
        wait_until(RESOLVE_TIMEOUT, || {
            rt_b.user_reader_matched_count(reader_eid) >= 1
        }),
        "a pair that belongs together did not match once its dependency was resolved"
    );

    // And it stays matched: no INCONSISTENT_TOPIC on either Topic.
    let mut received = false;
    for _ in 0..8u8 {
        rt_a.write_user_sample(writer_eid, b"nested".to_vec())
            .expect("write");
        if let Ok(UserSample::Alive { payload, .. }) = rx.recv_timeout(Duration::from_secs(2)) {
            assert_eq!(payload.as_slice(), b"nested");
            received = true;
            break;
        }
    }
    assert!(received, "no sample arrived over the resolved match");
    assert_eq!(
        rt_b.inconsistent_topic_count_for(TOPIC),
        0,
        "the reader called a resolvable pair inconsistent"
    );
    assert_eq!(
        rt_a.inconsistent_topic_count_for(TOPIC),
        0,
        "the writer called a resolvable pair inconsistent"
    );
    assert!(
        rt_a.user_writer_matched_count(writer_eid) >= 1,
        "the writer un-wired a pair that belongs together"
    );
}

// ---------------------------------------------------------------------
// Layer 4 — a peer that never answers: Step 2 after the grace
// ---------------------------------------------------------------------

/// A hash no registry holds: the peer announces it and serves nothing
/// for it, so the round trip never resolves. Past
/// `TYPE_RESOLUTION_GRACE` the clause's Step 2 decides on the type
/// names — equal here, so the pair matches and communicates.
#[test]
fn an_unanswered_lookup_falls_back_to_the_type_names_after_the_grace() {
    const TOPIC: &str = "TlUnanswered";
    const TYPE_NAME: &str = "tl::Unanswered";
    let (rt_a, rt_b) = two_runtimes(common::unique_domain());

    let reader_obj = TypeObject::Minimal(MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::tl::Unanswered")
            .extensibility(Extensibility::Final)
            .member("a", i32_id(), |m| m)
            .build_minimal(),
    ));
    let r_hash = hash_of(&reader_obj);
    rt_b.register_type_object(reader_obj).expect("register r");
    let unserved = EquivalenceHash([0xAB; 14]);
    assert_ne!(unserved, r_hash);

    let writer_eid = rt_a
        .register_user_writer(writer_config(
            TOPIC,
            TYPE_NAME,
            TypeIdentifier::EquivalenceHashMinimal(unserved),
        ))
        .expect("writer");
    let started = Instant::now();
    let (reader_eid, rx) = rt_b
        .register_user_reader(reader_config(
            TOPIC,
            TYPE_NAME,
            TypeIdentifier::EquivalenceHashMinimal(r_hash),
        ))
        .expect("reader");

    assert!(
        wait_until(RESOLVE_TIMEOUT + TYPE_RESOLUTION_GRACE, || {
            rt_b.user_reader_matched_count(reader_eid) >= 1
                && rt_a.user_writer_matched_count(writer_eid) >= 1
        }),
        "an unanswered lookup with equal type names never matched: reader={}, writer={}",
        rt_b.user_reader_matched_count(reader_eid),
        rt_a.user_writer_matched_count(writer_eid)
    );
    assert!(
        started.elapsed() >= TYPE_RESOLUTION_GRACE,
        "the pair matched inside the grace, before Step 2 could apply"
    );
    let mut received = false;
    for _ in 0..8u8 {
        rt_a.write_user_sample(writer_eid, b"late".to_vec())
            .expect("write");
        if let Ok(UserSample::Alive { payload, .. }) = rx.recv_timeout(Duration::from_secs(2)) {
            assert_eq!(payload.as_slice(), b"late");
            received = true;
            break;
        }
    }
    assert!(received, "no sample arrived over the Step 2 match");
    assert_eq!(rt_b.inconsistent_topic_count_for(TOPIC), 0);
}

/// The same unanswered hash with a differing type name: Step 2 fails,
/// the pair is never wired, and both Topics see INCONSISTENT_TOPIC.
#[test]
fn an_unanswered_lookup_refuses_differing_names_after_the_grace() {
    const TOPIC: &str = "TlUnansweredDiff";
    let (rt_a, rt_b) = two_runtimes(common::unique_domain());

    let reader_obj = TypeObject::Minimal(MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::tl::ReaderSide")
            .extensibility(Extensibility::Final)
            .member("a", i32_id(), |m| m)
            .build_minimal(),
    ));
    let r_hash = hash_of(&reader_obj);
    rt_b.register_type_object(reader_obj).expect("register r");

    let writer_eid = rt_a
        .register_user_writer(writer_config(
            TOPIC,
            "tl::WriterSide",
            TypeIdentifier::EquivalenceHashMinimal(EquivalenceHash([0xCD; 14])),
        ))
        .expect("writer");
    let (reader_eid, rx) = rt_b
        .register_user_reader(reader_config(
            TOPIC,
            "tl::ReaderSide",
            TypeIdentifier::EquivalenceHashMinimal(r_hash),
        ))
        .expect("reader");

    assert!(
        wait_until(RESOLVE_TIMEOUT + TYPE_RESOLUTION_GRACE, || {
            let _ = rt_a.write_user_sample(writer_eid, b"never".to_vec());
            rt_b.inconsistent_topic_count_for(TOPIC) >= 1
                && rt_a.inconsistent_topic_count_for(TOPIC) >= 1
        }),
        "no INCONSISTENT_TOPIC after the grace: reader topic={}, writer topic={}",
        rt_b.inconsistent_topic_count_for(TOPIC),
        rt_a.inconsistent_topic_count_for(TOPIC)
    );
    assert_eq!(rt_b.user_reader_matched_count(reader_eid), 0);
    assert_eq!(rt_a.user_writer_matched_count(writer_eid), 0);
    assert!(rx.try_recv().is_err(), "a sample reached a refused reader");
}

// ---------------------------------------------------------------------
// Layer 5 — the peer's identity is the key, not the hash we compute
// ---------------------------------------------------------------------

/// A peer that hashes the same object to a different identity — RTI
/// Connext 7.7.0 and CoreDX 6.15.3 do, for MINIMAL objects, measured
/// 2026-09-13 — announces its own hash and answers `getTypes` for it.
/// The reply's object has to be filed under the identifier it came
/// back with; filed under our own hash alone, the announced hash never
/// resolves and the pair falls to Step 2. Here the peer's "own hash" is
/// simulated by registering the writer's object under an arbitrary
/// hash and announcing that; the type names differ so that Step 2
/// cannot rescue the pair and only resolution can match it.
#[test]
fn a_peers_object_is_filed_under_the_identity_the_peer_announced() {
    const TOPIC: &str = "TlForeignHash";
    let (rt_a, rt_b) = two_runtimes(common::unique_domain());

    let writer_obj = MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::tl::WriterCamp")
            .extensibility(Extensibility::Appendable)
            .member("a", i32_id(), |m| m)
            .build_minimal(),
    );
    let reader_obj = TypeObject::Minimal(MinimalTypeObject::Struct(
        TypeObjectBuilder::struct_type("::tl::ReaderCamp")
            .extensibility(Extensibility::Appendable)
            .member("a", i32_id(), |m| m)
            .member("b", i32_id(), |m| m)
            .build_minimal(),
    ));
    let foreign = EquivalenceHash([0x5A; 14]);
    assert_ne!(
        foreign,
        hash_of(&TypeObject::Minimal(writer_obj.clone())),
        "the simulated peer hash must differ from ours"
    );
    rt_a.type_lookup_server
        .lock()
        .expect("server")
        .registry
        .insert_minimal(foreign, writer_obj);
    let r_hash = hash_of(&reader_obj);
    rt_b.register_type_object(reader_obj).expect("register r");

    let writer_eid = rt_a
        .register_user_writer(writer_config(
            TOPIC,
            "tl::WriterCamp",
            TypeIdentifier::EquivalenceHashMinimal(foreign),
        ))
        .expect("writer");
    let (reader_eid, rx) = rt_b
        .register_user_reader(reader_config(
            TOPIC,
            "tl::ReaderCamp",
            TypeIdentifier::EquivalenceHashMinimal(r_hash),
        ))
        .expect("reader");

    assert!(
        wait_until(RESOLVE_TIMEOUT, || {
            rt_b.type_lookup_server
                .lock()
                .map(|s| s.registry.get_minimal(&foreign).is_some())
                .unwrap_or(false)
        }),
        "the fetched object was not filed under the peer's announced hash"
    );
    assert!(
        wait_until(RESOLVE_TIMEOUT, || {
            rt_b.user_reader_matched_count(reader_eid) >= 1
                && rt_a.user_writer_matched_count(writer_eid) >= 1
        }),
        "an assignable pair announced under a foreign hash did not match: reader={}, writer={}, inconsistent={}",
        rt_b.user_reader_matched_count(reader_eid),
        rt_a.user_writer_matched_count(writer_eid),
        rt_b.inconsistent_topic_count_for(TOPIC)
    );
    let mut received = false;
    for _ in 0..8u8 {
        rt_a.write_user_sample(writer_eid, b"camp".to_vec())
            .expect("write");
        if let Ok(UserSample::Alive { .. }) = rx.recv_timeout(Duration::from_secs(2)) {
            received = true;
            break;
        }
    }
    assert!(received, "no sample arrived over the resolved match");
    assert_eq!(rt_b.inconsistent_topic_count_for(TOPIC), 0);
}
