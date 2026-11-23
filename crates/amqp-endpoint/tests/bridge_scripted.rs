// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon (`zerodds-amqp-bridge-daemon-1.0`) against a broker
//! side this test drives frame by frame (`support/scripted_broker.rs`): the
//! fields of the daemon's open and attaches (§4.1, §4.2), one session per
//! link, the credit it grants (§4.4), the settle mode `second` (§3,
//! AMQP 1.0 Part 2 §2.8.3), the message of a DDS dispose (§4.3), and what
//! goes out again after a dropped connection or an ended session (§9.3),
//! the HISTORY of the daemon's DDS writer (§6).
//! The DDS side is a participant of this process on a domain of its own.
//!
//! Oracles: AMQP 1.0 Part 2 §2.6.7 (link credit), §2.6.12 (settlement),
//! §2.7.1 to §2.7.6 (the fields of open, begin, attach, flow, transfer,
//! disposition), §2.8.3 (receiver settle mode); the daemon spec §4.1 to
//! §4.4, §6, §9.3; DDS 1.4 §2.2.3.18 (HISTORY KEEP_ALL) and §2.2.3.4
//! (DURABILITY TRANSIENT_LOCAL); the message-id of DDS-AMQP 1.0 §8.2 (writer GUID followed
//! by the sequence number).

#![cfg(all(feature = "std", unix))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

#[path = "support/scripted_broker.rs"]
mod scripted_broker;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use scripted_broker::{Broker, Frame, Link, accepted, descriptor_of};
use zerodds_amqp_bridge::extended_types::AmqpExtValue;
use zerodds_amqp_bridge::performatives::descriptor;
use zerodds_amqp_bridge::sections::MessageSection;
use zerodds_amqp_endpoint::bridge_daemon::{
    self, DaemonConfig, DaemonError, OutSample, decode_message, encode_message,
};
use zerodds_amqp_endpoint::coexistence::CoexistenceConfig;
use zerodds_amqp_endpoint::daemon_runtime::BridgeMetrics;
use zerodds_amqp_endpoint::properties::DdsOperation;
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_monitor::Registry;
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

thread_local! {
    static DOMAIN: zerodds_test_domains::DomainLease = zerodds_test_vendor_reader::lease_domain();
}

fn domain() -> i32 {
    DOMAIN.with(zerodds_test_domains::DomainLease::id)
}

const STEP: Duration = Duration::from_secs(10);

/// Waits until `cond` holds, at most [`STEP`]; `what` names it in the
/// panic.
fn wait_for(what: &str, cond: impl Fn() -> bool) {
    let deadline = Instant::now() + STEP;
    while !cond() {
        assert!(Instant::now() < deadline, "{what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn config(yaml: &str) -> DaemonConfig {
    let mut c = zerodds_config::Config::default();
    c.push(
        zerodds_config::parse_str(yaml, "<test>").unwrap(),
        "<test>".into(),
    )
    .unwrap();
    DaemonConfig::from_config(Some(&c)).unwrap()
}

/// The daemon's configuration towards `broker`: `amqp` lines below the
/// broker URL, then `rest` (topics, catalog).
fn yaml(broker: &Broker, amqp: &str, rest: &str) -> String {
    format!(
        "domain: {}\namqp:\n  broker_url: \"{}\"\n  container_id: \"zd-scripted\"\n  reconnect:\n    initial_delay_ms: 100\n    max_delay_ms: 400\n{amqp}{rest}",
        domain(),
        broker.url()
    )
}

/// The daemon in a thread until the flag is set.
struct Daemon {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<Result<(), DaemonError>>>,
}

impl Daemon {
    fn start(cfg: DaemonConfig) -> Self {
        Self::start_with(cfg, BridgeMetrics::register(&Registry::new()))
    }

    /// [`Daemon::start`] counting into `metrics`.
    fn start_with(cfg: DaemonConfig, metrics: BridgeMetrics) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let s = Arc::clone(&stop);
        let join = std::thread::spawn(move || {
            bridge_daemon::run(
                &cfg,
                RuntimeConfig::default(),
                &s,
                &metrics,
                false,
                Duration::from_secs(5),
            )
        });
        Self {
            stop,
            join: Some(join),
        }
    }

    fn stop(mut self) -> Result<(), DaemonError> {
        self.stop.store(true, Ordering::SeqCst);
        self.join.take().unwrap().join().unwrap()
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn participant() -> Arc<DcpsRuntime> {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[4..8].copy_from_slice(&NEXT.fetch_add(1, Ordering::Relaxed).to_be_bytes());
    prefix[11] = 0x5b;
    DcpsRuntime::start(
        domain(),
        GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .unwrap()
}

/// A writer of the test on `topic`, matched with the daemon's reader.
fn matched_writer(dds: &DcpsRuntime, topic: &str, keyed: bool) -> EntityId {
    let eid = dds
        .register_user_writer_kind(
            UserWriterConfig {
                topic_name: topic.into(),
                type_name: topic.into(),
                reliable: true,
                ..Default::default()
            },
            keyed,
        )
        .unwrap();
    let deadline = Instant::now() + STEP;
    while dds.user_writer_matched_count(eid) == 0 {
        assert!(
            Instant::now() < deadline,
            "the daemon's reader did not match"
        );
        dds.wait_match_event(Duration::from_millis(50));
    }
    eid
}

/// A message of another bridge for topic `t` of `cfg`.
fn message(cfg: &DaemonConfig, payload: &[u8], key: Option<[u8; 16]>, op: DdsOperation) -> Vec<u8> {
    encode_message(
        &cfg.topics[0],
        domain(),
        &CoexistenceConfig::new("99999999-2222-4333-8444-555555555555".into()),
        &OutSample {
            payload,
            representation_identifier: [0x00, 0x07],
            key_hash: key,
            writer_guid: [0x42; 16],
            sequence: 1,
            source_timestamp_ns: None,
            operation: op,
            lifespan_ms: None,
            partitions: &[],
        },
    )
    .unwrap()
}

/// The CDR payload of a transfer of the daemon (behind the encapsulation
/// header).
fn payload_of(transfer: &Frame) -> Vec<u8> {
    decode_message(&transfer.payload).unwrap().body[4..].to_vec()
}

/// The message-id of a transfer of the daemon.
fn message_id_of(transfer: &Frame) -> Vec<u8> {
    let mut rest = &transfer.payload[..];
    while !rest.is_empty() {
        let (s, n) = MessageSection::decode(rest).unwrap();
        rest = &rest[n..];
        if let MessageSection::Properties(AmqpExtValue::List(f)) = s {
            match f.first() {
                Some(AmqpExtValue::Binary(b)) => return b.clone(),
                other => panic!("message-id {other:?}"),
            }
        }
    }
    panic!("no properties")
}

fn symbols(v: &AmqpExtValue) -> Vec<String> {
    match v {
        AmqpExtValue::Array(a) => a
            .iter()
            .map(|s| match s {
                AmqpExtValue::Symbol(s) => s.clone(),
                other => panic!("{other:?}"),
            })
            .collect(),
        other => panic!("{other:?}"),
    }
}

fn the_link<'a>(links: &'a [Link], name: &str) -> &'a Link {
    links
        .iter()
        .find(|l| l.name == name)
        .unwrap_or_else(|| panic!("no link {name}: {links:?}"))
}

/// §4.1: the open of the daemon carries the values of its configuration,
/// the broker host as `hostname` when none is configured, the capability
/// `AMQP_DDS_BRIDGE` in both lists and the two bridge properties.
#[test]
fn amqpbridge_4_1_the_daemon_opens_with_its_container_id_and_bridge_properties() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "  max_frame_size: 4096\n  channel_max: 7\n  idle_time_out_ms: 20000\n",
        "catalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg);
    let mut conn = broker.accept(STEP);
    let open = conn.handshake();
    assert_eq!(open.text(0).as_deref(), Some("zd-scripted"), "container-id");
    assert_eq!(open.text(1).as_deref(), Some("127.0.0.1"), "hostname");
    assert_eq!(open.field(2), &AmqpExtValue::Uint(4096), "max-frame-size");
    assert_eq!(open.field(3), &AmqpExtValue::Ushort(7), "channel-max");
    assert_eq!(open.field(4), &AmqpExtValue::Uint(20_000), "idle-time-out");
    assert_eq!(symbols(open.field(7)), ["AMQP_DDS_BRIDGE"]);
    assert_eq!(symbols(open.field(8)), ["AMQP_DDS_BRIDGE"]);
    let AmqpExtValue::Map(props) = open.field(9) else {
        panic!("{:?}", open.field(9))
    };
    assert_eq!(
        props,
        &vec![
            (
                AmqpExtValue::Symbol("zerodds_version".into()),
                AmqpExtValue::Str("1.0".into())
            ),
            (
                AmqpExtValue::Symbol("zerodds_role".into()),
                AmqpExtValue::Str("bridge".into())
            ),
        ]
    );
    drop(conn);
    let _ = daemon.stop();
}

/// §4.2: a bidirectional topic and the catalog are three links, each on a
/// session of its own: three begins on three channels, one attach on each,
/// named `<container-id>/<topic>/<role>`.
#[test]
fn amqpbridge_4_2_every_link_has_a_session_of_its_own_channel() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S42\n    amqp_address: /queues/s42\ncatalog:\n  address: /queues/cat\n",
    ));
    let daemon = Daemon::start(cfg);
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let mut begins = Vec::new();
    let mut attaches: Vec<Frame> = Vec::new();
    while attaches.len() < 3 {
        let f = conn.next();
        match f.code {
            descriptor::BEGIN => {
                conn.answer_begin(&f);
                begins.push(f.channel);
            }
            descriptor::ATTACH => {
                assert!(
                    begins.contains(&f.channel),
                    "an attach on channel {} before its begin",
                    f.channel
                );
                conn.answer_attach(&f, 0);
                attaches.push(f);
            }
            _ => {}
        }
    }
    begins.sort_unstable();
    begins.dedup();
    assert_eq!(begins.len(), 3, "three sessions: {begins:?}");
    let mut channels: Vec<u16> = attaches.iter().map(|a| a.channel).collect();
    channels.sort_unstable();
    assert_eq!(channels, begins, "one attach per session");
    let mut names: Vec<String> = attaches.iter().map(|a| a.text(0).unwrap()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "zd-scripted/S42/receiver",
            "zd-scripted/S42/sender",
            "zd-scripted/catalog/sender"
        ]
    );
    drop(conn);
    let _ = daemon.stop();
}

/// §4.4: the receiving link grants `receiver.credit` and grants it again
/// when half of it is used (Part 2 §2.6.7: the flow carries the delivery
/// count the daemon counted). Credit 4: no flow after the first transfer,
/// a flow of 4 with delivery count 2 after the second.
#[test]
fn amqpbridge_4_4_credit_is_topped_up_at_half() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S44\n    amqp_address: /queues/s44\n    direction: in\n    receiver:\n      credit: 4\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg.clone());
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let links = conn.setup_links(1, 0);
    let link = &links[0];
    assert!(!link.daemon_sends);
    let flow = conn.next_of(descriptor::FLOW);
    assert_eq!(flow.uint(4), Some(link.handle));
    assert_eq!((flow.uint(5), flow.uint(6)), (Some(0), Some(4)));
    let m = message(&cfg, &[1, 0, 0, 0], None, DdsOperation::Write);
    conn.transfer(link, 0, false, &m);
    let d = conn.next();
    assert_eq!(d.code, descriptor::DISPOSITION, "{d:?}");
    assert_eq!(
        (d.boolean(0), d.uint(1), d.boolean(3)),
        (Some(true), Some(0), Some(true))
    );
    assert_eq!(descriptor_of(d.field(4)), Some(0x24), "accepted");
    conn.transfer(link, 1, false, &m);
    let d = conn.next();
    assert_eq!(
        (d.code, d.uint(1)),
        (descriptor::DISPOSITION, Some(1)),
        "{d:?}"
    );
    let flow = conn.next();
    assert_eq!(flow.code, descriptor::FLOW, "{flow:?}");
    assert_eq!((flow.uint(5), flow.uint(6)), (Some(2), Some(4)));
    drop(conn);
    let _ = daemon.stop();
}

/// §3 `sender.rcv_settle_mode: 1` (Part 2 §2.8.3, `second`), the daemon
/// sending: the broker answers the attach with mode second and its outcome
/// unsettled; the daemon settles the delivery (role sender, settled).
#[test]
fn amqpbridge_3_mode_second_settles_after_the_outcome_as_sender() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S3S\n    amqp_address: /queues/s3s\n    direction: out\n    sender:\n      rcv_settle_mode: 1\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg);
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let links = conn.setup_links(1, 1);
    let link = links[0].clone();
    assert_eq!(
        link.attach.field(4),
        &AmqpExtValue::Ubyte(1),
        "rcv-settle-mode second"
    );
    conn.flow(&link, 0, 10);
    let w = matched_writer(&dds, "S3S", false);
    dds.write_user_sample_borrowed(w, &[7, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    assert_eq!(t.boolean(4), Some(false), "unsettled");
    let id = t.uint(1).unwrap();
    conn.disposition(link.channel, true, id, id, false, accepted());
    let settle = conn.next_of(descriptor::DISPOSITION);
    assert_eq!(settle.boolean(0), Some(false), "role sender");
    assert_eq!((settle.uint(1), settle.uint(2)), (Some(id), Some(id)));
    assert_eq!(settle.boolean(3), Some(true), "settled");
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// §3 `sender.rcv_settle_mode: 1` on a receiving link: the daemon, the
/// receiver, sends its outcome unsettled and keeps the delivery until the
/// broker settles it; until then it takes up credit (credit 2: no new
/// credit while two deliveries wait, credit 2 once the broker settled).
#[test]
fn amqpbridge_3_mode_second_settles_after_the_outcome_as_receiver() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S3R\n    amqp_address: /queues/s3r\n    direction: in\n    sender:\n      rcv_settle_mode: 1\n    receiver:\n      credit: 2\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg.clone());
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let links = conn.setup_links(1, 1);
    let link = links[0].clone();
    assert_eq!(link.attach.field(4), &AmqpExtValue::Ubyte(1));
    let flow = conn.next_of(descriptor::FLOW);
    assert_eq!(flow.uint(6), Some(2));
    let m = message(&cfg, &[1, 0, 0, 0], None, DdsOperation::Write);
    for id in 0..2 {
        conn.transfer(&link, id, false, &m);
        let d = conn.next();
        assert_eq!(d.code, descriptor::DISPOSITION, "{d:?}");
        assert_eq!(
            (d.uint(1), d.boolean(3)),
            (Some(id), Some(false)),
            "unsettled"
        );
        assert_eq!(descriptor_of(d.field(4)), Some(0x24));
    }
    assert!(
        conn.try_next(Duration::from_millis(500)).is_none(),
        "no credit while both deliveries wait for the broker"
    );
    conn.disposition(link.channel, false, 0, 1, true, AmqpExtValue::Null);
    let flow = conn.next();
    assert_eq!(flow.code, descriptor::FLOW, "{flow:?}");
    assert_eq!((flow.uint(5), flow.uint(6)), (Some(2), Some(2)));
    drop(conn);
    let _ = daemon.stop();
}

