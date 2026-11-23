// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The cases of the broker matrix (`zerodds-amqp-bridge-daemon-1.0` §10):
//! the bridge daemon in the test process with a DDS participant of the test
//! on one side, a broker on the other, and qpid-proton's Python binding
//! (`proton_peer.py`) as the AMQP 1.0 client at the broker. Each suite
//! (`bridge_<broker>_e2e.rs`) implements [`MatrixBroker`] for its broker and
//! runs the cases against it:
//!
//! * DDS → AMQP (§4.3), [`samples_arrive_as_pass_through_messages`];
//! * AMQP → DDS (§4.4),
//!   [`a_message_reaches_a_reader_and_a_bad_body_is_rejected`];
//! * Part 2 §2.6.14, [`a_multi_frame_sample_crosses_in_both_directions`];
//! * §9.3, [`the_bridge_reconnects_after_the_broker_drops_the_connection`];
//! * §4.2, [`a_refused_address_leaves_the_other_links_running`].

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use zerodds_amqp_bridge::sections::MessageSection;
use zerodds_amqp_endpoint::bridge_daemon::{self, DaemonConfig, OutSample, encode_message};
use zerodds_amqp_endpoint::coexistence::CoexistenceConfig;
use zerodds_amqp_endpoint::daemon_runtime::BridgeMetrics;
use zerodds_amqp_endpoint::properties::DdsOperation;
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_monitor::Registry;
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

thread_local! {
    /// The domain of the running test, leased host-wide.
    static DOMAIN: zerodds_test_domains::DomainLease = zerodds_test_vendor_reader::lease_domain();
}

pub fn domain() -> i32 {
    DOMAIN.with(zerodds_test_domains::DomainLease::id)
}

/// How long a step may take before the test fails.
pub const STEP: Duration = Duration::from_secs(10);

/// A broker of the matrix.
pub trait MatrixBroker {
    /// The lines below `amqp:` that reach the broker: `broker_url`, `sasl`.
    fn amqp_yaml(&self) -> String;
    /// The URL qpid-proton connects to.
    fn peer_url(&self) -> String;
    /// The address of the node `name` in the broker's form (§5.1).
    fn address(&self, name: &str) -> String;
    /// Whether the broker keeps a message for a receiver that attaches
    /// later. A router does not: its receiver attaches first, and it passes
    /// the bridge's outcome back to the sender.
    fn stores(&self) -> bool;
    /// Makes the broker end the bridge's connection (and the peer's).
    fn drop_connections(&mut self);
    /// An address whose attach the broker refuses.
    fn refused_address(&self) -> String;
    /// The direction of the bridge's link to [`MatrixBroker::refused_address`]:
    /// `out` (the bridge's sender is refused), or `in` for a broker that
    /// attaches every sender and refuses a receiver (Solace).
    fn refused_direction(&self) -> &'static str {
        "out"
    }
}

/// A message qpid-proton took from the broker.
#[derive(Debug, Clone)]
pub struct PeerMessage {
    pub content_type: String,
    pub body: Vec<u8>,
    pub durable: bool,
    /// The application properties, `key=value` joined by `,`.
    pub properties: String,
}

/// qpid-proton on one connection to the broker.
pub struct ProtonPeer {
    child: Child,
    stdin: ChildStdin,
    lines: mpsc::Receiver<String>,
}

