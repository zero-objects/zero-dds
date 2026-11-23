// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::missing_panics_doc,
    clippy::missing_errors_doc
)]
//! Crate `zerodds-corba-interop`. Safety classification: **STANDARD**.
//!
//! CORBA speed + cross-ORB interop harness.
//!
//! This crate fills the last runtime gap between the (individually tested)
//! GIOP / IIOP / POA / CDR building blocks: a real **Acceptor↔POA request
//! loop** ([`serve`]) plus a client request helper ([`invoke_on`]), driving a
//! hand-marshalled `Echo` servant.
//!
//! It serves two purposes:
//! * **Speed** — a self-roundtrip latency benchmark (`echo_bench` binary).
//! * **Interop** — the same `serve`/`invoke_on` plumbing is the basis for the
//!   cross-ORB tests against omniORB / TAO / JacORB.
//!
//! Milestone 1 pins the wire to big-endian on both ends; honouring the
//! request byte-order flag (needed for foreign ORBs) is milestone 2.

pub mod mgm;
pub mod miop;
pub mod runtime;

use std::net::SocketAddr;
use std::sync::Arc;

use zerodds_corba_giop::{
    AlignedBody, LocateReply, LocateRequest, LocateStatusType, Message, Reply, ReplyStatusType,
    Request, ResponseFlags, ServiceContextList, TargetAddress, Version,
};
use zerodds_corba_iiop::{Acceptor, AcceptorConfig, Connection, IiopError};
use zerodds_corba_poa::{
    IdAssignmentPolicy, ObjectId, Poa, PoaConfig, PoaError, PoaManager, PolicySet, Servant,
};
use zerodds_wire::{Endianness, Reader, Writer};

/// Wire endianness used by the milestone-1 self-roundtrip (both ends ZeroDDS).
pub const WIRE: Endianness = Endianness::Big;

/// CDR-encode a single `string` value as a GIOP request/reply body.
#[must_use]
pub fn encode_string_body(s: &str) -> Vec<u8> {
    let mut w = Writer::new(WIRE);
    w.write_string(s);
    w.finish()
}

/// The arguments of a request whose one argument is the `string` `s`
/// ([`zerodds_corba_rust::MarshalArgs`]): written where the connection
/// places the body.
pub fn string_argument(
    s: &str,
) -> impl Fn(&mut Writer) -> Result<(), zerodds_corba_rust::CorbaException> + '_ {
    move |w: &mut Writer| {
        w.write_string(s);
        Ok(())
    }
}

/// CDR-decode a single `string` value from a GIOP body.
///
/// # Errors
/// A body that does not hold a CDR `string`: the error, not an empty
/// string that reads like one.
pub fn decode_string_body(body: &[u8]) -> Result<String, zerodds_wire::WireError> {
    Reader::new(body, WIRE).read_string()
}

/// `Echo` servant — `string ping(in string msg)` echoes its argument.
#[derive(Debug, Default)]
pub struct EchoServant;

impl Servant for EchoServant {
    fn primary_interface(&self) -> String {
        "IDL:demo/Echo:1.0".to_string()
    }

    fn invoke(
        &self,
        operation: &str,
        request_body: &AlignedBody,
        endianness: Endianness,
    ) -> Vec<u8> {
        match operation {
            // `Servant::invoke` has no exception path: a body that is not a
            // string gets no reply body, as an unknown operation does.
            "ping" => match request_body.reader(endianness).read_string() {
                Ok(text) => {
                    let mut w = Writer::new(endianness);
                    w.write_string(&text);
                    w.finish()
                }
                Err(_) => Vec::new(),
            },
            _ => Vec::new(),
        }
    }
}

/// Build a Root POA with an [`EchoServant`] activated under `object_key`
/// (USER id-assignment so a client can address it by a known key).
#[must_use]
pub fn echo_poa(object_key: &[u8]) -> Arc<Poa> {
    let mgr = Arc::new(PoaManager::new());
    mgr.activate().expect("poa manager activate");
    let poa = Poa::new(PoaConfig {
        adapter_name: "RootPOA".to_string(),
        policies: PolicySet {
            id_assignment: IdAssignmentPolicy::User,
            ..PolicySet::default()
        },
        manager: mgr,
    })
    .expect("poa new");
    poa.activate_object_with_id(object_key.into(), Box::new(EchoServant))
        .expect("activate echo servant");
    Arc::new(poa)
}

