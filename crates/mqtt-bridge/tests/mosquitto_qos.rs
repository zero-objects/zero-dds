// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! MQTT 5.0 §4.3 QoS flows of the bridge's client against Mosquitto:
//! inbound QoS 1/2 acknowledged, outbound QoS 2 completed, and the
//! shutdown of spec §9.2 (pending acks, then DISCONNECT 0x00).
//!
//! Needs `mosquitto`, `mosquitto_pub`, `mosquitto_sub`; a missing binary
//! fails the test instead of passing it.
//!
//! ```bash
//! cargo test -p zerodds-mqtt-bridge --features cross-vendor-tests \
//!     --test mosquitto_qos -- --ignored --test-threads=1
//! ```

#![cfg(all(feature = "daemon", feature = "cross-vendor-tests"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::io::Write;
use std::path::{Path, PathBuf};

use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use zerodds_mqtt_bridge::daemon::client::{InboundEvent, MqttClient, SubscribeEntry};
use zerodds_mqtt_bridge::daemon::config::DaemonConfig;

/// The daemon child, killed and reaped when the test ends, also when an
/// assertion fails before the test's own SIGTERM: a bare `Child` is not
/// killed on drop, and a failed run left the daemon running for hours.
struct Reaped(std::process::Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn require_mosquitto() {
    for bin in ["mosquitto", "mosquitto_pub", "mosquitto_sub"] {
        assert!(
            Command::new(bin).arg("--help").output().is_ok(),
            "{bin} is not installed; this test measures against it"
        );
    }
}

fn workdir(leaf: &str) -> PathBuf {
    // Under the system temp directory: Mosquitto started as root drops to
    // its own user, which cannot enter a checkout under /root.
    let dir = std::env::temp_dir()
        .join(format!("zerodds-mqtt-qos-{}", std::process::id()))
        .join(leaf);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    use std::os::unix::fs::PermissionsExt;
    for d in [dir.parent().unwrap(), dir.as_path()] {
        std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o777)).unwrap();
    }
    dir
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct Broker(Child);

impl Drop for Broker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Mosquitto on `port` with its default inflight window (20), logging
/// every packet to `log`.
fn start_broker(dir: &Path, port: u16) -> (Broker, PathBuf) {
    let log = dir.join("mosquitto.log");
    let conf = dir.join("mosquitto.conf");
    std::fs::write(
        &conf,
        format!(
            "listener {port} 127.0.0.1\nallow_anonymous true\nlog_type all\nlog_dest file {}\n",
            log.display()
        ),
    )
    .unwrap();
    let child = Command::new("mosquitto")
        .arg("-c")
        .arg(&conf)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mosquitto");
    let broker = Broker(child);
    let deadline = Instant::now() + Duration::from_secs(5);
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(Instant::now() < deadline, "mosquitto did not open {port}");
        thread::sleep(Duration::from_millis(50));
    }
    (broker, log)
}

fn client(port: u16, id: &str) -> MqttClient {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.client_id = id.to_string();
    cfg.clean_start = true;
    MqttClient::connect("127.0.0.1", port, &cfg).expect("CONNECT")
}

/// `n` messages with QoS `qos` from `mosquitto_pub`, one per line.
fn publish_lines(port: u16, topic: &str, qos: u8, n: usize) {
    let mut child = Command::new("mosquitto_pub")
        .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-t", topic])
        .args(["-q", &qos.to_string(), "-l"])
        .stdin(Stdio::piped())
        .spawn()
        .expect("mosquitto_pub");
    {
        let stdin = child.stdin.as_mut().unwrap();
        for i in 0..n {
            writeln!(stdin, "m{i}").unwrap();
        }
    }
    drop(child.stdin.take());
    assert!(child.wait().unwrap().success());
}

/// Counts the PUBLISH packets on `topic` until none arrives for `quiet`.
fn count_publishes(c: &mut MqttClient, topic: &str, quiet: Duration) -> usize {
    let mut n = 0;
    let mut last = Instant::now();
    while last.elapsed() < quiet {
        match c.next_event().expect("next_event") {
            Some(InboundEvent::Publish { topic: t, .. }) if t == topic => {
                n += 1;
                last = Instant::now();
            }
            Some(InboundEvent::Disconnected(why)) => panic!("disconnected: {why}"),
            _ => {}
        }
    }
    n
}

