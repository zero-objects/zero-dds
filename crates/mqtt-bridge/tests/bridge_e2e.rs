// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-mqtt-bridged` as a process and in the test process against a
//! broker in the test: the crate's own MQTT broker server
//! (`zerodds_mqtt_bridge::server`) or a broker that records what the daemon
//! sends and sends what the test gives it.
//!
//! Spec `zerodds-mqtt-bridge-1.0` §1 (L1 compatibility mode and Keep Alive,
//! L6 tenants), §2 (flags, exit codes), §4.1 (CONNECT properties, the limits
//! of both sides), §4.2 (PUBLISH properties), §6 (the history of an out
//! topic), §8.1 (the JSON log), §8.3 (the OTLP spans), §9.3 (the
//! reconnect), §11 (the packaging starts the binary).

#![cfg(feature = "daemon")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::too_many_lines,
    missing_docs
)]

#[path = "../../websocket-bridge/tests/support/otlp_collector.rs"]
mod otlp_collector;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use otlp_collector::{otlp_collector, span_attribute};

use zerodds_dcps::runtime::{UserReaderConfig, UserSample, UserWriterConfig};
use zerodds_mqtt_bridge::codec::{PublishPacket, decode_publish, encode_publish};
use zerodds_mqtt_bridge::control_packets::{
    ConnectBody, decode_connect_body, decode_subscribe_body,
};
use zerodds_mqtt_bridge::daemon::config::{DaemonConfig, TenantConfig, TopicConfig};
use zerodds_mqtt_bridge::daemon::server::{self, DaemonHandle};
use zerodds_mqtt_bridge::net::{MqttClient as TestClient, frame_packet, read_packet};
use zerodds_mqtt_bridge::packet::ControlPacketType;
use zerodds_mqtt_bridge::property_set::{PropertyContext, PropertySet};
use zerodds_mqtt_bridge::server::{Authenticator, MqttBrokerServer, ServerConfig, ServerHandle};
use zerodds_mqtt_bridge::vbi::encode_vbi;
use zerodds_mqtt_bridge::version::ProtocolVersion;
use zerodds_rtps::header_extension::HeTimestamp;

const BIN: &str = env!("CARGO_BIN_EXE_zerodds-mqtt-bridged");
const TO: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

/// A directory of this test process for the files of one test.
fn scratch(leaf: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join(format!("zerodds-mqtt-bridge-e2e-{}", std::process::id()))
        .join(leaf);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// An empty base file, so no file of the host reaches the daemon.
fn base_config(dir: &std::path::Path) -> PathBuf {
    let p = dir.join("mqtt-bridged.yaml");
    std::fs::write(&p, "log_level: info\n").unwrap();
    p
}

/// A port nothing listens on, of [`zerodds_test_domains::free_tcp_port`].
fn free_port() -> u16 {
    zerodds_test_domains::free_tcp_port()
}

/// The daemon process, killed and reaped when the test ends.
struct Daemon(std::process::Child);

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Runs the binary with `args` until it exits, at most `within`: its exit
/// code, its stderr and then its stdout (the JSON log of §8.1), and how
/// long it ran.
fn run_to_exit(args: &[String], within: Duration) -> (Option<i32>, String, Duration) {
    let t0 = Instant::now();
    let mut child = Daemon(
        Command::new(BIN)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let out = Log::of(child.0.stdout.take().unwrap());
    let status = loop {
        if let Some(st) = child.0.try_wait().unwrap() {
            break st;
        }
        assert!(t0.elapsed() < within, "still running after {within:?}");
        thread::sleep(Duration::from_millis(20));
    };
    let mut err = String::new();
    child
        .0
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .unwrap();
    out.wait_closed();
    err.push_str(&out.text());
    (status.code(), err, t0.elapsed())
}

/// The lines a daemon process writes on a pipe, read by a thread of their
/// own.
#[derive(Clone)]
struct Log {
    text: Arc<Mutex<String>>,
    closed: Arc<AtomicBool>,
}

impl Log {
    fn of(mut pipe: impl Read + Send + 'static) -> Self {
        let me = Self {
            text: Arc::default(),
            closed: Arc::default(),
        };
        let (text, closed) = (Arc::clone(&me.text), Arc::clone(&me.closed));
        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while let Ok(n) = pipe.read(&mut buf) {
                if n == 0 {
                    break;
                }
                text.lock()
                    .unwrap()
                    .push_str(&String::from_utf8_lossy(&buf[..n]));
            }
            closed.store(true, Ordering::SeqCst);
        });
        me
    }

    fn text(&self) -> String {
        self.text.lock().unwrap().clone()
    }

    /// The complete lines read so far, each a JSON object (§8.1).
    fn lines(&self) -> Vec<serde_json::Value> {
        let text = self.text();
        let complete = text.rfind('\n').map_or("", |i| &text[..i]);
        complete
            .lines()
            .map(|l| {
                serde_json::from_str(l).unwrap_or_else(|e| panic!("not a JSON line: {l}: {e}"))
            })
            .collect()
    }

    /// The lines of `event`.
    fn events(&self, event: &str) -> Vec<serde_json::Value> {
        self.lines()
            .into_iter()
            .filter(|v| v["event"] == event)
            .collect()
    }

    /// Waits for the first line of `event`.
    fn wait_for(&self, event: &str) -> serde_json::Value {
        let mut found = None;
        wait_until(&format!("the event {event}"), || {
            found = self.events(event).into_iter().next();
            found.is_some()
        });
        found.unwrap()
    }

    /// Waits until the writer closed the pipe.
    fn wait_closed(&self) {
        wait_until("the end of the output", || {
            self.closed.load(Ordering::SeqCst)
        });
    }
}

fn args(dir: &std::path::Path, domain: i32, rest: &[&str]) -> Vec<String> {
    let mut out = vec![
        "--config".to_string(),
        base_config(dir).display().to_string(),
        "--domain".to_string(),
        domain.to_string(),
    ];
    out.extend(rest.iter().map(|s| (*s).to_string()));
    out
}

/// A CONNECT's client identifier, user name and password.
type Credentials = (String, Option<String>, Option<Vec<u8>>);

/// An Authenticator that records each CONNECT's credentials and answers
/// with `answer`.
struct Recording {
    seen: Mutex<Vec<Credentials>>,
    answer: Result<(), u8>,
}

impl Authenticator for Recording {
    fn authenticate(
        &self,
        client_id: &str,
        user_name: Option<&str>,
        password: Option<&[u8]>,
    ) -> Result<(), u8> {
        self.seen.lock().unwrap().push((
            client_id.to_string(),
            user_name.map(str::to_string),
            password.map(<[u8]>::to_vec),
        ));
        self.answer
    }

    fn start(
        &self,
        _client_id: &str,
        _method: &str,
    ) -> Option<Box<dyn zerodds_mqtt_bridge::server::AuthExchange>> {
        None
    }
}

/// The crate's broker server on an ephemeral port.
fn broker_server(config: ServerConfig) -> ServerHandle {
    MqttBrokerServer::bind_with("127.0.0.1:0", config)
        .unwrap()
        .spawn()
        .unwrap()
}

fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let deadline = Instant::now() + TO;
    while !cond() {
        assert!(Instant::now() < deadline, "{what} within {TO:?}");
        thread::sleep(Duration::from_millis(20));
    }
}

/// A broker that records the CONNECT and the PUBLISH packets of the one
/// client it serves, answers CONNECT with `connack` properties, SUBSCRIBE
/// with SUBACK, a QoS 1 PUBLISH with PUBACK (held back while
/// [`Recorder::hold_acks`] is set), PINGREQ with PINGRESP, and sends the
/// packets the test injects.
struct Recorder {
    addr: SocketAddr,
    connect: Arc<Mutex<Option<ConnectBody>>>,
    subscribed: Arc<AtomicBool>,
    publishes: Arc<Mutex<Vec<PublishPacket>>>,
    conn: Arc<Mutex<Option<TcpStream>>>,
    /// While set, the PUBACKs wait in `held`.
    hold: Arc<AtomicBool>,
    held: Arc<Mutex<Vec<u16>>>,
    /// Each SUBSCRIBE, as it came.
    subscribes: Arc<Mutex<Vec<zerodds_mqtt_bridge::control_packets::SubscribeBody>>>,
    /// The filters whose SUBSCRIBE gets the Reason Code 0x87.
    refuse: Arc<Mutex<Vec<String>>>,
    /// The PUBLISH packets on `$zerodds/` topics.
    dollar: Arc<Mutex<Vec<PublishPacket>>>,
}

impl Recorder {
    fn start(connack: PropertySet) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let me = Self {
            addr,
            connect: Arc::default(),
            subscribed: Arc::default(),
            publishes: Arc::default(),
            conn: Arc::default(),
            hold: Arc::default(),
            held: Arc::default(),
            subscribes: Arc::default(),
            refuse: Arc::default(),
            dollar: Arc::default(),
        };
        let (connect, subscribed, publishes, conn, hold, held) = (
            Arc::clone(&me.connect),
            Arc::clone(&me.subscribed),
            Arc::clone(&me.publishes),
            Arc::clone(&me.conn),
            Arc::clone(&me.hold),
            Arc::clone(&me.held),
        );
        let (subscribes, refuse, dollar) = (
            Arc::clone(&me.subscribes),
            Arc::clone(&me.refuse),
            Arc::clone(&me.dollar),
        );
        thread::spawn(move || {
            let Ok((mut sock, _)) = listener.accept() else {
                return;
            };
            *conn.lock().unwrap() = Some(sock.try_clone().unwrap());
            while let Ok((byte0, body)) = read_packet(&mut sock) {
                let send = |sock: &mut TcpStream, b0: u8, b: &[u8]| {
                    let _ = sock.write_all(&frame_packet(b0, b).unwrap());
                };
                match ControlPacketType::from_bits(byte0 >> 4) {
                    Some(ControlPacketType::Connect) => {
                        *connect.lock().unwrap() = Some(decode_connect_body(&body).unwrap());
                        let p = connack.encode().unwrap();
                        let mut ack = vec![0, 0];
                        ack.extend(encode_vbi(p.len() as u32).unwrap());
                        ack.extend(p);
                        send(&mut sock, 0x20, &ack);
                    }
                    Some(ControlPacketType::Subscribe) => {
                        let sub = decode_subscribe_body(&body).unwrap();
                        let mut ack = sub.packet_id.to_be_bytes().to_vec();
                        ack.push(0);
                        let refused = refuse.lock().unwrap().clone();
                        ack.extend(sub.subscriptions.iter().map(|s| {
                            if refused.contains(&s.topic_filter) {
                                0x87
                            } else {
                                s.options & 0x03
                            }
                        }));
                        subscribes.lock().unwrap().push(sub);
                        send(&mut sock, 0x90, &ack);
                        subscribed.store(true, Ordering::SeqCst);
                    }
                    Some(ControlPacketType::Publish) => {
                        let mut full = vec![byte0];
                        full.extend(encode_vbi(body.len() as u32).unwrap());
                        full.extend(&body);
                        let (_, p) = decode_publish(&full).unwrap();
                        let id = p.packet_id;
                        // The daemon's own topics (the catalog of §5.2) are
                        // kept apart and always acknowledged.
                        let own = p.topic.starts_with("$zerodds/");
                        if own {
                            dollar.lock().unwrap().push(p.clone());
                        } else {
                            publishes.lock().unwrap().push(p.clone());
                        }
                        if let (1, Some(id)) = (p.qos, id) {
                            let mut held = held.lock().unwrap();
                            if hold.load(Ordering::SeqCst) && !own {
                                held.push(id);
                            } else {
                                let mut ack = id.to_be_bytes().to_vec();
                                ack.push(0);
                                send(&mut sock, 0x40, &ack);
                            }
                        }
                    }
                    Some(ControlPacketType::PingReq) => send(&mut sock, 0xD0, &[]),
                    Some(ControlPacketType::Disconnect) => break,
                    _ => {}
                }
            }
        });
        me
    }

    fn url(&self) -> String {
        format!("mqtt://{}", self.addr)
    }

    /// Sends a PUBLISH QoS 0 to the client.
    fn inject(&self, topic: &str, payload: &[u8], props: &PropertySet) {
        let frame = encode_publish(&PublishPacket {
            dup: false,
            qos: 0,
            retain: false,
            topic: topic.to_string(),
            packet_id: None,
            properties: props.encode().unwrap(),
            payload: payload.to_vec(),
        })
        .unwrap();
        self.conn
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .write_all(&frame)
            .unwrap();
    }

    fn wait_subscribed(&self) {
        wait_until("SUBSCRIBE", || self.subscribed.load(Ordering::SeqCst));
    }

    /// Sends `frame` to the client as it is.
    fn send_raw(&self, frame: &[u8]) {
        self.conn
            .lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .write_all(frame)
            .unwrap();
    }

    /// From now on the PUBACKs wait until [`Self::release_acks`].
    fn hold_acks(&self) {
        self.hold.store(true, Ordering::SeqCst);
    }

    /// Sends the PUBACKs held back and acknowledges at once again.
    fn release_acks(&self) {
        let mut held = self.held.lock().unwrap();
        self.hold.store(false, Ordering::SeqCst);
        let mut conn = self.conn.lock().unwrap();
        let sock = conn.as_mut().unwrap();
        for id in held.drain(..) {
            let mut ack = id.to_be_bytes().to_vec();
            ack.push(0);
            sock.write_all(&frame_packet(0x40, &ack).unwrap()).unwrap();
        }
    }

    /// The first payload octet after the encapsulation header of each
    /// PUBLISH on `topic`, in the order they came.
    fn markers(&self, topic: &str) -> Vec<u8> {
        self.publishes
            .lock()
            .unwrap()
            .iter()
            .filter(|p| p.topic == topic)
            .map(|p| p.payload[4])
            .collect()
    }
}

/// A topic entry of the in-process daemon.
fn topic(dds: &str, mqtt: &str, direction: &str) -> TopicConfig {
    TopicConfig {
        dds_name: dds.to_string(),
        dds_type: dds.to_string(),
        mqtt_topic: mqtt.to_string(),
        direction: direction.to_string(),
        reliability: "reliable".to_string(),
        durability: "volatile".to_string(),
        ..TopicConfig::default()
    }
}

/// The configuration of an in-process daemon towards `url` on a leased
/// domain.
fn config(url: &str, client_id: &str) -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.broker_url = url.to_string();
    cfg.client_id = client_id.to_string();
    cfg.session_dir = scratch(&format!("sessions-{client_id}"))
        .display()
        .to_string();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    (cfg, lease)
}

/// A reader of `topic` on `runtime`, beside the bridge's writer.
fn reader_on(
    runtime: &zerodds_dcps::runtime::DcpsRuntime,
    topic: &str,
) -> std::sync::mpsc::Receiver<UserSample> {
    runtime
        .register_user_reader(UserReaderConfig {
            topic_name: topic.to_string(),
            type_name: topic.to_string(),
            reliable: true,
            ..Default::default()
        })
        .unwrap()
        .1
}

/// A sample's payload, key hash and source timestamp.
type Alive = (Vec<u8>, Option<[u8; 16]>, Option<HeTimestamp>);

