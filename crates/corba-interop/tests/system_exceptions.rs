// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 Part 1 §8.12 over IIOP: a system exception a servant raises
//! reaches the client with its repository id, minor code and completion
//! status, and the reply body on the wire names the exception's own id.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{Read, Write};
use std::net::TcpStream;
use zerodds_corba_rust::no_arguments;

use zerodds_corba_giop::{
    Message, Reply, ReplyStatusType, Request, ResponseFlags, ServiceContextList, TargetAddress,
    Version,
};
use zerodds_corba_interop::runtime::{CorbaServer, IiopCorbaConnection, object_reference};
use zerodds_corba_rt::OMGVMCID;
use zerodds_corba_rust::{
    CompletionStatus, CorbaConnection, CorbaException, SkeletonResult, SystemException,
    SystemExceptionKind,
};
use zerodds_wire::{Endianness, Reader};

fn server() -> (zerodds_corba_iiop::Acceptor, String, u16) {
    let server = CorbaServer::new();
    server.register(b"Raiser", |op, _body, _e| match op {
        "bad_param" => SkeletonResult::Exception(CorbaException::omg(
            SystemExceptionKind::BadParam,
            7,
            CompletionStatus::Maybe,
        )),
        "vendor_minor" => SkeletonResult::Exception(CorbaException::system(
            SystemExceptionKind::Transient,
            0x5a44_0003,
            CompletionStatus::No,
        )),
        "ok" => SkeletonResult::Reply(Vec::new()),
        _ => SkeletonResult::BadOperation,
    });
    let acceptor = server.serve("127.0.0.1:0".parse().unwrap()).unwrap();
    let addr = acceptor.listen_addr();
    (acceptor, addr.ip().to_string(), addr.port())
}

#[test]
fn corba_p1_8_12_a_system_exception_crosses_with_kind_minor_and_completion() {
    let (acceptor, host, port) = server();
    let ior = object_reference("IDL:Raiser:1.0", &host, port, b"Raiser");
    let conn = IiopCorbaConnection::new();

    let err = conn
        .invoke(&ior, "bad_param", Endianness::Big, &no_arguments)
        .unwrap_err();
    assert_eq!(
        err,
        CorbaException::System(SystemException::new(
            SystemExceptionKind::BadParam,
            OMGVMCID | 7,
            CompletionStatus::Maybe
        ))
    );

    let err = conn
        .invoke(&ior, "vendor_minor", Endianness::Little, &no_arguments)
        .unwrap_err();
    assert_eq!(
        err.as_system().copied(),
        Some(SystemException::new(
            SystemExceptionKind::Transient,
            0x5a44_0003,
            CompletionStatus::No
        )),
        "a vendor minor code is carried unchanged"
    );

    // An operation the servant does not know: BAD_OPERATION minor 2
    // (Annex A.6, "Operation or attribute not known to target object").
    let err = conn
        .invoke(&ior, "no_such_op", Endianness::Big, &no_arguments)
        .unwrap_err();
    assert_eq!(
        err.as_system().copied(),
        Some(SystemException::omg(
            SystemExceptionKind::BadOperation,
            2,
            CompletionStatus::No
        ))
    );
    acceptor.shutdown();
}

/// The reply as a raw GIOP message: its body starts with the
/// exception's own repository id, not `UNKNOWN`.
#[test]
fn corba_p2_9_4_3_the_system_exception_reply_body_names_the_exception() {
    let (acceptor, host, port) = server();
    let mut stream = TcpStream::connect((host.as_str(), port)).unwrap();
    let version = Version::V1_2;
    let request = Message::Request(Request {
        request_id: 41,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(b"Raiser".to_vec()),
        operation: "bad_param".into(),
        requesting_principal: None,
        service_context: ServiceContextList::default(),
        body: Vec::new(),
    });
    let frame =
        zerodds_corba_giop::encode_message(version, Endianness::Big, false, &request).unwrap();
    stream.write_all(&frame).unwrap();

    let mut header = [0u8; 12];
    stream.read_exact(&mut header).unwrap();
    let size = u32::from_be_bytes([header[8], header[9], header[10], header[11]]) as usize;
    let mut rest = vec![0u8; size];
    stream.read_exact(&mut rest).unwrap();
    let mut whole = header.to_vec();
    whole.extend_from_slice(&rest);
    let (msg, _) = zerodds_corba_giop::decode_message(&whole).unwrap();
    let Message::Reply(Reply {
        request_id,
        reply_status,
        body,
        ..
    }) = msg
    else {
        panic!("expected a Reply, got {msg:?}");
    };
    assert_eq!(request_id, 41);
    assert_eq!(reply_status, ReplyStatusType::SystemException);
    let mut r = Reader::new(&body, Endianness::Big);
    assert_eq!(r.read_string().unwrap(), "IDL:omg.org/CORBA/BAD_PARAM:1.0");
    assert_eq!(r.read_u32().unwrap(), 0x4f4d_0007);
    assert_eq!(r.read_u32().unwrap(), 2, "COMPLETED_MAYBE");
    acceptor.shutdown();
}
