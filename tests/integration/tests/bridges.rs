// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The protocol bridges as daemons, each between a ZeroDDS participant made
//! through the public DCPS API and a broker or client this repository did
//! not write.
//!
//! Every scenario starts the bridge daemon with a configuration in the
//! format its specification gives (`docs/specs/zerodds-<x>-bridge-1.0.md`
//! §3) and moves `Reading` samples (`dds::Reading`, the XCDR2 bytes of which
//! DDS-XTypes 1.3 §7.4.3.5 and §7.6.3.1.2 state) both ways where the bridge
//! has both ways:
//!
//! * **DDS → foreign**: the foreign side reports the bytes it received, and
//!   they are the serialized payload the bridge specification names
//!   (encapsulation header and CDR body, pass-through), wrapped the way the
//!   foreign protocol's specification wraps a message.
//! * **foreign → DDS**: the foreign side sends the specification's bytes,
//!   and the DDS reader receives exactly those samples, in order.
//!
//! The foreign implementations: Eclipse Mosquitto (MQTT 5), libcoap
//! (CoAP, RFC 7252), the Python `websockets` library (RFC 6455), grpcio
//! (gRPC over HTTP/2), RabbitMQ 4 with pika (AMQP 1.0 and 0-9-1).

#![cfg(target_os = "linux")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    missing_docs
)]

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use zerodds_dcps::{DataReader, DataReaderQos, DataWriter, PublisherQos, SubscriberQos, TopicQos};
use zerodds_integration::dds::{Reading, participant, reliable_reader, reliable_writer};
use zerodds_integration::{Peer, Scratch, free_port, lease_domain, tools};

const STEP: Duration = Duration::from_secs(10);
/// Samples per direction.
const SAMPLES: u32 = 5;
const OUT: &str = "BridgeOut";
const IN: &str = "BridgeIn";

fn peers_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("peers")
}

/// The DDS side: a writer of [`OUT`] and a reader of [`IN`].
struct DdsSide {
    _participant: zerodds_dcps::DomainParticipant,
    writer: DataWriter<Reading>,
    reader: DataReader<Reading>,
}

impl DdsSide {
    fn new(domain: i32) -> Self {
        let p = participant(domain);
        let out = p.create_topic::<Reading>(OUT, TopicQos::default()).unwrap();
        let inn = p.create_topic::<Reading>(IN, TopicQos::default()).unwrap();
        let writer = p
            .create_publisher(PublisherQos::default())
            .create_datawriter::<Reading>(&out, reliable_writer())
            .unwrap();
        let reader = p
            .create_subscriber(SubscriberQos::default())
            .create_datareader::<Reading>(&inn, reliable_reader())
            .unwrap();
        Self {
            _participant: p,
            writer,
            reader,
        }
    }

    /// Waits until the bridge's reader of [`OUT`] and writer of [`IN`]
    /// match ours.
    fn wait_for_bridge(&self, bridge: &str) {
        self.wait_for_bridge_reader(bridge);
        self.wait_for_bridge_writer(bridge);
    }

    /// Waits until a bridge reader of [`OUT`] matches our writer.
    fn wait_for_bridge_reader(&self, bridge: &str) {
        self.writer
            .wait_for_matched_subscription(1, STEP)
            .unwrap_or_else(|e| panic!("{bridge}: no bridge reader of {OUT}: {e:?}"));
    }

    /// Waits until the bridge's writer of [`IN`] matches our reader.
    fn wait_for_bridge_writer(&self, bridge: &str) {
        self.reader
            .wait_for_matched_publication(1, STEP)
            .unwrap_or_else(|e| panic!("{bridge}: no bridge writer of {IN}: {e:?}"));
    }

    fn write_out(&self) -> Vec<Reading> {
        let samples: Vec<Reading> = (0..SAMPLES).map(|seq| Reading { id: 1, seq }).collect();
        for s in &samples {
            self.writer.write(s).unwrap();
        }
        samples
    }

    /// Takes until `count` samples of [`IN`] arrived.
    fn take_in(&self, bridge: &str, count: usize) -> Vec<Reading> {
        let deadline = std::time::Instant::now() + STEP;
        let mut got = Vec::new();
        while got.len() < count {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            self.reader.wait_for_data(left).unwrap_or_else(|_| {
                panic!(
                    "{bridge}: {} of {count} samples of {IN} reached DDS",
                    got.len()
                )
            });
            got.extend(
                self.reader
                    .take_with_info()
                    .unwrap()
                    .into_iter()
                    .filter(|s| s.info.valid_data)
                    .map(|s| s.data),
            );
        }
        got
    }
}

/// What the foreign side sends towards DDS.
fn inbound() -> Vec<Reading> {
    (0..SAMPLES)
        .map(|seq| Reading {
            id: 2,
            seq: 100 + seq,
        })
        .collect()
}

fn daemon(variable: &str, label: &str, config: &Path, ready: &str) -> Peer {
    let mut c = zerodds_integration::daemon_command(variable);
    c.arg("--config").arg(config);
    let mut peer = Peer::spawn(label, c);
    peer.wait_for(&format!("the line `{ready}`"), STEP, |l| {
        l.text.contains(ready)
    });
    peer
}

