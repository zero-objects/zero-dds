// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 Part 2 §7.7.2 and §7.9 in the request path of
//! `CorbaServer` and `IiopCorbaConnection`.
//!
//! A received context outside the OMG range, a standard one that Table
//! 7.2 does not have in the message's GIOP version and one of a later
//! GIOP version raise no exception and reach the server interceptors;
//! the client sends the contexts it originates only in the GIOP versions
//! Table 7.2 has them in and keeps the codeset context ahead of the
//! others (§7.10.2.6).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};
use zerodds_corba_interop::string_argument;

use zerodds_corba_ccm::portable_interceptor::{
    ClientRequestInterceptor, InitializerError, OrbInitInfo, OrbInitializer, PointResult,
    RequestInfo, ServerRequestInterceptor, register_orb_initializer,
};
use zerodds_corba_giop::{
    CodeSetContext, Message, Reply, ReplyStatusType, Request, ResponseFlags, ServiceContext,
    ServiceContextList, ServiceContextTag, TargetAddress, Version,
};
use zerodds_corba_iiop::{Connector, ConnectorConfig, IiopProfileBody, IiopVersion};
use zerodds_corba_interop::runtime::{CorbaServer, IiopCorbaConnection, orb_init};
use zerodds_corba_interop::{decode_string_body, encode_string_body};
use zerodds_corba_rust::{
    CorbaConnection, CorbaException, ObjectReference, SkeletonResult, SystemExceptionKind,
};
use zerodds_wire::Endianness;

/// A vendor service context id (VSCID 0x4a4300, outside the OMG range).
const VENDOR_CTX: u32 = 0x4a43_0001;
/// `RMICustomMaxStreamFormat`: Table 7.2 has it in GIOP 1.4 only.
const RMI_CUSTOM: u32 = 17;
/// `FIREWALL_PATH_RESP`: a standard id without a row in Table 7.2.
const FIREWALL_PATH_RESP: u32 = 21;
/// `CodeSets`: Table 7.2 has it from GIOP 1.1 on.
const CODE_SETS: u32 = 1;

/// The contexts of one request, as `(id, data)`.
type Contexts = Vec<(u32, Vec<u8>)>;

#[derive(Default)]
struct Seen {
    /// Per request: the contexts `receive_request_service_contexts`
    /// found.
    server: Mutex<Vec<Contexts>>,
}

struct ServerSide(Arc<Seen>);

impl ServerRequestInterceptor for ServerSide {
    fn name(&self) -> &str {
        "sc-server"
    }
    fn receive_request_service_contexts(&self, info: &mut RequestInfo) -> PointResult {
        let all = info
            .request_service_contexts()
            .iter()
            .map(|(id, data)| (*id, data.clone()))
            .collect();
        self.0.server.lock().unwrap().push(all);
        Ok(())
    }
}

/// Replaces the codeset context of every request with its own.
struct CodesetClient;

impl ClientRequestInterceptor for CodesetClient {
    fn name(&self) -> &str {
        "sc-codeset-client"
    }
    fn send_request(&self, info: &mut RequestInfo) -> PointResult {
        let own = CodeSetContext {
            char_data: 0x0001_0001,
            wchar_data: 0x0001_0109,
        }
        .to_service_context(Endianness::Big)
        .unwrap();
        info.add_request_service_context(CODE_SETS, own.context_data, true)?;
        Ok(())
    }
}

struct Init(Arc<Seen>);

impl OrbInitializer for Init {
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), InitializerError> {
        match info.orb_id() {
            "sc-server" => {
                info.add_server_request_interceptor(Arc::new(ServerSide(self.0.clone())))?;
            }
            "sc-codeset-client" => {
                info.add_client_request_interceptor(Arc::new(CodesetClient))?;
            }
            _ => {}
        }
        Ok(())
    }
}

