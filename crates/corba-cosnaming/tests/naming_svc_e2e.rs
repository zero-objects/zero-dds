// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// End-to-end tests for `zerodds-naming-svc`: spawns the real binary,
// speaks GIOP over a raw TCP socket (the same style
// `crates/corba-dds-bridge/tests/bridge_e2e.rs` uses), and drives
// bind/resolve/unbind, CSIv2 accept/reject, a `BindingIterator` over
// more bindings than `how_many`, and persistence across a restart.
//
// Needs `--features daemon` (the binary only exists with it).

// `missing_docs` first, before the `cfg` gate: without `daemon` the gate
// strips this whole file to nothing, and an empty test crate still
// needs a crate-level doc unless this allow survives past the gate.
#![allow(missing_docs)]
#![cfg(feature = "daemon")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr
)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use zerodds_corba_cosnaming::graph::ROOT_OBJECT_KEY;
use zerodds_corba_cosnaming::name::NameComponent;
use zerodds_corba_cosnaming::wire;
use zerodds_corba_csiv2::gssup_store::format_entry;
use zerodds_corba_giop::version::Version;
use zerodds_corba_giop::{
    LocateStatusType, Message, Reply, ReplyStatusType, Request, ResponseFlags, ServiceContext,
    ServiceContextList, ServiceContextTag, TargetAddress, decode_message, encode_message,
};
use zerodds_wire::Endianness;

fn daemon_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_zerodds-naming-svc"))
}

struct DaemonGuard {
    child: Child,
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        terminate_child_gracefully(&mut self.child);
    }
}

