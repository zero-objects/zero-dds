// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 Part 1 §16 Portable Interceptors.
//!
//! - Request interceptors (§16.4): [`ClientRequestInterceptor`] and
//!   [`ServerRequestInterceptor`] with the ten interception points; every
//!   point may raise a system exception or `ForwardRequest`
//!   ([`InterceptorError`]). [`InterceptorRegistry`] runs them with the
//!   flow-stack rules of §16.4.2, §16.4.7 and §16.4.10: in order on the
//!   starting points, in reverse order on the ending points, an ending
//!   point only for interceptors whose starting point completed, and the
//!   change of ending point an exception or a `ForwardRequest` causes.
//! - [`RequestInfo`] (§16.4.12 to §16.4.14) with the validity of Tables
//!   16.1 and 16.2 (`BAD_INV_ORDER` minor 14 outside them), the service
//!   context rules (`BAD_PARAM` minor 26, `BAD_INV_ORDER` minor 15) and
//!   the request-scoped slots.
//! - `PortableInterceptor::Current` (§16.5): [`PiCurrent`] with
//!   thread-scoped slots and the copies between thread and request scope.
//! - IOR interceptors (§16.6): [`IorInterceptor`] with [`IorInfo`].
//! - `PolicyFactory` (§16.8), `ORBInitializer` and `ORBInitInfo` (§16.9):
//!   [`OrbInitializer`], [`OrbInitInfo`], [`register_orb_initializer`].
//!
//! The request path that calls the registry is the ORB's transport:
//! `zerodds-corba-interop`'s `IiopCorbaConnection` (client) and
//! `CorbaServer` (server). This binding passes operation arguments as
//! encoded CDR, so `arguments`, `exceptions`, `contexts`,
//! `operation_context` and `result` raise `NO_RESOURCES` minor 1, as
//! §16.4.12.3 to §16.4.12.7 prescribe for environments without access to
//! them.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use zerodds_corba_rt::{
    CompletionStatus, CorbaAny, ObjectReference, SystemException, SystemExceptionKind,
};
use zerodds_wire::Endianness;

// ---------------------------------------------------------------------------
// Interception points, reply status, exceptions
// ---------------------------------------------------------------------------

/// The ten request interception points (§16.4.6, §16.4.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InterceptionPoint {
    /// Client starting point `send_request`.
    SendRequest,
    /// Client starting point `send_poll`.
    SendPoll,
    /// Client ending point `receive_reply`.
    ReceiveReply,
    /// Client ending point `receive_exception`.
    ReceiveException,
    /// Client ending point `receive_other`.
    ReceiveOther,
    /// Server starting point `receive_request_service_contexts`.
    ReceiveRequestServiceContexts,
    /// Server intermediate point `receive_request`.
    ReceiveRequest,
    /// Server ending point `send_reply`.
    SendReply,
    /// Server ending point `send_exception`.
    SendException,
    /// Server ending point `send_other`.
    SendOther,
}

/// `PortableInterceptor::ReplyStatus` (§16.4.12.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReplyStatus {
    /// `SUCCESSFUL` (0).
    Successful,
    /// `SYSTEM_EXCEPTION` (1).
    SystemException,
    /// `USER_EXCEPTION` (2).
    UserException,
    /// `LOCATION_FORWARD` (3).
    LocationForward,
    /// `TRANSPORT_RETRY` (4).
    TransportRetry,
    /// `UNKNOWN` (5).
    Unknown,
}

impl ReplyStatus {
    /// The IDL constant value.
    #[must_use]
    pub const fn to_i16(self) -> i16 {
        match self {
            Self::Successful => 0,
            Self::SystemException => 1,
            Self::UserException => 2,
            Self::LocationForward => 3,
            Self::TransportRetry => 4,
            Self::Unknown => 5,
        }
    }
}

/// What an interception point raises (§16.4): a system exception, or
/// `PortableInterceptor::ForwardRequest { Object forward; }` (§16.4.15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterceptorError {
    /// A system exception.
    System(SystemException),
    /// `ForwardRequest`: the request goes to this object instead.
    ForwardRequest(ObjectReference),
}

impl From<SystemException> for InterceptorError {
    fn from(e: SystemException) -> Self {
        Self::System(e)
    }
}

/// The exception of a request: what the client received or the server
/// sends (`received_exception`, `sending_exception`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestException {
    /// A system exception.
    System(SystemException),
    /// A user exception: its repository id and the complete reply body
    /// (repository id and members, CDR in `endianness`) at its offset in
    /// the reply message (CORBA 3.3 Part 2 §9.4.3).
    User {
        /// Repository id.
        repository_id: String,
        /// The exception body.
        body: zerodds_corba_giop::AlignedBody,
        /// Byte order of `body`.
        endianness: Endianness,
    },
}

impl RequestException {
    /// The repository id (`received_exception_id`).
    #[must_use]
    pub fn repository_id(&self) -> &str {
        match self {
            Self::System(e) => e.repository_id(),
            Self::User { repository_id, .. } => repository_id,
        }
    }
}

/// `PortableInterceptor::InvalidSlot` (§16.5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidSlot;

/// What `PortableInterceptor::Current::get_slot` and `set_slot` raise
/// (§16.5.3, §16.9.2.14).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurrentError {
    /// `InvalidSlot`: the slot id was not allocated.
    InvalidSlot,
    /// A system exception: `BAD_INV_ORDER` minor 14 while the ORB
    /// initializers run.
    System(SystemException),
}

impl From<InvalidSlot> for CurrentError {
    fn from(_: InvalidSlot) -> Self {
        Self::InvalidSlot
    }
}

fn bad_inv_order(code: u32) -> SystemException {
    SystemException::omg(SystemExceptionKind::BadInvOrder, code, CompletionStatus::No)
}

fn no_resources_1() -> SystemException {
    SystemException::omg(SystemExceptionKind::NoResources, 1, CompletionStatus::No)
}

// ---------------------------------------------------------------------------
// RequestInfo
// ---------------------------------------------------------------------------

/// `ClientRequestInfo` and `ServerRequestInfo` (§16.4.12 to §16.4.14):
/// one type for both sides; every accessor checks the interception point
/// against Table 16.1 or 16.2.
#[derive(Debug, Clone)]
pub struct RequestInfo {
    client: bool,
    point: Option<InterceptionPoint>,
    request_id: u32,
    operation: String,
    response_expected: bool,
    sync_scope: i16,
    reply_status: Option<ReplyStatus>,
    forward_reference: Option<ObjectReference>,
    request_sc: BTreeMap<u32, Vec<u8>>,
    reply_sc: BTreeMap<u32, Vec<u8>>,
    target: ObjectReference,
    effective_target: ObjectReference,
    exception: Option<RequestException>,
    object_id: Vec<u8>,
    orb_id: String,
    server_id: String,
    slots: Vec<CorbaAny>,
}

use InterceptionPoint as P;

