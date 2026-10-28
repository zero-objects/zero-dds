// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! D7 follow-up: a dispose forwarded through `ForwardingSession` carries its
//! `key_holder` onto the output writer's wire `serializedKey` — the form
//! Cyclone 11.0.1, Fast DDS 3.6.1 and OpenDDS 3.35 were measured to act on
//! (0/4 disposed for a hash-only marker, 4/4 once it carries the key) —
//! whenever `SessionParts::same_type` says the input reader's type and the
//! output writer's type are the same one. Otherwise the forwarded holder is
//! withheld, and the output writer reads the key from the key hash under
//! its own type when the key fits in it (DDSI-RTPS 2.5 §9.6.4.8).
//!
//! Two real `DcpsRuntime`s talk over real loopback RTPS: `bridge_rt` hosts
//! the forwarding session's output writer, `sink_rt` a plain keyed reader
//! that observes what actually went out on the wire. The dispose is injected
//! synthetically (a raw `mpsc` channel standing in for the forwarding
//! session's input reader — the input-side wire decode that fills
//! `UserSample::Lifecycle::key_holder` is unit-tested in `zerodds-dcps`
//! already), which also means `sink_rt`'s reader never sees a matching
//! ALIVE sample and so never caches a holder of its own: any `key_holder` it
//! reports on the dispose can only have come from decoding a genuine
//! `serializedKey` on THIS datagram, not a reused one.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_dcps::serialized_type::SerializedTypeSupport;
use zerodds_routing_service::forwarding::{ForwardingSession, SessionParts};
use zerodds_routing_service::metrics::RouteMetrics;
use zerodds_rtps::EntityId;
use zerodds_rtps::wire_types::GuidPrefix;

/// `@final struct KeyedFinal { @key long id; long value; };`'s
/// `CompleteTypeObject` — the same one-liner
/// `crates/dcps/src/runtime.rs`'s own `keyed_final_type_object` test helper
/// builds, so this test's wire form is the one already measured there.
fn keyed_final_type_object() -> zerodds_types::TypeObject {
    let ast = Arc::new(
        zerodds_wire_probe::encode::model_from_idl(
            "@final struct KeyedFinal { @key long id; long value; };",
            "keyed-final.idl",
        )
        .expect("model"),
    );
    let type_ = zerodds_types::dynamic::DynamicType::find(&ast, "KeyedFinal").expect("type");
    zerodds_types::TypeObject::Complete(
        zerodds_types::from_ast::complete_type_object(&ast, type_.id()).expect("object"),
    )
}

/// Registers `KeyedFinal` on `rt` and returns its resolved
/// `SerializedTypeSupport` (to compute the canonical key holder the test
/// injects) plus the `TypeIdentifier` a reader/writer config needs to
/// resolve the same registration.
fn register_keyed_final(
    rt: &Arc<DcpsRuntime>,
) -> (Arc<SerializedTypeSupport>, zerodds_types::TypeIdentifier) {
    let object = keyed_final_type_object();
    let bytes = object.to_bytes_le().expect("encode object");
    let support =
        SerializedTypeSupport::from_type_object("KeyedFinal", &bytes, true, 0).expect("support");
    let hash = rt.register_type_object(object).expect("register");
    (
        support,
        zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash),
    )
}

fn wait_until(mut cond: impl FnMut() -> bool, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !cond() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Builds `bridge_rt` (hosts the forwarding session's output writer) and
/// `sink_rt` (a plain keyed reader matched to it), both on the same fresh
/// domain, both with `KeyedFinal` registered. Returns the writer's
/// `EntityId` on `bridge_rt`, the sink's sample channel, and the
/// `SerializedTypeSupport` used to compute the injected `key_holder`.
fn setup() -> (
    Arc<DcpsRuntime>,
    EntityId,
    mpsc::Receiver<UserSample>,
    Arc<SerializedTypeSupport>,
) {
    let domain = zerodds_test_domains::unique_domain();
    let bridge_rt = DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes([0xB1; 12]),
        RuntimeConfig::default(),
    )
    .expect("bridge_rt");
    let sink_rt = DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes([0x51; 12]),
        RuntimeConfig::default(),
    )
    .expect("sink_rt");

    let (support, type_id) = register_keyed_final(&bridge_rt);
    let (_sink_support, sink_type_id) = register_keyed_final(&sink_rt);

    let writer_eid = bridge_rt
        .register_user_writer_kind(
            UserWriterConfig {
                topic_name: "KeyedFinal".into(),
                type_name: "KeyedFinal".into(),
                type_identifier: type_id,
                ..Default::default()
            },
            true,
        )
        .expect("writer");
    let (reader_eid, rx) = sink_rt
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "KeyedFinal".into(),
                type_name: "KeyedFinal".into(),
                reliable: true,
                type_identifier: sink_type_id,
                ..Default::default()
            },
            true,
        )
        .expect("reader");

    wait_until(
        || bridge_rt.user_writer_matched_count(writer_eid) >= 1,
        "the sink reader to match the bridge writer",
    );
    let _ = reader_eid;
    (bridge_rt, writer_eid, rx, support)
}