fn assert_received(bridge: &str, what: &str, got: &[String], expected: &[Reading]) {
    let want: Vec<String> = expected.iter().map(Reading::xcdr2_hex).collect();
    assert_eq!(
        got, want,
        "{bridge}: {what} carries each sample's serialized payload unchanged \
         (encapsulation header 00 07 00 00 and the XCDR2 body), in order"
    );
}

// ---------------------------------------------------------------------
// MQTT: Mosquitto broker, mosquitto_sub and mosquitto_pub
// ---------------------------------------------------------------------

#[test]
fn mqtt_bridge_against_mosquitto() {
    let scratch = Scratch::new("mqtt");
    let dir = scratch.path();
    let lease = lease_domain();
    let domain = lease.id();
    let port = free_port();

    let conf = dir.join("mosquitto.conf");
    std::fs::write(
        &conf,
        format!("listener {port} 127.0.0.1\nallow_anonymous true\n"),
    )
    .unwrap();
    let mut broker = Command::new(tools::require("mosquitto", &["/usr/sbin"]));
    broker.arg("-c").arg(&conf).arg("-v");
    let mut broker = Peer::spawn("mosquitto", broker);
    broker.wait_for("mosquitto running", STEP, |l| l.text.contains("running"));

    let config = dir.join("mqtt-bridged.yaml");
    std::fs::write(
        &config,
        format!(
            r#"domain: {domain}
log_level: info
mqtt:
  broker_url: "mqtt://127.0.0.1:{port}"
  client_id: "zerodds-it-bridge"
  keep_alive_secs: 30
  clean_start: true
topics:
  - dds_name: "{OUT}"
    dds_type: "{ty}"
    mqtt_topic: "it/out"
    direction: "out"
    mqtt_qos: 1
    qos:
      reliability: "reliable"
  - dds_name: "{IN}"
    dds_type: "{ty}"
    mqtt_topic: "it/in"
    direction: "in"
    mqtt_qos: 1
    qos:
      reliability: "reliable"
"#,
            ty = Reading::NAME
        ),
    )
    .unwrap();
    let dds = DdsSide::new(domain);
    let bridge = daemon(
        "ZERODDS_MQTT_BRIDGED",
        "mqtt-bridged",
        &config,
        "subscribed to 1 mqtt topic",
    );
    dds.wait_for_bridge("mqtt");

    // DDS -> MQTT: mosquitto_sub prints each payload as hex (`-F %x`).
    let mut sub = Command::new(tools::require("mosquitto_sub", &[]));
    sub.args([
        "-h",
        "127.0.0.1",
        "-p",
        &port.to_string(),
        "-V",
        "mqttv5",
        "-q",
        "1",
    ])
    .args([
        "-t",
        "it/out",
        "-i",
        "it-sub",
        "-F",
        "%x",
        "-C",
        &SAMPLES.to_string(),
    ]);
    let sub = Peer::spawn("mosquitto_sub", sub);
    broker.wait_for("mosquitto_sub's SUBSCRIBE", STEP, |l| {
        l.text.contains("Received SUBSCRIBE from it-sub")
    });
    let written = dds.write_out();
    let (status, lines) = sub.wait_exit_lines(STEP);
    assert!(status.success(), "mosquitto_sub failed");
    let got: Vec<String> = lines
        .iter()
        .filter(|l| l.stream == zerodds_integration::Stream::Out)
        .map(|l| l.text.trim().to_owned())
        .collect();
    assert_received(
        "mqtt",
        "every PUBLISH on it/out (zerodds-mqtt-bridge-1.0 §4.2)",
        &got,
        &written,
    );

    // MQTT -> DDS: mosquitto_pub sends the specification's bytes.
    let sent = inbound();
    for (i, sample) in sent.iter().enumerate() {
        let file = dir.join(format!("in-{i}.bin"));
        std::fs::write(&file, sample.xcdr2()).unwrap();
        let mut publish = Command::new(tools::require("mosquitto_pub", &[]));
        publish
            .args([
                "-h",
                "127.0.0.1",
                "-p",
                &port.to_string(),
                "-V",
                "mqttv5",
                "-q",
                "1",
            ])
            .args(["-t", "it/in", "-f"])
            .arg(&file);
        let status = Peer::spawn("mosquitto_pub", publish).wait_exit(STEP);
        assert!(status.success(), "mosquitto_pub failed");
    }
    assert_eq!(
        dds.take_in("mqtt", sent.len()),
        sent,
        "mqtt: every PUBLISH on it/in reaches DDS, in order"
    );

    let _ = bridge.stop();
    let _ = broker.stop();
    drop(lease);
}

// ---------------------------------------------------------------------
// CoAP: libcoap's client
// ---------------------------------------------------------------------

