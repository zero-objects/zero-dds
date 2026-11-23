// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! zerodds-mqtt-bridge-1.0 §10, §10a, §12.3: the daemon between the MQTT
//! clients of Mosquitto (`mosquitto_pub`, `mosquitto_sub`) on four brokers
//! and DDS participants of other vendors.
//!
//! The brokers: Mosquitto 2.x as a process of the test; EMQX 5, HiveMQ 4
//! and Aedes as running brokers whose MQTT listener a variable names
//! (`ZERODDS_EMQX_MATRIX`, `ZERODDS_HIVEMQ`, `ZERODDS_AEDES`, each
//! `host:port`; the job's services in CI). On each broker the daemon carries
//! samples both ways between a Cyclone DDS participant
//! (`tests/interop/cyclone_reading.c`, built by the test with `idlc` and the
//! C compiler) and the Mosquitto clients under XCDR1 and XCDR2; carries
//! samples of a second ZeroDDS participant both ways at QoS 0, 1 and 2,
//! retained messages both ways and the properties of §4.2; keeps its session
//! on the broker across a lost connection, and the broker publishes its
//! will; and the broker keeps its retained catalog (§5.2). On Mosquitto the
//! DDS side also is a Fast DDS participant (`tests/interop/fastdds_reading.cpp`)
//! and RTI Connext DDS 7.7.0 (the `shape_main` of the OMG `dds-rtps` suite).
//!
//! Two brokers need settings of the daemon, both measured on the Linux test
//! host: Aedes 1.0.2 refuses a CONNECT of protocol level 5, so its daemon
//! runs under `mqtt.protocol_version: 3.1.1` and its clients under
//! `-V mqttv311`; HiveMQ 4.57.0 answers a will topic beginning with `$` with
//! CONNACK 0x82 and a PUBLISH on such a topic with DISCONNECT 0x82 (MQTT 5.0
//! §4.7.2 leaves those topics to the server), so its daemon has
//! `mqtt.own_topic_prefix: zerodds`.
//!
//! The foreign programs are not in every build environment: the tests are
//! `#[ignore]`d, and the weekly job `integration-bridges-vendor` provisions
//! them and runs them; there a missing program or variable fails a test.
//!
//! ```bash
//! docker run -d --rm --name emqx -p 11883:1883 emqx/emqx:5.8.6
//! docker run -d --rm --name hivemq -p 21883:1883 hivemq/hivemq4:4.57.0
//! docker run -d --rm --name aedes -p 31883:1883 moscajs/aedes:1.0.2
//! ZERODDS_EMQX_MATRIX=127.0.0.1:11883 ZERODDS_HIVEMQ=127.0.0.1:21883 \
//!     ZERODDS_AEDES=127.0.0.1:31883 \
//!     cargo test -p zerodds-mqtt-bridge --features cross-vendor-tests \
//!     --test cross_vendor -- --ignored
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
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_mqtt_bridge::daemon::config::{DaemonConfig, ReconnectConfig, TopicConfig};
use zerodds_mqtt_bridge::daemon::server::{self, DaemonHandle};
use zerodds_mqtt_bridge::version::ProtocolVersion;
use zerodds_rtps::wire_types::EntityId;

// ---------------------------------------------------------------------
// The brokers
// ---------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Vendor {
    Mosquitto,
    Emqx,
    Hivemq,
    Aedes,
}

impl Vendor {
    /// A short tag for client identifiers and topics.
    fn tag(self) -> &'static str {
        match self {
            Self::Mosquitto => "mq",
            Self::Emqx => "em",
            Self::Hivemq => "hm",
            Self::Aedes => "ae",
        }
    }

    /// The variable that names the MQTT listener of a running broker.
    fn variable(self) -> Option<&'static str> {
        match self {
            Self::Mosquitto => None,
            Self::Emqx => Some("ZERODDS_EMQX_MATRIX"),
            Self::Hivemq => Some("ZERODDS_HIVEMQ"),
            Self::Aedes => Some("ZERODDS_AEDES"),
        }
    }

    /// Aedes 1.0.2 speaks MQTT 3.1.1 (a CONNECT of protocol level 5 is
    /// refused, measured); the others MQTT 5.
    fn version(self) -> ProtocolVersion {
        if self == Self::Aedes {
            ProtocolVersion::V311
        } else {
            ProtocolVersion::V5
        }
    }

    /// `mqtt.own_topic_prefix` of the daemon: HiveMQ 4.57.0 refuses the
    /// topics beginning with `$` to its clients (measured), the others take
    /// the default.
    fn own_prefix(self) -> &'static str {
        if self == Self::Hivemq {
            "zerodds"
        } else {
            "$zerodds"
        }
    }
}

/// A child process killed and reaped when the test ends, also when an
/// assertion fails first.
struct Reaped(Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A directory of the test under the system temp directory, open to all:
/// Mosquitto started as root drops to its own user. Dropped when the test
/// ends, also when an assertion fails first, it is removed, and the
/// process's directory once no other test of the process holds one.
struct TestDir(PathBuf);

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
        if let Some(parent) = self.0.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

fn workdir(leaf: &str) -> TestDir {
    let dir = std::env::temp_dir()
        .join(format!("zerodds-mqtt-xv-{}", std::process::id()))
        .join(leaf);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    use std::os::unix::fs::PermissionsExt;
    for d in [dir.parent().unwrap(), dir.as_path()] {
        std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o777)).unwrap();
    }
    TestDir(dir)
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct Broker {
    vendor: Vendor,
    host: String,
    port: u16,
    // The process before its directory: it ends before the directory goes.
    _process: Option<Reaped>,
    dir: TestDir,
}

impl Broker {
    /// Mosquitto as a process of the test on a free port, or the running
    /// broker the vendor's variable names.
    fn start(vendor: Vendor, leaf: &str) -> Self {
        for bin in ["mosquitto_pub", "mosquitto_sub"] {
            assert!(
                Command::new(bin).arg("--help").output().is_ok(),
                "{bin} is not installed; install mosquitto-clients"
            );
        }
        let dir = workdir(leaf);
        let Some(variable) = vendor.variable() else {
            let port = free_port();
            let conf = dir.0.join("mosquitto.conf");
            std::fs::write(
                &conf,
                format!(
                    "listener {port} 127.0.0.1\nallow_anonymous true\nlog_type all\nlog_dest file {}\n",
                    dir.0.join("mosquitto.log").display()
                ),
            )
            .unwrap();
            let child = Command::new("mosquitto")
                .arg("-c")
                .arg(&conf)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("mosquitto; install mosquitto");
            let process = Reaped(child);
            let deadline = Instant::now() + Duration::from_secs(5);
            while TcpStream::connect(("127.0.0.1", port)).is_err() {
                assert!(Instant::now() < deadline, "mosquitto did not open {port}");
                thread::sleep(Duration::from_millis(50));
            }
            return Self {
                vendor,
                host: "127.0.0.1".into(),
                port,
                _process: Some(process),
                dir,
            };
        };
        let addr = std::env::var(variable).unwrap_or_else(|_| {
            panic!("{variable} is not set; this test measures against a running {vendor:?}")
        });
        let (host, port) = addr.rsplit_once(':').expect("host:port");
        let port: u16 = port.parse().expect("a port");
        let deadline = Instant::now() + Duration::from_secs(60);
        while TcpStream::connect((host, port)).is_err() {
            assert!(
                Instant::now() < deadline,
                "{vendor:?} at {addr} is not reachable"
            );
            thread::sleep(Duration::from_millis(500));
        }
        Self {
            vendor,
            host: host.to_string(),
            port,
            _process: None,
            dir,
        }
    }