/// The next alive sample on `rx`.
fn next_alive(rx: &std::sync::mpsc::Receiver<UserSample>, within: Duration) -> Option<Alive> {
    let deadline = Instant::now() + within;
    while let Some(left) = deadline.checked_duration_since(Instant::now()) {
        match rx.recv_timeout(left) {
            Ok(UserSample::Alive {
                payload,
                key_hash,
                source_timestamp,
                ..
            }) => return Some((payload.to_vec(), key_hash, source_timestamp)),
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    None
}

fn shutdown(mut h: DaemonHandle) {
    h.shutdown();
}

// ---------------------------------------------------------------------
// §2: the binary, its flags and its exit codes
// ---------------------------------------------------------------------

/// Spec §2 exit 1: a configuration error, as a value outside a key's range
/// and a `mqtts://` URL without a TLS section; a `mqtt.session_dir` the
/// daemon cannot create for a lasting session (§4.1).
#[test]
fn mqttbridge_2_exit_1_on_a_config_error() {
    let dir = scratch("exit1");
    let lease = zerodds_test_domains::lease_domain();
    let plain = dir.join("plain-file");
    std::fs::write(&plain, b"x").unwrap();
    let below_a_file = format!("mqtt.session_dir={}", plain.join("sub").display());
    for (rest, needle) in [
        (
            &[
                "--set",
                "mqtt.clean_start=false",
                "--set",
                "mqtt.session_expiry_interval_secs=60",
                "--set",
                below_a_file.as_str(),
            ][..],
            "mqtt.session_dir",
        ),
        (
            &["--set", "mqtt.receive_maximum=0"][..],
            "mqtt.receive_maximum",
        ),
        (&["--broker", "mqtts://127.0.0.1:1"][..], "mqtt.broker_url"),
        (
            &["--set", "mqtt.tls.verify_hostname=false"][..],
            "mqtt.tls.verify_hostname",
        ),
    ] {
        let (code, err, _) = run_to_exit(&args(&dir, lease.id(), rest), TO);
        assert_eq!(code, Some(1), "{rest:?}: {err}");
        assert!(err.contains(needle), "{rest:?}: {err}");
    }
}

/// Spec §2 exit 2 and §9.1: no broker listens; the start tries again after
/// the waits of `mqtt.reconnect` and gives up after
/// `mqtt.connect_timeout_secs`.
#[test]
fn mqttbridge_2_exit_2_when_no_broker_listens() {
    let dir = scratch("exit2");
    let lease = zerodds_test_domains::lease_domain();
    let url = format!("mqtt://127.0.0.1:{}", free_port());
    let (code, err, took) = run_to_exit(
        &args(
            &dir,
            lease.id(),
            &[
                "--broker",
                &url,
                "--set",
                "mqtt.connect_timeout_secs=1",
                "--set",
                "mqtt.reconnect.initial_delay_ms=100",
                "--set",
                "mqtt.reconnect.max_delay_ms=200",
            ],
        ),
        TO,
    );
    assert_eq!(code, Some(2), "{err}");
    assert!(
        err.matches(r#""event":"connect_failed""#).count() >= 3,
        "{err}"
    );
    assert!(took >= Duration::from_millis(700), "{took:?}");
}

/// Spec §9.1: a SIGTERM while the start retries the broker is a normal
/// shutdown, exit 0.
#[test]
fn mqttbridge_9_1_sigterm_during_the_connect_retries_exits_0() {
    let dir = scratch("term-connecting");
    let lease = zerodds_test_domains::lease_domain();
    let url = format!("mqtt://127.0.0.1:{}", free_port());
    let mut child = Daemon(
        Command::new(BIN)
            .args(args(
                &dir,
                lease.id(),
                &[
                    "--broker",
                    &url,
                    "--set",
                    "mqtt.connect_timeout_secs=0",
                    "--set",
                    "mqtt.reconnect.initial_delay_ms=100",
                ],
            ))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    thread::sleep(Duration::from_millis(1500));
    assert!(
        child.0.try_wait().unwrap().is_none(),
        "ended before SIGTERM"
    );
    let killed = Command::new("kill")
        .args(["-TERM", &child.0.id().to_string()])
        .status()
        .unwrap();
    assert!(killed.success());
    let t0 = Instant::now();
    let status = loop {
        if let Some(st) = child.0.try_wait().unwrap() {
            break st;
        }
        assert!(t0.elapsed() < Duration::from_secs(5), "still running");
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(0));
}

/// Spec §2 exit 3: the DDS participant cannot start; domain 300 has no
/// ports under the port mapping of DDSI-RTPS 2.5 §9.6.1.1.
#[test]
fn mqttbridge_2_exit_3_when_the_participant_cannot_start() {
    let dir = scratch("exit3");
    let url = format!("mqtt://127.0.0.1:{}", free_port());
    let (code, err, _) = run_to_exit(&args(&dir, 300, &["--broker", &url]), TO);
    assert_eq!(code, Some(3), "{err}");
    assert!(err.contains("dds error"), "{err}");
}

/// A TLS listener with a certificate for `localhost` that answers no MQTT:
/// the handshake is all that matters.
fn tls_listener(cert_pem: &str, key_pem: &str) -> SocketAddr {
    use rustls::pki_types::{CertificateDer, PrivateKeyDer};
    let certs: Vec<CertificateDer<'static>> =
        rustls_pemfile::certs(&mut std::io::BufReader::new(cert_pem.as_bytes()))
            .map(Result::unwrap)
            .collect();
    let key = rustls_pemfile::pkcs8_private_keys(&mut std::io::BufReader::new(key_pem.as_bytes()))
        .next()
        .unwrap()
        .unwrap();
    let cfg = Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::new(
            zerodds_bridge_security::tls_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(certs, PrivateKeyDer::from(key))
        .unwrap(),
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    thread::spawn(move || {
        for sock in listener.incoming() {
            let Ok(mut sock) = sock else { continue };
            let mut conn = rustls::ServerConnection::new(Arc::clone(&cfg)).unwrap();
            let _ = sock.set_read_timeout(Some(Duration::from_secs(3)));
            while conn.is_handshaking() {
                if conn.complete_io(&mut sock).is_err() {
                    break;
                }
            }
        }
    });
    addr
}

/// Spec §2 exit 4: the broker's certificate is not signed by the CA of
/// `--tls-ca`; the TLS handshake fails before the CONNECT. It used to be a
/// failed write of the CONNECT, exit 2.
#[test]
fn mqttbridge_2_exit_4_on_an_untrusted_broker_certificate() {
    let dir = scratch("exit4");
    let lease = zerodds_test_domains::lease_domain();
    let broker = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let other = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let addr = tls_listener(&broker.cert.pem(), &broker.key_pair.serialize_pem());
    let ca = dir.join("other-ca.pem");
    std::fs::write(&ca, other.cert.pem()).unwrap();
    let url = format!("mqtts://localhost:{}", addr.port());
    let (code, err, _) = run_to_exit(
        &args(
            &dir,
            lease.id(),
            &["--broker", &url, "--tls-ca", ca.to_str().unwrap()],
        ),
        TO,
    );
    assert_eq!(code, Some(4), "{err}");
    assert!(err.contains("handshake"), "{err}");

    // Counter-check: with the broker's own certificate as the CA the handshake
    // passes; the listener answers no CONNECT, so the start fails later and
    // not with exit 4.
    let own = dir.join("own-ca.pem");
    std::fs::write(&own, broker.cert.pem()).unwrap();
    let (code, err, _) = run_to_exit(
        &args(
            &dir,
            lease.id(),
            &[
                "--broker",
                &url,
                "--tls-ca",
                own.to_str().unwrap(),
                "--set",
                "mqtt.connect_timeout_secs=1",
            ],
        ),
        TO,
    );
    assert_ne!(code, Some(4), "{err}");
}

/// Spec §2 exit 2, §9.1 step 6: a broker that accepts TCP and does not
/// answer the TLS ClientHello is a broker that does not answer in time. The
/// start tries it again and gives up with exit 2 after
/// `mqtt.connect_timeout_secs`; it used to end at once with exit 4. Each
/// attempt waits 5 s for the answer: the attempts start at 0 and 5.1 s.
#[test]
fn mqttbridge_2_exit_2_when_the_tls_handshake_gets_no_answer() {
    let dir = scratch("exit2-silent-tls");
    let lease = zerodds_test_domains::lease_domain();
    let broker = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let ca = dir.join("ca.pem");
    std::fs::write(&ca, broker.cert.pem()).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let accepted = Arc::new(Mutex::new(Vec::new()));
    let held = Arc::clone(&accepted);
    thread::spawn(move || {
        for sock in listener.incoming().flatten() {
            // Holds the connection and reads nothing.
            held.lock().unwrap().push(sock);
        }
    });
    let url = format!("mqtts://localhost:{port}");
    let (code, err, took) = run_to_exit(
        &args(
            &dir,
            lease.id(),
            &[
                "--broker",
                &url,
                "--tls-ca",
                ca.to_str().unwrap(),
                "--set",
                "mqtt.connect_timeout_secs=6",
                "--set",
                "mqtt.reconnect.initial_delay_ms=100",
            ],
        ),
        Duration::from_secs(20),
    );
    assert_eq!(code, Some(2), "{err}");
    assert_eq!(
        err.matches(r#""event":"connect_failed""#).count(),
        1,
        "{err}"
    );
    assert!(accepted.lock().unwrap().len() >= 2, "two attempts");
    assert!(took >= Duration::from_secs(6), "{took:?}");
}

/// Spec §2 exit 4: TLS material that does not load (here `mqtt.tls.ca_file`
/// names no file) ends the start with exit 4, before any connect.
#[test]
fn mqttbridge_2_exit_4_when_the_tls_material_does_not_load() {
    let dir = scratch("exit4-material");
    let lease = zerodds_test_domains::lease_domain();
    let missing = dir.join("no-such-ca.pem");
    let url = format!("mqtts://localhost:{}", free_port());
    let (code, err, _) = run_to_exit(
        &args(
            &dir,
            lease.id(),
            &["--broker", &url, "--tls-ca", missing.to_str().unwrap()],
        ),
        TO,
    );
    assert_eq!(code, Some(4), "{err}");
    assert!(err.contains("no-such-ca.pem"), "{err}");
    assert!(err.contains(r#""event":"start_failed""#), "{err}");
    assert!(!err.contains(r#""event":"connect_failed""#), "{err}");
}

/// Spec §2 exit 5: the broker refuses the authorization (0x87); in the
/// compatibility mode it answers return code 5 (MQTT 3.1.1), also exit 5.
/// A refusal for another reason (0x85) is exit 2.
#[test]
fn mqttbridge_2_exit_5_on_connack_0x87() {
    let dir = scratch("exit5");
    let lease = zerodds_test_domains::lease_domain();
    for (answer, version, want) in [
        (0x87, "5", 5),
        (0x87, "3.1.1", 5),
        (0x86, "5", 5),
        (0x85, "5", 2),
    ] {
        let h = broker_server(ServerConfig {
            authenticator: Some(Arc::new(Recording {
                seen: Mutex::default(),
                answer: Err(answer),
            })),
            ..ServerConfig::default()
        });
        let url = format!("mqtt://{}", h.local_addr());
        let (code, err, _) = run_to_exit(
            &args(
                &dir,
                lease.id(),
                &[
                    "--broker",
                    &url,
                    "--set",
                    &format!("mqtt.protocol_version={version}"),
                ],
            ),
            TO,
        );
        assert_eq!(code, Some(want), "0x{answer:02x} MQTT {version}: {err}");
        h.shutdown();
    }
}

/// Spec §2: `--broker`, `--client-id`, `--username`, `--password`,
/// `--topic` and `--metrics` reach the CONNECT, the SUBSCRIBE and the admin
/// endpoint of the running daemon: the broker server records the
/// credentials, a message on the topic of `--topic` reaches the daemon, its
/// metrics count it. `--domain`, `--log-level` and the `--tls-*` flags
/// reach the configuration `--check-config` prints.
#[test]
fn mqttbridge_2_every_flag_reaches_the_connect_and_the_subscribe() {
    let dir = scratch("flags");
    let lease = zerodds_test_domains::lease_domain();
    let auth = Arc::new(Recording {
        seen: Mutex::default(),
        answer: Ok(()),
    });
    let h = broker_server(ServerConfig {
        authenticator: Some(Arc::clone(&auth) as Arc<dyn Authenticator>),
        ..ServerConfig::default()
    });
    let url = format!("mqtt://{}", h.local_addr());
    let mut daemon = Daemon(
        Command::new(BIN)
            .args(args(
                &dir,
                lease.id(),
                &[
                    "--broker",
                    &url,
                    "--client-id",
                    "flag-cli",
                    "--username",
                    "flag-user",
                    "--password",
                    "flag-pass",
                    "--topic",
                    "Flag::T:flag/t",
                    "--metrics",
                    "127.0.0.1:0",
                ],
            ))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    // Spec §8.1: the JSON log is on stdout; `admin_bound` names the port
    // the admin endpoint bound for port 0.
    let metrics = {
        use std::io::BufRead as _;
        let stdout = daemon.0.stdout.take().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        thread::spawn(move || {
            // To the end: the daemon blocks on a full pipe.
            for line in std::io::BufReader::new(stdout)
                .lines()
                .map_while(Result::ok)
            {
                if let Some(port) =
                    zerodds_test_domains::logged_port(&line, "admin_bound", "listen")
                {
                    let _ = tx.send(port);
                }
            }
        });
        let port = rx.recv_timeout(TO).expect("the admin_bound line");
        format!("127.0.0.1:{port}")
    };
    wait_until("the CONNECT", || !auth.seen.lock().unwrap().is_empty());
    assert_eq!(
        auth.seen.lock().unwrap()[0],
        (
            "flag-cli".to_string(),
            Some("flag-user".to_string()),
            Some(b"flag-pass".to_vec())
        )
    );
    let get_metrics = || -> String {
        let Ok(mut s) = TcpStream::connect(&metrics) else {
            return String::new();
        };
        let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = s.write_all(b"GET /metrics HTTP/1.1\r\nHost: x\r\n\r\n");
        let mut body = String::new();
        let _ = s.read_to_string(&mut body);
        body
    };
    let mut publisher =
        TestClient::connect(h.local_addr(), "flag-pub", ProtocolVersion::V5).unwrap();
    let deadline = Instant::now() + TO;
    loop {
        // Until the daemon's subscription stands, a message reaches nobody.
        publisher
            .publish("flag/t", &[0x00, 0x01, 0x00, 0x00, 7], 0, false)
            .unwrap();
        if get_metrics()
            .contains("zerodds_mqtt_bridge_dds_samples_published_total{dds_topic=\"Flag::T\"} 1")
        {
            break;
        }
        assert!(Instant::now() < deadline, "{}", get_metrics());
        thread::sleep(Duration::from_millis(200));
    }
    h.shutdown();

    let printed: Output = Command::new(BIN)
        .args(args(
            &dir,
            lease.id(),
            &[
                "--check-config",
                "--log-level",
                "warn",
                "--broker",
                "mqtts://broker.example:8883",
                "--tls-ca",
                "/ca.pem",
                "--tls-cert",
                "/cert.pem",
                "--tls-key",
                "/key.pem",
            ],
        ))
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&printed.stdout);
    assert_eq!(printed.status.code(), Some(0), "{text}");
    for want in [
        format!("domain = {}", lease.id()),
        "log_level = warn".to_string(),
        "mqtt.tls.ca_file = /ca.pem".to_string(),
        "mqtt.tls.cert_file = /cert.pem".to_string(),
        "mqtt.tls.key_file = /key.pem".to_string(),
        "mqtt.tls.enabled = true".to_string(),
    ] {
        assert!(text.contains(&want), "missing `{want}` in\n{text}");
    }
}

// ---------------------------------------------------------------------
// §4.1: the CONNECT and the limits of both sides
// ---------------------------------------------------------------------

/// Spec §4.1: the CONNECT the broker reads carries the properties of the
/// table with the values of the configuration.
#[test]
fn mqttbridge_4_1_the_broker_reads_each_connect_property() {
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "props-cli");
    cfg.clean_start = false;
    cfg.session_expiry_interval_secs = 300;
    cfg.receive_maximum = 10;
    cfg.max_packet_size = 4096;
    cfg.topic_alias_maximum = 7;
    cfg.keep_alive_secs = 20;
    let h = server::start(cfg).expect("daemon start");
    let c = rec.connect.lock().unwrap().clone().expect("CONNECT");
    assert_eq!((c.protocol_version, c.keep_alive), (5, 20));
    assert_eq!(c.connect_flags & 0x02, 0, "Clean Start 0");
    assert_eq!(c.client_id, "props-cli");
    let p = PropertySet::parse(
        &c.properties,
        PropertyContext::Packet(ControlPacketType::Connect),
    )
    .unwrap();
    assert_eq!(p.session_expiry_interval, Some(300));
    assert_eq!(p.receive_maximum, Some(10));
    assert_eq!(p.maximum_packet_size, Some(4096));
    assert_eq!(p.topic_alias_maximum, Some(7));
    assert_eq!(p.request_response_information, Some(0));
    assert_eq!(p.request_problem_information, Some(1));
    assert_eq!(
        p.user_properties,
        [
            ("zerodds_version".to_string(), "1.0".to_string()),
            ("zerodds_role".to_string(), "bridge".to_string())
        ]
    );
    shutdown(h);
}

/// Spec §4.1, [MQTT-3.1.2-23], [MQTT-4.4.0-1]: a daemon with a session that
/// outlives its connection keeps its state in `mqtt.session_dir`. The first
/// daemon publishes a QoS 1 message the broker does not acknowledge and
/// stops; the second one, after Session Present 1, sends that PUBLISH
/// again with DUP and its packet identifier. Before, every start began at
/// identifier 1 with no state, and nothing was resent.
#[test]
fn mqttbridge_4_1_a_restarted_daemon_resends_what_the_broker_did_not_acknowledge() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<(u8, PublishPacket)>();
    let broker = thread::spawn(move || {
        for present in [0u8, 1] {
            let (mut sock, _) = listener.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let (_, body) = read_packet(&mut sock).unwrap();
            let c = decode_connect_body(&body).unwrap();
            assert_eq!(c.connect_flags & 0x02, 0, "Clean Start 0");
            sock.write_all(&frame_packet(0x20, &[present, 0, 0]).unwrap())
                .unwrap();
            while let Ok((byte0, body)) = read_packet(&mut sock) {
                match ControlPacketType::from_bits(byte0 >> 4) {
                    Some(ControlPacketType::Publish) => {
                        let mut full = vec![byte0];
                        full.extend(encode_vbi(body.len() as u32).unwrap());
                        full.extend(&body);
                        let (_, p) = decode_publish(&full).unwrap();
                        if present == 1 {
                            let mut ack = p.packet_id.unwrap().to_be_bytes().to_vec();
                            ack.push(0);
                            let _ = sock.write_all(&frame_packet(0x40, &ack).unwrap());
                        }
                        // The catalog of §5.2 is not the PUBLISH under test.
                        if !p.topic.starts_with("$zerodds/") {
                            tx.send((present, p)).unwrap();
                        }
                    }
                    Some(ControlPacketType::PingReq) => {
                        let _ = sock.write_all(&[0xD0, 0x00]);
                    }
                    Some(ControlPacketType::Disconnect) => break,
                    _ => {}
                }
            }
        }
    });
    let (mut cfg, _lease) = config(&format!("mqtt://{addr}"), "restart-cli");
    cfg.clean_start = false;
    cfg.session_expiry_interval_secs = 300;
    cfg.drain_timeout_secs = 1;
    let mut t = topic("Restart", "restart/out", "out");
    t.mqtt_qos = Some(1);
    cfg.topics.push(t);

    let first = server::start(cfg.clone()).expect("first start");
    assert!(
        first
            .runtime
            .test_inject_user_alive(first.user_readers["Restart"], vec![7])
    );
    let (conn, sent) = rx.recv_timeout(TO).expect("the PUBLISH");
    assert_eq!((conn, sent.dup, sent.payload[4]), (0, false, 7));
    shutdown(first);
    let file = zerodds_mqtt_bridge::daemon::session::session_file(&cfg);
    assert!(file.exists(), "{}", file.display());

    let second = server::start(cfg).expect("second start");
    let (conn, again) = rx.recv_timeout(TO).expect("the PUBLISH again");
    assert_eq!(conn, 1);
    assert!(again.dup, "DUP 1 on the resend");
    assert_eq!(again.packet_id, sent.packet_id);
    assert_eq!((again.topic.as_str(), again.payload[4]), ("restart/out", 7));
    // A record `C`, the identifier, length 0 — the broker completed the
    // flow (the format of daemon/session.rs); the catalog's flows of §5.2
    // follow it.
    let pid = again.packet_id.unwrap().to_be_bytes();
    // The file's octets on a timeout: the test once ran into its 10 s here
    // with 63 others beside it (OPEN-ITEMS 77).
    let deadline = Instant::now() + TO;
    loop {
        let bytes = std::fs::read(&file).unwrap_or_default();
        if bytes
            .windows(7)
            .any(|w| w == [b'C', pid[0], pid[1], 0, 0, 0, 0])
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "no completion of {pid:?} in the session file within {TO:?}: {bytes:?}"
        );
        thread::sleep(Duration::from_millis(20));
    }
    shutdown(second);
    broker.join().unwrap();
}

/// Spec §4.1, MQTT 5.0 §3.3.2.3.4: a Topic Alias the broker sets is
/// resolved for the PUBLISH packets that carry it alone; both reach DDS.
#[test]
fn mqttbridge_4_1_an_inbound_topic_alias_is_resolved() {
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "alias-cli");
    cfg.topics.push(topic("Alias", "alias/t", "in"));
    let h = server::start(cfg).expect("daemon start");
    let rx = reader_on(&h.runtime, "Alias");
    rec.wait_subscribed();
    let alias = PropertySet {
        topic_alias: Some(5),
        ..PropertySet::default()
    };
    rec.inject("alias/t", &[0x00, 0x01, 0x00, 0x00, 1], &alias);
    rec.inject("", &[0x00, 0x01, 0x00, 0x00, 2], &alias);
    for want in [1u8, 2] {
        let (payload, _, _) = next_alive(&rx, TO).expect("sample");
        assert_eq!(payload, [want]);
    }
    shutdown(h);
}

/// Spec §4.1: the broker's Maximum QoS bounds the daemon's PUBLISH: a topic
/// with QoS 2 towards a broker of Maximum QoS 1 is not published (counted
/// as an error), one with QoS 1 is.
#[test]
fn mqttbridge_4_1_the_maximum_qos_of_the_connack_holds_for_the_publish() {
    let rec = Recorder::start(PropertySet {
        maximum_qos: Some(1),
        ..PropertySet::default()
    });
    let (mut cfg, _lease) = config(&rec.url(), "maxqos-cli");
    let mut two = topic("Two", "q/two", "out");
    two.mqtt_qos = Some(2);
    cfg.topics.push(two);
    cfg.topics.push(topic("One", "q/one", "out"));
    let h = server::start(cfg).expect("daemon start");
    let errors = h.metrics.as_ref().unwrap().errors_total.get();
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["Two"], vec![2])
    );
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["One"], vec![1])
    );
    wait_until("the QoS 1 PUBLISH", || {
        rec.publishes
            .lock()
            .unwrap()
            .iter()
            .any(|p| p.topic == "q/one")
    });
    thread::sleep(Duration::from_millis(300));
    assert!(
        !rec.publishes
            .lock()
            .unwrap()
            .iter()
            .any(|p| p.topic == "q/two"),
        "a QoS 2 PUBLISH to a broker of Maximum QoS 1"
    );
    assert!(h.metrics.as_ref().unwrap().errors_total.get() > errors);
    shutdown(h);
}

// ---------------------------------------------------------------------
// §4.2: the properties of a PUBLISH
// ---------------------------------------------------------------------

/// Spec §4.2: a DDS sample with a key hash, a source timestamp and a
/// lifespan is published with Payload Format Indicator 0, its Content
/// Type, the user properties of the table and the Message Expiry Interval
/// of its lifespan.
#[test]
fn mqttbridge_4_2_a_dds_sample_is_published_with_content_type_and_user_properties() {
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "props-out");
    cfg.topics.push(topic("Keyed", "keyed/out", "out"));
    let h = server::start(cfg).expect("daemon start");
    let writer = h
        .runtime
        .register_user_writer(UserWriterConfig {
            topic_name: "Keyed".into(),
            type_name: "Keyed".into(),
            reliable: true,
            lifespan: zerodds_qos::LifespanQosPolicy {
                duration: zerodds_qos::Duration::from_secs(3),
            },
            ..Default::default()
        })
        .unwrap();
    wait_until("the bridge's reader matched", || {
        !h.runtime
            .user_writer_matched_reader_guids(writer)
            .is_empty()
    });
    let key = [0x5a; 16];
    let ts = HeTimestamp {
        seconds: 1_700_000_000,
        fraction: 0x4000_0000,
    };
    h.runtime
        .write_user_sample_keyed(writer, &[9, 9, 9, 9], key, Some(ts))
        .unwrap();
    wait_until("the PUBLISH", || !rec.publishes.lock().unwrap().is_empty());
    let p = rec.publishes.lock().unwrap()[0].clone();
    assert_eq!(p.topic, "keyed/out");
    assert_eq!(&p.payload[4..], &[9, 9, 9, 9]);
    let props = PropertySet::parse(
        &p.properties,
        PropertyContext::Packet(ControlPacketType::Publish),
    )
    .unwrap();
    assert_eq!(props.payload_format_indicator, Some(0));
    let want_ct = if p.payload[1] <= 0x03 {
        "application/x-dds-cdr"
    } else {
        "application/x-dds-cdr2"
    };
    assert_eq!(props.content_type.as_deref(), Some(want_ct));
    assert_eq!(props.message_expiry_interval, Some(3));
    let user: Vec<(&str, &str)> = props
        .user_properties
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    assert_eq!(
        user,
        [
            ("zerodds_type", "Keyed"),
            ("zerodds_topic", "Keyed"),
            ("zerodds_flags", "0x00"),
            ("zerodds_key_hash", "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a"),
            ("zerodds_source_ts_ns", "1700000000250000000"),
        ]
    );
    shutdown(h);
}

/// Spec §4.2 MQTT → DDS: `zerodds_source_ts_ns` is the source timestamp and
/// `zerodds_key_hash` the key hash of the DDS sample; a PUBLISH whose
/// Content Type is no CDR is refused and counted, the connection stays.
#[test]
fn mqttbridge_4_2_an_inbound_source_timestamp_is_the_dds_source_timestamp() {
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "props-in");
    cfg.topics.push(topic("Stamped", "stamped/in", "in"));
    let h = server::start(cfg).expect("daemon start");
    let rx = reader_on(&h.runtime, "Stamped");
    rec.wait_subscribed();
    let errors = h.metrics.as_ref().unwrap().errors_total.get();
    rec.inject(
        "stamped/in",
        &[0x00, 0x01, 0x00, 0x00, 1],
        &PropertySet {
            content_type: Some("application/json".into()),
            ..PropertySet::default()
        },
    );
    rec.inject(
        "stamped/in",
        &[0x00, 0x07, 0x00, 0x00, 2],
        &PropertySet {
            content_type: Some("application/x-dds-cdr2".into()),
            user_properties: vec![
                (
                    "zerodds_key_hash".into(),
                    "00112233445566778899aabbccddeeff".into(),
                ),
                ("zerodds_source_ts_ns".into(), "1700000000500000000".into()),
            ],
            ..PropertySet::default()
        },
    );
    let (payload, key, ts) = next_alive(&rx, TO).expect("the second PUBLISH");
    assert_eq!(payload, [2], "the first one is refused");
    assert_eq!(
        key,
        Some([
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff
        ])
    );
    let ts = ts.expect("a source timestamp");
    assert_eq!((ts.seconds, ts.fraction), (1_700_000_000, 0x8000_0000));
    assert!(h.metrics.as_ref().unwrap().errors_total.get() > errors);
    shutdown(h);
}

// ---------------------------------------------------------------------
// §4.3, §5.1: the subscriptions and the topics of the broker's messages
// ---------------------------------------------------------------------

