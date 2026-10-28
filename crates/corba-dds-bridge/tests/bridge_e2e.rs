// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// E2E tests for `zerodds-corba-bridged`: the binary's CLI and exit codes,
// the reply to an unknown object, and the shutdown drain of §9.2.
//
// Spec: `docs/specs/zerodds-corba-bridge-1.0.md` §2, §4.3, §9.2, §12.2.

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

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Arc as SyncArc;
use std::time::Duration;

use zerodds_corba_giop::version::Version;
use zerodds_corba_giop::{
    Message, Request, ResponseFlags, ServiceContextList, TargetAddress, decode_message,
    encode_message,
};
use zerodds_wire::Endianness;

fn daemon_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-corba-bridged"))
}

struct DaemonGuard {
    child: Child,
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        terminate_child_gracefully(&mut self.child);
    }
}

/// Graceful SIGTERM termination + 3 s wait, otherwise SIGKILL
/// fallback. A hard `child.kill()` (SIGKILL) prevents the LLVM
/// coverage `__llvm_profile_write_file` atexit handlers from running
/// under `cargo llvm-cov` → corrupt `.profraw` headers → `llvm-profdata
/// merge` fails the entire pipeline.
fn terminate_child_gracefully(child: &mut Child) {
    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        // SAFETY: `child.id()` is valid as long as we have not already
        // waited; Drop is exclusive. `libc::kill` with SIGTERM is
        // always call-safe.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => {
                    if std::time::Instant::now() >= deadline {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn pick_port() -> u16 {
    let s = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let p = s.local_addr().expect("addr").port();
    drop(s);
    p
}

fn spawn_daemon(port: u16) -> DaemonGuard {
    let bind = format!("127.0.0.1:{port}");
    let child = Command::new(daemon_binary())
        .args([
            "--iiop-bind",
            &bind,
            "--orb-id",
            "zerodds-test",
            "--topic",
            "IDL:Foo:1.0=Foo::Topic",
            "--idle-timeout-ms",
            "5000",
        ])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    std::thread::sleep(Duration::from_millis(300));
    DaemonGuard { child }
}

fn connect(port: u16) -> TcpStream {
    for _ in 0..30 {
        if let Ok(s) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
            let _ = s.set_write_timeout(Some(Duration::from_secs(3)));
            return s;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("connect failed after retries");
}

/// A request to an object key the bridge did not issue is answered
/// `CORBA::OBJECT_NOT_EXIST`, not with an empty success (which the daemon
/// used to send for every request).
#[test]
fn a_request_to_an_unknown_object_is_object_not_exist() {
    let port = pick_port();
    let _guard = spawn_daemon(port);
    let mut stream = connect(port);

    // Build a minimal GIOP 1.2 Request.
    let req = Request {
        request_id: 42,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(vec![0u8; 16]),
        operation: "request_quote".into(),
        requesting_principal: None,
        service_context: ServiceContextList::default(),
        body: vec![0xCA, 0xFE, 0xBA, 0xBE],
    };
    let bytes = encode_message(
        Version::V1_2,
        Endianness::Little,
        false,
        &Message::Request(req),
    )
    .expect("encode");
    stream.write_all(&bytes).expect("write request");

    // Read 12-byte header + body.
    let mut hdr = [0u8; 12];
    stream.read_exact(&mut hdr).expect("read header");
    assert_eq!(&hdr[..4], b"GIOP", "magic");
    let endian_le = hdr[6] & 0x01 != 0;
    let size = if endian_le {
        u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]])
    } else {
        u32::from_be_bytes([hdr[8], hdr[9], hdr[10], hdr[11]])
    };
    let mut body = vec![0u8; size as usize];
    stream.read_exact(&mut body).expect("read body");
    let mut full = hdr.to_vec();
    full.extend_from_slice(&body);
    let (msg, _) = decode_message(&full).expect("decode reply");
    match msg {
        Message::Reply(r) => {
            assert_eq!(r.request_id, 42);
            assert_eq!(
                r.reply_status,
                zerodds_corba_giop::ReplyStatusType::SystemException
            );
            let id = b"IDL:omg.org/CORBA/OBJECT_NOT_EXIST:1.0";
            assert!(
                r.body.windows(id.len()).any(|w| w == id),
                "the exception id in {:02x?}",
                r.body
            );
        }
        other => panic!("expected Reply, got {:?}", other.message_type()),
    }
}

