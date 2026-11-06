// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `zerodds-corba-bridge-1.0` §4.3 and §4.4: the DDS samples carry the
//! GIOP body as plain XCDR1, the representation identifier `CDR_BE` or
//! `CDR_LE` (DDS-XTypes 1.3 §7.6.3.1.2, Table 60). A sample under another
//! identifier — here `PL_CDR_LE`, an `@mutable` type in XCDR1, whose body is
//! a parameter list — is no GIOP body: a reply sample is answered `MARSHAL`,
//! a notify sample is not sent.
//!
//! The bridge runs in this process, the DDS side is a runtime of this
//! test, the CORBA side a GIOP 1.2 peer on a socket of this test.

#![cfg(feature = "std")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

mod common;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Bridge;
use zerodds_corba_dds_bridge::bridge_daemon::{DaemonConfig, object_key};
use zerodds_corba_giop::version::Version;
use zerodds_corba_giop::{
    LocateReply, LocateStatusType, Message, ReplyStatusType, Request, ResponseFlags,
    ServiceContextList, TargetAddress, decode_message, encode_message,
};
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_rtps::publication_data::data_representation as dr;
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};
use zerodds_types::qos::ExtensibilityForRepr;
use zerodds_wire::Endianness;

/// A runtime of its own on `domain`, with a prefix no other runtime of
/// this process has.
fn runtime(domain: i32, tag: u8) -> Arc<DcpsRuntime> {
    let mut prefix = [0u8; 12];
    prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
    prefix[4..8].copy_from_slice(&domain.to_be_bytes());
    prefix[8] = tag;
    prefix[11] = 0xd5;
    DcpsRuntime::start(
        domain,
        GuidPrefix::from_bytes(prefix),
        RuntimeConfig::default(),
    )
    .unwrap()
}

/// A writer on `topic` that offers XCDR and writes its samples under
/// `PL_CDR` (`mutable`) or `CDR` (otherwise).
fn writer(dds: &DcpsRuntime, topic: &str, mutable: bool) -> EntityId {
    let w = dds
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
    dds.set_user_writer_data_rep_override(w, Some(vec![dr::XCDR]))
        .unwrap();
    dds.set_user_writer_byte_order_override(w, false).unwrap();
    if mutable {
        dds.set_user_writer_wire_extensibility(w, ExtensibilityForRepr::Mutable)
            .unwrap();
    }
    w
}

/// Waits for a match event until `matched` holds.
fn until(dds: &DcpsRuntime, what: &str, matched: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !matched() {
        assert!(Instant::now() < deadline, "{what}");
        dds.wait_match_event(Duration::from_millis(200));
    }
}

/// One GIOP message from `stream`.
fn read_giop(stream: &mut TcpStream) -> Message {
    let mut full = vec![0u8; 12];
    stream.read_exact(&mut full).expect("a GIOP header");
    assert_eq!(&full[..4], b"GIOP");
    let size = if full[6] & 0x01 != 0 {
        u32::from_le_bytes([full[8], full[9], full[10], full[11]])
    } else {
        u32::from_be_bytes([full[8], full[9], full[10], full[11]])
    };
    full.resize(12 + size as usize, 0);
    stream.read_exact(&mut full[12..]).expect("a GIOP body");
    decode_message(&full).expect("a GIOP message").0
}