const CLIENT_ALL: &[P] = &[
    P::SendRequest,
    P::SendPoll,
    P::ReceiveReply,
    P::ReceiveException,
    P::ReceiveOther,
];
const SERVER_ALL: &[P] = &[
    P::ReceiveRequestServiceContexts,
    P::ReceiveRequest,
    P::SendReply,
    P::SendException,
    P::SendOther,
];
/// Where `exceptions` and `contexts` are valid (Tables 16.1 and 16.2):
/// every point but `send_poll` and `receive_request_service_contexts`.
const EXCEPTIONS_AND_CONTEXTS: &[P] = &[
    P::SendRequest,
    P::ReceiveReply,
    P::ReceiveException,
    P::ReceiveOther,
    P::ReceiveRequest,
    P::SendReply,
    P::SendException,
    P::SendOther,
];

impl RequestInfo {
    /// The information of a client request: `request_id`, `operation`,
    /// `response_expected`, the target and the request scope slots (the
    /// copy of the calling thread's slots, §16.5.4).
    #[must_use]
    pub fn client(
        request_id: u32,
        operation: impl Into<String>,
        response_expected: bool,
        target: ObjectReference,
    ) -> Self {
        Self::new(
            true,
            request_id,
            operation.into(),
            response_expected,
            target,
        )
    }

    /// The information of a request a server received: the object key
    /// is the `object_id`.
    #[must_use]
    pub fn server(
        request_id: u32,
        operation: impl Into<String>,
        response_expected: bool,
        object_id: Vec<u8>,
    ) -> Self {
        let mut info = Self::new(
            false,
            request_id,
            operation.into(),
            response_expected,
            ObjectReference::default(),
        );
        info.object_id = object_id;
        info
    }

    fn new(
        client: bool,
        request_id: u32,
        operation: String,
        response_expected: bool,
        target: ObjectReference,
    ) -> Self {
        Self {
            client,
            point: None,
            request_id,
            operation,
            response_expected,
            // Messaging::SYNC_WITH_TARGET for a two-way, SYNC_NONE for a
            // oneway (GIOP response_flags 3 and 0).
            sync_scope: if response_expected { 3 } else { 0 },
            reply_status: None,
            forward_reference: None,
            request_sc: BTreeMap::new(),
            reply_sc: BTreeMap::new(),
            effective_target: target.clone(),
            target,
            exception: None,
            object_id: Vec::new(),
            orb_id: String::new(),
            server_id: String::new(),
            slots: Vec::new(),
        }
    }

    /// Sets `sync_scope` (Messaging::SyncScope, the GIOP
    /// `response_flags`).
    pub fn set_sync_scope(&mut self, scope: i16) {
        self.sync_scope = scope;
    }

    /// Sets the ORB id and server id a server reports.
    pub fn set_server_identity(&mut self, orb_id: &str, server_id: &str) {
        self.orb_id = String::from(orb_id);
        self.server_id = String::from(server_id);
    }

    /// The interception point being run, `None` outside an interceptor.
    #[must_use]
    pub fn point(&self) -> Option<InterceptionPoint> {
        self.point
    }

    fn valid(&self, allowed: &[InterceptionPoint]) -> Result<(), SystemException> {
        match self.point {
            Some(p) if allowed.contains(&p) => Ok(()),
            // Outside an interceptor the transport reads the result.
            None => Ok(()),
            Some(_) => Err(bad_inv_order(14)),
        }
    }

    /// §16.4.12.1 `request_id`.
    #[must_use]
    pub fn request_id(&self) -> u32 {
        self.request_id
    }

    /// §16.4.12.2 `operation`.
    #[must_use]
    pub fn operation(&self) -> &str {
        &self.operation
    }

    /// §16.4.12.3 to §16.4.12.7 — `arguments`, `exceptions`,
    /// `contexts`, `operation_context`, `result`: not available in this
    /// binding, which passes encoded CDR.
    ///
    /// # Errors
    /// `NO_RESOURCES` minor 1 where Table 16.1/16.2 admit the attribute,
    /// else `BAD_INV_ORDER` minor 14.
    pub fn arguments(&self) -> Result<(), SystemException> {
        self.valid(&[
            P::SendRequest,
            P::ReceiveReply,
            P::ReceiveRequest,
            P::SendReply,
        ])?;
        Err(no_resources_1())
    }

    /// §16.4.12.4 `exceptions` — see [`arguments`](Self::arguments).
    ///
    /// # Errors
    /// `NO_RESOURCES` minor 1 at every point but `send_poll` and
    /// `receive_request_service_contexts`, there `BAD_INV_ORDER` minor 14.
    pub fn exceptions(&self) -> Result<(), SystemException> {
        self.valid(EXCEPTIONS_AND_CONTEXTS)?;
        Err(no_resources_1())
    }

    /// §16.4.12.5 `contexts` — see [`arguments`](Self::arguments).
    ///
    /// # Errors
    /// As [`exceptions`](Self::exceptions).
    pub fn contexts(&self) -> Result<(), SystemException> {
        self.valid(EXCEPTIONS_AND_CONTEXTS)?;
        Err(no_resources_1())
    }

    /// §16.4.12.6 `operation_context` — see [`arguments`](Self::arguments).
    ///
    /// # Errors
    /// `NO_RESOURCES` minor 1 at `send_request`, `receive_reply`,
    /// `receive_exception`, `receive_other`, `receive_request` and
    /// `send_reply`, else `BAD_INV_ORDER` minor 14.
    pub fn operation_context(&self) -> Result<(), SystemException> {
        self.valid(&[
            P::SendRequest,
            P::ReceiveReply,
            P::ReceiveException,
            P::ReceiveOther,
            P::ReceiveRequest,
            P::SendReply,
        ])?;
        Err(no_resources_1())
    }

    /// §16.4.12.7 `result` — see [`arguments`](Self::arguments).
    ///
    /// # Errors
    /// `NO_RESOURCES` minor 1 at `receive_reply`/`send_reply`, else
    /// `BAD_INV_ORDER` minor 14.
    pub fn result(&self) -> Result<(), SystemException> {
        self.valid(&[P::ReceiveReply, P::SendReply])?;
        Err(no_resources_1())
    }

    /// §16.4.12.8 `response_expected`.
    #[must_use]
    pub fn response_expected(&self) -> bool {
        self.response_expected
    }

    /// §16.4.12.9 `sync_scope`.
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 14 at `send_poll`.
    pub fn sync_scope(&self) -> Result<i16, SystemException> {
        if self.point == Some(P::SendPoll) {
            return Err(bad_inv_order(14));
        }
        Ok(self.sync_scope)
    }

    /// §16.4.12.10 `reply_status`.
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 14 at a starting or intermediate point.
    pub fn reply_status(&self) -> Result<ReplyStatus, SystemException> {
        self.valid(&[
            P::ReceiveReply,
            P::ReceiveException,
            P::ReceiveOther,
            P::SendReply,
            P::SendException,
            P::SendOther,
        ])?;
        self.reply_status.ok_or_else(|| bad_inv_order(14))
    }