#[test]
fn coap_bridge_against_libcoap() {
    let scratch = Scratch::new("coap");
    let dir = scratch.path();
    let lease = lease_domain();
    let domain = lease.id();
    let port = free_port();
    let config = dir.join("coap-bridged.yaml");
    std::fs::write(
        &config,
        format!(
            r#"domain: {domain}
log_level: info
coap:
  bind: "127.0.0.1:{port}"
auth:
  mode: "none"
acl:
  default_deny: false
topics:
  - dds_name: "{IN}"
    dds_type: "{ty}"
    coap_uri_path: "it/in"
    direction: "in"
    qos:
      reliability: "reliable"
  - dds_name: "{OUT}"
    dds_type: "{ty}"
    coap_uri_path: "it/out"
    direction: "out"
    qos:
      reliability: "reliable"
"#,
            ty = Reading::NAME
        ),
    )
    .unwrap();
    let dds = DdsSide::new(domain);
    let bridge = daemon("ZERODDS_COAP_BRIDGED", "coap-bridged", &config, "bound on");
    dds.wait_for_bridge("coap");

    // CoAP -> DDS: POST with Content-Format 65000, XCDR2 little endian: the
    // content format names the representation, the payload is the CDR body
    // (zerodds-coap-bridge-1.0 §4.1, §4.2, §4.5); libcoap reports 2.04 Changed.
    let client = tools::find("coap-client-notls", &[])
        .or_else(|| tools::find("coap-client-openssl", &[]))
        .or_else(|| tools::find("coap-client-gnutls", &[]))
        .unwrap_or_else(|| tools::require("coap-client", &[]));
    let sent = inbound();
    for (i, sample) in sent.iter().enumerate() {
        let file = dir.join(format!("in-{i}.bin"));
        std::fs::write(&file, sample.xcdr2_body()).unwrap();
        let mut post = Command::new(&client);
        post.args(["-m", "post", "-t", "65000", "-v", "6", "-f"])
            .arg(&file)
            .arg(format!("coap://127.0.0.1:{port}/it/in"));
        let (status, lines) = Peer::spawn("coap-client", post).wait_exit_lines(STEP);
        assert!(status.success(), "coap-client failed");
        assert!(
            lines.iter().any(|l| l.text.contains("2.04")),
            "coap: the POST is answered 2.04 Changed (RFC 7252 §5.9.1.4): {:?}",
            lines.iter().map(|l| &l.text).collect::<Vec<_>>()
        );
    }
    assert_eq!(
        dds.take_in("coap", sent.len()),
        sent,
        "coap: every POST reaches DDS, in order"
    );

    let _ = bridge.stop();
    drop(lease);
}

// ---------------------------------------------------------------------
// WebSocket: the Python `websockets` client
// ---------------------------------------------------------------------

/// A binary frame of zerodds-ws-bridge-1.0 §4.3: `ZDB1`, a flags octet,
/// the serialized payload.
fn ws_frame(sample: &Reading) -> String {
    format!(
        "{}00{}",
        zerodds_integration::hex(b"ZDB1"),
        sample.xcdr2_hex()
    )
}

#[test]
fn websocket_bridge_against_python_websockets() {
    let scratch = Scratch::new("ws");
    let dir = scratch.path();
    let lease = lease_domain();
    let domain = lease.id();
    let port = free_port();
    let config = dir.join("ws-bridged.yaml");
    std::fs::write(
        &config,
        format!(
            r#"listen: "127.0.0.1:{port}"
domain: {domain}
log_level: info
auth:
  mode: "none"
topics:
  - name: "{OUT}"
    type: "{ty}"
    direction: "out"
    ws_path: "/it/out"
    qos:
      reliability: "reliable"
  - name: "{IN}"
    type: "{ty}"
    direction: "in"
    ws_path: "/it/in"
    qos:
      reliability: "reliable"
"#,
            ty = Reading::NAME
        ),
    )
    .unwrap();
    let dds = DdsSide::new(domain);
    let bridge = daemon("ZERODDS_WS_BRIDGED", "ws-bridged", &config, "bound on");
    dds.wait_for_bridge("websocket");

    let sent = inbound();
    let mut client = Command::new(tools::require("python3", &[]));
    client
        .arg(peers_dir().join("ws_peer.py"))
        .arg(format!("ws://127.0.0.1:{port}/it/out"))
        .arg(SAMPLES.to_string())
        .arg(format!("ws://127.0.0.1:{port}/it/in"))
        .args(sent.iter().map(ws_frame));
    let mut client = Peer::spawn("websockets", client);
    client.wait_for("connected", STEP, |l| l.text == "connected");
    let written = dds.write_out();
    client.wait_for("sent", STEP, |l| l.text == "sent");
    let got: Vec<String> = client
        .lines()
        .iter()
        .filter_map(|l| l.text.strip_prefix("frame "))
        .map(str::to_owned)
        .collect();
    let want: Vec<String> = written.iter().map(ws_frame).collect();
    assert_eq!(
        got, want,
        "websocket: one binary frame per sample, `ZDB1`, flags 0, the serialized payload \
         (zerodds-ws-bridge-1.0 §4.3)"
    );
    assert_eq!(
        dds.take_in("websocket", sent.len()),
        sent,
        "websocket: every frame reaches DDS, in order"
    );
    assert!(
        client.release(STEP).success(),
        "the websockets client failed"
    );
    let _ = bridge.stop();
    drop(lease);
}