/// An echo server bound to an ORB whose server interceptor records the
/// received contexts; `observed` gets every received list in order.
fn echo_server(
    seen: &Arc<Seen>,
) -> (
    zerodds_corba_iiop::Acceptor,
    Arc<Mutex<Vec<Vec<ServiceContext>>>>,
) {
    register_orb_initializer(Arc::new(Init(seen.clone())));
    let orb = orb_init(&mut Vec::new(), "sc-server").unwrap();
    let server = CorbaServer::new();
    server.set_orb(orb);
    server.register(b"Echo", |op, body, _e| match op {
        "ping" => SkeletonResult::Reply(body.bytes().to_vec()),
        _ => SkeletonResult::BadOperation,
    });
    let observed = Arc::new(Mutex::new(Vec::new()));
    {
        let observed = observed.clone();
        server.on_request_contexts(move |list| observed.lock().unwrap().push(list.0.clone()));
    }
    (
        server.serve("127.0.0.1:0".parse().unwrap()).unwrap(),
        observed,
    )
}

/// A reference to `Echo` at `addr` with one IIOP profile of `version`.
fn echo_ref(addr: std::net::SocketAddr, version: IiopVersion) -> ObjectReference {
    let body = IiopProfileBody::new(
        version,
        addr.ip().to_string(),
        addr.port(),
        b"Echo".to_vec(),
    );
    ObjectReference {
        type_id: "IDL:Echo:1.0".into(),
        iiop_profile: body.encode_encapsulation(Endianness::Big).unwrap(),
        policy_overrides: Vec::new(),
        profiles: Vec::new(),
    }
}

/// Sends `ping` with `contexts` in GIOP `version` on a plain connection
/// and returns the reply.
fn raw_ping(addr: std::net::SocketAddr, version: Version, contexts: Vec<ServiceContext>) -> Reply {
    let connector = Connector::new(ConnectorConfig::default());
    let mut pooled = connector
        .connect(&addr.ip().to_string(), addr.port())
        .unwrap();
    let conn = pooled.connection().unwrap();
    let request = Message::Request(Request {
        request_id: 9,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(b"Echo".to_vec()),
        operation: "ping".into(),
        requesting_principal: Some(Vec::new()),
        service_context: ServiceContextList(contexts),
        body: encode_string_body("hi"),
    });
    conn.write_message(version, Endianness::Big, false, &request)
        .unwrap();
    let (reply, _e, reply_version) = conn.read_message_full().unwrap();
    assert_eq!(reply_version, version);
    match reply {
        Message::Reply(r) => r,
        other => panic!("expected a Reply, got {other:?}"),
    }
}

fn ids(list: &[ServiceContext]) -> Vec<u32> {
    list.iter().map(|c| c.context_id).collect()
}

/// §7.7.2: a context outside the OMG range, the codeset context in a
/// GIOP 1.0 request, a context of GIOP 1.4 and a standard id without a
/// row in Table 7.2: no exception, the call is answered, every context
/// reaches the server interceptor with its data and the observer sees
/// the list as it arrived.
#[test]
fn corba_p2_7_7_2_foreign_contexts_reach_the_server_interceptor_without_exception() {
    let seen = Arc::new(Seen::default());
    let (acceptor, observed) = echo_server(&seen);
    let addr = acceptor.listen_addr();
    let contexts = vec![
        ServiceContext::new(CODE_SETS, vec![0, 0, 0, 0, 0, 1, 0, 1, 0, 1, 1, 9]),
        ServiceContext::new(RMI_CUSTOM, vec![0, 2]),
        ServiceContext::new(FIREWALL_PATH_RESP, vec![0, 3]),
        ServiceContext::new(VENDOR_CTX, b"vendor".to_vec()),
    ];
    for version in [Version::V1_0, Version::V1_1, Version::V1_2] {
        let reply = raw_ping(addr, version, contexts.clone());
        assert_eq!(
            reply.reply_status,
            ReplyStatusType::NoException,
            "{version:?}"
        );
        assert_eq!(decode_string_body(&reply.body).unwrap(), "hi");
    }
    acceptor.shutdown();
    let server = seen.server.lock().unwrap();
    assert_eq!(server.len(), 3);
    for per_request in server.iter() {
        let expected: Contexts = {
            let mut v: Vec<_> = contexts
                .iter()
                .map(|c| (c.context_id, c.context_data.clone()))
                .collect();
            v.sort_by_key(|(id, _)| *id);
            v
        };
        assert_eq!(*per_request, expected);
    }
    let observed = observed.lock().unwrap();
    assert_eq!(observed.len(), 3);
    for list in observed.iter() {
        assert_eq!(ids(list), ids(&contexts));
    }
}

