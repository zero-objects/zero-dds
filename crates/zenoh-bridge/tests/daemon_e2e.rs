// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! In-process end-to-end tests for `zerodds-zenoh-bridged`: a real
//! `DcpsRuntime` producer/consumer (a genuine second DDS participant, not
//! the `test_inject_user_alive` bypass some sibling bridges' e2e tests use
//! to sidestep same-host RTPS discovery) against the daemon's own engine,
//! and an in-process Zenoh session (peer mode, TCP loopback, scouting
//! disabled) on the Zenoh side.
//!
//! # Byte-level wire compatibility
//!
//! `zenoh_payload_matches_what_zenoh_plugin_dds_would_put` checks the
//! Zenoh payload for a plain sample against DDSI-RTPS 2.5 §10.5 Table 10.3:
//! a `ZeroDDS` user writer with no override (this crate's `TopicSpec`
//! default: `Extensibility::Final`) puts `CDR_LE` (`0x0001`) as its
//! representation identifier and `0x0000` as the options, matching
//! `zenoh-plugin-dds`'s own behaviour of publishing the *full*
//! RTPS-encapsulated CDR blob unchanged (`zenoh-plugin-dds/src/dds_mgt.rs`
//! `DDSRawSample::data_as_slice`, cited in `crates/zenoh-bridge/src/mapping.rs`).
//! This is a reading of both sources' documented/measured behaviour, not an
//! independent measurement against a running `zenoh-plugin-dds` — no such
//! process runs in this test.
//!
//! # Platform note
//!
//! Cross-participant DDS discovery over loopback UDP is known to be
//! unreliable on macOS (ETIMEDOUT on a connected loopback UDP socket, a
//! host issue independent of ZeroDDS). If a test here fails only because
//! the DDS-side writer/reader never matched within its timeout, that is
//! this known host issue, not a bridge defect — run these tests on Linux to
//! confirm.

#![cfg(feature = "daemon")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::uninlined_format_args,
    // `SubscriberBuilderExt`/`FetchingSubscriber`: wire-compat with
    // zenoh-plugin-dds — see crates/zenoh-bridge/src/runtime.rs module docs.
    deprecated
)]

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use zenoh::Wait as _;
use zenoh::query::ReplyKeyExpr;
use zenoh::sample::Sample;
use zenoh_ext::SubscriberBuilderExt as _;

use zerodds_dcps::participant::bridge_guid_prefix;
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_qos::DurabilityKind;
use zerodds_rtps::inline_qos::status_info::DISPOSED;
use zerodds_zenoh_bridge::daemon::config::DaemonConfig;
use zerodds_zenoh_bridge::daemon::server;

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

/// A DDS domain per test, leased so that tests running in parallel (one
/// thread per test, and other test binaries on the same host) never see
/// each other's SPDP/SEDP traffic. A counter starting at 210 put the RTPS
/// ports at 59900 and up, inside the kernel's ephemeral port range, where
/// the SPDP bind can collide with any socket that asked for a free port.
/// The lease is held until the test's end.
fn next_domain() -> (zerodds_test_domains::DomainLease, i32) {
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id();
    (lease, domain)
}

/// The TCP port the daemon's Zenoh session listens on.
///
/// The daemon listens on `tcp/127.0.0.1:0`, so the kernel picks the port at
/// bind time and no parallel test can take it first. A port probed and
/// released before the daemon bound it was taken by a parallel test's
/// Zenoh session in 1 of 39 runs on the Linux test host: `daemon start`
/// failed with "Address already in use".
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

fn runtime_config() -> RuntimeConfig {
    RuntimeConfig::default()
}

/// An independent `DcpsRuntime` — a second, genuine DDS participant —
/// shut down when the test lets go of it.
///
/// A runtime's worker threads hold their own `Arc`, so a dropped handle
/// left it running for the rest of the binary, on a domain whose lease the
/// test had already given back. The tests run one after another in the
/// full pipeline (`--test-threads=1`) and the next one leased that domain
/// again: its daemon auto-discovered the previous test's topic
/// (`GuidFallbackKeyedRev`) and `/catalog` never listed only its own. Each
/// test declares its lease first, so this drops before it.
struct DcpsPeer(Arc<DcpsRuntime>);

impl std::ops::Deref for DcpsPeer {
    type Target = Arc<DcpsRuntime>;
    fn deref(&self) -> &Arc<DcpsRuntime> {
        &self.0
    }
}

impl Drop for DcpsPeer {
    fn drop(&mut self) {
        self.0.shutdown();
    }
}

fn start_dcps(domain: i32, seed: &str) -> DcpsPeer {
    let prefix = bridge_guid_prefix(seed);
    DcpsPeer(DcpsRuntime::start(domain, prefix, runtime_config()).expect("dcps runtime start"))
}

/// `UserWriterConfig` has no `Default` impl (by design — see its doc
/// comment in `crates/dcps/src/runtime.rs`); this is the same "every field
/// named" constructor `crates/zenoh-bridge/src/runtime.rs`'s own
/// `wire_reverse` uses.
fn writer_cfg(
    topic: &str,
    type_name: &str,
    reliable: bool,
    durability: DurabilityKind,
) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.to_string(),
        type_name: type_name.to_string(),
        reliable,
        durability,
        ..Default::default()
    }
}

fn reader_cfg(
    topic: &str,
    type_name: &str,
    reliable: bool,
    durability: DurabilityKind,
) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.to_string(),
        type_name: type_name.to_string(),
        reliable,
        durability,
        ..UserReaderConfig::default()
    }
}

/// A Zenoh peer-mode config: TCP loopback only, scouting disabled — so the
/// test's own Zenoh session only ever talks to the one daemon it dials
/// (or is dialed by), never anything else on the host's network.
fn zenoh_test_config(listen_port: Option<u16>, connect_port: Option<u16>) -> zenoh::Config {
    let mut cfg = zenoh::Config::default();
    cfg.scouting
        .multicast
        .set_enabled(Some(false))
        .expect("disable multicast scouting");
    if let Some(port) = listen_port {
        cfg.listen
            .endpoints
            .set(vec![
                format!("tcp/127.0.0.1:{port}")
                    .parse()
                    .expect("listen endpoint"),
            ])
            .expect("set listen");
    }
    if let Some(port) = connect_port {
        cfg.connect
            .endpoints
            .set(vec![
                format!("tcp/127.0.0.1:{port}")
                    .parse()
                    .expect("connect endpoint"),
            ])
            .expect("set connect");
    }
    cfg
}

/// Starts the daemon from a YAML string (peer mode, listening on a TCP
/// loopback port the kernel picks, scouting disabled); see
/// [`daemon_tcp_port`].
fn start_daemon(yaml: &str) -> server::DaemonHandle {
    let cfg = DaemonConfig::load_from_str(yaml).expect("daemon config parses");
    server::start_with_runtime(cfg, runtime_config()).expect("daemon start")
}

/// Polls `f` until it returns `Some`, or panics with `what` (including the
/// macOS-loopback caveat from the module docs) after `timeout`. Used only
/// where `f` itself blocks for a meaningful slice of the budget (a channel
/// `recv_timeout`) — see [`wait_for_match`] for a DDS match wait, which has
/// a real event to block on instead.
fn wait_for<T>(what: &str, timeout: Duration, mut f: impl FnMut() -> Option<T>) -> T {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(v) = f() {
            return v;
        }
        if std::time::Instant::now() >= deadline {
            panic!(
                "timed out after {timeout:?} waiting for: {what} \
                 (if this is the only failure, see the macOS loopback-UDP \
                 caveat in this file's module docs — run on Linux to confirm)"
            );
        }
    }
}