/// MQTT 5.0 §4.3.2/§4.3.3: every QoS 1 and QoS 2 message the broker sends
/// is acknowledged, so the broker's inflight window (Mosquitto: 20) does
/// not fill and all messages arrive.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_6_inbound_qos_1_and_2_are_acknowledged_and_all_messages_arrive() {
    require_mosquitto();
    for qos in [1u8, 2] {
        let dir = workdir(&format!("inbound-{qos}"));
        let port = free_port();
        let (_broker, log) = start_broker(&dir, port);
        let mut c = client(port, &format!("zerodds-qos-in-{qos}"));
        c.subscribe(&[SubscribeEntry {
            filter: "zerodds/qos/in".to_string(),
            qos,
            id: 1,
        }])
        .unwrap();
        thread::sleep(Duration::from_millis(300));
        publish_lines(port, "zerodds/qos/in", qos, 60);
        let got = count_publishes(&mut c, "zerodds/qos/in", Duration::from_secs(3));
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        assert_eq!(got, 60, "QoS {qos}: {got} of 60 arrived\n{}", tail(&text));
        c.graceful_disconnect();
    }
}

fn tail(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(30)..].join("\n")
}

/// `mosquitto_sub` for `n` messages on `topic` with QoS `qos`, started
/// before anything is published; returns the child to collect later.
fn subscriber(port: u16, topic: &str, qos: u8, n: usize) -> Child {
    let child = Command::new("mosquitto_sub")
        .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-t", topic])
        .args(["-q", &qos.to_string(), "-C", &n.to_string(), "-W", "10"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("mosquitto_sub");
    thread::sleep(Duration::from_millis(300));
    child
}

fn received_lines(child: Child) -> usize {
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout).lines().count()
}

/// §4.3.3 outbound: every QoS 2 PUBLISH gets PUBREC, the client answers
/// PUBREL and the broker PUBCOMP; afterwards nothing is pending. QoS 1
/// ends with PUBACK. Before, PUBREC went unanswered and every QoS 2 flow
/// stayed open. §4.9: 60 messages in a row stay within Mosquitto's Receive
/// Maximum of 20; sent without waiting, 10 QoS 1 messages were
/// acknowledged and not passed on.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_6_outbound_qos_1_and_2_complete_and_leave_nothing_pending() {
    require_mosquitto();
    let dir = workdir("outbound");
    let port = free_port();
    let (_broker, log) = start_broker(&dir, port);
    let sub = subscriber(port, "zerodds/qos/out", 2, 60);
    let mut c = client(port, "zerodds-qos-out");
    for i in 0..60u32 {
        let qos = if i % 2 == 0 { 1 } else { 2 };
        c.publish(
            "zerodds/qos/out",
            format!("m{i}").as_bytes(),
            qos,
            false,
            &Default::default(),
        )
        .unwrap();
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while c.pending_acks() > 0 && Instant::now() < deadline {
        let _ = c.next_event().unwrap();
    }
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    assert_eq!(c.pending_acks(), 0, "{}", tail(&text));
    assert_eq!(received_lines(sub), 60);
    assert_eq!(
        text.matches("Received PUBREL from zerodds-qos-out").count(),
        30
    );
    c.graceful_disconnect();
}

/// Spec §9.2 through the binary: SIGTERM ends the process with exit 0
/// after DISCONNECT (Mosquitto logs `Received DISCONNECT`). Before, the
/// process was still running 3 s after SIGTERM.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_9_2_sigterm_disconnects_and_exits_0() {
    require_mosquitto();
    let lease = zerodds_test_domains::lease_domain();
    let dir = workdir("sigterm");
    let port = free_port();
    let (_broker, log) = start_broker(&dir, port);
    let mut daemon = Reaped(
        Command::new(env!("CARGO_BIN_EXE_zerodds-mqtt-bridged"))
            .args(["--set", &format!("mqtt.broker_url=mqtt://127.0.0.1:{port}")])
            .args(["--set", "mqtt.client_id=zerodds-qos-term", "--set"])
            .arg(format!("domain={}", lease.id()))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !std::fs::read_to_string(&log)
        .unwrap_or_default()
        .contains("New client connected")
    {
        assert!(Instant::now() < deadline, "the daemon did not connect");
        thread::sleep(Duration::from_millis(50));
    }
    let started = Instant::now();
    let killed = Command::new("kill")
        .args(["-TERM", &daemon.0.id().to_string()])
        .status()
        .unwrap();
    assert!(killed.success());
    let status = loop {
        if let Some(st) = daemon.0.try_wait().unwrap() {
            break st;
        }
        if started.elapsed() > Duration::from_secs(5) {
            panic!("still running 5 s after SIGTERM");
        }
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(0));
    thread::sleep(Duration::from_millis(200));
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        text.contains("Received DISCONNECT from zerodds-qos-term"),
        "{}",
        tail(&text)
    );
}

