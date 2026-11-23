// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Live MQTT interop against the Eclipse Mosquitto reference tools, covering
//! the two matrix gaps:
//!
//! 1. **MQTT 3.1.1 native codec** — ZeroDDS' [`MqttClient`] in 3.1.1 mode talks
//!    to a Mosquitto **broker** (both directions, against `mosquitto_pub/sub
//!    -V mqttv311`).
//! 2. **Standalone broker** — real `mosquitto_pub`/`mosquitto_sub` clients
//!    (5.0 and 3.1.1) connect to the ZeroDDS [`MqttBrokerServer`].
//!
//! The tests that need Mosquitto are `#[ignore]`d; running them
//! (`--run-ignored`, the weekly job `integration-bridges-vendor`) measures,
//! and a missing `mosquitto`, `mosquitto_pub` or `mosquitto_sub` fails the
//! test. The broker of section 1 is started by the test on a port of its own
//! and stopped with it.

#![cfg(feature = "std")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stderr)]

use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use zerodds_mqtt_bridge::net::MqttClient;
use zerodds_mqtt_bridge::server::MqttBrokerServer;
use zerodds_mqtt_bridge::version::ProtocolVersion::{V5, V311};

/// The Mosquitto broker of one test, on a free port of its own, stopped
/// when the value goes.
struct Broker {
    child: std::process::Child,
    port: u16,
    _dir: std::path::PathBuf,
}

impl Drop for Broker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Broker {
    fn start(label: &str) -> Self {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        // Under the system temp directory: Mosquitto started as root drops
        // to its own user, which cannot enter a checkout under /root.
        let dir = std::env::temp_dir().join(format!(
            "zerodds-mqtt-interop-{}-{label}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o777)).unwrap();
        }
        let conf = dir.join("mosquitto.conf");
        std::fs::write(
            &conf,
            format!("listener {port} 127.0.0.1\nallow_anonymous true\n"),
        )
        .unwrap();
        let child = Command::new("mosquitto")
            .arg("-c")
            .arg(&conf)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("mosquitto is not installed; this test measures against it");
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
            assert!(
                std::time::Instant::now() < deadline,
                "mosquitto did not open {port}"
            );
            thread::sleep(Duration::from_millis(20));
        }
        Self {
            child,
            port,
            _dir: dir,
        }
    }

    fn address(&self) -> String {
        format!("127.0.0.1:{}", self.port)
    }
}

// ---- Gap 1: ZeroDDS 3.1.1 client ↔ Mosquitto broker --------------------

#[test]
#[ignore = "needs mosquitto, mosquitto_pub and mosquitto_sub; runs in integration-bridges-vendor"]
fn mqtt5_4_zerodds_311_client_receives_from_mosquitto() {
    let broker = Broker::start("rx");
    let topic = "zd/311/rx";
    let mut sub = MqttClient::connect(broker.address(), "zd-311-sub", V311).expect("3.1.1 connect");
    sub.subscribe(topic, 1).expect("subscribe");

    // A 3.1.1 reference publisher puts a message on the topic.
    let status = Command::new("mosquitto_pub")
        .args(["-h", "127.0.0.1", "-p", &broker.port.to_string()])
        .args([
            "-V",
            "mqttv311",
            "-t",
            topic,
            "-m",
            "from-mosq-311",
            "-q",
            "1",
        ])
        .status()
        .expect("mosquitto_pub");
    assert!(status.success(), "mosquitto_pub failed");

    let (got_topic, payload) = sub.recv_publish(Duration::from_secs(5)).expect("receive");
    sub.disconnect();
    assert_eq!(got_topic, topic);
    assert_eq!(payload, b"from-mosq-311");
    eprintln!("OK: ZeroDDS-3.1.1 consumed a mosquitto_pub -V mqttv311 message");
}

