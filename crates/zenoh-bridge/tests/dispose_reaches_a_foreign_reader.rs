// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! A Zenoh `delete` bridged to DDS, read by a DDS reader of another vendor
//! (D10).
//!
//! A Zenoh `delete` names its instance by the key hash in its key
//! expression alone. The route's reverse writer used to have no type — it
//! is registered without a TypeIdentifier, so peers match it by name — and
//! its dispose went out with `PID_KEY_HASH` alone, which Cyclone 11.0.1 and
//! OpenDDS 3.35 do not act on (Fast DDS 3.6.1 and OpenSplice 6.9 dispose an
//! instance they know by the hash, measured behind the routing service in
//! `crates/routing-service/tests/dispose_reaches_a_foreign_reader.rs`).
//! The route's `type_file`
//! now reads the instance keys: the writer takes the key of an instance it
//! wrote, of one the forward pump carried to Zenoh, or the one a key hash
//! within 16 octets spells out, and the dispose carries the `serializedKey`.
//!
//! On one domain: the daemon with a keyed `bidir` route of `KeyedPointA`, a
//! ZeroDDS writer that writes instance 20 directly in DDS, a ZeroDDS reader
//! that confirms the bridge's own writes, and a reader of another vendor.
//! Zenoh puts instances 7 and 8 and deletes 7 and 20; the vendor reader
//! reports 7 and 20 disposed and 8 alive.
//!
//! Live against another vendor, so `#[ignore]`: the Cyclone reader needs
//! python3 with the cyclonedds binding, the other test the reader
//! `ZERODDS_DISPOSE_READER` names (`tools/dispose-reader/build.sh`).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    missing_docs
)]

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use zenoh::Wait as _;

use zerodds_dcps::participant::bridge_guid_prefix;
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_test_vendor_reader::{
    VendorReader, keyed_point_a, keyed_point_a_key_hash, keyed_point_a_type_object, reader_command,
};
use zerodds_zenoh_bridge::daemon::config::DaemonConfig;
use zerodds_zenoh_bridge::daemon::server;

const CDR_LE_HEADER: [u8; 4] = [0x00, 0x01, 0x00, 0x00];

/// The TCP port the daemon's Zenoh session listens on. The daemon listens
/// on `tcp/127.0.0.1:0`, so the kernel picks the port at bind time; a port
/// probed and released first can be taken by another process before the
/// daemon binds it (see `daemon_tcp_port` in `daemon_e2e.rs`).
fn daemon_tcp_port(daemon: &server::DaemonHandle) -> u16 {
    let locators = daemon.bridge().session().info().locators().wait();
    locators
        .iter()
        .find_map(|l| {
            let s = l.to_string();
            s.strip_prefix("tcp/127.0.0.1:")
                .and_then(|rest| rest.split('?').next())
                .and_then(|port| port.parse().ok())
        })
        .unwrap_or_else(|| {
            panic!("the daemon's zenoh session listens on no tcp/127.0.0.1 port: {locators:?}")
        })
}

fn zenoh_client_config(connect_port: u16) -> zenoh::Config {
    let mut cfg = zenoh::Config::default();
    cfg.scouting
        .multicast
        .set_enabled(Some(false))
        .expect("disable multicast scouting");
    cfg.connect
        .endpoints
        .set(vec![
            format!("tcp/127.0.0.1:{connect_port}")
                .parse()
                .expect("connect endpoint"),
        ])
        .expect("set connect");
    cfg
}

/// Shuts its runtime down when the test lets go of it.
struct Peer(std::sync::Arc<DcpsRuntime>);