    /// §16.4.12.11 `forward_reference`.
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 14 outside `receive_other`/`send_other` and
    /// when `reply_status` is not `LOCATION_FORWARD`.
    pub fn forward_reference(&self) -> Result<&ObjectReference, SystemException> {
        self.valid(&[P::ReceiveOther, P::SendOther])?;
        match (&self.reply_status, &self.forward_reference) {
            (Some(ReplyStatus::LocationForward), Some(f)) => Ok(f),
            _ => Err(bad_inv_order(14)),
        }
    }

    /// §16.4.12.12 `get_slot`: the request scope slot; an unset slot is
    /// an `any` of `tk_null`.
    ///
    /// # Errors
    /// [`InvalidSlot`] for a slot id the ORB did not allocate.
    pub fn get_slot(&self, id: u32) -> Result<CorbaAny, InvalidSlot> {
        self.slots.get(id as usize).cloned().ok_or(InvalidSlot)
    }

    /// §16.4.14 `set_slot` (server side).
    ///
    /// # Errors
    /// [`InvalidSlot`] for a slot id the ORB did not allocate, also on
    /// the client side, where `ClientRequestInfo` has no `set_slot`.
    pub fn set_slot(&mut self, id: u32, data: CorbaAny) -> Result<(), InvalidSlot> {
        if self.client {
            return Err(InvalidSlot);
        }
        let slot = self.slots.get_mut(id as usize).ok_or(InvalidSlot)?;
        *slot = data;
        Ok(())
    }

    /// The request scope slots.
    #[must_use]
    pub fn slots(&self) -> &[CorbaAny] {
        &self.slots
    }

    /// Replaces the request scope slots (the transport copies the thread
    /// scope in or out, §16.5.4).
    pub fn set_slots(&mut self, slots: Vec<CorbaAny>) {
        self.slots = slots;
    }

    /// §16.4.12.13 `get_request_service_context`.
    ///
    /// # Errors
    /// `BAD_PARAM` minor 26 when the request has no context with `id`;
    /// `BAD_INV_ORDER` minor 14 at `send_poll`.
    pub fn get_request_service_context(&self, id: u32) -> Result<&[u8], SystemException> {
        if self.point == Some(P::SendPoll) {
            return Err(bad_inv_order(14));
        }
        self.request_sc.get(&id).map(Vec::as_slice).ok_or_else(|| {
            SystemException::omg(SystemExceptionKind::BadParam, 26, CompletionStatus::No)
        })
    }

    /// §16.4.12.14 `get_reply_service_context`.
    ///
    /// # Errors
    /// `BAD_PARAM` minor 26 when the reply has no context with `id`;
    /// `BAD_INV_ORDER` minor 14 before the reply exists.
    pub fn get_reply_service_context(&self, id: u32) -> Result<&[u8], SystemException> {
        self.valid(&[
            P::ReceiveReply,
            P::ReceiveException,
            P::ReceiveOther,
            P::SendReply,
            P::SendException,
            P::SendOther,
        ])?;
        self.reply_sc.get(&id).map(Vec::as_slice).ok_or_else(|| {
            SystemException::omg(SystemExceptionKind::BadParam, 26, CompletionStatus::No)
        })
    }

    /// §16.4.13.9 `add_request_service_context` (client, `send_request`).
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 15 when a context with that id exists and
    /// `replace` is false; minor 14 outside `send_request`.
    pub fn add_request_service_context(
        &mut self,
        id: u32,
        data: Vec<u8>,
        replace: bool,
    ) -> Result<(), SystemException> {
        if !self.client {
            return Err(bad_inv_order(14));
        }
        self.valid(&[P::SendRequest])?;
        add_context(&mut self.request_sc, id, data, replace)
    }

    /// §16.4.14 `add_reply_service_context` (server, every point).
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 15 when a context with that id exists and
    /// `replace` is false; minor 14 on the client side.
    pub fn add_reply_service_context(
        &mut self,
        id: u32,
        data: Vec<u8>,
        replace: bool,
    ) -> Result<(), SystemException> {
        if self.client {
            return Err(bad_inv_order(14));
        }
        self.valid(SERVER_ALL)?;
        add_context(&mut self.reply_sc, id, data, replace)
    }

    /// The request's service contexts, by id (the transport writes them
    /// into the GIOP request, or reads them from it).
    #[must_use]
    pub fn request_service_contexts(&self) -> &BTreeMap<u32, Vec<u8>> {
        &self.request_sc
    }

    /// The reply's service contexts, by id.
    #[must_use]
    pub fn reply_service_contexts(&self) -> &BTreeMap<u32, Vec<u8>> {
        &self.reply_sc
    }

    /// The transport sets a context that arrived with the request.
    pub fn set_received_request_context(&mut self, id: u32, data: Vec<u8>) {
        self.request_sc.insert(id, data);
    }

    /// The transport sets a context that arrived with the reply.
    pub fn set_received_reply_context(&mut self, id: u32, data: Vec<u8>) {
        self.reply_sc.insert(id, data);
    }

    /// §16.4.13.1 `target`.
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 14 on the server side.
    pub fn target(&self) -> Result<&ObjectReference, SystemException> {
        if !self.client {
            return Err(bad_inv_order(14));
        }
        self.valid(CLIENT_ALL)?;
        Ok(&self.target)
    }

    /// §16.4.13.2 `effective_target`: the target after forwards.
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 14 on the server side.
    pub fn effective_target(&self) -> Result<&ObjectReference, SystemException> {
        if !self.client {
            return Err(bad_inv_order(14));
        }
        Ok(&self.effective_target)
    }

    /// The transport records the object a forward led to.
    pub fn set_effective_target(&mut self, target: ObjectReference) {
        self.effective_target = target;
    }

    /// §16.4.13.4/§16.4.13.5 `received_exception`,
    /// `received_exception_id` (client, `receive_exception`).
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 14 elsewhere.
    pub fn received_exception(&self) -> Result<&RequestException, SystemException> {
        if !self.client {
            return Err(bad_inv_order(14));
        }
        self.valid(&[P::ReceiveException])?;
        self.exception.as_ref().ok_or_else(|| bad_inv_order(14))
    }

    /// §16.4.14 `sending_exception` (server, `send_exception`).
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 14 elsewhere.
    pub fn sending_exception(&self) -> Result<&RequestException, SystemException> {
        if self.client {
            return Err(bad_inv_order(14));
        }
        self.valid(&[P::SendException])?;
        self.exception.as_ref().ok_or_else(|| bad_inv_order(14))
    }

    /// §16.4.14 `object_id` (server): the object key of the request.
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 14 at `receive_request_service_contexts`
    /// and on the client side.
    pub fn object_id(&self) -> Result<&[u8], SystemException> {
        if self.client {
            return Err(bad_inv_order(14));
        }
        self.valid(&[
            P::ReceiveRequest,
            P::SendReply,
            P::SendException,
            P::SendOther,
        ])?;
        Ok(&self.object_id)
    }

    /// §16.4.14 `orb_id` (server).
    ///
    /// # Errors
    /// As [`object_id`](Self::object_id).
    pub fn orb_id(&self) -> Result<&str, SystemException> {
        self.object_id()?;
        Ok(&self.orb_id)
    }