#[test]
#[ignore = "needs mosquitto, mosquitto_pub and mosquitto_sub; runs in integration-bridges-vendor"]
fn mqtt5_4_mosquitto_311_client_receives_from_zerodds() {
    let broker = Broker::start("tx");
    let topic = "zd/311/tx";
    // Reference 3.1.1 subscriber, exits after one message or 5s.
    let sub = Command::new("mosquitto_sub")
        .args(["-h", "127.0.0.1", "-p", &broker.port.to_string()])
        .args(["-V", "mqttv311", "-t", topic, "-C", "1", "-W", "5"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("mosquitto_sub");
    thread::sleep(Duration::from_millis(500)); // let it connect + subscribe

    let mut pubc = MqttClient::connect(broker.address(), "zd-311-pub", V311).expect("connect");
    pubc.publish(topic, b"from-zerodds-311", 1, false)
        .expect("publish");
    pubc.disconnect();

    let out = sub.wait_with_output().expect("sub output");
    let got = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert_eq!(
        got, "from-zerodds-311",
        "mosquitto 3.1.1 must read the ZeroDDS publish"
    );
    eprintln!("OK: mosquitto_sub -V mqttv311 read a ZeroDDS-3.1.1 publish");
}

// ---- Gap 2: real Mosquitto clients ↔ ZeroDDS broker --------------------

fn mosquitto_against_zerodds_broker(version_flag: &str, label: &str) {
    let server = MqttBrokerServer::bind("127.0.0.1:0")
        .expect("bind")
        .spawn()
        .expect("spawn");
    let port = server.local_addr().port().to_string();
    let topic = "zd/broker/topic";

    let sub = Command::new("mosquitto_sub")
        .args([
            "-h",
            "127.0.0.1",
            "-p",
            &port,
            "-V",
            version_flag,
            "-t",
            topic,
            "-C",
            "1",
            "-W",
            "5",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("mosquitto_sub");
    thread::sleep(Duration::from_millis(500));

    let status = Command::new("mosquitto_pub")
        .args([
            "-h",
            "127.0.0.1",
            "-p",
            &port,
            "-V",
            version_flag,
            "-t",
            topic,
            "-m",
            "hello-broker",
            "-q",
            "1",
        ])
        .status()
        .expect("mosquitto_pub");
    assert!(status.success(), "mosquitto_pub failed");

    let out = sub.wait_with_output().expect("sub output");
    let got = String::from_utf8_lossy(&out.stdout).trim().to_string();
    server.shutdown();
    assert_eq!(
        got, "hello-broker",
        "{label}: mosquitto must round-trip via ZeroDDS broker"
    );
    eprintln!("OK: {label} — mosquitto_pub → ZeroDDS broker → mosquitto_sub");
}

#[test]
#[ignore = "needs mosquitto_pub and mosquitto_sub; runs in integration-bridges-vendor"]
fn mqtt5_4_mosquitto_v5_clients_through_zerodds_broker() {
    mosquitto_against_zerodds_broker("mqttv5", "MQTT 5.0");
}

#[test]
#[ignore = "needs mosquitto_pub and mosquitto_sub; runs in integration-bridges-vendor"]
fn mqtt5_4_mosquitto_v311_clients_through_zerodds_broker() {
    mosquitto_against_zerodds_broker("mqttv311", "MQTT 3.1.1");
}

// ---- MQTT 5.0 server behaviour seen by Mosquitto clients --------------

/// A `mosquitto_sub -V 5` against `port` with `args`, started now; its
/// standard output when it ends.
fn mosquitto_sub_v5(port: u16, args: &[&str]) -> std::process::Child {
    Command::new("mosquitto_sub")
        .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-V", "5"])
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .expect("mosquitto_sub is not installed; this test measures against it")
}

fn mosquitto_pub_v5(port: u16, args: &[&str]) {
    let status = Command::new("mosquitto_pub")
        .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-V", "5"])
        .args(args)
        .status()
        .expect("mosquitto_pub is not installed; this test measures against it");
    assert!(status.success(), "mosquitto_pub {args:?} failed");
}

fn output_of(child: std::process::Child) -> String {
    let out = child.wait_with_output().expect("mosquitto_sub output");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
#[ignore = "needs mosquitto_pub and mosquitto_sub; runs in integration-bridges-vendor"]
fn mqtt5_3_3_2_3_mosquitto_v5_properties_cross_the_zerodds_broker_unaltered() {
    let server = MqttBrokerServer::bind("127.0.0.1:0")
        .unwrap()
        .spawn()
        .unwrap();
    let port = server.local_addr().port();
    let sub = mosquitto_sub_v5(
        port,
        &[
            "-t",
            "p/t",
            "-C",
            "1",
            "-W",
            "5",
            "-D",
            "subscribe",
            "subscription-identifier",
            "7",
            "-F",
            "C=%C|R=%R|D=%D|F=%F|P=%P|S=%S|E=%E|p=%p",
        ],
    );
    thread::sleep(Duration::from_millis(500));
    mosquitto_pub_v5(
        port,
        &[
            "-t",
            "p/t",
            "-m",
            "hello",
            "-q",
            "1",
            "-D",
            "publish",
            "payload-format-indicator",
            "1",
            "-D",
            "publish",
            "message-expiry-interval",
            "60",
            "-D",
            "publish",
            "content-type",
            "text/plain",
            "-D",
            "publish",
            "response-topic",
            "reply/here",
            "-D",
            "publish",
            "correlation-data",
            "abc",
            "-D",
            "publish",
            "user-property",
            "z",
            "1",
            "-D",
            "publish",
            "user-property",
            "a",
            "2",
            "-D",
            "publish",
            "user-property",
            "z",
            "3",
        ],
    );
    let got = output_of(sub);
    server.shutdown();
    let (head, expiry) = got
        .rsplit_once("|E=")
        .expect("mosquitto_sub printed the message");
    assert_eq!(
        head, "C=text/plain|R=reply/here|D=abc|F=1|P=z:1 a:2 z:3|S=7",
        "[MQTT-3.3.2-4], [MQTT-3.3.2-15] to [MQTT-3.3.2-20], [MQTT-3.3.4-3]"
    );
    let (secs, payload) = expiry.split_once("|p=").unwrap();
    let secs: u32 = secs.parse().unwrap();
    assert!((58..=60).contains(&secs), "[MQTT-3.3.2-6]: {secs}");
    assert_eq!(payload, "hello");
}

#[test]
#[ignore = "needs mosquitto_pub and mosquitto_sub; runs in integration-bridges-vendor"]
fn mqtt5_3_1_2_5_a_mosquitto_v5_session_gets_its_queued_message_at_reconnect() {
    let server = MqttBrokerServer::bind("127.0.0.1:0")
        .unwrap()
        .spawn()
        .unwrap();
    let port = server.local_addr().port();
    let session = ["-c", "-i", "zd-persist", "-x", "60", "-q", "1", "-t", "s/q"];
    // Subscribe, then leave: the session stays (Session Expiry 60 s).
    let first = mosquitto_sub_v5(port, &[&session[..], &["-E"]].concat());
    assert_eq!(output_of(first), "");
    mosquitto_pub_v5(port, &["-t", "s/q", "-m", "while-away", "-q", "1"]);
    let back = mosquitto_sub_v5(port, &[&session[..], &["-C", "1", "-W", "5"]].concat());
    let got = output_of(back);
    server.shutdown();
    assert_eq!(got, "while-away", "[MQTT-3.1.2-5], [MQTT-4.5.0-1]");
}

#[test]
#[ignore = "needs mosquitto_pub and mosquitto_sub; runs in integration-bridges-vendor"]
fn mqtt5_4_8_2_two_mosquitto_v5_clients_share_a_subscription() {
    let server = MqttBrokerServer::bind("127.0.0.1:0")
        .unwrap()
        .spawn()
        .unwrap();
    let port = server.local_addr().port();
    let a = mosquitto_sub_v5(port, &["-t", "$share/g/sh/t", "-C", "2", "-W", "5"]);
    let b = mosquitto_sub_v5(port, &["-t", "$share/g/sh/t", "-C", "2", "-W", "5"]);
    thread::sleep(Duration::from_millis(500));
    for m in ["m0", "m1", "m2", "m3"] {
        mosquitto_pub_v5(port, &["-t", "sh/t", "-m", m]);
    }
    let (a, b) = (output_of(a), output_of(b));
    server.shutdown();
    let mut all: Vec<&str> = a.lines().chain(b.lines()).collect();
    assert_eq!(a.lines().count(), 2, "{a:?}");
    assert_eq!(b.lines().count(), 2, "{b:?}");
    all.sort_unstable();
    assert_eq!(
        all,
        ["m0", "m1", "m2", "m3"],
        "each message to one session of the group"
    );
}

// ---- Cross-stack: ZeroDDS client ↔ ZeroDDS broker over real TCP --------

#[test]
fn mqtt5_4_zerodds_client_through_zerodds_broker_cross_version() {
    let server = MqttBrokerServer::bind("127.0.0.1:0")
        .unwrap()
        .spawn()
        .unwrap();
    let addr = server.local_addr().to_string();

    let mut sub = MqttClient::connect(&addr, "zd-sub", V311).unwrap();
    sub.subscribe("x/topic", 1).unwrap();
    let mut pubc = MqttClient::connect(&addr, "zd-pub", V5).unwrap();
    pubc.publish("x/topic", b"zd-x", 1, false).unwrap();
    let (_, payload) = sub.recv_publish(Duration::from_secs(3)).unwrap();
    pubc.disconnect();
    sub.disconnect();
    server.shutdown();
    assert_eq!(payload, b"zd-x");
    eprintln!("OK: ZeroDDS-5.0 client → ZeroDDS broker → ZeroDDS-3.1.1 client");
}