    fn v5(&self) -> bool {
        self.vendor.version() == ProtocolVersion::V5
    }

    /// The connection arguments of `mosquitto_pub` and `mosquitto_sub`.
    fn client_args(&self) -> Vec<String> {
        let version = if self.v5() { "mqttv5" } else { "mqttv311" };
        vec![
            "-h".into(),
            self.host.clone(),
            "-p".into(),
            self.port.to_string(),
            "-V".into(),
            version.into(),
        ]
    }

    /// `mosquitto_sub` on `filter` with QoS 2 for `count` messages within
    /// `wait` seconds, each printed in `format`; started, given time to
    /// subscribe.
    fn subscriber(&self, filter: &str, count: usize, wait: u32, format: &str) -> Child {
        let child = Command::new("mosquitto_sub")
            .args(self.client_args())
            .args(["-t", filter, "-q", "2"])
            .args(["-C", &count.to_string(), "-W", &wait.to_string()])
            .args(["-F", format])
            .stdout(Stdio::piped())
            .spawn()
            .expect("mosquitto_sub");
        thread::sleep(Duration::from_millis(700));
        child
    }

    /// `mosquitto_pub` of the octets `payload` on `topic` with QoS `qos`
    /// and `extra` arguments.
    fn publish(&self, topic: &str, payload: &[u8], qos: u8, extra: &[&str]) {
        let file = self.dir.0.join(format!("payload-{}.bin", free_port()));
        std::fs::write(&file, payload).unwrap();
        let out = Command::new("mosquitto_pub")
            .args(self.client_args())
            .args(["-t", topic, "-q", &qos.to_string(), "-f"])
            .arg(&file)
            .args(extra)
            .output()
            .expect("mosquitto_pub");
        let _ = std::fs::remove_file(&file);
        assert!(
            out.status.success(),
            "mosquitto_pub on {topic}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// A retained message without payload on `topic`: the broker forgets
    /// the one it holds.
    fn clear_retained(&self, topic: &str) {
        let _ = Command::new("mosquitto_pub")
            .args(self.client_args())
            .args(["-t", topic, "-q", "1", "-r", "-n"])
            .output();
    }
}

/// The lines of a finished `mosquitto_sub`.
fn lines_of_output(child: Child) -> Vec<String> {
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

/// The lines a child writes to stdout, read on a thread.
fn lines_of(child: &mut Child) -> Receiver<String> {
    let out = child.stdout.take().expect("piped stdout");
    let (tx, rx) = std::sync::mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    rx
}

fn hex_bytes(hex: &str) -> Vec<u8> {
    let hex = hex.trim();
    assert!(hex.len().is_multiple_of(2), "{hex}");
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap_or_else(|_| panic!("{hex}")))
        .collect()
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A client identifier of this run: the vendor's tag, the test's and the
/// process id.
fn client_id(vendor: Vendor, test: &str) -> String {
    format!("zx-{}-{test}-{}", vendor.tag(), std::process::id())
}

/// The daemon's configuration towards `b` on a leased DDS domain, its
/// session directory of the test.
fn daemon_config(b: &Broker, id: &str) -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.broker_url = format!("mqtt://{}:{}", b.host, b.port);
    cfg.client_id = id.to_string();
    cfg.protocol_version = b.vendor.version();
    cfg.own_topic_prefix = b.vendor.own_prefix().to_string();
    cfg.session_dir = b.dir.0.join("sessions").display().to_string();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    (cfg, lease)
}

fn topic(dds: &str, ty: &str, mqtt: &str, direction: &str, qos: u8) -> TopicConfig {
    TopicConfig {
        dds_name: dds.into(),
        dds_type: ty.into(),
        mqtt_topic: mqtt.into(),
        direction: direction.into(),
        mqtt_qos: Some(qos),
        retain: Some(false),
        reliability: "reliable".into(),
        durability: "volatile".into(),
        ..TopicConfig::default()
    }
}

/// [`topic`] whose endpoints use the one data representation `xcdr`
/// (`xcdr1`, `xcdr2`; Spec §3): a reader of another vendor that accepts only
/// that one matches the daemon's writer before the first PUBLISH.
fn topic_in(dds: &str, ty: &str, mqtt: &str, direction: &str, xcdr: &str) -> TopicConfig {
    TopicConfig {
        data_representation: Some(xcdr.into()),
        ..topic(dds, ty, mqtt, direction, 1)
    }
}

/// Waits until `matched` holds, `what` naming it in the failure.
fn wait_for(what: &str, within: Duration, matched: impl Fn() -> bool) {
    let deadline = Instant::now() + within;
    while !matched() {
        assert!(Instant::now() < deadline, "{what}");
        thread::sleep(Duration::from_millis(50));
    }
}

// ---------------------------------------------------------------------
// The DDS participants of other vendors
// ---------------------------------------------------------------------

fn interop_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/interop")
}

/// The Cyclone DDS participant, built from its sources with `idlc` and the
/// C compiler in a directory of the test `leaf`.
fn cyclone_reading(leaf: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("mqtt-cyclone-{leaf}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for file in ["reading.idl", "cyclone_reading.c"] {
        std::fs::copy(interop_dir().join(file), dir.join(file)).unwrap();
    }
    let idlc = Command::new("idlc")
        .arg("reading.idl")
        .current_dir(&dir)
        .status()
        .expect("idlc; install cyclonedds-dev");
    assert!(idlc.success(), "idlc failed");
    let cc = Command::new("cc")
        .args([
            "-o",
            "cyclone_reading",
            "cyclone_reading.c",
            "reading.c",
            "-I.",
            "-lddsc",
        ])
        .current_dir(&dir)
        .status()
        .expect("cc");
    assert!(cc.success(), "the Cyclone DDS participant does not build");
    dir.join("cyclone_reading")
}

/// The Fast DDS participant, built from its source with the C++ compiler
/// against `libfastdds-dev`.
fn fastdds_reading() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mqtt-fastdds-reading");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(
        interop_dir().join("fastdds_reading.cpp"),
        dir.join("fastdds_reading.cpp"),
    )
    .unwrap();
    let cxx = Command::new("c++")
        .args([
            "-std=c++17",
            "-o",
            "fastdds_reading",
            "fastdds_reading.cpp",
            "-lfastdds",
            "-lfastcdr",
        ])
        .current_dir(&dir)
        .status()
        .expect("c++; install g++ and libfastdds-dev");
    assert!(cxx.success(), "the Fast DDS participant does not build");
    dir.join("fastdds_reading")
}