/// A writer of `topic` on the daemon's participant, matched with the
/// bridge's reader and the test's reader of the topic.
fn writer_on(h: &DaemonHandle, topic: &str) -> zerodds_rtps::wire_types::EntityId {
    let w = h
        .runtime
        .register_user_writer(UserWriterConfig {
            topic_name: topic.into(),
            type_name: topic.into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    wait_until("the writer's readers", || {
        h.runtime.user_writer_matched_reader_guids(w).len() >= 2
    });
    w
}

/// The properties of a PUBLISH with the Subscription Identifiers `ids`.
fn with_ids(ids: &[u32]) -> PropertySet {
    PropertySet {
        subscription_identifiers: ids.to_vec(),
        ..PropertySet::default()
    }
}

/// Spec §4.3: one SUBSCRIBE per filter with its Subscription Identifier
/// (counted from 1, shared by topics with the same filter), QoS 1 for a
/// reliable topic and No Local 1; an `out` topic subscribes nothing. A
/// PUBLISH goes to the topics its identifier names; one whose identifier's
/// filter does not match its Topic Name is not written and counted.
#[test]
fn mqttbridge_4_3_each_filter_is_subscribed_with_its_identifier_and_no_local() {
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "sub-ids");
    cfg.topics.push(topic("SubOut", "sub/out", "out"));
    cfg.topics.push(topic("Temp", "sub/+/temp", "in"));
    cfg.topics.push(topic("Chat", "sub/chat", "bidir"));
    cfg.topics.push(topic("Temp2", "sub/+/temp", "in"));
    let h = server::start(cfg).expect("daemon start");
    // `start` returns after the SUBACKs.
    let subs = rec.subscribes.lock().unwrap().clone();
    let seen: Vec<(String, u8, Vec<u32>)> = subs
        .iter()
        .map(|s| {
            assert_eq!(s.subscriptions.len(), 1, "one filter per SUBSCRIBE");
            let props = PropertySet::parse(
                &s.properties,
                PropertyContext::Packet(ControlPacketType::Subscribe),
            )
            .unwrap();
            (
                s.subscriptions[0].topic_filter.clone(),
                s.subscriptions[0].options,
                props.subscription_identifiers,
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("sub/+/temp".to_string(), 0x05, vec![1]),
            ("sub/chat".to_string(), 0x05, vec![2]),
        ]
    );
    let temp = reader_on(&h.runtime, "Temp");
    let temp2 = reader_on(&h.runtime, "Temp2");
    let chat = reader_on(&h.runtime, "Chat");
    rec.inject("sub/a/temp", &[0, 1, 0, 0, 1], &with_ids(&[1]));
    assert_eq!(next_alive(&temp, TO).unwrap().0, [1]);
    assert_eq!(next_alive(&temp2, TO).unwrap().0, [1]);
    rec.inject("sub/chat", &[0, 1, 0, 0, 2], &with_ids(&[2]));
    assert_eq!(next_alive(&chat, TO).unwrap().0, [2]);
    assert!(next_alive(&temp, Duration::from_millis(300)).is_none());
    let errors = h.metrics.as_ref().unwrap().errors_total.get();
    // Identifier 1 is `sub/+/temp`, which `sub/chat` does not match.
    rec.inject("sub/chat", &[0, 1, 0, 0, 3], &with_ids(&[1]));
    assert!(next_alive(&chat, Duration::from_millis(500)).is_none());
    assert!(next_alive(&temp, Duration::from_millis(100)).is_none());
    assert!(h.metrics.as_ref().unwrap().errors_total.get() > errors);
    shutdown(h);
}

/// Spec §4.3: a subscription the broker refuses (SUBACK 0x87) is logged
/// with its filter and Reason Code; the granted one carries messages.
#[test]
fn mqttbridge_4_3_a_refused_subscription_is_logged() {
    let dir = scratch("refused-sub");
    let lease = zerodds_test_domains::lease_domain();
    let rec = Recorder::start(PropertySet::default());
    rec.refuse.lock().unwrap().push("no/in".into());
    let file = dir.join("mqtt-bridged.yaml");
    std::fs::write(
        &file,
        format!(
            "mqtt:\n  broker_url: {}\n  client_id: refused-sub\n  session_dir: {}\ntopics:\n  - dds_name: Granted\n    mqtt_topic: ok/in\n    direction: in\n  - dds_name: Refused\n    mqtt_topic: no/in\n    direction: in\n",
            rec.url(),
            dir.join("sessions").display()
        ),
    )
    .unwrap();
    let mut child = Daemon(
        Command::new(BIN)
            .args(["--config", &file.display().to_string()])
            .args(["--domain", &lease.id().to_string()])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let log = Log::of(child.0.stdout.take().unwrap());
    let subscribed = log.wait_for("subscribed");
    assert_eq!(subscribed["granted"], 1, "{subscribed}");
    assert_eq!(subscribed["filters"], 2, "{subscribed}");
    let refused = log.events("subscription_refused");
    assert_eq!(refused.len(), 1, "{}", log.text());
    assert_eq!(refused[0]["level"], "warn");
    assert_eq!(refused[0]["mqtt_topic"], "no/in");
    assert_eq!(refused[0]["reason_code"], "0x87");
    assert_eq!(rec.subscribes.lock().unwrap().len(), 2);
}

/// Spec §4.3 on a bidir topic: a message of an MQTT client reaches DDS once
/// and is not published back (the bridge's own DataReader gets the sample
/// of its DataWriter, which came from the broker); a DDS sample is
/// published once, and the broker does not send it back (No Local), so it
/// does not reach DDS a second time.
#[test]
fn mqttbridge_4_3_a_bidir_topic_does_not_send_its_own_messages_back() {
    bidir_round(ProtocolVersion::V5, "loop-v5");
}

/// Spec §4.3 under `mqtt.protocol_version: 3.1.1`, which has no No Local:
/// the broker sends the bridge's PUBLISH on a bidir topic back, and the
/// daemon does not write that copy into DDS.
#[test]
fn mqttbridge_4_3_under_3_1_1_the_copy_of_its_own_publish_is_not_written() {
    bidir_round(ProtocolVersion::V311, "loop-v311");
}

fn bidir_round(version: ProtocolVersion, id: &str) {
    let b = broker_server(ServerConfig::default());
    let (mut cfg, _lease) = config(&format!("mqtt://{}", b.local_addr()), id);
    cfg.protocol_version = version;
    let dds_topic = format!("Loop{id}");
    let mqtt_topic = format!("loop/{id}");
    cfg.topics.push(topic(&dds_topic, &mqtt_topic, "bidir"));
    let h = server::start(cfg).expect("daemon start");
    let mut sub =
        TestClient::connect(b.local_addr(), &format!("{id}-sub"), ProtocolVersion::V5).unwrap();
    sub.subscribe(&mqtt_topic, 1).unwrap();
    let dds = reader_on(&h.runtime, &dds_topic);
    let mut publisher =
        TestClient::connect(b.local_addr(), &format!("{id}-pub"), ProtocolVersion::V5).unwrap();
    // MQTT → DDS.
    publisher
        .publish(&mqtt_topic, &[0x00, 0x01, 0x00, 0x00, 1], 1, false)
        .unwrap();
    assert_eq!(sub.recv_publish(TO).unwrap().1, [0x00, 0x01, 0x00, 0x00, 1]);
    assert_eq!(next_alive(&dds, TO).unwrap().0, [1]);
    assert!(
        sub.recv_publish(Duration::from_millis(800)).is_err(),
        "{version:?}: the bridge published the message back"
    );
    assert!(next_alive(&dds, Duration::from_millis(200)).is_none());
    // DDS → MQTT.
    let w = writer_on(&h, &dds_topic);
    h.runtime.write_user_sample(w, vec![2]).unwrap();
    let (t, p) = sub.recv_publish(TO).unwrap();
    assert_eq!((t.as_str(), &p[4..]), (mqtt_topic.as_str(), &[2u8][..]));
    // The test's reader gets the test's writer's sample, nothing more.
    assert_eq!(next_alive(&dds, TO).unwrap().0, [2]);
    assert!(
        next_alive(&dds, Duration::from_millis(800)).is_none(),
        "{version:?}: the broker's copy of the bridge's PUBLISH reached DDS"
    );
    assert!(sub.recv_publish(Duration::from_millis(200)).is_err());
    shutdown(h);
    b.shutdown();
}

/// Spec §5.1: an `in` topic may be a filter; a PUBLISH without
/// Subscription Identifier goes to it when its Topic Name matches the
/// filter, level by level (MQTT 5.0 §4.7); one that does not match is not
/// written and counted.
#[test]
fn mqttbridge_5_1_a_filter_topic_takes_every_topic_it_matches() {
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "filter-in");
    cfg.topics
        .push(topic("Telemetry", "factory/+/telemetry", "in"));
    let h = server::start(cfg).expect("daemon start");
    let rx = reader_on(&h.runtime, "Telemetry");
    for (n, press) in [(1u8, "press-1"), (2, "press-2")] {
        rec.inject(
            &format!("factory/{press}/telemetry"),
            &[0x00, 0x01, 0x00, 0x00, n],
            &PropertySet::default(),
        );
        assert_eq!(next_alive(&rx, TO).unwrap().0, [n]);
    }
    let errors = h.metrics.as_ref().unwrap().errors_total.get();
    rec.inject(
        "factory/press-1/x/telemetry",
        &[0x00, 0x01, 0x00, 0x00, 3],
        &PropertySet::default(),
    );
    assert!(next_alive(&rx, Duration::from_millis(500)).is_none());
    assert!(h.metrics.as_ref().unwrap().errors_total.get() > errors);
    shutdown(h);
}

// ---------------------------------------------------------------------
// §4.4: dispose and unregister
// ---------------------------------------------------------------------

/// A participant of the test on `domain`, beside the daemon's, reached over
/// RTPS.
fn foreign_participant(domain: i32, tag: u8) -> Arc<zerodds_dcps::runtime::DcpsRuntime> {
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[4..8].copy_from_slice(&domain.to_be_bytes());
    prefix[11] = tag;
    zerodds_dcps::runtime::DcpsRuntime::start(
        domain,
        zerodds_rtps::wire_types::GuidPrefix::from_bytes(prefix),
        zerodds_dcps::runtime::RuntimeConfig::default(),
    )
    .expect("the test's participant")
}

/// The user property `name` of a recorded PUBLISH.
fn user_prop(p: &PublishPacket, name: &str) -> Option<String> {
    PropertySet::parse(
        &p.properties,
        PropertyContext::Packet(ControlPacketType::Publish),
    )
    .unwrap()
    .user_properties
    .into_iter()
    .rev()
    .find(|(k, _)| k == name)
    .map(|(_, v)| v)
}

/// Spec §4.4 both ways, with a participant of the test over RTPS: its
/// dispose and its unregister of an instance become PUBLISH packets with
/// `zerodds_op` dispose / unregister, `zerodds_flags` and the key hash; a
/// PUBLISH with `zerodds_op` dispose and the key hash disposes that
/// instance in a DDS reader of the test (instance state NOT_ALIVE_DISPOSED),
/// an unknown operation is refused and counted.
#[test]
fn mqttbridge_4_4_a_dds_dispose_reaches_mqtt_and_an_mqtt_dispose_reaches_a_dds_reader() {
    use zerodds_rtps::history_cache::ChangeKind;
    use zerodds_rtps::inline_qos::status_info::{DISPOSED, UNREGISTERED};
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, lease) = config(&rec.url(), "lifecycle");
    cfg.topics.push(topic("Life", "life/out", "out"));
    cfg.topics.push(topic("LifeIn", "life/in", "in"));
    let h = server::start(cfg).expect("daemon start");
    let dds = foreign_participant(lease.id(), 0x44);
    let w = dds
        .register_user_writer(UserWriterConfig {
            topic_name: "Life".into(),
            type_name: "Life".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    let (_r, rx) = dds
        .register_user_reader(UserReaderConfig {
            topic_name: "LifeIn".into(),
            type_name: "LifeIn".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    wait_until("the bridge's reader over RTPS", || {
        !dds.user_writer_matched_reader_guids(w).is_empty()
    });
    let bridge_writer = h.user_writers["LifeIn"];
    wait_until("the bridge's writer over RTPS", || {
        !h.runtime
            .user_writer_matched_reader_guids(bridge_writer)
            .is_empty()
    });
    // DDS → MQTT.
    let key = [0x4a; 16];
    dds.write_user_sample_keyed(w, &[1, 2], key, None).unwrap();
    dds.write_user_lifecycle(w, key, None, DISPOSED, None)
        .unwrap();
    dds.write_user_lifecycle(w, key, None, UNREGISTERED, None)
        .unwrap();
    wait_until("three PUBLISH packets", || {
        rec.publishes.lock().unwrap().len() >= 3
    });
    let got = rec.publishes.lock().unwrap().clone();
    assert!(got.iter().all(|p| p.topic == "life/out"));
    assert_eq!(user_prop(&got[0], "zerodds_op"), None, "a sample has none");
    for (p, op, flags) in [
        (&got[1], "dispose", "0x01"),
        (&got[2], "unregister", "0x02"),
    ] {
        assert_eq!(user_prop(p, "zerodds_op").as_deref(), Some(op));
        assert_eq!(user_prop(p, "zerodds_flags").as_deref(), Some(flags));
        assert_eq!(
            user_prop(p, "zerodds_key_hash").as_deref(),
            Some("4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a4a")
        );
        assert_eq!(&p.payload[..2], &[0x00, 0x06]);
    }
    // MQTT → DDS.
    let key_props = |op: Option<&str>| PropertySet {
        user_properties: op
            .map(|o| ("zerodds_op".to_string(), o.to_string()))
            .into_iter()
            .chain([(
                "zerodds_key_hash".to_string(),
                "00112233445566778899aabbccddeeff".to_string(),
            )])
            .collect(),
        ..PropertySet::default()
    };
    rec.inject("life/in", &[0x00, 0x01, 0x00, 0x00, 9], &key_props(None));
    let errors = h.metrics.as_ref().unwrap().errors_total.get();
    rec.inject("life/in", &[], &key_props(Some("erase")));
    rec.inject("life/in", &[], &key_props(Some("dispose")));
    let want_key = [
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
        0xff,
    ];
    let deadline = Instant::now() + TO;
    let mut seen = Vec::new();
    while seen.len() < 2 {
        let left = deadline
            .checked_duration_since(Instant::now())
            .expect("the sample and the dispose over RTPS");
        match rx.recv_timeout(left) {
            Ok(UserSample::Alive { key_hash, .. }) => seen.push(("alive", key_hash)),
            Ok(UserSample::Lifecycle { key_hash, kind, .. }) => {
                assert_eq!(kind, ChangeKind::NotAliveDisposed);
                seen.push(("disposed", Some(key_hash)));
            }
            Ok(_) => {}
            Err(e) => panic!("{e}"),
        }
    }
    assert_eq!(
        seen,
        [("alive", Some(want_key)), ("disposed", Some(want_key))]
    );
    assert!(h.metrics.as_ref().unwrap().errors_total.get() > errors);
    shutdown(h);
    dds.shutdown();
}

/// Spec §4.4 with §1 L1: under MQTT 3.1.1 a PUBLISH carries no properties,
/// so a dispose or an unregister cannot carry `zerodds_op`; it is not
/// published (a receiver would take the header and the key holder for a
/// sample) and each is counted as an error. The samples before and after
/// it go out in their order.
#[test]
fn mqttbridge_4_4_under_3_1_1_a_dispose_is_not_published_as_a_sample() {
    use zerodds_rtps::inline_qos::status_info::{DISPOSED, UNREGISTERED};
    let b = broker_server(ServerConfig::default());
    let (mut cfg, lease) = config(&format!("mqtt://{}", b.local_addr()), "v311-life");
    cfg.protocol_version = ProtocolVersion::V311;
    cfg.topics.push(topic("Life311", "life311/out", "out"));
    let h = server::start(cfg).expect("daemon start");
    let mut sub =
        TestClient::connect(b.local_addr(), "v311-life-sub", ProtocolVersion::V5).unwrap();
    sub.subscribe("life311/out", 1).unwrap();
    let dds = foreign_participant(lease.id(), 0x45);
    let w = dds
        .register_user_writer(UserWriterConfig {
            topic_name: "Life311".into(),
            type_name: "Life311".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    wait_until("the bridge's reader over RTPS", || {
        !dds.user_writer_matched_reader_guids(w).is_empty()
    });
    let errors = h.metrics.as_ref().unwrap().errors_total.get();
    let key = [0x5b; 16];
    dds.write_user_sample_keyed(w, &[1], key, None).unwrap();
    dds.write_user_lifecycle(w, key, None, DISPOSED, None)
        .unwrap();
    dds.write_user_lifecycle(w, key, None, UNREGISTERED, None)
        .unwrap();
    dds.write_user_sample_keyed(w, &[2], key, None).unwrap();
    let mut got = Vec::new();
    for _ in 0..2 {
        let (t, payload) = sub.recv_publish(TO).unwrap();
        assert_eq!(t, "life311/out");
        got.push(payload[4..].to_vec());
    }
    assert_eq!(got, [vec![1], vec![2]], "the two samples, in their order");
    assert!(
        sub.recv_publish(Duration::from_millis(700)).is_err(),
        "a lifecycle change went out under MQTT 3.1.1"
    );
    assert_eq!(
        h.metrics.as_ref().unwrap().errors_total.get() - errors,
        2,
        "the dispose and the unregister"
    );
    shutdown(h);
    dds.shutdown();
    b.shutdown();
}

// ---------------------------------------------------------------------
// §5.2: the catalog
// ---------------------------------------------------------------------

/// Spec §5.2: after the CONNACK the daemon publishes its catalog retained
/// on `$zerodds/<client_id>/catalog`; a subscriber that comes later gets
/// it, the JSON of the spec with the configured `schema_url`. At the
/// shutdown the retained message is cleared: the subscriber gets the empty
/// PUBLISH, a later one nothing.
#[test]
fn mqttbridge_5_2_the_catalog_is_retained_and_cleared_at_the_shutdown() {
    let b = broker_server(ServerConfig::default());
    let (mut cfg, _lease) = config(&format!("mqtt://{}", b.local_addr()), "cat-1");
    let mut t = topic("Chat::Message", "chat/message", "bidir");
    t.schema_url = Some("https://schema.example/chat-message.idl".into());
    cfg.topics.push(t);
    let h = server::start(cfg).expect("daemon start");
    let mut late = TestClient::connect(b.local_addr(), "cat-late", ProtocolVersion::V5).unwrap();
    late.subscribe("$zerodds/+/catalog", 1).unwrap();
    let (topic_name, payload) = late.recv_publish(TO).unwrap();
    assert_eq!(topic_name, "$zerodds/cat-1/catalog");
    assert_eq!(
        String::from_utf8(payload).unwrap(),
        concat!(
            r#"{"topics":[{"dds_name":"Chat::Message","dds_type":"Chat::Message","#,
            r#""mqtt_topic":"chat/message","qos":{"reliability":"reliable","durability":"volatile"},"#,
            r#""schema_url":"https://schema.example/chat-message.idl"}]}"#
        )
    );
    shutdown(h);
    let (topic_name, payload) = late.recv_publish(TO).unwrap();
    assert_eq!(
        (topic_name.as_str(), payload.len()),
        ("$zerodds/cat-1/catalog", 0),
        "the clear at the shutdown"
    );
    let mut after = TestClient::connect(b.local_addr(), "cat-after", ProtocolVersion::V5).unwrap();
    after.subscribe("$zerodds/+/catalog", 1).unwrap();
    assert!(
        after.recv_publish(Duration::from_millis(700)).is_err(),
        "a stopped bridge stays in the catalog"
    );
    b.shutdown();
}

/// The topics on `filter` a new subscriber gets messages on within 700 ms
/// of the last one, at most 3 s (a report the daemon repeats does not hold
/// it longer); each topic once, sorted. Right after the SUBSCRIBE these are
/// the retained messages.
fn retained_on(b: &ServerHandle, client_id: &str, filter: &str) -> Vec<String> {
    let mut c = TestClient::connect(b.local_addr(), client_id, ProtocolVersion::V5).unwrap();
    c.subscribe(filter, 1).unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    let mut out = Vec::new();
    while Instant::now() < until {
        match c.recv_publish(Duration::from_millis(700)) {
            Ok((t, _)) => out.push(t),
            Err(_) => break,
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Spec §5.2 with §1 L6: the connection of a tenant publishes the catalog
/// of the tenant's topics on `$zerodds/<its client_id>/catalog`, the top
/// level's connection the catalog of its topics; at the shutdown both are
/// cleared.
#[test]
fn mqttbridge_5_2_a_tenant_publishes_the_catalog_of_its_topics_and_clears_it() {
    let b = broker_server(ServerConfig::default());
    let (mut cfg, _lease) = config(&format!("mqtt://{}", b.local_addr()), "cat-top");
    let tenant_lease = zerodds_test_domains::lease_domain();
    cfg.topics.push(topic("Top", "top/out", "out"));
    cfg.tenants.push(TenantConfig {
        name: "a".into(),
        domain: tenant_lease.id(),
        client_id: "cat-a".into(),
        username: None,
        password: None,
        topics: vec![
            topic("Plant", "a/plant", "in"),
            topic("Status", "a/status", "out"),
        ],
        acl_rules: Vec::new(),
        acl_default_deny: None,
    });
    let h = server::start(cfg).expect("daemon start");
    let mut watcher =
        TestClient::connect(b.local_addr(), "cat-watch", ProtocolVersion::V5).unwrap();
    watcher.subscribe("$zerodds/+/catalog", 1).unwrap();
    let mut got = Vec::new();
    for _ in 0..2 {
        let (t, p) = watcher.recv_publish(TO).unwrap();
        got.push((t, String::from_utf8(p).unwrap()));
    }
    got.sort();
    let entry = |dds: &str, mqtt: &str| {
        format!(
            r#"{{"dds_name":"{dds}","dds_type":"{dds}","mqtt_topic":"{mqtt}","qos":{{"reliability":"reliable","durability":"volatile"}}}}"#
        )
    };
    assert_eq!(
        got,
        [
            (
                "$zerodds/cat-a/catalog".to_string(),
                format!(
                    r#"{{"topics":[{},{}]}}"#,
                    entry("Plant", "a/plant"),
                    entry("Status", "a/status")
                )
            ),
            (
                "$zerodds/cat-top/catalog".to_string(),
                format!(r#"{{"topics":[{}]}}"#, entry("Top", "top/out"))
            ),
        ]
    );
    shutdown(h);
    let mut cleared = Vec::new();
    for _ in 0..2 {
        let (t, p) = watcher.recv_publish(TO).unwrap();
        assert!(p.is_empty(), "the clear of {t}");
        cleared.push(t);
    }
    cleared.sort();
    assert_eq!(
        cleared,
        ["$zerodds/cat-a/catalog", "$zerodds/cat-top/catalog"]
    );
    assert!(
        retained_on(&b, "cat-after", "$zerodds/+/catalog").is_empty(),
        "a stopped bridge stays in the catalog"
    );
    b.shutdown();
}

/// Spec §5.2, §6 with §9.3: with an empty `mqtt.client_id` the broker
/// assigns an identifier at each connect. After a reconnect under a new
/// one, the catalog of the identifier before is cleared, so the bridge is
/// listed once, under the new one; a missed deadline is reported under the
/// new one.
#[test]
fn mqttbridge_5_2_a_new_assigned_identifier_clears_the_catalog_of_the_one_before() {
    let b = broker_server(ServerConfig::default());
    let (mut cfg, _lease) = config(&format!("mqtt://{}", b.local_addr()), "");
    cfg.reconnect.initial_delay_ms = 100;
    cfg.reconnect.max_delay_ms = 300;
    let mut t = topic("Assigned", "assigned/out", "out");
    t.deadline_ms = Some(300);
    cfg.topics.push(t);
    let h = server::start(cfg).expect("daemon start");
    wait_until("the daemon's connection", || {
        b.connected_clients().len() == 1
    });
    let first = b.connected_clients()[0].clone();
    assert_eq!(
        retained_on(&b, "assigned-watch-1", "$zerodds/+/catalog"),
        [format!("$zerodds/{first}/catalog")]
    );
    // §4.11: every connection is closed; the daemon connects again.
    b.redirect_clients(&zerodds_mqtt_bridge::server::Redirect {
        permanent: false,
        server_reference: "elsewhere".into(),
    });
    let mut second = String::new();
    wait_until("the daemon's next connection", || {
        let c = b.connected_clients();
        match c.as_slice() {
            [id] if *id != first => {
                second.clone_from(id);
                true
            }
            _ => false,
        }
    });
    wait_until("the catalog under the new identifier", || {
        retained_on(&b, "assigned-watch-2", "$zerodds/+/catalog")
            .contains(&format!("$zerodds/{second}/catalog"))
    });
    assert_eq!(
        retained_on(&b, "assigned-watch-3", "$zerodds/+/catalog"),
        [format!("$zerodds/{second}/catalog")],
        "listed once"
    );
    let w = h
        .runtime
        .register_user_writer(UserWriterConfig {
            topic_name: "Assigned".into(),
            type_name: "Assigned".into(),
            reliable: true,
            deadline: zerodds_qos::DeadlineQosPolicy {
                period: zerodds_qos::Duration::from_millis(100),
            },
            ..Default::default()
        })
        .unwrap();
    wait_until("the bridge's reader matched", || {
        !h.runtime.user_writer_matched_reader_guids(w).is_empty()
    });
    h.runtime
        .write_user_sample_keyed(w, &[1], [0x77; 16], None)
        .unwrap();
    let report = format!("$zerodds/{second}/deadline_missed/assigned");
    wait_until("the deadline report under the new identifier", || {
        retained_on(&b, "assigned-watch-4", "$zerodds/+/deadline_missed/#").contains(&report)
    });
    assert_eq!(
        retained_on(&b, "assigned-watch-5", "$zerodds/+/deadline_missed/#"),
        [report],
        "a report under the identifier before"
    );
    shutdown(h);
    b.shutdown();
}

// ---------------------------------------------------------------------
// §6: the history of an out topic
// ---------------------------------------------------------------------

/// Spec §6 KEEP_LAST(N), §3 `qos.history` on an `out` topic: while the
/// broker's Receive Maximum (1) is reached, the samples wait in the daemon;
/// of those that arrive meanwhile the newest 2 of the instance are
/// published, the older one is dropped and counted.
#[test]
fn mqttbridge_6_keep_last_on_an_out_topic_publishes_the_newest_n() {
    let rec = Recorder::start(PropertySet {
        receive_maximum: Some(1),
        ..PropertySet::default()
    });
    let (mut cfg, _lease) = config(&rec.url(), "keep-last-out");
    let mut t = topic("KeepLast", "hist/last", "out");
    t.history_kind = "keep_last".into();
    t.history_depth = 2;
    cfg.topics.push(t);
    let h = server::start(cfg).expect("daemon start");
    let eid = h.user_readers["KeepLast"];
    rec.hold_acks();
    assert!(h.runtime.test_inject_user_alive(eid, vec![1]));
    wait_until("the first PUBLISH", || rec.markers("hist/last") == [1]);
    // The pump takes 2 and waits for a slot; 3, 4 and 5 wait behind it.
    assert!(h.runtime.test_inject_user_alive(eid, vec![2]));
    thread::sleep(Duration::from_millis(300));
    for v in [3u8, 4, 5] {
        assert!(h.runtime.test_inject_user_alive(eid, vec![v]));
    }
    thread::sleep(Duration::from_millis(300));
    rec.release_acks();
    wait_until("four PUBLISH packets", || {
        rec.markers("hist/last").len() == 4
    });
    thread::sleep(Duration::from_millis(300));
    assert_eq!(rec.markers("hist/last"), [1, 2, 4, 5]);
    let m = h.metrics.as_ref().unwrap();
    assert_eq!(
        m.total(zerodds_mqtt_bridge::daemon::metrics::family::DDS_DROPPED),
        1
    );
    shutdown(h);
}

/// Spec §3, §9.2: a `mqtt.drain_timeout_secs` beyond what the clock holds
/// (the configuration takes every u64) waits for the broker's acks without
/// end: the shutdown ends after the PUBACK held back; it used to panic.
#[test]
fn mqttbridge_9_2_a_drain_timeout_beyond_the_clock_waits_for_the_acks() {
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "drain-max");
    cfg.drain_timeout_secs = u64::MAX;
    let mut t = topic("DrainMax", "drain/max", "out");
    t.mqtt_qos = Some(1);
    cfg.topics.push(t);
    let mut h = server::start(cfg).expect("daemon start");
    rec.hold_acks();
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["DrainMax"], vec![1])
    );
    wait_until("the PUBLISH", || rec.markers("drain/max") == [1]);
    let took = thread::scope(|s| {
        s.spawn(|| {
            thread::sleep(Duration::from_millis(1500));
            rec.release_acks();
        });
        let t0 = Instant::now();
        h.shutdown();
        t0.elapsed()
    });
    assert!(took >= Duration::from_millis(1400), "{took:?}");
    assert!(took < TO, "{took:?}");
}

/// Spec §6 KEEP_ALL: no sample is dropped, also when the broker
/// acknowledges nothing for longer than a PUBLISH waits for a slot (10 s);
/// the samples go out in their order once it acknowledges again. A sample
/// used to be dropped and counted as an error after those 10 s.
#[test]
fn mqttbridge_6_keep_all_on_an_out_topic_drops_nothing() {
    let rec = Recorder::start(PropertySet {
        receive_maximum: Some(1),
        ..PropertySet::default()
    });
    let (mut cfg, _lease) = config(&rec.url(), "keep-all-out");
    let mut t = topic("KeepAll", "hist/all", "out");
    t.history_kind = "keep_all".into();
    cfg.topics.push(t);
    let h = server::start(cfg).expect("daemon start");
    let eid = h.user_readers["KeepAll"];
    rec.hold_acks();
    assert!(h.runtime.test_inject_user_alive(eid, vec![1]));
    wait_until("the first PUBLISH", || rec.markers("hist/all") == [1]);
    for v in [2u8, 3, 4] {
        assert!(h.runtime.test_inject_user_alive(eid, vec![v]));
    }
    thread::sleep(
        zerodds_mqtt_bridge::daemon::client::SEND_SLOT_WAIT + Duration::from_millis(1500),
    );
    assert_eq!(rec.markers("hist/all"), [1], "nothing without an ack");
    rec.release_acks();
    wait_until("four PUBLISH packets", || {
        rec.markers("hist/all").len() == 4
    });
    assert_eq!(rec.markers("hist/all"), [1, 2, 3, 4]);
    let m = h.metrics.as_ref().unwrap();
    assert_eq!(
        m.total(zerodds_mqtt_bridge::daemon::metrics::family::DDS_DROPPED),
        0
    );
    assert_eq!(m.errors_total.get(), 0);
    shutdown(h);
}

/// Spec §6 QoS derivation: a value that is set is kept against the
/// derivation — `mqtt_qos: 0` on a reliable topic stays 0, `retain: false`
/// on a transient-local topic stays false; without them a reliable
/// transient-local topic publishes QoS 1 retained.
#[test]
fn mqttbridge_6_an_explicit_mqtt_qos_0_stays_0_on_a_reliable_topic() {
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "derive-cli");
    let mut set = topic("Set", "derive/set", "out");
    set.mqtt_qos = Some(0);
    set.retain = Some(false);
    set.durability = "transient_local".into();
    let mut derived = topic("Derived", "derive/derived", "out");
    derived.durability = "transient_local".into();
    cfg.topics.push(set);
    cfg.topics.push(derived);
    let h = server::start(cfg).expect("daemon start");
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["Set"], vec![1])
    );
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["Derived"], vec![2])
    );
    wait_until("two PUBLISH packets", || {
        rec.publishes.lock().unwrap().len() == 2
    });
    let got = rec.publishes.lock().unwrap().clone();
    let of = |t: &str| got.iter().find(|p| p.topic == t).unwrap().clone();
    let (s, d) = (of("derive/set"), of("derive/derived"));
    assert_eq!((s.qos, s.retain), (0, false));
    assert_eq!((d.qos, d.retain), (1, true));
    shutdown(h);
}

/// Spec §6 Lifespan: on an `out` topic `qos.lifespan_ms` is the Message
/// Expiry Interval of a sample whose writer names no lifespan; on an `in`
/// topic it is the LIFESPAN of the DataWriter, which the samples written
/// from MQTT carry.
#[test]
fn mqttbridge_6_lifespan_is_the_message_expiry_interval() {
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "lifespan-cli");
    let mut out = topic("LsOut", "ls/out", "out");
    out.lifespan_ms = Some(3_000);
    let mut inn = topic("LsIn", "ls/in", "in");
    inn.lifespan_ms = Some(1_500);
    cfg.topics.push(out);
    cfg.topics.push(inn);
    let h = server::start(cfg).expect("daemon start");
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["LsOut"], vec![1])
    );
    wait_until("the PUBLISH", || !rec.publishes.lock().unwrap().is_empty());
    let p = rec.publishes.lock().unwrap()[0].clone();
    let props = PropertySet::parse(
        &p.properties,
        PropertyContext::Packet(ControlPacketType::Publish),
    )
    .unwrap();
    assert_eq!(props.message_expiry_interval, Some(3));
    let rx = reader_on(&h.runtime, "LsIn");
    rec.inject(
        "ls/in",
        &[0x00, 0x01, 0x00, 0x00, 4],
        &PropertySet::default(),
    );
    let deadline = Instant::now() + TO;
    let lifespan = loop {
        let left = deadline
            .checked_duration_since(Instant::now())
            .expect("the sample");
        if let Ok(UserSample::Alive { lifespan_nanos, .. }) = rx.recv_timeout(left) {
            break lifespan_nanos;
        }
    };
    assert_eq!(lifespan, 1_500_000_000);
    shutdown(h);
}