/// The JSON text frames of zerodds-ws-bridge-1.0 §4.3, to a client that asks
/// for them with the `+json` subprotocol: the daemon reads the topics' type
/// from their `idl_file` (§3). DDS to the client: a `publish` frame per
/// sample with the sample in `data` and the instance's `key_hash`, the
/// holder of `@key unsigned long id` padded to 16 octets (DDSI-RTPS 2.5
/// §9.6.4.8), then a `dispose` frame with the key hash when the writer
/// disposes the instance. The client to DDS: `publish` frames with `data`,
/// which reach the DDS reader as the samples they name.
#[test]
fn websocket_bridge_json_text_frames_against_python_websockets() {
    let scratch = Scratch::new("ws-json");
    let dir = scratch.path();
    let lease = lease_domain();
    let domain = lease.id();
    let port = free_port();
    let idl = dir.join("reading.idl");
    std::fs::write(
        &idl,
        "module integration { @final struct Reading { @key unsigned long id; unsigned long seq; }; };\n",
    )
    .unwrap();
    let config = dir.join("ws-bridged.yaml");
    std::fs::write(
        &config,
        format!(
            r#"listen: "127.0.0.1:{port}"
domain: {domain}
log_level: info
auth:
  mode: "none"
topics:
  - name: "{OUT}"
    type: "{ty}"
    idl_file: "{idl}"
    direction: "out"
    ws_path: "/it/out"
    qos:
      reliability: "reliable"
  - name: "{IN}"
    type: "{ty}"
    idl_file: "{idl}"
    direction: "in"
    ws_path: "/it/in"
    qos:
      reliability: "reliable"
"#,
            ty = Reading::NAME,
            idl = idl.display()
        ),
    )
    .unwrap();
    let dds = DdsSide::new(domain);
    let bridge = daemon("ZERODDS_WS_BRIDGED", "ws-bridged", &config, "bound on");
    dds.wait_for_bridge("websocket json");

    let sent = inbound();
    let mut client = Command::new(tools::require("python3", &[]));
    client
        .arg(peers_dir().join("ws_json_peer.py"))
        .arg(format!("ws://127.0.0.1:{port}/it/out"))
        .arg((SAMPLES + 1).to_string())
        .arg(format!("ws://127.0.0.1:{port}/it/in"))
        .args(sent.iter().map(|r| {
            format!(
                r#"{{"topic":"{IN}","op":"publish","data":{{"id":{},"seq":{}}}}}"#,
                r.id, r.seq
            )
        }));
    let mut client = Peer::spawn("websockets json", client);
    client.wait_for("connected", STEP, |l| l.text == "connected");
    let written = dds.write_out();
    dds.writer
        .dispose(&written[0], zerodds_dcps::HANDLE_NIL)
        .unwrap();
    client.wait_for("sent", STEP, |l| l.text == "sent");
    let lines: Vec<String> = client.lines().iter().map(|l| l.text.clone()).collect();
    assert!(
        lines
            .iter()
            .any(|l| l == "subprotocols zerodds-ws-bridge-1.0+json zerodds-ws-bridge-1.0+json"),
        "websocket json: the daemon names the subprotocol back (§4.1): {lines:?}"
    );
    let got: Vec<&str> = lines
        .iter()
        .filter_map(|l| l.strip_prefix("text "))
        .collect();
    let mut want: Vec<String> = written
        .iter()
        .map(|r| {
            format!(
                r#"{{"data":{{"id":{},"seq":{}}},"key_hash":"{:08x}000000000000000000000000","op":"publish","timestamp_ns":"int","topic":"{OUT}"}}"#,
                r.id, r.seq, r.id
            )
        })
        .collect();
    want.push(format!(
        r#"{{"key_hash":"{:08x}000000000000000000000000","op":"dispose","timestamp_ns":"int","topic":"{OUT}"}}"#,
        written[0].id
    ));
    assert_eq!(
        got, want,
        "websocket json: one publish frame per sample, the sample in data, its key hash and \
         source time, then the dispose (zerodds-ws-bridge-1.0 §4.3)"
    );
    assert_eq!(
        dds.take_in("websocket json", sent.len()),
        sent,
        "websocket json: every publish frame reaches DDS as its sample, in order"
    );
    assert!(
        client.release(STEP).success(),
        "the websockets client failed"
    );
    let _ = bridge.stop();
    drop(lease);
}

// ---------------------------------------------------------------------
// gRPC: grpcio
// ---------------------------------------------------------------------

#[test]
fn grpc_bridge_against_grpcio() {
    let scratch = Scratch::new("grpc");
    let dir = scratch.path();
    let lease = lease_domain();
    let domain = lease.id();
    let port = free_port();
    let config = dir.join("grpc-bridged.yaml");
    std::fs::write(
        &config,
        format!(
            r#"domain: {domain}
log_level: info
grpc:
  bind: "127.0.0.1:{port}"
  tls:
    enabled: false
  reflection:
    enabled: false
auth:
  mode: "none"
topics:
  - dds_name: "{OUT}"
    dds_type: "{ty}"
    grpc_service: "BridgeOutStream"
    grpc_package: "zerodds.it.v1"
    direction: "out"
    qos:
      reliability: "reliable"
  - dds_name: "{IN}"
    dds_type: "{ty}"
    grpc_service: "BridgeInStream"
    grpc_package: "zerodds.it.v1"
    direction: "in"
    qos:
      reliability: "reliable"
"#,
            ty = Reading::NAME
        ),
    )
    .unwrap();
    let dds = DdsSide::new(domain);
    let bridge = daemon(
        "ZERODDS_GRPC_BRIDGED",
        "grpc-bridged",
        &config,
        "\"event\":\"listening\"",
    );
    dds.wait_for_bridge_writer("grpc");

    // grpcio subscribes to BridgeOut (a server stream, zerodds-grpc-bridge-1.0
    // §4.3: the daemon registers a DataReader for it) and then publishes to
    // BridgeIn; once that reader matches ours, the samples DDS writes reach
    // the stream.
    let sent = inbound();
    let mut client = Command::new(tools::require("python3", &[]));
    client
        .arg(peers_dir().join("grpc_peer.py"))
        .arg(format!("127.0.0.1:{port}"))
        .arg("zerodds.it.v1.BridgeOutStream")
        .arg(SAMPLES.to_string())
        .arg("zerodds.it.v1.BridgeInStream")
        .args(sent.iter().map(Reading::xcdr2_hex));
    let mut client = Peer::spawn("grpcio", client);
    client.wait_for("subscribing", STEP, |l| l.text == "subscribing");
    dds.wait_for_bridge_reader("grpc");
    let written = dds.write_out();
    for _ in 0..SAMPLES {
        client.wait_for("ack", STEP, |l| l.text == "ack");
    }
    let got: Vec<String> = client
        .lines()
        .iter()
        .filter_map(|l| l.text.strip_prefix("sample "))
        .map(str::to_owned)
        .collect();
    assert_received(
        "grpc",
        "Sample.cdr_payload of each streamed message (§4.2, §4.4)",
        &got,
        &written,
    );
    assert_eq!(
        dds.take_in("grpc", sent.len()),
        sent,
        "grpc: every PublishOne reaches DDS, in order"
    );
    assert!(client.release(STEP).success(), "the grpcio client failed");
    let _ = bridge.stop();
    drop(lease);
}