/// The encapsulation header and the CDR bytes of a `Sensor::Reading`
/// (`@key long id; double value; string unit;`), little endian (DDS-XTypes
/// 1.3 §7.4.3, Table 60): `@final` under XCDR1 (`CDR_LE`, a double aligned
/// to 8) or XCDR2 (`CDR2_LE`, aligned to 4), `@appendable` under XCDR2
/// (`D_CDR2_LE`, a DHEADER in front).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layout {
    FinalXcdr1,
    FinalXcdr2,
    AppendableXcdr2,
}

impl Layout {
    fn identifier(self) -> [u8; 2] {
        match self {
            Self::FinalXcdr1 => [0x00, 0x01],
            Self::FinalXcdr2 => [0x00, 0x07],
            Self::AppendableXcdr2 => [0x00, 0x09],
        }
    }

    fn payload(self, id: i32, value: f64, unit: &str) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend(id.to_le_bytes());
        if self == Self::FinalXcdr1 {
            body.extend([0u8; 4]);
        }
        body.extend(value.to_le_bytes());
        body.extend(u32::try_from(unit.len() + 1).unwrap().to_le_bytes());
        body.extend(unit.as_bytes());
        body.push(0);
        let mut out = vec![self.identifier()[0], self.identifier()[1], 0, 0];
        if self == Self::AppendableXcdr2 {
            out.extend(u32::try_from(body.len()).unwrap().to_le_bytes());
        }
        out.extend(body);
        out
    }

    /// The members of a payload of this layout; the CDR bytes may be
    /// followed by padding.
    fn decode(self, payload: &[u8]) -> (i32, f64, String) {
        assert_eq!(payload[..2], self.identifier(), "{}", hex_of(payload));
        let mut at = 4;
        if self == Self::AppendableXcdr2 {
            let dheader = u32::from_le_bytes(payload[4..8].try_into().unwrap()) as usize;
            assert!(8 + dheader <= payload.len(), "{}", hex_of(payload));
            at = 8;
        }
        let u32_at = |i: usize| u32::from_le_bytes(payload[i..i + 4].try_into().unwrap());
        let id = u32_at(at) as i32;
        at += if self == Self::FinalXcdr1 { 8 } else { 4 };
        let value = f64::from_le_bytes(payload[at..at + 8].try_into().unwrap());
        at += 8;
        let len = u32_at(at) as usize;
        at += 4;
        let unit = &payload[at..at + len];
        assert_eq!(unit.last(), Some(&0), "{}", hex_of(payload));
        (
            id,
            value,
            String::from_utf8(unit[..len - 1].to_vec()).unwrap(),
        )
    }
}

fn reading_json(id: i32, value: f64, unit: &str) -> serde_json::Value {
    serde_json::json!({ "id": id, "value": value, "unit": unit })
}

/// The next JSON line of a foreign subscriber within `within` that is not
/// an event line.
fn next_json(lines: &Receiver<String>, within: Duration) -> Option<serde_json::Value> {
    let deadline = Instant::now() + within;
    loop {
        let left = deadline.checked_duration_since(Instant::now())?;
        let line = lines.recv_timeout(left).ok()?;
        let v: serde_json::Value =
            serde_json::from_str(&line).unwrap_or_else(|e| panic!("{line}: {e}"));
        if v.get("event").is_none() {
            return Some(v);
        }
    }
}

/// Waits for the event line `{"event":"<event>"}` of a foreign participant
/// within `within`, `what` naming it in the failure; a sample line before it
/// fails the test.
fn wait_event(lines: &Receiver<String>, event: &str, within: Duration, what: &str) {
    let deadline = Instant::now() + within;
    loop {
        let line = lines
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|_| panic!("{what}"));
        let v: serde_json::Value =
            serde_json::from_str(&line).unwrap_or_else(|e| panic!("{line}: {e}"));
        match v.get("event") {
            Some(e) if e == event => return,
            Some(_) => {}
            None => panic!("{what}: a sample before the event {event}: {line}"),
        }
    }
}

/// Both directions through the daemon between a foreign participant and
/// the Mosquitto clients on `b`: a PUBLISH of `layout`'s octets on
/// `in_topic` reaches the participant's reader of `dds_in`; a sample its
/// writer writes on `dds_out` reaches `mosquitto_sub` on `out_topic` as the
/// encapsulation header and the CDR bytes of `layout`; and those octets,
/// published on `in_topic`, reach the participant's reader again as the
/// sample its writer wrote. `peer(args)` runs the participant with
/// `sub`/`pub` arguments that follow the DDS domain.
#[allow(clippy::too_many_arguments)]
fn through_foreign(
    b: &Broker,
    handle: &DaemonHandle,
    domain: i32,
    layout: Layout,
    dds_in: &str,
    in_topic: &str,
    out_topic: &str,
    sub: &[&str],
    pub_: &[&str],
    peer: &dyn Fn(&[String]) -> Command,
    vendor: &str,
) {
    let domain = domain.to_string();
    let with_domain = |mode: &str, rest: &[&str]| -> Vec<String> {
        let mut args = vec![mode.to_string(), domain.clone()];
        args.extend(rest.iter().map(|s| (*s).to_string()));
        args
    };
    // MQTT → DDS: mosquitto_pub, the foreign reader takes.
    let mut reader = Reaped(
        peer(&with_domain("sub", sub))
            .stdout(Stdio::piped())
            .spawn()
            .expect("the foreign subscriber"),
    );
    let lines = lines_of(&mut reader.0);
    assert_eq!(
        lines.recv_timeout(Duration::from_secs(10)).expect("ready"),
        "{\"event\":\"ready\"}"
    );
    // Both sides of the match before the one PUBLISH: the daemon's writer
    // knows the reader, and the reader the writer (a volatile reader takes
    // nothing its writer wrote before then).
    let writer = handle.user_writers[dds_in];
    wait_for(
        &format!("the daemon's writer of {dds_in} matched no reader of {vendor}"),
        Duration::from_secs(15),
        || {
            !handle
                .runtime
                .user_writer_matched_reader_guids(writer)
                .is_empty()
        },
    );
    wait_event(
        &lines,
        "matched",
        Duration::from_secs(15),
        &format!("the reader of {vendor} matched no writer of {dds_in}"),
    );
    let sent = layout.payload(9, 4.5, "W");
    b.publish(in_topic, &sent, 1, &[]);
    let taken = next_json(&lines, Duration::from_secs(10))
        .unwrap_or_else(|| panic!("the PUBLISH of {layout:?} did not reach {vendor}"));
    assert_eq!(taken, reading_json(9, 4.5, "W"), "{vendor}, {layout:?}");

    // DDS → MQTT: the foreign writer writes, mosquitto_sub prints the octets.
    let msub = b.subscriber(out_topic, 1, 20, "%x");
    let written = peer(&with_domain("pub", pub_))
        .output()
        .expect("the foreign publisher");
    assert!(
        written.status.success(),
        "{vendor}: {}",
        String::from_utf8_lossy(&written.stderr)
    );
    let got = lines_of_output(msub);
    assert_eq!(got.len(), 1, "{vendor}, {layout:?}: {got:?}");
    let octets = hex_bytes(&got[0]);
    assert_eq!(octets[2..4], [0, 0], "{}", got[0]);
    let (id, value, unit) = layout.decode(&octets);
    assert_eq!((id, value, unit.as_str()), (10, 5.5, "Hz"), "{}", got[0]);

    // The octets back into DDS: the foreign reader takes the sample its
    // writer wrote.
    while lines.try_recv().is_ok() {}
    b.publish(in_topic, &octets, 1, &[]);
    let again = next_json(&lines, Duration::from_secs(10))
        .unwrap_or_else(|| panic!("the octets of {vendor} did not reach it again"));
    assert_eq!(again, reading_json(10, 5.5, "Hz"), "{vendor}, {layout:?}");
}