/// Spec §6 Lifespan with the history: a sample that waits in the daemon
/// (here 2.2 s for the broker's acknowledgement, its Receive Maximum 1)
/// goes out with the Message Expiry Interval reduced by the time since it
/// arrived (4 s lifespan: 2); one whose lifespan passed meanwhile (1.5 s)
/// is not published and is counted in `dds_samples_dropped_total`.
#[test]
fn mqttbridge_6_a_sample_that_waited_goes_out_with_the_time_left_or_not_at_all() {
    let rec = Recorder::start(PropertySet {
        receive_maximum: Some(1),
        ..PropertySet::default()
    });
    let (mut cfg, _lease) = config(&rec.url(), "lifespan-wait");
    for (name, mqtt, ms) in [("Short", "ls/short", 1_500), ("Long", "ls/long", 4_000)] {
        let mut t = topic(name, mqtt, "out");
        t.history_kind = "keep_all".into();
        t.lifespan_ms = Some(ms);
        cfg.topics.push(t);
    }
    let h = server::start(cfg).expect("daemon start");
    rec.hold_acks();
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["Short"], vec![1])
    );
    wait_until("the first PUBLISH", || rec.markers("ls/short") == [1]);
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["Short"], vec![2])
    );
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["Long"], vec![3])
    );
    thread::sleep(Duration::from_millis(2_200));
    assert_eq!(
        rec.publishes.lock().unwrap().len(),
        1,
        "nothing without an ack"
    );
    rec.release_acks();
    wait_until("the PUBLISH of the long one", || {
        rec.markers("ls/long") == [3]
    });
    thread::sleep(Duration::from_millis(300));
    assert_eq!(rec.markers("ls/short"), [1], "the expired sample went out");
    let p = rec
        .publishes
        .lock()
        .unwrap()
        .iter()
        .find(|p| p.topic == "ls/long")
        .cloned()
        .unwrap();
    let props = PropertySet::parse(
        &p.properties,
        PropertyContext::Packet(ControlPacketType::Publish),
    )
    .unwrap();
    assert_eq!(props.message_expiry_interval, Some(2), "4 s less 2.2 s");
    let m = h.metrics.as_ref().unwrap();
    assert_eq!(
        m.total(zerodds_mqtt_bridge::daemon::metrics::family::DDS_DROPPED),
        1
    );
    shutdown(h);
}

/// Spec §6 Partition: the DataReader of a topic with `qos.partition` takes
/// only the samples of writers in its partitions; a sample of another
/// partition is not published.
#[test]
fn mqttbridge_6_a_sample_of_another_partition_is_not_published() {
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "partition-cli");
    let mut t = topic("Part", "part/out", "out");
    t.partition = vec!["plant-a".into()];
    cfg.topics.push(t);
    let h = server::start(cfg).expect("daemon start");
    let writer_in = |partition: &str| {
        h.runtime
            .register_user_writer(UserWriterConfig {
                topic_name: "Part".into(),
                type_name: "Part".into(),
                reliable: true,
                partition: vec![partition.into()],
                ..Default::default()
            })
            .unwrap()
    };
    let other = writer_in("plant-b");
    let same = writer_in("plant-a");
    wait_until("the writer of plant-a matched", || {
        !h.runtime.user_writer_matched_reader_guids(same).is_empty()
    });
    assert!(h.runtime.user_writer_matched_reader_guids(other).is_empty());
    h.runtime.write_user_sample(other, vec![0xB]).unwrap();
    h.runtime.write_user_sample(same, vec![0xA]).unwrap();
    wait_until("the PUBLISH", || rec.markers("part/out") == [0xA]);
    thread::sleep(Duration::from_millis(400));
    assert_eq!(rec.markers("part/out"), [0xA]);
    shutdown(h);
}

