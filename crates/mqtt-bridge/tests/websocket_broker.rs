// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The `ws://` and `wss://` broker schemes (Spec §2, MQTT 5.0 §6) against
//! Mosquitto's WebSocket listener.
//!
//! One broker per test with a plain MQTT listener beside the WebSocket one:
//! `mosquitto_pub` / `mosquitto_sub` on the plain listener are the other
//! side, so a message crosses the broker between the two transports. The
//! messages include one above 64 KiB (the 64-bit frame length of RFC 6455
//! §5.2). Two refusals are measured as well: a `wss://` broker whose
//! certificate the client does not trust, and a `ws://` URL pointed at a
//! plain MQTT listener.
//!
//! Needs `mosquitto`, `mosquitto_pub`, `mosquitto_sub`; a missing binary
//! fails the test instead of passing it.
//!
//! ```bash
//! cargo test -p zerodds-mqtt-bridge --features cross-vendor-tests \
//!     --test websocket_broker -- --ignored --test-threads=1
//! ```

#![cfg(all(feature = "daemon", feature = "cross-vendor-tests"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use rustls::ClientConfig;
use rustls::pki_types::CertificateDer;
use rustls_pemfile::Item;
use zerodds_mqtt_bridge::daemon::client::{InboundEvent, MqttClient};
use zerodds_mqtt_bridge::daemon::config::{DaemonConfig, parse_broker_url};

fn require_mosquitto() {
    for bin in ["mosquitto", "mosquitto_pub", "mosquitto_sub"] {
        assert!(
            Command::new(bin).arg("--help").output().is_ok(),
            "{bin} is not installed; this test measures against it"
        );
    }
}

fn workdir(leaf: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("mqtt-websocket")
        .join(leaf);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Broker(Child);

impl Drop for Broker {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Mosquitto with the given listener blocks; waits until every port
/// accepts a TCP connection.
fn start_broker(dir: &Path, config: &str, ports: &[u16]) -> Broker {
    let conf = dir.join("mosquitto.conf");
    std::fs::write(&conf, config).unwrap();
    let child = Command::new("mosquitto")
        .arg("-c")
        .arg(&conf)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mosquitto");
    let broker = Broker(child);
    let deadline = Instant::now() + Duration::from_secs(5);
    for port in ports {
        while std::net::TcpStream::connect(("127.0.0.1", *port)).is_err() {
            assert!(Instant::now() < deadline, "mosquitto did not open {port}");
            thread::sleep(Duration::from_millis(50));
        }
    }
    broker
}

/// A readable directory for the broker's certificate and key, removed on
/// drop.
struct TlsFiles(PathBuf);

impl TlsFiles {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("zerodds-mqtt-wss-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        Self(dir)
    }

    fn write(&self, name: &str, text: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = self.0.join(name);
        std::fs::write(&path, text).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    }
}

impl Drop for TlsFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn load_certs(pem: &str) -> Vec<CertificateDer<'static>> {
    rustls_pemfile::read_all(&mut pem.as_bytes())
        .filter_map(|item| match item {
            Ok(Item::X509Certificate(d)) => Some(d),
            _ => None,
        })
        .collect()
}

fn trusting(cert_pem: &str) -> Arc<ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    for c in load_certs(cert_pem) {
        roots.add(c).unwrap();
    }
    Arc::new(
        ClientConfig::builder_with_provider(Arc::new(zerodds_bridge_security::tls_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    )
}

fn client_cfg(id: &str) -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.client_id = id.to_string();
    cfg.clean_start = true;
    cfg.broker_tls_server_name = "localhost".to_string();
    cfg
}

/// The next PUBLISH on `topic` within `within`.
fn next_publish(client: &mut MqttClient, topic: &str, within: Duration) -> Vec<u8> {
    let deadline = Instant::now() + within;
    while Instant::now() < deadline {
        match client.next_event().expect("next_event") {
            Some(InboundEvent::Publish {
                topic: t, payload, ..
            }) if t == topic => return payload,
            Some(InboundEvent::Disconnected(why)) => panic!("disconnected: {why}"),
            _ => {}
        }
    }
    panic!("no PUBLISH on {topic} within {within:?}");
}

/// Subscribes over the WebSocket client, lets `mosquitto_pub` on the plain
/// listener publish `payload`, then publishes from the WebSocket client to
/// a `mosquitto_sub` on the plain listener.
fn round_trip(client: &mut MqttClient, plain_port: u16, dir: &Path, payload: &[u8]) {
    let port = plain_port.to_string();
    client
        .subscribe(&[("zerodds/ws/in".to_string(), 0)])
        .expect("subscribe");
    // SUBACK is consumed by next_event; give the broker the subscription.
    thread::sleep(Duration::from_millis(300));
    let file = dir.join("payload.bin");
    std::fs::write(&file, payload).unwrap();
    let status = Command::new("mosquitto_pub")
        .args(["-h", "127.0.0.1", "-p", &port, "-t", "zerodds/ws/in", "-f"])
        .arg(&file)
        .status()
        .expect("mosquitto_pub");
    assert!(status.success());
    let got = next_publish(client, "zerodds/ws/in", Duration::from_secs(10));
    assert_eq!(got.len(), payload.len(), "payload length over ws");
    assert!(got == payload, "payload bytes over ws");

    let sub = Command::new("mosquitto_sub")
        .args([
            "-h",
            "127.0.0.1",
            "-p",
            &port,
            "-t",
            "zerodds/ws/out",
            "-C",
            "1",
            "-W",
            "10",
        ])
        .stdout(Stdio::piped())
        .spawn()
        .expect("mosquitto_sub");
    thread::sleep(Duration::from_millis(500));
    client
        .publish("zerodds/ws/out", b"from-the-websocket-client", 0, false)
        .expect("publish");
    let out = sub.wait_with_output().expect("mosquitto_sub output");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "from-the-websocket-client"
    );
}

