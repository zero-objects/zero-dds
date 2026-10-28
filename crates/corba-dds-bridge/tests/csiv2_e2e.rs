// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// E2E tests for CSIv2 (§7.2, CORBA 3.4 Part 2 §10) in `zerodds-corba-bridged`:
// a real GIOP client with GSSUP credentials against the real spawned
// daemon. `acl.default_deny: true` with a rule naming only the
// GSSUP-authenticated identity makes the three outcomes observable
// without a live DDS subscriber on the other end:
//
// * valid credentials -> CSIv2 accepts, ACL allows -> the request
//   proceeds to DDS and (nothing subscribes) eventually TIMEOUT, with
//   a SAS CompleteEstablishContext attached to that later reply --
//   proving the async Pending -> sas_reply threading works, not just
//   the synchronous refusal paths.
// * wrong password -> CSIv2 rejects -> NO_PERMISSION, immediately,
//   with a SAS ContextError attached.
// * no SecurityAttributeService context at all -> CSIv2 has nothing to
//   evaluate (additive, not required) -> falls through to the ACL,
//   which denies every unauthenticated caller here -> NO_PERMISSION,
//   with no SAS context attached (the denial is ACL's, not CSIv2's).
//
// Each test spawns its own real DDS participant. Run with
// `--test-threads=1`: three such daemons starting at once on one
// macOS host occasionally miss the connect-retry window (known host
// loopback/DDS-discovery flakiness, not a defect these tests found —
// all three pass reliably serialized).

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