/// Spec §6 Deadline: a DDS writer that misses the DEADLINE the topic's
/// DataReader requests, and MQTT publishers that miss the one its DataWriter
/// offers, are each a retained PUBLISH on
/// `$zerodds/<client_id>/deadline_missed/<slug>` naming the endpoint, the
/// count and the instance.
#[test]
fn mqttbridge_6_a_missed_deadline_is_a_retained_publish() {
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "deadline-cli");
    let mut out = topic("Dl::Out", "dl/out", "out");
    out.deadline_ms = Some(300);
    let mut inn = topic("Dl::In", "dl/in", "in");
    inn.deadline_ms = Some(300);
    cfg.topics.push(out);
    cfg.topics.push(inn);
    let h = server::start(cfg).expect("daemon start");
    let w = h
        .runtime
        .register_user_writer(UserWriterConfig {
            topic_name: "Dl::Out".into(),
            type_name: "Dl::Out".into(),
            reliable: true,
            deadline: zerodds_qos::DeadlineQosPolicy {
                period: zerodds_qos::Duration::from_millis(100),
            },
            ..Default::default()
        })
        .unwrap();
    wait_until("the bridge's reader matched", || {
        !h.runtime.user_writer_matched_reader_guids(w).is_empty()
    });
    h.runtime
        .write_user_sample_keyed(w, &[1], [0x33; 16], None)
        .unwrap();
    rec.inject(
        "dl/in",
        &[0x00, 0x01, 0x00, 0x00, 2],
        &PropertySet {
            user_properties: vec![("zerodds_key_hash".into(), "44".repeat(16))],
            ..PropertySet::default()
        },
    );
    let report = |topic: &str| {
        rec.dollar
            .lock()
            .unwrap()
            .iter()
            .find(|p| p.topic == topic)
            .cloned()
    };
    wait_until("the reader's report", || {
        report("$zerodds/deadline-cli/deadline_missed/dl/out").is_some()
    });
    wait_until("the writer's report", || {
        report("$zerodds/deadline-cli/deadline_missed/dl/in").is_some()
    });
    let r = report("$zerodds/deadline-cli/deadline_missed/dl/out").unwrap();
    assert!(r.retain);
    let text = String::from_utf8(r.payload).unwrap();
    assert!(
        text.starts_with(r#"{"dds_topic":"Dl::Out","endpoint":"reader","total_count":"#),
        "{text}"
    );
    assert!(
        text.ends_with(&format!(r#""key_hash":"{}"}}"#, "33".repeat(16))),
        "{text}"
    );
    let w_report = String::from_utf8(
        report("$zerodds/deadline-cli/deadline_missed/dl/in")
            .unwrap()
            .payload,
    )
    .unwrap();
    assert!(
        w_report.starts_with(r#"{"dds_topic":"Dl::In","endpoint":"writer","total_count":"#),
        "{w_report}"
    );
    assert!(
        w_report.ends_with(&format!(r#""key_hash":"{}"}}"#, "44".repeat(16))),
        "{w_report}"
    );
    shutdown(h);
}

/// Spec §6 Liveliness: a DDS writer of another participant that loses its
/// liveliness (MANUAL_BY_TOPIC, 300 ms lease, one write) is a PUBLISH on
/// `$zerodds/<client_id>/liveliness_lost/<slug>` with the counts of
/// LIVELINESS_CHANGED.
#[test]
fn mqttbridge_6_a_lost_writer_liveliness_is_published() {
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, lease) = config(&rec.url(), "liveliness-cli");
    let mut t = topic("Live", "live/out", "out");
    t.liveliness_lease_ms = Some(1_000);
    cfg.topics.push(t);
    let h = server::start(cfg).expect("daemon start");
    let dds = foreign_participant(lease.id(), 0x66);
    let w = dds
        .register_user_writer(UserWriterConfig {
            topic_name: "Live".into(),
            type_name: "Live".into(),
            reliable: true,
            liveliness: zerodds_qos::LivelinessQosPolicy {
                kind: zerodds_qos::LivelinessKind::ManualByTopic,
                lease_duration: zerodds_qos::Duration::from_millis(300),
            },
            ..Default::default()
        })
        .unwrap();
    wait_until("the bridge's reader over RTPS", || {
        !dds.user_writer_matched_reader_guids(w).is_empty()
    });
    dds.write_user_sample(w, vec![1]).unwrap();
    wait_until("the liveliness report", || {
        rec.dollar
            .lock()
            .unwrap()
            .iter()
            .any(|p| p.topic == "$zerodds/liveliness-cli/liveliness_lost/live")
    });
    let p = rec
        .dollar
        .lock()
        .unwrap()
        .iter()
        .find(|p| p.topic == "$zerodds/liveliness-cli/liveliness_lost/live")
        .cloned()
        .unwrap();
    assert!(!p.retain);
    assert_eq!(
        String::from_utf8(p.payload).unwrap(),
        r#"{"dds_topic":"Live","alive_count":0,"not_alive_count":1}"#
    );
    shutdown(h);
    dds.shutdown();
}

/// Spec §6 Liveliness: the CONNECT carries a will on
/// `$zerodds/<client_id>/liveliness_lost`; when the daemon process is
/// killed (SIGKILL, no DISCONNECT) the broker publishes it to a subscriber.
#[test]
fn mqttbridge_6_the_broker_publishes_the_will_when_the_daemon_is_killed() {
    let dir = scratch("will");
    let lease = zerodds_test_domains::lease_domain();
    let b = broker_server(ServerConfig::default());
    let mut watcher =
        TestClient::connect(b.local_addr(), "will-watcher", ProtocolVersion::V5).unwrap();
    watcher.subscribe("$zerodds/+/liveliness_lost", 0).unwrap();
    let url = format!("mqtt://{}", b.local_addr());
    let mut daemon = Daemon(
        Command::new(BIN)
            .args(args(
                &dir,
                lease.id(),
                &["--broker", &url, "--client-id", "will-cli"],
            ))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_until("the daemon's connection", || {
        b.connected_clients().contains(&"will-cli".to_string())
    });
    assert!(
        watcher.recv_publish(Duration::from_millis(300)).is_err(),
        "a will while the daemon runs"
    );
    daemon.0.kill().unwrap();
    let _ = daemon.0.wait();
    let (t, payload) = watcher.recv_publish(TO).unwrap();
    assert_eq!(t, "$zerodds/will-cli/liveliness_lost");
    assert_eq!(payload, br#"{"client_id":"will-cli"}"#);
    b.shutdown();
}

/// The crate's broker server on `port` of the loopback (a restarted
/// broker on the port of the one before).
fn broker_on(port: u16) -> ServerHandle {
    let deadline = Instant::now() + TO;
    loop {
        match MqttBrokerServer::bind_with(("127.0.0.1", port), ServerConfig::default()) {
            Ok(b) => return b.spawn().unwrap(),
            Err(e) => {
                assert!(Instant::now() < deadline, "bind {port}: {e}");
                thread::sleep(Duration::from_millis(50));
            }
        }
    }
}

/// Spec §6 TRANSIENT, §9.3: the broker restarts without its retained
/// messages; the daemon connects again and publishes its replay buffer, so
/// a subscriber of the new broker gets the newest sample retained.
#[test]
fn mqttbridge_6_transient_samples_are_replayed_after_the_broker_restarts() {
    let b1 = broker_server(ServerConfig::default());
    let port = b1.local_addr().port();
    let (mut cfg, _lease) = config(&format!("mqtt://127.0.0.1:{port}"), "transient-cli");
    cfg.reconnect.initial_delay_ms = 100;
    cfg.reconnect.max_delay_ms = 300;
    let mut t = topic("Tr", "tr/out", "out");
    t.durability = "transient".into();
    cfg.topics.push(t);
    let h = server::start(cfg).expect("daemon start");
    let mut first = TestClient::connect(b1.local_addr(), "tr-sub-1", ProtocolVersion::V5).unwrap();
    first.subscribe("tr/out", 1).unwrap();
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["Tr"], vec![7])
    );
    let (_, p) = first.recv_publish(TO).unwrap();
    assert_eq!(p[4..], [7]);
    b1.shutdown();
    let b2 = broker_on(port);
    let mut late = TestClient::connect(b2.local_addr(), "tr-sub-2", ProtocolVersion::V5);
    let deadline = Instant::now() + TO;
    let got = loop {
        if let Ok(c) = late.as_mut() {
            c.subscribe("tr/out", 1).unwrap();
            if let Ok((_, p)) = c.recv_publish(Duration::from_millis(500)) {
                break p;
            }
        }
        assert!(
            Instant::now() < deadline,
            "no replay on the restarted broker"
        );
        late = TestClient::connect(b2.local_addr(), "tr-sub-2", ProtocolVersion::V5);
    };
    assert_eq!(got[4..], [7]);
    shutdown(h);
    b2.shutdown();
}

/// An Authenticator that refuses the client `id` with 0x88 Server
/// unavailable until `open` is set, and lets every other client in.
struct Gate {
    id: String,
    open: AtomicBool,
}

impl Authenticator for Gate {
    fn authenticate(
        &self,
        client_id: &str,
        _user_name: Option<&str>,
        _password: Option<&[u8]>,
    ) -> Result<(), u8> {
        if client_id == self.id && !self.open.load(Ordering::SeqCst) {
            Err(0x88)
        } else {
            Ok(())
        }
    }

    fn start(
        &self,
        _client_id: &str,
        _method: &str,
    ) -> Option<Box<dyn zerodds_mqtt_bridge::server::AuthExchange>> {
        None
    }
}

/// Spec §6 TRANSIENT with §9.3: the samples that arrive while the broker
/// is away wait in the history; after the reconnect the replay buffer
/// (the sample published before the outage) goes out, then the waiting
/// samples, each once and in their order. The replay buffer used to take
/// a sample when it arrived, so the newest waiting one went out twice and
/// before the older ones.
#[test]
fn mqttbridge_6_the_samples_of_an_outage_arrive_once_and_in_order() {
    let b1 = broker_server(ServerConfig::default());
    let port = b1.local_addr().port();
    let (mut cfg, _lease) = config(&format!("mqtt://127.0.0.1:{port}"), "outage-cli");
    cfg.reconnect.initial_delay_ms = 100;
    cfg.reconnect.max_delay_ms = 300;
    let mut t = topic("Outage", "outage/out", "out");
    t.durability = "transient".into();
    t.history_kind = "keep_all".into();
    cfg.topics.push(t);
    let h = server::start(cfg).expect("daemon start");
    let eid = h.user_readers["Outage"];
    let mut first =
        TestClient::connect(b1.local_addr(), "outage-sub-1", ProtocolVersion::V5).unwrap();
    first.subscribe("outage/out", 1).unwrap();
    assert!(h.runtime.test_inject_user_alive(eid, vec![0]));
    let (_, p) = first.recv_publish(TO).unwrap();
    assert_eq!(p[4..], [0]);
    b1.shutdown();
    wait_until("the lost connection", || !h.healthy.load(Ordering::SeqCst));
    for v in [1u8, 2, 3] {
        assert!(h.runtime.test_inject_user_alive(eid, vec![v]));
    }
    let gate = Arc::new(Gate {
        id: "outage-cli".into(),
        open: AtomicBool::new(false),
    });
    let deadline = Instant::now() + TO;
    let b2 = loop {
        let config = ServerConfig {
            authenticator: Some(Arc::clone(&gate) as Arc<dyn Authenticator>),
            ..ServerConfig::default()
        };
        match MqttBrokerServer::bind_with(("127.0.0.1", port), config) {
            Ok(b) => break b.spawn().unwrap(),
            Err(e) => {
                assert!(Instant::now() < deadline, "bind {port}: {e}");
                thread::sleep(Duration::from_millis(50));
            }
        }
    };
    let mut second =
        TestClient::connect(b2.local_addr(), "outage-sub-2", ProtocolVersion::V5).unwrap();
    second.subscribe("outage/out", 1).unwrap();
    gate.open.store(true, Ordering::SeqCst);
    let mut got = Vec::new();
    for _ in 0..4 {
        let (_, p) = second.recv_publish(TO).unwrap();
        got.push(p[4]);
    }
    assert_eq!(got, [0, 1, 2, 3]);
    assert!(
        second.recv_publish(Duration::from_millis(700)).is_err(),
        "a sample went out twice"
    );
    shutdown(h);
    b2.shutdown();
}

/// Spec §6 PERSISTENT: the replay buffer outlives the daemon in
/// `mqtt.session_dir`; a restarted daemon towards a broker that holds
/// nothing publishes the newest sample again, retained.
#[test]
fn mqttbridge_6_persistent_samples_are_replayed_after_a_daemon_restart() {
    let b1 = broker_server(ServerConfig::default());
    let (mut cfg, _lease) = config(&format!("mqtt://{}", b1.local_addr()), "persistent-cli");
    let mut t = topic("Ps", "ps/out", "out");
    t.durability = "persistent".into();
    cfg.topics.push(t);
    let h = server::start(cfg.clone()).expect("first start");
    let mut sub = TestClient::connect(b1.local_addr(), "ps-sub-1", ProtocolVersion::V5).unwrap();
    sub.subscribe("ps/out", 1).unwrap();
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["Ps"], vec![8])
    );
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["Ps"], vec![9])
    );
    for want in [8u8, 9] {
        let (_, p) = sub.recv_publish(TO).unwrap();
        assert_eq!(p[4..], [want]);
    }
    shutdown(h);
    b1.shutdown();
    let file = zerodds_mqtt_bridge::daemon::replay::replay_file(&cfg);
    assert!(file.exists(), "{}", file.display());
    let b2 = broker_server(ServerConfig::default());
    cfg.broker_url = format!("mqtt://{}", b2.local_addr());
    let h2 = server::start(cfg).expect("second start");
    let mut late = TestClient::connect(b2.local_addr(), "ps-sub-2", ProtocolVersion::V5).unwrap();
    late.subscribe("ps/out", 1).unwrap();
    let (_, p) = late.recv_publish(TO).unwrap();
    assert_eq!(p[4..], [9], "the newest sample of the instance");
    assert!(late.recv_publish(Duration::from_millis(500)).is_err());
    shutdown(h2);
    b2.shutdown();
}

/// The DDS topic and the end offset of each record of a replay file, read
/// after the format of Spec §6.
fn replay_records(bytes: &[u8]) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let mut at = 8;
    while at < bytes.len() {
        let kind = bytes[at];
        let n = usize::from(u16::from_be_bytes([bytes[at + 1], bytes[at + 2]]));
        let name = String::from_utf8(bytes[at + 3..at + 3 + n].to_vec()).unwrap();
        at += 3 + n;
        at += if bytes[at] == 1 { 17 } else { 1 };
        if kind == b'S' {
            at += 8;
            for _ in 0..2 {
                let len = u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap());
                at += 4 + len as usize;
            }
        }
        out.push((name, at));
    }
    out
}

/// Spec §6 PERSISTENT: a replay file that ends inside its last record (the
/// daemon ended while writing it) does not prevent the start; the
/// restarted daemon publishes the sample of the record before it, not the
/// cut-off one.
#[test]
fn mqttbridge_6_a_replay_file_cut_off_in_its_last_record_does_not_prevent_the_start() {
    let b1 = broker_server(ServerConfig::default());
    let (mut cfg, _lease) = config(&format!("mqtt://{}", b1.local_addr()), "cut-replay-cli");
    for (dds, mqtt) in [("Pca", "pc/a"), ("Pcb", "pc/b")] {
        let mut t = topic(dds, mqtt, "out");
        t.durability = "persistent".into();
        cfg.topics.push(t);
    }
    let h = server::start(cfg.clone()).expect("first start");
    let mut sub = TestClient::connect(b1.local_addr(), "pc-sub-1", ProtocolVersion::V5).unwrap();
    sub.subscribe("pc/#", 1).unwrap();
    for (dds, mqtt, value) in [("Pca", "pc/a", 1u8), ("Pcb", "pc/b", 2)] {
        assert!(
            h.runtime
                .test_inject_user_alive(h.user_readers[dds], vec![value])
        );
        let (t, p) = sub.recv_publish(TO).unwrap();
        assert_eq!((t.as_str(), &p[4..]), (mqtt, &[value][..]));
    }
    shutdown(h);
    b1.shutdown();
    let file = zerodds_mqtt_bridge::daemon::replay::replay_file(&cfg);
    let bytes = std::fs::read(&file).unwrap();
    let records = replay_records(&bytes);
    assert_eq!(records.len(), 2, "{records:?}");
    assert_eq!(records[1].1, bytes.len());
    let (kept, value) = if records[0].0 == "Pca" {
        ("pc/a", 1u8)
    } else {
        ("pc/b", 2)
    };
    std::fs::write(&file, &bytes[..bytes.len() - 2]).unwrap();
    let b2 = broker_server(ServerConfig::default());
    cfg.broker_url = format!("mqtt://{}", b2.local_addr());
    let h2 = server::start(cfg).expect("the start with the cut-off file");
    let mut late = TestClient::connect(b2.local_addr(), "pc-sub-2", ProtocolVersion::V5).unwrap();
    late.subscribe("pc/#", 1).unwrap();
    let (t, p) = late.recv_publish(TO).unwrap();
    assert_eq!((t.as_str(), &p[4..]), (kept, &[value][..]));
    assert!(
        late.recv_publish(Duration::from_millis(500)).is_err(),
        "the cut-off record is not replayed"
    );
    assert_eq!(
        std::fs::read(&file).unwrap(),
        bytes[..records[0].1],
        "the file written again without the cut-off record"
    );
    shutdown(h2);
    b2.shutdown();
}

// ---------------------------------------------------------------------
// §1 L1: MQTT 3.1.1 and the Keep Alive
// ---------------------------------------------------------------------

/// Spec §1 L1: in the compatibility mode the daemon connects as an MQTT
/// 3.1.1 client to the crate's broker server (the broker reads protocol
/// level 4 in its CONNECT) and carries messages both ways.
#[test]
fn mqttbridge_1_l1_the_daemon_speaks_mqtt_3_1_1() {
    let h = broker_server(ServerConfig::default());
    let (mut cfg, _lease) = config(&format!("mqtt://{}", h.local_addr()), "v311-cli");
    cfg.protocol_version = ProtocolVersion::V311;
    cfg.topics.push(topic("In311", "v311/in", "in"));
    cfg.topics.push(topic("Out311", "v311/out", "out"));
    let d = server::start(cfg).expect("daemon start");
    wait_until("the daemon's connection", || {
        h.protocol_version_of("v311-cli").is_some()
    });
    assert_eq!(
        h.protocol_version_of("v311-cli"),
        Some(ProtocolVersion::V311),
        "the protocol level of the daemon's CONNECT"
    );
    let rx = reader_on(&d.runtime, "In311");
    let mut sub = TestClient::connect(h.local_addr(), "v311-sub", ProtocolVersion::V5).unwrap();
    sub.subscribe("v311/out", 1).unwrap();
    let mut publisher =
        TestClient::connect(h.local_addr(), "v311-pub", ProtocolVersion::V5).unwrap();
    let deadline = Instant::now() + TO;
    let got = loop {
        publisher
            .publish("v311/in", &[0x00, 0x01, 0x00, 0x00, 4], 0, false)
            .unwrap();
        if let Some(s) = next_alive(&rx, Duration::from_millis(300)) {
            break s;
        }
        assert!(Instant::now() < deadline, "nothing reached DDS");
    };
    assert_eq!(got.0, [4]);
    assert!(
        d.runtime
            .test_inject_user_alive(d.user_readers["Out311"], vec![5, 6])
    );
    let (t, payload) = sub.recv_publish(TO).unwrap();
    assert_eq!(t, "v311/out");
    assert_eq!(payload, [0x00, 0x01, 0x00, 0x00, 5, 6]);
    shutdown(d);
    h.shutdown();
}

/// Spec §1 L1, MQTT 5.0 §3.1.2.10: the broker server closes a connection
/// that is silent for 1.5 times its Keep Alive; the idle daemon pings and
/// stays connected, a silent client of the same Keep Alive is dropped.
#[test]
fn mqttbridge_1_l1_an_idle_daemon_stays_connected_past_one_and_a_half_keep_alives() {
    let h = broker_server(ServerConfig::default());
    let (mut cfg, _lease) = config(&format!("mqtt://{}", h.local_addr()), "idle-daemon");
    cfg.keep_alive_secs = 1;
    let d = server::start(cfg).expect("daemon start");
    let _silent =
        TestClient::connect_with_keep_alive(h.local_addr(), "silent", ProtocolVersion::V5, 1)
            .unwrap();
    wait_until("both connected", || h.connected_clients().len() == 2);
    thread::sleep(Duration::from_millis(3500));
    let connected = h.connected_clients();
    assert!(
        connected.contains(&"idle-daemon".to_string()),
        "{connected:?}"
    );
    assert!(!connected.contains(&"silent".to_string()), "{connected:?}");
    shutdown(d);
    h.shutdown();
}

// ---------------------------------------------------------------------
// §1 L6: tenants
// ---------------------------------------------------------------------

/// Spec §1 L6: a tenant is a participant on its domain and a connection
/// under its client identifier. A message on the tenant's MQTT topic
/// reaches the tenant's participant and not the top level's, though both
/// have a topic of the same DDS name; a sample of the tenant's participant
/// is published on the tenant's topic.
#[test]
fn mqttbridge_1_l6_each_tenant_has_its_participant_and_its_client_id() {
    let h = broker_server(ServerConfig::default());
    let (mut cfg, _lease) = config(&format!("mqtt://{}", h.local_addr()), "tenant-top");
    let tenant_lease = zerodds_test_domains::lease_domain();
    cfg.topics.push(topic("Plant", "top/plant", "in"));
    cfg.tenants.push(TenantConfig {
        name: "a".into(),
        domain: tenant_lease.id(),
        client_id: "tenant-a".into(),
        username: None,
        password: None,
        topics: vec![
            topic("Plant", "a/plant", "in"),
            topic("Status", "a/status", "out"),
        ],
        acl_rules: Vec::new(),
        acl_default_deny: None,
    });
    let d = server::start(cfg.clone()).expect("daemon start");
    assert_eq!(d.tenants.len(), 1);
    let tenant = &d.tenants[0];
    assert_eq!(tenant.name, "a");
    assert_eq!(tenant.runtime.domain_id, tenant_lease.id());
    assert_eq!(d.runtime.domain_id, cfg.domain);
    assert_ne!(tenant.runtime.guid_prefix, d.runtime.guid_prefix);
    wait_until("both connections", || {
        let c = h.connected_clients();
        c.contains(&"tenant-top".to_string()) && c.contains(&"tenant-a".to_string())
    });
    let top_rx = reader_on(&d.runtime, "Plant");
    let tenant_rx = reader_on(&tenant.runtime, "Plant");
    let mut publisher =
        TestClient::connect(h.local_addr(), "tenant-pub", ProtocolVersion::V5).unwrap();
    let deadline = Instant::now() + TO;
    loop {
        publisher
            .publish("a/plant", &[0x00, 0x01, 0x00, 0x00, 0xA], 0, false)
            .unwrap();
        if let Some((p, _, _)) = next_alive(&tenant_rx, Duration::from_millis(300)) {
            assert_eq!(p, [0xA]);
            break;
        }
        assert!(Instant::now() < deadline, "nothing reached the tenant");
    }
    assert!(
        next_alive(&top_rx, Duration::from_millis(500)).is_none(),
        "the tenant's message reached the top level"
    );
    loop {
        publisher
            .publish("top/plant", &[0x00, 0x01, 0x00, 0x00, 0xB], 0, false)
            .unwrap();
        if let Some((p, _, _)) = next_alive(&top_rx, Duration::from_millis(300)) {
            assert_eq!(p, [0xB]);
            break;
        }
        assert!(Instant::now() < deadline, "nothing reached the top level");
    }
    assert!(next_alive(&tenant_rx, Duration::from_millis(500)).is_none());
    let mut sub = TestClient::connect(h.local_addr(), "tenant-sub", ProtocolVersion::V5).unwrap();
    sub.subscribe("a/status", 0).unwrap();
    thread::sleep(Duration::from_millis(200));
    assert!(
        tenant
            .runtime
            .test_inject_user_alive(tenant.user_readers["Status"], vec![0xC])
    );
    let (t, payload) = sub.recv_publish(TO).unwrap();
    assert_eq!((t.as_str(), &payload[4..]), ("a/status", &[0xC][..]));
    shutdown(d);
    h.shutdown();
}

/// Spec §1 L6 with §9.2: a SIGHUP reload applies a tenant's new ACL to the
/// tenant's participant at once: after `default_deny: true` without a rule
/// its samples no longer go out; the top level's do.
#[test]
fn mqttbridge_1_l6_a_reload_applies_the_acl_of_a_tenant() {
    let h = broker_server(ServerConfig::default());
    let (mut cfg, _lease) = config(&format!("mqtt://{}", h.local_addr()), "acl-top");
    let tenant_lease = zerodds_test_domains::lease_domain();
    cfg.topics.push(topic("Top", "acl/top", "out"));
    cfg.tenants.push(TenantConfig {
        name: "a".into(),
        domain: tenant_lease.id(),
        client_id: "acl-a".into(),
        username: None,
        password: None,
        topics: vec![topic("Status", "acl/a", "out")],
        acl_rules: Vec::new(),
        acl_default_deny: Some(false),
    });
    let next = Arc::new(Mutex::new(cfg.clone()));
    let n = Arc::clone(&next);
    let d = server::start_with_reload(
        cfg.clone(),
        zerodds_dcps::runtime::RuntimeConfig::default(),
        Some(Box::new(move || Ok(n.lock().unwrap().clone()))),
    )
    .expect("daemon start");
    let mut sub = TestClient::connect(h.local_addr(), "acl-sub", ProtocolVersion::V5).unwrap();
    sub.subscribe("acl/#", 0).unwrap();
    thread::sleep(Duration::from_millis(200));
    let tenant = &d.tenants[0];
    let send = |n: u8| {
        assert!(
            tenant
                .runtime
                .test_inject_user_alive(tenant.user_readers["Status"], vec![n])
        );
        assert!(
            d.runtime
                .test_inject_user_alive(d.user_readers["Top"], vec![n])
        );
    };
    send(1);
    let mut got = Vec::new();
    for _ in 0..2 {
        let (t, p) = sub.recv_publish(TO).unwrap();
        got.push((t, p[4]));
    }
    got.sort();
    assert_eq!(got, [("acl/a".to_string(), 1), ("acl/top".to_string(), 1)]);
    next.lock().unwrap().tenants[0].acl_default_deny = Some(true);
    d.reload_flag.store(true, Ordering::SeqCst);
    thread::sleep(Duration::from_millis(500));
    send(2);
    let (t, p) = sub.recv_publish(TO).unwrap();
    assert_eq!((t.as_str(), p[4]), ("acl/top", 2));
    assert!(
        sub.recv_publish(Duration::from_millis(700)).is_err(),
        "the tenant's sample went out after its ACL closed the topic"
    );
    shutdown(d);
    h.shutdown();
}

// ---------------------------------------------------------------------
// §7.3: the ACL; §9.1: the start
// ---------------------------------------------------------------------

