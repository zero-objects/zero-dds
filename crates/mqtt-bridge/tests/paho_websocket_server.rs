// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The WebSocket listeners of the broker server (MQTT 5.0 §6) against
//! Eclipse Paho's Python client (`paho-mqtt`, `transport="websockets"`):
//! a foreign client takes the opening handshake, offers the subprotocol
//! `mqtt`, masks its frames with its own keys and reads the server's
//! frames, in MQTT 5.0 and 3.1.1, on `ws` and on `wss`.
//!
//! Each test lets a Paho subscriber on the WebSocket listener receive a
//! message of 70 000 octets (a frame with a 64-bit length, RFC 6455 §5.2)
//! from a ZeroDDS client on the TCP listener, a ZeroDDS subscriber on the
//! TCP listener receive one from a Paho publisher (QoS 1, so the server's
//! PUBACK crosses back), and one Paho client receive from the other. The
//! subscriber script checks every octet against the pattern it expects;
//! the exit status of each Paho process is checked, and a missing
//! `paho.mqtt` module fails the test instead of passing it.
//!
//! ```bash
//! cargo test -p zerodds-mqtt-bridge --features cross-vendor-tests \
//!     --test paho_websocket_server -- --ignored
//! ```

#![cfg(all(feature = "daemon", feature = "cross-vendor-tests"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::PrivatePkcs8KeyDer;
use zerodds_mqtt_bridge::net::MqttClient;
use zerodds_mqtt_bridge::server::{MqttBrokerServer, ServerConfig, ServerHandle, Transport};
use zerodds_mqtt_bridge::version::ProtocolVersion::V5;

/// The size of each message: above 65 535 octets.
const SIZE: usize = 70_000;

/// One Paho client: `sub` subscribes to the topic, prints `subscribed`,
/// and prints `received <length> <whether every octet matches>` for the
/// first message; `pub` publishes the pattern with QoS 1 and waits for the
/// PUBACK. Arguments: mode, host, port, MQTT version (5 or 311), CA file
/// (`-` for `ws`), topic, size.
const PAHO: &str = r#"
import sys, threading
import paho.mqtt.client as mqtt

mode, host, port, version, ca, topic, size = sys.argv[1:8]
size = int(size)
pattern = bytes(i % 251 for i in range(size))
client = mqtt.Client(
    mqtt.CallbackAPIVersion.VERSION2,
    client_id="paho-%s-%s-%s" % (mode, version, port),
    protocol=mqtt.MQTTv5 if version == "5" else mqtt.MQTTv311,
    transport="websockets",
)
client.ws_set_options(path="/mqtt")
if ca != "-":
    client.tls_set(ca_certs=ca)
connected, subscribed, received = threading.Event(), threading.Event(), threading.Event()
state = {}

def on_connect(c, userdata, flags, reason_code, properties):
    state["connack"] = str(reason_code)
    state["failed"] = reason_code.is_failure
    connected.set()

def on_subscribe(c, userdata, mid, reason_codes, properties):
    state["suback"] = [str(r) for r in reason_codes]
    subscribed.set()

def on_message(c, userdata, message):
    state.setdefault("payload", message.payload)
    received.set()

client.on_connect = on_connect
client.on_subscribe = on_subscribe
client.on_message = on_message
client.connect(host, int(port), keepalive=30)
client.loop_start()
if not connected.wait(10):
    sys.exit("no CONNACK")
if state["failed"]:
    sys.exit("CONNACK %s" % state["connack"])
if mode == "sub":
    client.subscribe(topic, qos=1)
    if not subscribed.wait(10):
        sys.exit("no SUBACK")
    print("subscribed", flush=True)
    if not received.wait(15):
        sys.exit("no message")
    p = state["payload"]
    print("received %d %s" % (len(p), p == pattern), flush=True)
else:
    info = client.publish(topic, pattern, qos=1)
    info.wait_for_publish(10)
    if not info.is_published():
        sys.exit("no PUBACK")
    print("published", flush=True)
client.disconnect()
client.loop_stop()
"#;

fn require_paho() {
    let ok = Command::new("python3")
        .args(["-I", "-c", "import paho.mqtt.client"])
        .status()
        .is_ok_and(|s| s.success());
    assert!(
        ok,
        "python3 with paho.mqtt (python3-paho-mqtt) is not installed; this test measures against it"
    );
}

fn pattern() -> Vec<u8> {
    (0..SIZE).map(|i| (i % 251) as u8).collect()
}

/// Where the Paho clients connect: the port and, for `wss`, the CA file.
struct Target {
    port: u16,
    ca: Option<PathBuf>,
}