/// A `SystemExceptionReplyBody` (GIOP §15.4.3.3) for `name`, minor 0,
/// `COMPLETED_NO`, in the byte order `endianness`.
fn system_exception_body(name: &str, endianness: Endianness) -> Vec<u8> {
    let mut w = Writer::new(endianness);
    w.write_string(&format!("IDL:omg.org/CORBA/{name}:1.0"));
    w.write_u32(0); // minor
    w.write_u32(1); // COMPLETED_NO
    w.finish()
}

/// The CORBA standard exception name a [`PoaError`] corresponds to —
/// every variant [`Poa::dispatch`] can actually return from this
/// crate's single-policy Echo POA gets its own name rather than a
/// catch-all, so a future `PoaError` variant fails to compile here
/// instead of silently picking a generic one.
const fn poa_error_exception_name(err: &PoaError) -> &'static str {
    match err {
        PoaError::ObjectNotActive | PoaError::NoServant | PoaError::ServantNotActive => {
            "OBJECT_NOT_EXIST"
        }
        PoaError::Marshal => "MARSHAL",
        PoaError::InterfaceRepositoryUnavailable => "INTF_REPOS",
        PoaError::AdapterInactive => "OBJ_ADAPTER",
        PoaError::ObjAdapter(_) => "OBJ_ADAPTER",
        PoaError::BadInvocationOrder(_) => "BAD_INV_ORDER",
        PoaError::WrongPolicy(_) => "NO_IMPLEMENT",
        PoaError::AdapterAlreadyExists(_)
        | PoaError::AdapterNonExistent(_)
        | PoaError::InvalidPolicy(_)
        | PoaError::ObjectAlreadyActive
        | PoaError::ServantAlreadyActive => "OBJ_ADAPTER",
    }
}

/// The missing **Acceptor↔POA glue**: start an IIOP server that decodes each
/// incoming GIOP request, dispatches it through `poa` to the servant, and
/// writes the GIOP reply back on the same connection. A `LocateRequest`
/// (GIOP §15.4.5 — a `corbaloc:` client, or any ORB that probes an object's
/// location before its first real invocation) is answered `OBJECT_HERE`/
/// `UNKNOWN_OBJECT` via [`Poa::locate`] instead of being dropped.
///
/// The servant gets the request body at its offset in the message and
/// the message's byte order (CORBA 3.3 Part 2 §9.4.2.1); the reply goes
/// out in the version and byte order of the request, its GIOP 1.0/1.1
/// body on a multiple of eight (§9.4.3).
pub fn serve(bind: SocketAddr, poa: Arc<Poa>) -> Result<Acceptor, IiopError> {
    Acceptor::start(AcceptorConfig::new(bind), move |mut conn| {
        while let Ok((msg, endianness, version)) = conn.read_message_full() {
            match msg {
                Message::Request(req) => {
                    // CORBA 3.3 Part 2 §9.4.2.1: the key of any addressing
                    // disposition; a target without a readable key is asked
                    // for KeyAddr (§9.4.3.2).
                    let Some(key) = runtime::target_object_key(&req.target) else {
                        if req.response_flags.response_expected() {
                            let reply = runtime::needs_key_addr_reply(req.request_id, endianness);
                            let _ = conn.write_message(version, endianness, false, &reply);
                        }
                        continue;
                    };
                    let oid: ObjectId = key.as_slice().into();
                    // §9.4.2.1: CDR aligns the arguments from the message
                    // start; the body is read at its offset.
                    let dispatched = req
                        .body_offset(version, endianness)
                        .map_err(|_| PoaError::Marshal)
                        .and_then(|offset| {
                            poa.dispatch(
                                &oid,
                                &req.operation,
                                &AlignedBody::new(&req.body, offset),
                                endianness,
                            )
                        });
                    let (reply_status, body) = match dispatched {
                        Ok(body) => (ReplyStatusType::NoException, body),
                        Err(err) => (
                            ReplyStatusType::SystemException,
                            system_exception_body(poa_error_exception_name(&err), endianness),
                        ),
                    };
                    // oneway: no reply.
                    if !req.response_flags.response_expected() {
                        continue;
                    }
                    let mut reply = Reply {
                        request_id: req.request_id,
                        reply_status,
                        service_context: ServiceContextList::default(),
                        body,
                    };
                    // §9.4.3: the body, written from offset 0, on a
                    // multiple of eight in GIOP 1.0/1.1 too. A reply that
                    // does not encode is not sent: the connection ends,
                    // as in `runtime::CorbaServer`.
                    if reply.place_body_at_eight(version, endianness).is_err() {
                        break;
                    }
                    let _ = conn.write_message(version, endianness, false, &Message::Reply(reply));
                }
                Message::LocateRequest(LocateRequest { request_id, target }) => {
                    let reply = match runtime::target_object_key(&target) {
                        Some(key) => Message::LocateReply(LocateReply {
                            request_id,
                            locate_status: poa.locate(&key.as_slice().into()),
                            body: Vec::new(),
                        }),
                        None => Message::LocateReply(LocateReply {
                            request_id,
                            locate_status: LocateStatusType::LocNeedsAddressingMode,
                            body: runtime::key_addr_body(endianness),
                        }),
                    };
                    let _ = conn.write_message(version, endianness, false, &reply);
                }
                _ => {}
            }
        }
    })
}

