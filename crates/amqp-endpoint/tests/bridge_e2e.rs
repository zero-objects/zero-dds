// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// E2E tests of the `zerodds-amqp-bridged` command line: exit codes, the
// refusals before any traffic flows. The traffic itself is measured against
// RabbitMQ in `bridge_rabbitmq_e2e.rs`.
//
// Spec: `docs/specs/zerodds-amqp-bridge-daemon-1.0.md` §12.2.

#![cfg(feature = "std")]
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

#[path = "support/scripted_broker.rs"]
mod scripted_broker;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use scripted_broker::Broker;
use zerodds_amqp_bridge::performatives::descriptor;

const AMQP_PROTOCOL_HEADER: [u8; 8] = [b'A', b'M', b'Q', b'P', 0x00, 0x01, 0x00, 0x00];

fn daemon_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-amqp-bridged"))
}

fn pick_port_listener() -> (TcpListener, String) {
    let l = TcpListener::bind("127.0.0.1:0").expect("ephemeral bind");
    let p = l.local_addr().expect("addr").port();
    (l, format!("127.0.0.1:{p}"))
}

/// A peer that answers the SASL header with the bare AMQP header: an
/// AMQP 1.0 peer without the SASL layer. The daemon must not continue
/// unauthenticated; it stops with the reason.
#[test]
fn amqp10_0_2_a_broker_without_the_sasl_layer_is_refused_by_name() {
    let (listener, addr) = pick_port_listener();
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let (mut s, _) = listener.accept().expect("accept");
        let mut hdr = [0u8; 8];
        let _ = s.read_exact(&mut hdr);
        let _ = s.write_all(&AMQP_PROTOCOL_HEADER);
        let _ = tx.send(hdr.to_vec());
        // Held open until the daemon closes its end.
        let _ = s.read(&mut [0u8; 1]);
    });
    let out = Command::new(daemon_binary())
        .args([
            "--broker",
            &format!("amqp://{addr}"),
            "--sasl-mechanism",
            "ANONYMOUS",
            "--once",
            "--connect-timeout-ms",
            "3000",
        ])
        .output()
        .expect("spawn daemon");
    let first = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("the daemon connected");
    assert_eq!(&first[..], b"AMQP\x03\x01\x00\x00", "SASL header first");
    assert_eq!(out.status.code(), Some(5), "SASL failure (spec §2)");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("does not offer SASL"), "{stderr}");
}

/// PLAIN sends the password in the clear; towards a remote broker the
/// daemon refuses it without TLS (spec §7.2).
#[test]
fn amqpbridge_2_plain_without_tls_is_refused_towards_a_remote_broker() {
    let out = Command::new(daemon_binary())
        .args([
            "--broker",
            "amqp://broker.example.com",
            "--sasl-mechanism",
            "PLAIN",
            "--user",
            "u",
            "--password",
            "p",
            "--once",
        ])
        .output()
        .expect("spawn daemon");
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("amqps://"));
}

/// ANONYMOUS is for a broker on the loopback interface; towards any other
/// the daemon refuses to start (spec §7.2), before it connects, also with
/// TLS.
#[test]
fn amqpbridge_7_2_anonymous_towards_a_remote_broker_is_refused() {
    for broker in ["amqp://broker.example.com", "amqp://192.0.2.1:5672"] {
        let out = Command::new(daemon_binary())
            .args([
                "--broker",
                broker,
                "--sasl-mechanism",
                "ANONYMOUS",
                "--once",
            ])
            .output()
            .expect("spawn daemon");
        assert_eq!(out.status.code(), Some(1), "{broker}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("ANONYMOUS is allowed only towards a loopback broker"),
            "{broker}: {stderr}"
        );
    }
}

#[test]
fn amqpbridge_2_daemon_with_no_broker_fails_with_exit_2() {
    // Connect to a port that's almost certainly closed.
    let (listener, addr) = pick_port_listener();
    drop(listener); // free the port.
    let mut child = Command::new(daemon_binary())
        .args([
            "--broker",
            &format!("amqp://{addr}"),
            "--once",
            "--connect-timeout-ms",
            "300",
        ])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    let status = child.wait().expect("wait");
    // exit 2 = broker connect failed (Spec §2 exit code).
    assert_eq!(status.code(), Some(2), "expected exit 2 for connect-fail");
}

#[test]
fn amqpbridge_2_version_flag_emits_one_line_and_exits_zero() {
    let out = Command::new(daemon_binary())
        .arg("--version")
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("zerodds-amqp-bridged"));
    assert!(s.contains("1.0"));
}

#[test]
fn amqpbridge_2_missing_broker_url_fails_with_exit_1() {
    let mut child = Command::new(daemon_binary())
        .args(["--once"])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn");
    let status = child.wait().expect("wait");
    assert_eq!(status.code(), Some(1), "expected exit 1 for missing broker");
}

