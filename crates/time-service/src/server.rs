// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! An IIOP server for [`crate::servant::TimeServant`]: the Time Service
//! object reachable as a CORBA object (OMG Time Service 1.1 §1.2).
//! Feature `serve`.
//!
//! [`TimeServer::start`] listens on a TCP address, reads GIOP 1.0 to 1.3
//! messages on every accepted connection (one thread each, from
//! `zerodds-corba-iiop`'s `Acceptor`), answers a `LocateRequest` with
//! `OBJECT_HERE` or `UNKNOWN_OBJECT`, dispatches a `Request` and writes
//! the reply. The service object is at
//! `corbaloc::<host>:<port>/TimeService`; [`TimeServer::service_ior`]
//! gives its IOR.
//!
//! [`TimeServer::start`] gives its callers no current time:
//! `universal_time` and `absolute_time` of a UTO raise
//! `CORBA::NO_PERMISSION` ([`RemoteTimePolicy::Refuse`], App. B.2);
//! [`TimeServer::start_with_policy`] chooses the policy.

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use std::net::SocketAddr;
use std::sync::OnceLock;

use zerodds_corba_giop::{
    LocateReply, LocateStatusType, Message, Reply, ReplyStatusType, Request, ServiceContextList,
    TargetAddress, Version, encode_message,
};
use zerodds_corba_iiop::{Acceptor, AcceptorConfig, Connection, IiopError};
use zerodds_corba_ior::Ior;
use zerodds_wire::Endianness;

use crate::servant::{RemoteTimePolicy, ServantReply, TimeServant, system_exception_body};
use crate::service::TimeService;

/// A running Time Service server; dropping it stops accepting.
pub struct TimeServer {
    acceptor: Acceptor,
    servant: Arc<OnceLock<TimeServant>>,
}

impl core::fmt::Debug for TimeServer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TimeServer")
            .field("listen_addr", &self.acceptor.listen_addr())
            .finish_non_exhaustive()
    }
}

impl TimeServer {
    /// Serves `service` on `bind`. The IORs it returns name
    /// `advertised_host` and the port the listener got (so `bind` may use
    /// port 0). `universal_time` and `absolute_time` raise
    /// `CORBA::NO_PERMISSION` ([`RemoteTimePolicy::Refuse`]).
    ///
    /// # Errors
    /// The listener's error if `bind` cannot be bound.
    pub fn start(
        service: TimeService,
        bind: SocketAddr,
        advertised_host: String,
    ) -> Result<Self, IiopError> {
        Self::start_with_policy(service, bind, advertised_host, RemoteTimePolicy::default())
    }

    /// As [`Self::start`], with `policy` for `universal_time` and
    /// `absolute_time` (App. B.2).
    ///
    /// # Errors
    /// The listener's error if `bind` cannot be bound.
    pub fn start_with_policy(
        service: TimeService,
        bind: SocketAddr,
        advertised_host: String,
        policy: RemoteTimePolicy,
    ) -> Result<Self, IiopError> {
        let cell: Arc<OnceLock<TimeServant>> = Arc::new(OnceLock::new());
        let for_handler = Arc::clone(&cell);
        let acceptor = Acceptor::start(AcceptorConfig::new(bind), move |conn| {
            // The servant is set right after the listener is bound; a
            // connection accepted in between waits for it.
            serve_connection(for_handler.wait(), conn);
        })?;
        let port = acceptor.listen_addr().port();
        let _ = cell.set(TimeServant::new(service, advertised_host, port).with_remote_time(policy));
        Ok(Self {
            acceptor,
            servant: cell,
        })
    }

    /// The address the server listens on.
    #[must_use]
    pub fn listen_addr(&self) -> SocketAddr {
        self.acceptor.listen_addr()
    }

    /// The IOR of the service object.
    ///
    /// # Errors
    /// The text of the IOR encoder's error.
    pub fn service_ior(&self) -> Result<Ior, String> {
        self.servant.wait().service_ior()
    }