// ---------------------------------------------------------------------
// AMQP: RabbitMQ 4 (AMQP 1.0 towards the bridge), pika (AMQP 0-9-1)
// ---------------------------------------------------------------------

/// A RabbitMQ node of this test alone: its own ports, its own Erlang port
/// mapper, its own data directory. A RabbitMQ service already on the host
/// is not touched.
struct Rabbit {
    peer: Option<Peer>,
    port: u16,
    env: Vec<(String, String)>,
}

impl Rabbit {
    fn start(dir: &Path) -> Self {
        // The server itself, not Debian's /usr/sbin wrapper, which switches to
        // the `rabbitmq` user and then cannot write this test's directory.
        let server = tools::find("rabbitmq-server", &[])
            .filter(|p| !p.starts_with("/usr/sbin"))
            .or_else(|| {
                let direct = std::path::PathBuf::from("/usr/lib/rabbitmq/bin/rabbitmq-server");
                direct.is_file().then_some(direct)
            })
            .unwrap_or_else(|| tools::require("rabbitmq-server", &["/usr/lib/rabbitmq/bin"]));
        let port = free_port();
        let node = format!("zdit{}@localhost", std::process::id());
        let base = dir.join("rabbitmq");
        std::fs::create_dir_all(&base).unwrap();
        let env = vec![
            ("RABBITMQ_NODENAME".to_owned(), node),
            ("RABBITMQ_NODE_PORT".to_owned(), port.to_string()),
            ("RABBITMQ_DIST_PORT".to_owned(), free_port().to_string()),
            ("ERL_EPMD_PORT".to_owned(), free_port().to_string()),
            (
                "RABBITMQ_MNESIA_BASE".to_owned(),
                base.join("mnesia").display().to_string(),
            ),
            (
                "RABBITMQ_LOG_BASE".to_owned(),
                base.join("log").display().to_string(),
            ),
            ("RABBITMQ_LOGS".to_owned(), "-".to_owned()),
            (
                "RABBITMQ_ENABLED_PLUGINS_FILE".to_owned(),
                base.join("enabled_plugins").display().to_string(),
            ),
            (
                "RABBITMQ_CONFIG_FILES".to_owned(),
                base.join("conf.d").display().to_string(),
            ),
            ("HOME".to_owned(), base.display().to_string()),
        ];
        std::fs::write(base.join("enabled_plugins"), "[].\n").unwrap();
        std::fs::create_dir_all(base.join("conf.d")).unwrap();
        let mut c = Command::new(server);
        c.envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())));
        let mut peer = Peer::spawn("rabbitmq", c);
        peer.wait_for("RabbitMQ's startup line", Duration::from_secs(20), |l| {
            l.text.contains("Server startup complete")
                || l.text.contains("Starting broker... completed")
        });
        Self {
            peer: Some(peer),
            port,
            env,
        }
    }
}

impl Drop for Rabbit {
    fn drop(&mut self) {
        if let Some(peer) = self.peer.take() {
            let _ = peer.stop();
        }
        // The node's own port mapper, started with the node.
        if let Some(epmd) = tools::find("epmd", &["/usr/lib/erlang/bin"]) {
            let port = self
                .env
                .iter()
                .find(|(k, _)| k == "ERL_EPMD_PORT")
                .map(|(_, v)| v.clone());
            if let Some(port) = port {
                let _ = Command::new(epmd)
                    .arg("-kill")
                    .env("ERL_EPMD_PORT", port)
                    .status();
            }
        }
    }
}