/// Spec §9.2 in the daemon: samples published with QoS 1 right before the
/// shutdown are acknowledged before DISCONNECT, and all reach a
/// subscriber.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_9_2_the_shutdown_waits_for_the_acks_then_disconnects() {
    use zerodds_mqtt_bridge::daemon::config::TopicConfig;
    use zerodds_mqtt_bridge::daemon::server;
    require_mosquitto();
    let dir = workdir("drain");
    let port = free_port();
    let (_broker, log) = start_broker(&dir, port);
    let sub = subscriber(port, "zerodds/qos/drain", 1, 40);
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.broker_url = format!("mqtt://127.0.0.1:{port}");
    cfg.client_id = "zerodds-qos-drain".into();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.topics.push(TopicConfig {
        dds_name: "Drain".into(),
        dds_type: "Drain".into(),
        mqtt_topic: "zerodds/qos/drain".into(),
        direction: "out".into(),
        mqtt_qos: Some(1),
        retain: Some(false),
        reliability: "reliable".into(),
        durability: "volatile".into(),
        history_depth: 100,
        ..TopicConfig::default()
    });
    let mut handle = server::start(cfg).expect("daemon start");
    let reader = *handle.user_readers.get("Drain").unwrap();
    for i in 0..40u32 {
        assert!(
            handle
                .runtime
                .test_inject_user_alive(reader, format!("d{i}").into_bytes())
        );
    }
    // Let the pump take the samples, then shut down at once.
    thread::sleep(Duration::from_millis(300));
    handle.shutdown();
    assert_eq!(received_lines(sub), 40);
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        text.contains("Received DISCONNECT from zerodds-qos-drain"),
        "{}",
        tail(&text)
    );
}

/// The daemon in the test process towards Mosquitto on `port`.
fn daemon_config(port: u16, client_id: &str) -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.broker_url = format!("mqtt://127.0.0.1:{port}");
    cfg.client_id = client_id.into();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    (cfg, lease)
}

fn out_in_topic(
    dds: &str,
    mqtt: &str,
    direction: &str,
) -> zerodds_mqtt_bridge::daemon::config::TopicConfig {
    zerodds_mqtt_bridge::daemon::config::TopicConfig {
        dds_name: dds.into(),
        dds_type: dds.into(),
        mqtt_topic: mqtt.into(),
        direction: direction.into(),
        mqtt_qos: Some(1),
        reliability: "reliable".into(),
        durability: "volatile".into(),
        ..Default::default()
    }
}

/// The next alive sample of `rx` within `within`.
fn next_sample(
    rx: &std::sync::mpsc::Receiver<zerodds_dcps::runtime::UserSample>,
    within: Duration,
) -> Option<zerodds_dcps::runtime::UserSample> {
    let deadline = Instant::now() + within;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(s @ zerodds_dcps::runtime::UserSample::Alive { .. }) => return Some(s),
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    None
}

