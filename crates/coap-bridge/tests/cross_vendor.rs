// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-vendor interop tests (spec `zerodds-coap-bridge-1.0.md` §10 +
//! §12.3): the daemon against CoAP clients of other implementations —
//! libcoap's `coap-client`, aiocoap, Eclipse Californium, Eclipse
//! Wakaama's LwM2M client — and against a Cyclone DDS participant on the
//! DDS side (`tests/interop/cyclone_reading.c`, built by the test with
//! `idlc` and the C compiler).
//!
//! The foreign programs are not in every build environment, so the tests
//! are `#[ignore]`d; the weekly job `integration-bridges-vendor`
//! provisions them (`tests/integration/provision.sh`) and runs them, and a
//! missing program fails a test there. Run via:
//! ```bash
//! cargo test -p zerodds-coap-bridge --features cross-vendor-tests \
//!     --test cross_vendor -- --ignored
//! ```

#![cfg(all(feature = "daemon", feature = "cross-vendor-tests"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use zerodds_coap_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_coap_bridge::daemon::server;

/// The configuration on a leased DDS domain (the host's share of the band,
/// `zerodds-test-domains`); the lease is held while the test runs.
fn make_test_config(bind: &str) -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.bind = bind.to_string();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.topics.push(TopicConfig {
        dds_name: "Trade".to_string(),
        dds_type: "Trade".to_string(),
        coap_uri_path: "trade".to_string(),
        direction: "bidir".to_string(),
        reliability: "best_effort".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
        ..Default::default()
    });
    (cfg, lease)
}

/// The libcoap client under every name Debian and upstream give it.
///
/// This looked for `coap-client` alone. Debian's `libcoap3-bin` installs one
/// binary per TLS backend — `coap-client-openssl`, `-gnutls`, `-notls` — and
/// registers no unversioned alias, so on a host with libcoap installed the
/// lookup found nothing, every test below returned early, and the suite
/// reported success with no foreign octet exchanged. Measured on the Linux test host
/// against libcoap 4.3.4.
fn libcoap_client() -> Option<String> {
    for name in [
        "coap-client",
        "coap-client-openssl",
        "coap-client-gnutls",
        "coap-client-notls",
    ] {
        if Command::new(name).arg("-h").output().is_ok() {
            return Some(name.to_string());
        }
    }
    None
}

#[test]
#[ignore = "needs libcoap (coap-client); runs in integration-bridges-vendor"]
fn coapbridge_10_libcoap_post_to_zerodds_daemon_returns_2_04_changed() {
    // Run with `--ignored` means measure: a missing client fails instead
    // of reporting interop that never happened.
    let bin = libcoap_client().expect("no libcoap client found; install libcoap3-bin");
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let port = handle.local_addr.rsplit(':').next().expect("port");

    let url = format!("coap://127.0.0.1:{port}/trade");
    // `-v 6`, not `-v 3`. Measured on the Linux test host against libcoap 4.3.4: at
    // `-v 3` the client prints the response payload and nothing else — no
    // response code on either stream — so an assertion on "2.04" at that
    // level can never hold. At `-v 6` it prints the compact message trace,
    // `v:1 t:ACK c:2.04 i:… `, on **stdout**; libcoap sends its whole log to
    // stdout, so stderr stays empty.
    let out = Command::new(&bin)
        .args(["-v", "6", "-m", "post", "-e", "AAPL@200", &url])
        .output()
        .expect("coap-client");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    eprintln!("{bin} stdout: {stdout}");
    eprintln!("{bin} stderr: {stderr}");
    assert!(
        out.status.success(),
        "{bin} should exit 0; stderr: {stderr}"
    );
    // The test is named for 2.04 Changed and nothing checked it: libcoap
    // exits 0 on any response it can parse, including 4.04. `c:2.04` is the
    // code field of the trace line for the response message.
    assert!(
        stdout.contains("c:2.04") || stderr.contains("c:2.04"),
        "expected a 2.04 Changed from the daemon; stdout={stdout} stderr={stderr}"
    );

    handle.shutdown();
}