impl std::ops::Deref for Peer {
    type Target = std::sync::Arc<DcpsRuntime>;
    fn deref(&self) -> &std::sync::Arc<DcpsRuntime> {
        &self.0
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn bridge_and_delete(which: &str) -> BTreeMap<String, String> {
    let lease = zerodds_test_vendor_reader::lease_domain();
    let domain = lease.id();
    let type_object = keyed_point_a_type_object();
    let type_path = std::env::temp_dir().join(format!(
        "zerodds-zenoh-keyed-point-a-{}.typeobject",
        std::process::id()
    ));
    std::fs::write(&type_path, &type_object).unwrap();
    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: false\n\
         topics:\n  - dds_name: ZenPoint\n    dds_type: KeyedPointA\n    direction: bidir\n    \
         keyed: true\n    type_file: \"{}\"\n",
        type_path.display()
    );
    let mut daemon = server::start_with_runtime(
        DaemonConfig::load_from_str(&yaml).expect("daemon config"),
        RuntimeConfig::default(),
    )
    .expect("daemon start");
    let daemon_port = daemon_tcp_port(&daemon);

    let (command, label) = reader_command(which);
    let mut reader = VendorReader::start(command, &label, domain, "ZenPoint");

    let object = zerodds_types::type_object::TypeObject::from_bytes_le(&type_object).unwrap();
    let hash = zerodds_types::compute_hash(&object).unwrap();
    let producer = Peer(
        DcpsRuntime::start(
            domain,
            bridge_guid_prefix("dispose-foreign-producer"),
            RuntimeConfig::default(),
        )
        .unwrap(),
    );
    producer.register_type_object(object).unwrap();
    let writer = producer
        .register_user_writer_kind(
            UserWriterConfig {
                topic_name: "ZenPoint".into(),
                type_name: "KeyedPointA".into(),
                type_identifier: zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash),
                ..Default::default()
            },
            true,
        )
        .unwrap();
    let consumer = Peer(
        DcpsRuntime::start(
            domain,
            bridge_guid_prefix("dispose-foreign-consumer"),
            RuntimeConfig::default(),
        )
        .unwrap(),
    );
    let (_reader_eid, alive_rx) = consumer
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "ZenPoint".into(),
                type_name: "KeyedPointA".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::Volatile,
                ..UserReaderConfig::default()
            },
            true,
        )
        .unwrap();

    // The producer's writer is matched by the consumer, the daemon's
    // forward reader and the vendor reader.
    let deadline = Instant::now() + Duration::from_secs(20);
    while producer.user_writer_matched_count(writer) < 3 {
        assert!(
            Instant::now() < deadline,
            "the producer matched {} of 3 readers",
            producer.user_writer_matched_count(writer)
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    reader.wait_matched();

    let rt = tokio::runtime::Runtime::new().expect("tokio");
    let session = rt
        .block_on(async { zenoh::open(zenoh_client_config(daemon_port)).await })
        .expect("zenoh open");
    // Zenoh -> DDS: instances 7 and 8, put until the consumer has both and
    // the vendor reader has them and instance 20. A reader reports a match
    // once it knows a writer, and a sample the bridge's writer sends before
    // it knows the reader is lost (VOLATILE).
    let mut seen = std::collections::BTreeSet::new();
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        for id in [7, 8] {
            let mut put = CDR_LE_HEADER.to_vec();
            put.extend_from_slice(&keyed_point_a(id, 100 + id));
            rt.block_on(async { session.put("ZenPoint", put).await })
                .expect("zenoh put");
        }
        producer
            .write_user_sample_keyed(
                writer,
                &keyed_point_a(20, 120),
                keyed_point_a_key_hash(20),
                None,
            )
            .unwrap();
        while let Ok(sample) = alive_rx.recv_timeout(Duration::from_millis(200)) {
            if let UserSample::Alive { payload, .. } = sample {
                let body = payload.as_slice();
                if body.len() >= 4 {
                    seen.insert(i32::from_le_bytes(body[..4].try_into().unwrap()));
                }
            }
        }
        if seen.contains(&7)
            && seen.contains(&8)
            && reader.wait_seen(&[7, 8, 20], Duration::from_secs(1))
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "instances 7, 8 and 20 did not reach the consumer ({seen:?}) and the {label} reader"
        );
    }
    for id in [7, 20] {
        let key = format!("ZenPoint/{}", hex(&keyed_point_a_key_hash(id)));
        rt.block_on(async { session.delete(key).await })
            .expect("zenoh delete");
    }
    std::thread::sleep(Duration::from_millis(1500));
    let states = reader.snapshot();
    daemon.shutdown();
    let _ = reader.finish();
    let _ = std::fs::remove_file(&type_path);
    println!("{label}: {states:?}");
    states
}

fn assert_bridged_states(label: &str, states: &BTreeMap<String, String>) {
    for (id, want) in [
        ("7", "NOT_ALIVE_DISPOSED"),
        ("8", "ALIVE"),
        ("20", "NOT_ALIVE_DISPOSED"),
    ] {
        assert_eq!(
            states.get(id).map(String::as_str),
            Some(want),
            "instance {id} at the {label} reader (all: {states:?})"
        );
    }
}

#[test]
#[ignore = "live against Eclipse Cyclone DDS: needs python3 with the cyclonedds binding; run with --ignored"]
fn a_zenoh_delete_reaches_a_cyclone_reader() {
    let states = bridge_and_delete("cyclone");
    assert_bridged_states("cyclone", &states);
}

#[test]
#[ignore = "live against another vendor: set ZERODDS_DISPOSE_READER (tools/dispose-reader/build.sh); run with --ignored"]
fn a_zenoh_delete_reaches_a_foreign_reader() {
    let states = bridge_and_delete("env");
    assert_bridged_states("foreign", &states);
}