fn reader_beside(
    runtime: &zerodds_dcps::runtime::DcpsRuntime,
    topic: &str,
) -> std::sync::mpsc::Receiver<zerodds_dcps::runtime::UserSample> {
    runtime
        .register_user_reader(zerodds_dcps::runtime::UserReaderConfig {
            topic_name: topic.into(),
            type_name: topic.into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap()
        .1
}

/// `mosquitto_pub` of the bytes `payload` (through a file) with `extra`
/// arguments.
fn publish_bytes(port: u16, dir: &Path, topic: &str, payload: &[u8], extra: &[&str]) {
    let file = dir.join("payload.bin");
    std::fs::write(&file, payload).unwrap();
    let status = Command::new("mosquitto_pub")
        .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-t", topic])
        .args(["-q", "1", "-f"])
        .arg(&file)
        .args(extra)
        .status()
        .unwrap();
    assert!(status.success());
}

/// Spec §1 L1: with `mqtt.protocol_version: 3.1.1` the daemon is an MQTT
/// 3.1.1 client of Mosquitto (protocol level 4, Mosquitto logs `p2`), and
/// carries messages both ways with 3.1.1 clients of Mosquitto.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_1_l1_the_daemon_speaks_mqtt_3_1_1_to_mosquitto() {
    use zerodds_mqtt_bridge::daemon::server;
    require_mosquitto();
    let dir = workdir("v311");
    let port = free_port();
    let (_broker, log) = start_broker(&dir, port);
    let (mut cfg, _lease) = daemon_config(port, "zerodds-v311");
    cfg.protocol_version = zerodds_mqtt_bridge::version::ProtocolVersion::V311;
    cfg.topics
        .push(out_in_topic("V311In", "zerodds/v311/in", "in"));
    cfg.topics
        .push(out_in_topic("V311Out", "zerodds/v311/out", "out"));
    let mut handle = server::start(cfg).expect("daemon start");
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        text.contains("as zerodds-v311 (p2,"),
        "Mosquitto did not log a 3.1.1 client\n{}",
        tail(&text)
    );
    let sub = Command::new("mosquitto_sub")
        .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-V", "mqttv311"])
        .args([
            "-t",
            "zerodds/v311/out",
            "-q",
            "1",
            "-C",
            "1",
            "-W",
            "10",
            "-F",
            "%x",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_millis(400));
    assert!(
        handle
            .runtime
            .test_inject_user_alive(handle.user_readers["V311Out"], vec![0xab, 0xcd])
    );
    let out = sub.wait_with_output().unwrap();
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "00010000abcd");
    let rx = reader_beside(&handle.runtime, "V311In");
    publish_bytes(
        port,
        &dir,
        "zerodds/v311/in",
        &[0x00, 0x01, 0x00, 0x00, 0x42],
        &["-V", "mqttv311"],
    );
    match next_sample(&rx, Duration::from_secs(5)) {
        Some(zerodds_dcps::runtime::UserSample::Alive { payload, .. }) => {
            assert_eq!(payload.to_vec(), [0x42]);
        }
        other => panic!("{other:?}"),
    }
    handle.shutdown();
}

/// Spec §1 L1, MQTT 5.0 §3.1.2.10 ([MQTT-3.1.2-20], [MQTT-3.1.2-22]): an
/// idle daemon with a Keep Alive of 2 s sends PINGREQ and Mosquitto keeps
/// it past one and a half Keep Alives; before, the daemon sent no PINGREQ.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_1_l1_an_idle_daemon_stays_connected_past_one_and_a_half_keep_alives() {
    use zerodds_mqtt_bridge::daemon::server;
    require_mosquitto();
    let dir = workdir("keepalive");
    let port = free_port();
    let (_broker, log) = start_broker(&dir, port);
    let (mut cfg, _lease) = daemon_config(port, "zerodds-idle");
    cfg.keep_alive_secs = 2;
    let mut handle = server::start(cfg).expect("daemon start");
    thread::sleep(Duration::from_millis(5500));
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        text.matches("Received PINGREQ from zerodds-idle").count() >= 2,
        "{}",
        tail(&text)
    );
    assert!(
        !text.contains("zerodds-idle has exceeded timeout"),
        "{}",
        tail(&text)
    );
    handle.shutdown();
    thread::sleep(Duration::from_millis(200));
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        text.contains("Received DISCONNECT from zerodds-idle"),
        "{}",
        tail(&text)
    );
}