/// Spec §7.3 with the rules of its example: the bridge authenticates as
/// `alice`; `chat/+` is hers to publish, `sensors/+/public/#` everyone's to
/// publish and subscribe, `default_deny: true` closes the rest. The samples
/// of the topic on `news/today` are not published and counted as publish
/// denials; the subscription to `chat/+` is not sent and counted as a
/// subscribe denial; a PUBLISH of the broker on a public sensor topic
/// reaches DDS.
#[test]
fn mqttbridge_7_3_the_rules_of_the_spec_let_alice_publish_chat_and_nothing_else() {
    use zerodds_mqtt_bridge::daemon::acl::AclRule;
    use zerodds_mqtt_bridge::daemon::metrics::family;
    let b = broker_server(ServerConfig::default());
    let (mut cfg, _lease) = config(&format!("mqtt://{}", b.local_addr()), "acl-alice");
    cfg.username = Some("alice".into());
    cfg.password = Some("pw".into());
    cfg.acl_default_deny = Some(true);
    cfg.acl_rules = vec![
        AclRule {
            subject: "*".into(),
            allow_publish: vec!["sensors/+/public/#".into()],
            allow_subscribe: vec!["sensors/+/public/#".into()],
        },
        AclRule {
            subject: "alice".into(),
            allow_publish: vec!["chat/+".into()],
            allow_subscribe: Vec::new(),
        },
    ];
    cfg.topics
        .push(topic("Chat::Message", "chat/general", "out"));
    cfg.topics.push(topic("News", "news/today", "out"));
    cfg.topics
        .push(topic("Public", "sensors/s1/public/temp", "out"));
    cfg.topics.push(topic("ChatIn", "chat/+", "in"));
    cfg.topics
        .push(topic("SensorsIn", "sensors/+/public/#", "in"));
    let h = server::start(cfg).expect("daemon start");
    let m = h.metrics.clone().unwrap();
    assert_eq!(
        m.value(family::ACL_DENIALS, &[("reason", "subscribe")]),
        1,
        "the subscription to chat/+"
    );
    let mut watcher =
        TestClient::connect(b.local_addr(), "acl-watch", ProtocolVersion::V5).unwrap();
    watcher.subscribe("#", 0).unwrap();
    for name in ["Chat::Message", "News", "Public"] {
        let eid = h.user_readers[name];
        assert!(h.runtime.test_inject_user_alive(eid, vec![1, 2, 3]));
    }
    let mut got = Vec::new();
    while let Ok((t, _)) = watcher.recv_publish(Duration::from_millis(1500)) {
        got.push(t);
    }
    got.sort();
    assert_eq!(got, ["chat/general", "sensors/s1/public/temp"]);
    assert_eq!(m.value(family::ACL_DENIALS, &[("reason", "publish")]), 1);
    assert_eq!(
        m.value(family::DDS_RECEIVED, &[("dds_topic", "News")]),
        1,
        "the sample came from DDS"
    );
    let rx = reader_on(&h.runtime, "SensorsIn");
    let mut publisher =
        TestClient::connect(b.local_addr(), "acl-pub", ProtocolVersion::V5).unwrap();
    let deadline = Instant::now() + TO;
    loop {
        publisher
            .publish(
                "sensors/k/public/hum",
                &[0x00, 0x01, 0x00, 0x00, 9],
                0,
                false,
            )
            .unwrap();
        if next_alive(&rx, Duration::from_millis(300)).is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "no sample of SensorsIn");
    }
    shutdown(h);
    b.shutdown();
}

/// Answers what follows the SUBACK of the start on `sock`: PUBACK to a
/// QoS 1 PUBLISH, PINGRESP, until DISCONNECT or the end.
fn serve_after_subscribe(mut sock: TcpStream) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let _ = sock.set_read_timeout(None);
        while let Ok((byte0, body)) = read_packet(&mut sock) {
            let answer = match byte0 >> 4 {
                3 if (byte0 >> 1) & 0x03 == 1 => {
                    let n = usize::from(u16::from_be_bytes([body[0], body[1]]));
                    Some(frame_packet(0x40, &[body[2 + n], body[3 + n], 0]).unwrap())
                }
                12 => Some(vec![0xD0, 0x00]),
                14 => return,
                _ => None,
            };
            if let Some(a) = answer {
                let _ = sock.write_all(&a);
            }
        }
    })
}

/// Spec §9.1: the participant (step 4) and the endpoints of the topics
/// (step 5) stand before the MQTT connect (step 6): while the broker holds
/// back its CONNACK, a participant of the test has discovered the bridge's
/// writer and reader; nothing goes out before the CONNACK, and the first
/// packet after it is the SUBSCRIBE (step 7).
#[test]
fn mqttbridge_9_1_the_order_is_participant_endpoints_connect_subscribe() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let (mut cfg, lease) = config(
        &format!("mqtt://{}", listener.local_addr().unwrap()),
        "order",
    );
    cfg.topics.push(topic("Order::In", "order/in", "in"));
    cfg.topics.push(topic("Order::Out", "order/out", "out"));
    let dds = foreign_participant(lease.id(), 0x91);
    let starter = thread::spawn(move || server::start(cfg).map_err(|e| e.to_string()));
    let (mut sock, _) = listener.accept().unwrap();
    sock.set_read_timeout(Some(TO)).unwrap();
    let (byte0, _) = read_packet(&mut sock).unwrap();
    assert_eq!(byte0 >> 4, 1, "the first packet is the CONNECT");
    wait_until("the bridge's writer of Order::In", || {
        dds.discovered_publication_topics()
            .iter()
            .any(|(t, _)| t == "Order::In")
    });
    wait_until("the bridge's reader of Order::Out", || {
        dds.discovered_subscription_topics()
            .iter()
            .any(|(t, _)| t == "Order::Out")
    });
    sock.set_read_timeout(Some(Duration::from_millis(300)))
        .unwrap();
    assert!(
        read_packet(&mut sock).is_err(),
        "a packet before the CONNACK"
    );
    sock.set_read_timeout(Some(TO)).unwrap();
    sock.write_all(&frame_packet(0x20, &[0, 0, 0]).unwrap())
        .unwrap();
    let (byte0, body) = read_packet(&mut sock).unwrap();
    assert_eq!(byte0 >> 4, 8, "the first packet after the CONNACK");
    let mut suback = body[..2].to_vec();
    suback.extend([0, 0]);
    sock.write_all(&frame_packet(0x90, &suback).unwrap())
        .unwrap();
    let served = serve_after_subscribe(sock);
    let h = starter.join().unwrap().expect("daemon start");
    shutdown(h);
    let _ = served.join();
    dds.shutdown();
}

/// Spec §9.1 step 6: a broker that comes up one second after the binary is
/// reached by the retries of `mqtt.reconnect`; the daemon then publishes its
/// catalog (§5.2), and a SIGTERM ends it with exit 0.
#[test]
fn mqttbridge_9_1_the_daemon_connects_once_the_broker_comes_up() {
    let dir = scratch("late-broker");
    let lease = zerodds_test_domains::lease_domain();
    let port = free_port();
    let url = format!("mqtt://127.0.0.1:{port}");
    let mut child = Daemon(
        Command::new(BIN)
            .args(args(
                &dir,
                lease.id(),
                &[
                    "--broker",
                    &url,
                    "--client-id",
                    "late-broker",
                    "--topic",
                    "Late::T:late/t",
                    "--set",
                    "mqtt.reconnect.initial_delay_ms=100",
                    "--set",
                    "mqtt.reconnect.max_delay_ms=200",
                    "--set",
                    &format!("mqtt.session_dir={}", dir.join("sessions").display()),
                ],
            ))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    thread::sleep(Duration::from_secs(1));
    assert!(child.0.try_wait().unwrap().is_none(), "gave up");
    let b = MqttBrokerServer::bind_with(format!("127.0.0.1:{port}"), ServerConfig::default())
        .unwrap()
        .spawn()
        .unwrap();
    wait_until("the catalog of the daemon", || {
        !retained_on(&b, "late-watch", "$zerodds/late-broker/catalog").is_empty()
    });
    let killed = Command::new("kill")
        .args(["-TERM", &child.0.id().to_string()])
        .status()
        .unwrap();
    assert!(killed.success());
    let t0 = Instant::now();
    let status = loop {
        if let Some(st) = child.0.try_wait().unwrap() {
            break st;
        }
        assert!(t0.elapsed() < TO, "still running");
        thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(0));
    b.shutdown();
}

// ---------------------------------------------------------------------
// §7.2 with §9.2: the credentials of a reload; §8.2: the counting of the
// daemon
// ---------------------------------------------------------------------

/// Spec §7.2, §9.2 under `password`: a SIGHUP reload with a new
/// `mqtt.password` keeps the standing connection; the CONNECT after the
/// broker ended it (another client took the session over, 0x8E) carries
/// the new password.
#[test]
fn mqttbridge_7_2_a_reload_gives_the_next_connect_the_new_password() {
    let rec = Arc::new(Recording {
        seen: Mutex::default(),
        answer: Ok(()),
    });
    let b = broker_server(ServerConfig {
        authenticator: Some(Arc::clone(&rec) as Arc<dyn Authenticator>),
        ..ServerConfig::default()
    });
    let (mut cfg, _lease) = config(&format!("mqtt://{}", b.local_addr()), "reload-pw");
    cfg.username = Some("bridge".into());
    cfg.password = Some("pw-1".into());
    cfg.reconnect.initial_delay_ms = 100;
    cfg.reconnect.max_delay_ms = 300;
    let next = Arc::new(Mutex::new(cfg.clone()));
    let n = Arc::clone(&next);
    let d = server::start_with_reload(
        cfg.clone(),
        zerodds_dcps::runtime::RuntimeConfig::default(),
        Some(Box::new(move || Ok(n.lock().unwrap().clone()))),
    )
    .expect("daemon start");
    let bridge_connects = || {
        rec.seen
            .lock()
            .unwrap()
            .iter()
            .filter(|(id, user, _)| id == "reload-pw" && user.as_deref() == Some("bridge"))
            .map(|(_, _, pass)| pass.clone().unwrap_or_default())
            .collect::<Vec<_>>()
    };
    assert_eq!(bridge_connects(), [b"pw-1".to_vec()]);
    next.lock().unwrap().password = Some("pw-2".into());
    d.reload_flag.store(true, Ordering::SeqCst);
    wait_until("the reload", || !d.reload_flag.load(Ordering::SeqCst));
    thread::sleep(Duration::from_millis(500));
    assert_eq!(bridge_connects().len(), 1, "the reload kept the connection");
    let _taker = TestClient::connect(b.local_addr(), "reload-pw", ProtocolVersion::V5).unwrap();
    wait_until("the next CONNECT", || bridge_connects().len() == 2);
    assert_eq!(bridge_connects()[1], b"pw-2");
    shutdown(d);
    b.shutdown();
}

/// An Authenticator that refuses the first CONNECT of `id` with 0x88
/// Server unavailable and lets every later one in.
struct RefuseOnce {
    id: String,
    refused: AtomicBool,
}

impl Authenticator for RefuseOnce {
    fn authenticate(
        &self,
        client_id: &str,
        _user_name: Option<&str>,
        _password: Option<&[u8]>,
    ) -> Result<(), u8> {
        if client_id == self.id && !self.refused.swap(true, Ordering::SeqCst) {
            Err(0x88)
        } else {
            Ok(())
        }
    }

    fn start(
        &self,
        _client_id: &str,
        _method: &str,
    ) -> Option<Box<dyn zerodds_mqtt_bridge::server::AuthExchange>> {
        None
    }
}

/// Spec §8.2: each connect attempt counts in connect_attempts_total, the
/// one the broker accepts also in connect_successes_total: a start whose
/// first CONNECT the broker refuses (0x88) and whose second it accepts is
/// two attempts and one success.
#[test]
fn mqttbridge_8_2_a_refused_and_an_accepted_connect_are_two_attempts_and_one_success() {
    use zerodds_mqtt_bridge::daemon::metrics::family;
    let b = broker_server(ServerConfig {
        authenticator: Some(Arc::new(RefuseOnce {
            id: "attempts-cli".into(),
            refused: AtomicBool::new(false),
        }) as Arc<dyn Authenticator>),
        ..ServerConfig::default()
    });
    let (mut cfg, _lease) = config(&format!("mqtt://{}", b.local_addr()), "attempts-cli");
    cfg.reconnect.initial_delay_ms = 100;
    let h = server::start(cfg).expect("daemon start");
    let m = h.metrics.clone().unwrap();
    assert_eq!(m.total(family::CONNECT_ATTEMPTS), 2);
    assert_eq!(m.total(family::CONNECT_SUCCESSES), 1);
    shutdown(h);
    b.shutdown();
}

/// Spec §8.2: a PUBLISH to the broker counts in publish_out_total under its
/// DDS topic and its QoS, its payload octets in bytes_out_total; a PUBLISH
/// of the broker counts in publish_in_total under the DDS topic it is
/// routed to and the QoS it came with, its payload octets in
/// bytes_in_total.
#[test]
fn mqttbridge_8_2_each_publish_counts_by_topic_qos_and_octets() {
    use zerodds_mqtt_bridge::daemon::metrics::family;
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "count-cli");
    cfg.topics.push(topic("CountOut", "count/out", "out"));
    cfg.topics.push(topic("CountIn", "count/in", "in"));
    let h = server::start(cfg).expect("daemon start");
    rec.wait_subscribed();
    let m = h.metrics.clone().unwrap();
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["CountOut"], vec![1, 2, 3])
    );
    wait_until("the PUBLISH", || rec.markers("count/out").len() == 1);
    let sent = rec
        .publishes
        .lock()
        .unwrap()
        .iter()
        .find(|p| p.topic == "count/out")
        .unwrap()
        .clone();
    assert_eq!(sent.qos, 1, "a reliable topic");
    wait_until("publish_out", || {
        m.value(
            family::PUBLISH_OUT,
            &[("dds_topic", "CountOut"), ("mqtt_qos", "1")],
        ) == 1
    });
    assert_eq!(
        m.value(family::BYTES_OUT, &[("dds_topic", "CountOut")]),
        sent.payload.len() as u64
    );
    rec.inject(
        "count/in",
        &[0x00, 0x01, 0x00, 0x00, 7, 8],
        &PropertySet::default(),
    );
    wait_until("publish_in", || {
        m.value(
            family::PUBLISH_IN,
            &[("dds_topic", "CountIn"), ("mqtt_qos", "0")],
        ) == 1
    });
    assert_eq!(m.value(family::BYTES_IN, &[("dds_topic", "CountIn")]), 6);
    assert_eq!(
        m.value(
            family::PUBLISH_IN,
            &[("dds_topic", "CountOut"), ("mqtt_qos", "1")]
        ),
        0
    );
    assert_eq!(
        m.value(
            family::PUBLISH_OUT,
            &[("dds_topic", "CountIn"), ("mqtt_qos", "1")]
        ),
        0
    );
    shutdown(h);
}

/// Spec §8.2 inflight_messages: `out` counts the bridge's QoS 1 PUBLISH
/// the broker has not acknowledged, `in` the broker's QoS 2 PUBLISH whose
/// PUBREL has not come; each goes back to 0 when its flow ends.
#[test]
fn mqttbridge_8_2_inflight_counts_the_flows_that_have_not_ended() {
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "inflight-cli");
    cfg.topics.push(topic("FlowOut", "flow/out", "out"));
    let mut inbound = topic("FlowIn", "flow/in", "in");
    inbound.mqtt_qos = Some(2);
    cfg.topics.push(inbound);
    let h = server::start(cfg).expect("daemon start");
    rec.wait_subscribed();
    let m = h.metrics.clone().unwrap();
    assert_eq!((m.inflight("out"), m.inflight("in")), (0, 0));
    rec.hold_acks();
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["FlowOut"], vec![1])
    );
    wait_until("the PUBLISH", || rec.markers("flow/out").len() == 1);
    wait_until("inflight out 1", || m.inflight("out") == 1);
    rec.release_acks();
    wait_until("inflight out 0", || m.inflight("out") == 0);
    rec.send_raw(
        &encode_publish(&PublishPacket {
            dup: false,
            qos: 2,
            retain: false,
            topic: "flow/in".to_string(),
            packet_id: Some(9),
            properties: PropertySet::default().encode().unwrap(),
            payload: vec![0x00, 0x01, 0x00, 0x00, 5],
        })
        .unwrap(),
    );
    wait_until("inflight in 1", || m.inflight("in") == 1);
    // PUBREL 9, Reason Code 0x00.
    rec.send_raw(&frame_packet(0x62, &[0, 9]).unwrap());
    wait_until("inflight in 0", || m.inflight("in") == 0);
    shutdown(h);
}

/// Spec §8.2 broker_disconnects_total: a connection the broker ends with a
/// DISCONNECT counts under its Reason Code (`0x8e`), one whose socket
/// closes without a DISCONNECT under `network`.
#[test]
fn mqttbridge_8_2_a_lost_connection_counts_under_its_reason_code() {
    use zerodds_mqtt_bridge::daemon::metrics::family;
    // The connection does not come back: the shutdown waits for the ack of
    // the catalog's clearing (§5.2, §9.2) no longer than this.
    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "disc-code-cli");
    cfg.drain_timeout_secs = 1;
    let h = server::start(cfg).expect("daemon start");
    let m = h.metrics.clone().unwrap();
    // DISCONNECT 0x8E Session taken over, no properties.
    rec.send_raw(&frame_packet(0xE0, &[0x8E, 0]).unwrap());
    wait_until("the count of 0x8e", || {
        m.value(family::BROKER_DISCONNECTS, &[("reason_code", "0x8e")]) == 1
    });
    assert_eq!(
        m.value(family::BROKER_DISCONNECTS, &[("reason_code", "network")]),
        0
    );
    shutdown(h);

    let rec = Recorder::start(PropertySet::default());
    let (mut cfg, _lease) = config(&rec.url(), "disc-net-cli");
    cfg.drain_timeout_secs = 1;
    let h = server::start(cfg).expect("daemon start");
    let m = h.metrics.clone().unwrap();
    rec.conn
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .shutdown(std::net::Shutdown::Both)
        .unwrap();
    wait_until("the count of network", || {
        m.value(family::BROKER_DISCONNECTS, &[("reason_code", "network")]) == 1
    });
    assert_eq!(m.total(family::BROKER_DISCONNECTS), 1);
    shutdown(h);
}

// ---------------------------------------------------------------------
// §8.1: the JSON log of the binary
// ---------------------------------------------------------------------

/// Sends `sig` (`TERM`, `HUP`) to the daemon process.
fn signal(child: &Daemon, sig: &str) {
    let sent = Command::new("kill")
        .args([&format!("-{sig}"), &child.0.id().to_string()])
        .status()
        .unwrap();
    assert!(sent.success());
}

/// Waits until the daemon process exits: its exit code.
fn exit_code(child: &mut Daemon) -> Option<i32> {
    let t0 = Instant::now();
    loop {
        if let Some(st) = child.0.try_wait().unwrap() {
            return st.code();
        }
        assert!(t0.elapsed() < TO, "still running");
        thread::sleep(Duration::from_millis(20));
    }
}