    /// §16.4.14 `server_id` (server).
    ///
    /// # Errors
    /// As [`object_id`](Self::object_id).
    pub fn server_id(&self) -> Result<&str, SystemException> {
        self.object_id()?;
        Ok(&self.server_id)
    }
}

fn add_context(
    map: &mut BTreeMap<u32, Vec<u8>>,
    id: u32,
    data: Vec<u8>,
    replace: bool,
) -> Result<(), SystemException> {
    if !replace && map.contains_key(&id) {
        return Err(bad_inv_order(15));
    }
    map.insert(id, data);
    Ok(())
}

// ---------------------------------------------------------------------------
// Interceptor traits
// ---------------------------------------------------------------------------

/// Result of an interception point.
pub type PointResult = Result<(), InterceptorError>;

/// `ClientRequestInterceptor` (§16.4.5). The defaults do nothing.
pub trait ClientRequestInterceptor: Send + Sync {
    /// `Interceptor::name` (§16.3); the empty string for an anonymous
    /// interceptor.
    fn name(&self) -> &str;
    /// `Interceptor::destroy`, called by `ORB::destroy`.
    fn destroy(&self) {}
    /// `send_request` (§16.4.6.1).
    fn send_request(&self, _info: &mut RequestInfo) -> PointResult {
        Ok(())
    }
    /// `send_poll` (§16.4.6.2).
    fn send_poll(&self, _info: &mut RequestInfo) -> PointResult {
        Ok(())
    }
    /// `receive_reply` (§16.4.6.3).
    fn receive_reply(&self, _info: &mut RequestInfo) -> PointResult {
        Ok(())
    }
    /// `receive_exception` (§16.4.6.4).
    fn receive_exception(&self, _info: &mut RequestInfo) -> PointResult {
        Ok(())
    }
    /// `receive_other` (§16.4.6.5).
    fn receive_other(&self, _info: &mut RequestInfo) -> PointResult {
        Ok(())
    }
}

/// `ServerRequestInterceptor` (§16.4.8). The defaults do nothing.
pub trait ServerRequestInterceptor: Send + Sync {
    /// `Interceptor::name`.
    fn name(&self) -> &str;
    /// `Interceptor::destroy`.
    fn destroy(&self) {}
    /// `receive_request_service_contexts` (§16.4.9.1).
    fn receive_request_service_contexts(&self, _info: &mut RequestInfo) -> PointResult {
        Ok(())
    }
    /// `receive_request` (§16.4.9.2).
    fn receive_request(&self, _info: &mut RequestInfo) -> PointResult {
        Ok(())
    }
    /// `send_reply` (§16.4.9.3).
    fn send_reply(&self, _info: &mut RequestInfo) -> PointResult {
        Ok(())
    }
    /// `send_exception` (§16.4.9.4).
    fn send_exception(&self, _info: &mut RequestInfo) -> PointResult {
        Ok(())
    }
    /// `send_other` (§16.4.9.5).
    fn send_other(&self, _info: &mut RequestInfo) -> PointResult {
        Ok(())
    }
}

/// `IORInfo` (§16.6.5): the components an IOR interceptor adds while an
/// object reference is made.
#[derive(Debug, Clone)]
pub struct IorInfo {
    profile_ids: Vec<u32>,
    known_policy_types: BTreeSet<u32>,
    policies: BTreeMap<u32, Policy>,
    components: Vec<(Option<u32>, u32, Vec<u8>)>,
}

impl IorInfo {
    /// The `IORInfo` of an object adapter: the ids of the profiles its
    /// references carry, the policy types the ORB knows (§16.6.5.1) and
    /// the adapter's effective policies, by type.
    #[must_use]
    pub fn new(
        profile_ids: Vec<u32>,
        known_policy_types: BTreeSet<u32>,
        policies: BTreeMap<u32, Policy>,
    ) -> Self {
        Self {
            profile_ids,
            known_policy_types,
            policies,
            components: Vec::new(),
        }
    }

    /// §16.6.5.1 `get_effective_policy`: the effective policy of the
    /// type, `None` (the nil reference) for a known type without one.
    ///
    /// # Errors
    /// `INV_POLICY` minor 3 for a policy type the ORB does not know.
    pub fn get_effective_policy(
        &self,
        policy_type: u32,
    ) -> Result<Option<&Policy>, SystemException> {
        if let Some(p) = self.policies.get(&policy_type) {
            return Ok(Some(p));
        }
        if self.known_policy_types.contains(&policy_type) {
            Ok(None)
        } else {
            Err(SystemException::omg(
                SystemExceptionKind::InvPolicy,
                3,
                CompletionStatus::No,
            ))
        }
    }

    /// §16.6.5.2 `add_ior_component`: a component for every profile.
    pub fn add_ior_component(&mut self, tag: u32, component_data: Vec<u8>) {
        self.components.push((None, tag, component_data));
    }

    /// §16.6.5.3 `add_ior_component_to_profile`: a component for the
    /// profiles with `profile_id`.
    ///
    /// # Errors
    /// `BAD_PARAM` minor 29 for a profile id the adapter's references do
    /// not carry.
    pub fn add_ior_component_to_profile(
        &mut self,
        profile_id: u32,
        tag: u32,
        data: Vec<u8>,
    ) -> Result<(), SystemException> {
        if !self.profile_ids.contains(&profile_id) {
            return Err(SystemException::omg(
                SystemExceptionKind::BadParam,
                29,
                CompletionStatus::No,
            ));
        }
        self.components.push((Some(profile_id), tag, data));
        Ok(())
    }

    /// The components for a profile with `profile_id`, in the order
    /// added: those for every profile and those for this one.
    #[must_use]
    pub fn components_for_profile(&self, profile_id: u32) -> Vec<(u32, Vec<u8>)> {
        self.components
            .iter()
            .filter(|(p, _, _)| p.is_none_or(|p| p == profile_id))
            .map(|(_, tag, data)| (*tag, data.clone()))
            .collect()
    }
}

/// `IORInterceptor` (§16.6.4).
pub trait IorInterceptor: Send + Sync {
    /// `Interceptor::name`.
    fn name(&self) -> &str;
    /// `Interceptor::destroy`.
    fn destroy(&self) {}
    /// `establish_components`: add components through `info`. An
    /// exception raised here is ignored by the ORB (§16.6.4.1), so the
    /// operation returns nothing.
    fn establish_components(&self, info: &mut IorInfo);
}

/// `CORBA::PolicyError` with its `PolicyErrorCode` (§8.8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PolicyError {
    /// `BAD_POLICY` 0, `UNSUPPORTED_POLICY` 1, `BAD_POLICY_TYPE` 2,
    /// `BAD_POLICY_VALUE` 3, `UNSUPPORTED_POLICY_VALUE` 4.
    pub reason: i16,
}