// ---------------------------------------------------------------------
// §12.3: Cyclone DDS through the daemon on each broker
// ---------------------------------------------------------------------

/// Spec §12.3 on the broker of `vendor`: a Cyclone DDS reader takes what
/// `mosquitto_pub` publishes, `mosquitto_sub` gets what a Cyclone DDS writer
/// writes, under XCDR1 (`CDR_LE`) and XCDR2 (`CDR2_LE`); the octets of
/// Cyclone DDS go back to it through the broker unchanged.
fn cyclone_through(vendor: Vendor) {
    let cyclone = cyclone_reading(vendor.tag());
    let b = Broker::start(vendor, &format!("cyclone-{}", vendor.tag()));
    let id = client_id(vendor, "cyc");
    let (mut cfg, lease) = daemon_config(&b, &id);
    for x in [1, 2] {
        cfg.topics.push(topic_in(
            &format!("ReadingIn{x}"),
            "Sensor::Reading",
            &format!("zerodds/{id}/in{x}"),
            "in",
            &format!("xcdr{x}"),
        ));
        cfg.topics.push(topic_in(
            &format!("ReadingOut{x}"),
            "Sensor::Reading",
            &format!("zerodds/{id}/out{x}"),
            "out",
            &format!("xcdr{x}"),
        ));
    }
    let mut handle = server::start(cfg).expect("daemon start");
    for (x, layout) in [("1", Layout::FinalXcdr1), ("2", Layout::FinalXcdr2)] {
        let dds_in = format!("ReadingIn{x}");
        let dds_out = format!("ReadingOut{x}");
        through_foreign(
            &b,
            &handle,
            lease.id(),
            layout,
            &dds_in,
            &format!("zerodds/{id}/in{x}"),
            &format!("zerodds/{id}/out{x}"),
            &[&dds_in, x, "40"],
            &[&dds_out, x, "10", "5.5", "Hz"],
            &|args: &[String]| {
                let mut c = Command::new(&cyclone);
                c.args(args);
                c
            },
            "Cyclone DDS",
        );
    }
    handle.shutdown();
}

#[test]
#[ignore = "needs mosquitto, Cyclone DDS (cyclonedds-dev, idlc) and a C compiler; runs in integration-bridges-vendor"]
fn mqttbridge_12_3_mosquitto_carries_mqtt_to_cyclone_and_cyclone_to_mqtt() {
    cyclone_through(Vendor::Mosquitto);
}

#[test]
#[ignore = "needs EMQX 5 (ZERODDS_EMQX_MATRIX), Cyclone DDS and a C compiler; runs in integration-bridges-vendor"]
fn mqttbridge_12_3_emqx_carries_mqtt_to_cyclone_and_cyclone_to_mqtt() {
    cyclone_through(Vendor::Emqx);
}

#[test]
#[ignore = "needs HiveMQ 4 (ZERODDS_HIVEMQ), Cyclone DDS and a C compiler; runs in integration-bridges-vendor"]
fn mqttbridge_12_3_hivemq_carries_mqtt_to_cyclone_and_cyclone_to_mqtt() {
    cyclone_through(Vendor::Hivemq);
}

#[test]
#[ignore = "needs Aedes (ZERODDS_AEDES), Cyclone DDS and a C compiler; runs in integration-bridges-vendor"]
fn mqttbridge_12_3_aedes_carries_mqtt_to_cyclone_and_cyclone_to_mqtt() {
    cyclone_through(Vendor::Aedes);
}

// ---------------------------------------------------------------------
// §10: Fast DDS and RTI Connext DDS through the daemon on Mosquitto
// ---------------------------------------------------------------------

/// Spec §10 with Fast DDS 3 (the `@appendable` `Sensor::Reading` built with
/// its DynamicTypes API, XCDR2: `D_CDR2_LE`): the exchanges of §12.3.
#[test]
#[ignore = "needs mosquitto, Fast DDS (libfastdds-dev) and a C++ compiler; runs in integration-bridges-vendor"]
fn mqttbridge_10_fastdds_carries_mqtt_to_dds_and_dds_to_mqtt() {
    let fastdds = fastdds_reading();
    let b = Broker::start(Vendor::Mosquitto, "fastdds");
    let id = client_id(Vendor::Mosquitto, "fdds");
    let (mut cfg, lease) = daemon_config(&b, &id);
    cfg.topics.push(topic_in(
        "ReadingIn",
        "Sensor::Reading",
        &format!("zerodds/{id}/in"),
        "in",
        "xcdr2",
    ));
    cfg.topics.push(topic_in(
        "ReadingOut",
        "Sensor::Reading",
        &format!("zerodds/{id}/out"),
        "out",
        "xcdr2",
    ));
    let mut handle = server::start(cfg).expect("daemon start");
    through_foreign(
        &b,
        &handle,
        lease.id(),
        Layout::AppendableXcdr2,
        "ReadingIn",
        &format!("zerodds/{id}/in"),
        &format!("zerodds/{id}/out"),
        &["ReadingIn", "40"],
        &["ReadingOut", "10", "5.5", "Hz"],
        &|args: &[String]| {
            let mut c = Command::new(&fastdds);
            c.args(args);
            c
        },
        "Fast DDS",
    );
    handle.shutdown();
}

/// The `shape_main` of RTI Connext DDS 7.7.0 that the OMG `dds-rtps` suite
/// releases; `tests/integration/provision.sh connext-shape-main` puts it at
/// the default path, `CONNEXT_SHAPE_MAIN` names another.
fn connext_shape_main() -> PathBuf {
    let exe = std::env::var("CONNEXT_SHAPE_MAIN")
        .unwrap_or_else(|_| "/opt/omg-connext/connext_dds-7.7.0_shape_main_linux".to_string());
    assert!(
        Path::new(&exe).is_file(),
        "no {exe}; provision connext-shape-main"
    );
    PathBuf::from(exe)
}

/// `shape_main` with `args`, its stdout line-buffered and read on a thread.
fn shape_main(exe: &Path, args: &[&str]) -> (Reaped, Receiver<String>) {
    let mut child = Command::new("stdbuf")
        .arg("-oL")
        .arg(exe)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("stdbuf and shape_main");
    let lines = lines_of(&mut child);
    (Reaped(child), lines)
}