fn read_giop(stream: &mut TcpStream) -> Option<Message> {
    let mut hdr = [0u8; 12];
    stream.read_exact(&mut hdr).ok()?;
    let le = hdr[6] & 0x01 != 0;
    let size = if le {
        u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]])
    } else {
        u32::from_be_bytes([hdr[8], hdr[9], hdr[10], hdr[11]])
    };
    let mut full = hdr.to_vec();
    full.resize(12 + size as usize, 0);
    stream.read_exact(&mut full[12..]).ok()?;
    Some(decode_message(&full).expect("decode").0)
}

fn system_exception_of(msg: Message) -> (u32, String) {
    let Message::Reply(r) = msg else {
        panic!("expected Reply, got {:?}", msg.message_type());
    };
    assert_eq!(
        r.reply_status,
        zerodds_corba_giop::ReplyStatusType::SystemException
    );
    let text = String::from_utf8_lossy(&r.body).to_string();
    let id = text
        .split("IDL:omg.org/CORBA/")
        .nth(1)
        .and_then(|t| t.split(':').next())
        .unwrap_or_default()
        .to_string();
    (r.request_id, id)
}

/// §9.2: at shutdown a request still pending gets its reply (here its
/// `TIMEOUT`, no backend answers), a request arriving during the drain gets
/// `TRANSIENT`, then the connection gets `CloseConnection` and is closed.
#[test]
fn the_shutdown_answers_pending_requests_then_closes_the_connection() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use zerodds_corba_dds_bridge::bridge_daemon::{self, DaemonConfig};
    use zerodds_corba_dds_bridge::daemon_runtime::BridgeMetrics;

    let port = pick_port();
    let yaml = format!(
        "domain: {d}\n\
         corba:\n  iiop:\n    bind: \"127.0.0.1:{port}\"\n\
         mappings:\n\
         \x20 - repo_id: \"IDL:zerodds_test/Slow:1.0\"\n    operation: wait\n    timeout_ms: 1500\n",
        d = 150 + (std::process::id() % 60)
    );
    let mut c = zerodds_config::Config::default();
    c.push(
        zerodds_config::parse_str(&yaml, "<test>").unwrap(),
        "<test>".into(),
    )
    .unwrap();
    let cfg = DaemonConfig::from_config(Some(&c)).unwrap();
    let key = bridge_daemon::object_key(&cfg.mappings[0]);

    let stop = Arc::new(AtomicBool::new(false));
    let bridge = {
        let stop = Arc::clone(&stop);
        std::thread::spawn(move || {
            let registry = zerodds_monitor::Registry::new();
            let metrics = BridgeMetrics::register(&registry);
            bridge_daemon::run(
                &cfg,
                zerodds_dcps::runtime::RuntimeConfig::default(),
                &stop,
                &metrics,
            )
        })
    };
    let mut stream = connect(port);
    let request = |id: u32| {
        encode_message(
            Version::V1_2,
            Endianness::Little,
            false,
            &Message::Request(Request {
                request_id: id,
                response_flags: ResponseFlags::SYNC_WITH_TARGET,
                target: TargetAddress::Key(key.clone()),
                operation: "wait".into(),
                requesting_principal: None,
                service_context: ServiceContextList::default(),
                body: Vec::new(),
            }),
        )
        .expect("encode")
    };
    stream.write_all(&request(1)).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let stopped = std::time::Instant::now();
    stop.store(true, Ordering::SeqCst);
    std::thread::sleep(Duration::from_millis(400));
    stream.write_all(&request(2)).unwrap();

    let first = system_exception_of(read_giop(&mut stream).expect("reply to 2"));
    assert_eq!(first, (2, "TRANSIENT".to_string()));
    let second = system_exception_of(read_giop(&mut stream).expect("reply to 1"));
    assert_eq!(second, (1, "TIMEOUT".to_string()));
    match read_giop(&mut stream) {
        Some(Message::CloseConnection(_)) => {}
        other => panic!(
            "expected CloseConnection, got {:?}",
            other.map(|m| m.message_type())
        ),
    }
    assert!(read_giop(&mut stream).is_none(), "the connection is closed");
    bridge.join().unwrap().expect("run");
    assert!(
        stopped.elapsed() < Duration::from_secs(4),
        "stopped after {:?}",
        stopped.elapsed()
    );
}