/// Spec §4.1: with `clean_start: false` and a Session Expiry Interval the
/// session outlives the connection: Mosquitto keeps the subscription and
/// the QoS 1 messages published meanwhile, and the next CONNECT gets them
/// with Session Present 1; the client's state of the session is in
/// `mqtt.session_dir` and the second client takes it up. With an interval
/// of 0 the session ends with the connection (MQTT 5.0 §3.1.2.11.2) and no
/// state is kept.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_4_1_the_session_survives_a_reconnect_with_clean_start_false() {
    require_mosquitto();
    let dir = workdir("session");
    let port = free_port();
    let (_broker, log) = start_broker(&dir, port);
    for (expiry, want) in [(300u32, 3usize), (0, 0)] {
        let topic = format!("zerodds/session/{expiry}");
        let mut cfg = DaemonConfig::default_for_dev();
        cfg.client_id = format!("zerodds-session-{expiry}");
        cfg.clean_start = false;
        cfg.session_expiry_interval_secs = expiry;
        cfg.session_dir = dir.join("sessions").display().to_string();
        let mut c = MqttClient::connect("127.0.0.1", port, &cfg).expect("CONNECT");
        c.subscribe(&[SubscribeEntry {
            filter: topic.clone(),
            qos: 1,
            id: 1,
        }])
        .unwrap();
        thread::sleep(Duration::from_millis(300));
        c.graceful_disconnect();
        thread::sleep(Duration::from_millis(200));
        publish_lines(port, &topic, 1, 3);
        assert_eq!(
            zerodds_mqtt_bridge::daemon::session::session_file(&cfg).exists(),
            expiry > 0,
            "the client's session state, expiry {expiry}"
        );
        let mut c = MqttClient::connect("127.0.0.1", port, &cfg).expect("CONNECT again");
        assert_eq!(c.session_present(), want > 0, "expiry {expiry}");
        let got = count_publishes(&mut c, &topic, Duration::from_secs(2));
        let text = std::fs::read_to_string(&log).unwrap_or_default();
        assert_eq!(got, want, "expiry {expiry}\n{}", tail(&text));
        c.graceful_disconnect();
    }
}