impl PolicyError {
    /// `BAD_POLICY` (0).
    pub const BAD_POLICY: i16 = 0;
    /// `UNSUPPORTED_POLICY` (1).
    pub const UNSUPPORTED_POLICY: i16 = 1;
    /// `BAD_POLICY_TYPE` (2).
    pub const BAD_POLICY_TYPE: i16 = 2;
    /// `BAD_POLICY_VALUE` (3).
    pub const BAD_POLICY_VALUE: i16 = 3;
    /// `UNSUPPORTED_POLICY_VALUE` (4).
    pub const UNSUPPORTED_POLICY_VALUE: i16 = 4;
}

/// A `CORBA::Policy` a [`PolicyFactory`] created: its type and value.
#[derive(Debug, Clone, PartialEq)]
pub struct Policy {
    /// `policy_type`.
    pub policy_type: u32,
    /// The value it was created from.
    pub value: CorbaAny,
}

/// `PolicyFactory` (§16.8.1).
pub trait PolicyFactory: Send + Sync {
    /// `create_policy(type, value)`.
    ///
    /// # Errors
    /// [`PolicyError`] for a value the factory refuses.
    fn create_policy(&self, policy_type: u32, value: &CorbaAny) -> Result<Policy, PolicyError>;
}

// ---------------------------------------------------------------------------
// Registry with the flow rules
// ---------------------------------------------------------------------------

/// The outcome of a request after the interceptors ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// A normal reply (`SUCCESSFUL`); on a starting point: send the
    /// request / call the servant.
    Proceed,
    /// The request ends with this exception.
    Exception(RequestException),
    /// The request goes to this object (`LOCATION_FORWARD`).
    Forward(ObjectReference),
    /// `receive_other`/`send_other` with `SUCCESSFUL`: a oneway completed.
    Other,
    /// `receive_other` with `TRANSPORT_RETRY` (§16.4.12.10): the
    /// transport resends the request, for instance after a
    /// `NEEDS_ADDRESSING_MODE` reply (Part 2 §9.4.3.2).
    Retry,
}

/// The ORB's ordered lists of interceptors and its policy factories.
#[derive(Default)]
pub struct InterceptorRegistry {
    client: Vec<Arc<dyn ClientRequestInterceptor>>,
    server: Vec<Arc<dyn ServerRequestInterceptor>>,
    ior: Vec<Arc<dyn IorInterceptor>>,
    policy_factories: BTreeMap<u32, Arc<dyn PolicyFactory>>,
}

impl core::fmt::Debug for InterceptorRegistry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("InterceptorRegistry")
            .field("client_count", &self.client.len())
            .field("server_count", &self.server.len())
            .field("ior_count", &self.ior.len())
            .finish()
    }
}

/// `PortableInterceptor::ORBInitInfo::DuplicateName` (§16.9.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateName {
    /// The name already registered.
    pub name: String,
}

fn check_name<T: ?Sized>(
    list: &[Arc<T>],
    name: &str,
    name_of: impl Fn(&T) -> &str,
) -> Result<(), DuplicateName> {
    if !name.is_empty() && list.iter().any(|i| name_of(i) == name) {
        return Err(DuplicateName {
            name: String::from(name),
        });
    }
    Ok(())
}