/// Waits for `condition` using `DcpsRuntime::wait_match_event` — a condvar
/// notified on any writer/reader match change on `rt` — instead of sleeping
/// a fixed interval between checks. `wait_match_event` is runtime-wide, not
/// scoped to one entity, so each wake re-checks `condition` itself; a
/// spurious wake (a match event for a different entity) just costs one
/// cheap re-check, not a busy loop, since the next call blocks again until
/// the next event or the remaining timeout.
fn wait_for_match<T>(
    rt: &DcpsRuntime,
    what: &str,
    timeout: Duration,
    mut condition: impl FnMut() -> Option<T>,
) -> T {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(v) = condition() {
            return v;
        }
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            panic!(
                "timed out after {timeout:?} waiting for: {what} \
                 (if this is the only failure, see the macOS loopback-UDP \
                 caveat in this file's module docs — run on Linux to confirm)"
            );
        }
        rt.wait_match_event(remaining);
    }
}

const CDR_LE_HEADER: [u8; 4] = [0x00, 0x01, 0x00, 0x00];

/// Writes `payload` until the Zenoh subscriber receives a sample, and
/// returns that sample.
///
/// A Zenoh `put` for which the publishing session knows no matching
/// subscriber is dropped — Zenoh's own semantics, the same for
/// `zenoh-plugin-dds`. The test's subscriber declaration reaches the
/// daemon's session asynchronously over the TCP link, and a DDS match says
/// nothing about it, so a single write right after the match was lost in
/// about half of the runs on the Linux host. Every write carries the same
/// payload, so whichever arrives is checked in full by the caller.
fn write_until_received(
    producer: &DcpsRuntime,
    writer_eid: zerodds_rtps::wire_types::EntityId,
    payload: &[u8],
    sub: &zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<Sample>>,
    what: &str,
) -> Sample {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        producer
            .write_user_sample_borrowed(writer_eid, payload)
            .expect("write sample");
        if let Ok(Some(sample)) = sub.recv_timeout(Duration::from_millis(300)) {
            return sample;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out after 20 s waiting for: {what}"
        );
    }
}

/// Waits until the daemon's Zenoh session has registered a remote
/// subscriber for `key_expr`, then writes a probe through `write_probe`
/// until the Zenoh subscriber `sub` has received one.
///
/// A DDS match is not enough before a single-shot write: Zenoh delivers a
/// put only to the subscribers the publishing session knows, and the test
/// session's subscriber declaration reaches the daemon asynchronously. A
/// probe that arrives does not show that it has. Until the test session
/// finalizes its initial interest, Zenoh 1.10 routes every put to it
/// (`compute_data_route` in `zenoh/src/net/routing/hat/peer/pubsub.rs`,
/// `has_unfinalized_initial_interest`); between that final and the
/// subscriber declaration it routes none. Measured on the Linux test host
/// with Zenoh tracing: the probe arrived, the daemon received
/// `declare_final` and 1.1 ms later `declare_subscriber`, and the put in
/// between was not delivered. The remote matching status of a publisher on
/// the daemon's session counts registered subscribers only, so it turns
/// true once the declaration has arrived. `Locality::Remote` leaves out the
/// daemon's own subscriber on a bidirectional route.
///
/// The probe then covers the DDS half of the route. Probes still in flight
/// may arrive later, so the checks after this look for the samples they
/// expect, not for a count.
fn wait_for_zenoh_route(
    daemon: &server::DaemonHandle,
    key_expr: &str,
    sub: &zenoh::pubsub::Subscriber<zenoh::handlers::FifoChannelHandler<Sample>>,
    what: &str,
    mut write_probe: impl FnMut(),
) {
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let publisher = daemon
        .bridge()
        .session()
        .declare_publisher(key_expr.to_string())
        .allowed_destination(zenoh::sample::Locality::Remote)
        .wait()
        .expect("declare a publisher for the matching status");
    while !publisher
        .matching_status()
        .wait()
        .expect("matching status")
        .matching()
    {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out after 20 s waiting for the daemon to register the zenoh \
             subscriber of {key_expr} ({what})"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    publisher
        .undeclare()
        .wait()
        .expect("undeclare the matching publisher");
    loop {
        write_probe();
        if let Ok(Some(_)) = sub.recv_timeout(Duration::from_millis(300)) {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out after 20 s waiting for: {what}"
        );
    }
}

/// Whether `puts` holds the forwarded put of instance `id`.
fn has_put(puts: &[Vec<u8>], id: i32) -> bool {
    puts.iter()
        .any(|b| b.as_slice() == [CDR_LE_HEADER.to_vec(), body_of(id).to_vec()].concat())
}

// ---------------------------------------------------------------------------
// DDS -> Zenoh
// ---------------------------------------------------------------------------

/// A real DDS writer, in its own `DcpsRuntime`, publishes; the daemon
/// (its own `DcpsRuntime`, wired to a Zenoh session) forwards it; an
/// in-process Zenoh subscriber (peer mode, TCP loopback, no scouting)
/// receives it.
#[test]
fn dds_writer_reaches_zenoh_subscriber() {
    let (_lease, domain) = next_domain();
    let producer = start_dcps(domain, "e2e-producer-fwd");
    let writer_eid = producer
        .register_user_writer(writer_cfg("Chatter", "T", true, DurabilityKind::Volatile))
        .expect("register writer");

    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: false\n\
         topics:\n  - dds_name: Chatter\n    dds_type: T\n    direction: out\n"
    );
    let mut daemon = start_daemon(&yaml);
    let daemon_port = daemon_tcp_port(&daemon);

    let rt = tokio::runtime::Runtime::new().expect("tokio rt");
    let session = rt
        .block_on(async { zenoh::open(zenoh_test_config(None, Some(daemon_port))).await })
        .expect("zenoh open");
    let sub = rt
        .block_on(async { session.declare_subscriber("Chatter").await })
        .expect("declare subscriber");

    // Give discovery a moment before writing, so the first sample is not
    // lost to a not-yet-matched writer/reader pair — a real late-joiner path
    // (TRANSIENT_LOCAL) is exercised separately below.
    wait_for_match(
        &producer,
        "producer writer matches the daemon's reader",
        Duration::from_secs(20),
        || (producer.user_writer_matched_count(writer_eid) > 0).then_some(()),
    );

    let sample = write_until_received(
        &producer,
        writer_eid,
        b"hello-zenoh",
        &sub,
        "zenoh subscriber receives the forwarded sample",
    );
    let bytes = sample.payload().to_bytes();
    assert_eq!(
        &bytes[..4],
        &CDR_LE_HEADER,
        "unexpected encapsulation header: {bytes:02x?}"
    );
    assert_eq!(&bytes[4..], b"hello-zenoh");

    daemon.shutdown();
}

/// Byte-level check: the encapsulation header ZeroDDS' default writer QoS
/// (Final extensibility, XCDR1, little-endian — see the module docs) puts
/// on the Zenoh payload is exactly `CDR_LE` (DDSI-RTPS 2.5 §10.5 Table
/// 10.3), the same header `zenoh-plugin-dds` would forward unchanged for
/// the same DDS sample.
#[test]
fn zenoh_payload_matches_what_zenoh_plugin_dds_would_put() {
    let (_lease, domain) = next_domain();
    let producer = start_dcps(domain, "e2e-producer-bytes");
    let writer_eid = producer
        .register_user_writer(writer_cfg(
            "BytesTopic",
            "T",
            true,
            DurabilityKind::Volatile,
        ))
        .expect("register writer");

    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: false\n\
         topics:\n  - dds_name: BytesTopic\n    dds_type: T\n    direction: out\n"
    );
    let mut daemon = start_daemon(&yaml);
    let daemon_port = daemon_tcp_port(&daemon);
    let rt = tokio::runtime::Runtime::new().expect("tokio rt");
    let session = rt
        .block_on(async { zenoh::open(zenoh_test_config(None, Some(daemon_port))).await })
        .expect("zenoh open");
    let sub = rt
        .block_on(async { session.declare_subscriber("BytesTopic").await })
        .expect("declare subscriber");
    wait_for_match(&producer, "writer matches", Duration::from_secs(20), || {
        (producer.user_writer_matched_count(writer_eid) > 0).then_some(())
    });

    let payload: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03];
    let sample = write_until_received(&producer, writer_eid, &payload, &sub, "sample arrives");
    let bytes = sample.payload().to_bytes();
    // DDSI-RTPS 2.5 §10.5 Table 10.3: CDR_LE = 0x0001, options = 0x0000.
    let mut expected = CDR_LE_HEADER.to_vec();
    expected.extend_from_slice(&payload);
    assert_eq!(bytes.as_ref(), expected.as_slice());

    daemon.shutdown();
}