/// The sample line of `shape_main` (`Square     BLUE       010 020 [30]`)
/// as color, x, y and shapesize; `None` for any other line.
fn shape_line(line: &str) -> Option<(String, i32, i32, i32)> {
    let f: Vec<&str> = line.split_whitespace().collect();
    if f.len() != 5 {
        return None;
    }
    let size = f[4].strip_prefix('[')?.strip_suffix(']')?;
    Some((
        f[1].to_string(),
        f[2].parse().ok()?,
        f[3].parse().ok()?,
        size.parse().ok()?,
    ))
}

/// A `ShapeType` (`interop/shapes/omg-dds-rtps.idl`, `@appendable`) under
/// `D_CDR2_LE`: DHEADER, the color, x, y, shapesize and an empty
/// `additional_payload_size`.
fn shape_payload(color: &str, x: i32, y: i32, size: i32) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend(u32::try_from(color.len() + 1).unwrap().to_le_bytes());
    body.extend(color.as_bytes());
    body.push(0);
    while !body.len().is_multiple_of(4) {
        body.push(0);
    }
    for v in [x, y, size] {
        body.extend(v.to_le_bytes());
    }
    body.extend(0u32.to_le_bytes());
    let mut out = vec![0x00, 0x09, 0x00, 0x00];
    out.extend(u32::try_from(body.len()).unwrap().to_le_bytes());
    out.extend(body);
    out
}

/// The color, x, y and shapesize of a `ShapeType` under `D_CDR2_LE`.
fn shape_of(payload: &[u8]) -> (String, i32, i32, i32) {
    assert_eq!(
        payload[..4],
        [0x00, 0x09, 0x00, 0x00],
        "{}",
        hex_of(payload)
    );
    let u32_at = |i: usize| u32::from_le_bytes(payload[i..i + 4].try_into().unwrap());
    let len = u32_at(8) as usize;
    let color = String::from_utf8(payload[12..12 + len - 1].to_vec()).unwrap();
    let mut at = 12 + len;
    at = at.div_ceil(4) * 4;
    (
        color,
        u32_at(at) as i32,
        u32_at(at + 4) as i32,
        u32_at(at + 8) as i32,
    )
}

/// Spec §10 with RTI Connext DDS 7.7.0 (`shape_main` of the OMG `dds-rtps`
/// suite, `ShapeType`, XCDR2 with `-x 2`): a BLUE square `mosquitto_pub`
/// publishes reaches the Connext reader of `Square`; a RED circle of the
/// Connext writer of `Circle` reaches `mosquitto_sub` as `D_CDR2_LE`, and
/// those octets, published on the square's topic, reach the Connext reader
/// as that circle's color, position and size.
#[test]
#[ignore = "needs mosquitto and the Connext shape_main of the OMG dds-rtps suite; runs in integration-bridges-vendor"]
fn mqttbridge_10_connext_carries_mqtt_to_dds_and_dds_to_mqtt() {
    let exe = connext_shape_main();
    let b = Broker::start(Vendor::Mosquitto, "connext");
    let id = client_id(Vendor::Mosquitto, "rti");
    let (mut cfg, lease) = daemon_config(&b, &id);
    let square = format!("zerodds/{id}/square");
    let circle = format!("zerodds/{id}/circle");
    cfg.topics
        .push(topic_in("Square", "ShapeType", &square, "in", "xcdr2"));
    cfg.topics
        .push(topic_in("Circle", "ShapeType", &circle, "out", "xcdr2"));
    let domain = lease.id().to_string();
    let mut handle = server::start(cfg).expect("daemon start");

    // MQTT → DDS: the reliable Connext reader of Square. Both sides of the
    // match before the one PUBLISH: shape_main prints
    // `on_subscription_matched()` once its reader matched a writer, and the
    // daemon's writer knows the reader.
    let (_reader, lines) = shape_main(
        &exe,
        &["-S", "-r", "-t", "Square", "-d", &domain, "-x", "2"],
    );
    let matched = Instant::now() + Duration::from_secs(15);
    loop {
        let line = lines
            .recv_timeout(matched.saturating_duration_since(Instant::now()))
            .expect("the Connext reader of Square matched no writer");
        assert!(
            shape_line(&line).is_none(),
            "a shape before the match: {line}"
        );
        if line.starts_with("on_subscription_matched()") {
            break;
        }
    }
    let writer = handle.user_writers["Square"];
    wait_for(
        "the daemon's writer of Square matched no Connext reader",
        Duration::from_secs(15),
        || {
            !handle
                .runtime
                .user_writer_matched_reader_guids(writer)
                .is_empty()
        },
    );
    b.publish(&square, &shape_payload("BLUE", 10, 20, 30), 1, &[]);
    let deadline = Instant::now() + Duration::from_secs(10);
    let taken = loop {
        let line = lines
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|_| panic!("the PUBLISH of the square did not reach Connext"));
        if let Some(s) = shape_line(&line) {
            break s;
        }
    };
    assert_eq!(taken, ("BLUE".to_string(), 10, 20, 30));

    // DDS → MQTT: the Connext writer of Circle, RED of size 25.
    let msub = b.subscriber(&circle, 1, 20, "%x");
    let (_publisher, _) = shape_main(
        &exe,
        &[
            "-P", "-t", "Circle", "-c", "RED", "-z", "25", "-d", &domain, "-x", "2",
        ],
    );
    let got = lines_of_output(msub);
    assert_eq!(got.len(), 1, "no circle of Connext reached mosquitto_sub");
    let octets = hex_bytes(&got[0]);
    let (color, x, y, size) = shape_of(&octets);
    assert_eq!((color.as_str(), size), ("RED", 25), "{}", got[0]);

    // The octets on the square's topic: the Connext reader takes that shape.
    thread::sleep(Duration::from_millis(300));
    while lines.try_recv().is_ok() {}
    b.publish(&square, &octets, 1, &[]);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let line = lines
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap_or_else(|_| panic!("the circle's octets did not reach Connext"));
        if shape_line(&line) == Some(("RED".to_string(), x, y, 25)) {
            break;
        }
    }
    handle.shutdown();
}

// ---------------------------------------------------------------------
// §10a: QoS 0/1/2, retained messages and properties on each broker
// ---------------------------------------------------------------------

