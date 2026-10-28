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

use zerodds_mqtt_bridge::daemon::client::{InboundEvent, MqttClient};
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
fn inbound_qos_1_and_2_are_acknowledged_and_all_messages_arrive() {
    require_mosquitto();
    for qos in [1u8, 2] {
        let dir = workdir(&format!("inbound-{qos}"));
        let port = free_port();
        let (_broker, log) = start_broker(&dir, port);
        let mut c = client(port, &format!("zerodds-qos-in-{qos}"));
        c.subscribe(&[("zerodds/qos/in".to_string(), qos)]).unwrap();
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
fn outbound_qos_1_and_2_complete_and_leave_nothing_pending() {
    require_mosquitto();
    let dir = workdir("outbound");
    let port = free_port();
    let (_broker, log) = start_broker(&dir, port);
    let sub = subscriber(port, "zerodds/qos/out", 2, 60);
    let mut c = client(port, "zerodds-qos-out");
    for i in 0..60u32 {
        let qos = if i % 2 == 0 { 1 } else { 2 };
        c.publish("zerodds/qos/out", format!("m{i}").as_bytes(), qos, false)
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
fn sigterm_disconnects_and_exits_0() {
    require_mosquitto();
    let dir = workdir("sigterm");
    let port = free_port();
    let (_broker, log) = start_broker(&dir, port);
    let mut daemon = Reaped(
        Command::new(env!("CARGO_BIN_EXE_zerodds-mqtt-bridged"))
            .args(["--set", &format!("mqtt.broker_url=mqtt://127.0.0.1:{port}")])
            .args([
                "--set",
                "mqtt.client_id=zerodds-qos-term",
                "--set",
                "domain=99",
            ])
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
fn the_shutdown_waits_for_the_acks_then_disconnects() {
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
    cfg.domain = 99;
    cfg.topics.push(TopicConfig {
        dds_name: "Drain".into(),
        dds_type: "Drain".into(),
        mqtt_topic: "zerodds/qos/drain".into(),
        direction: "out".into(),
        mqtt_qos: 1,
        retain: false,
        reliability: "reliable".into(),
        durability: "volatile".into(),
        history_depth: 100,
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