fn terminate_child_gracefully(child: &mut Child) {
    #[cfg(unix)]
    {
        let pid = child.id() as i32;
        // SAFETY: `child.id()` is valid before `wait`; `libc::kill` is
        // always call-safe.
        unsafe {
            libc::kill(pid, libc::SIGTERM);
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            match child.try_wait() {
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

fn spawn_daemon(extra_args: &[&str]) -> (DaemonGuard, u16) {
    let port = pick_port();
    let bind = format!("127.0.0.1:{port}");
    let child = Command::new(daemon_binary())
        .args(["--bind", &bind])
        .args(extra_args)
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    std::thread::sleep(Duration::from_millis(300));
    (DaemonGuard { child }, port)
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

fn send_request(
    stream: &mut TcpStream,
    request_id: u32,
    object_key: &[u8],
    operation: &str,
    body: Vec<u8>,
    service_context: ServiceContextList,
) {
    let req = Request {
        request_id,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(object_key.to_vec()),
        operation: operation.to_string(),
        requesting_principal: None,
        service_context,
        body,
    };
    let bytes = encode_message(
        Version::V1_2,
        Endianness::Little,
        false,
        &Message::Request(req),
    )
    .expect("encode");
    stream.write_all(&bytes).expect("write request");
}

fn call(
    stream: &mut TcpStream,
    request_id: u32,
    object_key: &[u8],
    operation: &str,
    body: Vec<u8>,
    service_context: ServiceContextList,
) -> Reply {
    send_request(
        stream,
        request_id,
        object_key,
        operation,
        body,
        service_context,
    );
    match read_giop(stream) {
        Message::Reply(r) => r,
        other => panic!("expected Reply, got {:?}", other.message_type()),
    }
}

fn name(parts: &[&str]) -> Vec<NameComponent> {
    parts.iter().map(|p| NameComponent::new(*p)).collect()
}

/// An `Ior` a client would offer as `bind`'s target — content does not
/// matter to these tests, only that it round-trips.
fn a_target_ior() -> zerodds_corba_ior::Ior {
    zerodds_corba_ior::Ior {
        type_id: "IDL:fin/Echo:1.0".into(),
        profiles: Vec::new(),
    }
}

/// `bind` then `resolve` round-trips the object reference over the real
/// wire, against the real spawned daemon.
#[test]
fn bind_then_resolve_round_trips_over_the_real_daemon() {
    let (_guard, port) = spawn_daemon(&[]);
    let mut s = connect(port);

    let target = a_target_ior();
    let bind_body =
        wire::encode_bind_request(&name(&["echo"]), &target, Endianness::Little).unwrap();
    let reply = call(
        &mut s,
        1,
        ROOT_OBJECT_KEY,
        "bind",
        bind_body,
        ServiceContextList::default(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::NoException);

    let resolve_body = wire::encode_name_request(&name(&["echo"]), Endianness::Little).unwrap();
    let reply = call(
        &mut s,
        2,
        ROOT_OBJECT_KEY,
        "resolve",
        resolve_body,
        ServiceContextList::default(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::NoException);
    let decoded = wire::decode_object_reply(&reply.body, Endianness::Little).unwrap();
    assert_eq!(decoded, target);
}

/// `resolve` of an unbound name raises `NotFound` over the wire.
#[test]
fn resolve_of_an_unbound_name_raises_not_found_over_the_wire() {
    let (_guard, port) = spawn_daemon(&[]);
    let mut s = connect(port);
    let body = wire::encode_name_request(&name(&["nope"]), Endianness::Little).unwrap();
    let reply = call(
        &mut s,
        1,
        ROOT_OBJECT_KEY,
        "resolve",
        body,
        ServiceContextList::default(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::UserException);
    let text = String::from_utf8_lossy(&reply.body);
    assert!(text.contains(wire::REPO_ID_NOT_FOUND));
}

/// `unbind` removes a binding — a second `resolve` then raises `NotFound`.
#[test]
fn unbind_removes_a_binding() {
    let (_guard, port) = spawn_daemon(&[]);
    let mut s = connect(port);
    let target = a_target_ior();
    let bind_body = wire::encode_bind_request(&name(&["x"]), &target, Endianness::Little).unwrap();
    let reply = call(
        &mut s,
        1,
        ROOT_OBJECT_KEY,
        "bind",
        bind_body,
        ServiceContextList::default(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::NoException);

    let unbind_body = wire::encode_name_request(&name(&["x"]), Endianness::Little).unwrap();
    let reply = call(
        &mut s,
        2,
        ROOT_OBJECT_KEY,
        "unbind",
        unbind_body,
        ServiceContextList::default(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::NoException);

    let resolve_body = wire::encode_name_request(&name(&["x"]), Endianness::Little).unwrap();
    let reply = call(
        &mut s,
        3,
        ROOT_OBJECT_KEY,
        "resolve",
        resolve_body,
        ServiceContextList::default(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::UserException);
}

/// `list` over more bindings than `how_many` returns a working
/// `BindingIterator` object; draining it via `next_one` reaches every
/// binding, and `destroy` releases it.
#[test]
fn list_over_more_bindings_than_how_many_yields_a_working_iterator() {
    let (_guard, port) = spawn_daemon(&[]);
    let mut s = connect(port);
    for i in 0..5 {
        let body = wire::encode_bind_request(
            &name(&[&format!("o{i}")]),
            &a_target_ior(),
            Endianness::Little,
        )
        .unwrap();
        let reply = call(
            &mut s,
            i as u32,
            ROOT_OBJECT_KEY,
            "bind",
            body,
            ServiceContextList::default(),
        );
        assert_eq!(reply.reply_status, ReplyStatusType::NoException);
    }

    let list_body = wire::encode_list_request(2, Endianness::Little);
    let reply = call(
        &mut s,
        100,
        ROOT_OBJECT_KEY,
        "list",
        list_body,
        ServiceContextList::default(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::NoException);
    let (first, iter_ior) = wire::decode_list_reply(&reply.body, Endianness::Little).unwrap();
    assert_eq!(first.len(), 2);
    let iter_ior = iter_ior.expect("3 bindings remain");
    let iter_key = zerodds_corba_cosnaming::graph::object_key_of(&iter_ior).unwrap();

    let mut seen = first.len();
    loop {
        let reply = call(
            &mut s,
            200,
            &iter_key,
            "next_one",
            Vec::new(),
            ServiceContextList::default(),
        );
        assert_eq!(reply.reply_status, ReplyStatusType::NoException);
        match wire::decode_next_one_reply(&reply.body, Endianness::Little).unwrap() {
            Some(_) => seen += 1,
            None => break,
        }
    }
    assert_eq!(seen, 5);

    let reply = call(
        &mut s,
        300,
        &iter_key,
        "destroy",
        Vec::new(),
        ServiceContextList::default(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::NoException);
    let reply = call(
        &mut s,
        301,
        &iter_key,
        "next_one",
        Vec::new(),
        ServiceContextList::default(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::SystemException);
    assert!(String::from_utf8_lossy(&reply.body).contains("OBJECT_NOT_EXIST"));
}

fn write_password_file(dir: &std::path::Path, username: &str, password: &str) -> PathBuf {
    let path = dir.join("gssup.passwd");
    let line = format_entry(username, password, b"e2e-salt", 1000);
    std::fs::write(&path, line).unwrap();
    path
}

fn csiv2_context(username: &str, password: &str) -> ServiceContextList {
    use zerodds_corba_csiv2::gssup::GssupCredentialToken;
    use zerodds_corba_csiv2::sas::{EstablishContext, IdentityToken, SasMessage};
    let gssup = GssupCredentialToken::new(username.into(), password.into(), Vec::new())
        .to_gss_token(Endianness::Little)
        .unwrap();
    let sas = SasMessage::EstablishContext(EstablishContext {
        client_context_id: 0,
        authorization_token: Vec::new(),
        identity_token: IdentityToken::Absent,
        client_authentication_token: gssup,
    })
    .encode_encapsulation(Endianness::Little)
    .unwrap();
    let mut list = ServiceContextList::default();
    list.0.push(ServiceContext::new(
        ServiceContextTag::SecurityAttributeService.as_u32(),
        sas,
    ));
    list
}

/// With `naming.csiv2.enabled`, a request with valid GSSUP credentials
/// is accepted; one with a wrong password (or none at all) is refused
/// `NO_PERMISSION`.
#[test]
fn csiv2_accepts_valid_and_refuses_invalid_or_missing_credentials() {
    let dir = std::env::temp_dir().join(format!("zd-naming-csiv2-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pw_file = write_password_file(&dir, "alice", "secret");

    let (_guard, port) = spawn_daemon(&[
        "--set",
        "naming.csiv2.enabled=true",
        "--set",
        &format!("naming.csiv2.gssup.password_file={}", pw_file.display()),
    ]);
    let mut s = connect(port);

    // Valid credentials: the list op itself succeeds.
    let body = wire::encode_list_request(0, Endianness::Little);
    let reply = call(
        &mut s,
        1,
        ROOT_OBJECT_KEY,
        "list",
        body.clone(),
        csiv2_context("alice", "secret"),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::NoException);

    // Wrong password: NO_PERMISSION, with a ContextError service context.
    let reply = call(
        &mut s,
        2,
        ROOT_OBJECT_KEY,
        "list",
        body.clone(),
        csiv2_context("alice", "wrong"),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::SystemException);
    assert!(String::from_utf8_lossy(&reply.body).contains("NO_PERMISSION"));
    assert!(
        reply
            .service_context
            .0
            .iter()
            .any(|c| c.context_id == ServiceContextTag::SecurityAttributeService.as_u32()),
        "a ContextError service context is attached to the refusal"
    );

    // No credentials at all: also NO_PERMISSION.
    let reply = call(
        &mut s,
        3,
        ROOT_OBJECT_KEY,
        "list",
        body,
        ServiceContextList::default(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::SystemException);
    assert!(String::from_utf8_lossy(&reply.body).contains("NO_PERMISSION"));

    let _ = std::fs::remove_dir_all(&dir);
}

/// A binding survives a SIGTERM restart when `naming.persist_file` is
/// set: bind, terminate, spawn a fresh daemon on the same file and
/// port, resolve the same name.
#[test]
fn a_binding_survives_a_restart_with_persist_file_set() {
    let dir = std::env::temp_dir().join(format!("zd-naming-restart-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let persist_file = dir.join("graph.txt");
    let port = pick_port();
    let bind = format!("127.0.0.1:{port}");
    let persist_arg = format!("naming.persist_file={}", persist_file.display());

    {
        let child = Command::new(daemon_binary())
            .args(["--bind", &bind, "--set", &persist_arg])
            .stderr(Stdio::null())
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn daemon");
        let mut guard = DaemonGuard { child };
        std::thread::sleep(Duration::from_millis(300));
        let mut s = connect(port);
        let body =
            wire::encode_bind_request(&name(&["durable"]), &a_target_ior(), Endianness::Little)
                .unwrap();
        let reply = call(
            &mut s,
            1,
            ROOT_OBJECT_KEY,
            "bind",
            body,
            ServiceContextList::default(),
        );
        assert_eq!(reply.reply_status, ReplyStatusType::NoException);
        drop(s);
        terminate_child_gracefully(&mut guard.child);
        std::mem::forget(guard); // already terminated
    }
    assert!(persist_file.exists(), "the daemon persists on clean stop");

    let child = Command::new(daemon_binary())
        .args(["--bind", &bind, "--set", &persist_arg])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    let _guard = DaemonGuard { child };
    std::thread::sleep(Duration::from_millis(300));
    let mut s = connect(port);
    let body = wire::encode_name_request(&name(&["durable"]), Endianness::Little).unwrap();
    let reply = call(
        &mut s,
        2,
        ROOT_OBJECT_KEY,
        "resolve",
        body,
        ServiceContextList::default(),
    );
    assert_eq!(
        reply.reply_status,
        ReplyStatusType::NoException,
        "the binding must survive the restart"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// `--version`/`--help` exit 0; an unknown flag exits 1.
#[test]
fn cli_version_help_and_unknown_flag() {
    let out = Command::new(daemon_binary())
        .arg("--version")
        .output()
        .unwrap();
    assert!(out.status.success());
    let out = Command::new(daemon_binary())
        .arg("--help")
        .output()
        .unwrap();
    assert!(out.status.success());
    let out = Command::new(daemon_binary())
        .arg("--not-a-real-flag")
        .output()
        .unwrap();
    assert!(!out.status.success());
}

// ============================================================================
// `CORBA::Object` operations (CORBA 3.3 Part 2 §15.4.2) over the real wire.
//
// omniORB 4.3's `nameclt` narrows `corbaloc::host:port/NameService` with a
// remote `_is_a` before sending any `NamingContext` operation; every
// `nameclt` subcommand failed with `BAD_OPERATION` until this was fixed.
// ============================================================================

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex"))
        .collect()
}

/// Sends `bytes` unmodified — used for the byte-for-byte captured request
/// below, so the test proves the exact frame `nameclt` sent, not a
/// same-shape one this test built itself.
fn send_raw(stream: &mut TcpStream, bytes: &[u8]) {
    stream.write_all(bytes).expect("write raw request");
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
    let bytes = encode_message(version, endianness, false, &Message::Request(req)).expect("encode");
    stream.write_all(&bytes).expect("write request");
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

/// The exact GIOP 1.0 little-endian request omniORB's `nameclt` sends to
/// narrow `corbaloc::127.0.0.1:27101/NameService` — object key
/// `NameService`, operation `_is_a`, argument
/// `IDL:omg.org/CosNaming/NamingContext:1.0`. Before this fix,
/// `zerodds-naming-svc` answered `CORBA::BAD_OPERATION`; every `nameclt`
/// subcommand failed the same way because every one of them narrows
/// first.
const CAPTURED_NAMECLT_IS_A_REQUEST: &str = "\
47494f5001000100580000000000000002000000010000000b0000004e616d65536572766963\
6500060000005f69735f61000000000000002800000049444c3a6f6d672e6f72672f436f734e\
616d696e672f4e616d696e67436f6e746578743a312e3000";

#[test]
fn the_captured_nameclt_is_a_request_gets_a_no_exception_true_reply() {
    let (_guard, port) = spawn_daemon(&[]);
    let mut s = connect(port);
    send_raw(&mut s, &hex_to_bytes(CAPTURED_NAMECLT_IS_A_REQUEST));
    let Message::Reply(reply) = read_giop(&mut s) else {
        panic!("expected a Reply");
    };
    assert_eq!(reply.request_id, 2); // the captured request's own id
    assert_eq!(reply.reply_status, ReplyStatusType::NoException);
    assert!(decode_bool_reply(&reply.body, Endianness::Little));
}

#[test]
fn is_a_is_true_for_every_base_interface_and_corba_object_and_false_for_a_foreign_id() {
    for version in [Version::V1_0, Version::V1_1, Version::V1_2] {
        for endianness in [Endianness::Little, Endianness::Big] {
            let (_guard, port) = spawn_daemon(&[]);
            let mut s = connect(port);
            for (id, expected) in [
                ("IDL:omg.org/CosNaming/NamingContextExt:1.0", true),
                ("IDL:omg.org/CosNaming/NamingContext:1.0", true),
                ("IDL:omg.org/CORBA/Object:1.0", true),
                ("IDL:omg.org/CosNaming/BindingIterator:1.0", false),
                ("IDL:demo/Nope:1.0", false),
            ] {
                let body = encode_is_a_body(id, endianness);
                let reply = call_versioned(
                    &mut s,
                    version,
                    endianness,
                    1,
                    ROOT_OBJECT_KEY,
                    "_is_a",
                    body,
                );
                assert_eq!(
                    reply.reply_status,
                    ReplyStatusType::NoException,
                    "{version:?}/{endianness:?}/{id}"
                );
                assert_eq!(
                    decode_bool_reply(&reply.body, endianness),
                    expected,
                    "{version:?}/{endianness:?}/{id}"
                );
            }
        }
    }
}

#[test]
fn non_existent_and_the_legacy_spelling_are_false_for_the_root_context_true_for_an_unknown_key() {
    let (_guard, port) = spawn_daemon(&[]);
    let mut s = connect(port);
    for op in ["_non_existent", "_not_existent"] {
        let reply = call_versioned(
            &mut s,
            Version::V1_2,
            Endianness::Little,
            1,
            ROOT_OBJECT_KEY,
            op,
            Vec::new(),
        );
        assert_eq!(reply.reply_status, ReplyStatusType::NoException);
        assert!(!decode_bool_reply(&reply.body, Endianness::Little));

        let reply = call_versioned(
            &mut s,
            Version::V1_2,
            Endianness::Little,
            2,
            b"no-such-object",
            op,
            Vec::new(),
        );
        assert_eq!(reply.reply_status, ReplyStatusType::NoException);
        assert!(decode_bool_reply(&reply.body, Endianness::Little));
    }
}

#[test]
fn repository_id_returns_the_naming_context_ext_id() {
    let (_guard, port) = spawn_daemon(&[]);
    let mut s = connect(port);
    let reply = call_versioned(
        &mut s,
        Version::V1_2,
        Endianness::Big,
        1,
        ROOT_OBJECT_KEY,
        "_repository_id",
        Vec::new(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::NoException);
    assert_eq!(
        decode_string_reply(&reply.body, Endianness::Big),
        "IDL:omg.org/CosNaming/NamingContextExt:1.0"
    );
}

#[test]
fn interface_without_an_interface_repository_is_intf_repos() {
    let (_guard, port) = spawn_daemon(&[]);
    let mut s = connect(port);
    let reply = call_versioned(
        &mut s,
        Version::V1_2,
        Endianness::Little,
        1,
        ROOT_OBJECT_KEY,
        "_interface",
        Vec::new(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::SystemException);
    let text = String::from_utf8_lossy(&reply.body);
    assert!(text.contains("IDL:omg.org/CORBA/INTF_REPOS:1.0"));
}

#[test]
fn domain_managers_is_empty_and_component_is_nil() {
    let (_guard, port) = spawn_daemon(&[]);
    let mut s = connect(port);
    let reply = call_versioned(
        &mut s,
        Version::V1_2,
        Endianness::Big,
        1,
        ROOT_OBJECT_KEY,
        "_domain_managers",
        Vec::new(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::NoException);
    let mut r = zerodds_wire::Reader::new(&reply.body, Endianness::Big);
    assert_eq!(r.read_u32().unwrap(), 0);

    let reply = call_versioned(
        &mut s,
        Version::V1_2,
        Endianness::Big,
        2,
        ROOT_OBJECT_KEY,
        "_component",
        Vec::new(),
    );
    assert_eq!(reply.reply_status, ReplyStatusType::NoException);
    let mut r = zerodds_wire::Reader::new(&reply.body, Endianness::Big);
    assert_eq!(r.read_string().unwrap(), "");
    assert_eq!(r.read_u32().unwrap(), 0);
}

#[test]
fn is_a_with_a_truncated_body_is_marshal() {
    let (_guard, port) = spawn_daemon(&[]);
    let mut s = connect(port);
    let reply = call_versioned(
        &mut s,
        Version::V1_2,
        Endianness::Little,
        1,
        ROOT_OBJECT_KEY,
        "_is_a",
        vec![1, 2],
    );
    assert_eq!(reply.reply_status, ReplyStatusType::SystemException);
    let text = String::from_utf8_lossy(&reply.body);
    assert!(text.contains("IDL:omg.org/CORBA/MARSHAL:1.0"));
}

/// A `corbaloc:` client (`nameclt` included) probes the object's location
/// with a `LocateRequest` before invoking on it (GIOP §15.4.5).
#[test]
fn locate_request_answers_object_here_and_unknown_object() {
    let (_guard, port) = spawn_daemon(&[]);
    let mut s = connect(port);

    let lr = zerodds_corba_giop::LocateRequest {
        request_id: 1,
        target: TargetAddress::Key(ROOT_OBJECT_KEY.to_vec()),
    };
    let bytes = encode_message(
        Version::V1_2,
        Endianness::Little,
        false,
        &Message::LocateRequest(lr),
    )
    .unwrap();
    s.write_all(&bytes).unwrap();
    let Message::LocateReply(reply) = read_giop(&mut s) else {
        panic!("expected a LocateReply");
    };
    assert_eq!(reply.locate_status, LocateStatusType::ObjectHere);

    let lr = zerodds_corba_giop::LocateRequest {
        request_id: 2,
        target: TargetAddress::Key(b"no-such-object".to_vec()),
    };
    let bytes = encode_message(
        Version::V1_2,
        Endianness::Little,
        false,
        &Message::LocateRequest(lr),
    )
    .unwrap();
    s.write_all(&bytes).unwrap();
    let Message::LocateReply(reply) = read_giop(&mut s) else {
        panic!("expected a LocateReply");
    };
    assert_eq!(reply.locate_status, LocateStatusType::UnknownObject);
}