#[test]
fn amqp_bridge_against_rabbitmq() {
    let scratch = Scratch::new("amqp");
    let dir = scratch.path();
    let rabbit = Rabbit::start(dir);
    let lease = lease_domain();
    let domain = lease.id();

    let sent = inbound();
    let mut client = Command::new(tools::require("python3", &[]));
    client
        .arg(peers_dir().join("amqp_peer.py"))
        .arg(rabbit.port.to_string())
        .arg("it.out")
        .arg(SAMPLES.to_string())
        .arg("it.in")
        .args(sent.iter().map(Reading::xcdr2_hex));
    let mut client = Peer::spawn("pika", client);
    client.wait_for("declared", STEP, |l| l.text == "declared");

    let config = dir.join("amqp-bridged.yaml");
    std::fs::write(
        &config,
        format!(
            r#"domain: {domain}
log_level: info
amqp:
  broker_url: "amqp://127.0.0.1:{port}"
  container_id: "zerodds-it-bridge"
  hostname: "localhost"
  sasl:
    mechanism: "PLAIN"
    username: "guest"
    password: "guest"
acl:
  default_deny: false
topics:
  - dds_name: "{OUT}"
    dds_type: "{ty}"
    keyed: true
    amqp_address: "/queues/it.out"
    direction: "out"
    qos:
      reliability: "reliable"
  - dds_name: "{IN}"
    dds_type: "{ty}"
    keyed: true
    amqp_address: "/queues/it.in"
    direction: "in"
    qos:
      reliability: "reliable"
"#,
            port = rabbit.port,
            ty = Reading::NAME
        ),
    )
    .unwrap();
    let dds = DdsSide::new(domain);
    let mut bridge = daemon(
        "ZERODDS_AMQP_BRIDGED",
        "amqp-bridged",
        &config,
        "\"event\":\"attached\"",
    );
    bridge.wait_for("the second link attached", STEP, |l| {
        l.text.contains("\"event\":\"attached\"")
    });
    dds.wait_for_bridge("amqp");

    let written = dds.write_out();
    client.send_line("consume");
    client.wait_for("published", STEP, |l| l.text == "published");
    let got: Vec<(String, String)> = client
        .lines()
        .iter()
        .filter_map(|l| l.text.strip_prefix("message "))
        .map(|rest| {
            let (content_type, body) = rest.split_once(' ').unwrap_or((rest, ""));
            (content_type.to_owned(), body.to_owned())
        })
        .collect();
    assert!(
        got.iter().all(|(ct, _)| ct == "application/vnd.dds.xcdr2"),
        "amqp: content type application/vnd.dds.xcdr2 (DDS-AMQP 1.0 §8.1.1): {got:?}"
    );
    let bodies: Vec<String> = got.into_iter().map(|(_, body)| body).collect();
    assert_received(
        "amqp",
        "each message body read by pika from it.out (DDS-AMQP 1.0 §8.1.1)",
        &bodies,
        &written,
    );
    assert_eq!(
        dds.take_in("amqp", sent.len()),
        sent,
        "amqp: every message of it.in reaches DDS, in order"
    );

    assert!(client.release(STEP).success(), "the pika client failed");
    let _ = bridge.stop();
    drop(rabbit);
    drop(lease);
}

// ---------------------------------------------------------------------
// Fixed-layout topic types of the OPC UA and CORBA scenarios
// ---------------------------------------------------------------------

/// A topic type whose sample is a fixed number of little-endian octets,
/// named after the IDL the bridge configuration declares.
macro_rules! octets_type {
    ($name:ident, $type_name:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Default)]
        struct $name(Vec<u8>);

        impl zerodds_dcps::DdsType for $name {
            const TYPE_NAME: &'static str = $type_name;
            const XCDR1_MATCHES_XCDR2: bool = true;

            fn encode(&self, out: &mut Vec<u8>) -> Result<(), zerodds_dcps::dds_type::EncodeError> {
                out.extend_from_slice(&self.0);
                Ok(())
            }

            fn decode(bytes: &[u8]) -> Result<Self, zerodds_dcps::dds_type::DecodeError> {
                Ok(Self(bytes.to_vec()))
            }
        }
    };
}

// `@final struct Telemetry { double value; }` and `@final struct Command
// { long value; }`: XCDR2 bodies of one member each (DDS-XTypes 1.3
// §7.4.3.5).
octets_type!(Telemetry, "Telemetry");
octets_type!(OpcuaCommand, "Command");
// The CORBA bridge's request, reply and notify samples
// (zerodds-corba-bridge-1.0 §5): the request id, then the operation's
// parameters as GIOP marshalled them.
octets_type!(AddRequest, "Calc_add_Request");
octets_type!(AddReply, "Calc_add_Reply");
octets_type!(ListenerValue, "ListenerValues");

fn built_peers() -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(
        std::env::var("ZERODDS_IT_PEERS")
            .expect("ZERODDS_IT_PEERS: the directory tests/integration/peers/build_peers.sh fills"),
    );
    assert!(dir.is_dir(), "{} does not exist", dir.display());
    dir
}

// ---------------------------------------------------------------------
// OPC UA: the open62541 client library
// ---------------------------------------------------------------------