    /// The `corbaloc` URL of the service object.
    #[must_use]
    pub fn corbaloc(&self, host: &str) -> String {
        alloc::format!(
            "corbaloc::{host}:{}/TimeService",
            self.acceptor.listen_addr().port()
        )
    }

    /// Stops accepting connections; open connections end when their
    /// client closes them.
    pub fn shutdown(self) {
        self.acceptor.shutdown();
    }
}

/// The offset of the body of `request` in its GIOP message: 0 for GIOP
/// 1.2 and 1.3, which put the body on an 8-octet boundary; for 1.0 and
/// 1.1 the length of the message without the body (CDR aligns from the
/// message start, CORBA 3.3 Part 2 §15.4.1).
fn request_body_offset(version: Version, endianness: Endianness, request: &Request) -> usize {
    if version.uses_v1_2_request_layout() {
        return 0;
    }
    let mut header = request.clone();
    header.body = Vec::new();
    encode_message(version, endianness, false, &Message::Request(header))
        .map(|b| b.len())
        .unwrap_or(0)
}

/// The offset of the body of the reply this server writes in `version`:
/// as [`request_body_offset`], with the empty service context list the
/// replies carry.
fn reply_body_offset(version: Version, endianness: Endianness, request_id: u32) -> usize {
    if version.uses_v1_2_request_layout() {
        return 0;
    }
    let header = Message::Reply(Reply {
        request_id,
        reply_status: ReplyStatusType::NoException,
        service_context: ServiceContextList::default(),
        body: Vec::new(),
    });
    encode_message(version, endianness, false, &header)
        .map(|b| b.len())
        .unwrap_or(0)
}

/// Serves one connection until the client closes it or a message does
/// not decode.
fn serve_connection(servant: &TimeServant, mut conn: Connection) {
    loop {
        let Ok((msg, endianness, version)) = conn.read_message_full() else {
            return;
        };
        let reply_version = if version.is_at_least(1, 3) {
            Version::V1_2
        } else {
            version
        };
        let request = match msg {
            Message::LocateRequest(lr) => {
                let known = matches!(&lr.target, TargetAddress::Key(k) if servant.knows(k));
                let reply = Message::LocateReply(LocateReply {
                    request_id: lr.request_id,
                    locate_status: if known {
                        LocateStatusType::ObjectHere
                    } else {
                        LocateStatusType::UnknownObject
                    },
                    body: Vec::new(),
                });
                if conn
                    .write_message(reply_version, endianness, false, &reply)
                    .is_err()
                {
                    return;
                }
                continue;
            }
            Message::Request(request) => request,
            Message::CloseConnection(_) => return,
            // CancelRequest, a stray Reply, Fragment without a start:
            // nothing to answer.
            _ => continue,
        };
        let object_key = match &request.target {
            TargetAddress::Key(k) => k.clone(),
            // A profile or reference addressed target: no key this server
            // gave out, so the object does not exist here.
            _ => Vec::new(),
        };
        let request_offset = request_body_offset(version, endianness, &request);
        let reply_offset = reply_body_offset(reply_version, endianness, request.request_id);
        let dispatched = servant.dispatch_at(
            &object_key,
            &request.operation,
            &request.body,
            request_offset,
            reply_offset,
            endianness,
        );
        if !request.response_flags.response_expected() {
            continue;
        }
        let (reply_status, body) = match dispatched {
            ServantReply::Ok(b) => (ReplyStatusType::NoException, b),
            ServantReply::UserException { body, .. } => (ReplyStatusType::UserException, body),
            ServantReply::SystemException(name) => (
                ReplyStatusType::SystemException,
                system_exception_body(name, endianness),
            ),
        };
        let reply = Message::Reply(Reply {
            request_id: request.request_id,
            reply_status,
            service_context: ServiceContextList::default(),
            body,
        });
        if conn
            .write_message(reply_version, endianness, false, &reply)
            .is_err()
        {
            return;
        }
    }
}