/// Table 7.2: the client sends no codeset context in GIOP 1.0, one from
/// 1.1 on, first in the list.
#[test]
fn corba_p2_7_9_a_1_0_request_carries_no_codesets_context() {
    let seen = Arc::new(Seen::default());
    let (acceptor, observed) = echo_server(&seen);
    let addr = acceptor.listen_addr();
    let conn = IiopCorbaConnection::new();
    for version in [IiopVersion::V1_0, IiopVersion::V1_1, IiopVersion::V1_2] {
        let (body, _) = conn
            .invoke(
                &echo_ref(addr, version),
                "ping",
                Endianness::Big,
                &string_argument("x"),
            )
            .unwrap();
        assert_eq!(decode_string_body(&body).unwrap(), "x");
    }
    acceptor.shutdown();
    let observed = observed.lock().unwrap();
    assert_eq!(observed.len(), 3);
    assert!(!ids(&observed[0]).contains(&CODE_SETS), "GIOP 1.0");
    assert_eq!(ids(&observed[1]).first(), Some(&CODE_SETS), "GIOP 1.1");
    assert_eq!(ids(&observed[2]).first(), Some(&CODE_SETS), "GIOP 1.2");
}

/// §7.10.2.6: the codeset context precedes the contexts that depend on
/// it. A client interceptor that replaces the codeset context does not
/// move it behind the SAS context.
#[test]
fn corba_p2_7_10_2_6_codesets_context_precedes_the_sas_context() {
    let seen = Arc::new(Seen::default());
    let (acceptor, observed) = echo_server(&seen);
    let addr = acceptor.listen_addr();
    let client_orb = orb_init(&mut Vec::new(), "sc-codeset-client").unwrap();
    let conn = IiopCorbaConnection::new()
        .with_csiv2_credentials("alice", "secret")
        .with_orb(&client_orb);
    conn.invoke(
        &echo_ref(addr, IiopVersion::V1_2),
        "ping",
        Endianness::Big,
        &string_argument("z"),
    )
    .unwrap();
    acceptor.shutdown();
    let observed = observed.lock().unwrap();
    let list = &observed[0];
    let sas = ServiceContextTag::SecurityAttributeService.as_u32();
    assert_eq!(ids(list)[..2], [CODE_SETS, sas]);
    let own = CodeSetContext {
        char_data: 0x0001_0001,
        wchar_data: 0x0001_0109,
    }
    .to_service_context(Endianness::Big)
    .unwrap();
    assert_eq!(list[0].context_data, own.context_data, "the interceptor's");
}

/// Table 7.2: the `SecurityAttributeService` context is not part of GIOP
/// 1.0 and 1.1. A client with CSIv2 credentials does not send its
/// request there without them: `NO_PERMISSION`, `COMPLETED_NO`, nothing
/// reaches the server.
#[test]
fn corba_p2_7_9_credentials_are_not_dropped_below_giop_1_2() {
    let seen = Arc::new(Seen::default());
    let (acceptor, observed) = echo_server(&seen);
    let addr = acceptor.listen_addr();
    let conn = IiopCorbaConnection::new().with_csiv2_credentials("alice", "secret");
    for version in [IiopVersion::V1_0, IiopVersion::V1_1] {
        let err = conn
            .invoke(
                &echo_ref(addr, version),
                "ping",
                Endianness::Big,
                &string_argument("w"),
            )
            .unwrap_err();
        match err {
            CorbaException::System(e) => {
                assert_eq!(e.kind, SystemExceptionKind::NoPermission);
                assert_eq!(e.completed, zerodds_corba_rust::CompletionStatus::No);
            }
            other => panic!("{other:?}"),
        }
    }
    acceptor.shutdown();
    assert!(observed.lock().unwrap().is_empty());
}