/// Waits for the daemon's exit, at most `within`; kills it after that.
fn wait_exit(child: &mut Child, within: Duration) -> ExitStatus {
    let deadline = Instant::now() + within;
    loop {
        if let Some(status) = child.try_wait().expect("try_wait") {
            return status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            panic!("the daemon did not exit within {within:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// §2 exit 4: the broker's certificate is issued by a CA the daemon does
/// not trust. The TLS handshake fails, and the daemon ends with 4 at the
/// first connect, without the test hook `--once`: a retry would not cure it.
/// Sends `signal` to the daemon, reads the broker side of `conn` until
/// the daemon's CLOSE and waits for the daemon's exit 0. Returns the codes
/// of the frames read and the daemon's stderr.
fn stop_until_close(
    child: &mut Child,
    conn: &mut scripted_broker::Conn,
    signal: libc::c_int,
) -> (Vec<u64>, String) {
    #[allow(unsafe_code)]
    // SAFETY: kill(2) takes a pid and a signal number and touches no memory
    // of this process. `child` is not reaped before this call (`wait_exit`
    // reaps it afterwards), so its pid still names the daemon this test
    // spawned and not a process that took the pid over.
    let rc = unsafe { libc::kill(child.id() as libc::pid_t, signal) };
    assert_eq!(rc, 0);
    let mut seen = Vec::new();
    while let Some(f) = conn.try_next(Duration::from_secs(10)) {
        seen.push(f.code);
        if f.code == descriptor::CLOSE {
            break;
        }
    }
    let status = wait_exit(child, Duration::from_secs(15));
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(status.code(), Some(0), "{stderr}");
    (seen, stderr)
}

/// The frames of a shutdown with two links: a DETACH and an END each,
/// and the CLOSE last.
fn assert_both_links_closed(seen: &[u64]) {
    let count = |c: u64| seen.iter().filter(|x| **x == c).count();
    assert_eq!(count(descriptor::DETACH), 2, "{seen:x?}");
    assert_eq!(count(descriptor::END), 2, "{seen:x?}");
    assert_eq!(seen.last(), Some(&descriptor::CLOSE), "{seen:x?}");
}

/// A TLS server with the certificate `cert` and its key `key`, ALPN
/// `amqp`, no client authentication.
fn amqp_tls_server(
    cert: &rcgen::Certificate,
    key: &rcgen::KeyPair,
) -> std::sync::Arc<rustls::ServerConfig> {
    let provider = std::sync::Arc::new(zerodds_bridge_security::tls_provider());
    let mut cfg = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
        )
        .unwrap();
    cfg.alpn_protocols = vec![b"amqp".to_vec()];
    std::sync::Arc::new(cfg)
}

#[test]
fn amqpbridge_2_a_tls_handshake_that_fails_is_exit_4() {
    let issue_ca = || {
        let key = rcgen::KeyPair::generate().unwrap();
        let mut p = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let cert = p.self_signed(&key).unwrap();
        (cert, key)
    };
    let (trusted, _) = issue_ca();
    let (other, other_key) = issue_ca();
    let server_key = rcgen::KeyPair::generate().unwrap();
    let server_cert = rcgen::CertificateParams::new(vec!["localhost".to_string()])
        .unwrap()
        .signed_by(&server_key, &other, &other_key)
        .unwrap();
    let ca_file =
        std::env::temp_dir().join(format!("amqpbridge-2-exit4-{}.pem", std::process::id()));
    std::fs::write(&ca_file, trusted.pem()).unwrap();

    let cfg = amqp_tls_server(&server_cert, &server_key);
    let (listener, addr) = pick_port_listener();
    let port = addr.rsplit(':').next().unwrap().to_string();
    // Every accepted connection counts, whether its handshake ends or not.
    let accepted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = std::sync::Arc::clone(&accepted);
    std::thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            match listener.accept() {
                Ok((mut sock, _)) => {
                    counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    sock.set_nonblocking(false).unwrap();
                    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                    let mut conn = rustls::ServerConnection::new(cfg.clone()).unwrap();
                    while conn.is_handshaking() {
                        if conn.complete_io(&mut sock).is_err() {
                            break;
                        }
                    }
                }
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    });
    let mut child = Command::new(daemon_binary())
        .args([
            "--broker",
            &format!("amqps://localhost:{port}"),
            "--tls-ca",
            ca_file.to_str().unwrap(),
            "--sasl-mechanism",
            "PLAIN",
            "--user",
            "u",
            "--password",
            "p",
            "--connect-timeout-ms",
            "3000",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon");
    let status = wait_exit(&mut child, Duration::from_secs(15));
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    let _ = std::fs::remove_file(&ca_file);
    assert_eq!(status.code(), Some(4), "TLS error (spec §2): {stderr}");
    assert!(stderr.contains("tls"), "{stderr}");
    // A second connect of the daemon before its exit would wait in the
    // listen backlog; the accept loop polls every 10 ms and takes it within
    // this pause.
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        accepted.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "one attempt"
    );
}

/// §2 exit 5 without `--once`: the broker offers only PLAIN, the daemon is
/// configured for ANONYMOUS; the SASL exchange cannot succeed, and the
/// daemon ends with 5 instead of trying again.
#[test]
fn amqpbridge_2_a_refused_sasl_exchange_is_exit_5_without_once() {
    let broker = Broker::bind();
    let url = broker.url();
    let server = std::thread::spawn(move || {
        let mut conn = broker.accept(Duration::from_secs(10));
        let chosen = conn.sasl(&["PLAIN"], 1);
        conn.expect_eof(Duration::from_secs(10));
        chosen
    });
    let mut child = Command::new(daemon_binary())
        .args(["--broker", &url, "--sasl-mechanism", "ANONYMOUS"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon");
    let status = wait_exit(&mut child, Duration::from_secs(15));
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(status.code(), Some(5), "SASL error (spec §2): {stderr}");
    assert!(stderr.contains("ANONYMOUS"), "{stderr}");
    assert_eq!(server.join().unwrap(), "", "no mechanism was tried");
}

/// §2 exit 6: the broker answers the open with a close (here
/// `amqp:unauthorized-access`, as a broker does for an unknown virtual
/// host); the daemon ends with 6 without `--once`.
#[test]
fn amqpbridge_2_a_refused_open_is_exit_6() {
    let broker = Broker::bind();
    let url = broker.url();
    let server = std::thread::spawn(move || {
        let mut conn = broker.accept(Duration::from_secs(10));
        assert_eq!(conn.sasl(&["ANONYMOUS"], 0), "ANONYMOUS");
        let open = conn.read_open();
        conn.close("amqp:unauthorized-access");
        conn.expect_eof(Duration::from_secs(10));
        open.text(1)
    });
    let mut child = Command::new(daemon_binary())
        .args(["--broker", &url, "--set", "amqp.hostname=vhost:missing"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon");
    let status = wait_exit(&mut child, Duration::from_secs(15));
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(status.code(), Some(6), "open refused (spec §2): {stderr}");
    assert!(stderr.contains("amqp:unauthorized-access"), "{stderr}");
    assert_eq!(server.join().unwrap().as_deref(), Some("vhost:missing"));
}

/// §2 exit 3: a DDS participant that cannot start (domain 300 has no UDP
/// port under the default port mapping, DDSI-RTPS 2.5 §9.6.1.1).
#[test]
fn amqpbridge_2_a_participant_that_cannot_start_is_exit_3() {
    let out = Command::new(daemon_binary())
        .args(["--broker", "amqp://127.0.0.1:1", "--domain", "300"])
        .output()
        .expect("spawn daemon");
    assert_eq!(
        out.status.code(),
        Some(3),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stderr).contains("DDS participant"));
}

/// §2: an option is `--set` of its file key, and `--check-config` prints
/// every value with where it came from; `--topic DDS=ADDR` is an entry of
/// `topics`.
#[test]
fn amqpbridge_2_set_and_check_config_print_the_origin() {
    let out = Command::new(daemon_binary())
        .args([
            "--check-config",
            "--container-id",
            "cid1",
            "--set",
            "amqp.channel_max=9",
            "--topic",
            "A=/queues/a",
        ])
        .output()
        .expect("spawn daemon");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(out.status.code(), Some(0), "{text}");
    for line in [
        "amqp.container_id = cid1  (command line `--set amqp.container_id=cid1`)",
        "amqp.channel_max = 9  (command line `--set amqp.channel_max=9`)",
        "topics[0] = A=/queues/a  (command line `--set +topics=A=/queues/a`)",
        "configuration valid",
    ] {
        assert!(text.contains(line), "missing `{line}` in\n{text}");
    }
}

/// §2, §7.2: `--check-config` shows `***` in place of the shared access key
/// of claims-based security, from a file and from `--set` (whose origin
/// names the flag), and shows the token lifetime beside it.
#[test]
fn amqpbridge_2_check_config_masks_the_cbs_secret() {
    let dir = std::env::temp_dir().join(format!("amqp-e2e-mask-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bridge.yaml");
    std::fs::write(
        &file,
        "amqp:\n  broker_url: \"amqp://127.0.0.1:5672\"\n  cbs:\n    enabled: true\n    key_name: kn\n    secret: from-the-file-4711\n    ttl_s: 120\n",
    )
    .unwrap();
    let check = |extra: &[&str]| {
        let out = Command::new(daemon_binary())
            .args(["--check-config", "--config", file.to_str().unwrap()])
            .args(extra)
            .output()
            .expect("spawn daemon");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(out.status.code(), Some(0), "{text}");
        text
    };
    let from_file = check(&[]);
    assert!(!from_file.contains("from-the-file-4711"), "{from_file}");
    assert!(
        from_file
            .lines()
            .any(|l| l.trim_start().starts_with("amqp.cbs.secret = ***  (")),
        "{from_file}"
    );
    assert!(
        from_file
            .lines()
            .any(|l| l.trim_start().starts_with("amqp.cbs.ttl_s = 120  (")),
        "{from_file}"
    );
    let from_flag = check(&["--set", "amqp.cbs.secret=from-the-flag-0815"]);
    assert!(!from_flag.contains("from-the-flag-0815"), "{from_flag}");
    assert!(
        from_flag.contains("amqp.cbs.secret = ***  (command line `--set amqp.cbs.secret=***`)"),
        "{from_flag}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// §2: `--help` names the environment layer of both secrets.
#[test]
fn amqpbridge_2_help_names_the_environment_of_both_secrets() {
    let out = Command::new(daemon_binary())
        .arg("--help")
        .output()
        .expect("spawn daemon");
    assert_eq!(out.status.code(), Some(0));
    let text = String::from_utf8_lossy(&out.stderr);
    for name in [
        "$AMQP_PASSWORD for amqp.sasl.password",
        "$AMQP_CBS_SECRET for",
        "amqp.cbs.secret",
    ] {
        assert!(text.contains(name), "`{name}` in\n{text}");
    }
}

/// §2: `--topic DDS=ADDR` adds a mapping the daemon attaches (both
/// directions, the address as node), and SIGTERM is a normal shutdown:
/// detach, end, close, exit 0 (§9.2).
#[cfg(unix)]
#[test]
fn amqpbridge_2_topic_flag_adds_a_mapping_and_sigterm_exits_0() {
    let broker = Broker::bind();
    let domain = zerodds_test_vendor_reader::lease_domain();
    let mut child = Command::new(daemon_binary())
        .args([
            "--broker",
            &broker.url(),
            "--container-id",
            "zd-e2e-sigterm",
            "--domain",
            &domain.id().to_string(),
            "--topic",
            "E2eTopic=/queues/e2e",
            "--set",
            "catalog.enabled=false",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon");
    let mut conn = broker.accept(Duration::from_secs(10));
    conn.handshake();
    let mut links = conn.setup_links(2, 0);
    links.sort_by(|a, b| a.name.cmp(&b.name));
    assert_eq!(links[0].name, "zd-e2e-sigterm/E2eTopic/receiver");
    assert_eq!(links[1].name, "zd-e2e-sigterm/E2eTopic/sender");
    assert!(
        links.iter().all(|l| l.address == "/queues/e2e"),
        "{links:?}"
    );
    let (seen, _) = stop_until_close(&mut child, &mut conn, libc::SIGTERM);
    assert_both_links_closed(&seen);
}

/// A collector of OTLP/HTTP on a port of its own: every request it reads
/// is appended to the text it returns.
fn otlp_collector() -> (u16, std::sync::Arc<std::sync::Mutex<String>>) {
    let collector = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = collector.local_addr().unwrap().port();
    let received = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let r = std::sync::Arc::clone(&received);
    std::thread::spawn(move || {
        for mut stream in collector.incoming().flatten() {
            stream
                .set_read_timeout(Some(Duration::from_millis(500)))
                .unwrap();
            let mut text = Vec::new();
            let _ = stream.read_to_end(&mut text);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n");
            r.lock().unwrap().push_str(&String::from_utf8_lossy(&text));
        }
    });
    (port, received)
}

/// The program's own lines (§8.1), written before a participant runs.
const PROGRAM_EVENTS: [&str; 4] = [
    "startup",
    "admin_endpoint",
    "admin_endpoint_bind_failed",
    "signal_watcher_init_failed",
];

/// §8.1: every line on stdout is one JSON object that begins with
/// `timestamp`; every line but the program's own carries the
/// participant's `container_id`, `startup` names it as a field.
fn every_line_is_one_of_the_program_or_the_participant(lines: &[String], container_id: &str) {
    let id = format!("\"container_id\":\"{container_id}\"");
    for l in lines {
        assert!(l.starts_with("{\"timestamp\":\""), "{l}");
        assert!(l.ends_with('}'), "{l}");
        let program = PROGRAM_EVENTS
            .iter()
            .any(|e| l.contains(&format!("\"event\":\"{e}\"")));
        if !program || l.contains("\"event\":\"startup\"") {
            assert!(l.contains(&id), "{id} in {l}");
        }
    }
}

/// §8.1: the log goes to stdout, one JSON object per line with
/// `timestamp`, `level`, `event` and the participant's `container_id`; a
/// link event names `link_name`, `dds_topic`, `amqp_address`; with
/// `--log-level debug` a transfer the broker sends is the line
/// `transfer_in` with `delivery_id`, `bytes` and `latency_us`. §8.3: with
/// `OTEL_EXPORTER_OTLP_ENDPOINT` the program sends the transfer's spans
/// `amqp.transfer` and `dds.write` to the collector, at the latest when it
/// stops.
#[cfg(unix)]
#[test]
fn amqpbridge_8_1_8_3_the_log_lines_on_stdout_and_the_spans_of_a_transfer() {
    use std::io::BufRead;
    use zerodds_amqp_endpoint::bridge_daemon::{DaemonConfig, OutSample, encode_message};
    use zerodds_amqp_endpoint::coexistence::CoexistenceConfig;
    use zerodds_amqp_endpoint::properties::DdsOperation;

    let broker = Broker::bind();
    let domain = zerodds_test_vendor_reader::lease_domain();
    let (cport, received) = otlp_collector();
    let dir = std::env::temp_dir().join(format!("amqp-e2e-log-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let yaml = format!(
        "domain: {}\nlog_level: debug\namqp:\n  broker_url: \"{}\"\n  container_id: zd-e2e-log\ntopics:\n  - dds_name: Log81\n    amqp_address: /queues/log81\n    direction: in\ncatalog:\n  enabled: false\n",
        domain.id(),
        broker.url()
    );
    let file = dir.join("bridge.yaml");
    std::fs::write(&file, &yaml).unwrap();
    let mut child = Command::new(daemon_binary())
        .args(["--config", file.to_str().unwrap()])
        .env(
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            format!("http://127.0.0.1:{cport}"),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon");
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
        {
            let _ = tx.send(line);
        }
    });
    let mut conn = broker.accept(Duration::from_secs(10));
    conn.handshake();
    let links = conn.setup_links(1, 0);
    let link = &links[0];
    conn.next_of(descriptor::FLOW);
    let mut c = zerodds_config::Config::default();
    c.push(
        zerodds_config::parse_str(&yaml, "<test>").unwrap(),
        "<test>".into(),
    )
    .unwrap();
    let cfg = DaemonConfig::from_config(Some(&c)).unwrap();
    let m = encode_message(
        &cfg.topics[0],
        domain.id(),
        &CoexistenceConfig::new("99999999-2222-4333-8444-555555555555".into()),
        &OutSample {
            payload: &[1, 0, 0, 0],
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
    .unwrap();
    conn.transfer(link, 0, false, &m);
    let d = conn.next_of(descriptor::DISPOSITION);
    assert_eq!(d.uint(1), Some(0));
    // The line of the transfer, then a normal stop.
    let mut lines = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !lines
        .iter()
        .any(|l: &String| l.contains("\"event\":\"transfer_in\""))
    {
        let left = deadline.saturating_duration_since(Instant::now());
        lines.push(rx.recv_timeout(left).expect("the transfer_in line"));
    }
    let (_, stderr) = stop_until_close(&mut child, &mut conn, libc::SIGTERM);
    while let Ok(l) = rx.recv_timeout(Duration::from_millis(500)) {
        lines.push(l);
    }
    assert!(
        !stderr.contains("\"event\""),
        "no log line on stderr: {stderr}"
    );
    every_line_is_one_of_the_program_or_the_participant(&lines, "zd-e2e-log");
    let event = |name: &str| -> &String {
        lines
            .iter()
            .find(|l| l.contains(&format!("\"event\":\"{name}\"")))
            .unwrap_or_else(|| panic!("no {name} line in {lines:#?}"))
    };
    assert!(event("startup").contains("\"container_id\":\"zd-e2e-log\""));
    let attached = event("attached");
    for field in [
        "\"level\":\"info\"",
        "\"container_id\":\"zd-e2e-log\"",
        "\"link_name\":\"zd-e2e-log/Log81/receiver\"",
        "\"dds_topic\":\"Log81\"",
        "\"amqp_address\":\"/queues/log81\"",
    ] {
        assert!(attached.contains(field), "{field} in {attached}");
    }
    let t = event("transfer_in");
    for field in [
        "\"level\":\"debug\"".to_string(),
        "\"container_id\":\"zd-e2e-log\"".to_string(),
        "\"dds_topic\":\"Log81\"".to_string(),
        "\"delivery_id\":0".to_string(),
        format!("\"bytes\":{}", m.len()),
        "\"outcome\":\"accepted\"".to_string(),
        "\"latency_us\":".to_string(),
    ] {
        assert!(t.contains(&field), "{field} in {t}");
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let text = received.lock().unwrap().clone();
        if text.contains("\"amqp.transfer\"") && text.contains("\"dds.write\"") {
            assert!(text.contains("POST /v1/traces"), "{text}");
            assert!(text.contains("/queues/log81"), "{text}");
            break;
        }
        assert!(
            Instant::now() < deadline,
            "no span reached the collector:\n{text}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// §8.1, DDS → AMQP with `log_level: debug`: a DDS sample sent to the
/// broker is the line `transfer_out` with `link_name`, `dds_topic`,
/// `amqp_address`, `delivery_id` and `bytes`; its round trip ends with the
/// line `disposition`, outcome `accepted` from the broker's disposition on
/// an unsettled link, outcome `settled` with the transfer on a link that
/// sends settled, each with `latency_us`. A link the broker detaches is the
/// line `link_detached` with `link_name`, `dds_topic`, `amqp_address` and
/// the broker's condition. The DDS writers are a participant of this test.
#[cfg(unix)]
#[test]
fn amqpbridge_8_1_transfer_out_and_disposition_lines_of_both_settle_modes() {
    use std::io::BufRead;
    use zerodds_amqp_bridge::extended_types::AmqpExtValue;
    use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserWriterConfig};
    use zerodds_rtps::wire_types::GuidPrefix;

    let broker = Broker::bind();
    let domain = zerodds_test_vendor_reader::lease_domain();
    let dir = std::env::temp_dir().join(format!("amqp-e2e-out-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let yaml = format!(
        "domain: {}\nlog_level: debug\namqp:\n  broker_url: \"{}\"\n  container_id: zd-e2e-out\ntopics:\n  - dds_name: Out81U\n    amqp_address: /queues/out81u\n    direction: out\n  - dds_name: Out81S\n    amqp_address: /queues/out81s\n    direction: out\n    sender:\n      settled: true\ncatalog:\n  enabled: false\n",
        domain.id(),
        broker.url()
    );
    let file = dir.join("bridge.yaml");
    std::fs::write(&file, &yaml).unwrap();
    let mut child = Command::new(daemon_binary())
        .args(["--config", file.to_str().unwrap()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon");
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
        {
            let _ = tx.send(line);
        }
    });
    let mut conn = broker.accept(Duration::from_secs(10));
    conn.handshake();
    let links = conn.setup_links(2, 0);
    let link = |name: &str| {
        links
            .iter()
            .find(|l| l.name == name)
            .unwrap_or_else(|| panic!("no link {name}: {links:?}"))
            .clone()
    };
    let unsettled = link("zd-e2e-out/Out81U/sender");
    let settled = link("zd-e2e-out/Out81S/sender");
    conn.flow(&unsettled, 0, 10);
    conn.flow(&settled, 0, 10);

    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[11] = 0x81;
    let dds = DcpsRuntime::start(
        domain.id(),
        GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .unwrap();
    let matched_writer = |topic: &str| {
        let eid = dds
            .register_user_writer_kind(
                UserWriterConfig {
                    topic_name: topic.into(),
                    type_name: topic.into(),
                    reliable: true,
                    ..Default::default()
                },
                false,
            )
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while dds.user_writer_matched_count(eid) == 0 {
            assert!(
                Instant::now() < deadline,
                "the daemon's reader of {topic} did not match"
            );
            dds.wait_match_event(Duration::from_millis(50));
        }
        eid
    };
    let wu = matched_writer("Out81U");
    let ws = matched_writer("Out81S");

    // Unsettled: the broker accepts.
    dds.write_user_sample_borrowed(wu, &[1, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    assert_eq!(t.uint(0), Some(unsettled.handle));
    assert_eq!(t.boolean(4), Some(false), "unsettled");
    let (id_u, bytes_u) = (t.uint(1).unwrap(), t.payload.len());
    conn.disposition(
        unsettled.channel,
        true,
        id_u,
        id_u,
        true,
        AmqpExtValue::Described {
            descriptor: 0x24,
            value: Box::new(AmqpExtValue::List(vec![])),
        },
    );
    // Settled: the round trip ends with the transfer.
    dds.write_user_sample_borrowed(ws, &[2, 0, 0, 0, 9, 9, 9, 9])
        .unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    assert_eq!(t.uint(0), Some(settled.handle));
    assert_eq!(t.boolean(4), Some(true), "settled");
    let (id_s, bytes_s) = (t.uint(1).unwrap(), t.payload.len());
    assert_ne!(bytes_u, bytes_s);
    // The broker detaches the settled link (Part 2 §2.7.7).
    conn.send(
        settled.channel,
        descriptor::DETACH,
        vec![
            AmqpExtValue::Uint(settled.handle),
            AmqpExtValue::Boolean(true),
            scripted_broker::error("amqp:resource-deleted"),
        ],
        &[],
    );
    conn.next_of(descriptor::DETACH);

    let has = |lines: &[String], event: &str, link: &str| {
        lines.iter().any(|l| {
            l.contains(&format!("\"event\":\"{event}\""))
                && l.contains(&format!("\"link_name\":\"{link}\""))
        })
    };
    let mut lines: Vec<String> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !(has(&lines, "disposition", &unsettled.name)
        && has(&lines, "disposition", &settled.name)
        && has(&lines, "link_detached", &settled.name))
    {
        let left = deadline.saturating_duration_since(Instant::now());
        lines.push(
            rx.recv_timeout(left)
                .unwrap_or_else(|_| panic!("the lines of both round trips: {lines:#?}")),
        );
    }
    let (_, stderr) = stop_until_close(&mut child, &mut conn, libc::SIGTERM);
    while let Ok(l) = rx.recv_timeout(Duration::from_millis(500)) {
        lines.push(l);
    }
    dds.shutdown();
    every_line_is_one_of_the_program_or_the_participant(&lines, "zd-e2e-out");
    let only = |event: &str, link: &str| -> &String {
        let found: Vec<&String> = lines
            .iter()
            .filter(|l| {
                l.contains(&format!("\"event\":\"{event}\""))
                    && l.contains(&format!("\"link_name\":\"{link}\""))
            })
            .collect();
        assert_eq!(found.len(), 1, "one {event} line of {link}: {lines:#?}");
        found[0]
    };
    let latency = |l: &str| -> u64 {
        let rest = &l[l.find("\"latency_us\":").unwrap_or_else(|| panic!("{l}")) + 13..];
        rest[..rest.find([',', '}']).unwrap()].parse().unwrap()
    };
    for (link, topic, address, id, bytes, outcome) in [
        (
            &unsettled,
            "Out81U",
            "/queues/out81u",
            id_u,
            bytes_u,
            "accepted",
        ),
        (
            &settled,
            "Out81S",
            "/queues/out81s",
            id_s,
            bytes_s,
            "settled",
        ),
    ] {
        let out = only("transfer_out", &link.name);
        for field in [
            "\"level\":\"debug\"".to_string(),
            format!("\"dds_topic\":\"{topic}\""),
            format!("\"amqp_address\":\"{address}\""),
            format!("\"delivery_id\":{id},"),
            format!("\"bytes\":{bytes}}}"),
        ] {
            assert!(out.contains(&field), "{field} in {out}");
        }
        let d = only("disposition", &link.name);
        for field in [
            "\"level\":\"debug\"".to_string(),
            format!("\"dds_topic\":\"{topic}\""),
            format!("\"amqp_address\":\"{address}\""),
            format!("\"delivery_id\":{id},"),
            format!("\"outcome\":\"{outcome}\""),
        ] {
            assert!(d.contains(&field), "{field} in {d}");
        }
        // From the sample taken to the disposition: below the ten seconds
        // the test waits at most.
        assert!(latency(d) < 10_000_000, "{d}");
    }
    let detached = only("link_detached", &settled.name);
    for field in [
        "\"level\":\"warn\"",
        "\"dds_topic\":\"Out81S\"",
        "\"amqp_address\":\"/queues/out81s\"",
        "amqp:resource-deleted",
    ] {
        assert!(detached.contains(field), "{field} in {detached}");
    }
    assert!(
        !stderr.contains("\"event\""),
        "no log line on stderr: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The JSON log lines of `child`'s stdout, as they come.
fn stdout_lines(child: &mut Child) -> mpsc::Receiver<String> {
    use std::io::BufRead;
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
        {
            let _ = tx.send(line);
        }
    });
    rx
}

/// The position of the first line of `event` in `lines`.
fn position_of(lines: &[String], event: &str) -> usize {
    lines
        .iter()
        .position(|l| l.contains(&format!("\"event\":\"{event}\"")))
        .unwrap_or_else(|| panic!("no {event} line in {lines:#?}"))
}

/// §9.1: the log lines of the program follow the steps of the startup:
/// the configuration (`startup`), the DDS participant
/// (`participant_started` with its domain and GUID prefix), its readers
/// and writers (`endpoints_registered`), the broker connection
/// (`connected`), then the links (`attached`). §9.2: SIGINT stops the
/// daemon as SIGTERM does: detach, end, close, the line `closed`, exit 0.
#[cfg(unix)]
#[test]
fn amqpbridge_9_1_the_log_lines_follow_the_steps_and_sigint_exits_0() {
    let broker = Broker::bind();
    let domain = zerodds_test_vendor_reader::lease_domain();
    let mut child = Command::new(daemon_binary())
        .args([
            "--broker",
            &broker.url(),
            "--container-id",
            "zd-e2e-start",
            "--domain",
            &domain.id().to_string(),
            "--topic",
            "Start91=/queues/start91",
            "--set",
            "catalog.enabled=false",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon");
    let rx = stdout_lines(&mut child);
    let mut conn = broker.accept(Duration::from_secs(10));
    conn.handshake();
    conn.setup_links(2, 0);
    let mut lines = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while lines
        .iter()
        .filter(|l: &&String| l.contains("\"event\":\"attached\""))
        .count()
        < 2
    {
        let left = deadline.saturating_duration_since(Instant::now());
        lines.push(rx.recv_timeout(left).expect("the attached lines"));
    }
    let (seen, _) = stop_until_close(&mut child, &mut conn, libc::SIGINT);
    assert_both_links_closed(&seen);
    while let Ok(l) = rx.recv_timeout(Duration::from_millis(500)) {
        lines.push(l);
    }
    let steps = [
        "startup",
        "participant_started",
        "endpoints_registered",
        "connected",
        "attached",
        "closed",
    ]
    .map(|e| position_of(&lines, e));
    assert!(
        steps.windows(2).all(|w| w[0] < w[1]),
        "{steps:?} in {lines:#?}"
    );
    let started = &lines[steps[1]];
    assert!(
        started.contains(&format!("\"domain\":{}", domain.id())),
        "{started}"
    );
    assert!(started.contains("\"guid_prefix\":\""), "{started}");
    let endpoints = &lines[steps[2]];
    assert!(
        endpoints.contains("\"readers\":1") && endpoints.contains("\"writers\":1"),
        "{endpoints}"
    );
    assert!(
        !lines.iter().any(|l| l.contains("\"event\":\"tls_loaded\"")),
        "no TLS configured"
    );
}

/// §9.1 step 2 before step 3: TLS material that does not load stops the
/// daemon with exit 1 before the DDS participant starts and before the
/// broker is contacted.
#[cfg(unix)]
#[test]
fn amqpbridge_9_1_tls_material_that_does_not_load_stops_before_dds() {
    let broker = Broker::bind();
    let domain = zerodds_test_vendor_reader::lease_domain();
    let missing = std::env::temp_dir().join(format!("amqp-e2e-no-ca-{}.pem", std::process::id()));
    let mut child = Command::new(daemon_binary())
        .args([
            "--broker",
            &broker.url().replace("amqp://", "amqps://"),
            "--domain",
            &domain.id().to_string(),
            "--tls-ca",
            missing.to_str().unwrap(),
            "--topic",
            "Start91T=/queues/start91t",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon");
    let rx = stdout_lines(&mut child);
    let status = wait_exit(&mut child, Duration::from_secs(10));
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(status.code(), Some(1), "{stderr}");
    assert!(stderr.contains("amqp.tls"), "{stderr}");
    let mut lines = Vec::new();
    while let Ok(l) = rx.recv_timeout(Duration::from_millis(300)) {
        lines.push(l);
    }
    assert!(
        !lines
            .iter()
            .any(|l| l.contains("\"event\":\"participant_started\"")),
        "{lines:#?}"
    );
    assert!(
        broker.try_accept(Duration::from_millis(300)).is_none(),
        "the daemon contacted the broker"
    );
}

/// The daemon of the §9.2 tests: one `out` topic `topic` (KEEP_ALL) to
/// `/queues/<topic>` at `broker_url`, the back-off `delay_ms`, its JSON log
/// lines as they come, and the directory of its configuration file.
#[cfg(unix)]
fn out_daemon(
    broker_url: &str,
    domain: i32,
    container_id: &str,
    topic: &str,
    delay_ms: u32,
) -> (Child, mpsc::Receiver<String>, PathBuf) {
    let dir = std::env::temp_dir().join(format!("amqp-e2e-{container_id}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bridge.yaml");
    std::fs::write(
        &file,
        format!(
            "domain: {domain}\namqp:\n  broker_url: \"{broker_url}\"\n  container_id: {container_id}\n  reconnect:\n    initial_delay_ms: {delay_ms}\n    max_delay_ms: {delay_ms}\ntopics:\n  - dds_name: {topic}\n    amqp_address: /queues/{topic}\n    direction: out\n    qos:\n      history: {{ kind: keep_all }}\ncatalog:\n  enabled: false\n"
        ),
    )
    .unwrap();
    let mut child = Command::new(daemon_binary())
        .args(["--config", file.to_str().unwrap()])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon");
    let rx = stdout_lines(&mut child);
    (child, rx, dir)
}

/// A DDS participant of the test with a writer on `topic`, matched with the
/// daemon's reader.
#[cfg(unix)]
fn matched_dds_writer(
    domain: i32,
    tag: u8,
    topic: &str,
) -> (
    std::sync::Arc<zerodds_dcps::runtime::DcpsRuntime>,
    zerodds_rtps::wire_types::EntityId,
) {
    use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserWriterConfig};
    use zerodds_rtps::wire_types::GuidPrefix;
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[11] = tag;
    let dds = DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .unwrap();
    let eid = dds
        .register_user_writer_kind(
            UserWriterConfig {
                topic_name: topic.into(),
                type_name: topic.into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while dds.user_writer_matched_count(eid) == 0 {
        assert!(
            Instant::now() < deadline,
            "the daemon's reader of {topic} did not match"
        );
        dds.wait_match_event(Duration::from_millis(50));
    }
    (dds, eid)
}

#[cfg(unix)]
fn sigint(child: &Child) {
    #[allow(unsafe_code)]
    // SAFETY: kill(2) takes a pid and a signal number and touches no memory
    // of this process. Every caller reaps `child` only after this call
    // (`exit_code`), so its pid still names the daemon the test spawned and
    // not a process that took the pid over.
    let rc = unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGINT) };
    assert_eq!(rc, 0);
}

/// The exit code of `child` within 15 s, its stderr in the message.
#[cfg(unix)]
fn exit_code(child: &mut Child) -> (Option<i32>, String) {
    let status = wait_exit(child, Duration::from_secs(15));
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    (status.code(), stderr)
}

/// The lines left in `rx` after the daemon's exit.
fn rest_of(rx: &mpsc::Receiver<String>, mut lines: Vec<String>) -> Vec<String> {
    while let Ok(l) = rx.recv_timeout(Duration::from_millis(500)) {
        lines.push(l);
    }
    lines
}

/// The one line of `event` in `lines`.
fn only_line<'a>(lines: &'a [String], event: &str) -> &'a str {
    let found: Vec<&String> = lines
        .iter()
        .filter(|l| l.contains(&format!("\"event\":\"{event}\"")))
        .collect();
    assert_eq!(found.len(), 1, "one {event} line: {lines:#?}");
    found[0]
}

/// The number in field `name` of the JSON line `line`.
fn number_field(line: &str, name: &str) -> u64 {
    let key = format!("\"{name}\":");
    let at = line
        .find(&key)
        .unwrap_or_else(|| panic!("no {name} in {line}"))
        + key.len();
    let rest = &line[at..];
    rest[..rest.find([',', '}']).unwrap()]
        .parse()
        .unwrap_or_else(|_| panic!("{name} in {line}"))
}

/// §9.2: a stop while no broker connection stands. Nothing listens on the
/// broker's port; after the first failed connect the daemon waits out its
/// back-off (60 s), and two samples written meanwhile are delivered by the
/// DDS reader but taken by no pump. SIGINT ends the daemon at once with
/// exit 0; the line `shutdown_with_queued` counts the two samples as
/// `not_taken` (nothing was queued on the link: `count` 0), and no
/// `closed` line follows, since no connection was closed.
#[cfg(unix)]
#[test]
fn amqpbridge_9_2_a_stop_without_a_broker_connection_counts_the_samples_left() {
    let port = Broker::free_port();
    let domain = zerodds_test_vendor_reader::lease_domain();
    let (mut child, rx, dir) = out_daemon(
        &format!("amqp://127.0.0.1:{port}"),
        domain.id(),
        "zd-e2e-noconn",
        "Stop92N",
        60_000,
    );
    let mut lines = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !lines
        .iter()
        .any(|l: &String| l.contains("\"event\":\"connect_error\""))
    {
        let left = deadline.saturating_duration_since(Instant::now());
        lines.push(rx.recv_timeout(left).expect("the connect_error line"));
    }
    let (dds, w) = matched_dds_writer(domain.id(), 0x92, "Stop92N");
    dds.write_user_sample_borrowed(w, &[1, 0, 0, 0]).unwrap();
    dds.write_user_sample_borrowed(w, &[2, 0, 0, 0]).unwrap();
    std::thread::sleep(Duration::from_millis(800));
    let stopped = Instant::now();
    sigint(&child);
    let (code, stderr) = exit_code(&mut child);
    assert_eq!(code, Some(0), "{stderr}");
    assert!(
        stopped.elapsed() < Duration::from_secs(5),
        "the stop waited {:?}",
        stopped.elapsed()
    );
    let lines = rest_of(&rx, lines);
    dds.shutdown();
    let queued = only_line(&lines, "shutdown_with_queued");
    assert!(queued.contains("\"level\":\"error\""), "{queued}");
    assert_eq!(number_field(queued, "count"), 0, "{queued}");
    assert_eq!(number_field(queued, "not_taken"), 2, "{queued}");
    assert!(
        !lines.iter().any(|l| l.contains("\"event\":\"closed\"")),
        "{lines:#?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// §9.2: a message queued for credit when SIGINT comes is not sent; the
/// line `shutdown_with_queued` counts it (`count` 1, `not_taken` 0) and
/// the daemon exits with 0 after detach, end and close.
#[cfg(unix)]
#[test]
fn amqpbridge_9_2_the_stop_line_counts_a_message_waiting_for_credit() {
    let broker = Broker::bind();
    let domain = zerodds_test_vendor_reader::lease_domain();
    let (mut child, rx, dir) =
        out_daemon(&broker.url(), domain.id(), "zd-e2e-credit", "Stop92Q", 200);
    let mut conn = broker.accept(Duration::from_secs(10));
    conn.handshake();
    conn.setup_links(1, 0);
    let (dds, w) = matched_dds_writer(domain.id(), 0x93, "Stop92Q");
    dds.write_user_sample_borrowed(w, &[5, 0, 0, 0]).unwrap();
    assert!(
        conn.try_next(Duration::from_millis(600)).is_none(),
        "a frame without credit"
    );
    sigint(&child);
    let mut seen = Vec::new();
    while let Some(f) = conn.try_next(Duration::from_secs(10)) {
        seen.push(f.code);
        if f.code == descriptor::CLOSE {
            break;
        }
    }
    assert!(!seen.contains(&descriptor::TRANSFER), "{seen:x?}");
    assert_eq!(seen.last(), Some(&descriptor::CLOSE), "{seen:x?}");
    let (code, stderr) = exit_code(&mut child);
    assert_eq!(code, Some(0), "{stderr}");
    let lines = rest_of(&rx, Vec::new());
    dds.shutdown();
    let queued = only_line(&lines, "shutdown_with_queued");
    assert_eq!(number_field(queued, "count"), 1, "{queued}");
    assert_eq!(number_field(queued, "not_taken"), 0, "{queued}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §9.2: the line `drain_started` tells the stop the pump has seen, with
/// the delivery in flight (`in_flight` 1) and nothing queued (`queued` 0).
/// A sample the DDS reader delivers during the drain is not sent; the line
/// `shutdown_with_queued` counts it as `not_taken` 1 (`count` 0: the
/// delivery in flight was settled by the broker's disposition).
#[cfg(unix)]
#[test]
fn amqpbridge_9_2_the_stop_line_counts_a_sample_delivered_during_the_drain() {
    let broker = Broker::bind();
    let domain = zerodds_test_vendor_reader::lease_domain();
    let (mut child, rx, dir) =
        out_daemon(&broker.url(), domain.id(), "zd-e2e-drain", "Stop92S", 200);
    let mut conn = broker.accept(Duration::from_secs(10));
    conn.handshake();
    let link = conn.setup_links(1, 0).remove(0);
    conn.flow(&link, 0, 10);
    let (dds, w) = matched_dds_writer(domain.id(), 0x94, "Stop92S");
    dds.write_user_sample_borrowed(w, &[3, 0, 0, 0]).unwrap();
    let t = conn.next_of(descriptor::TRANSFER);
    let id = t.uint(1).unwrap();
    sigint(&child);
    // The line `drain_started`: the pump has seen the stop and takes no
    // sample from here on; it waits for the one delivery in flight.
    let mut lines = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !lines
        .iter()
        .any(|l: &String| l.contains("\"event\":\"drain_started\""))
    {
        let left = deadline.saturating_duration_since(Instant::now());
        lines.push(rx.recv_timeout(left).expect("the line drain_started"));
    }
    let started = only_line(&lines, "drain_started");
    assert_eq!(number_field(started, "in_flight"), 1, "{started}");
    assert_eq!(number_field(started, "queued"), 0, "{started}");
    dds.write_user_sample_borrowed(w, &[4, 0, 0, 0]).unwrap();
    assert!(
        dds.await_user_writer_acknowledged(w, Instant::now() + Duration::from_secs(10)),
        "the daemon's reader has sample 4"
    );
    assert!(
        conn.try_next(Duration::from_millis(800)).is_none(),
        "a frame after the stop before the disposition"
    );
    conn.disposition(
        link.channel,
        true,
        id,
        id,
        true,
        scripted_broker::accepted(),
    );
    let mut seen = Vec::new();
    while let Some(f) = conn.try_next(Duration::from_secs(10)) {
        seen.push(f.code);
        if f.code == descriptor::CLOSE {
            break;
        }
    }
    assert!(!seen.contains(&descriptor::TRANSFER), "{seen:x?}");
    assert_eq!(seen.last(), Some(&descriptor::CLOSE), "{seen:x?}");
    let (code, stderr) = exit_code(&mut child);
    assert_eq!(code, Some(0), "{stderr}");
    let lines = rest_of(&rx, lines);
    dds.shutdown();
    let queued = only_line(&lines, "shutdown_with_queued");
    assert_eq!(number_field(queued, "count"), 0, "{queued}");
    assert_eq!(number_field(queued, "not_taken"), 1, "{queued}");
    assert!(
        position_of(&lines, "drain_started") < position_of(&lines, "shutdown_with_queued"),
        "{lines:#?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// §9.1 step 2: with TLS material that loads, the line `tls_loaded` (with
/// the CA file) comes after `startup` and before `participant_started`.
/// The broker side is a TLS server with a certificate of that CA: the
/// handshake stands and the daemon's SASL header arrives over it; then the
/// server ends the connection, a refusal in the SASL stage (exit 5, §2).
#[cfg(unix)]
#[test]
fn amqpbridge_9_1_tls_loaded_comes_between_startup_and_the_participant() {
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let server_key = rcgen::KeyPair::generate().unwrap();
    let server_cert = rcgen::CertificateParams::new(vec!["localhost".to_string()])
        .unwrap()
        .signed_by(&server_key, &ca, &ca_key)
        .unwrap();
    let ca_file =
        std::env::temp_dir().join(format!("amqpbridge-9-1-ca-{}.pem", std::process::id()));
    std::fs::write(&ca_file, ca.pem()).unwrap();
    let cfg = amqp_tls_server(&server_cert, &server_key);
    let (listener, addr) = pick_port_listener();
    let port = addr.rsplit(':').next().unwrap().to_string();
    let (header_tx, header_rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let (sock, _) = listener.accept().unwrap();
        sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let conn = rustls::ServerConnection::new(cfg).unwrap();
        let mut tls = rustls::StreamOwned::new(conn, sock);
        let mut header = [0u8; 8];
        if tls.read_exact(&mut header).is_ok() {
            let _ = header_tx.send(header.to_vec());
        }
        tls.conn.send_close_notify();
        let _ = tls.flush();
    });
    let domain = zerodds_test_vendor_reader::lease_domain();
    let mut child = Command::new(daemon_binary())
        .args([
            "--broker",
            &format!("amqps://localhost:{port}"),
            "--tls-ca",
            ca_file.to_str().unwrap(),
            "--sasl-mechanism",
            "PLAIN",
            "--user",
            "u",
            "--password",
            "p",
            "--domain",
            &domain.id().to_string(),
            "--topic",
            "Start91L=/queues/start91l",
            "--set",
            "catalog.enabled=false",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn daemon");
    let rx = stdout_lines(&mut child);
    let header = header_rx
        .recv_timeout(Duration::from_secs(10))
        .expect("the SASL header over TLS");
    assert_eq!(&header[..5], b"AMQP\x03", "{header:?}");
    let (code, stderr) = exit_code(&mut child);
    let _ = std::fs::remove_file(&ca_file);
    assert_eq!(code, Some(5), "{stderr}");
    let lines = rest_of(&rx, Vec::new());
    let steps = ["startup", "tls_loaded", "participant_started"].map(|e| position_of(&lines, e));
    assert!(
        steps.windows(2).all(|w| w[0] < w[1]),
        "{steps:?} in {lines:#?}"
    );
    let loaded = &lines[steps[1]];
    assert!(
        loaded.contains(&format!("\"ca_file\":\"{}\"", ca_file.to_str().unwrap())),
        "{loaded}"
    );
}