#[test]
#[ignore = "needs mosquitto; run with --features cross-vendor-tests -- --ignored"]
fn ws_broker_url_carries_mqtt_both_ways_through_mosquitto() {
    require_mosquitto();
    let dir = workdir("ws");
    let (plain, ws) = (21901, 21902);
    let _broker = start_broker(
        &dir,
        &format!(
            "listener {plain} 127.0.0.1\nlistener {ws} 127.0.0.1\nprotocol websockets\nallow_anonymous true\n"
        ),
        &[plain, ws],
    );
    let url = parse_broker_url(&format!("ws://127.0.0.1:{ws}/mqtt")).unwrap();
    let mut client =
        MqttClient::connect_url(&url, &client_cfg("zerodds-ws"), None).expect("CONNECT over ws");
    round_trip(&mut client, plain, &dir, b"hello over ws");
    // 70 000 bytes: the frame carries a 64-bit length (RFC 6455 §5.2).
    let big: Vec<u8> = (0..70_000u32).map(|i| (i % 251) as u8).collect();
    round_trip(&mut client, plain, &dir, &big);
    client.graceful_disconnect();
}

#[test]
#[ignore = "needs mosquitto; run with --features cross-vendor-tests -- --ignored"]
fn wss_broker_url_carries_mqtt_through_mosquitto_and_checks_the_certificate() {
    require_mosquitto();
    let dir = workdir("wss");
    // Started as root, mosquitto switches to its own user and must still
    // read the certificate and key; the target directory may sit below a
    // 0700 home. A directory of their own under the system temp dir, readable.
    let tls = TlsFiles::new();
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_pem = ck.cert.pem();
    tls.write("cert.pem", &cert_pem);
    tls.write("key.pem", &ck.key_pair.serialize_pem());
    let (plain, wss) = (21903, 21904);
    let _broker = start_broker(
        &dir,
        &format!(
            "listener {plain} 127.0.0.1\nlistener {wss} 127.0.0.1\nprotocol websockets\n\
             certfile {cert}\nkeyfile {key}\nallow_anonymous true\n",
            cert = tls.0.join("cert.pem").display(),
            key = tls.0.join("key.pem").display()
        ),
        &[plain, wss],
    );
    let url = parse_broker_url(&format!("wss://127.0.0.1:{wss}/mqtt")).unwrap();
    let mut client =
        MqttClient::connect_url(&url, &client_cfg("zerodds-wss"), Some(trusting(&cert_pem)))
            .expect("CONNECT over wss");
    round_trip(&mut client, plain, &dir, b"hello over wss");
    client.graceful_disconnect();

    // Gegenprobe: a certificate the client does not trust is refused.
    let other = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let refused = MqttClient::connect_url(
        &url,
        &client_cfg("zerodds-wss-untrusted"),
        Some(trusting(&other.cert.pem())),
    );
    assert!(
        refused.is_err(),
        "an untrusted broker certificate was accepted"
    );
}

/// Gegenprobe: a `ws://` URL on a plain MQTT listener fails the handshake
/// with a message, instead of reading the broker's bytes as frames.
#[test]
#[ignore = "needs mosquitto; run with --features cross-vendor-tests -- --ignored"]
fn a_ws_url_on_a_plain_listener_fails_the_handshake() {
    require_mosquitto();
    let dir = workdir("ws-on-plain");
    let plain = 21905;
    let _broker = start_broker(
        &dir,
        &format!("listener {plain} 127.0.0.1\nallow_anonymous true\n"),
        &[plain],
    );
    let url = parse_broker_url(&format!("ws://127.0.0.1:{plain}/mqtt")).unwrap();
    let err = match MqttClient::connect_url(&url, &client_cfg("zerodds-ws-plain"), None) {
        Ok(_) => panic!("a ws:// handshake succeeded against a plain MQTT listener"),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("websocket"), "{err}");
}