#[test]
#[ignore = "needs libcoap (coap-client); runs in integration-bridges-vendor"]
fn coapbridge_10_libcoap_get_well_known_core_returns_catalog() {
    // Run with `--ignored` means measure: a missing client fails instead
    // of reporting interop that never happened.
    let bin = libcoap_client().expect("no libcoap client found; install libcoap3-bin");
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let port = handle.local_addr.rsplit(':').next().expect("port");
    let url = format!("coap://127.0.0.1:{port}/.well-known/core");
    let out = Command::new(&bin)
        .args(["-m", "get", &url])
        .output()
        .expect("coap-client");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{bin} should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("</trade>"),
        "expected </trade> in catalog: {stdout}"
    );
    handle.shutdown();
}

#[test]
#[ignore = "needs libcoap (coap-client); runs in integration-bridges-vendor"]
fn coapbridge_10_libcoap_observe_receives_notification() {
    // Run with `--ignored` means measure: a missing client fails instead
    // of reporting interop that never happened.
    let bin = libcoap_client().expect("no libcoap client found; install libcoap3-bin");
    let (cfg, _lease) = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let port = handle.local_addr.rsplit(':').next().expect("port");
    let url = format!("coap://127.0.0.1:{port}/trade");
    // Observe for 2 seconds. `-s <duration>` is libcoap's Observe flag and
    // is the whole of it; `-o` is `[-o file]`, an output-file argument.
    // `-s 2 -o <url>` therefore consumed the URL as a filename, left the
    // client with no address, and it exited on its usage message —
    // measured against coap-client-openssl 4.3.4.
    let out = Command::new(&bin)
        .args(["-v", "6", "-s", "2", &url])
        .output()
        .expect("coap-client");
    // This was `let _ = out;` under a comment reading "Functional assert:
    // process exited cleanly" — there was no assert, so the test passed
    // whatever libcoap did, including failing to reach the daemon at all.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "{bin} -s should exit 0; stdout={stdout} stderr={stderr}"
    );
    // A server that does not implement Observe answers the same request with
    // a plain 2.05 and no Observe option, and libcoap exits 0 on it — so a
    // non-empty stdout proves nothing. RFC 7641 §4.2: the registration is
    // accepted only if the **response** carries the Observe option. At
    // `-v 6` libcoap prints one trace line per message; the request line
    // carries `Observe:` too, so the response line has to be picked out by
    // its code.
    let accepted = stdout
        .lines()
        .any(|l| l.contains("c:2.05") && l.contains("Observe"));
    assert!(
        accepted,
        "the daemon must accept the Observe registration and answer with the \
         Observe option (RFC 7641 §4.2); stdout={stdout} stderr={stderr}"
    );
    handle.shutdown();
}

/// The assets of these tests: the IDL, the Cyclone DDS participant, the
/// aiocoap and Californium clients.
fn interop_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/interop")
}

/// A daemon with the typed topic `SensorReading` (type `Sensor::Reading`
/// from `tests/interop/reading.idl`) at `sensors/reading`, RELIABLE.
fn typed_config() -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.bind = "127.0.0.1:0".to_string();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.topics.push(TopicConfig {
        dds_name: "SensorReading".to_string(),
        dds_type: "Sensor::Reading".to_string(),
        coap_uri_path: "sensors/reading".to_string(),
        direction: "bidir".to_string(),
        reliability: "reliable".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
        history_kind: "keep_last".to_string(),
        idl_file: interop_dir().join("reading.idl").display().to_string(),
        extensibility: "appendable".to_string(),
        data_representation: "xcdr2".to_string(),
        ..Default::default()
    });
    (cfg, lease)
}