/// Spec §4.2 against `mosquitto_sub`: a DDS sample's PUBLISH carries the
/// Content Type, Payload Format Indicator 0, the Message Expiry Interval of
/// its lifespan and the user properties, as Mosquitto passes them on.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_4_2_mosquitto_sub_prints_the_user_properties() {
    use zerodds_mqtt_bridge::daemon::server;
    require_mosquitto();
    let dir = workdir("props");
    let port = free_port();
    let (_broker, _log) = start_broker(&dir, port);
    let (mut cfg, _lease) = daemon_config(port, "zerodds-props");
    cfg.topics
        .push(out_in_topic("Props", "zerodds/props", "out"));
    let mut handle = server::start(cfg).expect("daemon start");
    let writer = handle
        .runtime
        .register_user_writer(zerodds_dcps::runtime::UserWriterConfig {
            topic_name: "Props".into(),
            type_name: "Props".into(),
            reliable: true,
            lifespan: zerodds_qos::LifespanQosPolicy {
                duration: zerodds_qos::Duration::from_secs(30),
            },
            ..Default::default()
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while handle
        .runtime
        .user_writer_matched_reader_guids(writer)
        .is_empty()
    {
        assert!(Instant::now() < deadline, "the bridge's reader");
        thread::sleep(Duration::from_millis(20));
    }
    let sub = Command::new("mosquitto_sub")
        .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-V", "mqttv5"])
        .args(["-t", "zerodds/props", "-q", "1", "-C", "1", "-W", "10"])
        .args(["-F", "%C|%F|%E|%P|%x"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_millis(400));
    let ts = zerodds_rtps::header_extension::HeTimestamp {
        seconds: 1_700_000_000,
        fraction: 0,
    };
    handle
        .runtime
        .write_user_sample_keyed(writer, &[7, 7], [0x11; 16], Some(ts))
        .unwrap();
    let out = sub.wait_with_output().unwrap();
    let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let fields: Vec<&str> = line.split('|').collect();
    assert_eq!(fields.len(), 5, "{line}");
    assert!(fields[0].starts_with("application/x-dds-cdr"), "{line}");
    assert_eq!(fields[1], "0", "{line}");
    let expiry: u32 = fields[2].parse().unwrap_or(0);
    assert!((28..=30).contains(&expiry), "{line}");
    for want in [
        "zerodds_type:Props",
        "zerodds_topic:Props",
        "zerodds_flags:0x00",
        "zerodds_key_hash:11111111111111111111111111111111",
        "zerodds_source_ts_ns:1700000000000000000",
    ] {
        assert!(fields[3].contains(want), "{want} not in {line}");
    }
    assert!(fields[4].ends_with("0707"), "{line}");
    handle.shutdown();
}

/// Spec §4.2 MQTT → DDS against `mosquitto_pub`: its user properties
/// `zerodds_source_ts_ns` and `zerodds_key_hash` become the source
/// timestamp and the key hash of the DDS sample.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_4_2_mosquitto_pub_user_properties_reach_dds() {
    use zerodds_mqtt_bridge::daemon::server;
    require_mosquitto();
    let dir = workdir("props-in");
    let port = free_port();
    let (_broker, _log) = start_broker(&dir, port);
    let (mut cfg, _lease) = daemon_config(port, "zerodds-props-in");
    cfg.topics
        .push(out_in_topic("PropsIn", "zerodds/props/in", "in"));
    let mut handle = server::start(cfg).expect("daemon start");
    let rx = reader_beside(&handle.runtime, "PropsIn");
    thread::sleep(Duration::from_millis(300));
    publish_bytes(
        port,
        &dir,
        "zerodds/props/in",
        &[0x00, 0x07, 0x00, 0x00, 0x33],
        &[
            "-V",
            "mqttv5",
            "-D",
            "publish",
            "content-type",
            "application/x-dds-cdr2",
            "-D",
            "publish",
            "user-property",
            "zerodds_source_ts_ns",
            "1700000000500000000",
            "-D",
            "publish",
            "user-property",
            "zerodds_key_hash",
            "ffeeddccbbaa99887766554433221100",
        ],
    );
    match next_sample(&rx, Duration::from_secs(5)) {
        Some(zerodds_dcps::runtime::UserSample::Alive {
            payload,
            key_hash,
            source_timestamp,
            ..
        }) => {
            assert_eq!(payload.to_vec(), [0x33]);
            assert_eq!(
                key_hash,
                Some([
                    0xff, 0xee, 0xdd, 0xcc, 0xbb, 0xaa, 0x99, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33,
                    0x22, 0x11, 0x00
                ])
            );
            let ts = source_timestamp.expect("a source timestamp");
            assert_eq!((ts.seconds, ts.fraction), (1_700_000_000, 0x8000_0000));
        }
        other => panic!("{other:?}"),
    }
    handle.shutdown();
}

/// Spec §4.3 against Mosquitto on a bidir topic: the daemon subscribes with
/// No Local, so Mosquitto sends it none of its own PUBLISH packets (its log
/// has one `Sending PUBLISH to` the daemon, the 5 octets of `mosquitto_pub`,
/// none of the 7 octets of the daemon's own); a DDS sample reaches a second
/// subscriber once and DDS not again; a message of `mosquitto_pub` reaches
/// DDS once and the second subscriber once, not a second time through the
/// bridge.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_4_3_a_bidir_topic_does_not_get_its_own_publish_back() {
    use zerodds_mqtt_bridge::daemon::server;
    require_mosquitto();
    let dir = workdir("bidir-loop");
    let port = free_port();
    let (_broker, log) = start_broker(&dir, port);
    let (mut cfg, _lease) = daemon_config(port, "zerodds-loop");
    cfg.topics
        .push(out_in_topic("Loop", "zerodds/loop", "bidir"));
    let mut handle = server::start(cfg).expect("daemon start");
    let rx = reader_beside(&handle.runtime, "Loop");
    let sub = Command::new("mosquitto_sub")
        .args([
            "-h",
            "127.0.0.1",
            "-p",
            &port.to_string(),
            "-t",
            "zerodds/loop",
        ])
        .args(["-q", "1", "-C", "3", "-W", "4", "-F", "%x"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    thread::sleep(Duration::from_millis(400));
    // DDS → MQTT: a sample of another writer (the injection carries none).
    assert!(
        handle
            .runtime
            .test_inject_user_alive(handle.user_readers["Loop"], vec![0x11, 0x11, 0x11])
    );
    thread::sleep(Duration::from_millis(500));
    // MQTT → DDS.
    publish_bytes(
        port,
        &dir,
        "zerodds/loop",
        &[0x00, 0x01, 0x00, 0x00, 0x22],
        &[],
    );
    match next_sample(&rx, Duration::from_secs(5)) {
        Some(zerodds_dcps::runtime::UserSample::Alive { payload, .. }) => {
            assert_eq!(payload.to_vec(), [0x22]);
        }
        other => panic!("{other:?}"),
    }
    assert!(
        next_sample(&rx, Duration::from_millis(800)).is_none(),
        "a second sample reached DDS"
    );
    let out = sub.wait_with_output().unwrap();
    let lines: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].ends_with("111111"), "{lines:?}");
    assert!(lines[1].ends_with("22"), "{lines:?}");
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(
        text.contains("Received SUBSCRIBE from zerodds-loop"),
        "{}",
        tail(&text)
    );
    let to_daemon: Vec<&str> = text
        .lines()
        .filter(|l| l.contains("Sending PUBLISH to zerodds-loop"))
        .collect();
    assert_eq!(to_daemon.len(), 1, "{to_daemon:?}\n{}", tail(&text));
    assert!(to_daemon[0].contains("(5 bytes)"), "{to_daemon:?}");
    handle.shutdown();
}