impl InterceptorRegistry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// §16.9.2 `add_client_request_interceptor`.
    ///
    /// # Errors
    /// [`DuplicateName`] when a client interceptor with the same
    /// non-empty name is registered.
    pub fn add_client(
        &mut self,
        c: Arc<dyn ClientRequestInterceptor>,
    ) -> Result<(), DuplicateName> {
        check_name(&self.client, c.name(), |i| i.name())?;
        self.client.push(c);
        Ok(())
    }

    /// §16.9.2 `add_server_request_interceptor`.
    ///
    /// # Errors
    /// [`DuplicateName`] as [`add_client`](Self::add_client).
    pub fn add_server(
        &mut self,
        s: Arc<dyn ServerRequestInterceptor>,
    ) -> Result<(), DuplicateName> {
        check_name(&self.server, s.name(), |i| i.name())?;
        self.server.push(s);
        Ok(())
    }

    /// §16.9.2 `add_ior_interceptor`.
    ///
    /// # Errors
    /// [`DuplicateName`] as [`add_client`](Self::add_client).
    pub fn add_ior(&mut self, i: Arc<dyn IorInterceptor>) -> Result<(), DuplicateName> {
        check_name(&self.ior, i.name(), |x| x.name())?;
        self.ior.push(i);
        Ok(())
    }

    /// §16.9.2 `register_policy_factory`.
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 16 when a factory for `policy_type` exists.
    pub fn register_policy_factory(
        &mut self,
        policy_type: u32,
        factory: Arc<dyn PolicyFactory>,
    ) -> Result<(), SystemException> {
        if self.policy_factories.contains_key(&policy_type) {
            return Err(bad_inv_order(16));
        }
        self.policy_factories.insert(policy_type, factory);
        Ok(())
    }

    /// `ORB::create_policy` over the registered factories (§8.8.2).
    ///
    /// # Errors
    /// `PolicyError` `BAD_POLICY_TYPE` when no factory knows the type,
    /// else the factory's error.
    pub fn create_policy(&self, policy_type: u32, value: &CorbaAny) -> Result<Policy, PolicyError> {
        let factory = self.policy_factories.get(&policy_type).ok_or(PolicyError {
            reason: PolicyError::BAD_POLICY_TYPE,
        })?;
        factory.create_policy(policy_type, value)
    }

    /// The policy types a factory is registered for: the types the ORB
    /// knows (`IORInfo::get_effective_policy`, §16.6.5.1).
    #[must_use]
    pub fn policy_types(&self) -> BTreeSet<u32> {
        self.policy_factories.keys().copied().collect()
    }

    /// Number of client interceptors.
    #[must_use]
    pub fn client_count(&self) -> usize {
        self.client.len()
    }

    /// Number of server interceptors.
    #[must_use]
    pub fn server_count(&self) -> usize {
        self.server.len()
    }

    /// Number of IOR interceptors.
    #[must_use]
    pub fn ior_count(&self) -> usize {
        self.ior.len()
    }

    /// `true` when no request interceptor is registered.
    #[must_use]
    pub fn has_request_interceptors(&self) -> bool {
        !self.client.is_empty() || !self.server.is_empty()
    }

    /// `Interceptor::destroy` on every interceptor (`ORB::destroy`).
    pub fn destroy_all(&self) {
        for i in &self.client {
            i.destroy();
        }
        for i in &self.server {
            i.destroy();
        }
        for i in &self.ior {
            i.destroy();
        }
    }

    /// §16.6.4 — runs `establish_components` of every IOR interceptor.
    pub fn establish_components(&self, info: &mut IorInfo) {
        for i in &self.ior {
            i.establish_components(info);
        }
    }

    /// Client starting point `send_request` (or `send_poll` with
    /// `poll`), in registration order. Returns the depth of the flow
    /// stack for [`client_end`](Self::client_end); when an interceptor
    /// raises, the interceptors already on the stack get their ending
    /// point and the outcome is returned as `Err` (`Exception` or
    /// `Forward`).
    ///
    /// # Errors
    /// The outcome after the unwinding, as described.
    pub fn client_start(&self, info: &mut RequestInfo, poll: bool) -> Result<usize, Outcome> {
        let point = if poll { P::SendPoll } else { P::SendRequest };
        for (depth, ic) in self.client.iter().enumerate() {
            info.point = Some(point);
            let r = if poll {
                ic.send_poll(info)
            } else {
                ic.send_request(info)
            };
            if let Err(e) = r {
                let state = match e {
                    InterceptorError::System(e) => End::Exception(RequestException::System(e)),
                    // `send_poll` does not raise ForwardRequest (§16.4.5).
                    InterceptorError::ForwardRequest(_) if poll => {
                        End::Exception(RequestException::System(bad_inv_order(14)))
                    }
                    InterceptorError::ForwardRequest(f) => End::Forward(f),
                };
                return Err(self.client_unwind(info, depth, state));
            }
        }
        info.point = None;
        Ok(self.client.len())
    }

    /// Client ending points for the `depth` interceptors on the flow
    /// stack, in reverse order, starting from what the request produced:
    /// a reply (`Proceed`), an exception, a forward, or `Other` for a
    /// oneway.
    pub fn client_end(&self, info: &mut RequestInfo, depth: usize, received: Outcome) -> Outcome {
        let state = match received {
            Outcome::Proceed => End::Reply,
            Outcome::Exception(e) => End::Exception(e),
            Outcome::Forward(f) => End::Forward(f),
            Outcome::Other => End::Other,
            Outcome::Retry => End::Retry,
        };
        self.client_unwind(info, depth, state)
    }

    fn client_unwind(&self, info: &mut RequestInfo, depth: usize, mut state: End) -> Outcome {
        for ic in self.client[..depth].iter().rev() {
            state.prepare(info, true);
            let r = match &state {
                End::Reply => ic.receive_reply(info),
                End::Exception(_) => ic.receive_exception(info),
                End::Forward(_) | End::Other | End::Retry => ic.receive_other(info),
            };
            state = state.after(r);
        }
        info.point = None;
        state.prepare_final(info);
        state.into_outcome()
    }

    /// Server starting point `receive_request_service_contexts`, in
    /// registration order. `Ok(depth)`: the transport copies the request
    /// scope slots into the thread scope (§16.5.4.5 step 5), then runs
    /// [`server_receive_request`](Self::server_receive_request).
    ///
    /// # Errors
    /// When an interceptor raises, the interceptors on the flow stack get
    /// their ending point; the outcome to reply with is returned.
    pub fn server_start(&self, info: &mut RequestInfo) -> Result<usize, Outcome> {
        for (depth, ic) in self.server.iter().enumerate() {
            info.point = Some(P::ReceiveRequestServiceContexts);
            if let Err(e) = ic.receive_request_service_contexts(info) {
                return Err(self.server_unwind(info, depth, End::from_error(e)));
            }
        }
        info.point = None;
        Ok(self.server.len())
    }

    /// Server intermediate point `receive_request`, in registration
    /// order, for the `depth` interceptors [`server_start`](Self::server_start)
    /// put on the flow stack. `Ok(())`: call the servant, then
    /// [`server_end`](Self::server_end).
    ///
    /// # Errors
    /// When an interceptor raises, the interceptors on the flow stack get
    /// their ending point; the outcome to reply with is returned.
    pub fn server_receive_request(
        &self,
        info: &mut RequestInfo,
        depth: usize,
    ) -> Result<(), Outcome> {
        for ic in self.server.iter().take(depth) {
            info.point = Some(P::ReceiveRequest);
            if let Err(e) = ic.receive_request(info) {
                return Err(self.server_unwind(info, depth, End::from_error(e)));
            }
        }
        info.point = None;
        Ok(())
    }

    /// Server ending points in reverse order for what the servant
    /// produced; the outcome is what the reply carries.
    pub fn server_end(&self, info: &mut RequestInfo, depth: usize, produced: Outcome) -> Outcome {
        let state = match produced {
            Outcome::Proceed => End::Reply,
            Outcome::Exception(e) => End::Exception(e),
            Outcome::Forward(f) => End::Forward(f),
            Outcome::Other => End::Other,
            // TRANSPORT_RETRY is a client status (§16.4.12.10); a server
            // transport that hands it in has no reply to send.
            Outcome::Retry => End::Exception(RequestException::System(SystemException::new(
                SystemExceptionKind::Internal,
                0,
                CompletionStatus::No,
            ))),
        };
        self.server_unwind(info, depth, state)
    }

    fn server_unwind(&self, info: &mut RequestInfo, depth: usize, mut state: End) -> Outcome {
        for ic in self.server[..depth].iter().rev() {
            state.prepare(info, false);
            let r = match &state {
                End::Reply => ic.send_reply(info),
                End::Exception(_) => ic.send_exception(info),
                End::Forward(_) | End::Other | End::Retry => ic.send_other(info),
            };
            state = state.after(r);
        }
        info.point = None;
        state.prepare_final(info);
        state.into_outcome()
    }
}

/// The ending point state while the flow stack unwinds.
enum End {
    Reply,
    Exception(RequestException),
    Forward(ObjectReference),
    Other,
    Retry,
}

impl End {
    fn from_error(e: InterceptorError) -> Self {
        match e {
            InterceptorError::System(e) => Self::Exception(RequestException::System(e)),
            InterceptorError::ForwardRequest(f) => Self::Forward(f),
        }
    }

    /// Sets the point and the reply information of `info` for the next
    /// ending point.
    fn prepare(&self, info: &mut RequestInfo, client: bool) {
        info.point = Some(match (self, client) {
            (Self::Reply, true) => P::ReceiveReply,
            (Self::Reply, false) => P::SendReply,
            (Self::Exception(_), true) => P::ReceiveException,
            (Self::Exception(_), false) => P::SendException,
            (Self::Forward(_) | Self::Other | Self::Retry, true) => P::ReceiveOther,
            (Self::Forward(_) | Self::Other | Self::Retry, false) => P::SendOther,
        });
        self.prepare_final(info);
    }

    fn prepare_final(&self, info: &mut RequestInfo) {
        match self {
            Self::Reply | Self::Other => {
                info.reply_status = Some(ReplyStatus::Successful);
                info.exception = None;
                info.forward_reference = None;
            }
            Self::Exception(e) => {
                info.reply_status = Some(match e {
                    RequestException::System(_) => ReplyStatus::SystemException,
                    RequestException::User { .. } => ReplyStatus::UserException,
                });
                info.exception = Some(e.clone());
                info.forward_reference = None;
            }
            Self::Forward(f) => {
                info.reply_status = Some(ReplyStatus::LocationForward);
                info.forward_reference = Some(f.clone());
                info.exception = None;
            }
            Self::Retry => {
                info.reply_status = Some(ReplyStatus::TransportRetry);
                info.exception = None;
                info.forward_reference = None;
            }
        }
    }