#[test]
fn dump_iors_writes_stringified_ior() {
    let out = Command::new(daemon_binary())
        .args([
            "--dump-iors",
            "--iiop-bind",
            "127.0.0.1:6833",
            "--topic",
            "IDL:Foo/Bar:1.0=Foo::Bar::Req",
        ])
        .output()
        .expect("spawn");
    assert!(out.status.success(), "exit: {:?}", out.status);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(
        s.contains("IOR:"),
        "stdout should contain stringified IOR, got: {s}"
    );
    assert!(s.contains("IDL:Foo/Bar:1.0"));
}

#[test]
fn version_flag_emits_one_line() {
    let out = Command::new(daemon_binary())
        .arg("--version")
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("zerodds-corba-bridged"));
    assert!(s.contains("1.0"));
}

#[test]
fn unknown_arg_yields_exit_1() {
    let mut child = Command::new(daemon_binary())
        .args(["--no-such-arg", "x"])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn");
    let status = child.wait().expect("wait");
    assert_eq!(status.code(), Some(1));
}

/// §2 exit code 4: an SSLIOP certificate that does not load.
#[test]
fn a_certificate_that_does_not_load_is_exit_4() {
    let dir = std::env::temp_dir().join(format!("corba-exit4-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cert = dir.join("cert.pem");
    std::fs::write(&cert, "not a certificate\n").unwrap();
    let status = Command::new(daemon_binary())
        .args([
            "--iiop-bind",
            &format!("127.0.0.1:{}", pick_port()),
            "--ssliop-bind",
            &format!("127.0.0.1:{}", pick_port()),
            "--tls-cert",
            cert.to_str().unwrap(),
            "--tls-key",
            cert.to_str().unwrap(),
        ])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .expect("spawn");
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(status.code(), Some(4));
}

/// §8.2: the families `zerodds_corba_bridge_*` on the admin endpoint,
/// labelled with the configured operation and topics. One request without a
/// backend ends in `TIMEOUT`, one LocateRequest is answered.
#[test]
fn the_metrics_of_section_8_2_count_a_timed_out_request() {
    let port = pick_port();
    let metrics_port = pick_port();
    let dir = std::env::temp_dir().join(format!("corba-metrics-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let yaml = format!(
        "corba:\n  iiop:\n    bind: \"127.0.0.1:{port}\"\n\
         mappings:\n  - repo_id: \"IDL:zerodds_test/Slow:1.0\"\n    operation: wait\n    request_topic:\n      dds_name: Slow_wait_Request\n    reply_topic:\n      dds_name: Slow_wait_Reply\n    timeout_ms: 300\n\
         metrics:\n  enabled: true\n  listen: \"127.0.0.1:{metrics_port}\"\n"
    );
    let file = dir.join("bridge.yaml");
    std::fs::write(&file, &yaml).unwrap();
    let mut c = zerodds_config::Config::default();
    c.push(
        zerodds_config::parse_str(&yaml, "<test>").unwrap(),
        "<test>".into(),
    )
    .unwrap();
    let cfg = zerodds_corba_dds_bridge::bridge_daemon::DaemonConfig::from_config(Some(&c)).unwrap();
    let key = zerodds_corba_dds_bridge::bridge_daemon::object_key(&cfg.mappings[0]);
    let child = Command::new(daemon_binary())
        .args([
            "--config",
            file.to_str().unwrap(),
            "--idle-timeout-ms",
            "8000",
        ])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    let _guard = DaemonGuard { child };
    let mut stream = connect(port);
    let req = encode_message(
        Version::V1_2,
        Endianness::Little,
        false,
        &Message::Request(Request {
            request_id: 7,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(key.clone()),
            operation: "wait".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: Vec::new(),
        }),
    )
    .unwrap();
    stream.write_all(&req).unwrap();
    let locate = encode_message(
        Version::V1_2,
        Endianness::Little,
        false,
        &Message::LocateRequest(zerodds_corba_giop::LocateRequest {
            request_id: 8,
            target: TargetAddress::Key(key),
        }),
    )
    .unwrap();
    stream.write_all(&locate).unwrap();
    let mut seen = 0;
    while seen < 2 {
        let mut hdr = [0u8; 12];
        stream.read_exact(&mut hdr).expect("a reply");
        let size = u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]]) as usize;
        let mut body = vec![0u8; size];
        stream.read_exact(&mut body).unwrap();
        seen += 1;
    }
    let mut http = TcpStream::connect(("127.0.0.1", metrics_port)).unwrap();
    http.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    http.write_all(b"GET /metrics HTTP/1.0\r\nHost: x\r\n\r\n")
        .unwrap();
    let mut text = String::new();
    let _ = http.read_to_string(&mut text);
    let _ = std::fs::remove_dir_all(&dir);
    for line in [
        "zerodds_corba_bridge_requests_total{operation=\"wait\",status=\"system_exception\"} 1",
        "zerodds_corba_bridge_requests_total{operation=\"wait\",status=\"no_exception\"} 0",
        "zerodds_corba_bridge_timeouts_total{operation=\"wait\"} 1",
        "zerodds_corba_bridge_replies_sent_total{operation=\"wait\"} 1",
        "zerodds_corba_bridge_pending_requests{operation=\"wait\"} 0",
        "zerodds_corba_bridge_dds_samples_published{dds_topic=\"Slow_wait_Request\"} 1",
        "zerodds_corba_bridge_dds_samples_received{dds_topic=\"Slow_wait_Reply\"} 0",
        "zerodds_corba_bridge_locate_requests_total 1",
        "zerodds_corba_bridge_giop_message_errors_total 0",
    ] {
        assert!(text.lines().any(|l| l == line), "missing {line} in\n{text}");
    }
    assert!(
        text.contains("zerodds_corba_bridge_request_latency_seconds"),
        "the latency histogram"
    );
}