/// Spec §5.2 against Mosquitto: Mosquitto takes the daemon's PUBLISH on
/// `$zerodds/<client_id>/catalog` and keeps it retained; `mosquitto_sub`
/// started after the daemon connected prints the catalog, its Content Type
/// `application/json` and Payload Format Indicator 1. After the shutdown
/// Mosquitto holds no catalog of the daemon.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_5_2_a_late_subscriber_gets_the_retained_catalog() {
    use zerodds_mqtt_bridge::daemon::server;
    require_mosquitto();
    let dir = workdir("catalog");
    let port = free_port();
    let (_broker, _log) = start_broker(&dir, port);
    let (mut cfg, _lease) = daemon_config(port, "zerodds-catalog");
    cfg.topics
        .push(out_in_topic("Cat::Topic", "zerodds/cat", "out"));
    let mut handle = server::start(cfg).expect("daemon start");
    let catalog_sub = |wait: &str| {
        Command::new("mosquitto_sub")
            .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-V", "mqttv5"])
            .args(["-t", "$zerodds/+/catalog", "-C", "1", "-W", wait])
            .args(["-F", "%t|%C|%F|%p"])
            .output()
            .unwrap()
    };
    let out = catalog_sub("5");
    let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(
        line,
        concat!(
            "$zerodds/zerodds-catalog/catalog|application/json|1|",
            r#"{"topics":[{"dds_name":"Cat::Topic","dds_type":"Cat::Topic","mqtt_topic":"zerodds/cat","#,
            r#""qos":{"reliability":"reliable","durability":"volatile"}}]}"#
        )
    );
    handle.shutdown();
    let out = catalog_sub("2");
    assert!(
        String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        "Mosquitto still holds a catalog: {}",
        String::from_utf8_lossy(&out.stdout)
    );
}

/// Spec §6 Liveliness against Mosquitto: the daemon's CONNECT carries the
/// will on `$zerodds/<client_id>/liveliness_lost`; when the process is
/// killed (SIGKILL, no DISCONNECT) Mosquitto publishes it, and
/// `mosquitto_sub` prints its payload and Content Type.
#[test]
#[ignore = "needs mosquitto; run with --ignored"]
fn mqttbridge_6_mosquitto_publishes_the_will_when_the_daemon_is_killed() {
    require_mosquitto();
    let lease = zerodds_test_domains::lease_domain();
    let dir = workdir("will");
    let port = free_port();
    let (_broker, log) = start_broker(&dir, port);
    let sub = Command::new("mosquitto_sub")
        .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-V", "mqttv5"])
        .args(["-t", "$zerodds/+/liveliness_lost", "-C", "1", "-W", "10"])
        .args(["-F", "%t|%C|%p"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut daemon = Reaped(
        Command::new(env!("CARGO_BIN_EXE_zerodds-mqtt-bridged"))
            .args(["--set", &format!("mqtt.broker_url=mqtt://127.0.0.1:{port}")])
            .args(["--set", "mqtt.client_id=zerodds-will", "--set"])
            .arg(format!("domain={}", lease.id()))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !std::fs::read_to_string(&log)
        .unwrap_or_default()
        .contains("Received SUBSCRIBE from")
        || !std::fs::read_to_string(&log)
            .unwrap_or_default()
            .contains("New client connected from 127.0.0.1")
        || !std::fs::read_to_string(&log)
            .unwrap_or_default()
            .contains("zerodds-will")
    {
        assert!(Instant::now() < deadline, "the daemon did not connect");
        thread::sleep(Duration::from_millis(50));
    }
    thread::sleep(Duration::from_millis(300));
    daemon.0.kill().unwrap();
    let _ = daemon.0.wait();
    let out = sub.wait_with_output().unwrap();
    let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(
        line,
        r#"$zerodds/zerodds-will/liveliness_lost|application/json|{"client_id":"zerodds-will"}"#,
        "{}",
        tail(&std::fs::read_to_string(&log).unwrap_or_default())
    );
}