impl ProtonPeer {
    /// Connects to `url`; panics when the binding or the broker is missing.
    pub fn connect(url: &str) -> Self {
        let script = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/support/proton_peer.py");
        let mut child = Command::new("python3")
            .arg(script)
            .arg(url)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("python3 (qpid-proton: tests/integration/provision.sh proton)");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for l in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(l).is_err() {
                    break;
                }
            }
        });
        let mut peer = Self {
            child,
            stdin,
            lines,
        };
        assert_eq!(
            peer.line(Duration::from_secs(25)),
            "connected",
            "qpid-proton at {url}"
        );
        peer
    }

    fn line(&mut self, within: Duration) -> String {
        self.lines
            .recv_timeout(within)
            .unwrap_or_else(|_| panic!("qpid-proton gave no answer within {within:?}"))
    }

    fn command(&mut self, line: &str) {
        writeln!(self.stdin, "{line}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// A receiver from `address` with `credit`.
    pub fn receiver(&mut self, address: &str, credit: u32) {
        self.command(&format!("receiver {address} {credit}"));
        assert_eq!(self.line(STEP + STEP), "attached", "receiver {address}");
    }

    /// Up to `n` messages from `address` within `within`, each accepted.
    pub fn take(&mut self, address: &str, n: usize, within: Duration) -> Vec<PeerMessage> {
        self.command(&format!("take {address} {n} {}", within.as_secs_f64()));
        let mut out = Vec::new();
        loop {
            let l = self.line(within + STEP);
            if l.starts_with("done ") {
                return out;
            }
            let mut f = l.splitn(5, ' ');
            assert_eq!(f.next(), Some("message"), "{l}");
            let content_type = f.next().unwrap_or_default().to_string();
            let body_hex = f.next().unwrap_or_default();
            assert!(
                !body_hex.starts_with('?'),
                "a body that is no data section: {l}"
            );
            let body = (0..body_hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&body_hex[i..i + 2], 16).unwrap())
                .collect();
            let durable = f.next() == Some("True");
            let properties = f.next().unwrap_or_default().to_string();
            out.push(PeerMessage {
                content_type,
                body,
                durable,
                properties,
            });
        }
    }

    /// Sends the encoded `message` to `address`; the outcome as the line
    /// after `outcome ` (`accepted`, `rejected <condition>`, …).
    pub fn send(&mut self, address: &str, message: &[u8]) -> String {
        let hex: String = message.iter().map(|b| format!("{b:02x}")).collect();
        self.command(&format!("send {address} {hex}"));
        let l = self.line(STEP + STEP);
        l.strip_prefix("outcome ")
            .unwrap_or_else(|| panic!("{l}"))
            .to_string()
    }
}

impl Drop for ProtonPeer {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn config(yaml: &str) -> DaemonConfig {
    let mut c = zerodds_config::Config::default();
    c.push(
        zerodds_config::parse_str(yaml, "<test>").unwrap(),
        "<test>".into(),
    )
    .unwrap();
    DaemonConfig::from_config(Some(&c)).unwrap()
}

/// The daemon in a thread until the flag is set, its metrics in the
/// registry.
pub struct Bridge {
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
    pub registry: Arc<Registry>,
}

impl Bridge {
    pub fn start(cfg: DaemonConfig) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let registry = Arc::new(Registry::new());
        let join = {
            let (s, r) = (Arc::clone(&stop), Arc::clone(&registry));
            std::thread::spawn(move || {
                let metrics = BridgeMetrics::register_shared(&r);
                let res = bridge_daemon::run(
                    &cfg,
                    RuntimeConfig::default(),
                    &s,
                    &metrics,
                    false,
                    Duration::from_secs(5),
                );
                assert!(res.is_ok(), "bridge stopped: {res:?}");
            })
        };
        Self {
            stop,
            join: Some(join),
            registry,
        }
    }

    /// The value of the metric line that begins with `line_start`.
    pub fn metric(&self, line_start: &str) -> Option<u64> {
        zerodds_monitor::render_prometheus(&self.registry.snapshot())
            .lines()
            .find(|l| l.starts_with(line_start))
            .and_then(|l| l.rsplit(' ').next())
            .and_then(|v| v.parse().ok())
    }