/// Sends `wait` to the mapping's object and returns the system exception
/// the reply names.
fn call_wait(port: u16, key: &[u8], request_id: u32) -> String {
    let mut stream = connect(port);
    let req = encode_message(
        Version::V1_2,
        Endianness::Little,
        false,
        &Message::Request(Request {
            request_id,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(key.to_vec()),
            operation: "wait".into(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body: Vec::new(),
        }),
    )
    .unwrap();
    stream.write_all(&req).unwrap();
    system_exception_of(read_giop(&mut stream).expect("a reply")).1
}

#[cfg(unix)]
fn sighup(child: &Child) {
    // SAFETY: the pid is the live child's.
    unsafe {
        libc::kill(child.id() as i32, libc::SIGHUP);
    }
}

/// §9.2 SIGHUP: the ACL of the file as it is now applies; a file that does
/// not load leaves the last one in place.
#[cfg(unix)]
#[test]
fn a_sighup_applies_the_new_acl_and_a_broken_file_keeps_the_old_one() {
    let port = pick_port();
    let dir = std::env::temp_dir().join(format!("corba-reload-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let base = format!(
        "corba:\n  iiop:\n    bind: \"127.0.0.1:{port}\"\n\
         mappings:\n  - repo_id: \"IDL:zerodds_test/Slow:1.0\"\n    operation: wait\n    timeout_ms: 300\n"
    );
    let deny = format!("{base}acl:\n  default_deny: true\n");
    let allow = format!(
        "{deny}  rules:\n    - subject: \"*\"\n      allow_invoke: [\"IDL:zerodds_test/*\"]\n"
    );
    let file = dir.join("bridge.yaml");
    std::fs::write(&file, &deny).unwrap();
    let mut c = zerodds_config::Config::default();
    c.push(
        zerodds_config::parse_str(&base, "<t>").unwrap(),
        "<t>".into(),
    )
    .unwrap();
    let cfg = zerodds_corba_dds_bridge::bridge_daemon::DaemonConfig::from_config(Some(&c)).unwrap();
    let key = zerodds_corba_dds_bridge::bridge_daemon::object_key(&cfg.mappings[0]);
    let child = Command::new(daemon_binary())
        .args([
            "--config",
            file.to_str().unwrap(),
            "--idle-timeout-ms",
            "10000",
        ])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    let guard = DaemonGuard { child };
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(call_wait(port, &key, 1), "NO_PERMISSION");
    std::fs::write(&file, &allow).unwrap();
    sighup(&guard.child);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        call_wait(port, &key, 2),
        "TIMEOUT",
        "the rule of the reloaded file"
    );
    std::fs::write(&file, format!("{allow}  no_such_key: 1\n")).unwrap();
    sighup(&guard.child);
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        call_wait(port, &key, 3),
        "TIMEOUT",
        "a file that does not load changes nothing"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// §9.2 SIGHUP: new SSLIOP material serves the connections that follow. A
/// client that trusts only the second CA fails against the first
/// certificate and succeeds after the files are replaced and the signal
/// sent.
#[cfg(unix)]
#[test]
fn a_sighup_serves_new_connections_with_the_new_certificate() {
    use rcgen::{CertificateParams, DistinguishedName, DnType, IsCa, KeyPair, SanType};
    let mint = |ca_name: &str| {
        let mut ca_p = CertificateParams::new(vec![ca_name.to_string()]).unwrap();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, ca_name);
        ca_p.distinguished_name = dn;
        ca_p.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca_key = KeyPair::generate().unwrap();
        let ca = ca_p.self_signed(&ca_key).unwrap();
        let mut p = CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "localhost");
        p.distinguished_name = dn;
        p.subject_alt_names
            .push(SanType::IpAddress(std::net::IpAddr::from([127, 0, 0, 1])));
        let k = KeyPair::generate().unwrap();
        let c = p.signed_by(&k, &ca, &ca_key).unwrap();
        (ca.der().clone(), c.pem(), k.serialize_pem())
    };
    let (_ca1, cert1, key1) = mint("reload CA one");
    let (ca2, cert2, key2) = mint("reload CA two");
    let (port, ssl_port) = (pick_port(), pick_port());
    let dir = std::env::temp_dir().join(format!("corba-reload-tls-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (cert_file, key_file) = (dir.join("cert.pem"), dir.join("key.pem"));
    std::fs::write(&cert_file, &cert1).unwrap();
    std::fs::write(&key_file, &key1).unwrap();
    let file = dir.join("bridge.yaml");
    std::fs::write(
        &file,
        format!(
            "corba:\n  iiop:\n    bind: \"127.0.0.1:{port}\"\n  ssliop:\n    enabled: true\n    bind: \"127.0.0.1:{ssl_port}\"\n    cert_file: \"{}\"\n    key_file: \"{}\"\n",
            cert_file.display(),
            key_file.display()
        ),
    )
    .unwrap();
    let child = Command::new(daemon_binary())
        .args([
            "--config",
            file.to_str().unwrap(),
            "--idle-timeout-ms",
            "10000",
        ])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    let guard = DaemonGuard { child };
    std::thread::sleep(Duration::from_millis(300));
    let handshake = || -> Result<(), String> {
        let mut roots = rustls::RootCertStore::empty();
        roots.add(ca2.clone()).unwrap();
        let config = SyncArc::new(
            rustls::ClientConfig::builder_with_provider(SyncArc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth(),
        );
        let name = rustls::pki_types::ServerName::try_from("127.0.0.1").unwrap();
        let mut conn = rustls::ClientConnection::new(config, name).map_err(|e| e.to_string())?;
        let mut tcp = connect(ssl_port);
        while conn.is_handshaking() {
            conn.complete_io(&mut tcp).map_err(|e| e.to_string())?;
        }
        Ok(())
    };
    assert!(
        handshake().is_err(),
        "the first certificate is not the second CA's"
    );
    std::fs::write(&cert_file, &cert2).unwrap();
    std::fs::write(&key_file, &key2).unwrap();
    sighup(&guard.child);
    std::thread::sleep(Duration::from_millis(500));
    handshake().expect("after SIGHUP the second CA's certificate");
    let _ = std::fs::remove_dir_all(&dir);
}