#[test]
fn opcua_bridge_against_open62541() {
    let scratch = Scratch::new("opcua");
    let dir = scratch.path();
    let lease = lease_domain();
    let domain = lease.id();
    let port = free_port();
    let idl = dir.join("plant.idl");
    std::fs::write(
        &idl,
        "@final struct Telemetry { double value; };\n@final struct Command { long value; };\n",
    )
    .unwrap();
    let metrics_port = free_port();
    let gateway = dir.join("gateway.xml");
    std::fs::write(
        &gateway,
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<zerodds_opcua_gateway>
  <bridge name="integration">
    <domain_id>{domain}</domain_id>
    <dds_to_ua_connection>
      <dds_topic>Telemetry</dds_topic>
      <dds_type>Telemetry</dds_type>
    </dds_to_ua_connection>
    <ua_to_dds_connection>
      <dds_topic>Commands</dds_topic>
      <dds_type>Command</dds_type>
    </ua_to_dds_connection>
  </bridge>
</zerodds_opcua_gateway>
"#
        ),
    )
    .unwrap();
    let config = dir.join("opcua-bridged.yaml");
    std::fs::write(
        &config,
        format!(
            r#"domain: {domain}
log_level: info
opcua:
  idl:
    - "{idl}"
  gateway_config: "{gateway}"
  server:
    enabled: true
    bind: "127.0.0.1:{port}"
    security_policies: ["none"]
acl:
  default_deny: false
metrics:
  enabled: true
  listen: "127.0.0.1:{metrics_port}"
"#,
            idl = idl.display(),
            gateway = gateway.display()
        ),
    )
    .unwrap();

    let p = participant(domain);
    let telemetry_topic = p
        .create_topic::<Telemetry>("Telemetry", TopicQos::default())
        .unwrap();
    let command_topic = p
        .create_topic::<OpcuaCommand>("Commands", TopicQos::default())
        .unwrap();
    let writer = p
        .create_publisher(PublisherQos::default())
        .create_datawriter::<Telemetry>(&telemetry_topic, reliable_writer())
        .unwrap();
    // The daemon's DDS endpoints are BEST_EFFORT (`dds_bridge.rs`, the
    // `reliable` argument `false`); a RELIABLE reader would request more than
    // its writer offers and match nothing (DDS 1.4 §2.2.3.14 RxO). The
    // DataReader default is BEST_EFFORT.
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<OpcuaCommand>(&command_topic, DataReaderQos::default())
        .unwrap();
    let bridge = daemon(
        "ZERODDS_OPCUA_BRIDGED",
        "opcua-bridged",
        &config,
        "running on",
    );
    writer
        .wait_for_matched_subscription(1, STEP)
        .expect("opcua: the bridge's reader of Telemetry");
    reader
        .wait_for_matched_publication(1, STEP)
        .expect("opcua: the bridge's writer of Commands");

    // DDS -> OPC UA: the value the open62541 client reads.
    writer
        .write(&Telemetry(21.5f64.to_le_bytes().to_vec()))
        .unwrap();
    // A best-effort reader acknowledges nothing: the daemon's own counter of
    // the samples it applied to its address space says when the value is
    // there (zerodds-opcua-bridge-1.0 §8.2 metrics).
    wait_for_metric(
        metrics_port,
        "zerodds_opcua_dds_to_ua_samples_total",
        1.0,
        "opcua: the bridge applies the Telemetry sample",
    );

    let mut client = Command::new(built_peers().join("open62541_client"));
    client.args([
        &format!("opc.tcp://127.0.0.1:{port}"),
        "Topic:Telemetry/Value/value",
        "Topic:Commands/Value/value",
        "777",
    ]);
    let (status, lines) = Peer::spawn("open62541_client", client).wait_exit_lines(STEP);
    let out: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
    assert!(status.success(), "open62541 client failed: {out:?}");
    for (step, what) in [
        ("CONNECT_OK", "connects"),
        ("BROWSE_OK", "browses the Objects folder"),
        ("READ_OK value=21.5", "reads the value DDS wrote"),
        ("WRITE_OK value=777", "writes and reads back"),
    ] {
        assert!(
            out.iter().any(|l| l.starts_with(step)),
            "opcua: the open62541 client {what} ({step}): {out:?}"
        );
    }

    // OPC UA -> DDS: the client's write reaches the reader.
    reader
        .wait_for_data(STEP)
        .expect("opcua: the client's write reaches DDS");
    let got: Vec<Vec<u8>> = reader
        .take_with_info()
        .unwrap()
        .into_iter()
        .filter(|s| s.info.valid_data)
        .map(|s| s.data.0)
        .collect();
    assert_eq!(
        got,
        vec![777i32.to_le_bytes().to_vec()],
        "opcua: the Int32 777 the client wrote, as the XCDR2 body of Command"
    );
    let _ = bridge.stop();
    drop(lease);
}