/// A 1 MiB payload is forwarded intact — no bridge-level fragmentation
/// limit (`zenoh-plugin-dds` imposes none either; see the module docs).
#[test]
fn one_mebibyte_payload_forwards_intact() {
    let (_lease, domain) = next_domain();
    let producer = start_dcps(domain, "e2e-producer-big");
    let writer_eid = producer
        .register_user_writer(writer_cfg("BigTopic", "T", true, DurabilityKind::Volatile))
        .expect("register writer");

    let yaml = format!(
        "domain: {domain}\nlimits:\n  max_payload_bytes: 2097152\nzenoh:\n  mode: peer\n  \
         listen: [\"tcp/127.0.0.1:0\"]\n  scouting:\n    multicast:\n      enabled: false\n\
         discovery:\n  auto_bridge: false\ntopics:\n  - dds_name: BigTopic\n    dds_type: T\n    direction: out\n"
    );
    let mut daemon = start_daemon(&yaml);
    let daemon_port = daemon_tcp_port(&daemon);
    let rt = tokio::runtime::Runtime::new().expect("tokio rt");
    let session = rt
        .block_on(async { zenoh::open(zenoh_test_config(None, Some(daemon_port))).await })
        .expect("zenoh open");
    let sub = rt
        .block_on(async { session.declare_subscriber("BigTopic").await })
        .expect("declare subscriber");
    wait_for_match(&producer, "writer matches", Duration::from_secs(20), || {
        (producer.user_writer_matched_count(writer_eid) > 0).then_some(())
    });

    let payload: Vec<u8> = (0..1024 * 1024).map(|i| (i % 251) as u8).collect();
    let sample = write_until_received(
        &producer,
        writer_eid,
        &payload,
        &sub,
        "1 MiB sample arrives",
    );
    let bytes = sample.payload().to_bytes();
    assert_eq!(bytes.len(), 4 + payload.len());
    assert_eq!(&bytes[4..], payload.as_slice());

    daemon.shutdown();
}

// ---------------------------------------------------------------------------
// Zenoh -> DDS
// ---------------------------------------------------------------------------

/// A Zenoh `put` (an in-process peer session) reaches a real DDS reader in
/// its own `DcpsRuntime`, through the daemon.
#[test]
fn zenoh_put_reaches_dds_reader() {
    let (_lease, domain) = next_domain();
    let consumer = start_dcps(domain, "e2e-consumer-rev");
    let (reader_eid, rx) = consumer
        .register_user_reader(reader_cfg("Cmd", "T", true, DurabilityKind::Volatile))
        .expect("register reader");
    let _ = reader_eid;

    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: false\n\
         topics:\n  - dds_name: Cmd\n    dds_type: T\n    direction: in\n"
    );
    let mut daemon = start_daemon(&yaml);
    let daemon_port = daemon_tcp_port(&daemon);
    let rt = tokio::runtime::Runtime::new().expect("tokio rt");
    let session = rt
        .block_on(async { zenoh::open(zenoh_test_config(None, Some(daemon_port))).await })
        .expect("zenoh open");

    let mut body = CDR_LE_HEADER.to_vec();
    body.extend_from_slice(b"do-something");
    // The put is repeated until it arrives: a put the daemon's subscriber
    // declaration has not yet reached this session is not delivered (see
    // `wait_for_zenoh_route`).
    let sample = wait_for(
        "dds reader receives the sample",
        Duration::from_secs(20),
        || {
            rt.block_on(async { session.put("Cmd", body.clone()).await })
                .expect("zenoh put");
            rx.recv_timeout(Duration::from_millis(300)).ok()
        },
    );
    match sample {
        UserSample::Alive { payload, .. } => {
            assert_eq!(payload.as_slice(), b"do-something");
        }
        other => panic!("expected an Alive sample, got {other:?}"),
    }

    daemon.shutdown();
}

// ---------------------------------------------------------------------------
// Keyed topic: instances + dispose
// ---------------------------------------------------------------------------

/// Builds a `CompleteTypeObject` for `@final struct <name> { @key long id; }`
/// and writes it to a temp file, returning the path. Mirrors the pattern
/// `crates/dcps/src/serialized_type.rs`'s own test uses to build a minimal
/// keyed type without an IDL toolchain step.
fn build_keyed_type_object(struct_name: &str) -> zerodds_types::TypeObject {
    let idl = format!("@final struct {struct_name} {{ @key long id; }};");
    let ast = std::sync::Arc::new(
        zerodds_wire_probe::encode::model_from_idl(&idl, "e2e-keyed.idl").expect("model_from_idl"),
    );
    let type_ = zerodds_types::dynamic::DynamicType::find(&ast, struct_name).expect("find type");
    zerodds_types::TypeObject::Complete(
        zerodds_types::from_ast::complete_type_object(&ast, type_.id())
            .expect("complete_type_object"),
    )
}

fn write_keyed_type_object(struct_name: &str) -> std::path::PathBuf {
    let object = build_keyed_type_object(struct_name);
    let bytes = object.to_bytes_le().expect("encode type object");
    let path = std::env::temp_dir().join(format!(
        "zerodds-zenoh-bridge-e2e-{struct_name}-{}.typeobject",
        std::process::id()
    ));
    std::fs::write(&path, bytes).expect("write type object file");
    path
}

/// The DDS key hash for `@final struct { @key long id; }`: the 4-byte
/// big-endian encoding of `id`, zero-padded to 16 bytes (verified against
/// `crates/dcps/src/serialized_type.rs`'s own test:
/// `assert_eq!(&hash[..4], holder.as_slice())` for this exact type shape).
fn key_hash_of(id: i32) -> [u8; 16] {
    let mut h = [0u8; 16];
    h[..4].copy_from_slice(&id.to_be_bytes());
    h
}

/// The XCDR2 little-endian plain-CDR body of `{ id }` for this type: just
/// the 4-byte little-endian `i32` (a single `Final` `long` field has no
/// framing of its own).
fn body_of(id: i32) -> [u8; 4] {
    id.to_le_bytes()
}