use zerodds_corba_csiv2::gssup::GssupCredentialToken;
use zerodds_corba_csiv2::gssup_store::format_entry;
use zerodds_corba_csiv2::sas::{EstablishContext, IdentityToken, SasMessage};
use zerodds_corba_giop::version::Version;
use zerodds_corba_giop::{
    Message, Reply, ReplyStatusType, Request, ResponseFlags, ServiceContext, ServiceContextList,
    ServiceContextTag, TargetAddress, decode_message, encode_message,
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
        #[cfg(unix)]
        {
            let pid = self.child.id() as i32;
            // SAFETY: `child.id()` is valid before `wait`; `libc::kill`
            // is always call-safe.
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            loop {
                match self.child.try_wait() {
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
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        #[cfg(not(unix))]
        {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn pick_port() -> u16 {
    let s = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral");
    let p = s.local_addr().expect("addr").port();
    drop(s);
    p
}

/// Spawns the daemon against a YAML config naming one mapping, CSIv2
/// (a GSSUP password file for `alice`/`secret`) and an ACL that allows
/// only `gssup:alice`.
fn spawn_daemon() -> (DaemonGuard, u16, PathBuf) {
    let port = pick_port();
    // One directory per spawned daemon: the tests of this file run in
    // parallel threads of one process, and a directory named after the
    // process alone let one test's config (with its port) overwrite
    // another's between the write and the daemon reading it.
    let dir = std::env::temp_dir().join(format!("zd-csiv2-e2e-{}-{port}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let pw_file = dir.join("gssup.passwd");
    std::fs::write(&pw_file, format_entry("alice", "secret", b"e2e-salt", 1000)).unwrap();

    // A domain unique per spawned daemon, not just per test *process* —
    // three tests in this file each spawn their own daemon and need one
    // each. `unique_domain()` leases host-wide (the domain number is the
    // UDP port), so a concurrent run of any other test binary is safe
    // too (`crates/dcps/tests/domain_hygiene.rs`'s
    // no_domain_is_computed_from_a_process_id_or_a_counter).
    let domain = zerodds_test_domains::unique_domain();
    let yaml = format!(
        "domain: {domain}\n\
         corba:\n\
         \x20 iiop:\n\
         \x20   bind: \"127.0.0.1:{port}\"\n\
         \x20 csiv2:\n\
         \x20   enabled: true\n\
         \x20   gssup:\n\
         \x20     password_file: \"{}\"\n\
         acl:\n\
         \x20 default_deny: true\n\
         \x20 rules:\n\
         \x20   - subject: \"gssup:alice\"\n\
         \x20     allow_invoke: [\"*\"]\n\
         mappings:\n\
         \x20 - repo_id: \"IDL:zerodds_test/Echo:1.0\"\n\
         \x20   operation: call\n\
         \x20   timeout_ms: 400\n",
        pw_file.display()
    );
    let cfg_path = dir.join("corba-bridged.yaml");
    std::fs::write(&cfg_path, yaml).unwrap();

    let child = Command::new(daemon_binary())
        .args(["--config", cfg_path.to_str().unwrap()])
        .args(["--idle-timeout-ms", "8000"])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn daemon");
    std::thread::sleep(Duration::from_millis(800));
    (DaemonGuard { child }, port, dir)
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

fn security_attribute_service_id() -> u32 {
    ServiceContextTag::SecurityAttributeService.as_u32()
}

fn csiv2_context(username: &str, password: &str) -> ServiceContextList {
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
    list.0
        .push(ServiceContext::new(security_attribute_service_id(), sas));
    list
}

/// The one mapping every test daemon in this file configures:
/// `IDL:zerodds_test/Echo:1.0` / `call`. `slug()` drops repo_id's first
/// path segment as the prefix pragma, so the derived topics are
/// `Echo::call::{Request,Reply}`.
fn echo_object_key() -> Vec<u8> {
    object_key_for(
        "IDL:zerodds_test/Echo:1.0",
        "Echo::call::Request",
        "Echo::call::Reply",
    )
}

fn call(stream: &mut TcpStream, request_id: u32, service_context: ServiceContextList) -> Reply {
    let req = Request {
        request_id,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        // The real object key of the daemon's one mapping — CSIv2 is
        // evaluated before the object lookup and the ACL after it, so a
        // real key is needed to observe the ACL outcome specifically
        // (a wrong key would answer OBJECT_NOT_EXIST first).
        target: TargetAddress::Key(echo_object_key()),
        operation: "call".into(),
        requesting_principal: None,
        service_context,
        body: vec![0xCA, 0xFE],
    };
    let bytes = encode_message(
        Version::V1_2,
        Endianness::Little,
        false,
        &Message::Request(req),
    )
    .expect("encode");
    stream.write_all(&bytes).expect("write request");
    match read_giop(stream) {
        Message::Reply(r) => r,
        other => panic!("expected Reply, got {:?}", other.message_type()),
    }
}

fn object_key_for(repo_id: &str, request_topic: &str, reply_topic: &str) -> Vec<u8> {
    // Mirrors `bridge_daemon::object_key`: SHA-256(repo_id + "\0" + "req->reply")[..16].
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(repo_id.as_bytes());
    h.update(b"\0");
    h.update(format!("{request_topic}->{reply_topic}").as_bytes());
    h.finalize()[..16].to_vec()
}

fn has_sas_context(reply: &Reply) -> bool {
    reply
        .service_context
        .0
        .iter()
        .any(|c| c.context_id == security_attribute_service_id())
}

fn system_exception_name(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    text.split("IDL:omg.org/CORBA/")
        .nth(1)
        .and_then(|t| t.split(':').next())
        .unwrap_or_default()
        .to_string()
}

/// Wrong password: CSIv2 rejects, NO_PERMISSION immediately, with a SAS
/// `ContextError` attached — no DDS backend needed since this refusal
/// happens before the target is even looked up.
#[test]
fn csiv2_wrong_password_is_no_permission_with_a_context_error() {
    let (_guard, port, dir) = spawn_daemon();
    let mut s = connect(port);
    let reply = call(&mut s, 1, csiv2_context("alice", "wrong-password"));
    assert_eq!(reply.reply_status, ReplyStatusType::SystemException);
    assert_eq!(system_exception_name(&reply.body), "NO_PERMISSION");
    assert!(has_sas_context(&reply), "a ContextError is attached");
    let _ = std::fs::remove_dir_all(&dir);
}

/// No `SecurityAttributeService` context at all: CSIv2 has nothing to
/// evaluate (additive here, unlike `zerodds-naming-svc`'s required
/// stance), so the request falls through to the ACL, which denies every
/// caller without the `gssup:alice` identity — NO_PERMISSION, but no SAS
/// context attached (this is the ACL's refusal, not CSIv2's).
#[test]
fn missing_credentials_falls_through_to_the_acl_which_denies() {
    let (_guard, port, dir) = spawn_daemon();
    let mut s = connect(port);
    let reply = call(&mut s, 1, ServiceContextList::default());
    assert_eq!(reply.reply_status, ReplyStatusType::SystemException);
    assert_eq!(system_exception_name(&reply.body), "NO_PERMISSION");
    assert!(
        !has_sas_context(&reply),
        "no SAS context: this is an ACL denial, not a CSIv2 one"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Valid credentials: CSIv2 accepts (`gssup:alice`), the ACL allows it,
/// so the request proceeds to DDS. Nothing subscribes, so it times out
/// -- and that later, asynchronous reply still carries the SAS
/// `CompleteEstablishContext` this request's CSIv2 evaluation produced,
/// proving the accept path threads through `Pending`, not only the
/// synchronous refusal paths.
#[test]
fn csiv2_accepted_credentials_proceed_past_acl_to_a_dds_timeout() {
    let (_guard, port, dir) = spawn_daemon();
    let mut s = connect(port);
    let req = Request {
        request_id: 7,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(echo_object_key()),
        operation: "call".into(),
        requesting_principal: None,
        service_context: csiv2_context("alice", "secret"),
        body: vec![0xCA, 0xFE],
    };
    let bytes = encode_message(
        Version::V1_2,
        Endianness::Little,
        false,
        &Message::Request(req),
    )
    .unwrap();
    s.write_all(&bytes).unwrap();
    let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
    let reply = match read_giop(&mut s) {
        Message::Reply(r) => r,
        other => panic!("expected Reply, got {:?}", other.message_type()),
    };
    assert_eq!(reply.reply_status, ReplyStatusType::SystemException);
    assert_eq!(
        system_exception_name(&reply.body),
        "TIMEOUT",
        "not NO_PERMISSION: CSIv2 + ACL both passed"
    );
    assert!(
        has_sas_context(&reply),
        "CompleteEstablishContext is attached even to the delayed DDS-side reply"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