/// §4.3: a reply sample under `PL_CDR_LE` is answered
/// `CORBA::MARSHAL`, not passed on as the reply body.
#[test]
fn corbabridge_4_3_a_reply_sample_that_is_not_plain_cdr_is_marshal() {
    let lease = common::lease();
    let iiop_port = common::free_port();
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("plain-cdr-reply");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ior_file = dir.join("bridge.ior");
    let yaml = format!(
        "domain: {d}\n\
         corba:\n  iiop:\n    bind: \"127.0.0.1:{iiop_port}\"\n  ior:\n    ior_file: \"{ior}\"\n\
         mappings:\n  - repo_id: \"IDL:zerodds_test/Calc:1.0\"\n    operation: add\n    request_topic:\n      dds_name: Calc_add_Request\n    reply_topic:\n      dds_name: Calc_add_Reply\n    timeout_ms: 5000\n",
        d = lease.id(),
        ior = ior_file.display(),
    );
    let mut config = zerodds_config::Config::default();
    config
        .push(
            zerodds_config::parse_str(&yaml, "<test>").unwrap(),
            "<test>".into(),
        )
        .unwrap();
    let key = object_key(&DaemonConfig::from_config(Some(&config)).unwrap().mappings[0]);
    let bridge = Bridge::start(&yaml, &ior_file);

    // The backend: reads the request, answers under PL_CDR_LE.
    let dds = runtime(lease.id(), 1);
    let (requests_id, requests) = dds
        .register_user_reader_kind(
            UserReaderConfig {
                topic_name: "Calc_add_Request".into(),
                type_name: "Calc_add_Request".into(),
                reliable: true,
                ..Default::default()
            },
            false,
        )
        .unwrap();
    let replies = writer(&dds, "Calc_add_Reply", true);
    until(
        &dds,
        "the bridge's request writer and reply reader did not match",
        || {
            dds.user_reader_matched_count(requests_id) > 0
                && dds.user_writer_matched_count(replies) > 0
        },
    );

    let mut client = TcpStream::connect(("127.0.0.1", iiop_port)).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut body = Vec::new();
    body.extend_from_slice(&2i64.to_le_bytes());
    body.extend_from_slice(&40i64.to_le_bytes());
    client
        .write_all(
            &encode_message(
                Version::V1_2,
                Endianness::Little,
                false,
                &Message::Request(Request {
                    request_id: 5,
                    response_flags: ResponseFlags::SYNC_WITH_TARGET,
                    target: TargetAddress::Key(key),
                    operation: "add".into(),
                    requesting_principal: None,
                    service_context: ServiceContextList::default(),
                    body,
                }),
            )
            .unwrap(),
        )
        .unwrap();

    let Ok(UserSample::Alive { payload, .. }) = requests.recv_timeout(Duration::from_secs(10))
    else {
        panic!("the bridge published no request sample");
    };
    // request_id of the bridge, reply_status 0, reserved 0, the sum.
    let mut reply = payload.as_ref()[..8].to_vec();
    reply.extend_from_slice(&[0; 8]);
    reply.extend_from_slice(&42i64.to_le_bytes());
    dds.write_user_sample_borrowed(replies, &reply).unwrap();

    let Message::Reply(r) = read_giop(&mut client) else {
        panic!("the bridge did not reply");
    };
    assert_eq!(r.request_id, 5);
    assert_eq!(r.reply_status, ReplyStatusType::SystemException);
    let id = b"IDL:omg.org/CORBA/MARSHAL:1.0";
    assert!(
        r.body.windows(id.len()).any(|w| w == id),
        "MARSHAL in {:02x?}",
        r.body
    );

    bridge.stop();
    dds.shutdown();
}

/// §4.4: a notify sample under `PL_CDR_LE` is not sent; the next sample,
/// under `CDR_LE`, is the first request the target gets.
#[test]
fn corbabridge_4_4_a_notify_sample_that_is_not_plain_cdr_is_not_sent() {
    let lease = common::lease();
    let iiop_port = common::free_port();
    let target = TcpListener::bind("127.0.0.1:0").unwrap();
    let target_port = target.local_addr().unwrap().port();
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("plain-cdr-notify");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let ior_file = dir.join("bridge.ior");
    let yaml = format!(
        "domain: {d}\n\
         corba:\n  iiop:\n    bind: \"127.0.0.1:{iiop_port}\"\n  ior:\n    ior_file: \"{ior}\"\n\
         mappings:\n  - repo_id: \"IDL:zerodds_test/Listener:1.0\"\n    operation: on_value\n    direction: dds_to_corba\n    notify_topic:\n      dds_name: ListenerValues\n    targets:\n      - \"corbaloc:iiop:1.2@127.0.0.1:{target_port}/Listener\"\n",
        d = lease.id(),
        ior = ior_file.display(),
    );
    let bridge = Bridge::start(&yaml, &ior_file);

    let dds = runtime(lease.id(), 2);
    let mutable = writer(&dds, "ListenerValues", true);
    let plain = writer(&dds, "ListenerValues", false);
    until(&dds, "the bridge's notify reader did not match", || {
        dds.user_writer_matched_count(mutable) > 0 && dds.user_writer_matched_count(plain) > 0
    });

    // The PL_CDR sample first, received by the bridge before the plain one
    // is written: had the bridge sent it, it would be the first request.
    dds.write_user_sample_borrowed(mutable, &7i64.to_le_bytes())
        .unwrap();
    assert!(
        dds.await_user_writer_acknowledged(mutable, Instant::now() + Duration::from_secs(10)),
        "the bridge did not acknowledge the PL_CDR sample"
    );
    dds.write_user_sample_borrowed(plain, &8i64.to_le_bytes())
        .unwrap();

    let (mut conn, _) = target.accept().unwrap();
    conn.set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    // The bridge resolves the `corbaloc:` target first (GIOP §15.4.5).
    let r = loop {
        match read_giop(&mut conn) {
            Message::LocateRequest(lr) => conn
                .write_all(
                    &encode_message(
                        Version::V1_2,
                        Endianness::Little,
                        false,
                        &Message::LocateReply(LocateReply {
                            request_id: lr.request_id,
                            locate_status: LocateStatusType::ObjectHere,
                            body: Vec::new(),
                        }),
                    )
                    .unwrap(),
                )
                .unwrap(),
            Message::Request(r) => break r,
            other => panic!("expected a request, got {:?}", other.message_type()),
        }
    };
    assert_eq!(r.operation, "on_value");
    assert!(
        !r.response_flags.response_expected(),
        "a notify is a oneway request"
    );
    assert_eq!(
        r.body,
        8i64.to_le_bytes(),
        "the first request is the plain sample"
    );

    bridge.stop();
    dds.shutdown();
}