fn reading_uri(handle: &server::DaemonHandle) -> String {
    let port = handle.local_addr.rsplit(':').next().expect("port");
    format!("coap://127.0.0.1:{port}/sensors/reading")
}

/// The lines a child wrote to stdout until it ended, read on a thread.
fn lines_of(child: &mut Child) -> std::sync::mpsc::Receiver<String> {
    let out = child.stdout.take().expect("piped stdout");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    rx
}

/// The `<code> <payload>` line of a client: its code and its payload as
/// JSON (none for an empty payload).
fn code_and_json(line: &str) -> (String, Option<serde_json::Value>) {
    let (code, payload) = line.split_once(' ').unwrap_or((line, ""));
    let json = (!payload.trim().is_empty())
        .then(|| serde_json::from_str(payload.trim()).unwrap_or_else(|e| panic!("{line}: {e}")));
    (code.to_string(), json)
}

/// The three exchanges of §10 through a foreign client: a POST of a JSON
/// sample (2.04), a GET of it in JSON (2.05, the same sample), and an
/// observation that gets the next sample as a notification in JSON.
/// `run` starts the client with `post URI JSON`, `get URI` or `observe URI
/// SECONDS`, each answer a `<code> <payload>` line on stdout.
fn post_get_observe(run: &dyn Fn(&[&str]) -> Command) {
    let (cfg, _lease) = typed_config();
    let mut handle = server::start(cfg).expect("daemon start");
    let uri = reading_uri(&handle);
    let first = serde_json::json!({ "id": 7, "value": 1.5, "unit": "V" });
    let out = run(&["post", &uri, &first.to_string()])
        .output()
        .expect("post");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        out.status.success(),
        "post: {text} {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        code_and_json(text.lines().last().unwrap_or("")).0,
        "2.04",
        "{text}"
    );
    // The daemon's own reader has the sample, then the GET gives it.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let out = run(&["get", &uri]).output().expect("get");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let (code, json) = code_and_json(text.lines().last().unwrap_or(""));
        if code == "2.05" && json.as_ref() == Some(&first) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the GET never gave the sample: {text}"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    let mut observer = run(&["observe", &uri, "8"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("observe");
    let lines = lines_of(&mut observer);
    let registered = lines
        .recv_timeout(Duration::from_secs(15))
        .expect("the first response");
    assert_eq!(code_and_json(&registered).0, "2.05", "{registered}");
    let next = serde_json::json!({ "id": 7, "value": 2.5, "unit": "V" });
    let out = run(&["post", &uri, &next.to_string()])
        .output()
        .expect("post");
    assert!(out.status.success());
    let deadline = Instant::now() + Duration::from_secs(10);
    let notified = loop {
        let left = deadline.saturating_duration_since(Instant::now());
        let line = lines.recv_timeout(left).expect("a notification");
        let (code, json) = code_and_json(&line);
        if json.as_ref() == Some(&next) {
            break code;
        }
    };
    assert_eq!(notified, "2.05");
    let _ = observer.wait();
    handle.shutdown();
}

#[test]
#[ignore = "needs aiocoap (python3-aiocoap); runs in integration-bridges-vendor"]
fn coapbridge_10_aiocoap_posts_reads_and_observes_json() {
    let ok = Command::new("python3")
        .args(["-c", "import aiocoap"])
        .status()
        .is_ok_and(|s| s.success());
    assert!(ok, "no aiocoap; install python3-aiocoap");
    let script = interop_dir().join("aiocoap_client.py");
    post_get_observe(&|args: &[&str]| {
        let mut c = Command::new("python3");
        c.arg(&script).args(args).env("PYTHONUNBUFFERED", "1");
        c
    });
}