    /// Waits until the metric `line_start` reaches `want`.
    pub fn wait_metric(&self, line_start: &str, want: u64) {
        let deadline = Instant::now() + STEP;
        while self.metric(line_start) != Some(want) {
            assert!(
                Instant::now() < deadline,
                "{line_start} did not reach {want}: {:?}",
                self.metric(line_start)
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    pub fn stop(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            j.join().unwrap();
        }
    }
}

impl Drop for Bridge {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

pub fn participant() -> Arc<DcpsRuntime> {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[4..8].copy_from_slice(&NEXT.fetch_add(1, Ordering::Relaxed).to_be_bytes());
    prefix[11] = 0x5c;
    DcpsRuntime::start(
        domain(),
        GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .unwrap()
}

/// A writer of the test on `topic`, matched with the bridge's reader.
pub fn matched_writer(dds: &DcpsRuntime, topic: &str) -> EntityId {
    let eid = dds
        .register_user_writer_kind(
            UserWriterConfig {
                topic_name: topic.into(),
                type_name: format!("{topic}Type"),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let deadline = Instant::now() + STEP;
    while dds.user_writer_matched_count(eid) == 0 {
        assert!(Instant::now() < deadline, "the bridge's reader of {topic}");
        dds.wait_match_event(Duration::from_millis(50));
    }
    eid
}

/// A reader of the test on `topic`, matched with the bridge's writer.
pub fn matched_reader(dds: &DcpsRuntime, topic: &str) -> mpsc::Receiver<UserSample> {
    let (eid, rx) = dds
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: topic.into(),
                type_name: format!("{topic}Type"),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let deadline = Instant::now() + STEP;
    while dds.user_reader_matched_count(eid) == 0 {
        assert!(Instant::now() < deadline, "the bridge's writer of {topic}");
        dds.wait_match_event(Duration::from_millis(50));
    }
    rx
}

/// The payloads of the samples that reach `rx` within `within`, up to `n`.
pub fn received(rx: &mpsc::Receiver<UserSample>, n: usize, within: Duration) -> Vec<Vec<u8>> {
    let deadline = Instant::now() + within;
    let mut out = Vec::new();
    while out.len() < n {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(UserSample::Alive { payload, .. }) => out.push(payload.as_ref().to_vec()),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    out
}

/// A topic entry of the daemon's configuration.
pub fn topic(name: &str, address: &str, direction: &str) -> String {
    format!(
        "  - dds_name: {name}\n    dds_type: {name}Type\n    amqp_address: \"{address}\"\n    direction: {direction}\n    qos:\n      history: {{ kind: keep_all }}\n"
    )
}

/// The daemon's configuration towards `b` with the `topics` entries and
/// `extra` lines below `amqp:`.
pub fn bridge_yaml(b: &impl MatrixBroker, id: &str, extra: &str, topics: &str) -> String {
    format!(
        "domain: {}\nlog_level: debug\namqp:\n  container_id: \"zd-matrix-{id}-{}\"\n  reconnect:\n    initial_delay_ms: 200\n    max_delay_ms: 1000\n{}{extra}topics:\n{topics}catalog:\n  enabled: false\n",
        domain(),
        std::process::id(),
        b.amqp_yaml()
    )
}

/// The message another bridge sends for `payload` (XCDR2, a foreign
/// `dds:bridge-id`).
pub fn foreign_message(cfg: &DaemonConfig, topic: usize, payload: &[u8]) -> Vec<u8> {
    encode_message(
        &cfg.topics[topic],
        domain(),
        &CoexistenceConfig::new("99999999-2222-4333-8444-555555555555".into()),
        &OutSample {
            payload,
            representation_identifier: [0x00, 0x07],
            key_hash: None,
            writer_guid: [0x42; 16],
            sequence: 1,
            source_timestamp_ns: None,
            operation: DdsOperation::Write,
            lifespan_ms: None,
            partitions: &[],
        },
    )
    .unwrap()
}

/// A message whose body is no DDS sample: a data section `xy` with the
/// content type of XCDR2.
pub fn bad_message() -> Vec<u8> {
    use zerodds_amqp_bridge::extended_types::AmqpExtValue;
    let mut out = MessageSection::Properties(AmqpExtValue::List(vec![
        AmqpExtValue::Null,
        AmqpExtValue::Null,
        AmqpExtValue::Null,
        AmqpExtValue::Null,
        AmqpExtValue::Null,
        AmqpExtValue::Null,
        AmqpExtValue::Symbol("application/vnd.dds.xcdr2".into()),
    ]))
    .encode()
    .unwrap();
    out.extend_from_slice(&MessageSection::Data(b"xy".to_vec()).encode().unwrap());
    out
}

/// §4.3: three samples written in DDS arrive at the peer as pass-through
/// messages: the body the encapsulation header and the payload, the
/// content type naming the representation, the `dds:*` application
/// properties with this bridge's `dds:bridge-id`; each delivery ends
/// `accepted` at the bridge.
pub fn samples_arrive_as_pass_through_messages(b: &mut impl MatrixBroker) {
    let out = b.address("zd_out");
    let mut peer = ProtonPeer::connect(&b.peer_url());
    peer.receiver(&out, 10);
    let bridge = Bridge::start(config(&bridge_yaml(
        b,
        "out",
        "",
        &topic("MatrixOut", &out, "out"),
    )));
    let dds = participant();
    let w = matched_writer(&dds, "MatrixOut");
    for n in 1u8..=3 {
        dds.write_user_sample(w, vec![n, n, n, n]).unwrap();
    }
    let got = peer.take(&out, 3, STEP);
    assert_eq!(got.len(), 3, "{got:?}");
    // The outcome of each round trip reaches the bridge: the broker's, or
    // through a router the peer's.
    bridge.wait_metric("zerodds_amqp_bridge_dispositions_accepted_total", 3);
    bridge.stop();
    dds.shutdown();
    for (i, m) in got.iter().enumerate() {
        let n = (i + 1) as u8;
        assert_eq!(&m.body[4..], &[n, n, n, n], "{m:?}");
        match &m.body[..2] {
            [0x00, 0x01] => assert_eq!(m.content_type, "application/vnd.dds.xcdr1"),
            [0x00, 0x07] => assert_eq!(m.content_type, "application/vnd.dds.xcdr2"),
            other => panic!("encapsulation {other:?}: {m:?}"),
        }
        for key in ["dds:bridge-id=", "dds:domain-id=", "dds:bridge-hop="] {
            assert!(m.properties.contains(key), "{key} in {m:?}");
        }
    }
}

/// §4.4: a message qpid-proton sends reaches a DDS reader and is accepted;
/// a body that is no DDS sample is rejected (`amqp:decode-error`) and
/// reaches no reader. A router passes the bridge's outcome to the sender;
/// a broker that stores takes the message itself.
pub fn a_message_reaches_a_reader_and_a_bad_body_is_rejected(b: &mut impl MatrixBroker) {
    let inbound = b.address("zd_in");
    let cfg = config(&bridge_yaml(
        b,
        "in",
        "",
        &topic("MatrixIn", &inbound, "in"),
    ));
    let good = foreign_message(&cfg, 0, &[0x2a, 0, 0, 0]);
    let dds = participant();
    let bridge = Bridge::start(cfg);
    let rx = matched_reader(&dds, "MatrixIn");
    let mut peer = ProtonPeer::connect(&b.peer_url());
    assert_eq!(peer.send(&inbound, &good), "accepted");
    let first = received(&rx, 1, STEP);
    let bad = peer.send(&inbound, &bad_message());
    if b.stores() {
        assert_eq!(bad, "accepted", "the broker takes the message");
    } else {
        assert_eq!(bad, "rejected amqp:decode-error", "the bridge's outcome");
    }
    bridge.wait_metric(
        "zerodds_amqp_bridge_dispositions_rejected_total{reason=\"amqp:decode-error\"}",
        1,
    );
    let extra = received(&rx, 1, Duration::from_millis(500));
    bridge.stop();
    dds.shutdown();
    assert_eq!(first, vec![vec![0x2a, 0, 0, 0]], "the one DDS sample");
    assert!(extra.is_empty(), "the bad body reached DDS: {extra:?}");
}

/// Part 2 §2.6.14, §4.3: a 20 000-byte sample at a frame size of 4096
/// crosses as a multi-frame transfer from DDS to the peer and, sent back
/// by the peer as the message of another bridge, from the broker to DDS.
pub fn a_multi_frame_sample_crosses_in_both_directions(b: &mut impl MatrixBroker) {
    let (out, inbound) = (b.address("zd_big_out"), b.address("zd_big_in"));
    let payload: Vec<u8> = (0..20_000u32).map(|i| (i % 251) as u8).collect();
    let topics = format!(
        "{}{}",
        topic("MatrixBigOut", &out, "out"),
        topic("MatrixBigIn", &inbound, "in")
    );
    let cfg = config(&bridge_yaml(b, "big", "  max_frame_size: 4096\n", &topics));
    let back = foreign_message(&cfg, 1, &payload);
    let mut peer = ProtonPeer::connect(&b.peer_url());
    peer.receiver(&out, 10);
    let dds = participant();
    let bridge = Bridge::start(cfg);
    let w = matched_writer(&dds, "MatrixBigOut");
    let rx = matched_reader(&dds, "MatrixBigIn");
    dds.write_user_sample(w, payload.clone()).unwrap();
    let got = peer.take(&out, 1, STEP);
    assert_eq!(got.len(), 1, "the large sample at the peer");
    assert_eq!(
        got[0].body.len(),
        20_004,
        "header and payload in one message"
    );
    assert_eq!(&got[0].body[4..], &payload[..]);
    assert_eq!(peer.send(&inbound, &back), "accepted");
    let received = received(&rx, 1, STEP);
    bridge.stop();
    dds.shutdown();
    assert_eq!(received, vec![payload], "the same 20 000 bytes in DDS");
}

/// §9.3: the broker ends the bridge's connection; the bridge connects
/// again, attaches again and delivers the sample written meanwhile.
pub fn the_bridge_reconnects_after_the_broker_drops_the_connection(b: &mut impl MatrixBroker) {
    let out = b.address("zd_reconnect");
    let mut peer = ProtonPeer::connect(&b.peer_url());
    peer.receiver(&out, 10);
    let bridge = Bridge::start(config(&bridge_yaml(
        b,
        "reconnect",
        "",
        &topic("MatrixReconnect", &out, "out"),
    )));
    let dds = participant();
    let w = matched_writer(&dds, "MatrixReconnect");
    dds.write_user_sample(w, vec![1, 1, 1, 1]).unwrap();
    let first = peer.take(&out, 1, STEP);
    drop(peer);
    let connections = bridge
        .metric("zerodds_amqp_bridge_connections_total{state=\"established\"}")
        .unwrap_or(0);
    b.drop_connections();
    dds.write_user_sample(w, vec![2, 2, 2, 2]).unwrap();
    let mut peer = ProtonPeer::connect(&b.peer_url());
    peer.receiver(&out, 10);
    let second = peer.take(&out, 1, Duration::from_secs(20));
    let again = bridge
        .metric("zerodds_amqp_bridge_connections_total{state=\"established\"}")
        .unwrap_or(0);
    bridge.stop();
    dds.shutdown();
    assert_eq!(
        first
            .iter()
            .map(|m| m.body[4..].to_vec())
            .collect::<Vec<_>>(),
        [vec![1u8; 4]]
    );
    assert_eq!(
        second
            .iter()
            .map(|m| m.body[4..].to_vec())
            .collect::<Vec<_>>(),
        [vec![2u8; 4]],
        "the sample written after the drop, after the reconnect"
    );
    assert!(
        again > connections,
        "a new connection: {connections} -> {again}"
    );
}

/// §4.2: the broker refuses the attach of one link; the other link of the
/// same connection keeps carrying samples. The refused link is a sender
/// or, where the broker attaches every sender, a receiver
/// ([`MatrixBroker::refused_direction`]).
pub fn a_refused_address_leaves_the_other_links_running(b: &mut impl MatrixBroker) {
    let (out, refused) = (b.address("zd_beside"), b.refused_address());
    let direction = b.refused_direction();
    let topics = format!(
        "{}{}",
        topic("MatrixRefused", &refused, direction),
        topic("MatrixBeside", &out, "out")
    );
    let mut peer = ProtonPeer::connect(&b.peer_url());
    peer.receiver(&out, 10);
    let bridge = Bridge::start(config(&bridge_yaml(b, "refused", "", &topics)));
    let dds = participant();
    if direction == "out" {
        let w_refused = matched_writer(&dds, "MatrixRefused");
        dds.write_user_sample(w_refused, vec![7, 7, 7, 7]).unwrap();
    }
    let w = matched_writer(&dds, "MatrixBeside");
    dds.write_user_sample(w, vec![8, 8, 8, 8]).unwrap();
    let got = peer.take(&out, 2, Duration::from_secs(3));
    let senders = bridge.metric("zerodds_amqp_bridge_links_active{role=\"sender\"}");
    let receivers = bridge.metric("zerodds_amqp_bridge_links_active{role=\"receiver\"}");
    bridge.stop();
    dds.shutdown();
    assert_eq!(
        got.iter().map(|m| m.body[4..].to_vec()).collect::<Vec<_>>(),
        [vec![8u8; 4]],
        "the sample of the running link, nothing else"
    );
    assert_eq!(
        senders,
        Some(1),
        "one sender link up (the refused one down when it is a sender)"
    );
    if direction == "in" {
        assert_eq!(receivers.unwrap_or(0), 0, "the refused receiver link down");
    }
}