    /// The state after one ending point returned `r` (§16.4.6.3 to
    /// §16.4.6.5, §16.4.9.3 to §16.4.9.5).
    fn after(self, r: PointResult) -> Self {
        match r {
            Ok(()) => self,
            Err(InterceptorError::System(e)) => Self::Exception(RequestException::System(e)),
            // `receive_reply` and `send_reply` do not raise ForwardRequest
            // (§16.4.5, §16.4.8; §16.4.1 item 3: "other than a successful
            // reply"). An interceptor that does is an invalid call.
            Err(InterceptorError::ForwardRequest(_)) if matches!(self, Self::Reply) => {
                Self::Exception(RequestException::System(SystemException::omg(
                    SystemExceptionKind::BadInvOrder,
                    14,
                    CompletionStatus::Yes,
                )))
            }
            Err(InterceptorError::ForwardRequest(f)) => Self::Forward(f),
        }
    }

    fn into_outcome(self) -> Outcome {
        match self {
            Self::Reply => Outcome::Proceed,
            Self::Exception(e) => Outcome::Exception(e),
            Self::Forward(f) => Outcome::Forward(f),
            Self::Other => Outcome::Other,
            Self::Retry => Outcome::Retry,
        }
    }
}

// ---------------------------------------------------------------------------
// PortableInterceptor::Current
// ---------------------------------------------------------------------------

std::thread_local! {
    /// The thread scope slots of every `PiCurrent` on this thread, by
    /// its id.
    static THREAD_SLOTS: RefCell<BTreeMap<usize, Vec<CorbaAny>>> = const { RefCell::new(BTreeMap::new()) };
}

/// `PortableInterceptor::Current` (§16.5): slots allocated by the ORB
/// initializers, a thread scope (TSC) per thread and the request scope
/// (RSC) of [`RequestInfo`].
#[derive(Debug)]
pub struct PiCurrent {
    id: usize,
    slot_count: Mutex<u32>,
    /// `true` while the ORB initializers run (§16.9.2.14).
    initializing: AtomicBool,
}

/// Source of the [`PiCurrent`] ids that key the thread scope slots.
static NEXT_CURRENT_ID: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(1);

impl Default for PiCurrent {
    fn default() -> Self {
        Self::new()
    }
}

impl PiCurrent {
    /// No slots.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: NEXT_CURRENT_ID.fetch_add(1, Ordering::Relaxed),
            slot_count: Mutex::new(0),
            initializing: AtomicBool::new(false),
        }
    }

    /// Marks the time the ORB initializers run, in which the slots
    /// cannot be read or written (§16.9.2.14).
    pub(crate) fn set_initializing(&self, on: bool) {
        self.initializing.store(on, Ordering::SeqCst);
    }

    fn check_slot(&self, id: u32) -> Result<(), CurrentError> {
        if self.initializing.load(Ordering::SeqCst) {
            return Err(CurrentError::System(bad_inv_order(14)));
        }
        if id >= self.slot_count() {
            return Err(CurrentError::InvalidSlot);
        }
        Ok(())
    }

    /// §16.9.2 `allocate_slot_id`.
    pub fn allocate_slot_id(&self) -> u32 {
        let mut g = self
            .slot_count
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let id = *g;
        *g += 1;
        id
    }

    /// The number of allocated slots.
    #[must_use]
    pub fn slot_count(&self) -> u32 {
        *self
            .slot_count
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn key(&self) -> usize {
        self.id
    }

    fn with_tsc<R>(&self, f: impl FnOnce(&mut Vec<CorbaAny>) -> R) -> R {
        let n = self.slot_count() as usize;
        THREAD_SLOTS.with(|t| {
            let mut t = t.borrow_mut();
            let slots = t.entry(self.key()).or_default();
            if slots.len() < n {
                slots.resize(n, CorbaAny::default());
            }
            f(slots)
        })
    }

    /// §16.5.3 `get_slot` (thread scope); an unset slot is `tk_null`.
    ///
    /// # Errors
    /// [`CurrentError::InvalidSlot`] for an id that was not allocated;
    /// `BAD_INV_ORDER` minor 14 while the ORB initializers run
    /// (§16.9.2.14 and Annex A.6; §16.5.3 names minor 10, which Annex A.6
    /// gives to a DII request sent twice).
    pub fn get_slot(&self, id: u32) -> Result<CorbaAny, CurrentError> {
        self.check_slot(id)?;
        Ok(self.with_tsc(|s| s[id as usize].clone()))
    }

    /// §16.5.3 `set_slot` (thread scope).
    ///
    /// # Errors
    /// As [`get_slot`](Self::get_slot).
    pub fn set_slot(&self, id: u32, data: CorbaAny) -> Result<(), CurrentError> {
        self.check_slot(id)?;
        self.with_tsc(|s| s[id as usize] = data);
        Ok(())
    }

    /// §16.5.4 — a copy of the calling thread's slots: the request scope
    /// of a client request starts as it.
    #[must_use]
    pub fn thread_slots(&self) -> Vec<CorbaAny> {
        self.with_tsc(|s| s.clone())
    }

    /// §16.5.4 — replaces the thread's slots (the server copies the
    /// request scope into the thread that runs the servant, and the
    /// thread scope back after it).
    pub fn set_thread_slots(&self, slots: Vec<CorbaAny>) {
        let n = self.slot_count() as usize;
        self.with_tsc(|s| {
            *s = slots;
            s.resize(n, CorbaAny::default());
        });
    }

    /// Empty request scope slots, one per allocated slot (server side:
    /// the RSC starts empty, §16.5.4).
    #[must_use]
    pub fn empty_slots(&self) -> Vec<CorbaAny> {
        alloc::vec![CorbaAny::default(); self.slot_count() as usize]
    }
}

// ---------------------------------------------------------------------------
// ORBInitializer / ORBInitInfo
// ---------------------------------------------------------------------------

/// `ORBInitInfo` (§16.9.2): what an `ORBInitializer` sees while the ORB
/// is being initialised.
pub struct OrbInitInfo<'a> {
    pub(crate) arguments: &'a [String],
    pub(crate) orb: &'a crate::orb_core::Orb,
    pub(crate) registry: &'a mut InterceptorRegistry,
    pub(crate) in_post_init: bool,
}

/// What the `ORBInitInfo::add_*_interceptor` operations raise.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationError {
    /// `DuplicateName` (§16.9.2.1).
    DuplicateName(DuplicateName),
    /// `BAD_INV_ORDER` minor 26 in `post_init` (§16.9.1.2).
    System(SystemException),
}

impl From<DuplicateName> for RegistrationError {
    fn from(e: DuplicateName) -> Self {
        Self::DuplicateName(e)
    }
}

/// What an `ORBInitializer` operation raises (§16.9.1): any of them
/// abandons the initialization of the ORB.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitializerError {
    /// A system exception.
    System(SystemException),
    /// `ORBInitInfo::DuplicateName`.
    DuplicateName(DuplicateName),
    /// `ORBInitInfo::InvalidName`.
    InvalidName,
}

impl From<SystemException> for InitializerError {
    fn from(e: SystemException) -> Self {
        Self::System(e)
    }
}

impl From<DuplicateName> for InitializerError {
    fn from(e: DuplicateName) -> Self {
        Self::DuplicateName(e)
    }
}