/// Spec §8.1: with `log_level: debug` every line of the binary on stdout is
/// one JSON object with `timestamp` (RFC 3339 UTC), `level` and `event`; a
/// message each way is a `publish_in` and a `publish_out` line with
/// `mqtt_topic`, `dds_topic`, `bytes`, `mqtt_qos` and `latency_us`; the
/// connection is `connect` with `connect_state` connected, the shutdown a
/// `disconnect` with `connect_state` disconnected and reason code 0x00;
/// stderr stays empty.
#[test]
fn mqttbridge_8_1_the_binary_logs_a_publish_with_the_fields_of_the_spec() {
    let dir = scratch("log-fields");
    let lease = zerodds_test_domains::lease_domain();
    let b = broker_server(ServerConfig::default());
    let file = dir.join("mqtt-bridged.yaml");
    std::fs::write(
        &file,
        format!(
            "log_level: debug\nmqtt:\n  broker_url: mqtt://{}\n  client_id: log-cli\n  session_dir: {}\ntopics:\n  - dds_name: LogIn\n    mqtt_topic: log/in\n    direction: in\n  - dds_name: LogOut\n    mqtt_topic: log/out\n    direction: out\n",
            b.local_addr(),
            dir.join("sessions").display()
        ),
    )
    .unwrap();
    let mut child = Daemon(
        Command::new(BIN)
            .args(["--config", &file.display().to_string()])
            .args(["--domain", &lease.id().to_string()])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let log = Log::of(child.0.stdout.take().unwrap());
    let err = Log::of(child.0.stderr.take().unwrap());
    log.wait_for("running");
    let connect = log.wait_for("connect");
    assert_eq!(connect["level"], "info");
    assert_eq!(connect["connect_state"], "connected");
    assert_eq!(connect["client_id"], "log-cli");
    assert_eq!(connect["protocol"], "5.0");
    let subscribed = log.wait_for("subscribed");
    assert_eq!(subscribed["granted"], 1);

    // DDS -> MQTT.
    let dds = foreign_participant(lease.id(), 0x81);
    let w = dds
        .register_user_writer(UserWriterConfig {
            topic_name: "LogOut".into(),
            type_name: "LogOut".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    let (_r, rx) = dds
        .register_user_reader(UserReaderConfig {
            topic_name: "LogIn".into(),
            type_name: "LogIn".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    wait_until("the bridge's reader over RTPS", || {
        !dds.user_writer_matched_reader_guids(w).is_empty()
    });
    let mut sub = TestClient::connect(b.local_addr(), "log-sub", ProtocolVersion::V5).unwrap();
    sub.subscribe("log/out", 1).unwrap();
    dds.write_user_sample(w, vec![9, 8, 7]).unwrap();
    let (t, payload) = sub.recv_publish(TO).unwrap();
    assert_eq!(t, "log/out");
    let out = log.wait_for("publish_out");
    assert_eq!(out["level"], "debug");
    assert_eq!(out["mqtt_topic"], "log/out");
    assert_eq!(out["dds_topic"], "LogOut");
    assert_eq!(out["bytes"], payload.len());
    assert_eq!(out["mqtt_qos"], 1);
    assert!(out["latency_us"].is_u64(), "{out}");

    // MQTT -> DDS.
    let mut publisher =
        TestClient::connect(b.local_addr(), "log-pub", ProtocolVersion::V5).unwrap();
    let deadline = Instant::now() + TO;
    loop {
        publisher
            .publish("log/in", &[0x00, 0x01, 0x00, 0x00, 4, 5], 0, false)
            .unwrap();
        if next_alive(&rx, Duration::from_millis(300)).is_some() {
            break;
        }
        assert!(Instant::now() < deadline, "nothing reached DDS");
    }
    let inbound = log.wait_for("publish_in");
    assert_eq!(inbound["level"], "debug");
    assert_eq!(inbound["mqtt_topic"], "log/in");
    assert_eq!(inbound["dds_topic"], "LogIn");
    assert_eq!(inbound["bytes"], 6);
    assert_eq!(inbound["mqtt_qos"], 0);
    assert!(inbound["latency_us"].is_u64(), "{inbound}");

    signal(&child, "TERM");
    assert_eq!(exit_code(&mut child), Some(0));
    log.wait_closed();
    let disconnect = log
        .events("disconnect")
        .into_iter()
        .last()
        .expect("a disconnect line");
    assert_eq!(disconnect["connect_state"], "disconnected");
    assert_eq!(disconnect["reason_code"], "0x00");
    assert_eq!(log.events("stopped").len(), 1);
    for line in log.lines() {
        let ts = line["timestamp"].as_str().unwrap_or_default();
        assert!(
            ts.len() == 24 && ts.ends_with('Z') && &ts[10..11] == "T",
            "{line}"
        );
        assert!(
            ["trace", "debug", "info", "warn", "error"].contains(&line["level"].as_str().unwrap()),
            "{line}"
        );
        assert!(line["event"].is_string(), "{line}");
    }
    err.wait_closed();
    assert_eq!(err.text(), "", "the log goes to stdout");
    b.shutdown();
}

/// Spec §8.1: `log_level` is the threshold of the binary's lines: under
/// `warn` the connection, the subscription and a message write no line,
/// the refused subscription writes its `warn` line; after the file says
/// `debug` and a SIGHUP, the `reload` line and the next message's
/// `publish_in` line are written.
#[test]
fn mqttbridge_8_1_log_level_warn_writes_no_info_line_and_a_reload_applies_debug() {
    let dir = scratch("log-level");
    let lease = zerodds_test_domains::lease_domain();
    let rec = Recorder::start(PropertySet::default());
    rec.refuse.lock().unwrap().push("lvl/refused".into());
    let file = dir.join("mqtt-bridged.yaml");
    let write = |level: &str| {
        std::fs::write(
            &file,
            format!(
                "log_level: {level}\nmqtt:\n  broker_url: {}\n  client_id: level-cli\n  session_dir: {}\ntopics:\n  - dds_name: LvlIn\n    mqtt_topic: lvl/in\n    direction: in\n  - dds_name: LvlRefused\n    mqtt_topic: lvl/refused\n    direction: in\n",
                rec.url(),
                dir.join("sessions").display()
            ),
        )
        .unwrap();
    };
    write("warn");
    let mut child = Daemon(
        Command::new(BIN)
            .args(["--config", &file.display().to_string()])
            .args(["--domain", &lease.id().to_string()])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let log = Log::of(child.0.stdout.take().unwrap());
    rec.wait_subscribed();
    let refused = log.wait_for("subscription_refused");
    assert_eq!(refused["level"], "warn");
    rec.inject(
        "lvl/in",
        &[0x00, 0x01, 0x00, 0x00, 1],
        &PropertySet::default(),
    );
    thread::sleep(Duration::from_millis(500));
    let below: Vec<_> = log
        .lines()
        .into_iter()
        .filter(|l| !matches!(l["level"].as_str(), Some("warn" | "error")))
        .collect();
    assert!(below.is_empty(), "lines below warn: {below:?}");

    write("debug");
    signal(&child, "HUP");
    let reload = log.wait_for("reload");
    assert_eq!(reload["level"], "info");
    rec.inject(
        "lvl/in",
        &[0x00, 0x01, 0x00, 0x00, 2],
        &PropertySet::default(),
    );
    let inbound = log.wait_for("publish_in");
    assert_eq!(inbound["mqtt_topic"], "lvl/in");
    assert_eq!(inbound["dds_topic"], "LvlIn");
    assert_eq!(inbound["bytes"], 5);
    signal(&child, "TERM");
    assert_eq!(exit_code(&mut child), Some(0));
}

/// A broker that answers the first CONNECT of each client identifier with
/// Session Present 1 and every later one with 0, grants each subscription
/// and refuses each QoS 1 PUBLISH with PUBACK 0x87 Not authorized.
fn resuming_refusing_broker() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    thread::spawn(move || {
        for sock in listener.incoming() {
            let Ok(mut sock) = sock else { return };
            let seen = Arc::clone(&seen);
            thread::spawn(move || {
                let send = |sock: &mut TcpStream, b0: u8, b: &[u8]| {
                    let _ = sock.write_all(&frame_packet(b0, b).unwrap());
                };
                while let Ok((byte0, body)) = read_packet(&mut sock) {
                    match ControlPacketType::from_bits(byte0 >> 4) {
                        Some(ControlPacketType::Connect) => {
                            let id = decode_connect_body(&body).unwrap().client_id;
                            let mut seen = seen.lock().unwrap();
                            let present = !seen.contains(&id);
                            seen.push(id);
                            send(&mut sock, 0x20, &[u8::from(present), 0, 0]);
                        }
                        Some(ControlPacketType::Subscribe) => {
                            let sub = decode_subscribe_body(&body).unwrap();
                            let mut ack = sub.packet_id.to_be_bytes().to_vec();
                            ack.push(0);
                            ack.extend(sub.subscriptions.iter().map(|s| s.options & 0x03));
                            send(&mut sock, 0x90, &ack);
                        }
                        Some(ControlPacketType::Publish) if (byte0 >> 1) & 0x03 == 1 => {
                            let len = usize::from(u16::from_be_bytes([body[0], body[1]]));
                            let id = &body[2 + len..4 + len];
                            send(&mut sock, 0x40, &[id[0], id[1], 0x87, 0]);
                        }
                        Some(ControlPacketType::PingReq) => send(&mut sock, 0xD0, &[]),
                        Some(ControlPacketType::Disconnect) => return,
                        _ => {}
                    }
                }
            });
        }
    });
    addr
}

/// Spec §8.1 with §1 L6: the lines of a tenant's connection carry
/// `tenant`, also those the MQTT client and the Session State write. The
/// broker resumes a session neither connection holds state of
/// (`session_unknown`, written while the connection is made) and refuses
/// each catalog PUBLISH (`publish_refused_by_broker`, written when the
/// PUBACK is read): the tenant's lines carry `tenant` a, the top level's
/// none.
#[test]
fn mqttbridge_8_1_the_client_lines_of_a_tenant_carry_its_name() {
    let dir = scratch("tenant-lines");
    let lease = zerodds_test_domains::lease_domain();
    let tenant_lease = zerodds_test_domains::lease_domain();
    let addr = resuming_refusing_broker();
    let file = dir.join("mqtt-bridged.yaml");
    std::fs::write(
        &file,
        format!(
            "mqtt:\n  broker_url: mqtt://{addr}\n  client_id: tl-top\n  clean_start: false\n  session_expiry_interval_secs: 300\n  session_dir: {}\ntenants:\n  - name: a\n    domain: {}\n    mqtt:\n      client_id: tl-a\n",
            dir.join("sessions").display(),
            tenant_lease.id()
        ),
    )
    .unwrap();
    let mut child = Daemon(
        Command::new(BIN)
            .args(["--config", &file.display().to_string()])
            .args(["--domain", &lease.id().to_string()])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let log = Log::of(child.0.stdout.take().unwrap());
    log.wait_for("running");
    let mut refused = Vec::new();
    wait_until("the refused catalog of both connections", || {
        refused = log.events("publish_refused_by_broker");
        refused.len() >= 2
    });
    let unknown = log.events("session_unknown");
    for (event, lines) in [
        ("session_unknown", &unknown),
        ("publish_refused_by_broker", &refused),
    ] {
        let of = |id: &str| -> Vec<&serde_json::Value> {
            lines.iter().filter(|l| l["client_id"] == id).collect()
        };
        let (top, tenant) = (of("tl-top"), of("tl-a"));
        assert!(!top.is_empty() && !tenant.is_empty(), "{event}: {lines:?}");
        for l in tenant {
            assert_eq!(l["tenant"], "a", "{event}: {l}");
        }
        for l in top {
            assert!(l.get("tenant").is_none(), "{event}: {l}");
        }
    }
    assert_eq!(refused[0]["reason_code"], "0x87");
    signal(&child, "TERM");
    assert_eq!(exit_code(&mut child), Some(0));
}

// ---------------------------------------------------------------------
// §9.3: the reconnect
// ---------------------------------------------------------------------

/// The status line of `GET /healthz` on the admin endpoint `addr`.
fn healthz(addr: &str) -> String {
    let Ok(mut s) = TcpStream::connect(addr) else {
        return String::new();
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let _ = s.write_all(b"GET /healthz HTTP/1.1\r\nHost: x\r\n\r\n");
    let mut body = String::new();
    let _ = s.read_to_string(&mut body);
    body.lines().next().unwrap_or_default().to_string()
}

/// Spec §9.3: the broker goes away and comes back on its port without the
/// daemon's subscriptions and retained messages. While it is away
/// `/healthz` answers 503; the daemon connects again, `/healthz` answers
/// 200, a message on the subscribed topic reaches DDS again (the daemon
/// subscribed again, §4.3) and the new broker holds the retained catalog
/// (§5.2).
#[test]
fn mqttbridge_9_3_after_a_broker_restart_the_daemon_subscribes_and_publishes_its_catalog_again() {
    let b1 = broker_server(ServerConfig::default());
    let port = b1.local_addr().port();
    let (mut cfg, _lease) = config(&format!("mqtt://127.0.0.1:{port}"), "restart-cli");
    cfg.reconnect.initial_delay_ms = 100;
    cfg.reconnect.max_delay_ms = 300;
    cfg.metrics_enabled = true;
    cfg.metrics_addr = "127.0.0.1:0".into();
    cfg.topics.push(topic("Re", "re/in", "in"));
    let h = server::start(cfg).expect("daemon start");
    let admin = h.admin_addr.clone().expect("the admin endpoint");
    assert_eq!(healthz(&admin), "HTTP/1.1 200 OK");
    let rx = reader_on(&h.runtime, "Re");
    let carried = |b: &ServerHandle, marker: u8| {
        let mut publisher =
            TestClient::connect(b.local_addr(), "restart-pub", ProtocolVersion::V5).unwrap();
        let deadline = Instant::now() + TO;
        loop {
            publisher
                .publish("re/in", &[0x00, 0x01, 0x00, 0x00, marker], 0, false)
                .unwrap();
            if let Some((p, _, _)) = next_alive(&rx, Duration::from_millis(300)) {
                return p;
            }
            assert!(Instant::now() < deadline, "nothing reached DDS");
        }
    };
    assert_eq!(carried(&b1, 1), [1]);

    b1.shutdown();
    wait_until("/healthz 503 while the broker is away", || {
        healthz(&admin) == "HTTP/1.1 503 Service Unavailable"
    });
    assert!(!h.healthy.load(Ordering::SeqCst));
    let b2 = broker_on(port);
    wait_until("/healthz 200 after the reconnect", || {
        healthz(&admin) == "HTTP/1.1 200 OK"
    });
    assert_eq!(
        carried(&b2, 2),
        [2],
        "the subscription of the new connection"
    );
    assert_eq!(
        retained_on(&b2, "restart-catalog", "$zerodds/+/catalog"),
        ["$zerodds/restart-cli/catalog"]
    );
    shutdown(h);
    b2.shutdown();
}

/// Spec §9.3: after a lost connection the first attempt goes out at once
/// (its `connect_failed` line comes less than half of `initial_delay_ms`
/// after the `disconnect` line), then one after each wait of
/// `mqtt.reconnect`: 400, 800, 1600, 1600 ms for `initial_delay_ms` 400,
/// `factor` 2, `max_delay_ms` 1600 — the binary logs each failed attempt
/// as `connect_failed` with `connect_state` reconnecting and the wait in
/// `next_attempt_ms` (§8.1), and the attempts are that far apart. When the
/// broker is back, the next attempt stands.
#[test]
fn mqttbridge_9_3_the_waits_between_the_attempts_grow_to_the_maximum() {
    let dir = scratch("reconnect-waits");
    let lease = zerodds_test_domains::lease_domain();
    let b1 = broker_server(ServerConfig::default());
    let port = b1.local_addr().port();
    let url = format!("mqtt://127.0.0.1:{port}");
    let mut child = Daemon(
        Command::new(BIN)
            .args(args(
                &dir,
                lease.id(),
                &[
                    "--broker",
                    &url,
                    "--client-id",
                    "waits-cli",
                    "--set",
                    "mqtt.reconnect.initial_delay_ms=400",
                    "--set",
                    "mqtt.reconnect.factor=2.0",
                    "--set",
                    "mqtt.reconnect.max_delay_ms=1600",
                ],
            ))
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let log = Log::of(child.0.stdout.take().unwrap());
    log.wait_for("connect");
    b1.shutdown();
    let lost = log.wait_for("disconnect");
    assert_eq!(lost["connect_state"], "disconnected");
    // The broker server's DISCONNECT 0x8B Server shutting down.
    assert_eq!(lost["reason_code"], "0x8b");
    let mut failed = Vec::new();
    wait_until("five failed attempts", || {
        failed = log.events("connect_failed");
        failed.len() >= 5
    });
    let waits: Vec<u64> = failed
        .iter()
        .map(|l| {
            assert_eq!(l["connect_state"], "reconnecting", "{l}");
            l["next_attempt_ms"].as_u64().unwrap()
        })
        .collect();
    assert_eq!(waits[..5], [400, 800, 1600, 1600, 1600], "{waits:?}");
    let at = |l: &serde_json::Value| -> u64 {
        // `hh:mm:ss.mmm` of the timestamp, in milliseconds of the day.
        let t = &l["timestamp"].as_str().unwrap()[11..23];
        let n = |r: std::ops::Range<usize>| t[r].parse::<u64>().unwrap();
        ((n(0..2) * 60 + n(3..5)) * 60 + n(6..8)) * 1000 + n(9..12)
    };
    // The first attempt does not wait `initial_delay_ms`.
    let first = at(&failed[0]).saturating_sub(at(&lost));
    assert!(
        first < 200,
        "the first attempt came {first} ms after the disconnect, initial_delay_ms is 400"
    );
    for (i, pair) in failed[..5].windows(2).enumerate() {
        let apart = at(&pair[1]).saturating_sub(at(&pair[0]));
        assert!(
            apart + 20 >= waits[i],
            "attempt {} came {apart} ms after the one before, the wait was {} ms",
            i + 2,
            waits[i]
        );
    }
    let _b2 = broker_on(port);
    wait_until("the connection to the broker that is back", || {
        log.events("connect").len() == 2
    });
    signal(&child, "TERM");
    assert_eq!(exit_code(&mut child), Some(0));
}

/// An Authenticator that refuses the client `id` with 0x86 Bad User Name or
/// Password `refusals` times and then lets it in; it notes when each of
/// its CONNECT packets came.
struct RefuseFirst {
    id: String,
    refusals: usize,
    seen: Mutex<Vec<Instant>>,
}

impl Authenticator for RefuseFirst {
    fn authenticate(
        &self,
        client_id: &str,
        _user_name: Option<&str>,
        _password: Option<&[u8]>,
    ) -> Result<(), u8> {
        if client_id != self.id {
            return Ok(());
        }
        let mut seen = self.seen.lock().unwrap();
        seen.push(Instant::now());
        if seen.len() <= self.refusals {
            Err(0x86)
        } else {
            Ok(())
        }
    }

    fn start(
        &self,
        _client_id: &str,
        _method: &str,
    ) -> Option<Box<dyn zerodds_mqtt_bridge::server::AuthExchange>> {
        None
    }
}

/// Spec §9.3: after a lost connection a refusal of the credentials (CONNACK
/// 0x86, which ends a start with exit 5) does not end the daemon: it is
/// tried again after the next wait of `mqtt.reconnect`, until the broker
/// lets it in.
#[test]
fn mqttbridge_9_3_a_refused_reconnect_is_tried_again_after_the_next_wait() {
    let b1 = broker_server(ServerConfig::default());
    let port = b1.local_addr().port();
    let (mut cfg, _lease) = config(&format!("mqtt://127.0.0.1:{port}"), "refused-re");
    cfg.reconnect.initial_delay_ms = 200;
    cfg.reconnect.factor = 2.0;
    cfg.reconnect.max_delay_ms = 400;
    let h = server::start(cfg).expect("daemon start");
    b1.shutdown();
    wait_until("the lost connection", || !h.healthy.load(Ordering::SeqCst));
    let gate = Arc::new(RefuseFirst {
        id: "refused-re".into(),
        refusals: 2,
        seen: Mutex::default(),
    });
    let deadline = Instant::now() + TO;
    let b2 = loop {
        let config = ServerConfig {
            authenticator: Some(Arc::clone(&gate) as Arc<dyn Authenticator>),
            ..ServerConfig::default()
        };
        match MqttBrokerServer::bind_with(("127.0.0.1", port), config) {
            Ok(b) => break b.spawn().unwrap(),
            Err(e) => {
                assert!(Instant::now() < deadline, "bind {port}: {e}");
                thread::sleep(Duration::from_millis(50));
            }
        }
    };
    wait_until("the reconnect after two refusals", || {
        h.healthy.load(Ordering::SeqCst)
    });
    let seen = gate.seen.lock().unwrap().clone();
    assert_eq!(
        seen.len(),
        3,
        "two refused CONNECT packets, then one let in"
    );
    assert!(
        seen[1] - seen[0] >= Duration::from_millis(180),
        "{:?}",
        seen[1] - seen[0]
    );
    assert!(
        seen[2] - seen[1] >= Duration::from_millis(380),
        "{:?}",
        seen[2] - seen[1]
    );
    assert!(b2.connected_clients().contains(&"refused-re".to_string()));
    shutdown(h);
    b2.shutdown();
}

/// A broker that serves one connection after the other: CONNACK, SUBACK,
/// PUBACK; the connection number `n` (from 0) is served by `rule(n)`: whether
/// it answers PINGREQ, and a Reason Code it ends the connection with by a
/// DISCONNECT 200 ms after the PUBACK of the first PUBLISH (the catalog of
/// §5.2, which follows the SUBSCRIBE). It notes when each CONNECT came.
fn serial_broker(rule: fn(usize) -> (bool, Option<u8>)) -> (SocketAddr, Arc<Mutex<Vec<Instant>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let connects: Arc<Mutex<Vec<Instant>>> = Arc::default();
    let seen = Arc::clone(&connects);
    thread::spawn(move || {
        for (n, sock) in listener.incoming().enumerate() {
            let Ok(mut sock) = sock else { return };
            let (pings, end) = rule(n);
            let seen = Arc::clone(&seen);
            thread::spawn(move || {
                let send = |sock: &mut TcpStream, b0: u8, b: &[u8]| {
                    let _ = sock.write_all(&frame_packet(b0, b).unwrap());
                };
                while let Ok((byte0, body)) = read_packet(&mut sock) {
                    match ControlPacketType::from_bits(byte0 >> 4) {
                        Some(ControlPacketType::Connect) => {
                            seen.lock().unwrap().push(Instant::now());
                            send(&mut sock, 0x20, &[0, 0, 0]);
                        }
                        Some(ControlPacketType::Subscribe) => {
                            let sub = decode_subscribe_body(&body).unwrap();
                            let mut ack = sub.packet_id.to_be_bytes().to_vec();
                            ack.push(0);
                            ack.extend(sub.subscriptions.iter().map(|s| s.options & 0x03));
                            send(&mut sock, 0x90, &ack);
                        }
                        Some(ControlPacketType::Publish) => {
                            let qos = (byte0 >> 1) & 0x03;
                            if qos == 1 {
                                let len = usize::from(u16::from_be_bytes([body[0], body[1]]));
                                let id = &body[2 + len..4 + len];
                                send(&mut sock, 0x40, &[id[0], id[1], 0]);
                            }
                            if let Some(code) = end {
                                thread::sleep(Duration::from_millis(200));
                                send(&mut sock, 0xE0, &[code, 0]);
                                // The DISCONNECT is read before the close.
                                thread::sleep(Duration::from_millis(300));
                                return;
                            }
                        }
                        Some(ControlPacketType::PingReq) if pings => send(&mut sock, 0xD0, &[]),
                        Some(ControlPacketType::Disconnect) => return,
                        _ => {}
                    }
                }
            });
        }
    });
    (addr, connects)
}

/// Spec §9.3 with §1 L1: a PINGRESP that does not come ends the connection
/// (the broker holds the socket open and answers nothing to the PINGREQ);
/// the daemon counts it under `network` and connects again.
#[test]
fn mqttbridge_9_3_a_missing_pingresp_ends_the_connection_and_the_daemon_connects_again() {
    let (addr, connects) = serial_broker(|n| (n > 0, None));
    let (mut cfg, _lease) = config(&format!("mqtt://{addr}"), "no-pingresp");
    cfg.keep_alive_secs = 1;
    cfg.reconnect.initial_delay_ms = 100;
    cfg.topics.push(topic("Np", "np/in", "in"));
    let h = server::start(cfg).expect("daemon start");
    wait_until("the second CONNECT", || connects.lock().unwrap().len() == 2);
    let seen = connects.lock().unwrap().clone();
    // The PINGREQ after 1 s, the PINGRESP awaited for half of it.
    assert!(
        seen[1] - seen[0] >= Duration::from_millis(1400),
        "{:?}",
        seen[1] - seen[0]
    );
    wait_until("the reconnect", || h.healthy.load(Ordering::SeqCst));
    let m = h.metrics.as_ref().unwrap();
    assert_eq!(
        m.value(
            zerodds_mqtt_bridge::daemon::metrics::family::BROKER_DISCONNECTS,
            &[("reason_code", "network")]
        ),
        1
    );
    thread::sleep(Duration::from_millis(2500));
    assert_eq!(
        connects.lock().unwrap().len(),
        2,
        "the second connection gets its PINGRESP and stands"
    );
    shutdown(h);
}

/// Spec §9.3: a connection the broker ends with DISCONNECT 0x8E Session
/// taken over is tried again only after `max_delay_ms`; one it ends with
/// another Reason Code at once.
#[test]
fn mqttbridge_9_3_a_session_taken_over_is_tried_again_after_the_longest_wait() {
    for (code, id, at_least, below) in [
        (
            0x8Eu8,
            "taken-over",
            Some(Duration::from_millis(1400)),
            None,
        ),
        (0x89, "server-busy", None, Some(Duration::from_millis(1000))),
    ] {
        let (addr, connects) = match code {
            0x8E => serial_broker(|n| (true, (n == 0).then_some(0x8E))),
            _ => serial_broker(|n| (true, (n == 0).then_some(0x89))),
        };
        let (mut cfg, _lease) = config(&format!("mqtt://{addr}"), id);
        cfg.reconnect.initial_delay_ms = 100;
        cfg.reconnect.max_delay_ms = 1500;
        cfg.topics.push(topic("To", "to/in", "in"));
        let h = server::start(cfg).expect("daemon start");
        wait_until("the second CONNECT", || connects.lock().unwrap().len() == 2);
        let seen = connects.lock().unwrap().clone();
        let gap = seen[1] - seen[0];
        if let Some(min) = at_least {
            assert!(gap >= min, "0x{code:02x}: {gap:?}");
        }
        if let Some(max) = below {
            assert!(gap < max, "0x{code:02x}: {gap:?}");
        }
        let m = h.metrics.as_ref().unwrap();
        assert_eq!(
            m.value(
                zerodds_mqtt_bridge::daemon::metrics::family::BROKER_DISCONNECTS,
                &[("reason_code", &format!("0x{code:02x}"))]
            ),
            1
        );
        shutdown(h);
    }
}

/// Spec §9.3 with §4.1, [MQTT-3.1.2-4], [MQTT-4.4.0-1]: with
/// `clean_start` false and a Session Expiry Interval, a lost connection
/// keeps the session. The broker of the test holds the PUBACK of the
/// daemon's QoS 1 PUBLISH and closes the socket without DISCONNECT; the
/// daemon (the same process, no restart) connects again with Clean Start
/// 0, and after Session Present 1 the PUBLISH goes out again with DUP 1,
/// its packet identifier, topic and payload. The broker's PUBACK of the
/// resend completes the flow in the session file.
#[test]
fn mqttbridge_9_3_after_a_lost_connection_the_session_resends_what_the_broker_did_not_acknowledge()
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<(usize, PublishPacket)>();
    let (clean_tx, clean_rx) = std::sync::mpsc::channel::<bool>();
    let broker = thread::spawn(move || {
        for n in 0..2usize {
            let (mut sock, _) = listener.accept().unwrap();
            sock.set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let (_, body) = read_packet(&mut sock).unwrap();
            let c = decode_connect_body(&body).unwrap();
            clean_tx.send(c.connect_flags & 0x02 != 0).unwrap();
            // Session Present 1 on the second connection.
            sock.write_all(&frame_packet(0x20, &[u8::from(n == 1), 0, 0]).unwrap())
                .unwrap();
            while let Ok((byte0, body)) = read_packet(&mut sock) {
                match ControlPacketType::from_bits(byte0 >> 4) {
                    Some(ControlPacketType::Publish) => {
                        let mut full = vec![byte0];
                        full.extend(encode_vbi(body.len() as u32).unwrap());
                        full.extend(&body);
                        let (_, p) = decode_publish(&full).unwrap();
                        // The catalog of §5.2 is acknowledged and is not the
                        // PUBLISH under test.
                        let catalog = p.topic.starts_with("$zerodds/");
                        if catalog || n == 1 {
                            let mut ack = p.packet_id.unwrap().to_be_bytes().to_vec();
                            ack.push(0);
                            let _ = sock.write_all(&frame_packet(0x40, &ack).unwrap());
                        }
                        if !catalog {
                            tx.send((n, p)).unwrap();
                            if n == 0 {
                                // No PUBACK: the socket closes, the
                                // connection is lost.
                                break;
                            }
                        }
                    }
                    Some(ControlPacketType::PingReq) => {
                        let _ = sock.write_all(&[0xD0, 0x00]);
                    }
                    Some(ControlPacketType::Disconnect) => break,
                    _ => {}
                }
            }
        }
    });
    let (mut cfg, _lease) = config(&format!("mqtt://{addr}"), "lost-session-cli");
    cfg.clean_start = false;
    cfg.session_expiry_interval_secs = 300;
    cfg.drain_timeout_secs = 1;
    cfg.reconnect.initial_delay_ms = 100;
    let mut t = topic("LostSession", "lost/out", "out");
    t.mqtt_qos = Some(1);
    cfg.topics.push(t);
    let file = zerodds_mqtt_bridge::daemon::session::session_file(&cfg);

    let h = server::start(cfg).expect("daemon start");
    assert!(
        h.runtime
            .test_inject_user_alive(h.user_readers["LostSession"], vec![7])
    );
    let (conn, sent) = rx.recv_timeout(TO).expect("the PUBLISH");
    assert_eq!((conn, sent.dup, sent.payload[4]), (0, false, 7));
    let (conn, again) = rx.recv_timeout(TO).expect("the PUBLISH again");
    assert_eq!(conn, 1, "on the second connection");
    assert!(again.dup, "DUP 1 on the resend");
    assert_eq!(again.packet_id, sent.packet_id);
    assert_eq!(
        (again.topic.as_str(), &again.payload[..]),
        ("lost/out", &sent.payload[..])
    );
    assert_eq!(
        clean_rx.try_iter().collect::<Vec<_>>(),
        [false, false],
        "Clean Start 0 on both CONNECT packets"
    );
    wait_until("the reconnect", || h.healthy.load(Ordering::SeqCst));
    // A record `C`, the identifier, length 0: the broker completed the
    // flow (the format of daemon/session.rs).
    let pid = again.packet_id.unwrap().to_be_bytes();
    wait_until("the completion in the session file", || {
        let bytes = std::fs::read(&file).unwrap_or_default();
        bytes
            .windows(7)
            .any(|w| w == [b'C', pid[0], pid[1], 0, 0, 0, 0])
    });
    shutdown(h);
    broker.join().unwrap();
}

// ---------------------------------------------------------------------
// §11: the packaging starts the binary
// ---------------------------------------------------------------------

/// A file of the repository.
fn repo_file(rel: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The text between each `open` and the next `close` in `text`.
fn between<'a>(text: &'a str, open: &str, close: &str) -> Vec<&'a str> {
    text.split(open)
        .skip(1)
        .filter_map(|rest| rest.split_once(close).map(|(inside, _)| inside))
        .collect()
}

/// Spec §11: the arguments with which the systemd unit, the launchd plist,
/// the Homebrew service, the Windows service script and the Docker image
/// start `zerodds-mqtt-bridged` are flags the binary knows: each argument
/// list, its configuration path replaced by the shipped example
/// configuration, passes `--check-config` with exit 0.
#[test]
fn mqttbridge_11_the_service_units_start_the_binary_with_flags_it_knows() {
    let dir = scratch("service-flags");
    let example = dir.join("mqtt-bridged.yaml");
    std::fs::write(
        &example,
        repo_file("packaging/linux/configs/mqtt-bridged.yaml.example"),
    )
    .unwrap();
    let mut launches: Vec<(&str, Vec<String>)> = Vec::new();

    let unit = repo_file("packaging/linux/systemd/zerodds-mqtt-bridged.service");
    let exec = unit
        .lines()
        .find_map(|l| l.strip_prefix("ExecStart="))
        .expect("ExecStart");
    let mut words = exec.split_whitespace();
    assert_eq!(words.next(), Some("/usr/bin/zerodds-mqtt-bridged"));
    launches.push(("systemd", words.map(str::to_string).collect()));

    let plist = repo_file("packaging/macos/launchd/org.zerodds.mqtt-bridged.plist");
    let array = between(&plist, "<key>ProgramArguments</key>", "</array>");
    let strings: Vec<String> = between(array[0], "<string>", "</string>")
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(strings[0], "/usr/local/bin/zerodds-mqtt-bridged");
    launches.push(("launchd", strings[1..].to_vec()));

    let formula = repo_file("packaging/macos/homebrew/zerodds-mqtt-bridge.rb");
    let run = between(&formula, "run [opt_bin/\"zerodds-mqtt-bridged\",", "]");
    // The quoted strings: every second piece between the quotes.
    let quoted: Vec<String> = run[0]
        .split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect();
    launches.push(("homebrew", quoted));

    let ps1 = repo_file("packaging/windows/services/Install-Services.ps1");
    assert!(
        ps1.contains("Name = 'ZeroDDSMQTTBridge';  Bin = 'zerodds-mqtt-bridged.exe'"),
        "the service name of the binary's run_as_service"
    );
    // `$binPath = "`"$exe`" --config `"$cfg`""`: a PowerShell string whose
    // words are the executable and its arguments; `` `" `` is a quote in it.
    let bin_path = ps1
        .lines()
        .find_map(|l| l.trim().strip_prefix("$binPath = "))
        .expect("binPath");
    let inner = bin_path
        .strip_prefix('"')
        .and_then(|b| b.strip_suffix('"'))
        .expect("a double-quoted string");
    let mut words = inner.split_whitespace().map(|w| w.replace("`\"", ""));
    assert_eq!(words.next().as_deref(), Some("$exe"), "{bin_path}");
    // Every argument after the executable, the configuration path as the
    // other launches have it.
    let argv: Vec<String> = words
        .map(|w| if w == "$cfg" { "CFG".to_string() } else { w })
        .collect();
    launches.push(("windows", argv));

    let dockerfile = repo_file("packaging/docker/mqtt-bridged/Dockerfile");
    let cmd = dockerfile
        .lines()
        .find_map(|l| l.strip_prefix("CMD "))
        .expect("CMD");
    let cmd: Vec<String> = serde_json::from_str(cmd).unwrap();
    assert!(
        dockerfile.contains(
            "COPY packaging/linux/configs/mqtt-bridged.yaml.example /etc/zerodds/mqtt-bridged.yaml"
        ),
        "the image's configuration is the example"
    );
    launches.push(("docker", cmd));

    let lease = zerodds_test_domains::lease_domain();
    for (what, args) in launches {
        assert_eq!(args.first().map(String::as_str), Some("--config"), "{what}");
        let mut run: Vec<String> = args.clone();
        run[1] = example.display().to_string();
        run.extend(["--domain".into(), lease.id().to_string()]);
        run.push("--check-config".into());
        let out = Command::new(BIN).args(&run).output().unwrap();
        assert_eq!(
            out.status.code(),
            Some(0),
            "{what} {args:?}: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// Spec §11 with zerodds-deployment-1.0: the systemd unit is
/// `Type=notify`; the binary sends `READY=1` to `$NOTIFY_SOCKET` once it
/// serves and `STOPPING=1` when SIGTERM begins the shutdown.
#[cfg(target_os = "linux")]
#[test]
fn mqttbridge_11_the_binary_tells_the_service_manager_ready_and_stopping() {
    use std::os::unix::net::UnixDatagram;
    let unit = repo_file("packaging/linux/systemd/zerodds-mqtt-bridged.service");
    assert!(unit.contains("\nType=notify\n"), "{unit}");
    let dir = scratch("notify");
    let lease = zerodds_test_domains::lease_domain();
    let socket_path = dir.join("notify.sock");
    let socket = UnixDatagram::bind(&socket_path).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_millis(200)))
        .unwrap();
    let b = broker_server(ServerConfig::default());
    let url = format!("mqtt://{}", b.local_addr());
    let mut child = Daemon(
        Command::new(BIN)
            .args(args(
                &dir,
                lease.id(),
                &["--broker", &url, "--client-id", "notify-cli"],
            ))
            .env("NOTIFY_SOCKET", &socket_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let next = |socket: &UnixDatagram| -> String {
        let deadline = Instant::now() + TO;
        let mut buf = [0u8; 256];
        loop {
            if let Ok(n) = socket.recv(&mut buf) {
                return String::from_utf8_lossy(&buf[..n]).to_string();
            }
            assert!(Instant::now() < deadline, "no notification");
        }
    };
    assert_eq!(next(&socket), "READY=1");
    assert!(
        b.connected_clients().contains(&"notify-cli".to_string()),
        "READY=1 once the broker connection stands"
    );
    signal(&child, "TERM");
    assert_eq!(next(&socket), "STOPPING=1");
    assert_eq!(exit_code(&mut child), Some(0));
    b.shutdown();
}

// ---------------------------------------------------------------------
// §8.3: the OTLP spans of the binary
// ---------------------------------------------------------------------

/// A nanosecond field of an OTLP/JSON span.
fn span_ns(span: &serde_json::Value, key: &str) -> u64 {
    span[key].as_str().unwrap().parse().unwrap()
}

/// One run of the binary for §8.3 against a broker of the test that holds
/// the PUBACK of the daemon's QoS 1 PUBLISH for 1.5 s: a DDS sample out, an
/// MQTT message in, then SIGTERM and the exit.
fn otlp_run(dir_leaf: &str, client_id: &str, endpoint: Option<&str>) {
    let dir = scratch(dir_leaf);
    let lease = zerodds_test_domains::lease_domain();
    let rec = Recorder::start(PropertySet::default());
    rec.hold_acks();
    let file = dir.join("mqtt-bridged.yaml");
    std::fs::write(
        &file,
        format!(
            "mqtt:\n  broker_url: {}\n  client_id: {client_id}\n  session_dir: {}\ntopics:\n  - dds_name: OtIn\n    mqtt_topic: ot/in\n    direction: in\n  - dds_name: OtOut\n    mqtt_topic: ot/out\n    direction: out\n",
            rec.url(),
            dir.join("sessions").display()
        ),
    )
    .unwrap();
    let mut command = Command::new(BIN);
    command
        .args(["--config", &file.display().to_string()])
        .args(["--domain", &lease.id().to_string()])
        .env_remove("OTEL_EXPORTER_OTLP_ENDPOINT")
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Some(endpoint) = endpoint {
        command.env("OTEL_EXPORTER_OTLP_ENDPOINT", endpoint);
    }
    let mut child = Daemon(command.spawn().unwrap());
    let log = Log::of(child.0.stdout.take().unwrap());
    log.wait_for("running");
    assert_eq!(
        log.events("otlp_active").len(),
        usize::from(endpoint.is_some()),
        "{}",
        log.text()
    );
    rec.wait_subscribed();
    let dds = foreign_participant(lease.id(), 0x83);
    let w = dds
        .register_user_writer(UserWriterConfig {
            topic_name: "OtOut".into(),
            type_name: "OtOut".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap();
    wait_until("the bridge's reader over RTPS", || {
        !dds.user_writer_matched_reader_guids(w).is_empty()
    });
    dds.write_user_sample(w, vec![1, 2, 3]).unwrap();
    wait_until("the PUBLISH on ot/out", || {
        !rec.markers("ot/out").is_empty()
    });
    thread::sleep(Duration::from_millis(1500));
    rec.release_acks();
    rec.inject(
        "ot/in",
        &[0x00, 0x01, 0x00, 0x00, 9],
        &PropertySet::default(),
    );
    thread::sleep(Duration::from_millis(500));
    signal(&child, "TERM");
    assert_eq!(exit_code(&mut child), Some(0));
}

/// Spec §8.3: with `OTEL_EXPORTER_OTLP_ENDPOINT` the binary sends a span
/// per PUBLISH roundtrip to the collector: the daemon's QoS 1 PUBLISH is an
/// `mqtt.publish` of kind client with the topic, the QoS, the packet
/// identifier and the DDS topic, which lasts until the broker's PUBACK
/// (held 1.5 s); the broker's PUBLISH is an `mqtt.publish` of kind server
/// with the child `dds.write` in its trace. Without the variable the same
/// run sends nothing.
#[test]
fn mqttbridge_8_3_the_binary_sends_a_span_per_publish_roundtrip_only_with_the_variable() {
    let (endpoint, spans) = otlp_collector();
    otlp_run("otlp-off", "otlp-off", None);
    thread::sleep(Duration::from_millis(300));
    assert!(
        spans.lock().unwrap().is_empty(),
        "spans without the variable"
    );

    otlp_run("otlp-on", "otlp-on", Some(&endpoint));
    wait_until("the spans at the collector", || {
        spans.lock().unwrap().len() >= 3
    });
    let spans = spans.lock().unwrap().clone();
    let named = |name: &str, kind: u64| -> Vec<serde_json::Value> {
        spans
            .iter()
            .filter(|s| s["name"] == name && s["kind"] == kind)
            .cloned()
            .collect()
    };
    let out = named("mqtt.publish", 3);
    let out: Vec<_> = out
        .iter()
        .filter(|s| span_attribute(s, "mqtt.topic") == Some("ot/out"))
        .collect();
    assert_eq!(out.len(), 1, "{spans:?}");
    let out = out[0];
    assert!(out.get("parentSpanId").is_none(), "{out}");
    assert_eq!(span_attribute(out, "dds.topic"), Some("OtOut"));
    assert_eq!(span_attribute(out, "mqtt.qos"), Some("1"));
    assert!(span_attribute(out, "mqtt.packet_id").is_some(), "{out}");
    assert_eq!(out["status"]["code"], 1, "{out}");
    let took = span_ns(out, "endTimeUnixNano") - span_ns(out, "startTimeUnixNano");
    assert!(
        took >= 1_400_000_000,
        "the span ends with the PUBACK, held 1.5 s: {took} ns"
    );

    let inbound: Vec<_> = named("mqtt.publish", 2)
        .into_iter()
        .filter(|s| span_attribute(s, "mqtt.topic") == Some("ot/in"))
        .collect();
    assert_eq!(inbound.len(), 1, "{spans:?}");
    let root = &inbound[0];
    assert_eq!(span_attribute(root, "mqtt.message.bytes"), Some("5"));
    let writes: Vec<_> = named("dds.write", 3)
        .into_iter()
        .filter(|s| s["parentSpanId"] == root["spanId"])
        .collect();
    assert_eq!(writes.len(), 1, "{spans:?}");
    assert_eq!(writes[0]["traceId"], root["traceId"]);
    assert_eq!(span_attribute(&writes[0], "dds.topic"), Some("OtIn"));
    assert!(span_ns(&writes[0], "startTimeUnixNano") >= span_ns(root, "startTimeUnixNano"));
    assert!(span_ns(&writes[0], "endTimeUnixNano") <= span_ns(root, "endTimeUnixNano"));
}

/// Spec §8.3 with §6 and §9.3: a PUBLISH of the replay buffer after a
/// reconnect is a roundtrip of its DDS topic. The broker restarts without
/// its retained messages; the daemon publishes the transient topic's sample
/// again, and the collector gets a second `mqtt.publish` of kind client of
/// that topic and DDS topic, begun after the new broker came up and ended
/// with its PUBACK (status ok).
#[test]
fn mqttbridge_8_3_a_replayed_publish_is_a_span_of_its_topic() {
    let (endpoint, spans) = otlp_collector();
    let dir = scratch("otlp-replay");
    let lease = zerodds_test_domains::lease_domain();
    let b1 = broker_server(ServerConfig::default());
    let port = b1.local_addr().port();
    let file = dir.join("mqtt-bridged.yaml");
    std::fs::write(
        &file,
        format!(
            "mqtt:\n  broker_url: mqtt://127.0.0.1:{port}\n  client_id: otlp-replay\n  session_dir: {}\n  reconnect:\n    initial_delay_ms: 100\n    max_delay_ms: 300\ntopics:\n  - dds_name: OtRe\n    mqtt_topic: ot/replay\n    direction: out\n    qos:\n      reliability: reliable\n      durability: transient\n",
            dir.join("sessions").display()
        ),
    )
    .unwrap();
    let mut child = Daemon(
        Command::new(BIN)
            .args(["--config", &file.display().to_string()])
            .args(["--domain", &lease.id().to_string()])
            .env("OTEL_EXPORTER_OTLP_ENDPOINT", &endpoint)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let log = Log::of(child.0.stdout.take().unwrap());
    log.wait_for("running");
    let mut sub =
        TestClient::connect(b1.local_addr(), "otlp-replay-sub", ProtocolVersion::V5).unwrap();
    sub.subscribe("ot/replay", 1).unwrap();
    let dds = foreign_participant(lease.id(), 0x84);
    // The bridge's reader asks for TRANSIENT; the writer offers it.
    let w = dds
        .register_user_writer(UserWriterConfig {
            topic_name: "OtRe".into(),
            type_name: "OtRe".into(),
            reliable: true,
            durability: zerodds_qos::DurabilityKind::Transient,
            ..Default::default()
        })
        .unwrap();
    wait_until("the bridge's reader over RTPS", || {
        !dds.user_writer_matched_reader_guids(w).is_empty()
    });
    dds.write_user_sample(w, vec![1, 2, 3]).unwrap();
    let (topic, _) = sub
        .recv_publish(TO)
        .expect("the PUBLISH on the first broker");
    assert_eq!(topic, "ot/replay");
    b1.shutdown();
    log.wait_for("disconnect");
    let back = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    let b2 = broker_on(port);
    let replay = log.wait_for("replay");
    assert_eq!(replay["sent"], 1, "{replay}");
    // The flush at the end of the shutdown sends what is left.
    signal(&child, "TERM");
    assert_eq!(exit_code(&mut child), Some(0));
    b2.shutdown();
    let of_topic = || -> Vec<serde_json::Value> {
        spans
            .lock()
            .unwrap()
            .iter()
            .filter(|s| {
                s["name"] == "mqtt.publish"
                    && s["kind"] == 3
                    && span_attribute(s, "mqtt.topic") == Some("ot/replay")
            })
            .cloned()
            .collect()
    };
    wait_until("the two spans of the topic", || of_topic().len() >= 2);
    let mut both = of_topic();
    assert_eq!(both.len(), 2, "{both:?}");
    both.sort_by_key(|s| span_ns(s, "startTimeUnixNano"));
    assert!(span_ns(&both[0], "startTimeUnixNano") < back, "{both:?}");
    let replayed = &both[1];
    assert!(
        span_ns(replayed, "startTimeUnixNano") >= back,
        "the replay began after the new broker came up: {replayed}"
    );
    assert!(replayed.get("parentSpanId").is_none(), "{replayed}");
    assert_eq!(span_attribute(replayed, "dds.topic"), Some("OtRe"));
    assert_eq!(span_attribute(replayed, "mqtt.qos"), Some("1"));
    assert!(
        span_attribute(replayed, "mqtt.packet_id").is_some(),
        "{replayed}"
    );
    assert_eq!(replayed["status"]["code"], 1, "{replayed}");
}