#[test]
fn dispose_with_matching_type_forwards_the_serialized_key() {
    let (bridge_rt, writer_eid, sink_rx, support) = setup();
    let sample = support
        .read_body(&[0, 0, 0, 7, 0, 0, 0, 42], 2, true)
        .expect("decode a synthetic id=7 body (PLAIN_CDR2-BE, no encap header)");
    let (key_hash, key_holder) = support.key_material(&sample).expect("keyed");

    let (tx, rx) = mpsc::channel();
    tx.send(UserSample::Lifecycle {
        key_hash,
        kind: zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed,
        key_holder: Some(key_holder.clone()),
        source_timestamp: None,
        group: None,
        writer_guid: [0; 16],
        coherent_set_sn: None,
        source_sequence_number: -1,
    })
    .expect("send synthetic dispose");
    drop(tx); // the pump exits once the sender side is gone and rx drains empty

    let metrics = Arc::new(RouteMetrics::new("kh-same-type"));
    let mut session = ForwardingSession::start(SessionParts {
        route_name: "kh-same-type".into(),
        rx,
        output_rt: Arc::clone(&bridge_rt),
        writer_eid,
        keyed: true,
        same_type: true,
        processor: None,
        metrics: Arc::clone(&metrics),
        preserve_source_timestamp: false,
        max_samples_per_second: None,
        relay: false,
    })
    .expect("session");

    let received = sink_rx
        .recv_timeout(Duration::from_secs(15))
        .expect("the sink reader must observe the forwarded dispose");
    session.stop();

    match received {
        UserSample::Lifecycle {
            key_hash: recv_hash,
            kind,
            key_holder: recv_holder,
            ..
        } => {
            assert_eq!(
                kind,
                zerodds_rtps::history_cache::ChangeKind::NotAliveDisposed
            );
            assert_eq!(recv_hash, key_hash);
            assert_eq!(
                recv_holder,
                Some(key_holder),
                "the sink never saw an ALIVE sample to cache a holder from — this key_holder \
                 can only have been decoded from a real serializedKey on the wire datagram, \
                 proving the K flag and the key reached it"
            );
        }
        other => panic!("expected Lifecycle, got {other:?}"),
    }
}

/// `same_type: false` withholds the input's key holder: it would be bytes
/// of a type the output writer may not have. The output writer then reads
/// the key from the key hash under its own type — a key within 16 octets
/// is the hash (DDSI-RTPS 2.5 §9.6.4.8). The injected holder names instance
/// 10 and the hash instance 9: the sink seeing 9's holder shows the
/// forwarded one was not used and the marker still carries a key.
#[test]
fn dispose_with_same_type_false_withholds_the_forwarded_holder() {
    let (bridge_rt, writer_eid, sink_rx, support) = setup();
    let nine = support
        .read_body(&[0, 0, 0, 9, 0, 0, 0, 1], 2, true)
        .expect("decode a synthetic id=9 body");
    let (key_hash, key_holder) = support.key_material(&nine).expect("keyed");
    let ten = support
        .read_body(&[0, 0, 0, 10, 0, 0, 0, 1], 2, true)
        .expect("decode a synthetic id=10 body");
    let (_, other_holder) = support.key_material(&ten).expect("keyed");

    let (tx, rx) = mpsc::channel();
    tx.send(UserSample::Lifecycle {
        key_hash,
        kind: zerodds_rtps::history_cache::ChangeKind::NotAliveUnregistered,
        key_holder: Some(other_holder),
        source_timestamp: None,
        group: None,
        writer_guid: [0; 16],
        coherent_set_sn: None,
        source_sequence_number: -1,
    })
    .expect("send synthetic unregister");
    drop(tx);

    let metrics = Arc::new(RouteMetrics::new("kh-diff-type"));
    let mut session = ForwardingSession::start(SessionParts {
        route_name: "kh-diff-type".into(),
        rx,
        output_rt: Arc::clone(&bridge_rt),
        writer_eid,
        keyed: true,
        // The gate under test: an unestablished (or known-different) type
        // relationship must never forward the key_holder, even though this
        // call has one on hand.
        same_type: false,
        processor: None,
        metrics: Arc::clone(&metrics),
        preserve_source_timestamp: false,
        max_samples_per_second: None,
        relay: false,
    })
    .expect("session");

    let received = sink_rx
        .recv_timeout(Duration::from_secs(15))
        .expect("the sink reader must observe the forwarded unregister");
    session.stop();

    match received {
        UserSample::Lifecycle {
            key_hash: recv_hash,
            key_holder: recv_holder,
            ..
        } => {
            assert_eq!(recv_hash, key_hash);
            assert_eq!(
                recv_holder,
                Some(key_holder),
                "the key the hash spells, not the forwarded holder of instance 10"
            );
        }
        other => panic!("expected Lifecycle, got {other:?}"),
    }
}