/// §9.3: deliveries the broker had not settled when the connection broke
/// go out again on the next connection, first and in their order, before
/// a sample written after the break.
#[test]
fn amqpbridge_9_3_unsettled_deliveries_go_out_again_first_and_in_order() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S93\n    amqp_address: /queues/s93\n    direction: out\n    qos:\n      history: { kind: keep_all }\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg);
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    conn.flow(&link, 0, 10);
    let w = matched_writer(&dds, "S93", false);
    for n in 1..=3u8 {
        dds.write_user_sample_borrowed(w, &[n, 0, 0, 0]).unwrap();
    }
    for n in 1..=3u8 {
        let t = conn.next_of(descriptor::TRANSFER);
        assert_eq!(t.boolean(4), Some(false));
        assert_eq!(payload_of(&t), [n, 0, 0, 0]);
    }
    conn.drop_connection();
    dds.write_user_sample_borrowed(w, &[4, 0, 0, 0]).unwrap();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    conn.flow(&link, 0, 10);
    let got: Vec<Vec<u8>> = (0..4)
        .map(|_| payload_of(&conn.next_of(descriptor::TRANSFER)))
        .collect();
    assert_eq!(
        got,
        [[1, 0, 0, 0], [2, 0, 0, 0], [3, 0, 0, 0], [4, 0, 0, 0]],
        "the three unsettled first, in order"
    );
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// §4.2/§9.3: a session the broker ends is answered, begun again on the
/// same channel after the back-off (`initial_delay_ms`), and its unsettled
/// delivery goes out again on it.
#[test]
fn amqpbridge_9_3_a_session_the_broker_ends_is_begun_again_after_the_delay() {
    let broker = Broker::bind();
    let cfg = config(&format!(
        "domain: {}\namqp:\n  broker_url: \"{}\"\n  container_id: \"zd-scripted\"\n  reconnect:\n    initial_delay_ms: 400\n    max_delay_ms: 2000\ntopics:\n  - dds_name: S93E\n    amqp_address: /queues/s93e\n    direction: out\ncatalog:\n  enabled: false\n",
        domain(),
        broker.url()
    ));
    let daemon = Daemon::start(cfg);
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    conn.flow(&link, 0, 10);
    let w = matched_writer(&dds, "S93E", false);
    dds.write_user_sample_borrowed(w, &[9, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    assert_eq!(payload_of(&t), [9, 0, 0, 0]);
    let ended = Instant::now();
    conn.end(link.channel, "amqp:resource-limit-exceeded");
    let end = conn.next_of(descriptor::END);
    assert_eq!(end.channel, link.channel, "the daemon answers the end");
    let begin = conn.next_of(descriptor::BEGIN);
    let waited = ended.elapsed();
    assert_eq!(begin.channel, link.channel, "begun again on its channel");
    assert!(
        waited >= Duration::from_millis(400) && waited < Duration::from_secs(3),
        "begun again after {waited:?}"
    );
    conn.answer_begin(&begin);
    let attach = conn.next_of(descriptor::ATTACH);
    let link = conn.answer_attach(&attach, 0);
    conn.flow(&link, 0, 10);
    let again = conn.next_of(descriptor::TRANSFER);
    assert_eq!(
        payload_of(&again),
        [9, 0, 0, 0],
        "the unsettled delivery again"
    );
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// §4.3: the message of a DDS dispose names the writer and the sequence
/// number of the dispose in its message-id (DDS-AMQP 1.0 §8.2), not 24 zero
/// octets: the same writer as its write, the next sequence number.
#[test]
fn amqpbridge_4_3_a_dispose_carries_the_writer_and_sequence_in_its_message_id() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S43D\n    amqp_address: /queues/s43d\n    direction: out\n    keyed: true\n    qos:\n      history: { kind: keep_all }\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg);
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    conn.flow(&link, 0, 10);
    let w = matched_writer(&dds, "S43D", true);
    let key = [0x11; 16];
    dds.write_user_sample_keyed(w, &[1, 0, 0, 0], key, None)
        .unwrap();
    dds.write_user_lifecycle(
        w,
        key,
        None,
        zerodds_rtps::inline_qos::status_info::DISPOSED,
        None,
    )
    .unwrap();
    let write = conn.next_of(descriptor::TRANSFER);
    let dispose = conn.next_of(descriptor::TRANSFER);
    assert_eq!(
        decode_message(&dispose.payload).unwrap().operation,
        DdsOperation::Dispose
    );
    let (w_id, d_id) = (message_id_of(&write), message_id_of(&dispose));
    assert_eq!((w_id.len(), d_id.len()), (24, 24));
    assert_ne!(d_id[..16], [0u8; 16], "the writer GUID");
    assert_eq!(d_id[..16], w_id[..16], "the same writer");
    let seq = |id: &[u8]| u64::from_be_bytes(id[16..].try_into().unwrap());
    assert_eq!(
        seq(&d_id),
        seq(&w_id) + 1,
        "the dispose's own sequence number"
    );
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// §4.3: a dispose this bridge wrote to DDS from AMQP comes back on its own
/// reader of the same topic; it is recognized by the GUID prefix and not
/// sent to the broker. The first message on the sending link is the sample
/// another participant wrote after it.
#[test]
fn amqpbridge_4_3_a_dispose_from_amqp_is_not_sent_back() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S43L\n    amqp_address: /queues/s43l\n    keyed: true\n    qos:\n      history: { kind: keep_all }\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg.clone());
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let links = conn.setup_links(2, 0);
    let out = the_link(&links, "zd-scripted/S43L/sender").clone();
    let inn = the_link(&links, "zd-scripted/S43L/receiver").clone();
    conn.flow(&out, 0, 10);
    let w = matched_writer(&dds, "S43L", true);
    let key = [0x22; 16];
    conn.transfer(
        &inn,
        0,
        false,
        &message(&cfg, &[1, 0, 0, 0], Some(key), DdsOperation::Write),
    );
    conn.transfer(
        &inn,
        1,
        false,
        &message(&cfg, &[], Some(key), DdsOperation::Dispose),
    );
    for id in 0..2 {
        let d = conn.next_of(descriptor::DISPOSITION);
        assert_eq!(d.uint(1), Some(id));
        assert_eq!(descriptor_of(d.field(4)), Some(0x24), "written to DDS");
    }
    dds.write_user_sample_keyed(w, &[5, 0, 0, 0], [0x33; 16], None)
        .unwrap();
    let first = conn.next_of(descriptor::TRANSFER);
    let m = decode_message(&first.payload).unwrap();
    assert_eq!(
        m.operation,
        DdsOperation::Write,
        "not the bridge's own dispose"
    );
    assert_eq!(payload_of(&first), [5, 0, 0, 0]);
    assert!(
        conn.try_next(Duration::from_millis(500))
            .is_none_or(|f| f.code != descriptor::TRANSFER),
        "nothing else goes out"
    );
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// §6 HISTORY on the daemon's DDS writer: a KEEP_ALL, TRANSIENT_LOCAL
/// topic from AMQP keeps every sample for a DDS reader that joins after
/// they were written (DDS 1.4 §2.2.3.18). With the depth KEEP_LAST(1) the
/// late reader would get only the last of the three.
#[test]
fn amqpbridge_6_a_keep_all_topic_from_amqp_keeps_every_sample_for_a_late_reader() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S6KA\n    amqp_address: /queues/s6ka\n    direction: in\n    qos:\n      reliability: reliable\n      durability: transient_local\n      history: { kind: keep_all }\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg.clone());
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    assert!(!link.daemon_sends);
    let _ = conn.next_of(descriptor::FLOW);
    for n in 0..3u8 {
        conn.transfer(
            &link,
            u32::from(n),
            false,
            &message(&cfg, &[n + 1, 0, 0, 0], None, DdsOperation::Write),
        );
        let d = conn.next_of(descriptor::DISPOSITION);
        assert_eq!(d.uint(1), Some(u32::from(n)));
        assert_eq!(descriptor_of(d.field(4)), Some(0x24), "written to DDS");
    }
    // The reader joins after the three samples were written.
    let dds = participant();
    let (_rid, rx) = dds
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "S6KA".into(),
                type_name: "S6KA".into(),
                reliable: true,
                durability: zerodds_qos::DurabilityKind::TransientLocal,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let deadline = Instant::now() + STEP;
    let mut got: Vec<Vec<u8>> = Vec::new();
    while got.len() < 3 {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(UserSample::Alive { payload, .. }) => got.push(payload.as_ref().to_vec()),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    assert_eq!(
        got,
        [[1, 0, 0, 0], [2, 0, 0, 0], [3, 0, 0, 0]],
        "every sample of the KEEP_ALL writer reaches the late reader"
    );
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// The header fields of a transfer of the daemon.
fn header_of(transfer: &Frame) -> Vec<AmqpExtValue> {
    match MessageSection::decode(&transfer.payload).unwrap().0 {
        MessageSection::Header(AmqpExtValue::List(f)) => f,
        other => panic!("the header first: {other:?}"),
    }
}

/// The message-annotations of a transfer of the daemon; empty without the
/// section.
fn annotations_of(transfer: &Frame) -> Vec<(AmqpExtValue, AmqpExtValue)> {
    let mut rest = &transfer.payload[..];
    while !rest.is_empty() {
        let (s, n) = MessageSection::decode(rest).unwrap();
        rest = &rest[n..];
        if let MessageSection::MessageAnnotations(AmqpExtValue::Map(m)) = s {
            return m;
        }
    }
    Vec::new()
}

/// An outcome of Part 3 §3.4 as the broker sends it.
fn outcome(descriptor: u64, fields: Vec<AmqpExtValue>) -> AmqpExtValue {
    AmqpExtValue::Described {
        descriptor,
        value: Box::new(AmqpExtValue::List(fields)),
    }
}

/// A daemon with one `out` topic `name` (KEEP_ALL) towards the scripted
/// broker, its link attached with credit 100, and a matched DDS writer.
fn sending_daemon(
    broker: &Broker,
    name: &str,
) -> (
    Daemon,
    Arc<DcpsRuntime>,
    scripted_broker::Conn,
    Link,
    EntityId,
) {
    let cfg = config(&yaml(
        broker,
        "",
        &format!(
            "topics:\n  - dds_name: {name}\n    amqp_address: /queues/{name}\n    direction: out\n    qos:\n      history: {{ kind: keep_all }}\ncatalog:\n  enabled: false\n"
        ),
    ));
    let daemon = Daemon::start(cfg);
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    conn.flow(&link, 0, 100);
    let w = matched_writer(&dds, name, false);
    (daemon, dds, conn, link, w)
}

/// §4.5, AMQP 1.0 Part 3 §3.4.4: a delivery the broker releases goes out
/// again with `first-acquirer` false and its delivery-count unchanged; after
/// the fifth release it is discarded, and the next sample follows.
#[test]
fn amqpbridge_4_5_a_released_delivery_is_sent_again_and_dropped_after_five() {
    let broker = Broker::bind();
    let (daemon, dds, mut conn, link, w) = sending_daemon(&broker, "S45R");
    dds.write_user_sample_borrowed(w, &[1, 0, 0, 0]).unwrap();
    let mut ids = Vec::new();
    for attempt in 0..5 {
        let t = conn.next_of(descriptor::TRANSFER);
        assert_eq!(payload_of(&t), [1, 0, 0, 0], "attempt {attempt}");
        let h = header_of(&t);
        assert_eq!(
            h[3],
            AmqpExtValue::Boolean(attempt == 0),
            "first-acquirer on attempt {attempt}"
        );
        assert!(
            matches!(h.get(4), None | Some(AmqpExtValue::Null)),
            "released does not count: {h:?}"
        );
        let id = t.uint(1).unwrap();
        ids.push(id);
        conn.disposition(link.channel, true, id, id, true, outcome(0x26, vec![]));
    }
    assert_eq!(
        ids.len(),
        ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
        "a new delivery id each time: {ids:?}"
    );
    dds.write_user_sample_borrowed(w, &[2, 0, 0, 0]).unwrap();
    let next = conn.next_of(descriptor::TRANSFER);
    assert_eq!(
        payload_of(&next),
        [2, 0, 0, 0],
        "the released one was not sent a sixth time"
    );
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// §4.5, Part 3 §3.4.3: a rejected delivery is not sent again.
#[test]
fn amqpbridge_4_5_a_rejected_delivery_is_not_sent_again() {
    let broker = Broker::bind();
    let (daemon, dds, mut conn, link, w) = sending_daemon(&broker, "S45J");
    dds.write_user_sample_borrowed(w, &[1, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    let id = t.uint(1).unwrap();
    conn.disposition(
        link.channel,
        true,
        id,
        id,
        true,
        outcome(
            0x25,
            vec![scripted_broker::error("amqp:resource-limit-exceeded")],
        ),
    );
    dds.write_user_sample_borrowed(w, &[2, 0, 0, 0]).unwrap();
    let next = conn.next_of(descriptor::TRANSFER);
    assert_eq!(payload_of(&next), [2, 0, 0, 0], "1 is not sent again");
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// §4.5, Part 3 §3.4.5: `modified` with delivery-failed sends the message
/// again with its delivery-count one more and the outcome's
/// message-annotations; with undeliverable-here it is not sent again on the
/// link.
#[test]
fn amqpbridge_4_5_modified_counts_annotates_and_respects_undeliverable_here() {
    let broker = Broker::bind();
    let (daemon, dds, mut conn, link, w) = sending_daemon(&broker, "S45M");
    let key = AmqpExtValue::Symbol("x-opt-broker-note".into());
    dds.write_user_sample_borrowed(w, &[1, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    assert!(annotations_of(&t).is_empty());
    let id = t.uint(1).unwrap();
    conn.disposition(
        link.channel,
        true,
        id,
        id,
        true,
        outcome(
            0x27,
            vec![
                AmqpExtValue::Boolean(true),
                AmqpExtValue::Boolean(false),
                AmqpExtValue::Map(vec![(key.clone(), AmqpExtValue::Str("full".into()))]),
            ],
        ),
    );
    let again = conn.next_of(descriptor::TRANSFER);
    assert_eq!(payload_of(&again), [1, 0, 0, 0]);
    let h = header_of(&again);
    assert_eq!(h[3], AmqpExtValue::Boolean(false), "first-acquirer");
    assert_eq!(h[4], AmqpExtValue::Uint(1), "delivery-count");
    assert_eq!(
        annotations_of(&again),
        [(key, AmqpExtValue::Str("full".into()))]
    );
    let id = again.uint(1).unwrap();
    conn.disposition(
        link.channel,
        true,
        id,
        id,
        true,
        outcome(
            0x27,
            vec![AmqpExtValue::Boolean(true), AmqpExtValue::Boolean(true)],
        ),
    );
    dds.write_user_sample_borrowed(w, &[2, 0, 0, 0]).unwrap();
    let next = conn.next_of(descriptor::TRANSFER);
    assert_eq!(
        payload_of(&next),
        [2, 0, 0, 0],
        "undeliverable here: not sent again"
    );
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// A writer of the test on `topic` with `cfg`'s QoS, matched with the
/// daemon's reader.
fn matched_writer_with(dds: &DcpsRuntime, cfg: UserWriterConfig) -> EntityId {
    let eid = dds.register_user_writer_kind(cfg, false).unwrap();
    let deadline = Instant::now() + STEP;
    while dds.user_writer_matched_count(eid) == 0 {
        assert!(
            Instant::now() < deadline,
            "the daemon's reader did not match"
        );
        dds.wait_match_event(Duration::from_millis(50));
    }
    eid
}

/// The application property `key` of a transfer of the daemon.
fn app_property(transfer: &Frame, key: &str) -> Option<AmqpExtValue> {
    match decode_message(&transfer.payload)
        .unwrap()
        .application_properties
    {
        AmqpExtValue::Map(m) => m.into_iter().find_map(|(k, v)| match k {
            AmqpExtValue::Str(s) | AmqpExtValue::Symbol(s) if s == key => Some(v),
            _ => None,
        }),
        _ => None,
    }
}

/// §6 History KEEP_LAST(N): while the broker grants no credit the daemon
/// holds the last N samples; the older ones are dropped.
#[test]
fn amqpbridge_6_keep_last_holds_n_and_drops_the_oldest() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S6KL\n    amqp_address: /queues/s6kl\n    direction: out\n    qos:\n      history: { kind: keep_last, depth: 2 }\ncatalog:\n  enabled: false\n",
    ));
    let metrics = BridgeMetrics::register(&Registry::new());
    let daemon = Daemon::start_with(cfg, metrics.clone());
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    let w = matched_writer(&dds, "S6KL", false);
    for n in 1..=5u8 {
        dds.write_user_sample_borrowed(w, &[n, 0, 0, 0]).unwrap();
    }
    // Every sample has reached the daemon before the credit comes. The
    // pump counts a sample before it queues it, and takes the flow below
    // from the same channel only after that sample is queued.
    wait_for("the daemon took the five samples", || {
        metrics.dds_samples_in_total.get() == 5
    });
    conn.flow(&link, 0, 10);
    let got: Vec<Vec<u8>> = (0..2)
        .map(|_| payload_of(&conn.next_of(descriptor::TRANSFER)))
        .collect();
    assert_eq!(got, [[4, 0, 0, 0], [5, 0, 0, 0]], "the last two");
    assert!(
        conn.try_next(Duration::from_millis(500))
            .is_none_or(|f| f.code != descriptor::TRANSFER),
        "nothing more"
    );
    assert_eq!(
        metrics.errors_total.get(),
        3,
        "three queue_full_dropped_oldest"
    );
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// §6 Durability TRANSIENT_LOCAL: a durable message (`header.durable`) to a
/// durable node (target `durable` 1, configuration) when `sender.durable`
/// is not set; VOLATILE: neither. The source of the receiver link of a
/// TRANSIENT_LOCAL topic stays `durable` 0: the default is for the
/// messages the daemon sends.
#[test]
fn amqpbridge_6_transient_local_is_a_durable_message_to_a_durable_node() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S6TL\n    amqp_address: /queues/s6tl\n    direction: out\n    qos:\n      durability: transient_local\n  - dds_name: S6VO\n    amqp_address: /queues/s6vo\n    direction: out\n  - dds_name: S6TI\n    amqp_address: /queues/s6ti\n    direction: in\n    qos:\n      durability: transient_local\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg);
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let links = conn.setup_links(3, 0);
    let target_durable = |l: &Link| match l.attach.field(6) {
        AmqpExtValue::Described { value, .. } => match &**value {
            AmqpExtValue::List(f) => f.get(1).cloned(),
            _ => None,
        },
        _ => None,
    };
    let tl = the_link(&links, "zd-scripted/S6TL/sender").clone();
    let vo = the_link(&links, "zd-scripted/S6VO/sender").clone();
    let ti = the_link(&links, "zd-scripted/S6TI/receiver");
    let source_durable = match ti.attach.field(5) {
        AmqpExtValue::Described { value, .. } => match &**value {
            AmqpExtValue::List(f) => f.get(1).cloned(),
            _ => None,
        },
        _ => None,
    };
    assert!(
        matches!(
            source_durable,
            None | Some(AmqpExtValue::Uint(0)) | Some(AmqpExtValue::Null)
        ),
        "the source of the receiver link: {source_durable:?}"
    );
    assert_eq!(target_durable(&tl), Some(AmqpExtValue::Uint(1)));
    assert!(
        matches!(
            target_durable(&vo),
            None | Some(AmqpExtValue::Uint(0)) | Some(AmqpExtValue::Null)
        ),
        "{:?}",
        target_durable(&vo)
    );
    conn.flow(&tl, 0, 10);
    conn.flow(&vo, 0, 10);
    let w_tl = matched_writer_with(
        &dds,
        UserWriterConfig {
            topic_name: "S6TL".into(),
            type_name: "S6TL".into(),
            reliable: true,
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            ..Default::default()
        },
    );
    let w_vo = matched_writer(&dds, "S6VO", false);
    dds.write_user_sample_borrowed(w_tl, &[1, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    assert_eq!(t.channel, tl.channel);
    assert_eq!(header_of(&t)[0], AmqpExtValue::Boolean(true), "durable");
    dds.write_user_sample_borrowed(w_vo, &[2, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    assert_eq!(t.channel, vo.channel);
    assert_eq!(
        header_of(&t)[0],
        AmqpExtValue::Boolean(false),
        "not durable"
    );
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// §6 Liveliness: a broker that announces an idle time-out of 600 ms gets
/// an empty frame at least every 300 ms (AMQP 1.0 Part 2 §2.4.5).
#[test]
fn amqpbridge_6_a_heartbeat_goes_out_at_half_the_idle_time_out() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S6HB\n    amqp_address: /queues/s6hb\n    direction: out\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg);
    let mut conn = broker.accept(STEP);
    assert_eq!(conn.sasl(&["ANONYMOUS"], 0), "ANONYMOUS");
    conn.read_open();
    conn.send_open_with_idle(255, 600);
    let start = Instant::now();
    let beats = conn.heartbeats(Duration::from_millis(1600));
    assert!(beats.len() >= 4, "{} heartbeats in 1.6 s", beats.len());
    let mut last = start;
    for b in &beats {
        let gap = b.duration_since(last);
        assert!(gap <= Duration::from_millis(400), "a gap of {gap:?}");
        last = *b;
    }
    drop(conn);
    let _ = daemon.stop();
}

/// §6 Deadline: the daemon's reader requests `qos.deadline_ms`; after a gap
/// longer than the period the next message carries the reader's
/// REQUESTED_DEADLINE_MISSED total as the annotation
/// `x-opt-zerodds-deadline-missed` (AMQP 1.0 Part 3 §3.2.10: an `x-opt-`
/// key a receiver may ignore); the message before the gap has none.
#[test]
fn amqpbridge_6_a_missed_deadline_is_annotated() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S6DL\n    amqp_address: /queues/s6dl\n    direction: out\n    qos:\n      deadline_ms: 200\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg);
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    conn.flow(&link, 0, 10);
    // RxO (DDS 1.4 §2.2.3.7): the offered period is at most the requested.
    let w = matched_writer_with(
        &dds,
        UserWriterConfig {
            topic_name: "S6DL".into(),
            type_name: "S6DL".into(),
            reliable: true,
            deadline: zerodds_qos::DeadlineQosPolicy {
                period: zerodds_qos::Duration::from_millis(100),
            },
            ..Default::default()
        },
    );
    dds.write_user_sample_borrowed(w, &[1, 0, 0, 0]).unwrap();
    let first = conn.next_of(descriptor::TRANSFER);
    assert!(annotations_of(&first).is_empty(), "no deadline missed yet");
    std::thread::sleep(Duration::from_millis(700));
    dds.write_user_sample_borrowed(w, &[2, 0, 0, 0]).unwrap();
    let second = conn.next_of(descriptor::TRANSFER);
    assert_eq!(payload_of(&second), [2, 0, 0, 0]);
    let a = annotations_of(&second);
    let total = a.iter().find_map(|(k, v)| match (k, v) {
        (AmqpExtValue::Symbol(k), AmqpExtValue::Ulong(n))
            if k == "x-opt-zerodds-deadline-missed" =>
        {
            Some(*n)
        }
        _ => None,
    });
    assert!(total.is_some_and(|n| n >= 1), "{a:?}");
    dds.write_user_sample_borrowed(w, &[3, 0, 0, 0]).unwrap();
    let third = conn.next_of(descriptor::TRANSFER);
    assert!(annotations_of(&third).is_empty(), "told once");
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// §6 Partition, DDS → AMQP: the daemon's reader is in the topic's
/// partitions, matches a writer there, and the message names the writer's
/// partition in `dds:partition` (DDS-AMQP 1.0 §8.3). A writer in the
/// default partition does not match it: by the time a reader of another
/// participant in the default partition has matched that writer, the
/// daemon's reader has not, and a sample of the writer is not sent.
#[test]
fn amqpbridge_6_partitions_reach_the_reader_and_the_message() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S6PO\n    amqp_address: /queues/s6po\n    direction: out\n    qos:\n      partition: [\"site-*\"]\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg);
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    conn.flow(&link, 0, 10);
    let w = matched_writer_with(
        &dds,
        UserWriterConfig {
            topic_name: "S6PO".into(),
            type_name: "S6PO".into(),
            reliable: true,
            partition: vec!["site-a".into()],
            ..Default::default()
        },
    );
    // A writer in the default partition does not match the daemon's reader.
    let other = dds
        .register_user_writer_kind(
            UserWriterConfig {
                topic_name: "S6PO".into(),
                type_name: "S6PO".into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    dds.write_user_sample_borrowed(w, &[1, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    assert_eq!(
        app_property(&t, "dds:partition"),
        Some(AmqpExtValue::Str("site-a".into()))
    );
    not_matched_but_discovered(&dds, other, "S6PO");
    dds.write_user_sample_borrowed(other, &[9, 0, 0, 0])
        .unwrap();
    assert!(
        no_transfer_within(&mut conn, Duration::from_millis(500)),
        "a sample of the default partition went out"
    );
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// Whether no transfer of the daemon comes within `within` (other frames
/// are read and dropped).
fn no_transfer_within(conn: &mut scripted_broker::Conn, within: Duration) -> bool {
    let end = Instant::now() + within;
    while let Some(left) = end.checked_duration_since(Instant::now()) {
        match conn.try_next(left) {
            Some(f) if f.code == descriptor::TRANSFER => return false,
            Some(_) => {}
            None => return true,
        }
    }
    true
}

/// The writer `other` of `dds` on `topic` stays unmatched with the daemon's
/// reader although discovery has run: a reader of a second participant
/// (default partition, infinite lease) matches `other` first, and then
/// `other` has that one match only. The daemon's reader was discovered
/// before (a writer of `dds` matched it).
fn not_matched_but_discovered(dds: &DcpsRuntime, other: EntityId, topic: &str) {
    let second = participant();
    let (_rid, _rx) = second
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: topic.into(),
                type_name: topic.into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let deadline = Instant::now() + STEP;
    while dds.user_writer_matched_count(other) == 0 {
        assert!(
            Instant::now() < deadline,
            "the control reader did not match"
        );
        dds.wait_match_event(Duration::from_millis(50));
    }
    // A match with the daemon's reader would have come with the control's.
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        dds.user_writer_matched_count(other),
        1,
        "the daemon's reader matched"
    );
    second.shutdown();
}

/// §6 Liveliness: `qos.liveliness_lease_ms` is the lease of the daemon's
/// DDS endpoints (AUTOMATIC). RxO (DDS 1.4 §2.2.3.11): the offered lease
/// is at most the requested. A writer offering 200 ms matches the daemon's
/// reader of a 500 ms topic and its sample goes out; one offering 2000 ms
/// does not. A reader requesting 1000 ms matches the daemon's writer of the
/// topic from AMQP; one requesting 300 ms does not.
#[test]
fn amqpbridge_6_the_liveliness_lease_reaches_the_dds_endpoints() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S6LO\n    amqp_address: /queues/s6lo\n    direction: out\n    qos:\n      liveliness_lease_ms: 500\n  - dds_name: S6LI\n    amqp_address: /queues/s6li\n    direction: in\n    qos:\n      liveliness_lease_ms: 500\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg);
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let links = conn.setup_links(2, 0);
    let out = the_link(&links, "zd-scripted/S6LO/sender").clone();
    conn.flow(&out, 0, 10);
    let lease = |ms| zerodds_qos::LivelinessQosPolicy {
        kind: zerodds_qos::LivelinessKind::Automatic,
        lease_duration: zerodds_qos::Duration::from_millis(ms),
    };
    let writer = |ms| UserWriterConfig {
        topic_name: "S6LO".into(),
        type_name: "S6LO".into(),
        reliable: true,
        liveliness: lease(ms),
        ..Default::default()
    };
    // DDS → AMQP: the daemon's reader requests 500 ms.
    let short = matched_writer_with(&dds, writer(200));
    dds.write_user_sample_borrowed(short, &[1, 0, 0, 0])
        .unwrap();
    assert_eq!(
        payload_of(&conn.next_of(descriptor::TRANSFER)),
        [1, 0, 0, 0]
    );
    let long = dds.register_user_writer_kind(writer(2000), false).unwrap();
    not_matched_but_discovered(&dds, long, "S6LO");
    dds.write_user_sample_borrowed(long, &[2, 0, 0, 0]).unwrap();
    assert!(
        no_transfer_within(&mut conn, Duration::from_millis(500)),
        "a sample of a writer with a longer lease went out"
    );
    // AMQP → DDS: the daemon's writer offers 500 ms.
    let reader = |ms| UserReaderConfig {
        topic_name: "S6LI".into(),
        type_name: "S6LI".into(),
        reliable: true,
        liveliness: lease(ms),
        ..Default::default()
    };
    let (loose, _rx_loose) = dds.register_user_reader_kind(reader(1000), false).unwrap();
    let deadline = Instant::now() + STEP;
    while dds.user_reader_matched_count(loose) == 0 {
        assert!(
            Instant::now() < deadline,
            "the daemon's writer did not match"
        );
        dds.wait_match_event(Duration::from_millis(50));
    }
    let (strict, _rx_strict) = dds.register_user_reader_kind(reader(300), false).unwrap();
    // A writer of a second participant offering 200 ms matches `strict`;
    // by then the daemon's writer, discovered before, has not.
    let second = participant();
    let _control = second
        .register_user_writer_kind(
            UserWriterConfig {
                topic_name: "S6LI".into(),
                type_name: "S6LI".into(),
                reliable: true,
                liveliness: lease(200),
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let deadline = Instant::now() + STEP;
    while dds.user_reader_matched_count(strict) == 0 {
        assert!(
            Instant::now() < deadline,
            "the control writer did not match"
        );
        dds.wait_match_event(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(
        dds.user_reader_matched_count(strict),
        1,
        "the daemon's writer matched a reader requesting a shorter lease"
    );
    second.shutdown();
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// §6 Partition, AMQP → DDS: the daemon's writer is in the topic's
/// partitions; a message whose `dds:partition` matches none of them is
/// rejected (`amqp:precondition-failed`) and not written; one that matches,
/// and one without `dds:partition`, are written.
#[test]
fn amqpbridge_6_a_message_of_another_partition_is_rejected() {
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S6PI\n    amqp_address: /queues/s6pi\n    direction: in\n    qos:\n      partition: [\"site-a\"]\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg.clone());
    let dds = participant();
    let (rid, rx) = dds
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "S6PI".into(),
                type_name: "S6PI".into(),
                reliable: true,
                partition: vec!["site-a".into()],
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    let _ = conn.next_of(descriptor::FLOW);
    let deadline = Instant::now() + STEP;
    while dds.user_reader_matched_count(rid) == 0 {
        assert!(
            Instant::now() < deadline,
            "the daemon's writer did not match"
        );
        dds.wait_match_event(Duration::from_millis(50));
    }
    let with_partition = |payload: &[u8], partition: Option<&str>| {
        let mut sections = Vec::new();
        let mut rest = &message(&cfg, payload, None, DdsOperation::Write)[..];
        while !rest.is_empty() {
            let (s, n) = MessageSection::decode(rest).unwrap();
            rest = &rest[n..];
            let s = match (s, partition) {
                (MessageSection::ApplicationProperties(AmqpExtValue::Map(mut m)), Some(p)) => {
                    m.push((
                        AmqpExtValue::Str("dds:partition".into()),
                        AmqpExtValue::Str(p.into()),
                    ));
                    MessageSection::ApplicationProperties(AmqpExtValue::Map(m))
                }
                (s, _) => s,
            };
            sections.extend_from_slice(&s.encode().unwrap());
        }
        sections
    };
    conn.transfer(
        &link,
        0,
        false,
        &with_partition(&[1, 0, 0, 0], Some("site-b")),
    );
    let d = conn.next_of(descriptor::DISPOSITION);
    assert_eq!(descriptor_of(d.field(4)), Some(0x25), "rejected: {d:?}");
    let AmqpExtValue::Described { value, .. } = d.field(4) else {
        panic!()
    };
    let AmqpExtValue::List(f) = &**value else {
        panic!()
    };
    let AmqpExtValue::Described { value: error, .. } = &f[0] else {
        panic!("{f:?}")
    };
    let AmqpExtValue::List(error) = &**error else {
        panic!()
    };
    assert_eq!(
        error[0],
        AmqpExtValue::Symbol("amqp:precondition-failed".into())
    );
    conn.transfer(
        &link,
        1,
        false,
        &with_partition(&[2, 0, 0, 0], Some("site-a")),
    );
    assert_eq!(
        descriptor_of(conn.next_of(descriptor::DISPOSITION).field(4)),
        Some(0x24)
    );
    conn.transfer(&link, 2, false, &with_partition(&[3, 0, 0, 0], None));
    assert_eq!(
        descriptor_of(conn.next_of(descriptor::DISPOSITION).field(4)),
        Some(0x24)
    );
    let mut got = Vec::new();
    let deadline = Instant::now() + STEP;
    while got.len() < 2 {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(UserSample::Alive { payload, .. }) => got.push(payload.as_ref().to_vec()),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    assert_eq!(
        got,
        [[2, 0, 0, 0], [3, 0, 0, 0]],
        "the rejected one is not written"
    );
    drop(conn);
    let _ = daemon.stop();
    dds.shutdown();
}

/// §1 L6: a tenant is a participant of its own on its domain and a
/// connection of its own, bound to it by container id, `open.hostname` and
/// SASL identity: a sample of the tenant's domain goes out on the tenant's
/// connection and no other, and the metrics carry the tenant's label.
#[test]
fn amqpbridge_1_l6_each_tenant_has_its_participant_and_its_connection() {
    let broker = Broker::bind();
    let lease_b = zerodds_test_vendor_reader::lease_domain();
    let cfg = config(&format!(
        "domain: {}\namqp:\n  broker_url: \"{}\"\n  container_id: \"zd-own\"\ncatalog:\n  enabled: false\ntopics:\n  - dds_name: L6\n    amqp_address: /queues/own\n    direction: out\ntenants:\n  - name: plant-b\n    domain: {}\n    amqp:\n      container_id: zd-plant-b\n      hostname: vhost:b\n      sasl:\n        mechanism: PLAIN\n        username: bob\n        password: secret\n    topics:\n      - dds_name: L6\n        amqp_address: /queues/b\n        direction: out\n",
        domain(),
        broker.url(),
        lease_b.id()
    ));
    let registry = Arc::new(Registry::new());
    let stop = Arc::new(AtomicBool::new(false));
    let join = {
        let (s, r) = (Arc::clone(&stop), Arc::clone(&registry));
        std::thread::spawn(move || {
            let metrics = BridgeMetrics::register_shared(&r);
            bridge_daemon::run(
                &cfg,
                RuntimeConfig::default(),
                &s,
                &metrics,
                false,
                Duration::from_secs(5),
            )
        })
    };
    let mut conns = std::collections::BTreeMap::new();
    for _ in 0..2 {
        let mut conn = broker.accept(STEP);
        let mechanism = conn.sasl(&["ANONYMOUS", "PLAIN"], 0);
        let open = conn.read_open();
        conn.send_open(255);
        let id = open.text(0).unwrap();
        conns.insert(id, (conn, mechanism, open.text(1)));
    }
    let (mut own, own_mech, own_host) = conns.remove("zd-own").expect("the daemon's own");
    let (mut b, b_mech, b_host) = conns.remove("zd-plant-b").expect("the tenant's");
    assert_eq!(
        (own_mech.as_str(), own_host.as_deref()),
        ("ANONYMOUS", Some("127.0.0.1"))
    );
    assert_eq!(
        (b_mech.as_str(), b_host.as_deref()),
        ("PLAIN", Some("vhost:b"))
    );
    let own_link = own.setup_links(1, 0).remove(0);
    let b_link = b.setup_links(1, 0).remove(0);
    assert_eq!(
        (own_link.address.as_str(), b_link.address.as_str()),
        ("/queues/own", "/queues/b")
    );
    own.flow(&own_link, 0, 10);
    b.flow(&b_link, 0, 10);
    // A writer on the tenant's domain: its sample goes out on the tenant's
    // connection only.
    let dds_b = DcpsRuntime::start(
        lease_b.id(),
        GuidPrefix::from_bytes([0x6b; 12]),
        RuntimeConfig::default(),
    )
    .unwrap();
    let wb = matched_writer(&dds_b, "L6", false);
    dds_b
        .write_user_sample_borrowed(wb, &[0xb, 0, 0, 0])
        .unwrap();
    let t = b.next_of(descriptor::TRANSFER);
    assert_eq!(payload_of(&t), [0xb, 0, 0, 0]);
    assert!(
        own.try_next(Duration::from_millis(500))
            .is_none_or(|f| f.code != descriptor::TRANSFER),
        "not on the daemon's own connection"
    );
    let dds = participant();
    let wa = matched_writer(&dds, "L6", false);
    dds.write_user_sample_borrowed(wa, &[0xa, 0, 0, 0]).unwrap();
    assert_eq!(
        payload_of(&own.next_of(descriptor::TRANSFER)),
        [0xa, 0, 0, 0]
    );
    assert!(
        b.try_next(Duration::from_millis(500))
            .is_none_or(|f| f.code != descriptor::TRANSFER),
        "not on the tenant's connection"
    );
    let text = zerodds_monitor::render_prometheus(&registry.snapshot());
    let value = |pred: &dyn Fn(&str) -> bool| {
        text.lines()
            .find(|l| pred(l))
            .and_then(|l| l.rsplit(' ').next())
            .and_then(|v| v.parse::<u64>().ok())
    };
    assert_eq!(
        value(
            &|l: &str| l.starts_with("zerodds_amqp_bridge_transfers_out_total{")
                && l.contains("tenant=\"plant-b\"")
                && l.contains("dds_topic=\"L6\"")
        ),
        Some(1),
        "{text}"
    );
    assert_eq!(
        value(
            &|l: &str| l.starts_with("zerodds_amqp_bridge_transfers_out_total{")
                && !l.contains("tenant=")
                && l.contains("dds_topic=\"L6\"")
        ),
        Some(1),
        "{text}"
    );
    stop.store(true, Ordering::SeqCst);
    drop(own);
    drop(b);
    assert!(join.join().unwrap().is_ok());
    dds.shutdown();
    dds_b.shutdown();
}

/// §1 L6: a tenant whose broker refuses its SASL identity before its
/// first connection stood stops the daemon with exit code 5 (§2), the
/// daemon's own participant included.
#[test]
fn amqpbridge_1_l6_a_refused_tenant_stops_the_daemon() {
    let broker = Broker::bind();
    let lease_b = zerodds_test_vendor_reader::lease_domain();
    let cfg = config(&format!(
        "domain: {}\namqp:\n  broker_url: \"{}\"\n  container_id: \"zd-own5\"\ncatalog:\n  enabled: false\ntopics:\n  - dds_name: L65\n    amqp_address: /queues/own\n    direction: out\ntenants:\n  - name: plant-c\n    domain: {}\n    amqp:\n      container_id: zd-plant-c\n      sasl:\n        mechanism: PLAIN\n        username: carol\n        password: wrong\n    topics:\n      - dds_name: L65\n        amqp_address: /queues/c\n        direction: out\n",
        domain(),
        broker.url(),
        lease_b.id()
    ));
    let daemon = Daemon::start(cfg);
    let mut held = Vec::new();
    for _ in 0..2 {
        let mut conn = broker.accept(STEP);
        // SASL outcome `auth` (1, Part 5 §5.3.3.6) for the tenant's PLAIN.
        let mechanism = conn.sasl_by(&["ANONYMOUS", "PLAIN"], |m| u8::from(m == "PLAIN"));
        if mechanism == "ANONYMOUS" {
            conn.read_open();
            conn.send_open(255);
        }
        held.push(conn);
    }
    let started = Instant::now();
    let result = loop {
        if daemon
            .join
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
        {
            break daemon.stop();
        }
        assert!(started.elapsed() < STEP, "the daemon kept running");
        std::thread::sleep(Duration::from_millis(20));
    };
    let e = result.expect_err("the tenant's refusal ends the daemon");
    assert_eq!(e.code, 5, "{e}");
    drop(held);
}

/// §8.3: the spans of both directions reach an OTLP collector. A DDS
/// sample the broker accepts is `amqp.transfer` (kind client, the outcome
/// `accepted`) with the child `dds.read`; a broker message that is no DDS
/// sample is `amqp.transfer` with the outcome `rejected`, status error and
/// the condition `amqp:decode-error`, without a DDS child; a sample written
/// to DDS has the child `dds.write`.
#[test]
fn amqpbridge_8_3_the_spans_of_a_transfer_reach_the_collector() {
    use std::io::{Read, Write};
    let collector = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let cport = collector.local_addr().unwrap().port();
    let received = Arc::new(std::sync::Mutex::new(String::new()));
    {
        let r = Arc::clone(&received);
        std::thread::spawn(move || {
            for mut stream in collector.incoming().flatten() {
                stream
                    .set_read_timeout(Some(Duration::from_millis(500)))
                    .unwrap();
                let mut text = Vec::new();
                let _ = stream.read_to_end(&mut text);
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
                r.lock().unwrap().push_str(&String::from_utf8_lossy(&text));
            }
        });
    }
    let exporter = Arc::new(zerodds_observability_otlp::OtlpExporter::new(
        zerodds_amqp_endpoint::daemon_runtime::otlp_config_from_endpoint(
            "zerodds-amqp-bridged",
            &format!("http://127.0.0.1:{cport}"),
        ),
    ));
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S83\n    amqp_address: /queues/s83\n    direction: bidir\ncatalog:\n  enabled: false\n",
    ));
    let stop = Arc::new(AtomicBool::new(false));
    let join = {
        let (s, cfg, e) = (Arc::clone(&stop), cfg.clone(), Arc::clone(&exporter));
        std::thread::spawn(move || {
            let metrics = BridgeMetrics::register(&Registry::new());
            bridge_daemon::run_with_reload(
                &cfg,
                RuntimeConfig::default(),
                &s,
                &metrics,
                false,
                Duration::from_secs(5),
                None,
                bridge_daemon::Tracer::new(Some(e)),
            )
        })
    };
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let links = conn.setup_links(2, 0);
    let out = the_link(&links, "zd-scripted/S83/sender").clone();
    let inn = the_link(&links, "zd-scripted/S83/receiver").clone();
    conn.next_of(descriptor::FLOW);
    conn.flow(&out, 0, 10);
    // DDS → AMQP, accepted by the broker.
    let w = matched_writer(&dds, "S83", false);
    dds.write_user_sample_borrowed(w, &[7, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    let id = t.uint(1).unwrap();
    conn.disposition(out.channel, true, id, id, true, accepted());
    // AMQP → DDS: a body that is no DDS sample, then a sample.
    conn.transfer(&inn, 0, false, b"\x00\x53\x75\xa0\x01x");
    let d = conn.next_of(descriptor::DISPOSITION);
    assert_eq!(descriptor_of(d.field(4)), Some(0x25), "rejected");
    let m = message(&cfg, &[1, 0, 0, 0], None, DdsOperation::Write);
    conn.transfer(&inn, 1, false, &m);
    let d = conn.next_of(descriptor::DISPOSITION);
    assert_eq!(descriptor_of(d.field(4)), Some(0x24), "accepted");
    drop(conn);
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap().unwrap();
    exporter.flush().unwrap();
    let text = received.lock().unwrap().clone();
    assert!(text.contains("POST /v1/traces"), "{text}");
    // Three round trips: three roots, one `dds.read`, one `dds.write`.
    assert_eq!(
        text.matches("\"name\":\"amqp.transfer\"").count(),
        3,
        "{text}"
    );
    assert_eq!(text.matches("\"name\":\"dds.read\"").count(), 1, "{text}");
    assert_eq!(text.matches("\"name\":\"dds.write\"").count(), 1, "{text}");
    for attr in [
        r#"{"key":"messaging.operation","value":{"stringValue":"publish"}}"#,
        r#"{"key":"messaging.operation","value":{"stringValue":"receive"}}"#,
        r#"{"key":"messaging.destination.name","value":{"stringValue":"/queues/s83"}}"#,
        r#"{"key":"amqp.outcome","value":{"stringValue":"rejected"}}"#,
        r#"{"key":"amqp.error_condition","value":{"stringValue":"amqp:decode-error"}}"#,
        r#"{"key":"dds.topic","value":{"stringValue":"S83"}}"#,
    ] {
        assert!(text.contains(attr), "{attr} in {text}");
    }
    assert_eq!(
        text.matches(r#"{"key":"amqp.outcome","value":{"stringValue":"accepted"}}"#)
            .count(),
        2,
        "{text}"
    );
    dds.shutdown();
}

/// The condition of a `rejected` outcome (Part 3 §3.4.3) in a disposition.
fn rejected_condition(d: &Frame) -> String {
    assert_eq!(descriptor_of(d.field(4)), Some(0x25), "rejected: {d:?}");
    let AmqpExtValue::Described { value, .. } = d.field(4) else {
        panic!()
    };
    let AmqpExtValue::List(f) = &**value else {
        panic!()
    };
    let AmqpExtValue::Described { value: error, .. } = &f[0] else {
        panic!("{f:?}")
    };
    let AmqpExtValue::List(error) = &**error else {
        panic!()
    };
    match &error[0] {
        AmqpExtValue::Symbol(s) => s.clone(),
        other => panic!("{other:?}"),
    }
}

/// §7.3: a direction the ACL denies at start gets neither its DDS endpoint
/// nor its link. `read: []` on a `bidir` topic: only the receiving link is
/// attached, and a DDS sample of the topic reaches the broker on no link
/// while a broker message is written to DDS; `write: []`: only the sending
/// link, a DDS sample goes out, no DDS writer of the daemon matches.
#[test]
fn amqpbridge_7_3_a_direction_denied_at_start_moves_no_data() {
    // read denied, write allowed.
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S73R\n    amqp_address: /queues/s73r\n    direction: bidir\nacl:\n  rules:\n    - topic: S73R\n      read: []\n      write: [\"*\"]\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg.clone());
    let dds = participant();
    let (rid, rx) = dds
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "S73R".into(),
                type_name: "S73R".into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    // The writer on a participant of its own: the reader above would
    // match it inside one participant.
    let publisher = participant();
    let w = publisher
        .register_user_writer_kind(
            UserWriterConfig {
                topic_name: "S73R".into(),
                type_name: "S73R".into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let links = conn.setup_links(1, 0);
    assert_eq!(links[0].name, "zd-scripted/S73R/receiver", "{links:?}");
    let _ = conn.next_of(descriptor::FLOW);
    let deadline = Instant::now() + STEP;
    while dds.user_reader_matched_count(rid) == 0 {
        assert!(
            Instant::now() < deadline,
            "the daemon's writer did not match"
        );
        dds.wait_match_event(Duration::from_millis(50));
    }
    // The daemon has no reader: the test's writer matches the test's
    // reader and nothing else.
    let deadline = Instant::now() + STEP;
    while publisher.user_writer_matched_count(w) == 0 {
        assert!(Instant::now() < deadline, "the test's reader did not match");
        publisher.wait_match_event(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        publisher.user_writer_matched_count(w),
        1,
        "the test's reader alone; no DDS reader of the daemon for a denied read"
    );
    publisher
        .write_user_sample_borrowed(w, &[9, 0, 0, 0])
        .unwrap();
    let m = message(&cfg, &[1, 0, 0, 0], None, DdsOperation::Write);
    conn.transfer(&links[0], 0, false, &m);
    let d = conn.next_of(descriptor::DISPOSITION);
    assert_eq!(descriptor_of(d.field(4)), Some(0x24), "accepted");
    let deadline = Instant::now() + STEP;
    let got = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(UserSample::Alive { payload, .. }) if payload.as_ref() == [1, 0, 0, 0] => {
                break payload.as_ref().to_vec();
            }
            Ok(_) => {}
            Err(_) => panic!("the allowed direction wrote nothing"),
        }
    };
    assert_eq!(got, [1, 0, 0, 0]);
    assert!(
        conn.try_next(Duration::from_millis(500))
            .is_none_or(|f| f.code != descriptor::TRANSFER),
        "a sample of a denied read went out"
    );
    drop(conn);
    let _ = daemon.stop();
    publisher.shutdown();

    // write denied, read allowed.
    let broker = Broker::bind();
    let cfg = config(&yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S73W\n    amqp_address: /queues/s73w\n    direction: bidir\nacl:\n  rules:\n    - topic: S73W\n      read: [\"*\"]\n      write: []\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg);
    // The reader on a participant of its own: the writer of the test would
    // match it inside one participant.
    let observer = participant();
    let (rid, _rx) = observer
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "S73W".into(),
                type_name: "S73W".into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let links = conn.setup_links(1, 0);
    assert_eq!(links[0].name, "zd-scripted/S73W/sender", "{links:?}");
    conn.flow(&links[0], 0, 10);
    let w = matched_writer(&dds, "S73W", false);
    dds.write_user_sample_borrowed(w, &[5, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    assert_eq!(payload_of(&t), [5, 0, 0, 0], "the allowed direction sends");
    let deadline = Instant::now() + STEP;
    while observer.user_reader_matched_count(rid) == 0 {
        assert!(Instant::now() < deadline, "the test's writer did not match");
        observer.wait_match_event(Duration::from_millis(50));
    }
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        observer.user_reader_matched_count(rid),
        1,
        "the test's writer alone; no DDS writer of the daemon for a denied write"
    );
    drop(conn);
    let _ = daemon.stop();
    observer.shutdown();
    dds.shutdown();
}

/// The configuration a reload reads from the YAML text `yaml`.
fn reload_config(yaml: &str) -> Result<DaemonConfig, String> {
    let mut config = zerodds_config::Config::default();
    zerodds_config::parse_str(yaml, "<reload>")
        .map_err(|e| e.to_string())
        .and_then(|p| config.push(p, "<reload>".into()).map_err(|e| e.to_string()))
        .and_then(|()| DaemonConfig::from_config(Some(&config)).map_err(|e| e.to_string()))
}

/// §7.3, §9.2: a write right a reload withdraws is gone at once; the next
/// broker message is settled `rejected` with `amqp:unauthorized-access` and
/// is not written to DDS, while the message before the reload was.
#[test]
fn amqpbridge_7_3_a_write_right_withdrawn_by_reload_rejects_the_transfer() {
    let broker = Broker::bind();
    let open = yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S73L\n    amqp_address: /queues/s73l\n    direction: in\ncatalog:\n  enabled: false\n",
    );
    let closed =
        format!("{open}acl:\n  rules:\n    - topic: S73L\n      read: [\"*\"]\n      write: []\n");
    let cfg = config(&open);
    let current = Arc::new(std::sync::Mutex::new(open.clone()));
    let flag = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let (loaded_tx, loaded) = std::sync::mpsc::channel::<()>();
    let join = {
        let (s, f, c, cfg) = (
            Arc::clone(&stop),
            Arc::clone(&flag),
            Arc::clone(&current),
            cfg.clone(),
        );
        let loaded_tx = std::sync::Mutex::new(loaded_tx);
        std::thread::spawn(move || {
            let metrics = BridgeMetrics::register(&Registry::new());
            let load = move || -> Result<DaemonConfig, String> {
                let parsed = reload_config(&c.lock().unwrap());
                let _ = loaded_tx.lock().unwrap().send(());
                parsed
            };
            bridge_daemon::run_with_reload(
                &cfg,
                RuntimeConfig::default(),
                &s,
                &metrics,
                false,
                Duration::from_secs(5),
                Some(bridge_daemon::Reload {
                    flag: f,
                    load: Box::new(load),
                }),
                bridge_daemon::Tracer::default(),
            )
        })
    };
    let dds = participant();
    let (rid, rx) = dds
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "S73L".into(),
                type_name: "S73L".into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    let _ = conn.next_of(descriptor::FLOW);
    let deadline = Instant::now() + STEP;
    while dds.user_reader_matched_count(rid) == 0 {
        assert!(
            Instant::now() < deadline,
            "the daemon's writer did not match"
        );
        dds.wait_match_event(Duration::from_millis(50));
    }
    conn.transfer(
        &link,
        0,
        false,
        &message(&cfg, &[1, 0, 0, 0], None, DdsOperation::Write),
    );
    assert_eq!(
        descriptor_of(conn.next_of(descriptor::DISPOSITION).field(4)),
        Some(0x24)
    );
    *current.lock().unwrap() = closed;
    flag.store(true, Ordering::SeqCst);
    loaded
        .recv_timeout(STEP)
        .expect("the daemon read the configuration");
    // The new ACL applies right after the load.
    std::thread::sleep(Duration::from_millis(200));
    conn.transfer(
        &link,
        1,
        false,
        &message(&cfg, &[2, 0, 0, 0], None, DdsOperation::Write),
    );
    let d = conn.next_of(descriptor::DISPOSITION);
    assert_eq!(d.uint(1), Some(1));
    assert_eq!(rejected_condition(&d), "amqp:unauthorized-access");
    let mut got = Vec::new();
    while let Ok(s) = rx.recv_timeout(Duration::from_secs(1)) {
        if let UserSample::Alive { payload, .. } = s {
            got.push(payload.as_ref().to_vec());
        }
    }
    assert_eq!(
        got,
        [[1, 0, 0, 0]],
        "the message after the reload is not written"
    );
    drop(conn);
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap().unwrap();
    dds.shutdown();
}

/// §7.3, §9.2: a read right a reload withdraws stops the topic's messages
/// that are still in the daemon. A message queued for credit and one the
/// broker releases after the reload do not go out when credit comes; the
/// samples were taken from DDS before the reload (four counted in).
#[test]
fn amqpbridge_7_3_a_read_right_withdrawn_by_reload_stops_the_queued_messages() {
    let broker = Broker::bind();
    let open = yaml(
        &broker,
        "",
        "topics:\n  - dds_name: S73Q\n    amqp_address: /queues/s73q\n    direction: out\n    qos:\n      history: { kind: keep_all }\ncatalog:\n  enabled: false\n",
    );
    let closed =
        format!("{open}acl:\n  rules:\n    - topic: S73Q\n      read: []\n      write: [\"*\"]\n");
    let cfg = config(&open);
    let current = Arc::new(std::sync::Mutex::new(open.clone()));
    let flag = Arc::new(AtomicBool::new(false));
    let stop = Arc::new(AtomicBool::new(false));
    let registry = Arc::new(Registry::new());
    let (loaded_tx, loaded) = std::sync::mpsc::channel::<()>();
    let join = {
        let (s, f, c, cfg, r) = (
            Arc::clone(&stop),
            Arc::clone(&flag),
            Arc::clone(&current),
            cfg.clone(),
            Arc::clone(&registry),
        );
        let loaded_tx = std::sync::Mutex::new(loaded_tx);
        std::thread::spawn(move || {
            let metrics = BridgeMetrics::register(&r);
            let load = move || -> Result<DaemonConfig, String> {
                let parsed = reload_config(&c.lock().unwrap());
                let _ = loaded_tx.lock().unwrap().send(());
                parsed
            };
            bridge_daemon::run_with_reload(
                &cfg,
                RuntimeConfig::default(),
                &s,
                &metrics,
                false,
                Duration::from_secs(5),
                Some(bridge_daemon::Reload {
                    flag: f,
                    load: Box::new(load),
                }),
                bridge_daemon::Tracer::default(),
            )
        })
    };
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    // Credit for two: 1 is accepted, 2 stays unsettled.
    conn.flow(&link, 0, 2);
    let w = matched_writer(&dds, "S73Q", false);
    dds.write_user_sample_borrowed(w, &[1, 0, 0, 0]).unwrap();
    dds.write_user_sample_borrowed(w, &[2, 0, 0, 0]).unwrap();
    let t1 = conn.next_of(descriptor::TRANSFER);
    let t2 = conn.next_of(descriptor::TRANSFER);
    assert_eq!(payload_of(&t1), [1, 0, 0, 0]);
    assert_eq!(payload_of(&t2), [2, 0, 0, 0]);
    let (id1, id2) = (t1.uint(1).unwrap(), t2.uint(1).unwrap());
    conn.disposition(link.channel, true, id1, id1, true, accepted());
    // No credit left: 3 and 4 wait in the daemon's queue.
    dds.write_user_sample_borrowed(w, &[3, 0, 0, 0]).unwrap();
    dds.write_user_sample_borrowed(w, &[4, 0, 0, 0]).unwrap();
    let deadline = Instant::now() + STEP;
    while !registry
        .render_prometheus()
        .contains("\nzerodds_amqp_dds_samples_in_total 4\n")
    {
        assert!(
            Instant::now() < deadline,
            "the daemon did not take 3 and 4: {}",
            registry.render_prometheus()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    *current.lock().unwrap() = closed;
    flag.store(true, Ordering::SeqCst);
    loaded
        .recv_timeout(STEP)
        .expect("the daemon read the configuration");
    std::thread::sleep(Duration::from_millis(200));
    // The broker releases 2 and grants credit for ten.
    conn.disposition(
        link.channel,
        true,
        id2,
        id2,
        true,
        AmqpExtValue::Described {
            descriptor: 0x26,
            value: Box::new(AmqpExtValue::List(vec![])),
        },
    );
    conn.flow(&link, 2, 10);
    let deadline = Instant::now() + Duration::from_secs(1);
    while let Some(f) = conn.try_next(deadline.saturating_duration_since(Instant::now())) {
        assert_ne!(
            f.code,
            descriptor::TRANSFER,
            "a message of a withdrawn read right went out: {f:?}"
        );
    }
    drop(conn);
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap().unwrap();
    dds.shutdown();
}

/// §9.1: the DDS endpoints of steps 3 and 4 stand before the broker
/// connection of step 5. While nothing listens on the broker's port, a
/// second participant matches the daemon's reader and its writer; then the
/// broker comes up, and the daemon's frames follow steps 5 and 6: the SASL
/// header and init, the AMQP header and open (`handshake` checks these),
/// then per link its begin before its attach, nothing else in between.
#[test]
fn amqpbridge_9_1_the_dds_endpoints_stand_before_the_broker_is_contacted() {
    let port = Broker::free_port();
    let cfg = config(&format!(
        "domain: {}\namqp:\n  broker_url: \"amqp://127.0.0.1:{port}\"\n  container_id: \"zd-scripted\"\n  reconnect:\n    initial_delay_ms: 100\n    max_delay_ms: 200\ntopics:\n  - dds_name: S91O\n    amqp_address: /queues/s91o\n    direction: out\n  - dds_name: S91I\n    amqp_address: /queues/s91i\n    direction: in\ncatalog:\n  enabled: false\n",
        domain()
    ));
    let daemon = Daemon::start(cfg);
    let dds = participant();
    let _w = matched_writer(&dds, "S91O", false);
    let (r, _rx) = dds
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "S91I".into(),
                type_name: "S91I".into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let deadline = Instant::now() + STEP;
    while dds.user_reader_matched_count(r) == 0 {
        assert!(
            Instant::now() < deadline,
            "the daemon's writer did not match while the broker was down"
        );
        dds.wait_match_event(Duration::from_millis(50));
    }
    let broker = Broker::bind_port(port);
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let mut begun = Vec::new();
    let mut attached = Vec::new();
    while attached.len() < 2 {
        let f = conn.next();
        match f.code {
            descriptor::BEGIN => {
                conn.answer_begin(&f);
                begun.push(f.channel);
            }
            descriptor::ATTACH => {
                assert!(
                    begun.contains(&f.channel),
                    "attach on {} before its begin",
                    f.channel
                );
                attached.push(conn.answer_attach(&f, 0).name);
            }
            other => panic!("frame {other:#x} before the attaches"),
        }
    }
    attached.sort();
    assert_eq!(
        attached,
        ["zd-scripted/S91I/receiver", "zd-scripted/S91O/sender"]
    );
    drop(conn);
    daemon.stop().unwrap();
    dds.shutdown();
}

/// A daemon with one `out` topic `name` (KEEP_ALL, `drain_budget`)
/// counting into the returned metrics, its link attached; credit 100 when
/// `credit`, none otherwise.
fn draining_daemon(
    broker: &Broker,
    name: &str,
    drain_budget: Duration,
    credit: bool,
) -> (
    Daemon,
    BridgeMetrics,
    Arc<DcpsRuntime>,
    scripted_broker::Conn,
    Link,
    EntityId,
) {
    let mut cfg = config(&yaml(
        broker,
        "",
        &format!(
            "topics:\n  - dds_name: {name}\n    amqp_address: /queues/{name}\n    direction: out\n    qos:\n      history: {{ kind: keep_all }}\ncatalog:\n  enabled: false\n"
        ),
    ));
    cfg.drain_budget = drain_budget;
    let metrics = BridgeMetrics::register(&Registry::new());
    let daemon = Daemon::start_with(cfg, metrics.clone());
    let dds = participant();
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    if credit {
        conn.flow(&link, 0, 100);
    }
    let w = matched_writer(&dds, name, false);
    (daemon, metrics, dds, conn, link, w)
}

/// The detach of `link`, the end of its session and the close, in this
/// order, then the end of the stream (§9.2).
fn expect_detach_end_close(conn: &mut scripted_broker::Conn, link: &Link) {
    let detach = conn.next();
    assert_eq!(detach.code, descriptor::DETACH, "{detach:?}");
    assert_eq!(
        (detach.channel, detach.uint(0)),
        (link.channel, Some(link.handle))
    );
    assert_eq!(detach.boolean(1), Some(true), "closed");
    let end = conn.next();
    assert_eq!(end.code, descriptor::END, "{end:?}");
    assert_eq!(end.channel, link.channel);
    let close = conn.next();
    assert_eq!(close.code, descriptor::CLOSE, "{close:?}");
    assert_eq!(close.field(0), &AmqpExtValue::Null, "a close without error");
    conn.expect_eof(STEP);
}

/// §9.2: a stop waits for the broker's disposition of the delivery in
/// flight; then the daemon detaches the link, ends its session, closes the
/// connection, and the run ends without error.
#[test]
fn amqpbridge_9_2_the_drain_waits_for_the_disposition_then_detaches_ends_and_closes() {
    let broker = Broker::bind();
    let (daemon, metrics, dds, mut conn, link, w) =
        draining_daemon(&broker, "S92D", bridge_daemon::DRAIN_BUDGET, true);
    dds.write_user_sample_borrowed(w, &[1, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    assert_eq!(t.boolean(4), Some(false), "unsettled");
    let id = t.uint(1).unwrap();
    daemon.stop.store(true, Ordering::SeqCst);
    assert!(
        conn.try_next(Duration::from_millis(800)).is_none(),
        "nothing before the disposition"
    );
    conn.disposition(link.channel, true, id, id, true, accepted());
    expect_detach_end_close(&mut conn, &link);
    daemon.stop().unwrap();
    assert_eq!(metrics.errors_total.get(), 0, "nothing left behind");
    dds.shutdown();
}

/// §9.2: without a disposition the drain ends after its budget; the
/// delivery left unsettled is counted in `errors_total`, then the detach,
/// end and close follow.
#[test]
fn amqpbridge_9_2_the_drain_gives_up_after_its_budget_and_counts_what_is_left() {
    let broker = Broker::bind();
    let (daemon, metrics, dds, mut conn, link, w) =
        draining_daemon(&broker, "S92B", Duration::from_millis(600), true);
    dds.write_user_sample_borrowed(w, &[2, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    assert_eq!(t.boolean(4), Some(false), "unsettled");
    let before = metrics.errors_total.get();
    let stopped = Instant::now();
    daemon.stop.store(true, Ordering::SeqCst);
    expect_detach_end_close(&mut conn, &link);
    let took = stopped.elapsed();
    assert!(
        took >= Duration::from_millis(600) && took < Duration::from_secs(5),
        "the drain took {took:?}"
    );
    daemon.stop().unwrap();
    assert_eq!(
        metrics.errors_total.get() - before,
        1,
        "shutdown_with_unsettled"
    );
    dds.shutdown();
}

/// §9.2: once the stop is seen, a sample the DDS reader delivers is not
/// sent although the link has credit; it is counted (`shutdown_with_queued`).
#[test]
fn amqpbridge_9_2_a_sample_written_during_the_drain_is_not_sent() {
    let broker = Broker::bind();
    let (daemon, metrics, dds, mut conn, link, w) =
        draining_daemon(&broker, "S92S", bridge_daemon::DRAIN_BUDGET, true);
    dds.write_user_sample_borrowed(w, &[3, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    let id = t.uint(1).unwrap();
    daemon.stop.store(true, Ordering::SeqCst);
    // A frame sent after the stop: once the pump has counted it, it looks
    // at the stop before it takes the next event of its channel. The flow
    // states what the broker holds after one transfer, credit 99, and
    // changes nothing.
    let frames = metrics.frames_in_total.get();
    conn.flow(&link, 1, 99);
    wait_for("the pump took the flow after the stop", || {
        metrics.frames_in_total.get() > frames
    });
    dds.write_user_sample_borrowed(w, &[4, 0, 0, 0]).unwrap();
    assert!(
        dds.await_user_writer_acknowledged(w, Instant::now() + STEP),
        "the daemon's reader has sample 4"
    );
    assert!(
        no_transfer_within(&mut conn, Duration::from_millis(800)),
        "a sample after the stop was sent"
    );
    conn.disposition(link.channel, true, id, id, true, accepted());
    expect_detach_end_close(&mut conn, &link);
    daemon.stop().unwrap();
    assert_eq!(metrics.errors_total.get(), 1, "shutdown_with_queued");
    dds.shutdown();
}

/// §9.2: a message queued for credit when the stop comes is not sent; the
/// stop does not wait for credit, and the message is counted
/// (`shutdown_with_queued`).
#[test]
fn amqpbridge_9_2_a_message_waiting_for_credit_is_counted_at_the_stop() {
    let broker = Broker::bind();
    let (daemon, metrics, dds, mut conn, link, w) =
        draining_daemon(&broker, "S92Q", bridge_daemon::DRAIN_BUDGET, false);
    dds.write_user_sample_borrowed(w, &[5, 0, 0, 0]).unwrap();
    assert!(
        no_transfer_within(&mut conn, Duration::from_millis(500)),
        "sent without credit"
    );
    let stopped = Instant::now();
    daemon.stop.store(true, Ordering::SeqCst);
    expect_detach_end_close(&mut conn, &link);
    assert!(
        stopped.elapsed() < Duration::from_secs(3),
        "waited for credit"
    );
    daemon.stop().unwrap();
    assert_eq!(metrics.errors_total.get(), 1, "shutdown_with_queued");
    dds.shutdown();
}

/// Sends `link` a flow after the stop that states what the broker holds
/// (`delivery_count`, no credit) and changes nothing, and waits until the
/// pump has taken it: the pump then looks at the stop before it takes the
/// next frame (§9.2).
fn after_the_stop_seen(
    conn: &mut scripted_broker::Conn,
    link: &Link,
    metrics: &BridgeMetrics,
    delivery_count: u32,
) {
    let frames = metrics.frames_in_total.get();
    conn.flow(link, delivery_count, 0);
    wait_for("the pump took the flow after the stop", || {
        metrics.frames_in_total.get() > frames
    });
}

/// §9.2: a message queued for credit when the stop comes is not sent when
/// the broker grants credit during the drain; the drain still waits for
/// the delivery in flight, and the message is counted
/// (`shutdown_with_queued`).
#[test]
fn amqpbridge_9_2_credit_granted_during_the_drain_sends_no_queued_message() {
    let broker = Broker::bind();
    let (daemon, metrics, dds, mut conn, link, w) =
        draining_daemon(&broker, "S92C", bridge_daemon::DRAIN_BUDGET, false);
    conn.flow(&link, 0, 1);
    dds.write_user_sample_borrowed(w, &[7, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    let id = t.uint(1).unwrap();
    dds.write_user_sample_borrowed(w, &[8, 0, 0, 0]).unwrap();
    wait_for("the second sample queued for credit", || {
        metrics.dds_samples_in_total.get() == 2
    });
    daemon.stop.store(true, Ordering::SeqCst);
    after_the_stop_seen(&mut conn, &link, &metrics, 1);
    conn.flow(&link, 1, 5);
    assert!(
        no_transfer_within(&mut conn, Duration::from_millis(800)),
        "a message queued for credit before the stop was sent"
    );
    conn.disposition(link.channel, true, id, id, true, accepted());
    expect_detach_end_close(&mut conn, &link);
    daemon.stop().unwrap();
    assert_eq!(metrics.errors_total.get(), 1, "shutdown_with_queued");
    dds.shutdown();
}

/// §9.2, §4.5: a delivery the broker releases during the drain is settled
/// and goes back to the queue, where it waits for credit: it is not sent
/// again although the link has credit; the drain ends and the message is
/// counted (`shutdown_with_queued`).
#[test]
fn amqpbridge_9_2_a_message_released_during_the_drain_is_not_sent_again() {
    let broker = Broker::bind();
    let (daemon, metrics, dds, mut conn, link, w) =
        draining_daemon(&broker, "S92R", bridge_daemon::DRAIN_BUDGET, true);
    dds.write_user_sample_borrowed(w, &[9, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    let id = t.uint(1).unwrap();
    daemon.stop.store(true, Ordering::SeqCst);
    after_the_stop_seen(&mut conn, &link, &metrics, 1);
    conn.flow(&link, 1, 99);
    conn.disposition(link.channel, true, id, id, true, outcome(0x26, vec![]));
    expect_detach_end_close(&mut conn, &link);
    daemon.stop().unwrap();
    assert_eq!(metrics.errors_total.get(), 1, "shutdown_with_queued");
    dds.shutdown();
}

/// §9.2: a stop while no broker connection stands ends the run at once and
/// without error; the samples the DDS reader delivered meanwhile are one
/// `errors_total` (`shutdown_with_queued`; its fields are checked on the
/// program's log in `bridge_e2e`).
#[test]
fn amqpbridge_9_2_a_stop_without_a_broker_connection_counts_in_errors_total() {
    let port = Broker::free_port();
    let cfg = config(&format!(
        "domain: {}\namqp:\n  broker_url: \"amqp://127.0.0.1:{port}\"\n  container_id: \"zd-scripted-noconn\"\n  reconnect:\n    initial_delay_ms: 60000\n    max_delay_ms: 60000\ntopics:\n  - dds_name: S92N\n    amqp_address: /queues/s92n\n    direction: out\n    qos:\n      history: {{ kind: keep_all }}\ncatalog:\n  enabled: false\n",
        domain()
    ));
    let metrics = BridgeMetrics::register(&Registry::new());
    let daemon = Daemon::start_with(cfg, metrics.clone());
    let deadline = Instant::now() + STEP;
    while metrics.errors_total.get() == 0 {
        assert!(Instant::now() < deadline, "no failed connect");
        std::thread::sleep(Duration::from_millis(20));
    }
    let dds = participant();
    let w = matched_writer(&dds, "S92N", false);
    dds.write_user_sample_borrowed(w, &[6, 0, 0, 0]).unwrap();
    std::thread::sleep(Duration::from_millis(600));
    let before = metrics.errors_total.get();
    let stopped = Instant::now();
    daemon
        .stop()
        .expect("a stop without a connection is no error");
    assert!(
        stopped.elapsed() < Duration::from_secs(2),
        "the stop waited {:?}",
        stopped.elapsed()
    );
    assert_eq!(
        metrics.errors_total.get() - before,
        1,
        "shutdown_with_queued"
    );
    dds.shutdown();
}

/// §9.2: the SIGHUP watcher of `run_with_reload` ends with the run, also
/// when the run ends with an error and no stop (`once` against a port
/// nobody listens on, exit 2). A reload signalled afterwards is taken by
/// nobody: the flag stays set and the configuration is not read again.
#[test]
fn amqpbridge_9_2_the_reload_watcher_ends_with_the_run() {
    let port = Broker::free_port();
    let cfg = config(&format!(
        "domain: {}\namqp:\n  broker_url: \"amqp://127.0.0.1:{port}\"\n  container_id: \"zd-scripted-reload-end\"\ntopics:\n  - dds_name: S92R\n    amqp_address: /queues/s92r\n    direction: out\ncatalog:\n  enabled: false\n",
        domain()
    ));
    let flag = Arc::new(AtomicBool::new(false));
    let loads = Arc::new(AtomicUsize::new(0));
    let reload = {
        let (cfg, loads) = (cfg.clone(), Arc::clone(&loads));
        bridge_daemon::Reload {
            flag: Arc::clone(&flag),
            load: Box::new(move || {
                loads.fetch_add(1, Ordering::SeqCst);
                Ok(cfg.clone())
            }),
        }
    };
    let shutdown = Arc::new(AtomicBool::new(false));
    let e = bridge_daemon::run_with_reload(
        &cfg,
        RuntimeConfig::default(),
        &shutdown,
        &BridgeMetrics::register(&Registry::new()),
        true,
        Duration::from_secs(2),
        Some(reload),
        bridge_daemon::Tracer::default(),
    )
    .expect_err("nobody listens on the broker port");
    assert_eq!(e.code, 2, "{e}");
    flag.store(true, Ordering::SeqCst);
    // The watcher looked at the flag every 100 ms.
    std::thread::sleep(Duration::from_millis(400));
    assert!(
        flag.load(Ordering::SeqCst),
        "a watcher took the reload after the run"
    );
    assert_eq!(
        loads.load(Ordering::SeqCst),
        0,
        "the configuration was read again"
    );
}

/// PEM files in `dir`: a CA, a server certificate for `localhost` and two
/// client certificates `a` and `b`. Returns the DER of `a` and `b` and a
/// TLS server that requires a client certificate of the CA.
fn two_client_certificates(dir: &std::path::Path) -> (Vec<u8>, Vec<u8>, Arc<rustls::ServerConfig>) {
    use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
    std::fs::create_dir_all(dir).unwrap();
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca = ca_params.self_signed(&ca_key).unwrap();
    std::fs::write(dir.join("ca.pem"), ca.pem()).unwrap();
    let issue = |name: &str| {
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = rcgen::CertificateParams::new(vec![name.to_string()])
            .unwrap()
            .signed_by(&key, &ca, &ca_key)
            .unwrap();
        (cert, key)
    };
    let mut ders = Vec::new();
    for n in ["a", "b"] {
        let (cert, key) = issue(&format!("bridge-{n}.example"));
        std::fs::write(dir.join(format!("{n}.pem")), cert.pem()).unwrap();
        std::fs::write(dir.join(format!("{n}.key")), key.serialize_pem()).unwrap();
        ders.push(cert.der().to_vec());
    }
    let (server, server_key) = issue("localhost");
    let mut roots = rustls::RootCertStore::empty();
    roots.add(ca.der().clone()).unwrap();
    let provider = Arc::new(zerodds_bridge_security::tls_provider());
    let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
        Arc::new(roots),
        Arc::clone(&provider),
    )
    .build()
    .unwrap();
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_client_cert_verifier(verifier)
        .with_single_cert(
            vec![CertificateDer::from(server.der().to_vec())],
            PrivatePkcs8KeyDer::from(server_key.serialize_der()).into(),
        )
        .unwrap();
    cfg.alpn_protocols = vec![b"amqp".to_vec()];
    let b = ders.pop().unwrap();
    let a = ders.pop().unwrap();
    (a, b, Arc::new(cfg))
}

/// One AMQP frame of `body` on channel 0.
fn write_frame0(
    w: &mut impl std::io::Write,
    t: zerodds_amqp_bridge::frame::FrameType,
    body: &[u8],
) {
    use zerodds_amqp_bridge::frame::{FrameHeader, encode_frame_header};
    let h = encode_frame_header(FrameHeader {
        size: 8 + body.len() as u32,
        doff: 2,
        frame_type: t,
        channel: 0,
    });
    w.write_all(&h).unwrap();
    w.write_all(body).unwrap();
    w.flush().unwrap();
}

/// The performative code of the next frame.
fn read_code(r: &mut impl std::io::Read) -> u64 {
    let mut h = [0u8; 8];
    r.read_exact(&mut h).unwrap();
    let hdr = zerodds_amqp_bridge::frame::decode_frame_header(&h).unwrap();
    let mut body = vec![0u8; hdr.size as usize - 8];
    r.read_exact(&mut body).unwrap();
    zerodds_amqp_bridge::performatives::decode_performative(&body)
        .unwrap()
        .0
}

/// §9.2: SIGHUP gives the next connect the TLS material the reload read.
/// The broker side (TLS with a required client certificate) records the
/// daemon's certificate per connection: `a` on the first; after the
/// reload, which names `b`, the broker drops the connection, and the
/// daemon's next connection shows `b`.
#[test]
fn amqpbridge_9_2_a_reload_gives_the_next_connect_the_new_certificate() {
    use std::io::{Read, Write};
    use zerodds_amqp_bridge::frame::FrameType;
    use zerodds_amqp_bridge::performatives::encode_performative;
    let dir = std::env::temp_dir().join(format!("amqp-9-2-tls-{}", std::process::id()));
    let (der_a, der_b, server) = two_client_certificates(&dir);
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (leaf_tx, leaf_rx) = std::sync::mpsc::channel::<Vec<u8>>();
    let (drop_tx, drop_rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        for _ in 0..2 {
            let Ok((sock, _)) = listener.accept() else {
                return;
            };
            sock.set_read_timeout(Some(STEP)).unwrap();
            let conn = rustls::ServerConnection::new(Arc::clone(&server)).unwrap();
            let mut tls = rustls::StreamOwned::new(conn, sock);
            let mut hdr = [0u8; 8];
            if tls.read_exact(&mut hdr).is_err() {
                return;
            }
            let leaf = tls
                .conn
                .peer_certificates()
                .and_then(|c| c.first())
                .map(|c| c.to_vec())
                .unwrap_or_default();
            let _ = leaf_tx.send(leaf);
            tls.write_all(&hdr).unwrap();
            let p = |code, fields| encode_performative(code, &AmqpExtValue::List(fields)).unwrap();
            write_frame0(
                &mut tls,
                FrameType::Sasl,
                &p(
                    descriptor::SASL_MECHANISMS,
                    vec![AmqpExtValue::Array(vec![AmqpExtValue::Symbol(
                        "ANONYMOUS".into(),
                    )])],
                ),
            );
            assert_eq!(read_code(&mut tls), descriptor::SASL_INIT);
            write_frame0(
                &mut tls,
                FrameType::Sasl,
                &p(descriptor::SASL_OUTCOME, vec![AmqpExtValue::Ubyte(0)]),
            );
            let mut amqp = [0u8; 8];
            tls.read_exact(&mut amqp).unwrap();
            tls.write_all(&amqp).unwrap();
            assert_eq!(read_code(&mut tls), descriptor::OPEN);
            write_frame0(
                &mut tls,
                FrameType::Amqp,
                &p(
                    descriptor::OPEN,
                    vec![
                        AmqpExtValue::Str("tls-broker".into()),
                        AmqpExtValue::Null,
                        AmqpExtValue::Uint(65_536),
                    ],
                ),
            );
            let _ = drop_rx.recv_timeout(STEP);
            let _ = tls.sock.shutdown(std::net::Shutdown::Both);
        }
    });
    let yaml = |n: &str| {
        format!(
            "domain: {}\namqp:\n  broker_url: \"amqps://localhost:{port}\"\n  container_id: zd-tls-reload\n  reconnect:\n    initial_delay_ms: 100\n    max_delay_ms: 200\n  tls:\n    enabled: true\n    ca_file: \"{}\"\n    cert_file: \"{}\"\n    key_file: \"{}\"\ntopics:\n  - dds_name: S92T\n    amqp_address: /queues/s92t\n    direction: out\ncatalog:\n  enabled: false\n",
            domain(),
            dir.join("ca.pem").display(),
            dir.join(format!("{n}.pem")).display(),
            dir.join(format!("{n}.key")).display(),
        )
    };
    let started = config(&yaml("a"));
    let reloaded = config(&yaml("b"));
    let flag = Arc::new(AtomicBool::new(false));
    let reload = bridge_daemon::Reload {
        flag: Arc::clone(&flag),
        load: Box::new(move || Ok(reloaded.clone())),
    };
    let stop = Arc::new(AtomicBool::new(false));
    let s = Arc::clone(&stop);
    let join = std::thread::spawn(move || {
        let metrics = BridgeMetrics::register(&Registry::new());
        bridge_daemon::run_with_reload(
            &started,
            RuntimeConfig::default(),
            &s,
            &metrics,
            false,
            Duration::from_secs(5),
            Some(reload),
            bridge_daemon::Tracer::default(),
        )
    });
    let first = leaf_rx.recv_timeout(STEP).expect("the first connection");
    assert_eq!(first, der_a, "certificate a at start");
    flag.store(true, Ordering::SeqCst);
    // The reload thread looks at the flag every 100 ms.
    let deadline = Instant::now() + STEP;
    while flag.load(Ordering::SeqCst) {
        assert!(Instant::now() < deadline, "the reload did not run");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(200));
    drop_tx.send(()).unwrap();
    let second = leaf_rx.recv_timeout(STEP).expect("the second connection");
    assert_eq!(second, der_b, "certificate b after the reload");
    drop_tx.send(()).unwrap();
    stop.store(true, Ordering::SeqCst);
    join.join().unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

/// An answer of `$cbs` to request `id` with `status`.
fn cbs_answer(id: u64, status: i32) -> Vec<u8> {
    let mut m = MessageSection::Properties(AmqpExtValue::List(vec![
        AmqpExtValue::Null,
        AmqpExtValue::Null,
        AmqpExtValue::Null,
        AmqpExtValue::Null,
        AmqpExtValue::Null,
        AmqpExtValue::Ulong(id),
    ]))
    .encode()
    .unwrap();
    m.extend_from_slice(
        &MessageSection::ApplicationProperties(AmqpExtValue::Map(vec![
            (
                AmqpExtValue::Str("status-code".into()),
                AmqpExtValue::Int(status),
            ),
            (
                AmqpExtValue::Str("status-description".into()),
                AmqpExtValue::Str(
                    if status == 202 {
                        "Accepted"
                    } else {
                        "Unauthorized"
                    }
                    .into(),
                ),
            ),
        ]))
        .encode()
        .unwrap(),
    );
    m
}

/// A put-token request of the daemon: message-id, reply-to, the
/// application properties, the token.
struct PutToken {
    id: u64,
    reply_to: String,
    operation: String,
    token_type: String,
    name: String,
    token: String,
}

fn put_token_of(transfer: &Frame) -> PutToken {
    let mut p = PutToken {
        id: 0,
        reply_to: String::new(),
        operation: String::new(),
        token_type: String::new(),
        name: String::new(),
        token: String::new(),
    };
    let mut rest = &transfer.payload[..];
    while !rest.is_empty() {
        let (s, n) = MessageSection::decode(rest).unwrap();
        rest = &rest[n..];
        match s {
            MessageSection::Properties(AmqpExtValue::List(f)) => {
                if let Some(AmqpExtValue::Ulong(id)) = f.first() {
                    p.id = *id;
                }
                if let Some(AmqpExtValue::Str(r)) = f.get(4) {
                    p.reply_to = r.clone();
                }
            }
            MessageSection::ApplicationProperties(AmqpExtValue::Map(m)) => {
                for (k, v) in m {
                    if let (AmqpExtValue::Str(k), AmqpExtValue::Str(v)) = (k, v) {
                        match k.as_str() {
                            "operation" => p.operation = v,
                            "type" => p.token_type = v,
                            "name" => p.name = v,
                            _ => {}
                        }
                    }
                }
            }
            MessageSection::AmqpValue(AmqpExtValue::Str(t)) => p.token = t,
            _ => {}
        }
    }
    p
}

/// The value of `field` (`sr`, `sig`, `se`, `skn`) of a SAS token.
fn sas_field<'a>(token: &'a str, field: &str) -> &'a str {
    token
        .trim_start_matches("SharedAccessSignature ")
        .split('&')
        .find_map(|kv| kv.strip_prefix(&format!("{field}=")))
        .unwrap_or_else(|| panic!("no {field} in {token}"))
}

/// The address of a terminus field of an attach (source or target).
fn terminus_address(v: &AmqpExtValue) -> Option<String> {
    match v {
        AmqpExtValue::Described { value, .. } => match &**value {
            AmqpExtValue::List(f) => match f.first() {
                Some(AmqpExtValue::Str(s)) => Some(s.clone()),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

/// A daemon with CBS on (`amqp.cbs`, key name `kn`, key `secret`,
/// `ttl_s`) and one `out` topic on `/queues/s10c`; the broker side
/// has answered the CBS session's begin and attaches and granted credit.
/// Returns the CBS sender and receiver as the broker sees them.
fn cbs_daemon(broker: &Broker, ttl_s: u32) -> (Daemon, scripted_broker::Conn, Link, Link) {
    let cfg = config(&yaml(
        broker,
        &format!(
            "  cbs:\n    enabled: true\n    key_name: kn\n    secret: secret\n    ttl_s: {ttl_s}\n"
        ),
        "topics:\n  - dds_name: S10C\n    amqp_address: /queues/s10c\n    direction: out\ncatalog:\n  enabled: false\n",
    ));
    let daemon = Daemon::start(cfg);
    let mut conn = broker.accept(STEP);
    conn.handshake();
    let begin = conn.next();
    assert_eq!(begin.code, descriptor::BEGIN, "{begin:?}");
    assert_eq!(begin.channel, 1, "the CBS session follows the link's");
    conn.answer_begin(&begin);
    let mut sender = None;
    let mut receiver = None;
    while sender.is_none() || receiver.is_none() {
        let f = conn.next();
        assert_eq!(
            f.code,
            descriptor::ATTACH,
            "only the CBS links first: {f:?}"
        );
        let link = conn.answer_attach(&f, 0);
        assert_eq!(link.address, "$cbs", "{f:?}");
        if link.daemon_sends {
            sender = Some(link);
        } else {
            assert_eq!(
                terminus_address(f.field(6)).as_deref(),
                Some("zd-scripted/$cbs/reply"),
                "the receiver's target is the reply-to"
            );
            receiver = Some(link);
        }
    }
    let (sender, receiver) = (sender.unwrap(), receiver.unwrap());
    let flow = conn.next_of(descriptor::FLOW);
    assert_eq!(
        flow.uint(4),
        Some(receiver.handle),
        "credit for the answers"
    );
    conn.flow(&sender, 0, 10);
    (daemon, conn, sender, receiver)
}

fn hmac_sig(sr: &str, se: &str, key: &str) -> String {
    use base64::Engine as _;
    use hmac::Mac as _;
    let mut m = <hmac::Hmac<sha2::Sha256> as hmac::Mac>::new_from_slice(key.as_bytes()).unwrap();
    m.update(format!("{sr}\n{se}").as_bytes());
    let b64 = base64::engine::general_purpose::STANDARD.encode(m.finalize().into_bytes());
    b64.replace('+', "%2B").replace('=', "%3D")
}

/// §7.2 (CBS): before the topic link is begun the daemon puts a SAS token
/// for its address on `$cbs` (operation `put-token`, the token type of
/// Azure Service Bus, the audience `amqp://<host>/<address>`, the
/// `reply-to` of its receiver); the link follows the broker's 202 only.
/// At four fifths of the token's lifetime (10 s) a new token goes out on
/// the same links.
#[test]
fn amqpbridge_10_cbs_a_token_is_put_before_the_link_and_renewed() {
    let broker = Broker::bind();
    let (daemon, mut conn, sender, receiver) = cbs_daemon(&broker, 10);
    let t = conn.next_of(descriptor::TRANSFER);
    assert_eq!(
        (t.channel, t.uint(0)),
        (sender.channel, Some(sender.handle))
    );
    let put = put_token_of(&t);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert_eq!(put.operation, "put-token");
    assert_eq!(put.token_type, "servicebus.windows.net:sastoken");
    assert_eq!(put.name, "amqp://127.0.0.1/queues/s10c");
    assert_eq!(put.reply_to, "zd-scripted/$cbs/reply");
    let sr = sas_field(&put.token, "sr");
    assert_eq!(sr, "amqp%3A%2F%2F127.0.0.1%2Fqueues%2Fs10c");
    assert_eq!(sas_field(&put.token, "skn"), "kn");
    let se: u64 = sas_field(&put.token, "se").parse().unwrap();
    assert!((now + 8..=now + 12).contains(&se), "se {se} for now {now}");
    assert_eq!(
        sas_field(&put.token, "sig"),
        hmac_sig(sr, &se.to_string(), "secret")
    );
    assert!(
        conn.try_next(Duration::from_millis(400)).is_none(),
        "nothing of the topic link before the answer"
    );
    let answered = Instant::now();
    conn.transfer(&receiver, 0, true, &cbs_answer(put.id, 202));
    let links = conn.setup_links(1, 0);
    assert_eq!(links[0].channel, 0);
    assert_eq!(links[0].address, "/queues/s10c");
    let again = conn.next_of(descriptor::TRANSFER);
    let waited = answered.elapsed();
    assert_eq!(
        (again.channel, again.uint(0)),
        (sender.channel, Some(sender.handle))
    );
    assert!(
        waited >= Duration::from_secs(7) && waited < Duration::from_secs(10),
        "renewed after {waited:?}"
    );
    let renewed = put_token_of(&again);
    assert!(renewed.id > put.id);
    let se2: u64 = sas_field(&renewed.token, "se").parse().unwrap();
    assert!(se2 > se, "a later expiry: {se} -> {se2}");
    conn.transfer(&receiver, 1, true, &cbs_answer(renewed.id, 200));
    drop(conn);
    daemon.stop().unwrap();
}

/// §7.2 (CBS): a token the broker refuses (401) ends the connection with
/// the broker's status before the topic link is begun; the daemon connects
/// again after its back-off.
#[test]
fn amqpbridge_10_cbs_a_refused_token_ends_the_connection_before_the_link() {
    let broker = Broker::bind();
    let (daemon, mut conn, _sender, receiver) = cbs_daemon(&broker, 3600);
    let put = put_token_of(&conn.next_of(descriptor::TRANSFER));
    conn.transfer(&receiver, 0, true, &cbs_answer(put.id, 401));
    let mut seen = Vec::new();
    let close = loop {
        let f = conn.next();
        if f.code == descriptor::CLOSE {
            break f;
        }
        seen.push(f);
    };
    assert!(
        seen.iter().all(|f| f.channel == 1),
        "no frame of the topic link: {seen:?}"
    );
    let text = format!("{close:?}");
    assert!(text.contains("401"), "{text}");
    drop(conn);
    let mut again = broker.accept(STEP);
    again.handshake();
    assert_eq!(again.next().code, descriptor::BEGIN);
    drop(again);
    daemon.stop().unwrap();
}

/// The frames up to the daemon's close, within `within`; panics without a
/// close.
fn frames_to_close(conn: &mut scripted_broker::Conn, within: Duration) -> (Vec<Frame>, Frame) {
    let deadline = Instant::now() + within;
    let mut seen = Vec::new();
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let f = conn
            .try_next(left)
            .unwrap_or_else(|| panic!("no close within {within:?}: {seen:?}"));
        if f.code == descriptor::CLOSE {
            return (seen, f);
        }
        seen.push(f);
    }
}

/// §7.2 (CBS): a put-token the broker never answers ends the connection
/// once the round's bound has passed (the connect time-out, 5 s here),
/// without a frame of the topic link; the daemon connects again after its
/// back-off.
#[test]
fn amqpbridge_10_cbs_a_put_token_without_answer_ends_the_connection() {
    let broker = Broker::bind();
    let (daemon, mut conn, _sender, _receiver) = cbs_daemon(&broker, 3600);
    let put = conn.next_of(descriptor::TRANSFER);
    let sent = Instant::now();
    let _ = put_token_of(&put);
    let (seen, close) = frames_to_close(&mut conn, Duration::from_secs(8));
    let waited = sent.elapsed();
    assert!(
        waited >= Duration::from_secs(3) && waited < Duration::from_secs(7),
        "closed after {waited:?}"
    );
    assert!(
        seen.iter().all(|f| f.channel == 1),
        "no frame of the topic link: {seen:?}"
    );
    let text = format!("{close:?}");
    assert!(
        text.contains("put-token round not answered within 5000 ms"),
        "{text}"
    );
    drop(conn);
    let mut again = broker.accept(STEP);
    again.handshake();
    assert_eq!(again.next().code, descriptor::BEGIN);
    drop(again);
    daemon.stop().unwrap();
}

/// §7.2 (CBS): a renewal round the broker never answers ends the connection
/// too: after the first token was accepted and the link attached, the
/// renewal at four fifths of 10 s gets no answer, and the close follows
/// within the bound of 5 s.
#[test]
fn amqpbridge_10_cbs_a_renewal_without_answer_ends_the_connection() {
    let broker = Broker::bind();
    let (daemon, mut conn, _sender, receiver) = cbs_daemon(&broker, 10);
    let put = put_token_of(&conn.next_of(descriptor::TRANSFER));
    conn.transfer(&receiver, 0, true, &cbs_answer(put.id, 202));
    conn.setup_links(1, 0);
    let again = conn.next_of(descriptor::TRANSFER);
    let renewal_sent = Instant::now();
    assert!(put_token_of(&again).id > put.id, "a renewal");
    let (_, close) = frames_to_close(&mut conn, Duration::from_secs(8));
    let waited = renewal_sent.elapsed();
    assert!(
        waited >= Duration::from_secs(4) && waited < Duration::from_secs(7),
        "closed after {waited:?}"
    );
    let text = format!("{close:?}");
    assert!(
        text.contains("put-token round not answered within 5000 ms"),
        "{text}"
    );
    drop(conn);
    daemon.stop().unwrap();
}