impl From<RegistrationError> for InitializerError {
    fn from(e: RegistrationError) -> Self {
        match e {
            RegistrationError::DuplicateName(d) => Self::DuplicateName(d),
            RegistrationError::System(s) => Self::System(s),
        }
    }
}

impl From<crate::orb_core::ResolveError> for InitializerError {
    fn from(e: crate::orb_core::ResolveError) -> Self {
        match e {
            crate::orb_core::ResolveError::InvalidName => Self::InvalidName,
            crate::orb_core::ResolveError::System(s) => Self::System(s),
        }
    }
}

impl OrbInitInfo<'_> {
    /// `arguments` (§16.9.2.3): the `ORB_init` argument list as passed.
    #[must_use]
    pub fn arguments(&self) -> &[String] {
        self.arguments
    }

    /// `orb_id` (§16.9.2.4).
    #[must_use]
    pub fn orb_id(&self) -> &str {
        self.orb.id()
    }

    /// `codec_factory` (§16.9.2.5): the `IOP::CodecFactory` (Part 2
    /// §7.8), available in `pre_init` and `post_init`, before the ORB's
    /// initial references resolve.
    #[must_use]
    pub fn codec_factory(&self) -> crate::iop_codec::CodecFactory {
        crate::iop_codec::CodecFactory
    }

    /// `register_initial_reference` (§16.9.2.6): the semantics of
    /// `ORB::register_initial_reference`; the id must not be registered
    /// yet, and the ORB's own services (`PICurrent`, `DynAnyFactory`)
    /// count as registered.
    ///
    /// # Errors
    /// `InvalidName` for an empty or taken id, `BAD_PARAM` minor 27 for
    /// a nil object.
    pub fn register_initial_reference(
        &mut self,
        id: &str,
        obj: crate::orb_core::ObjectRef,
    ) -> Result<(), crate::orb_core::ResolveError> {
        self.orb.register_initial_reference(id, obj)
    }

    /// `resolve_initial_references` (§16.9.2.7): valid in `post_init`;
    /// in `pre_init` it raises `BAD_INV_ORDER` minor 14.
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 14 in `pre_init`, else as
    /// `ORB::resolve_initial_references`.
    pub fn resolve_initial_references(
        &self,
        id: &str,
    ) -> Result<crate::orb_core::ObjectRef, crate::orb_core::ResolveError> {
        if !self.in_post_init {
            return Err(crate::orb_core::ResolveError::System(bad_inv_order(14)));
        }
        self.orb.resolve_initial_references(id)
    }

    /// `BAD_INV_ORDER` minor 26 in `post_init` (§16.9.1.2).
    fn check_pre_init(&self) -> Result<(), SystemException> {
        if self.in_post_init {
            Err(bad_inv_order(26))
        } else {
            Ok(())
        }
    }

    /// `add_client_request_interceptor` (§16.9.2.8).
    ///
    /// # Errors
    /// [`RegistrationError::DuplicateName`]; `BAD_INV_ORDER` minor 26 in
    /// `post_init`.
    pub fn add_client_request_interceptor(
        &mut self,
        i: Arc<dyn ClientRequestInterceptor>,
    ) -> Result<(), RegistrationError> {
        self.check_pre_init().map_err(RegistrationError::System)?;
        Ok(self.registry.add_client(i)?)
    }

    /// `add_server_request_interceptor` (§16.9.2.9).
    ///
    /// # Errors
    /// As [`add_client_request_interceptor`](Self::add_client_request_interceptor).
    pub fn add_server_request_interceptor(
        &mut self,
        i: Arc<dyn ServerRequestInterceptor>,
    ) -> Result<(), RegistrationError> {
        self.check_pre_init().map_err(RegistrationError::System)?;
        Ok(self.registry.add_server(i)?)
    }

    /// `add_ior_interceptor` (§16.9.2.10).
    ///
    /// # Errors
    /// As [`add_client_request_interceptor`](Self::add_client_request_interceptor).
    pub fn add_ior_interceptor(
        &mut self,
        i: Arc<dyn IorInterceptor>,
    ) -> Result<(), RegistrationError> {
        self.check_pre_init().map_err(RegistrationError::System)?;
        Ok(self.registry.add_ior(i)?)
    }

    /// `allocate_slot_id` (§16.9.2.14).
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 26 in `post_init`.
    pub fn allocate_slot_id(&mut self) -> Result<u32, SystemException> {
        self.check_pre_init()?;
        Ok(self.orb.pi_current().allocate_slot_id())
    }

    /// `register_policy_factory` (§16.9.2.15).
    ///
    /// # Errors
    /// `BAD_INV_ORDER` minor 16 for a type that has a factory.
    pub fn register_policy_factory(
        &mut self,
        policy_type: u32,
        factory: Arc<dyn PolicyFactory>,
    ) -> Result<(), SystemException> {
        self.registry.register_policy_factory(policy_type, factory)
    }
}

/// `ORBInitializer` (§16.9.1). An error from either operation abandons
/// the initialization: `ORB_init` destroys the ORB and raises the system
/// exception, or `INITIALIZE` for a user exception.
pub trait OrbInitializer: Send + Sync {
    /// `pre_init`: register interceptors, allocate slots, register
    /// policy factories and initial references.
    ///
    /// # Errors
    /// What the initializer raises.
    fn pre_init(&self, info: &mut OrbInitInfo<'_>) -> Result<(), InitializerError>;
    /// `post_init`: initial references resolve; interceptors and slots
    /// can no longer be added.
    ///
    /// # Errors
    /// What the initializer raises.
    fn post_init(&self, _info: &mut OrbInitInfo<'_>) -> Result<(), InitializerError> {
        Ok(())
    }
}

/// The initializers `register_orb_initializer` collected; every
/// `ORB_init` that creates an ORB runs them (§16.9.3).
static INITIALIZERS: Mutex<Vec<Arc<dyn OrbInitializer>>> = Mutex::new(Vec::new());

/// §16.9.3 `PortableInterceptor::register_orb_initializer`.
pub fn register_orb_initializer(init: Arc<dyn OrbInitializer>) {
    INITIALIZERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(init);
}

/// The registered initializers, in registration order.
pub(crate) fn registered_initializers() -> Vec<Arc<dyn OrbInitializer>> {
    INITIALIZERS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// A client interceptor that adds one service context to every request
/// at `send_request`: the way to send an OTS (id 0), CSIv2 (id 15) or
/// codeset context as a registered interceptor rather than from the
/// transport code.
pub struct ServiceContextInjector {
    name: String,
    context_id: u32,
    data: Vec<u8>,
}

impl ServiceContextInjector {
    /// Adds `(context_id, data)` to every request.
    pub fn new(name: impl Into<String>, context_id: u32, data: Vec<u8>) -> Self {
        Self {
            name: name.into(),
            context_id,
            data,
        }
    }
}

impl ClientRequestInterceptor for ServiceContextInjector {
    fn name(&self) -> &str {
        &self.name
    }
    fn send_request(&self, info: &mut RequestInfo) -> PointResult {
        info.add_request_service_context(self.context_id, self.data.clone(), true)?;
        Ok(())
    }
}