/// DDS instances + dispose, both directions: two DDS-side writes (two
/// instances) plus a dispose reach Zenoh as two `put`s and one `delete` on
/// `<key-expr>/<key-hash-hex>` (see `crates/zenoh-bridge/src/runtime.rs`
/// module docs — `zenoh-plugin-dds` itself does not forward dispose at
/// all); a Zenoh `put` plus a `delete` on that same suffixed key reach a
/// real DDS reader as an alive sample for the right instance and a dispose
/// of the other.
#[test]
fn keyed_topic_instances_and_dispose_both_directions() {
    let type_path = write_keyed_type_object("Keyed");
    let (_lease, domain) = next_domain();
    let producer = start_dcps(domain, "e2e-keyed-producer");
    let consumer = start_dcps(domain, "e2e-keyed-consumer");

    let writer_eid = producer
        .register_user_writer_kind(
            writer_cfg("KeyedTopic", "Keyed", true, DurabilityKind::Volatile),
            true,
        )
        .expect("register keyed writer");
    let (reader_eid, dds_rx) = consumer
        .register_user_reader_kind(
            reader_cfg("KeyedTopic", "Keyed", true, DurabilityKind::Volatile),
            true,
        )
        .expect("register keyed reader");
    let _ = reader_eid;

    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: false\n\
         topics:\n  - dds_name: KeyedTopic\n    dds_type: Keyed\n    direction: bidir\n    \
         keyed: true\n    type_file: \"{}\"\n",
        type_path.display()
    );
    let mut daemon = start_daemon(&yaml);
    let daemon_port = daemon_tcp_port(&daemon);
    let rt = tokio::runtime::Runtime::new().expect("tokio rt");
    let session = rt
        .block_on(async { zenoh::open(zenoh_test_config(None, Some(daemon_port))).await })
        .expect("zenoh open");
    let sub = rt
        .block_on(async { session.declare_subscriber("KeyedTopic/**").await })
        .expect("declare subscriber");
    // Two distinct readers must match before a sample is safe to write: the
    // consumer's own (directly SEDP-matched) reader, and the daemon's
    // explicitly-configured bridging reader. Both are Volatile, so waiting
    // only for "any" match races ahead of whichever of the two matches
    // second — usually the daemon's, since its reader is registered only
    // once `start_daemon` has parsed the YAML and stood up the route.
    wait_for_match(
        &producer,
        "keyed writer matched by both the consumer and the daemon",
        Duration::from_secs(20),
        || (producer.user_writer_matched_count(writer_eid) >= 2).then_some(()),
    );

    wait_for_zenoh_route(
        &daemon,
        "KeyedTopic",
        &sub,
        "a probe instance reaches the zenoh subscriber",
        || {
            producer
                .write_user_sample_keyed(writer_eid, &body_of(0), key_hash_of(0), None)
                .expect("write probe instance 0");
        },
    );

    // DDS -> Zenoh: two instances, then dispose instance 1.
    producer
        .write_user_sample_keyed(writer_eid, &body_of(1), key_hash_of(1), None)
        .expect("write instance 1");
    producer
        .write_user_sample_keyed(writer_eid, &body_of(2), key_hash_of(2), None)
        .expect("write instance 2");
    producer
        .write_user_lifecycle(writer_eid, key_hash_of(1), None, DISPOSED, None)
        .expect("dispose instance 1");

    let mut puts = Vec::new();
    let mut delete_key = None;
    wait_for(
        "zenoh sees the puts of instances 1 and 2 and the delete for the keyed topic",
        Duration::from_secs(20),
        || {
            while let Ok(Some(s)) = sub.recv_timeout(Duration::from_millis(50)) {
                if s.kind() == zenoh::sample::SampleKind::Delete {
                    delete_key = Some(s.key_expr().to_string());
                } else {
                    puts.push(s.payload().to_bytes().to_vec());
                }
            }
            (has_put(&puts, 1) && has_put(&puts, 2) && delete_key.is_some()).then_some(())
        },
    );
    assert!(
        puts.iter()
            .any(|b| b.as_slice() == [CDR_LE_HEADER.to_vec(), body_of(1).to_vec()].concat()),
        "missing put for instance 1: {puts:02x?}"
    );
    assert!(
        puts.iter()
            .any(|b| b.as_slice() == [CDR_LE_HEADER.to_vec(), body_of(2).to_vec()].concat()),
        "missing put for instance 2: {puts:02x?}"
    );
    let expected_suffix = key_hash_of(1)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert!(
        delete_key.unwrap().ends_with(&expected_suffix),
        "dispose delete did not name instance 1's key hash"
    );

    // Zenoh -> DDS: put a new instance (id=3), then dispose instance 2 via a
    // suffixed delete.
    let mut put3 = CDR_LE_HEADER.to_vec();
    put3.extend_from_slice(&body_of(3));
    rt.block_on(async { session.put("KeyedTopic", put3).await })
        .expect("put instance 3");
    let hash2_hex = key_hash_of(2)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    rt.block_on(async { session.delete(format!("KeyedTopic/{hash2_hex}")).await })
        .expect("delete instance 2");

    let mut saw_alive_3 = false;
    let mut saw_dispose_2 = false;
    wait_for(
        "dds reader sees instance 3 alive and instance 2 disposed",
        Duration::from_secs(20),
        || {
            while let Ok(s) = dds_rx.recv_timeout(Duration::from_millis(50)) {
                match s {
                    UserSample::Alive { payload, .. } if payload.as_slice() == body_of(3) => {
                        saw_alive_3 = true;
                    }
                    UserSample::Lifecycle { key_hash, .. } if key_hash == key_hash_of(2) => {
                        saw_dispose_2 = true;
                    }
                    _ => {}
                }
            }
            (saw_alive_3 && saw_dispose_2).then_some(())
        },
    );

    daemon.shutdown();
    let _ = std::fs::remove_file(&type_path);
}

// ---------------------------------------------------------------------------
// TRANSIENT_LOCAL late joiner
// ---------------------------------------------------------------------------

/// A DDS `TRANSIENT_LOCAL` writer publishes before any Zenoh subscriber
/// exists; a Zenoh subscriber declared afterwards (a `FetchingSubscriber`
/// querying `<key>/@dds_pub_cache/*`, the late-joiner mechanism this
/// crate's engine wires for a `TRANSIENT_LOCAL` topic — see
/// `crates/zenoh-bridge/src/runtime.rs`) still receives the retained
/// sample, via its initial fetch against the daemon's `PublicationCache`.
#[test]
fn transient_local_late_joiner_gets_the_retained_sample() {
    let (_lease, domain) = next_domain();
    let producer = start_dcps(domain, "e2e-tl-producer");
    let writer_eid = producer
        .register_user_writer(writer_cfg(
            "Retained",
            "T",
            true,
            DurabilityKind::TransientLocal,
        ))
        .expect("register writer");

    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: false\n\
         topics:\n  - dds_name: Retained\n    dds_type: T\n    direction: out\n    \
         qos:\n      durability: transient_local\n"
    );
    let mut daemon = start_daemon(&yaml);
    let daemon_port = daemon_tcp_port(&daemon);

    let rt = tokio::runtime::Runtime::new().expect("tokio rt");
    wait_for_match(
        &producer,
        "writer matches the daemon's reader",
        Duration::from_secs(20),
        || (producer.user_writer_matched_count(writer_eid) > 0).then_some(()),
    );
    producer
        .write_user_sample_borrowed(writer_eid, b"retained-value")
        .expect("write retained sample");
    // Give the daemon's PublicationCache a moment to observe its own put
    // (it locally subscribes to the same key) before the late joiner asks.
    std::thread::sleep(Duration::from_millis(500));

    let session = rt
        .block_on(async { zenoh::open(zenoh_test_config(None, Some(daemon_port))).await })
        .expect("zenoh open (late joiner)");
    let late = rt
        .block_on(async {
            session
                .declare_subscriber("Retained")
                .querying()
                .query_timeout(Duration::from_secs(5))
                .query_selector("Retained/@dds_pub_cache/*")
                .query_accept_replies(ReplyKeyExpr::Any)
                .await
        })
        .expect("declare late FetchingSubscriber");

    let sample = wait_for(
        "late joiner receives the retained sample via its initial fetch",
        Duration::from_secs(20),
        || late.recv_timeout(Duration::from_millis(200)).ok().flatten(),
    );
    let bytes = sample.payload().to_bytes();
    assert_eq!(&bytes[4..], b"retained-value");

    daemon.shutdown();
}

// ---------------------------------------------------------------------------
// Type resolution for auto-discovered topics
// ---------------------------------------------------------------------------

