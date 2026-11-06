// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The service replays a sample under the representation identifier it came
//! in (DDSI-RTPS 2.5 §10.5: the identifier names the format and the byte
//! order of the serialized body; DDS-XTypes 1.3 §7.6.3.1.2 Table 60), from
//! its pump and, after a restart, from the store.
//!
//! A writer sends the vendors' octets of the interop corpus
//! (`tests/interop-corpus/expected-bytes.tsv`, `struct Sample { long a;
//! double b; short c; }`, a = -1234567, b = pi, c = -2) under four headers,
//! one topic each, and goes away; a late joiner takes what the service
//! replays. The service is then stopped and started again on the same
//! database, and a second late joiner takes what it primed from the store.
//! Before, the service kept only the XCDR version and the byte order and
//! replayed a `D_CDR2` or `PL_CDR2` body under `CDR2` (OPEN-ITEMS 60).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_durability_service::DurabilityService;
use zerodds_durability_store::DurabilityStore;
use zerodds_durability_store_sqlite::SqliteStore;
use zerodds_qos::policies::durability::DurabilityKind;
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_types::qos::ExtensibilityForRepr;

const TYPE: &str = "zerodds::RawBytes";

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// One topic: the writer's framing and what it sends.
struct Case {
    topic: &'static str,
    representation: i16,
    extensibility: ExtensibilityForRepr,
    big_endian: bool,
    identifier: [u8; 2],
    octets: Vec<u8>,
}

fn cases() -> Vec<Case> {
    vec![
        // struct-appendable, XCDR2 big endian.
        Case {
            topic: "DelimitedBe",
            representation: 2,
            extensibility: ExtensibilityForRepr::Appendable,
            big_endian: true,
            identifier: [0x00, 0x08],
            octets: hex("0000000effed2979400921fb54442d18fffe"),
        },
        // struct-appendable, XCDR2 little endian.
        Case {
            topic: "DelimitedLe",
            representation: 2,
            extensibility: ExtensibilityForRepr::Appendable,
            big_endian: false,
            identifier: [0x00, 0x09],
            octets: hex("0e0000007929edff182d4454fb210940feff"),
        },
        // struct-mutable, XCDR2 big endian.
        Case {
            topic: "ParameterList",
            representation: 2,
            extensibility: ExtensibilityForRepr::Mutable,
            big_endian: true,
            identifier: [0x00, 0x0a],
            octets: hex("0000001a20000000ffed297930000001400921fb54442d1810000002fffe"),
        },
        // struct-final, XCDR1 big endian.
        Case {
            topic: "Plain",
            representation: 0,
            extensibility: ExtensibilityForRepr::Final,
            big_endian: true,
            identifier: [0x00, 0x00],
            octets: hex("ffed297900000000400921fb54442d18fffe"),
        },
    ]
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !done() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A late joiner on `domain`: what it takes on each case's topic, as
/// (identifier, octets).
fn late_joiner(domain: i32, prefix: u8) -> Vec<([u8; 2], Vec<u8>)> {
    let rt = DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes([prefix; 12]),
        RuntimeConfig::default(),
    )
    .unwrap();
    let mut got = Vec::new();
    for case in cases() {
        let (_reader, rx) = rt
            .register_user_reader(UserReaderConfig {
                topic_name: case.topic.into(),
                type_name: TYPE.into(),
                reliable: true,
                durability: DurabilityKind::TransientLocal,
                data_representation_offer: Some(vec![0, 2]),
                ..Default::default()
            })
            .unwrap();
        match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(UserSample::Alive {
                payload,
                representation_identifier,
                ..
            }) => got.push((representation_identifier, payload.as_ref().to_vec())),
            other => panic!("{}: {other:?}", case.topic),
        }
    }
    rt.shutdown();
    got
}

#[test]
fn rtps_10_5_a_replayed_sample_keeps_the_representation_identifier_it_came_in() {
    let domain = zerodds_test_domains::unique_domain();
    let db = common::tmp_db("encapsulation");
    let want: Vec<([u8; 2], Vec<u8>)> = cases()
        .into_iter()
        .map(|c| (c.identifier, c.octets))
        .collect();

    let store: Arc<dyn DurabilityStore> =
        Arc::new(SqliteStore::open(&db, common::keep_all()).unwrap());
    let service = DurabilityService::start(domain, Arc::clone(&store)).unwrap();
    for case in cases() {
        service
            .serve_typed(case.topic, TYPE, false, common::keep_all())
            .unwrap();
    }
    let alice = DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes([0xA1; 12]),
        RuntimeConfig::default(),
    )
    .unwrap();
    for case in cases() {
        let writer = alice
            .register_user_writer(UserWriterConfig {
                topic_name: case.topic.into(),
                type_name: TYPE.into(),
                reliable: true,
                data_representation_offer: Some(vec![case.representation]),
                ..Default::default()
            })
            .unwrap();
        alice
            .set_user_writer_wire_extensibility(writer, case.extensibility)
            .unwrap();
        alice
            .set_user_writer_byte_order_override(writer, case.big_endian)
            .unwrap();
        wait_until("the service's reader did not match", || {
            alice.user_writer_matched_count(writer) >= 1
        });
        alice
            .write_user_sample(writer, case.octets.clone())
            .unwrap();
        wait_until("the service did not store the sample", || {
            store.stats(case.topic).map(|s| s.samples).unwrap_or(0) == 1
        });
    }
    alice.shutdown();

    assert_eq!(
        late_joiner(domain, 0xC3),
        want,
        "replayed by the pump, under the header each sample came with"
    );
    service.shutdown();
    drop(store);

    let store: Arc<dyn DurabilityStore> =
        Arc::new(SqliteStore::open(&db, common::keep_all()).unwrap());
    let service = DurabilityService::start(domain, Arc::clone(&store)).unwrap();
    for case in cases() {
        assert!(service.serve_if_stored(case.topic, None).unwrap());
    }
    assert_eq!(
        late_joiner(domain, 0xD4),
        want,
        "primed from the store after a restart, under the same headers"
    );
    service.shutdown();
    let _ = std::fs::remove_file(&db);
    let _ = std::fs::remove_file(format!("{}-wal", db.display()));
    let _ = std::fs::remove_file(format!("{}-shm", db.display()));
}
