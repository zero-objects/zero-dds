// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// E2E test of `corba.ior.naming_service`/`naming_client.rs` against a
// real, separately spawned `zerodds-naming-svc`: the bridge daemon
// rebinds its one mapping's servant into it at start, a third GIOP
// client resolves the servant's name through the NameService and gets
// back an IOR pointing at the bridge's own IIOP address, and the
// binding is gone (`NotFound`) once the bridge stops cleanly.
//
// Needs both binaries; `zerodds-naming-svc` only exists in
// `zerodds-corba-cosnaming` built with `--features daemon` (the
// workspace's default dev build includes it).

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    missing_docs
)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use zerodds_corba_cosnaming::graph::ROOT_OBJECT_KEY;
use zerodds_corba_cosnaming::name::NameComponent;
use zerodds_corba_cosnaming::wire;
use zerodds_corba_giop::version::Version;
use zerodds_corba_giop::{
    LocateStatusType, Message, Reply, ReplyStatusType, Request, ResponseFlags, ServiceContextList,
    TargetAddress, decode_message, encode_message,
};
use zerodds_wire::Endianness;

struct Guard(Child);
impl Drop for Guard {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let pid = self.0.id() as i32;
            // SAFETY: `id()` is valid before `wait`; `kill` is call-safe.
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            loop {
                match self.0.try_wait() {
                    Ok(Some(_)) => return,
                    Ok(None) => {
                        if std::time::Instant::now() >= deadline {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    Err(_) => break,
                }
            }
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
        #[cfg(not(unix))]
        {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

/// The directory `CARGO_BIN_EXE_zerodds-corba-bridged` (this crate's own
/// binary) lives in — the same one every workspace binary, including a
/// sibling crate's, is placed in under one `cargo build`/`cargo test`.
fn workspace_target_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-corba-bridged"))
        .parent()
        .expect("bin has a parent dir")
        .to_path_buf()
}

fn pick_port() -> u16 {
    let s = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let p = s.local_addr().expect("addr").port();
    drop(s);
    p
}

fn connect(port: u16) -> TcpStream {
    for _ in 0..80 {
        if let Ok(s) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
            let _ = s.set_write_timeout(Some(Duration::from_secs(3)));
            return s;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("connect failed after retries");
}

fn read_giop(stream: &mut TcpStream) -> Message {
    let mut hdr = [0u8; 12];
    stream.read_exact(&mut hdr).expect("read header");
    let le = hdr[6] & 0x01 != 0;
    let size = if le {
        u32::from_le_bytes([hdr[8], hdr[9], hdr[10], hdr[11]])
    } else {
        u32::from_be_bytes([hdr[8], hdr[9], hdr[10], hdr[11]])
    };
    let mut full = hdr.to_vec();
    full.resize(12 + size as usize, 0);
    stream.read_exact(&mut full[12..]).expect("read body");
    decode_message(&full).expect("decode").0
}

fn call(
    stream: &mut TcpStream,
    request_id: u32,
    object_key: &[u8],
    operation: &str,
    body: Vec<u8>,
) -> Reply {
    let req = Request {
        request_id,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(object_key.to_vec()),
        operation: operation.to_string(),
        requesting_principal: None,
        service_context: ServiceContextList::default(),
        body,
    };
    let bytes = encode_message(
        Version::V1_2,
        Endianness::Little,
        false,
        &Message::Request(req),
    )
    .unwrap();
    stream.write_all(&bytes).unwrap();
    match read_giop(stream) {
        Message::Reply(r) => r,
        other => panic!("expected Reply, got {:?}", other.message_type()),
    }
}

/// A real, standalone zerodds-naming-svc binary path, built on demand.
/// It is a `[[bin]]` of a *different* crate (zerodds-corba-cosnaming,
/// feature `daemon`), so Cargo does not expose
/// `CARGO_BIN_EXE_zerodds-naming-svc` here the way it does for this
/// crate's own `zerodds-corba-bridged` — this crate's dev-dependency on
/// it (feature `daemon`) only pulls in the library, not the binary
/// artifact, so it is built explicitly, into the same target directory
/// `cargo test` itself already holds the lock for (a nested
/// `cargo build` waits for that lock rather than racing it).
fn naming_svc_binary() -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let status = Command::new(&cargo)
        .args([
            "build",
            "-q",
            "-p",
            "zerodds-corba-cosnaming",
            "--bin",
            "zerodds-naming-svc",
            "--features",
            "daemon",
        ])
        .status()
        .expect("run cargo build for zerodds-naming-svc");
    assert!(status.success(), "building zerodds-naming-svc failed");
    let naming_bin = workspace_target_dir().join("zerodds-naming-svc");
    assert!(
        naming_bin.exists(),
        "{} does not exist after build",
        naming_bin.display()
    );
    naming_bin
}

/// Resolves `name` against `object_key`, retrying (the bridge's own DDS
/// participant start, before it ever reaches the naming rebind, has no
/// fixed duration — a fixed sleep here would be exactly the
/// flaky-by-construction wait this project's own tests avoid elsewhere)
/// until it succeeds or 10s pass.
fn resolve_until_ready(stream: &mut TcpStream, object_key: &[u8], name: &[NameComponent]) -> Reply {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let req_body = wire::encode_name_request(&name.to_vec(), Endianness::Little).unwrap();
        let reply = call(stream, 1, object_key, "resolve", req_body);
        if reply.reply_status == ReplyStatusType::NoException
            || std::time::Instant::now() >= deadline
        {
            return reply;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[test]
fn the_bridge_rebinds_into_a_real_naming_svc_and_unbinds_on_stop() {
    let naming_bin = naming_svc_binary();
    let naming_port = pick_port();
    let naming_child = Command::new(&naming_bin)
        .args(["--bind", &format!("127.0.0.1:{naming_port}")])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn zerodds-naming-svc");
    let _naming_guard = Guard(naming_child);
    std::thread::sleep(Duration::from_millis(500));

    // 2. The bridge, configured to rebind into it.
    let bridge_bin = PathBuf::from(env!("CARGO_BIN_EXE_zerodds-corba-bridged"));
    let bridge_port = pick_port();
    // Host-wide lease (the domain number is the UDP port) — see
    // `crates/dcps/tests/domain_hygiene.rs`.
    let domain = zerodds_test_domains::unique_domain();
    let dir = std::env::temp_dir().join(format!("zd-naming-svc-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let yaml = format!(
        "domain: {domain}\n\
         corba:\n\
         \x20 iiop:\n\
         \x20   bind: \"127.0.0.1:{bridge_port}\"\n\
         \x20 ior:\n\
         \x20   naming_service: \"corbaloc::127.0.0.1:{naming_port}/NameService\"\n\
         \x20   publish_to_naming: true\n\
         \x20   naming_retry_ms: 8000\n\
         mappings:\n\
         \x20 - repo_id: \"IDL:zerodds_test/Echo:1.0\"\n\
         \x20   operation: call\n"
    );
    let cfg_path = dir.join("corba-bridged.yaml");
    std::fs::write(&cfg_path, yaml).unwrap();

    let bridge_child = Command::new(&bridge_bin)
        .args(["--config", cfg_path.to_str().unwrap()])
        .args(["--idle-timeout-ms", "8000"])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn zerodds-corba-bridged");
    let bridge_guard = Guard(bridge_child);

    // 3. A third client resolves the bridge's servant through the
    // NameService: "Echo::call" (slug of the mapping's repo_id +
    // operation) under the root. Polled: the bridge's own DDS
    // participant start (before it ever gets to the naming rebind) has
    // no fixed duration, so a fixed sleep here would be exactly the
    // flaky-by-construction wait this project's own tests avoid
    // elsewhere (`connect`'s retry loop, above).
    let mut naming_stream = connect(naming_port);
    let name = vec![NameComponent::new("Echo::call")];
    let reply = resolve_until_ready(&mut naming_stream, ROOT_OBJECT_KEY, &name);
    assert_eq!(
        reply.reply_status,
        ReplyStatusType::NoException,
        "the bridge's servant must be bound under Echo::call within 10s"
    );
    let ior = wire::decode_object_reply(&reply.body, Endianness::Little).unwrap();
    let profile = ior.profiles[0].as_iiop().unwrap().unwrap();
    assert_eq!(
        profile.port, bridge_port,
        "the resolved IOR names the bridge's own IIOP port"
    );

    // 4. Stop the bridge cleanly (SIGTERM) and confirm it unbound.
    drop(bridge_guard);
    std::thread::sleep(Duration::from_millis(300));
    let req_body = wire::encode_name_request(&name, Endianness::Little).unwrap();
    let reply = call(&mut naming_stream, 2, ROOT_OBJECT_KEY, "resolve", req_body);
    assert_eq!(
        reply.reply_status,
        ReplyStatusType::UserException,
        "unbind on a clean stop must remove the binding"
    );
    assert!(String::from_utf8_lossy(&reply.body).contains(wire::REPO_ID_NOT_FOUND));

    let _ = std::fs::remove_dir_all(&dir);
}

/// `corba.ior.naming_service` over `ssliop:`: the bridge connects to a
/// real `zerodds-naming-svc` SSLIOP listener (self-signed cert, trusted
/// via `corba.ior.naming_ssliop.ca_file`) and rebinds successfully.
#[test]
fn the_bridge_rebinds_over_ssliop() {
    let naming_bin = naming_svc_binary();
    let dir = std::env::temp_dir().join(format!("zd-naming-ssliop-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_path = dir.join("naming.crt");
    let key_path = dir.join("naming.key");
    std::fs::write(&cert_path, cert.cert.pem()).unwrap();
    std::fs::write(&key_path, cert.key_pair.serialize_pem()).unwrap();

    let naming_iiop_port = pick_port();
    let naming_ssliop_port = pick_port();
    let naming_child = Command::new(&naming_bin)
        .args(["--bind", &format!("127.0.0.1:{naming_iiop_port}")])
        .args(["--ssliop-bind", &format!("127.0.0.1:{naming_ssliop_port}")])
        .args(["--tls-cert", cert_path.to_str().unwrap()])
        .args(["--tls-key", key_path.to_str().unwrap()])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn zerodds-naming-svc");
    let _naming_guard = Guard(naming_child);
    std::thread::sleep(Duration::from_millis(500));

    let bridge_bin = PathBuf::from(env!("CARGO_BIN_EXE_zerodds-corba-bridged"));
    let bridge_port = pick_port();
    // Host-wide lease (the domain number is the UDP port) — see
    // `crates/dcps/tests/domain_hygiene.rs`.
    let domain = zerodds_test_domains::unique_domain();
    let yaml = format!(
        "domain: {domain}\n\
         corba:\n\
         \x20 iiop:\n\
         \x20   bind: \"127.0.0.1:{bridge_port}\"\n\
         \x20 ior:\n\
         \x20   naming_service: \"corbaloc:ssliop:1.2@localhost:{naming_ssliop_port}/NameService\"\n\
         \x20   publish_to_naming: true\n\
         \x20   naming_retry_ms: 8000\n\
         \x20   naming_ssliop:\n\
         \x20     ca_file: \"{}\"\n\
         mappings:\n\
         \x20 - repo_id: \"IDL:zerodds_test/Echo:1.0\"\n\
         \x20   operation: call\n",
        cert_path.display()
    );
    let cfg_path = dir.join("corba-bridged.yaml");
    std::fs::write(&cfg_path, yaml).unwrap();

    let bridge_child = Command::new(&bridge_bin)
        .args(["--config", cfg_path.to_str().unwrap()])
        .args(["--idle-timeout-ms", "8000"])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn zerodds-corba-bridged");
    let _bridge_guard = Guard(bridge_child);

    // The plain-IIOP listener (not SSLIOP) is used to verify the
    // binding, so this test's assertion does not itself depend on the
    // TLS client path that was just exercised to make the rebind.
    let mut naming_stream = connect(naming_iiop_port);
    let name = vec![NameComponent::new("Echo::call")];
    let reply = resolve_until_ready(&mut naming_stream, ROOT_OBJECT_KEY, &name);
    assert_eq!(
        reply.reply_status,
        ReplyStatusType::NoException,
        "the bridge must rebind over SSLIOP within 10s"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// `corba.ior.naming_service = "corbaloc:rir:/NameService"` resolves
/// through `corba.ior.initial_references.NameService` to a real
/// `zerodds-naming-svc` and rebinds successfully.
#[test]
fn the_bridge_rebinds_through_a_rir_initial_reference() {
    let naming_bin = naming_svc_binary();
    let naming_port = pick_port();
    let naming_child = Command::new(&naming_bin)
        .args(["--bind", &format!("127.0.0.1:{naming_port}")])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn zerodds-naming-svc");
    let _naming_guard = Guard(naming_child);
    std::thread::sleep(Duration::from_millis(500));

    let bridge_bin = PathBuf::from(env!("CARGO_BIN_EXE_zerodds-corba-bridged"));
    let bridge_port = pick_port();
    // Host-wide lease (the domain number is the UDP port) — see
    // `crates/dcps/tests/domain_hygiene.rs`.
    let domain = zerodds_test_domains::unique_domain();
    let dir = std::env::temp_dir().join(format!("zd-naming-rir-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let yaml = format!(
        "domain: {domain}\n\
         corba:\n\
         \x20 iiop:\n\
         \x20   bind: \"127.0.0.1:{bridge_port}\"\n\
         \x20 ior:\n\
         \x20   naming_service: \"corbaloc:rir:/NameService\"\n\
         \x20   publish_to_naming: true\n\
         \x20   naming_retry_ms: 8000\n\
         \x20   initial_references:\n\
         \x20     NameService: \"corbaloc::127.0.0.1:{naming_port}/NameService\"\n\
         mappings:\n\
         \x20 - repo_id: \"IDL:zerodds_test/Echo:1.0\"\n\
         \x20   operation: call\n"
    );
    let cfg_path = dir.join("corba-bridged.yaml");
    std::fs::write(&cfg_path, yaml).unwrap();

    let bridge_child = Command::new(&bridge_bin)
        .args(["--config", cfg_path.to_str().unwrap()])
        .args(["--idle-timeout-ms", "8000"])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn zerodds-corba-bridged");
    let _bridge_guard = Guard(bridge_child);

    let mut naming_stream = connect(naming_port);
    let name = vec![NameComponent::new("Echo::call")];
    let reply = resolve_until_ready(&mut naming_stream, ROOT_OBJECT_KEY, &name);
    assert_eq!(
        reply.reply_status,
        ReplyStatusType::NoException,
        "the bridge must rebind through rir:/NameService within 10s"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// `corba.naming.serve`: the bridge itself hosts a `CosNaming::NamingContext`
/// (object key `NameService`) on its own IIOP listener -- no separate
/// `zerodds-naming-svc` involved. A client binds a name and resolves it
/// back through the very same connection the bridge's DDS<->GIOP
/// mapping requests would use.
#[test]
fn the_bridge_serves_its_own_nameservice() {
    let bridge_bin = PathBuf::from(env!("CARGO_BIN_EXE_zerodds-corba-bridged"));
    let bridge_port = pick_port();
    // Host-wide lease (the domain number is the UDP port) — see
    // `crates/dcps/tests/domain_hygiene.rs`.
    let domain = zerodds_test_domains::unique_domain();
    let dir = std::env::temp_dir().join(format!("zd-naming-serve-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let persist_path = dir.join("naming.txt");
    let yaml = format!(
        "domain: {domain}\n\
         corba:\n\
         \x20 iiop:\n\
         \x20   bind: \"127.0.0.1:{bridge_port}\"\n\
         \x20 naming:\n\
         \x20   serve: true\n\
         \x20   persist_file: \"{}\"\n\
         mappings:\n\
         \x20 - repo_id: \"IDL:zerodds_test/Echo:1.0\"\n\
         \x20   operation: call\n",
        persist_path.display()
    );
    let cfg_path = dir.join("corba-bridged.yaml");
    std::fs::write(&cfg_path, yaml).unwrap();

    let bridge_child = Command::new(&bridge_bin)
        .args(["--config", cfg_path.to_str().unwrap()])
        .args(["--idle-timeout-ms", "8000"])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn zerodds-corba-bridged");
    let bridge_guard = Guard(bridge_child);

    let mut stream = connect(bridge_port);
    let name = vec![NameComponent::new("thing")];
    let obj = wire::nil_object_reference();
    let bind_body = wire::encode_bind_request(&name, &obj, Endianness::Little).unwrap();

    // The DDS participant start (before the IIOP listener even accepts)
    // has no fixed duration -- retry `bind` itself, the same pattern
    // `resolve_until_ready` uses for `resolve`.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let bind_reply = loop {
        let reply = call(&mut stream, 1, ROOT_OBJECT_KEY, "bind", bind_body.clone());
        if reply.reply_status == ReplyStatusType::NoException
            || std::time::Instant::now() >= deadline
        {
            break reply;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    assert_eq!(
        bind_reply.reply_status,
        ReplyStatusType::NoException,
        "bind against the bridge's own hosted NameService must succeed"
    );

    let resolve_reply = resolve_until_ready(&mut stream, ROOT_OBJECT_KEY, &name);
    assert_eq!(
        resolve_reply.reply_status,
        ReplyStatusType::NoException,
        "resolve against the bridge's own hosted NameService must succeed"
    );
    let resolved = wire::decode_object_reply(&resolve_reply.body, Endianness::Little).unwrap();
    assert!(
        resolved.is_nil(),
        "the bound nil object reference must round-trip unchanged"
    );

    // A mapping's own object key (Echo::call) still dispatches as a
    // regular DDS<->GIOP request, not a naming one -- the "unknown
    // object" path was never removed, only extended.
    let missing = call(&mut stream, 2, b"no-such-key", "resolve", Vec::new());
    assert_eq!(
        missing.reply_status,
        ReplyStatusType::SystemException,
        "an object key neither a mapping nor a NameService key stays OBJECT_NOT_EXIST"
    );

    drop(bridge_guard);
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        persist_path.exists(),
        "corba.naming.persist_file must be saved on a clean stop"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

// ============================================================================
// `CORBA::Object` operations (CORBA 3.3 Part 2 §15.4.2) against the bridge
// -- both its own hosted `NameService` and one of its mapped objects.
// omniORB/JacORB/TAO all narrow an object reference with a remote `_is_a`
// before using it, and a `corbaloc:`/`corbaname:` client sends a
// `LocateRequest` first; the bridge must answer both, for the NameService
// AND for a business mapping.
// ============================================================================

/// Mirrors `bridge_daemon::object_key` (SHA-256(repo_id + "\0" +
/// "request_topic->reply_topic")[..16]) for the one mapping every test in
/// this section configures: `repo_id`/`operation`, no explicit topics, so
/// `slug()` derives `<Iface>::<operation>::{Request,Reply}` from `repo_id`
/// (dropping its first `::`-segment as the prefix pragma) — `csiv2_e2e.rs`
/// exercises the same derivation for the same mapping shape.
fn mapping_object_key(repo_id: &str, operation: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let iface = repo_id.split("/").nth(1).unwrap_or(repo_id);
    let iface = iface.split(':').next().unwrap_or(iface);
    let request_topic = format!("{iface}::{operation}::Request");
    let reply_topic = format!("{iface}::{operation}::Reply");
    let mut h = Sha256::new();
    h.update(repo_id.as_bytes());
    h.update(b"\0");
    h.update(format!("{request_topic}->{reply_topic}").as_bytes());
    h.finalize()[..16].to_vec()
}

fn send_request_versioned(
    stream: &mut TcpStream,
    version: Version,
    endianness: Endianness,
    request_id: u32,
    object_key: &[u8],
    operation: &str,
    body: Vec<u8>,
) {
    let req = Request {
        request_id,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(object_key.to_vec()),
        operation: operation.to_string(),
        requesting_principal: None,
        service_context: ServiceContextList::default(),
        body,
    };
    let bytes = encode_message(version, endianness, false, &Message::Request(req)).unwrap();
    stream.write_all(&bytes).unwrap();
}

fn call_versioned(
    stream: &mut TcpStream,
    version: Version,
    endianness: Endianness,
    request_id: u32,
    object_key: &[u8],
    operation: &str,
    body: Vec<u8>,
) -> Reply {
    send_request_versioned(
        stream, version, endianness, request_id, object_key, operation, body,
    );
    match read_giop(stream) {
        Message::Reply(r) => r,
        other => panic!("expected Reply, got {:?}", other.message_type()),
    }
}

fn encode_is_a_body(logical_type_id: &str, e: Endianness) -> Vec<u8> {
    let mut w = zerodds_wire::Writer::new(e);
    w.write_string(logical_type_id);
    w.finish()
}

fn decode_bool_reply(body: &[u8], e: Endianness) -> bool {
    let mut r = zerodds_wire::Reader::new(body, e);
    r.read_bool().expect("boolean reply")
}

fn decode_string_reply(body: &[u8], e: Endianness) -> String {
    let mut r = zerodds_wire::Reader::new(body, e);
    r.read_string().expect("string reply")
}

fn locate(
    stream: &mut TcpStream,
    request_id: u32,
    key: &[u8],
) -> zerodds_corba_giop::LocateStatusType {
    let lr = zerodds_corba_giop::LocateRequest {
        request_id,
        target: TargetAddress::Key(key.to_vec()),
    };
    let bytes = encode_message(
        Version::V1_2,
        Endianness::Little,
        false,
        &Message::LocateRequest(lr),
    )
    .unwrap();
    stream.write_all(&bytes).unwrap();
    match read_giop(stream) {
        Message::LocateReply(r) => r.locate_status,
        other => panic!("expected a LocateReply, got {:?}", other.message_type()),
    }
}

/// `_is_a` true for the object's own id, false for a foreign one, against
/// both the hosted NameService and the mapping — the first call retries:
/// the mapping's `d.by_key` entry is only populated once the bridge's DDS
/// participant + writer/reader registration for it has completed, which
/// has no fixed duration (the same reason `resolve_until_ready` retries).
#[test]
fn the_bridge_answers_corba_object_operations_on_its_nameservice_and_a_mapping() {
    let bridge_bin = PathBuf::from(env!("CARGO_BIN_EXE_zerodds-corba-bridged"));
    let bridge_port = pick_port();
    let lease = zerodds_test_domains::lease_domain();
    let domain = lease.id();
    let dir = std::env::temp_dir().join(format!("zd-object-ops-e2e-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let persist_path = dir.join("naming.txt");
    let yaml = format!(
        "domain: {domain}\n\
         corba:\n\
         \x20 iiop:\n\
         \x20   bind: \"127.0.0.1:{bridge_port}\"\n\
         \x20 naming:\n\
         \x20   serve: true\n\
         \x20   persist_file: \"{}\"\n\
         mappings:\n\
         \x20 - repo_id: \"IDL:zerodds_test/Echo:1.0\"\n\
         \x20   operation: call\n",
        persist_path.display()
    );
    let cfg_path = dir.join("corba-bridged.yaml");
    std::fs::write(&cfg_path, yaml).unwrap();

    let bridge_child = Command::new(&bridge_bin)
        .args(["--config", cfg_path.to_str().unwrap()])
        .args(["--idle-timeout-ms", "8000"])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn zerodds-corba-bridged");
    let bridge_guard = Guard(bridge_child);

    let mapping_key = mapping_object_key("IDL:zerodds_test/Echo:1.0", "call");
    let mut stream = connect(bridge_port);
    let mut rid = 0u32;

    // Establish readiness against the mapping (see the function doc);
    // the NameService key answers correctly from the first accepted
    // connection onward, so it needs no such retry.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        rid += 1;
        let body = encode_is_a_body("IDL:zerodds_test/Echo:1.0", Endianness::Little);
        let reply = call_versioned(
            &mut stream,
            Version::V1_2,
            Endianness::Little,
            rid,
            &mapping_key,
            "_is_a",
            body,
        );
        if reply.reply_status == ReplyStatusType::NoException {
            assert!(decode_bool_reply(&reply.body, Endianness::Little));
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the mapping never became ready"
        );
        std::thread::sleep(Duration::from_millis(200));
    }

    // GIOP 1.0 little-endian (the byte order every captured `nameclt`
    // request in this workspace uses) and GIOP 1.2 big-endian, against
    // both objects: the object's own id is `_is_a` true, an unrelated one
    // false.
    for (version, endianness) in [
        (Version::V1_0, Endianness::Little),
        (Version::V1_2, Endianness::Big),
    ] {
        for (key, own_id) in [
            (
                ROOT_OBJECT_KEY,
                "IDL:omg.org/CosNaming/NamingContextExt:1.0",
            ),
            (mapping_key.as_slice(), "IDL:zerodds_test/Echo:1.0"),
        ] {
            rid += 1;
            let body = encode_is_a_body(own_id, endianness);
            let reply = call_versioned(&mut stream, version, endianness, rid, key, "_is_a", body);
            assert_eq!(
                reply.reply_status,
                ReplyStatusType::NoException,
                "{version:?}/{endianness:?}/{own_id}"
            );
            assert!(
                decode_bool_reply(&reply.body, endianness),
                "{version:?}/{endianness:?}/{own_id}"
            );

            rid += 1;
            let body = encode_is_a_body("IDL:demo/Nope:1.0", endianness);
            let reply = call_versioned(&mut stream, version, endianness, rid, key, "_is_a", body);
            assert!(
                !decode_bool_reply(&reply.body, endianness),
                "{version:?}/{endianness:?} foreign id against {own_id}'s object"
            );
        }
    }

    // `_non_existent` and `_repository_id` against both objects.
    for (key, expected_repo_id) in [
        (
            ROOT_OBJECT_KEY,
            "IDL:omg.org/CosNaming/NamingContextExt:1.0",
        ),
        (mapping_key.as_slice(), "IDL:zerodds_test/Echo:1.0"),
    ] {
        rid += 1;
        let reply = call_versioned(
            &mut stream,
            Version::V1_2,
            Endianness::Little,
            rid,
            key,
            "_non_existent",
            Vec::new(),
        );
        assert_eq!(reply.reply_status, ReplyStatusType::NoException);
        assert!(!decode_bool_reply(&reply.body, Endianness::Little));

        rid += 1;
        let reply = call_versioned(
            &mut stream,
            Version::V1_2,
            Endianness::Little,
            rid,
            key,
            "_repository_id",
            Vec::new(),
        );
        assert_eq!(reply.reply_status, ReplyStatusType::NoException);
        assert_eq!(
            decode_string_reply(&reply.body, Endianness::Little),
            expected_repo_id
        );
    }

    // `_non_existent` on a key neither the NameService nor the mapping
    // serves answers `true` instead of `OBJECT_NOT_EXIST`.
    rid += 1;
    let reply = call_versioned(
        &mut stream,
        Version::V1_2,
        Endianness::Little,
        rid,
        b"no-such-object",
        "_non_existent",
        Vec::new(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::NoException);
    assert!(decode_bool_reply(&reply.body, Endianness::Little));

    // A `corbaloc:` client's `LocateRequest` (GIOP §15.4.5) — OBJECT_HERE
    // for both objects, UNKNOWN_OBJECT otherwise.
    rid += 1;
    assert_eq!(
        locate(&mut stream, rid, ROOT_OBJECT_KEY),
        LocateStatusType::ObjectHere
    );
    rid += 1;
    assert_eq!(
        locate(&mut stream, rid, &mapping_key),
        LocateStatusType::ObjectHere
    );
    rid += 1;
    assert_eq!(
        locate(&mut stream, rid, b"no-such-object"),
        LocateStatusType::UnknownObject
    );

    drop(bridge_guard);
    let _ = std::fs::remove_dir_all(&dir);
}