/// An auto-discovered (not configured) keyed topic whose type is registered
/// in the peer's own `DcpsRuntime` (so `DcpsRuntime`'s automatic
/// `getTypes` exchange — `crates/dcps/src/runtime.rs`'s
/// `send_type_lookup_request` call sites — fills the daemon's own
/// `type_lookup_server.registry`) is bridged keyed, with real key material
/// in both directions: DDS instances + dispose reach Zenoh with the correct
/// key-hash suffix, and a Zenoh put reaches a real (`WithKey`) DDS reader
/// with the key hash `SerializedTypeSupport` derived from the body — not
/// the single default instance a `keyed: false` route would collapse every
/// sample onto (`crates/zenoh-bridge/src/daemon/server.rs`
/// `resolve_discovered_type`/`resolve_or_defer`).
#[test]
fn auto_discovered_keyed_topic_with_registered_type_bridges_keyed() {
    let (_lease, domain) = next_domain();
    let producer = start_dcps(domain, "e2e-autokeyed-producer");
    let consumer = start_dcps(domain, "e2e-autokeyed-consumer");

    // Register the type on BOTH sides: the producer so its writer announces
    // a resolvable `EquivalenceHashComplete` identity in PID_TYPE_INFORMATION,
    // the consumer so its own (separately auto-discovered, Zenoh->DDS)
    // keyed reader can compute a dispose's KeyHash the same way a real
    // application's reader would — this test is about the *bridge's* type
    // resolution, not the consumer's.
    let type_object = build_keyed_type_object("AutoKeyed");
    let producer_hash = producer
        .register_type_object(type_object.clone())
        .expect("register type on producer");
    let _consumer_hash = consumer
        .register_type_object(type_object)
        .expect("register type on consumer");

    let writer_eid = producer
        .register_user_writer_kind(
            UserWriterConfig {
                type_identifier: zerodds_types::TypeIdentifier::EquivalenceHashComplete(
                    producer_hash,
                ),
                ..writer_cfg("AutoKeyed", "AutoKeyed", true, DurabilityKind::Volatile)
            },
            true,
        )
        .expect("register keyed writer");
    let (_reader_eid, dds_rx) = consumer
        .register_user_reader_kind(
            reader_cfg("AutoKeyed", "AutoKeyed", true, DurabilityKind::Volatile),
            true,
        )
        .expect("register keyed reader");

    // No `topics:` entry — auto_bridge (default true) must discover and
    // resolve this topic on its own. A short poll interval keeps the test
    // fast; the topic waits for the getTypes round trip
    // (`resolve_or_defer`).
    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: true\n  \
         poll_interval_ms: 100\n"
    );
    let mut daemon = start_daemon(&yaml);
    let daemon_port = daemon_tcp_port(&daemon);
    let session = zenoh::open(zenoh_test_config(None, Some(daemon_port)))
        .wait()
        .expect("zenoh open");
    let sub = session
        .declare_subscriber("AutoKeyed/**")
        .wait()
        .expect("declare subscriber");
    // Two distinct readers must match the producer's writer before any
    // sample is safe to write: the consumer's own (directly SEDP-matched)
    // reader, and the daemon's separately auto-discovered bridging reader.
    // Both readers are Volatile, so a reader that matches *after* a sample
    // was written never sees it — waiting only for "any" match (count > 0)
    // is satisfied by the consumer's fast direct match alone and races
    // ahead of the daemon's own (slower: getTypes + resolve) bridging
    // reader, which is the bug this test exists to catch.
    wait_for_match(
        &producer,
        "keyed writer auto-discovered and matched by both the consumer and the daemon",
        Duration::from_secs(20),
        || (producer.user_writer_matched_count(writer_eid) >= 2).then_some(()),
    );

    wait_for_zenoh_route(
        &daemon,
        "AutoKeyed",
        &sub,
        "a probe instance reaches the zenoh subscriber",
        || {
            producer
                .write_user_sample_keyed(writer_eid, &body_of(0), key_hash_of(0), None)
                .expect("write probe instance 0");
        },
    );

    // DDS -> Zenoh: two instances, then dispose instance 1 (same shape as
    // the explicitly-configured keyed_topic_instances_and_dispose_both_directions).
    producer
        .write_user_sample_keyed(writer_eid, &body_of(1), key_hash_of(1), None)
        .expect("write instance 1");
    producer
        .write_user_sample_keyed(writer_eid, &body_of(2), key_hash_of(2), None)
        .expect("write instance 2");
    producer
        .write_user_lifecycle(writer_eid, key_hash_of(1), None, DISPOSED, None)
        .expect("dispose instance 1");

    let mut puts = Vec::new();
    let mut delete_key = None;
    wait_for(
        "zenoh sees the puts of instances 1 and 2 and the delete for the auto-discovered keyed topic",
        Duration::from_secs(20),
        || {
            while let Ok(Some(s)) = sub.recv_timeout(Duration::from_millis(50)) {
                if s.kind() == zenoh::sample::SampleKind::Delete {
                    delete_key = Some(s.key_expr().to_string());
                } else {
                    puts.push(s.payload().to_bytes().to_vec());
                }
            }
            (has_put(&puts, 1) && has_put(&puts, 2) && delete_key.is_some()).then_some(())
        },
    );
    assert!(
        puts.iter()
            .any(|b| b.as_slice() == [CDR_LE_HEADER.to_vec(), body_of(1).to_vec()].concat()),
        "missing put for instance 1: {puts:02x?}"
    );
    assert!(
        puts.iter()
            .any(|b| b.as_slice() == [CDR_LE_HEADER.to_vec(), body_of(2).to_vec()].concat()),
        "missing put for instance 2: {puts:02x?}"
    );
    let expected_suffix = key_hash_of(1)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert!(
        delete_key.unwrap().ends_with(&expected_suffix),
        "dispose delete did not name instance 1's key hash"
    );

    // Zenoh -> DDS: put instance 3. Only a route that actually resolved the
    // type derives the real per-instance key hash via `SerializedTypeSupport`
    // (`crate::runtime::reverse_deliver`); a `keyed: false` fallback would
    // write it as the single default (all-zero) instance instead.
    let mut put3 = CDR_LE_HEADER.to_vec();
    put3.extend_from_slice(&body_of(3));
    session
        .put("AutoKeyed", put3)
        .wait()
        .expect("put instance 3");

    let alive = wait_for(
        "dds reader sees instance 3 with its real key hash",
        Duration::from_secs(20),
        || match dds_rx.recv_timeout(Duration::from_millis(200)) {
            Ok(UserSample::Alive {
                payload, key_hash, ..
            }) if payload.as_slice() == body_of(3) => Some(key_hash),
            _ => None,
        },
    );
    assert_eq!(
        alive,
        Some(key_hash_of(3)),
        "instance 3's key hash was not derived from the type — the route fell back to keyless"
    );

    daemon.shutdown();
}