/// The classpath of Californium: `CALIFORNIUM_CP`, or the jars under
/// `/opt/californium` (tests/integration/provision.sh).
fn californium_classpath() -> String {
    if let Ok(cp) = std::env::var("CALIFORNIUM_CP") {
        return cp;
    }
    let jars: Vec<String> = std::fs::read_dir("/opt/californium")
        .expect("no /opt/californium; provision californium")
        .filter_map(Result::ok)
        .map(|e| e.path().display().to_string())
        .filter(|p| p.ends_with(".jar"))
        .collect();
    assert!(
        !jars.is_empty(),
        "no Californium jars under /opt/californium"
    );
    jars.join(":")
}

#[test]
#[ignore = "needs Eclipse Californium and a JDK; runs in integration-bridges-vendor"]
fn coapbridge_10_californium_posts_reads_and_observes_json() {
    let classpath = californium_classpath();
    let source = interop_dir().join("CfClient.java");
    post_get_observe(&|args: &[&str]| {
        let mut c = Command::new("java");
        c.arg("-cp").arg(&classpath).arg(&source).args(args);
        c
    });
}

/// Eclipse Wakaama's LwM2M client against the daemon (§10: the LwM2M stack
/// on its CoAP layer). Its registration, a Confirmable `POST /rd` with
/// Uri-Query and a link-format body, reaches the daemon, which serves `rd`
/// as a topic without a writer and answers `4.05` piggybacked; Wakaama reads
/// the answer, which carries its Message ID, and sends the registration
/// once.
#[test]
#[ignore = "needs Eclipse Wakaama's lwm2mclient; runs in integration-bridges-vendor"]
fn coapbridge_10_wakaama_registration_is_read_and_answered() {
    let client = std::env::var("WAKAAMA_LWM2MCLIENT")
        .unwrap_or_else(|_| "/opt/wakaama/lwm2mclient".to_string());
    assert!(
        std::path::Path::new(&client).is_file(),
        "no {client}; provision wakaama"
    );
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.bind = "127.0.0.1:0".to_string();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.topics.push(TopicConfig {
        dds_name: "LwM2MRegistration".to_string(),
        coap_uri_path: "rd".to_string(),
        direction: "out".to_string(),
        ..Default::default()
    });
    let mut handle = server::start(cfg).expect("daemon start");
    let port = handle
        .local_addr
        .rsplit(':')
        .next()
        .expect("port")
        .to_string();
    let local = std::net::UdpSocket::bind("127.0.0.1:0")
        .and_then(|s| s.local_addr())
        .expect("a free port")
        .port()
        .to_string();
    // Its trace of each datagram goes to stderr, its states to stdout: both
    // into one pipe.
    let mut child = Command::new("sh")
        .args(["-c", "exec \"$0\" \"$@\" 2>&1"])
        .arg(&client)
        .args([
            "-4",
            "-h",
            "127.0.0.1",
            "-p",
            &port,
            "-n",
            "zerodds-interop",
            "-l",
            &local,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("lwm2mclient");
    let lines = lines_of(&mut child);
    // lwm2mclient buffers its stdout in a pipe; SIGINT ends it through its
    // handler, which flushes.
    std::thread::sleep(Duration::from_secs(5));
    let _ = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status();
    let _ = child.wait();
    let text: Vec<String> = lines.try_iter().collect();
    handle.shutdown();
    let text = text.join("\n");
    // Wakaama prints each datagram as a hex dump after "Sending N bytes"
    // and "N bytes received".
    let first_hex = |marker: &str| -> Vec<u8> {
        let at = text
            .find(marker)
            .unwrap_or_else(|| panic!("no {marker:?} in:\n{text}"));
        let line = text[at..].lines().nth(1).unwrap_or("");
        line.split_whitespace()
            .take(4)
            .map(|b| u8::from_str_radix(b, 16).unwrap_or_else(|_| panic!("{line}")))
            .collect()
    };
    let sent = first_hex("Sending ");
    let received = first_hex(" bytes received from");
    // RFC 7252 §3: version 1, CON, POST; the answer: version 1, ACK, 4.05,
    // the same Message ID.
    assert_eq!(sent[0] >> 4, 0x4, "CON: {text}");
    assert_eq!(sent[1], 0x02, "POST");
    assert_eq!(received[0] >> 4, 0x6, "ACK: {text}");
    assert_eq!(received[1], (4 << 5) | 5, "4.05 Method Not Allowed: {text}");
    assert_eq!(received[2..4], sent[2..4], "the same Message ID");
    assert_eq!(
        text.matches("Sending ").count(),
        1,
        "the registration went out once: {text}"
    );
}

/// The libcoap client.
fn libcoap() -> String {
    libcoap_client().expect("no libcoap client found; install libcoap3-bin")
}

/// Spec §12.3: a Cyclone DDS participant and a libcoap client against the
/// daemon. A JSON sample POSTed by libcoap reaches a Cyclone DDS reader of
/// `Sensor::Reading` as that sample; a sample a Cyclone DDS writer writes
/// reaches libcoap's observation as a JSON notification.
#[test]
#[ignore = "needs Cyclone DDS (cyclonedds-dev, idlc), a C compiler and libcoap; runs in integration-bridges-vendor"]
fn coapbridge_12_3_a_coap_post_reaches_cyclone_and_a_cyclone_sample_an_observer() {
    let coap = libcoap();
    // The Cyclone DDS participant, built from its sources.
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cyclone-reading");
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
    let cyclone = dir.join("cyclone_reading");

    let (cfg, lease) = typed_config();
    let domain = lease.id().to_string();
    let mut handle = server::start(cfg).expect("daemon start");
    let uri = reading_uri(&handle);

    // CoAP → DDS: libcoap POSTs, Cyclone DDS takes.
    let mut sub = Command::new(&cyclone)
        .args(["sub", &domain, "20"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("cyclone sub");
    let lines = lines_of(&mut sub);
    assert_eq!(
        lines.recv_timeout(Duration::from_secs(10)).expect("ready"),
        "{\"event\":\"ready\"}"
    );
    let value = serde_json::json!({ "id": 9, "value": 4.5, "unit": "W" });
    let deadline = Instant::now() + Duration::from_secs(15);
    let taken = loop {
        // Until the daemon's writer and the Cyclone reader have matched.
        let out = Command::new(&coap)
            .args(["-m", "post", "-t", "50", "-e", &value.to_string(), &uri])
            .output()
            .expect("coap-client post");
        assert!(out.status.success());
        if let Ok(line) = lines.recv_timeout(Duration::from_secs(1)) {
            break line;
        }
        assert!(Instant::now() < deadline, "no sample reached Cyclone DDS");
    };
    let taken: serde_json::Value = serde_json::from_str(&taken).expect("a JSON line");
    assert_eq!(taken, value);
    let _ = sub.kill();
    let _ = sub.wait();

    // DDS → CoAP: Cyclone DDS writes, libcoap's observation gets it.
    // libcoap writes the payloads one after the other without a line
    // break; its output is read as a stream of JSON values once it ends.
    let observer = Command::new(&coap)
        .args(["-s", "6", "-A", "50", &uri])
        .stdout(Stdio::piped())
        .spawn()
        .expect("coap-client observe");
    std::thread::sleep(Duration::from_secs(1));
    let written = Command::new(&cyclone)
        .args(["pub", &domain, "10", "5.5", "Hz"])
        .output()
        .expect("cyclone pub");
    assert!(
        written.status.success(),
        "{}",
        String::from_utf8_lossy(&written.stderr)
    );
    let out = observer.wait_with_output().expect("coap-client observe");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let values: Vec<serde_json::Value> = serde_json::Deserializer::from_str(&text)
        .into_iter::<serde_json::Value>()
        .map_while(Result::ok)
        .collect();
    let want = serde_json::json!({ "id": 10, "value": 5.5, "unit": "Hz" });
    assert!(
        values.contains(&want),
        "libcoap's observation did not get Cyclone DDS's sample: {text}"
    );
    handle.shutdown();
}