/// A ZeroDDS participant of the test on `domain`, beside the daemon's,
/// reached over RTPS.
fn foreign_participant(domain: i32, tag: u8) -> Arc<DcpsRuntime> {
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[4..8].copy_from_slice(&domain.to_be_bytes());
    prefix[11] = tag;
    DcpsRuntime::start(
        domain,
        zerodds_rtps::wire_types::GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .expect("the test's participant")
}

fn peer_reader(peer: &DcpsRuntime, name: &str, durability: &str) -> Receiver<UserSample> {
    peer.register_user_reader(UserReaderConfig {
        topic_name: name.into(),
        type_name: "Q".into(),
        reliable: true,
        durability: if durability == "transient_local" {
            zerodds_qos::DurabilityKind::TransientLocal
        } else {
            zerodds_qos::DurabilityKind::Volatile
        },
        ..Default::default()
    })
    .unwrap()
    .1
}

/// A writer of the test's participant; `lifespan_secs` its LIFESPAN, `None`
/// for an infinite one.
fn peer_writer(
    peer: &DcpsRuntime,
    name: &str,
    durability: &str,
    lifespan_secs: Option<i32>,
) -> EntityId {
    let mut lifespan = zerodds_qos::LifespanQosPolicy::default();
    if let Some(secs) = lifespan_secs {
        lifespan.duration = zerodds_qos::Duration::from_secs(secs);
    }
    peer.register_user_writer(UserWriterConfig {
        topic_name: name.into(),
        type_name: "Q".into(),
        reliable: true,
        durability: if durability == "transient_local" {
            zerodds_qos::DurabilityKind::TransientLocal
        } else {
            zerodds_qos::DurabilityKind::Volatile
        },
        lifespan,
        ..Default::default()
    })
    .unwrap()
}

/// The alive samples of `rx` until none came for `quiet`.
fn samples(rx: &Receiver<UserSample>, first: Duration, quiet: Duration) -> Vec<UserSample> {
    let mut out = Vec::new();
    let mut wait = first;
    while let Ok(s) = rx.recv_timeout(wait) {
        if matches!(s, UserSample::Alive { .. }) {
            out.push(s);
            wait = quiet;
        }
    }
    out
}

fn payload_of(s: &UserSample) -> Vec<u8> {
    match s {
        UserSample::Alive { payload, .. } => payload.to_vec(),
        other => panic!("{other:?}"),
    }
}

/// Spec §10a on the broker of `vendor`: samples of a ZeroDDS participant
/// reach `mosquitto_sub` with the QoS of their topic, 0, 1 and 2, and under
/// MQTT 5 with the properties of §4.2: Content Type, Payload Format
/// Indicator, the user properties, and the Message Expiry Interval of the
/// writer's LIFESPAN (the QoS 1 writer's 30 s, as the time left when the
/// broker forwards it, MQTT 5.0 §3.3.2.3.3; none for the infinite lifespan
/// of the others); messages of `mosquitto_pub` at QoS 0,
/// 1 and 2 reach the participant once each, under MQTT 5 with the key hash
/// and the source timestamp of their user properties; a retained message the
/// broker held before the daemon subscribed reaches DDS, and the sample of a
/// TRANSIENT_LOCAL topic is retained on the broker for a later subscriber.
fn qos_retain_properties(vendor: Vendor) {
    let b = Broker::start(vendor, &format!("qos-{}", vendor.tag()));
    let id = client_id(vendor, "qos");
    let base = format!("zerodds/{id}");
    let (mut cfg, lease) = daemon_config(&b, &id);
    for q in 0..=2u8 {
        cfg.topics.push(topic(
            &format!("QIn{q}"),
            "Q",
            &format!("{base}/in/{q}"),
            "in",
            q,
        ));
        cfg.topics.push(topic(
            &format!("QOut{q}"),
            "Q",
            &format!("{base}/out/{q}"),
            "out",
            q,
        ));
    }
    let ret_in = format!("{base}/ret/in");
    let ret_out = format!("{base}/ret/out");
    for (dds, mqtt, direction) in [("RetIn", &ret_in, "in"), ("RetOut", &ret_out, "out")] {
        cfg.topics.push(TopicConfig {
            retain: None,
            durability: "transient_local".into(),
            ..topic(dds, "Q", mqtt, direction, 1)
        });
    }
    // The retained message the broker holds before the daemon subscribes.
    b.publish(&ret_in, &[0x00, 0x01, 0x00, 0x00, 0x52], 1, &["-r"]);

    let peer = foreign_participant(lease.id(), 1);
    let readers: Vec<Receiver<UserSample>> = (0..=2u8)
        .map(|q| peer_reader(&peer, &format!("QIn{q}"), "volatile"))
        .collect();
    let writers: Vec<EntityId> = (0..=2u8)
        .map(|q| {
            peer_writer(
                &peer,
                &format!("QOut{q}"),
                "volatile",
                (q == 1).then_some(30),
            )
        })
        .collect();
    let ret_reader = peer_reader(&peer, "RetIn", "transient_local");
    let ret_writer = peer_writer(&peer, "RetOut", "transient_local", None);
    let mut handle = server::start(cfg).expect("daemon start");
    for w in writers.iter().chain([&ret_writer]) {
        wait_for(
            "the daemon's reader matched the participant's writer",
            Duration::from_secs(15),
            || !peer.user_writer_matched_reader_guids(*w).is_empty(),
        );
    }
    for name in ["QIn0", "QIn1", "QIn2"] {
        let w = handle.user_writers[name];
        wait_for(
            &format!("the daemon's writer of {name} matched no reader"),
            Duration::from_secs(15),
            || {
                !handle
                    .runtime
                    .user_writer_matched_reader_guids(w)
                    .is_empty()
            },
        );
    }

    // The retained message of the broker reached DDS.
    let ret = samples(
        &ret_reader,
        Duration::from_secs(10),
        Duration::from_millis(500),
    );
    assert_eq!(
        ret.iter().map(payload_of).collect::<Vec<_>>(),
        [vec![0x52]],
        "{vendor:?}: the retained message of the broker"
    );

    // DDS → MQTT at QoS 0, 1, 2.
    let format = if b.v5() {
        "%t|%q|%r|%C|%F|%E|%P|%x"
    } else {
        "%t|%q|%r|%x"
    };
    let msub = b.subscriber(&format!("{base}/out/+"), 3, 15, format);
    let ts = zerodds_rtps::header_extension::HeTimestamp {
        seconds: 1_700_000_000,
        fraction: 0x8000_0000,
    };
    for (q, w) in writers.iter().enumerate() {
        let q = u8::try_from(q).unwrap();
        peer.write_user_sample_keyed(*w, &[0xa0 + q, 0x5a], [0x10 + q; 16], Some(ts))
            .unwrap();
    }
    let mut got = lines_of_output(msub);
    got.sort();
    assert_eq!(got.len(), 3, "{vendor:?}: {got:?}");
    for (q, line) in got.iter().enumerate() {
        let f: Vec<&str> = line.split('|').collect();
        let octets = hex_bytes(f[f.len() - 1]);
        assert_eq!(f[0], format!("{base}/out/{q}"), "{line}");
        assert_eq!(f[1], q.to_string(), "{vendor:?}: the QoS of {line}");
        assert_eq!(f[2], "0", "{line}");
        let q = u8::try_from(q).unwrap();
        assert_eq!(octets[4..], [0xa0 + q, 0x5a], "{line}");
        if b.v5() {
            let xcdr1 = octets[1] <= 0x03;
            assert_eq!(
                f[3],
                if xcdr1 {
                    "application/x-dds-cdr"
                } else {
                    "application/x-dds-cdr2"
                },
                "{line}"
            );
            assert_eq!(f[4], "0", "{line}");
            if q == 1 {
                let expiry: u32 = f[5]
                    .parse()
                    .unwrap_or_else(|_| panic!("{vendor:?}: no Message Expiry Interval: {line}"));
                assert!((28..=30).contains(&expiry), "{vendor:?}: {line}");
            } else {
                assert_eq!(f[5], "", "{vendor:?}: an expiry of no lifespan: {line}");
            }
            for want in [
                "zerodds_type:Q".to_string(),
                format!("zerodds_topic:QOut{q}"),
                "zerodds_flags:0x00".to_string(),
                format!("zerodds_key_hash:{}", hex_of(&[0x10 + q; 16])),
                "zerodds_source_ts_ns:1700000000500000000".to_string(),
            ] {
                assert!(f[6].contains(&want), "{vendor:?}: {want} not in {line}");
            }
        }
    }

    // MQTT → DDS at QoS 0, 1, 2; under MQTT 5 with the properties of §4.2.
    for q in 0..=2u8 {
        let mut extra: Vec<String> = Vec::new();
        if b.v5() {
            for (name, value) in [
                ("zerodds_key_hash", hex_of(&[0x20 + q; 16])),
                ("zerodds_source_ts_ns", "1700000001250000000".to_string()),
            ] {
                extra.extend(["-D", "publish", "user-property", name].map(String::from));
                extra.push(value);
            }
            extra.extend(
                ["-D", "publish", "content-type", "application/x-dds-cdr"].map(String::from),
            );
        }
        let extra: Vec<&str> = extra.iter().map(String::as_str).collect();
        b.publish(
            &format!("{base}/in/{q}"),
            &[0x00, 0x01, 0x00, 0x00, 0xb0 + q],
            q,
            &extra,
        );
    }
    for (q, rx) in readers.iter().enumerate() {
        let q = u8::try_from(q).unwrap();
        let got = samples(rx, Duration::from_secs(10), Duration::from_millis(800));
        assert_eq!(got.len(), 1, "{vendor:?}: QoS {q}: {got:?}");
        assert_eq!(payload_of(&got[0]), [0xb0 + q]);
        if b.v5() {
            let UserSample::Alive {
                key_hash,
                source_timestamp,
                ..
            } = &got[0]
            else {
                panic!("an alive sample")
            };
            assert_eq!(*key_hash, Some([0x20 + q; 16]), "{vendor:?}");
            let ts = source_timestamp.expect("a source timestamp");
            assert_eq!((ts.seconds, ts.fraction), (1_700_000_001, 0x4000_0000));
        }
    }

    // DDS → MQTT retained: a later subscriber gets the sample of the
    // TRANSIENT_LOCAL topic from the broker.
    peer.write_user_sample(ret_writer, vec![0x53]).unwrap();
    thread::sleep(Duration::from_millis(800));
    let late = b.subscriber(&ret_out, 1, 8, "%r|%x");
    let got = lines_of_output(late);
    assert_eq!(got.len(), 1, "{vendor:?}: no retained sample: {got:?}");
    assert!(
        got[0].starts_with("1|") && got[0].ends_with("53"),
        "{vendor:?}: {got:?}"
    );
    handle.shutdown();
    b.clear_retained(&ret_in);
    b.clear_retained(&ret_out);
}

#[test]
#[ignore = "needs mosquitto; runs in integration-bridges-vendor"]
fn mqttbridge_10_mosquitto_carries_qos_0_1_2_retained_messages_and_the_properties() {
    qos_retain_properties(Vendor::Mosquitto);
}

#[test]
#[ignore = "needs EMQX 5 (ZERODDS_EMQX_MATRIX); runs in integration-bridges-vendor"]
fn mqttbridge_10_emqx_carries_qos_0_1_2_retained_messages_and_the_properties() {
    qos_retain_properties(Vendor::Emqx);
}

#[test]
#[ignore = "needs HiveMQ 4 (ZERODDS_HIVEMQ); runs in integration-bridges-vendor"]
fn mqttbridge_10_hivemq_carries_qos_0_1_2_retained_messages_and_the_properties() {
    qos_retain_properties(Vendor::Hivemq);
}

#[test]
#[ignore = "needs Aedes (ZERODDS_AEDES); runs in integration-bridges-vendor"]
fn mqttbridge_10_aedes_carries_qos_0_1_2_and_retained_messages() {
    qos_retain_properties(Vendor::Aedes);
}

// ---------------------------------------------------------------------
// §9.3: the session across a lost connection on each broker
// ---------------------------------------------------------------------

/// A TCP forwarder between the daemon and a broker that the test can cut:
/// the open connections end without DISCONNECT, new ones are closed at once
/// until it is restored.
struct Proxy {
    port: u16,
    cut: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    open: Arc<Mutex<Vec<TcpStream>>>,
}

impl Proxy {
    fn start(host: &str, port: u16) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let local = listener.local_addr().unwrap().port();
        let cut = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let open: Arc<Mutex<Vec<TcpStream>>> = Arc::new(Mutex::new(Vec::new()));
        let (c, s, o) = (Arc::clone(&cut), Arc::clone(&stop), Arc::clone(&open));
        let upstream = (host.to_string(), port);
        thread::spawn(move || {
            for down in listener.incoming() {
                if s.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(down) = down else { continue };
                if c.load(Ordering::SeqCst) {
                    let _ = down.shutdown(Shutdown::Both);
                    continue;
                }
                let Ok(up) = TcpStream::connect((upstream.0.as_str(), upstream.1)) else {
                    let _ = down.shutdown(Shutdown::Both);
                    continue;
                };
                if let Ok(mut list) = o.lock() {
                    list.push(down.try_clone().unwrap());
                    list.push(up.try_clone().unwrap());
                }
                for (mut from, mut to) in [
                    (down.try_clone().unwrap(), up.try_clone().unwrap()),
                    (up, down),
                ] {
                    thread::spawn(move || {
                        let _ = std::io::copy(&mut from, &mut to);
                        let _ = to.shutdown(Shutdown::Both);
                        let _ = from.shutdown(Shutdown::Both);
                    });
                }
            }
        });
        Self {
            port: local,
            cut,
            stop,
            open,
        }
    }

    fn cut(&self) {
        self.cut.store(true, Ordering::SeqCst);
        for s in self.open.lock().unwrap().drain(..) {
            let _ = s.shutdown(Shutdown::Both);
        }
    }

    fn restore(&self) {
        self.cut.store(false, Ordering::SeqCst);
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.cut();
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

/// Spec §9.3 and §6 on the broker of `vendor`: the connection of a daemon
/// with `clean_start: false` ends without DISCONNECT; the broker publishes
/// its will on `<own prefix>/<client_id>/liveliness_lost`, keeps the
/// daemon's session with its subscription and queues the QoS 1 messages of
/// the outage; after the reconnect they reach DDS once each, in order.
fn session_resumes(vendor: Vendor) {
    let b = Broker::start(vendor, &format!("session-{}", vendor.tag()));
    let proxy = Proxy::start(&b.host, b.port);
    let id = client_id(vendor, "ses");
    let in_topic = format!("zerodds/{id}/in");
    let (mut cfg, lease) = daemon_config(&b, &id);
    cfg.broker_url = format!("mqtt://127.0.0.1:{}", proxy.port);
    cfg.clean_start = false;
    cfg.session_expiry_interval_secs = 300;
    cfg.reconnect = ReconnectConfig {
        initial_delay_ms: 200,
        max_delay_ms: 500,
        factor: 2.0,
    };
    cfg.topics.push(topic("SesIn", "Q", &in_topic, "in", 1));
    let peer = foreign_participant(lease.id(), 2);
    let rx = peer_reader(&peer, "SesIn", "volatile");
    let will_topic = format!("{}/{id}/liveliness_lost", vendor.own_prefix());
    let will = b.subscriber(&will_topic, 1, 30, "%t|%p");
    let mut handle = server::start(cfg).expect("daemon start");
    let w = handle.user_writers["SesIn"];
    wait_for(
        "the daemon's writer matched no reader",
        Duration::from_secs(15),
        || {
            !handle
                .runtime
                .user_writer_matched_reader_guids(w)
                .is_empty()
        },
    );
    b.publish(&in_topic, &[0x00, 0x01, 0x00, 0x00, 0x01], 1, &[]);
    let before = samples(&rx, Duration::from_secs(10), Duration::from_millis(300));
    assert_eq!(
        before.iter().map(payload_of).collect::<Vec<_>>(),
        [vec![0x01]],
        "{vendor:?}"
    );

    proxy.cut();
    let got = lines_of_output(will);
    assert_eq!(
        got,
        [format!("{will_topic}|{{\"client_id\":\"{id}\"}}")],
        "{vendor:?}: the will"
    );
    for i in 2..=4u8 {
        b.publish(&in_topic, &[0x00, 0x01, 0x00, 0x00, i], 1, &[]);
    }
    thread::sleep(Duration::from_millis(500));
    assert!(
        samples(&rx, Duration::from_millis(200), Duration::from_millis(200)).is_empty(),
        "{vendor:?}: a sample while the connection was cut"
    );
    proxy.restore();
    let after = samples(&rx, Duration::from_secs(15), Duration::from_millis(1500));
    assert_eq!(
        after.iter().map(payload_of).collect::<Vec<_>>(),
        [vec![0x02], vec![0x03], vec![0x04]],
        "{vendor:?}: the messages of the outage"
    );
    handle.shutdown();
}

#[test]
#[ignore = "needs mosquitto; runs in integration-bridges-vendor"]
fn mqttbridge_9_3_mosquitto_keeps_the_session_and_publishes_the_will() {
    session_resumes(Vendor::Mosquitto);
}

#[test]
#[ignore = "needs EMQX 5 (ZERODDS_EMQX_MATRIX); runs in integration-bridges-vendor"]
fn mqttbridge_9_3_emqx_keeps_the_session_and_publishes_the_will() {
    session_resumes(Vendor::Emqx);
}

#[test]
#[ignore = "needs HiveMQ 4 (ZERODDS_HIVEMQ); runs in integration-bridges-vendor"]
fn mqttbridge_9_3_hivemq_keeps_the_session_and_publishes_the_will() {
    session_resumes(Vendor::Hivemq);
}

#[test]
#[ignore = "needs Aedes (ZERODDS_AEDES); runs in integration-bridges-vendor"]
fn mqttbridge_9_3_aedes_keeps_the_session_and_publishes_the_will() {
    session_resumes(Vendor::Aedes);
}

// ---------------------------------------------------------------------
// §5.2: the retained catalog on each broker
// ---------------------------------------------------------------------

/// Spec §5.2 on the broker of `vendor`: a subscriber that comes after the
/// daemon gets its catalog on `<own prefix>/<client_id>/catalog`, retained,
/// under MQTT 5 with Content Type `application/json` and Payload Format
/// Indicator 1; after the shutdown the broker holds none.
fn catalog_kept(vendor: Vendor) {
    let b = Broker::start(vendor, &format!("catalog-{}", vendor.tag()));
    let id = client_id(vendor, "cat");
    let (mut cfg, _lease) = daemon_config(&b, &id);
    cfg.topics.push(topic(
        "Cat::Topic",
        "Cat::Topic",
        &format!("zerodds/{id}/cat"),
        "out",
        1,
    ));
    let mut handle = server::start(cfg).expect("daemon start");
    let catalog = format!("{}/{id}/catalog", vendor.own_prefix());
    let format = if b.v5() { "%t|%r|%C|%F|%p" } else { "%t|%r|%p" };
    let got = lines_of_output(b.subscriber(&catalog, 1, 8, format));
    let json = format!(
        r#"{{"topics":[{{"dds_name":"Cat::Topic","dds_type":"Cat::Topic","mqtt_topic":"zerodds/{id}/cat","qos":{{"reliability":"reliable","durability":"volatile"}}}}]}}"#
    );
    let want = if b.v5() {
        format!("{catalog}|1|application/json|1|{json}")
    } else {
        format!("{catalog}|1|{json}")
    };
    assert_eq!(got, [want], "{vendor:?}");
    handle.shutdown();
    let after = lines_of_output(b.subscriber(&catalog, 1, 2, "%p"));
    assert!(
        after.is_empty(),
        "{vendor:?} still holds a catalog: {after:?}"
    );
}

#[test]
#[ignore = "needs mosquitto; runs in integration-bridges-vendor"]
fn mqttbridge_5_2_mosquitto_keeps_the_retained_catalog() {
    catalog_kept(Vendor::Mosquitto);
}

#[test]
#[ignore = "needs EMQX 5 (ZERODDS_EMQX_MATRIX); runs in integration-bridges-vendor"]
fn mqttbridge_5_2_emqx_keeps_the_retained_catalog() {
    catalog_kept(Vendor::Emqx);
}

#[test]
#[ignore = "needs Aedes (ZERODDS_AEDES); runs in integration-bridges-vendor"]
fn mqttbridge_5_2_aedes_keeps_the_retained_catalog() {
    catalog_kept(Vendor::Aedes);
}

/// Spec §3, §5.2 with HiveMQ 4: under the default prefix `$zerodds` HiveMQ
/// refuses the CONNECT, whose will topic begins with `$`, with Reason Code
/// 0x82, and the start fails naming it; under `mqtt.own_topic_prefix:
/// zerodds` it keeps the catalog.
#[test]
#[ignore = "needs HiveMQ 4 (ZERODDS_HIVEMQ); runs in integration-bridges-vendor"]
fn mqttbridge_5_2_hivemq_refuses_the_dollar_prefix_and_keeps_the_catalog_under_another() {
    let b = Broker::start(Vendor::Hivemq, "catalog-hm-dollar");
    let id = client_id(Vendor::Hivemq, "dol");
    let (mut cfg, _lease) = daemon_config(&b, &id);
    cfg.own_topic_prefix = "$zerodds".into();
    cfg.connect_timeout_secs = 2;
    let err = match server::start(cfg) {
        Ok(mut handle) => {
            handle.shutdown();
            panic!("HiveMQ took a will under $zerodds")
        }
        Err(e) => e.to_string(),
    };
    assert!(err.contains("0x82"), "{err}");
    catalog_kept(Vendor::Hivemq);
}