/// A keyed topic whose announced `EquivalenceHashComplete` its participant
/// cannot answer for yet is not bridged without its type: it waits, the
/// daemon asks again (`resolve_or_defer`, every `TYPE_LOOKUP_RETRY`), and
/// once the producer has the type the topic is bridged with it — a Zenoh put
/// reaches the DDS reader with the key hash the type derives. Before, the
/// topic was bridged after one wait of 1 s and stayed without its type.
#[test]
fn a_keyed_topic_whose_type_arrives_late_is_bridged_with_it() {
    let (_lease, domain) = next_domain();
    let producer = start_dcps(domain, "e2e-latetype-producer");
    let consumer = start_dcps(domain, "e2e-latetype-consumer");
    let type_object = build_keyed_type_object("LateKeyed");
    let hash = zerodds_types::compute_hash(&type_object).expect("type hash");
    consumer
        .register_type_object(type_object.clone())
        .expect("register type on consumer");
    producer
        .register_user_writer_kind(
            UserWriterConfig {
                type_identifier: zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash),
                ..writer_cfg("LateKeyed", "LateKeyed", true, DurabilityKind::Volatile)
            },
            true,
        )
        .expect("register keyed writer");
    let (_reader_eid, dds_rx) = consumer
        .register_user_reader_kind(
            reader_cfg("LateKeyed", "LateKeyed", true, DurabilityKind::Volatile),
            true,
        )
        .expect("register keyed reader");

    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: true\n  \
         poll_interval_ms: 100\n"
    );
    let mut daemon = start_daemon(&yaml);
    let daemon_port = daemon_tcp_port(&daemon);
    // The daemon sees the writer and asks for its type; the producer has
    // none to give yet.
    std::thread::sleep(Duration::from_secs(2));
    assert!(
        daemon.bridge().map().by_topic("LateKeyed").is_none(),
        "the topic was bridged before its type resolved"
    );
    producer
        .register_type_object(type_object)
        .expect("register type on producer");
    let entry = wait_for_map_entry(
        &daemon,
        "LateKeyed",
        "the topic bridged once its type resolves",
        |_| true,
    );
    assert!(entry.keyed, "the topic was bridged keyless");

    let session = zenoh::open(zenoh_test_config(None, Some(daemon_port)))
        .wait()
        .expect("zenoh open");
    let alive = wait_for(
        "the dds reader sees instance 3 put on zenoh",
        Duration::from_secs(20),
        || {
            let mut put3 = CDR_LE_HEADER.to_vec();
            put3.extend_from_slice(&body_of(3));
            session
                .put("LateKeyed", put3)
                .wait()
                .expect("put instance 3");
            match dds_rx.recv_timeout(Duration::from_millis(200)) {
                Ok(UserSample::Alive {
                    payload, key_hash, ..
                }) if payload.as_slice() == body_of(3) => Some(key_hash),
                _ => None,
            }
        },
    );
    assert_eq!(
        alive,
        Some(key_hash_of(3)),
        "instance 3's key hash was not derived from the type"
    );
    daemon.shutdown();
}

/// A keyed topic whose announced type never resolves is bridged after
/// `TYPE_RESOLUTION_DEADLINE` (10 s), keyed by its endpoint GUID's entity
/// kind (DDSI-RTPS 2.5 §9.3.1.2 Table 9.1): the wait has an end.
#[test]
fn a_keyed_topic_whose_type_never_resolves_is_bridged_after_the_deadline() {
    let (_lease, domain) = next_domain();
    let producer = start_dcps(domain, "e2e-notype-producer");
    let hash =
        zerodds_types::compute_hash(&build_keyed_type_object("NeverKeyed")).expect("type hash");
    producer
        .register_user_writer_kind(
            UserWriterConfig {
                type_identifier: zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash),
                ..writer_cfg("NeverKeyed", "NeverKeyed", true, DurabilityKind::Volatile)
            },
            true,
        )
        .expect("register keyed writer");

    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: true\n  \
         poll_interval_ms: 100\n"
    );
    let started = std::time::Instant::now();
    let mut daemon = start_daemon(&yaml);
    let entry = wait_for(
        "the topic bridged after the deadline",
        Duration::from_secs(20),
        || {
            std::thread::sleep(Duration::from_millis(50));
            daemon.bridge().map().by_topic("NeverKeyed").cloned()
        },
    );
    assert!(
        started.elapsed() >= Duration::from_secs(9),
        "bridged after {:?}, before the deadline",
        started.elapsed()
    );
    assert!(entry.keyed, "not bridged keyed by the entity kind");
    daemon.shutdown();
}

/// An auto-discovered topic whose type does not resolve (here: no
/// `TypeIdentifier` at all, the same as a DDS writer that never announces
/// `PID_TYPE_INFORMATION` — nothing for `resolve_discovered_type` to look
/// up) is still bridged, keyless, as before; its lifecycle
/// (dispose/unregister) samples are still forwarded — never dropped — but
/// counted separately (`BridgeStats::lifecycle_type_unresolved`) instead of
/// blending into ordinary traffic.
#[test]
fn auto_discovered_topic_with_unresolvable_type_counts_lifecycle_samples() {
    zenoh::try_init_log_from_env();
    let (_lease, domain) = next_domain();
    let producer = start_dcps(domain, "e2e-unresolved-producer");

    // A keyless writer (`TypeIdentifier::None`, the `writer_cfg` default) —
    // deliberately not keyed, so there is no entity-kind mismatch to
    // confound the result: this test is about the *type resolution*
    // fallback, not about entity-kind matching. A keyless topic's single
    // implicit instance can still be disposed (DDS 1.4 §2.2.2.4.2.6).
    let writer_eid = producer
        .register_user_writer(writer_cfg(
            "UnresolvedType",
            "UnresolvedType",
            true,
            DurabilityKind::Volatile,
        ))
        .expect("register writer");

    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: true\n  \
         poll_interval_ms: 100\n"
    );
    let mut daemon = start_daemon(&yaml);
    let daemon_port = daemon_tcp_port(&daemon);
    let session = zenoh::open(zenoh_test_config(None, Some(daemon_port)))
        .wait()
        .expect("zenoh open");
    eprintln!(
        "[test] unresolved-type subscriber session zid={}",
        session.zid()
    );
    let sub = session
        .declare_subscriber("UnresolvedType/**")
        .wait()
        .expect("declare subscriber");
    wait_for_match(
        &producer,
        "writer auto-discovered and matched",
        Duration::from_secs(20),
        || (producer.user_writer_matched_count(writer_eid) > 0).then_some(()),
    );

    let mut probes_written = 0u64;
    wait_for_zenoh_route(
        &daemon,
        "UnresolvedType",
        &sub,
        "a probe sample reaches the zenoh subscriber",
        || {
            producer
                .write_user_sample_borrowed(writer_eid, b"probe")
                .expect("write probe sample");
            probes_written += 1;
        },
    );
    let forwarded_after_route = daemon.bridge().stats().dds_to_zenoh.load(Ordering::Relaxed);
    producer
        .write_user_sample_borrowed(writer_eid, b"alive-before-dispose")
        .expect("write alive sample");
    producer
        .write_user_lifecycle(writer_eid, [0u8; 16], None, DISPOSED, None)
        .expect("dispose the implicit instance");

    // Never dropped: both the alive sample and the dispose still cross to
    // Zenoh. On a timeout the message says which of the two is missing and
    // what the bridge counted, so a failing run shows on which side of the
    // Zenoh put the sample was lost.
    let mut saw_alive = false;
    let mut saw_delete = false;
    let mut probes_late = 0u64;
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while !(saw_alive && saw_delete) {
        while let Ok(Some(s)) = sub.recv_timeout(Duration::from_millis(50)) {
            if s.kind() == zenoh::sample::SampleKind::Delete {
                saw_delete = true;
            } else if s.payload().to_bytes().ends_with(b"alive-before-dispose") {
                saw_alive = true;
            } else {
                probes_late += 1;
            }
        }
        if std::time::Instant::now() >= deadline {
            let stats = daemon.bridge().stats();
            panic!(
                "timed out after 20 s waiting for zenoh to see the alive sample and the \
                 dispose: alive seen {saw_alive}, delete seen {saw_delete}; probes \
                 written {probes_written}, received after the first {probes_late}; bridge \
                 dds_to_zenoh={} (={forwarded_after_route} when the route stood) \
                 forward_errors={} lifecycle_type_unresolved={}",
                stats.dds_to_zenoh.load(Ordering::Relaxed),
                stats.forward_errors.load(Ordering::Relaxed),
                stats.lifecycle_type_unresolved.load(Ordering::Relaxed),
            );
        }
    }

    // Counted, not blended into ordinary DDS->Zenoh traffic.
    let unresolved = daemon
        .bridge()
        .stats()
        .lifecycle_type_unresolved
        .load(Ordering::Relaxed);
    assert_eq!(
        unresolved, 1,
        "the dispose on the unresolved-type route was not counted in \
         lifecycle_type_unresolved (BridgeStats)"
    );

    daemon.shutdown();
}