fn paho(mode: &str, target: &Target, version: &str, topic: &str) -> Child {
    let ca = target
        .ca
        .as_ref()
        .map_or_else(|| "-".to_string(), |p| p.display().to_string());
    Command::new("python3")
        .args(["-I", "-c", PAHO, mode, "localhost"])
        .arg(target.port.to_string())
        .args([version, &ca, topic, &SIZE.to_string()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn python3")
}

/// Waits for `child` and returns its stdout; a non-zero exit fails.
fn finish(child: Child, what: &str) -> String {
    let out = child.wait_with_output().expect("paho output");
    assert!(
        out.status.success(),
        "{what}: status={:?} stdout={} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// A Paho subscriber that has its SUBACK.
fn paho_subscriber(target: &Target, version: &str, topic: &str) -> Child {
    let mut child = paho("sub", target, version, topic);
    let mut line = String::new();
    BufReader::new(child.stdout.as_mut().unwrap())
        .read_line(&mut line)
        .unwrap();
    if line.trim() != "subscribed" {
        let out = child.wait_with_output().unwrap();
        panic!(
            "the Paho subscriber ({version}) did not subscribe: {line} {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    child
}

fn received_all(child: Child, what: &str) {
    let out = finish(child, what);
    assert!(
        out.contains(&format!("received {SIZE} True")),
        "{what}: {out}"
    );
}

/// The three exchanges of the module documentation against `target`.
fn exchanges(h: &ServerHandle, target: &Target, tag: &str) {
    let payload = pattern();
    // ZeroDDS on TCP -> Paho 5.0 on the WebSocket.
    let topic = format!("{tag}/to-paho");
    let sub = paho_subscriber(target, "5", &topic);
    let mut tcp = MqttClient::connect(h.local_addr(), &format!("{tag}-tcp-pub"), V5).unwrap();
    tcp.publish(&topic, &payload, 0, false).unwrap();
    received_all(sub, &format!("{tag}: Paho 5.0 subscriber"));

    // Paho 3.1.1 on the WebSocket -> ZeroDDS on TCP.
    let topic = format!("{tag}/from-paho");
    let mut zsub = MqttClient::connect(h.local_addr(), &format!("{tag}-tcp-sub"), V5).unwrap();
    zsub.subscribe(&topic, 0).unwrap();
    finish(
        paho("pub", target, "311", &topic),
        &format!("{tag}: Paho 3.1.1 publisher"),
    );
    let (t, got) = zsub.recv_publish(Duration::from_secs(10)).unwrap();
    assert_eq!(t, topic);
    assert!(got == payload, "{tag}: the octets of the Paho publisher");

    // Paho 5.0 -> Paho 3.1.1, both on the WebSocket.
    let topic = format!("{tag}/paho-to-paho");
    let sub = paho_subscriber(target, "311", &topic);
    finish(
        paho("pub", target, "5", &topic),
        &format!("{tag}: Paho 5.0 publisher"),
    );
    received_all(sub, &format!("{tag}: Paho 3.1.1 subscriber"));
}

#[test]
#[ignore = "needs python3-paho-mqtt; runs in integration-bridges-vendor"]
fn mqtt5_6_paho_clients_publish_and_subscribe_through_the_zerodds_broker_over_websocket() {
    require_paho();
    let h = MqttBrokerServer::bind_with("127.0.0.1:0", ServerConfig::default())
        .unwrap()
        .with_websocket("127.0.0.1:0")
        .unwrap()
        .spawn()
        .unwrap();
    let target = Target {
        port: h.transport_addr(Transport::WebSocket).unwrap().port(),
        ca: None,
    };
    exchanges(&h, &target, "ws");
    assert_eq!(h.handshake_failures(), 0);
    h.shutdown();
}

/// The CA file of the `wss` test, removed on drop.
struct CaFile(PathBuf);

impl Drop for CaFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[test]
#[ignore = "needs python3-paho-mqtt; runs in integration-bridges-vendor"]
fn mqtt5_6_paho_clients_publish_and_subscribe_through_the_zerodds_broker_over_wss() {
    require_paho();
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let ca = CaFile(
        PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("paho-wss-ca-{}.pem", std::process::id())),
    );
    std::fs::write(&ca.0, ck.cert.pem()).unwrap();
    let tls = rustls::ServerConfig::builder_with_provider(Arc::new(
        zerodds_bridge_security::tls_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![ck.cert.der().clone()],
        PrivatePkcs8KeyDer::from(ck.key_pair.serialize_der()).into(),
    )
    .unwrap();
    let h = MqttBrokerServer::bind_with("127.0.0.1:0", ServerConfig::default())
        .unwrap()
        .with_secure_websocket("127.0.0.1:0", Arc::new(tls))
        .unwrap()
        .spawn()
        .unwrap();
    let target = Target {
        port: h.transport_addr(Transport::SecureWebSocket).unwrap().port(),
        ca: Some(ca.0.clone()),
    };
    exchanges(&h, &target, "wss");
    assert_eq!(h.handshake_failures(), 0);
    h.shutdown();
}