/// Send one GIOP request on an existing connection and return the reply body.
pub fn invoke_on(
    conn: &mut Connection,
    request_id: u32,
    object_key: &[u8],
    operation: &str,
    body: Vec<u8>,
) -> Result<Vec<u8>, IiopError> {
    let request = Message::Request(Request {
        request_id,
        response_flags: ResponseFlags::SYNC_WITH_TARGET,
        target: TargetAddress::Key(object_key.to_vec()),
        operation: operation.to_string(),
        requesting_principal: None,
        service_context: ServiceContextList::default(),
        body,
    });
    conn.write_message(Version::V1_2, WIRE, false, &request)?;
    match conn.read_message()? {
        Message::Reply(r) => Ok(r.body),
        _ => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zerodds_corba_iiop::{Connector, ConnectorConfig};

    #[test]
    fn echo_roundtrip_through_poa() {
        let key = b"Echo";
        let poa = echo_poa(key);
        let acceptor = serve("127.0.0.1:0".parse().unwrap(), poa).unwrap();
        let addr = acceptor.listen_addr();

        let connector = Connector::new(ConnectorConfig::default());
        let mut pooled = connector
            .connect(&addr.ip().to_string(), addr.port())
            .unwrap();
        let conn = pooled.connection().unwrap();

        for i in 0..100u32 {
            let reply = invoke_on(conn, i, key, "ping", encode_string_body("hello")).unwrap();
            assert_eq!(decode_string_body(&reply).unwrap(), "hello");
        }
        acceptor.shutdown();
    }

    fn call(
        conn: &mut Connection,
        request_id: u32,
        object_key: &[u8],
        operation: &str,
        body: Vec<u8>,
    ) -> Reply {
        let request = Message::Request(Request {
            request_id,
            response_flags: ResponseFlags::SYNC_WITH_TARGET,
            target: TargetAddress::Key(object_key.to_vec()),
            operation: operation.to_string(),
            requesting_principal: None,
            service_context: ServiceContextList::default(),
            body,
        });
        conn.write_message(Version::V1_2, WIRE, false, &request)
            .unwrap();
        match conn.read_message().unwrap() {
            Message::Reply(r) => r,
            other => panic!("expected a Reply, got {:?}", other.message_type()),
        }
    }

    /// CORBA 3.3 Part 2 §15.4.2: `_is_a`/`_non_existent`/`_repository_id`
    /// answer against the real `EchoServant`, dispatched through the
    /// same `Poa::dispatch` call `serve` makes for every other
    /// operation — this is the bug `zerodds-naming-svc`'s omniORB
    /// `nameclt` test found, reproduced against this crate's own
    /// Acceptor↔POA server.
    #[test]
    fn corba_object_operations_over_the_real_server() {
        let key = b"Echo";
        let poa = echo_poa(key);
        let acceptor = serve("127.0.0.1:0".parse().unwrap(), poa).unwrap();
        let addr = acceptor.listen_addr();
        let connector = Connector::new(ConnectorConfig::default());
        let mut pooled = connector
            .connect(&addr.ip().to_string(), addr.port())
            .unwrap();
        let conn = pooled.connection().unwrap();

        let is_a_body = |id: &str| {
            let mut w = Writer::new(WIRE);
            w.write_string(id);
            w.finish()
        };
        let decode_bool = |b: &[u8]| Reader::new(b, WIRE).read_bool().unwrap();

        let reply = call(conn, 1, key, "_is_a", is_a_body("IDL:demo/Echo:1.0"));
        assert_eq!(reply.reply_status, ReplyStatusType::NoException);
        assert!(decode_bool(&reply.body));

        let reply = call(
            conn,
            2,
            key,
            "_is_a",
            is_a_body("IDL:omg.org/CORBA/Object:1.0"),
        );
        assert_eq!(reply.reply_status, ReplyStatusType::NoException);
        assert!(decode_bool(&reply.body));

        let reply = call(conn, 3, key, "_is_a", is_a_body("IDL:demo/Nope:1.0"));
        assert_eq!(reply.reply_status, ReplyStatusType::NoException);
        assert!(!decode_bool(&reply.body));

        let reply = call(conn, 4, key, "_non_existent", Vec::new());
        assert_eq!(reply.reply_status, ReplyStatusType::NoException);
        assert!(!decode_bool(&reply.body));

        let reply = call(conn, 5, b"no-such-key", "_non_existent", Vec::new());
        assert_eq!(reply.reply_status, ReplyStatusType::NoException);
        assert!(decode_bool(&reply.body));

        let reply = call(conn, 6, key, "_repository_id", Vec::new());
        assert_eq!(reply.reply_status, ReplyStatusType::NoException);
        assert_eq!(
            decode_string_body(&reply.body).unwrap(),
            "IDL:demo/Echo:1.0"
        );

        let reply = call(conn, 7, b"no-such-key", "ping", Vec::new());
        assert_eq!(reply.reply_status, ReplyStatusType::SystemException);
        assert!(String::from_utf8_lossy(&reply.body).contains("OBJECT_NOT_EXIST"));

        acceptor.shutdown();
    }

    /// A `LocateRequest` (GIOP §15.4.5) used to be silently dropped —
    /// no reply at all, not even a wrong one.
    #[test]
    fn locate_request_answers_object_here_and_unknown_object() {
        let key = b"Echo";
        let poa = echo_poa(key);
        let acceptor = serve("127.0.0.1:0".parse().unwrap(), poa).unwrap();
        let addr = acceptor.listen_addr();
        let connector = Connector::new(ConnectorConfig::default());
        let mut pooled = connector
            .connect(&addr.ip().to_string(), addr.port())
            .unwrap();
        let conn = pooled.connection().unwrap();

        let locate = |conn: &mut Connection, request_id: u32, object_key: &[u8]| {
            let msg = Message::LocateRequest(LocateRequest {
                request_id,
                target: TargetAddress::Key(object_key.to_vec()),
            });
            conn.write_message(Version::V1_2, WIRE, false, &msg)
                .unwrap();
            match conn.read_message().unwrap() {
                Message::LocateReply(r) => r.locate_status,
                other => panic!("expected a LocateReply, got {:?}", other.message_type()),
            }
        };

        assert_eq!(
            locate(conn, 1, key),
            zerodds_corba_giop::LocateStatusType::ObjectHere
        );
        assert_eq!(
            locate(conn, 2, b"no-such-key"),
            zerodds_corba_giop::LocateStatusType::UnknownObject
        );
        acceptor.shutdown();
    }
}