// ---------------------------------------------------------------------------
// Keyed topic, type never resolves: GUID entity-kind fallback (the measured
// Cyclone DDS 11.0.1 bug — see crates/zenoh-bridge/src/daemon/server.rs
// `note_discovery`/`topic_kind_from_guid`).
// ---------------------------------------------------------------------------

/// A raw HTTP GET against the daemon's admin endpoint, blocking up to 2 s.
/// Mirrors `daemon/runtime_common.rs`'s own `admin_endpoint_serves_metrics`
/// unit test.
fn http_get(admin_addr: &str, path: &str) -> String {
    use std::io::{Read as _, Write as _};
    let mut s = std::net::TcpStream::connect(admin_addr).expect("connect to daemon admin endpoint");
    s.set_read_timeout(Some(Duration::from_secs(2))).ok();
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").as_bytes())
        .expect("write request");
    let mut body = String::new();
    s.read_to_string(&mut body).ok();
    body
}

/// A keyed writer that never announces `PID_TYPE_INFORMATION`
/// (`TypeIdentifier::None`, the `writer_cfg` default) — the same wire
/// shape measured against Cyclone DDS 11.0.1 on domain 151/152: DATA(w)
/// carried `ENDPOINT_GUID`/`PROTOCOL_VERSION`/`RELIABILITY`/
/// `RESOURCE_LIMIT`/`TOPIC_NAME`/`TYPE_NAME`/`VENDOR_ID`/`0x800c`, no
/// `PID_TYPE_INFORMATION`, so nothing for `resolve_discovered_type` to
/// look up — `note_discovery` must fall back to the GUID's own entity-kind
/// octet (DDSI-RTPS 2.5 §9.3.1.2 Table 9.1) instead of defaulting to
/// keyless.
///
/// This checks the fallback directly, by reading the daemon's own
/// auto-discovered reader's *announced* GUID back through the producer's
/// discovery cache — not "did a sample arrive": ZeroDDS's own SEDP matcher
/// (`crates/discovery/src/sedp/stack.rs`) does not itself check entity-kind
/// compatibility the way Cyclone's does (checked: no such condition in that
/// file), so an all-ZeroDDS test would still deliver samples even with the
/// keyless-fallback bug reintroduced — only the announced entity kind tells
/// the two cases apart here. The live cross-vendor check (does Cyclone
/// actually match and deliver) is the Linux-host measurement this task
/// started from, not something this in-process test can reproduce.
#[test]
fn auto_discovered_keyed_topic_with_no_type_information_announces_keyed_reader() {
    let (_lease, domain) = next_domain();
    let producer = start_dcps(domain, "e2e-guidfallback-producer");
    let writer_eid = producer
        .register_user_writer_kind(
            writer_cfg(
                "GuidFallbackKeyed",
                "GuidFallbackKeyed",
                true,
                DurabilityKind::Volatile,
            ),
            true,
        )
        .expect("register keyed writer");

    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: true\n  \
         poll_interval_ms: 100\nmetrics:\n  enabled: true\n  listen: \"127.0.0.1:0\"\n"
    );
    let mut daemon = start_daemon(&yaml);
    let daemon_port = daemon_tcp_port(&daemon);

    wait_for_match(
        &producer,
        "keyed writer auto-discovered and matched by the daemon's reader",
        Duration::from_secs(20),
        || (producer.user_writer_matched_count(writer_eid) > 0).then_some(()),
    );

    // The regression this guards: the daemon's auto-discovered reader
    // announces entity kind WITH_KEY (0x07), never NO_KEY (0x04) — a
    // 0x04 reader is what a keyless-fallback bridge would register, and no
    // spec-conformant vendor's 0x02 writer ever matches one (DDSI-RTPS 2.5
    // §8.4.1), which is the exact 0-received failure this task measured.
    let daemon_reader = wait_for(
        "producer discovers the daemon's bridging reader",
        Duration::from_secs(20),
        || {
            producer
                .discovered_subscription_endpoints()
                .into_iter()
                .find(|e| e.topic_name == "GuidFallbackKeyed")
        },
    );
    assert_eq!(
        zerodds_rtps::wire_types::EntityKind::from_byte(daemon_reader.endpoint_guid[15]),
        zerodds_rtps::wire_types::EntityKind::UserReaderWithKey,
        "the daemon's auto-discovered reader announced NO_KEY for a keyed topic \
         whose type never resolved — the exact bug measured against Cyclone DDS 11.0.1"
    );

    // The happy path still works end to end (confirms the fix does not
    // merely flip a flag while breaking delivery).
    let rt = tokio::runtime::Runtime::new().expect("tokio rt");
    let session = rt
        .block_on(async { zenoh::open(zenoh_test_config(None, Some(daemon_port))).await })
        .expect("zenoh open");
    let sub = rt
        .block_on(async { session.declare_subscriber("GuidFallbackKeyed/**").await })
        .expect("declare subscriber");
    let sample = write_until_received(
        &producer,
        writer_eid,
        &body_of(9),
        &sub,
        "zenoh receives the keyed, unresolved-type sample",
    );
    let bytes = sample.payload().to_bytes();
    assert_eq!(
        bytes.as_ref(),
        [CDR_LE_HEADER.to_vec(), body_of(9).to_vec()]
            .concat()
            .as_slice()
    );

    // /catalog and zerodds_zenoh_topics_active list the auto-bridged topic
    // and nothing else — before this fix, `/catalog` reported
    // `"topics":[]` for a topic the daemon had already logged as
    // auto-bridged (config-only snapshot, never refreshed from the live
    // topic map).
    let admin_addr = daemon.admin_addr.clone().expect("admin endpoint enabled");
    wait_for(
        "/catalog lists GuidFallbackKeyed, keyed, and nothing else",
        Duration::from_secs(20),
        || {
            let body = http_get(&admin_addr, "/catalog");
            (body.matches("\"dds_name\":").count() == 1
                && body.contains("\"dds_name\":\"GuidFallbackKeyed\"")
                && body.contains("\"keyed\":true"))
            .then_some(())
        },
    );
    let metrics_body = http_get(&admin_addr, "/metrics");
    let topics_active: i64 = metrics_body
        .lines()
        .find_map(|l| {
            l.strip_prefix("zerodds_zenoh_topics_active ")?
                .trim()
                .parse()
                .ok()
        })
        .expect("zerodds_zenoh_topics_active line present");
    assert_eq!(
        topics_active, 1,
        "zerodds_zenoh_topics_active must count the one auto-bridged topic: {metrics_body}"
    );

    daemon.shutdown();
}