/// Polls the Prometheus text of a daemon's metrics listener until `series`
/// reaches `at_least`, at most [`STEP`].
fn wait_for_metric(port: u16, series: &str, at_least: f64, what: &str) {
    use std::io::{Read, Write};
    let deadline = std::time::Instant::now() + STEP;
    let mut last = String::new();
    while std::time::Instant::now() < deadline {
        if let Ok(mut c) = std::net::TcpStream::connect(("127.0.0.1", port)) {
            let _ = c.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = c.write_all(b"GET /metrics HTTP/1.0\r\n\r\n");
            let mut body = String::new();
            let _ = c.read_to_string(&mut body);
            let value = body
                .lines()
                .find_map(|l| l.strip_prefix(series).map(str::trim))
                .and_then(|v| v.parse::<f64>().ok());
            if value.is_some_and(|v| v >= at_least) {
                return;
            }
            last = body;
        }
        // The interval of the poll on the daemon's counter, not a wait in
        // place of an event.
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("{what}: {series} below {at_least} within {STEP:?}\n{last}");
}

// ---------------------------------------------------------------------
// CORBA: omniORB
// ---------------------------------------------------------------------

#[test]
fn corba_bridge_against_omniorb() {
    let scratch = Scratch::new("corba");
    let dir = scratch.path();
    let lease = lease_domain();
    let domain = lease.id();
    let (iiop_port, listener_port) = (free_port(), free_port());
    let ior_file = dir.join("bridge.ior");
    let config = dir.join("corba-bridged.yaml");
    std::fs::write(
        &config,
        format!(
            r#"domain: {domain}
log_level: info
corba:
  iiop:
    bind: "127.0.0.1:{iiop_port}"
  ior:
    ior_file: "{ior}"
mappings:
  - repo_id: "IDL:zerodds_test/Calc:1.0"
    operation: add
    request_topic:
      dds_name: Calc_add_Request
    reply_topic:
      dds_name: Calc_add_Reply
    timeout_ms: 5000
  - repo_id: "IDL:zerodds_test/Listener:1.0"
    operation: on_value
    direction: dds_to_corba
    notify_topic:
      dds_name: ListenerValues
    targets:
      - "corbaloc:iiop:1.2@127.0.0.1:{listener_port}/Listener"
"#,
            ior = ior_file.display()
        ),
    )
    .unwrap();

    // The DDS backend of `add`, and the writer of the notify topic.
    let p = participant(domain);
    let request_topic = p
        .create_topic::<AddRequest>("Calc_add_Request", TopicQos::default())
        .unwrap();
    let reply_topic = p
        .create_topic::<AddReply>("Calc_add_Reply", TopicQos::default())
        .unwrap();
    let notify_topic = p
        .create_topic::<ListenerValue>("ListenerValues", TopicQos::default())
        .unwrap();
    let requests = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<AddRequest>(&request_topic, reliable_reader())
        .unwrap();
    // The bridge carries the GIOP body unchanged, so its samples are XCDR1,
    // whose alignment is GIOP 1.2's (zerodds-corba-bridge-1.0 §5; the module
    // documentation of the daemon).
    let mut giop = reliable_writer();
    giop.data_representation = Some(vec![zerodds_integration::dds::XCDR]);
    let publisher = p.create_publisher(PublisherQos::default());
    let replies = publisher
        .create_datawriter::<AddReply>(&reply_topic, giop.clone())
        .unwrap();
    let notify = publisher
        .create_datawriter::<ListenerValue>(&notify_topic, giop)
        .unwrap();

    let bridge = daemon(
        "ZERODDS_CORBA_BRIDGED",
        "corba-bridged",
        &config,
        "\"event\":\"ior\"",
    );
    requests
        .wait_for_matched_publication(1, STEP)
        .expect("corba: the bridge's request writer");
    replies
        .wait_for_matched_subscription(1, STEP)
        .expect("corba: the bridge's reply reader");
    notify
        .wait_for_matched_subscription(1, STEP)
        .expect("corba: the bridge's notify reader");

    // CORBA -> DDS -> CORBA: omniORB calls add(2, 40); the request reaches
    // DDS as the request id and the two `long long` parameters, the reply
    // carries the sum back.
    let peers = built_peers();
    let mut client = Command::new(peers.join("calc_client"));
    client
        .arg(&ior_file)
        .args(["-ORBclientCallTimeOutPeriod", "8000"]);
    let mut client = Peer::spawn("calc_client", client);
    requests
        .wait_for_data(STEP)
        .expect("corba: the add request reaches DDS");
    let request = requests
        .take_with_info()
        .unwrap()
        .into_iter()
        .find(|s| s.info.valid_data)
        .expect("a valid request sample")
        .data
        .0;
    assert!(request.len() >= 24, "corba: request {request:02x?}");
    let operand = |at: usize| i64::from_le_bytes(request[at..at + 8].try_into().unwrap());
    assert_eq!(
        (operand(8), operand(16)),
        (2, 40),
        "corba: the GIOP parameters omniORB marshalled, after the request id \
         (zerodds-corba-bridge-1.0 §5): {request:02x?}"
    );
    let mut reply = request[..8].to_vec();
    reply.extend_from_slice(&[0; 8]);
    reply.extend_from_slice(&42i64.to_le_bytes());
    replies.write(&AddReply(reply)).unwrap();
    client.wait_for("add 42", STEP, |l| l.text.starts_with("add "));
    let client_lines: Vec<String> = client.lines().iter().map(|l| l.text.clone()).collect();
    assert!(
        client_lines.iter().any(|l| l == "add 42"),
        "corba: omniORB receives the sum the DDS backend computed: {client_lines:?}"
    );
    let _ = client.stop();

    // DDS -> CORBA: three samples become three oneway on_value calls on an
    // omniORB servant.
    let mut server = Command::new(peers.join("listener_server"));
    server.args([listener_port.to_string(), "3".to_owned()]);
    let mut server = Peer::spawn("listener_server", server);
    server.wait_for("ready", STEP, |l| l.text == "ready");
    for v in [7i64, 8, 9] {
        notify
            .write(&ListenerValue(v.to_le_bytes().to_vec()))
            .unwrap();
    }
    let (status, lines) = server.wait_exit_lines(STEP);
    assert!(status.success(), "listener_server failed");
    let calls: Vec<String> = lines
        .iter()
        .filter(|l| l.text.starts_with("on_value"))
        .map(|l| l.text.clone())
        .collect();
    assert_eq!(
        calls,
        vec!["on_value 7", "on_value 8", "on_value 9"],
        "corba: each DDS sample is one oneway call, in order"
    );
    let _ = bridge.stop();
    drop(lease);
}
