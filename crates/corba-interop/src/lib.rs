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

/// Codegen runtime: `IiopCorbaConnection` (client) + `CorbaServer` (server),
/// which connect the generated stubs/skeletons to the IIOP transport.
pub mod runtime;

use std::net::SocketAddr;
use std::sync::Arc;

use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
use zerodds_corba_giop::{
    Message, Reply, ReplyStatusType, Request, ResponseFlags, ServiceContextList, TargetAddress,
    Version,
};
use zerodds_corba_iiop::{Acceptor, AcceptorConfig, Connection, IiopError};
use zerodds_corba_poa::{
    IdAssignmentPolicy, ObjectId, Poa, PoaConfig, PoaManager, PolicySet, Servant,
};

/// Wire endianness used by the milestone-1 self-roundtrip (both ends ZeroDDS).
pub const WIRE: Endianness = Endianness::Big;

/// CDR-encode a single `string` value as a GIOP request/reply body.
#[must_use]
pub fn encode_string_body(s: &str) -> Vec<u8> {
    let mut w = BufferWriter::new(WIRE);
    w.write_string(s).expect("cdr write string");
    w.into_bytes()
}

/// CDR-decode a single `string` value from a GIOP body.
#[must_use]
pub fn decode_string_body(body: &[u8]) -> String {
    BufferReader::new(body, WIRE)
        .read_string()
        .unwrap_or_default()
}

/// `Echo` servant — `string ping(in string msg)` echoes its argument.
#[derive(Debug, Default)]
pub struct EchoServant;

impl Servant for EchoServant {
    fn primary_interface(&self) -> String {
        "IDL:demo/Echo:1.0".to_string()
    }

    fn invoke(&self, operation: &str, request_body: &[u8]) -> Vec<u8> {
        match operation {
            "ping" => encode_string_body(&decode_string_body(request_body)),
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

/// The missing **Acceptor↔POA glue**: start an IIOP server that decodes each
/// incoming GIOP request, dispatches it through `poa` to the servant, and
/// writes the GIOP reply back on the same connection.
pub fn serve(bind: SocketAddr, poa: Arc<Poa>) -> Result<Acceptor, IiopError> {
    Acceptor::start(AcceptorConfig::new(bind), move |mut conn| {
        while let Ok(msg) = conn.read_message() {
            let Message::Request(req) = msg else {
                continue;
            };
            let oid: ObjectId = match &req.target {
                TargetAddress::Key(k) => k.as_slice().into(),
                _ => continue,
            };
            let body = poa
                .dispatch(&oid, &req.operation, &req.body)
                .unwrap_or_default();
            let reply = Message::Reply(Reply {
                request_id: req.request_id,
                reply_status: ReplyStatusType::NoException,
                service_context: ServiceContextList::default(),
                body,
            });
            let _ = conn.write_message(Version::V1_2, WIRE, false, &reply);
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
            assert_eq!(decode_string_body(&reply), "hello");
        }
        acceptor.shutdown();
    }
}