/// The reverse direction: a keyed ZeroDDS reader that never announces
/// `PID_TYPE_INFORMATION` is auto-discovered `ZenohToDds`. Before this fix,
/// `ZenohBridge::add_topic` refused any keyed Zenoh->DDS route without a
/// resolved `type_support` outright (`BridgeError::Dds`, logged by
/// `note_discovery`'s `Err(e)` branch and otherwise silently not bridged —
/// no writer, no Zenoh subscriber, so the `put` below would vanish with no
/// receiver). It writes without `PID_KEY_HASH` — spec-conformant per
/// DDSI-RTPS 2.5 §8.7.10, see `crate::runtime::ZenohBridge::wire_reverse`'s
/// doc comment — and the real DDS reader still receives it: a KeyHash is
/// only a lookup hint for the *reader's* own instance bookkeeping, not a
/// precondition for delivery.
#[test]
fn auto_discovered_keyed_reader_with_no_type_information_receives_from_zenoh() {
    let (_lease, domain) = next_domain();
    let consumer = start_dcps(domain, "e2e-guidfallback-consumer");
    let (_reader_eid, dds_rx) = consumer
        .register_user_reader_kind(
            reader_cfg(
                "GuidFallbackKeyedRev",
                "GuidFallbackKeyedRev",
                true,
                DurabilityKind::Volatile,
            ),
            true,
        )
        .expect("register keyed reader");

    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: true\n  \
         poll_interval_ms: 100\n"
    );
    let mut daemon = start_daemon(&yaml);
    let daemon_port = daemon_tcp_port(&daemon);
    let session = zenoh::open(zenoh_test_config(None, Some(daemon_port)))
        .wait()
        .expect("zenoh open");

    // Give the discovery loop a few poll ticks to see the reader and wire
    // its own (keyed, per the fix) writer + Zenoh subscriber before the
    // put — there is no DDS match event to wait on here, only a Zenoh
    // subscriber declaration on the other end.
    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    let mut put_ok = false;
    let mut body = CDR_LE_HEADER.to_vec();
    body.extend_from_slice(&body_of(11));
    while std::time::Instant::now() < deadline {
        if session
            .put("GuidFallbackKeyedRev", body.clone())
            .wait()
            .is_ok()
        {
            put_ok = true;
        }
        if let Ok(sample) = dds_rx.recv_timeout(Duration::from_millis(300)) {
            match sample {
                UserSample::Alive { payload, .. } => {
                    assert_eq!(payload.as_slice(), body_of(11));
                    daemon.shutdown();
                    return;
                }
                other => panic!("expected an Alive sample, got {other:?}"),
            }
        }
    }
    panic!(
        "timed out after 20 s waiting for: dds reader receives the sample \
         forwarded with no PID_KEY_HASH (put succeeded at least once: {put_ok})"
    );
}

// ---------------------------------------------------------------------------
// Discovery timing and widening
// ---------------------------------------------------------------------------

/// Waits until the daemon's topic map has an entry for `topic` that
/// `accept` takes. `ZenohBridge::add_topic` enters a topic after wiring it,
/// so a DDS match on the wired endpoint can come first; the map has no
/// event to wait on.
fn wait_for_map_entry(
    daemon: &server::DaemonHandle,
    topic: &str,
    what: &str,
    accept: impl Fn(&zerodds_zenoh_bridge::TopicMapEntry) -> bool,
) -> zerodds_zenoh_bridge::TopicMapEntry {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(e) = daemon.bridge().map().by_topic(topic).filter(|e| accept(e)) {
            return e.clone();
        }
        assert!(
            std::time::Instant::now() < deadline,
            "timed out after 10 s waiting for: {what}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// An auto-discovered topic is bridged when the SEDP datagram announcing
/// its writer arrives, not at the next `poll_interval_ms` tick: with the
/// interval at 30 s the daemon's bridging reader still matches the
/// producer's writer within seconds, the `getTypes` round trip for the
/// writer's `EquivalenceHashComplete` identity included. Measured before the
/// change: `zenoh-bridge-dds` 1.10.1 bridged a Cyclone ShapeDemo writer's
/// first sample, this daemon (1000 ms interval) began about one second
/// later.
#[test]
fn an_auto_discovered_topic_is_bridged_on_discovery_not_on_the_poll_interval() {
    let (_lease, domain) = next_domain();
    let producer = start_dcps(domain, "e2e-eventdisc-producer");
    let type_object = build_keyed_type_object("EventDiscovered");
    let hash = producer
        .register_type_object(type_object)
        .expect("register type on producer");
    let writer_eid = producer
        .register_user_writer_kind(
            UserWriterConfig {
                type_identifier: zerodds_types::TypeIdentifier::EquivalenceHashComplete(hash),
                ..writer_cfg(
                    "EventDiscovered",
                    "EventDiscovered",
                    true,
                    DurabilityKind::Volatile,
                )
            },
            true,
        )
        .expect("register keyed writer");

    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: true\n  \
         poll_interval_ms: 30000\n"
    );
    let mut daemon = start_daemon(&yaml);
    wait_for_match(
        &producer,
        "the daemon's bridging reader matched within 5 s of start, 30 s poll interval",
        Duration::from_secs(5),
        || (producer.user_writer_matched_count(writer_eid) >= 1).then_some(()),
    );
    let entry = wait_for_map_entry(&daemon, "EventDiscovered", "the topic in the map", |_| true);
    assert!(
        entry.keyed,
        "the writer's type did not resolve before bridging"
    );

    // The discovery thread waits on the runtime, not on a sleep: shutting
    // down does not sit out the 30 s interval.
    let started = std::time::Instant::now();
    daemon.shutdown();
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "shutdown took {:?}",
        started.elapsed()
    );
    // No reader of the topic exists but the daemon's own: it must not have
    // bridged that as the topic's subscription side.
    let after = daemon
        .bridge()
        .map()
        .by_topic("EventDiscovered")
        .cloned()
        .expect("topic in the map");
    assert!(
        !after.zenoh_to_dds,
        "the daemon bridged its own reader: {after:?}"
    );
}

/// A topic bridged DDS → Zenoh because its writer was discovered first gets
/// the Zenoh → DDS direction when a reader of it appears later, without
/// touching the first direction — the route from DDS and the route to DDS
/// are independent, as in `zenoh-plugin-dds`.
#[test]
fn a_topic_bridged_one_way_gets_the_other_direction_when_its_reader_appears() {
    let (_lease, domain) = next_domain();
    let producer = start_dcps(domain, "e2e-widen-producer");
    let writer_eid = producer
        .register_user_writer(writer_cfg("Widen", "T", true, DurabilityKind::Volatile))
        .expect("register writer");

    let yaml = format!(
        "domain: {domain}\nzenoh:\n  mode: peer\n  listen: [\"tcp/127.0.0.1:0\"]\n  \
         scouting:\n    multicast:\n      enabled: false\ndiscovery:\n  auto_bridge: true\n  \
         poll_interval_ms: 100\n"
    );
    let mut daemon = start_daemon(&yaml);
    let daemon_port = daemon_tcp_port(&daemon);
    wait_for_match(
        &producer,
        "the daemon's bridging reader matched the producer's writer",
        Duration::from_secs(10),
        || (producer.user_writer_matched_count(writer_eid) >= 1).then_some(()),
    );
    let first = wait_for_map_entry(&daemon, "Widen", "the topic in the map", |_| true);
    assert!(first.dds_to_zenoh && !first.zenoh_to_dds, "{first:?}");

    // Only now a reader of the topic: the daemon must add its writer.
    let consumer = start_dcps(domain, "e2e-widen-consumer");
    let (reader_eid, dds_rx) = consumer
        .register_user_reader(reader_cfg("Widen", "T", true, DurabilityKind::Volatile))
        .expect("register reader");
    wait_for_match(
        &consumer,
        "the consumer's reader matched the producer's writer and the daemon's writer",
        Duration::from_secs(10),
        || (consumer.user_reader_matched_count(reader_eid) >= 2).then_some(()),
    );
    let widened = wait_for_map_entry(
        &daemon,
        "Widen",
        "the Zenoh -> DDS direction in the map",
        |e| e.zenoh_to_dds,
    );
    assert!(widened.dds_to_zenoh && widened.zenoh_to_dds, "{widened:?}");
    assert_eq!(
        daemon.bridge().forward_thread_count(),
        1,
        "the DDS -> Zenoh direction was wired a second time"
    );

    let session = zenoh::open(zenoh_test_config(None, Some(daemon_port)))
        .wait()
        .expect("zenoh open");
    let mut body = CDR_LE_HEADER.to_vec();
    body.extend_from_slice(&[0x57, 0x69, 0x64, 0x65]);
    let got = wait_for(
        "the consumer's reader receives a Zenoh put through the added direction",
        Duration::from_secs(20),
        || {
            let _ = session.put("Widen", body.clone()).wait();
            match dds_rx.recv_timeout(Duration::from_millis(300)) {
                Ok(UserSample::Alive { payload, .. }) => Some(payload.to_vec()),
                _ => None,
            }
        },
    );
    assert_eq!(got, [0x57, 0x69, 0x64, 0x65]);
    daemon.shutdown();
}
