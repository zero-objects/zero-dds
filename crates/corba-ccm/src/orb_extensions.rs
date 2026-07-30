// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! CORBA 3.3 ORB extensions — stub layer for:
//! - §16 Portable Interceptors (Part 1)
//! - §17 CORBA Messaging (Part 1)
//! - §18 Compression (Part 1) + Part 2 §12 ZIOP
//! - Part 2 §11 MIOP (Multicast Inter-ORB Protocol)
//! - Part 2 §8 Inter-ORB Bridges
//! - Part 2 §9.8/§9.9 BiDirectional GIOP
//!
//! ZeroDDS has no full ORB; these modules provide a configuration
//! + data-model layer as a stub for migration tooling.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

// ===========================================================================
// §16 Portable Interceptors
// ===========================================================================

/// Spec §16 — interception point for the client side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientInterceptionPoint {
    /// `send_request` (Spec §16.4.2).
    SendRequest,
    /// `send_poll`.
    SendPoll,
    /// `receive_reply`.
    ReceiveReply,
    /// `receive_exception`.
    ReceiveException,
    /// `receive_other`.
    ReceiveOther,
}

/// Spec §16 — interception point for the server side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerInterceptionPoint {
    /// `receive_request_service_contexts` (Spec §16.4.3).
    ReceiveRequestServiceContexts,
    /// `receive_request`.
    ReceiveRequest,
    /// `send_reply`.
    SendReply,
    /// `send_exception`.
    SendException,
    /// `send_other`.
    SendOther,
}

/// Spec §16.4.1 — `RequestInfo`: the invocation context that interceptors read
/// and mutate at the points. In particular it carries the **service-context lists**
/// (request + reply), over which frameworks such as OTS (`TransactionService` id 0)
/// and CSIv2 (`SecurityAttributeService` id 15) propagate their context — cleanly per
/// spec instead of hard-wired into the transport.
#[derive(Debug, Clone, Default)]
pub struct RequestInfo {
    /// GIOP `request_id`.
    pub request_id: u32,
    /// Operation name.
    pub operation: String,
    /// Whether a reply is expected (non-oneway).
    pub response_expected: bool,
    request_sc: BTreeMap<u32, Vec<u8>>,
    reply_sc: BTreeMap<u32, Vec<u8>>,
    forward: Option<Vec<u8>>,
}

impl RequestInfo {
    /// New `RequestInfo` for `request_id` + `operation`.
    #[must_use]
    pub fn new(request_id: u32, operation: impl Into<alloc::string::String>) -> Self {
        Self {
            request_id,
            operation: operation.into(),
            response_expected: true,
            request_sc: BTreeMap::new(),
            reply_sc: BTreeMap::new(),
            forward: None,
        }
    }

    /// Spec §16.4.1 — `add_request_service_context`.
    pub fn add_request_service_context(&mut self, id: u32, data: Vec<u8>) {
        self.request_sc.insert(id, data);
    }
    /// Spec §16.4.1 — `get_request_service_context`.
    #[must_use]
    pub fn get_request_service_context(&self, id: u32) -> Option<&[u8]> {
        self.request_sc.get(&id).map(Vec::as_slice)
    }
    /// All request service contexts (id → encapsulated data).
    #[must_use]
    pub fn request_service_contexts(&self) -> &BTreeMap<u32, Vec<u8>> {
        &self.request_sc
    }
    /// Spec §16.4.1 — `add_reply_service_context`.
    pub fn add_reply_service_context(&mut self, id: u32, data: Vec<u8>) {
        self.reply_sc.insert(id, data);
    }
    /// Spec §16.4.1 — `get_reply_service_context`.
    #[must_use]
    pub fn get_reply_service_context(&self, id: u32) -> Option<&[u8]> {
        self.reply_sc.get(&id).map(Vec::as_slice)
    }
    /// All reply service contexts.
    #[must_use]
    pub fn reply_service_contexts(&self) -> &BTreeMap<u32, Vec<u8>> {
        &self.reply_sc
    }
    /// Spec §16.4.5 — set `forward_reference` (LOCATION_FORWARD): the IOR bytes
    /// the interceptor redirects the call to.
    pub fn set_forward_reference(&mut self, ior: Vec<u8>) {
        self.forward = Some(ior);
    }
    /// The forward reference that was set (if any).
    #[must_use]
    pub fn forward_reference(&self) -> Option<&[u8]> {
        self.forward.as_deref()
    }
}

/// Spec §16.4.2 — `ClientRequestInterceptor` trait. The named points take
/// a [`RequestInfo`] (full spec); `intercept` remains a lightweight
/// tracing hook for the connection path (default no-op).
pub trait ClientRequestInterceptor: Send + Sync {
    /// Spec-compliant interceptor name.
    fn name(&self) -> &str;
    /// Lightweight point hook (connection path, default no-op).
    fn intercept(&self, _point: ClientInterceptionPoint, _op: &str) {}
    /// Spec §16.4.2 `send_request` — before sending; add service contexts here.
    fn send_request(&self, _info: &mut RequestInfo) {}
    /// Spec §16.4.2 `receive_reply` — after a successful reply.
    fn receive_reply(&self, _info: &mut RequestInfo) {}
    /// Spec §16.4.2 `receive_exception` — on an exception reply.
    fn receive_exception(&self, _info: &mut RequestInfo) {}
    /// Spec §16.4.2 `receive_other` — on LOCATION_FORWARD etc.
    fn receive_other(&self, _info: &mut RequestInfo) {}
}

/// Spec §16.4.3 — `ServerRequestInterceptor` trait.
pub trait ServerRequestInterceptor: Send + Sync {
    /// Spec-compliant interceptor name.
    fn name(&self) -> &str;
    /// Lightweight point hook (default no-op).
    fn intercept(&self, _point: ServerInterceptionPoint, _op: &str) {}
    /// Spec §16.4.3 `receive_request_service_contexts` — evaluate request SCs
    /// (e.g. verify CSIv2 credentials, join an OTS transaction).
    fn receive_request_service_contexts(&self, _info: &mut RequestInfo) {}
    /// Spec §16.4.3 `receive_request` — after argument demarshalling.
    fn receive_request(&self, _info: &mut RequestInfo) {}
    /// Spec §16.4.3 `send_reply` — before sending the reply; add reply SCs.
    fn send_reply(&self, _info: &mut RequestInfo) {}
    /// Spec §16.4.3 `send_exception`.
    fn send_exception(&self, _info: &mut RequestInfo) {}
    /// Spec §16.4.3 `send_other`.
    fn send_other(&self, _info: &mut RequestInfo) {}
}

/// Spec §16.4.4 — `IORInterceptor` trait for object-reference creation.
pub trait IorInterceptor: Send + Sync {
    /// Interceptor name.
    fn name(&self) -> &str;
    /// Invoked on the `establish_components` call.
    /// Returns optional tagged components (see `corba-ior::tags`).
    fn establish_components(&self) -> Vec<u32>;
}

/// Spec §16.4.x — interceptor registry per ORB.
#[derive(Default)]
pub struct InterceptorRegistry {
    client: Vec<Arc<dyn ClientRequestInterceptor>>,
    server: Vec<Arc<dyn ServerRequestInterceptor>>,
    ior: Vec<Arc<dyn IorInterceptor>>,
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

impl InterceptorRegistry {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §16.4.x — `add_client_request_interceptor`.
    pub fn add_client(&mut self, c: Arc<dyn ClientRequestInterceptor>) {
        self.client.push(c);
    }

    /// Spec §16.4.x — `add_server_request_interceptor`.
    pub fn add_server(&mut self, s: Arc<dyn ServerRequestInterceptor>) {
        self.server.push(s);
    }

    /// Spec §16.4.x — `add_ior_interceptor`.
    pub fn add_ior(&mut self, i: Arc<dyn IorInterceptor>) {
        self.ior.push(i);
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

    /// Spec §16.4.2 — accessor for pipeline walks in the
    /// connection send/receive path.
    #[must_use]
    pub fn client_interceptors(&self) -> &[Arc<dyn ClientRequestInterceptor>] {
        &self.client
    }

    /// Spec §16.4.3 — accessor for pipeline walks in the
    /// acceptor/POA dispatch.
    #[must_use]
    pub fn server_interceptors(&self) -> &[Arc<dyn ServerRequestInterceptor>] {
        &self.server
    }

    /// Spec §16.4.4 — accessor for the IOR build path.
    #[must_use]
    pub fn ior_interceptors(&self) -> &[Arc<dyn IorInterceptor>] {
        &self.ior
    }

    /// Spec §16.4.2 — walk through all client interceptors at point
    /// `point` with the operation name `op`. Invoked from
    /// `corba-iiop::Connection::run_client_pipeline`.
    pub fn walk_client(&self, point: ClientInterceptionPoint, op: &str) {
        for ic in &self.client {
            ic.intercept(point, op);
        }
    }

    /// Spec §16.4.3 — walk through all server interceptors at point
    /// `point` with the operation name `op`.
    pub fn walk_server(&self, point: ServerInterceptionPoint, op: &str) {
        for ic in &self.server {
            ic.intercept(point, op);
        }
    }

    /// Spec §16.4.4 — walk through all IOR interceptors. Returns
    /// the accumulated TaggedComponent tags that feed into the
    /// IOR build.
    #[must_use]
    pub fn walk_ior(&self) -> Vec<u32> {
        let mut tags = Vec::new();
        for ic in &self.ior {
            tags.extend(ic.establish_components());
        }
        tags
    }

    /// Spec §16.4.2 — full-spec walk through all client interceptors at `point`
    /// with a [`RequestInfo`] (service-context manipulation, forward_reference).
    pub fn run_client(&self, point: ClientInterceptionPoint, info: &mut RequestInfo) {
        for ic in &self.client {
            match point {
                ClientInterceptionPoint::SendRequest => ic.send_request(info),
                ClientInterceptionPoint::ReceiveReply => ic.receive_reply(info),
                ClientInterceptionPoint::ReceiveException => ic.receive_exception(info),
                ClientInterceptionPoint::ReceiveOther => ic.receive_other(info),
                ClientInterceptionPoint::SendPoll => {}
            }
            ic.intercept(point, &info.operation);
        }
    }

    /// Spec §16.4.3 — full-spec walk through all server interceptors at `point`.
    pub fn run_server(&self, point: ServerInterceptionPoint, info: &mut RequestInfo) {
        for ic in &self.server {
            match point {
                ServerInterceptionPoint::ReceiveRequestServiceContexts => {
                    ic.receive_request_service_contexts(info);
                }
                ServerInterceptionPoint::ReceiveRequest => ic.receive_request(info),
                ServerInterceptionPoint::SendReply => ic.send_reply(info),
                ServerInterceptionPoint::SendException => ic.send_exception(info),
                ServerInterceptionPoint::SendOther => ic.send_other(info),
            }
            ic.intercept(point, &info.operation);
        }
    }
}

/// Generic client interceptor that injects a fixed
/// `ServiceContext` `(id, data)` into the request at `send_request`. This is the
/// spec-clean way to attach OTS (`TransactionService` id 0), CSIv2 (id 15) or
/// codeset contexts — as a registrable interceptor instead of hardcoded in the
/// transport code.
pub struct ServiceContextInjector {
    name: alloc::string::String,
    context_id: u32,
    data: Vec<u8>,
}

impl ServiceContextInjector {
    /// New injector: attaches `(context_id, data)` to every request.
    pub fn new(name: impl Into<alloc::string::String>, context_id: u32, data: Vec<u8>) -> Self {
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
    fn send_request(&self, info: &mut RequestInfo) {
        info.add_request_service_context(self.context_id, self.data.clone());
    }
}

/// Spec §16.5 — `PolicyFactory` trait for policy creation.
pub trait PolicyFactory: Send + Sync {
    /// Policy type (the spec uses `PolicyType` as a u32).
    fn policy_type(&self) -> u32;
    /// Creates a policy from an Any value (CDR-encoded).
    ///
    /// # Errors
    /// `()` on invalid encoding.
    #[allow(clippy::result_unit_err)]
    fn create_policy(&self, value: &[u8]) -> Result<Vec<u8>, ()>;
}

/// Spec §16.6 — `PICurrent` (per-invocation slot storage).
#[derive(Debug, Clone, Default)]
pub struct PiCurrent {
    slots: BTreeMap<u32, Vec<u8>>,
}

impl PiCurrent {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §16.6 — `set_slot(slot_id, value)`.
    pub fn set_slot(&mut self, slot_id: u32, value: Vec<u8>) {
        self.slots.insert(slot_id, value);
    }

    /// Spec §16.6 — `get_slot(slot_id)`.
    #[must_use]
    pub fn get_slot(&self, slot_id: u32) -> Option<&[u8]> {
        self.slots.get(&slot_id).map(Vec::as_slice)
    }
}

// ===========================================================================
// §17 CORBA Messaging (Part 1)
// ===========================================================================

/// Spec §17 — messaging policy type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessagingPolicy {
    /// `RebindPolicy` — rebinding behavior (spec §17.3).
    Rebind,
    /// `SyncScopePolicy` — synchronization scope (spec §17.4).
    SyncScope,
    /// `RequestPriorityPolicy` — request priority (spec §17.5).
    RequestPriority,
    /// `ReplyPriorityPolicy`.
    ReplyPriority,
    /// `RoutingPolicy` — TII (time-independent invocation) (spec §17.7).
    Routing,
    /// `MaxHopsPolicy`.
    MaxHops,
    /// `RequestStartTimePolicy` / `RequestEndTimePolicy`.
    RequestTime,
    /// `ReplyStartTimePolicy` / `ReplyEndTimePolicy`.
    ReplyTime,
    /// `RelativeRoundtripTimeoutPolicy`.
    RelativeRoundtripTimeout,
    /// `RoutingTypeRange`.
    RoutingTypeRange,
}

impl MessagingPolicy {
    /// Spec §17 — wire value (`PolicyType` is u32, vendor-defined).
    /// Follows the order in the OMG `Messaging.idl` (formal/2011-11-02
    /// §B.5.1, policy types 23..32).
    #[must_use]
    pub const fn policy_type(self) -> u32 {
        match self {
            Self::Rebind => 23,
            Self::SyncScope => 24,
            Self::RequestPriority => 25,
            Self::ReplyPriority => 26,
            Self::Routing => 30,
            Self::MaxHops => 32,
            Self::RequestTime => 27,
            Self::ReplyTime => 28,
            Self::RelativeRoundtripTimeout => 31,
            Self::RoutingTypeRange => 33,
        }
    }
}

/// Spec §17.1 — AMI (Asynchronous Method Invocation) reply-handler style.
/// `ReplyHandlerStyle` marks the code-generator path for the
/// stub code (callback vs. polling).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmiReplyHandler {
    /// Callback style — reply-handler object.
    Callback,
    /// Polling style — poller object.
    Polling,
}

/// Spec §17.5 — `AmiReplySink` trait for asynchronous reply processing.
///
/// Adapter trait that makes the three AMI callback methods from the spec IDL
/// (`handle_reply` / `handle_excep` / `handle_other`) concrete.
/// The `dispatch_async_reply` function maps a GIOP reply
/// onto the matching callback method.
pub trait AmiReplySink: Send + Sync {
    /// Spec §17.5.1 — `handle_reply(...)` for `NoException`.
    fn handle_reply(&self, request_id: u32, body: &[u8]);
    /// Spec §17.5.1 — `handle_excep(...)` for user/system exception.
    fn handle_excep(&self, request_id: u32, body: &[u8]);
    /// Spec §17.5.1 — `handle_other(...)` for LocationForward etc.
    fn handle_other(&self, request_id: u32, body: &[u8]);
}

/// Spec §17.5 — maps a `corba_giop::Reply` onto the matching
/// AMI callback method. This is the bridge between the GIOP wire and
/// the `AmiReplySink` surface.
pub fn dispatch_async_reply<S: AmiReplySink + ?Sized>(sink: &S, reply: &zerodds_corba_giop::Reply) {
    use zerodds_corba_giop::ReplyStatusType;
    match reply.reply_status {
        ReplyStatusType::NoException => sink.handle_reply(reply.request_id, &reply.body),
        ReplyStatusType::UserException | ReplyStatusType::SystemException => {
            sink.handle_excep(reply.request_id, &reply.body);
        }
        ReplyStatusType::LocationForward
        | ReplyStatusType::LocationForwardPerm
        | ReplyStatusType::NeedsAddressingMode => {
            sink.handle_other(reply.request_id, &reply.body);
        }
    }
}

/// Spec §17.7 — persistent request store for time-independent
/// invocations (TII). In-memory; the backing layer may be on disk.
#[cfg(feature = "std")]
#[derive(Debug, Default)]
pub struct PersistentRequestStore {
    inner: std::sync::Mutex<BTreeMap<u32, PersistentRequestEntry>>,
}

/// Entry in the persistent request store.
#[cfg(feature = "std")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistentRequestEntry {
    /// Body of the original request (CDR-encoded).
    pub body: Vec<u8>,
    /// Spec §17.7 — deadline for the request (epoch seconds).
    pub deadline_secs: u64,
}

#[cfg(feature = "std")]
impl PersistentRequestStore {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §17.7 — stores a request for asynchronous
    /// processing.
    pub fn add(&self, request_id: u32, body: Vec<u8>, deadline_secs: u64) {
        if let Ok(mut g) = self.inner.lock() {
            g.insert(
                request_id,
                PersistentRequestEntry {
                    body,
                    deadline_secs,
                },
            );
        }
    }

    /// Spec §17.7 — retrieves a request (removes it from the store).
    #[must_use]
    pub fn poll(&self, request_id: u32) -> Option<PersistentRequestEntry> {
        self.inner
            .lock()
            .ok()
            .and_then(|mut g| g.remove(&request_id))
    }

    /// Spec §17.7 — removes all entries with `deadline_secs < now`.
    /// Returns the request_ids of the expired entries.
    pub fn timeout_expired(&self, now_secs: u64) -> Vec<u32> {
        let Ok(mut g) = self.inner.lock() else {
            return Vec::new();
        };
        let expired: Vec<u32> = g
            .iter()
            .filter(|(_, e)| e.deadline_secs < now_secs)
            .map(|(k, _)| *k)
            .collect();
        for k in &expired {
            g.remove(k);
        }
        expired
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.inner.lock().map_or(0, |g| g.len())
    }

    /// `true` if there are no entries.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ===========================================================================
// §18 Compression (Part 1) + Part 2 §12 ZIOP
// ===========================================================================

/// Cap for [`CompressionAlgorithm::decompress`] output (1 MiB).
///
/// # DoS posture
///
/// `decompress` runs a zlib/gzip/deflate `read_to_end` over
/// attacker-controlled input. Without a bound, a small compressed
/// payload (a "zip bomb") can expand to gigabytes and exhaust memory
/// before the caller ever sees a result. This mirrors the cap
/// convention in `rtps::fragment_assembler` (`DEFAULT_MAX_SAMPLE_BYTES`,
/// 1 MiB) — dormant today (ZIOP is not wired to GIOP receive), but
/// required before ZIOP decompression sits on a wire-facing path.
#[cfg(feature = "std")]
pub const MAX_DECOMPRESSED_BYTES: usize = 1024 * 1024;

/// Spec §18 / Part 2 §12 — compression-algorithm identifier
/// (the CORBA 3.3 ZIOP spec standardizes "vendor-defined" algorithms).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionAlgorithm {
    /// `None` — no compression.
    None,
    /// `Zlib` (RFC 1950).
    Zlib,
    /// `Gzip` (RFC 1952).
    Gzip,
    /// `Lzma` (XZ).
    Lzma,
    /// `Deflate` (raw, RFC 1951).
    Deflate,
}

impl CompressionAlgorithm {
    /// Spec wire value (vendor-defined; we use 0=None, 1=Zlib, ...).
    #[must_use]
    pub const fn to_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Zlib => 1,
            Self::Gzip => 2,
            Self::Lzma => 3,
            Self::Deflate => 4,
        }
    }

    /// Conversion from the wire value.
    ///
    /// # Errors
    /// `()` if the value is unknown.
    #[allow(clippy::result_unit_err)]
    pub const fn from_u8(v: u8) -> Result<Self, ()> {
        match v {
            0 => Ok(Self::None),
            1 => Ok(Self::Zlib),
            2 => Ok(Self::Gzip),
            3 => Ok(Self::Lzma),
            4 => Ok(Self::Deflate),
            _ => Err(()),
        }
    }

    /// Spec §18 — compresses `input` with the chosen algorithm.
    ///
    /// Backend mapping:
    /// * [`Self::None`] — passthrough (bytes are cloned).
    /// * [`Self::Zlib`] — RFC 1950 (zlib-wrapped deflate) via `flate2`.
    /// * [`Self::Gzip`] — RFC 1952 via `flate2`.
    /// * [`Self::Deflate`] — RFC 1951 (raw deflate) via `flate2`.
    /// * [`Self::Lzma`] — XZ; not covered (the extra `xz2`/`liblzma`
    ///   build risk is disproportionate). Returns
    ///   [`CompressionError::Unsupported`].
    ///
    /// # Errors
    /// I/O error of the compression backend, or [`CompressionError::Unsupported`].
    #[cfg(feature = "std")]
    pub fn compress(self, input: &[u8]) -> Result<Vec<u8>, CompressionError> {
        use std::io::Write;
        match self {
            Self::None => Ok(input.to_vec()),
            Self::Zlib => {
                let mut e =
                    flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(input).map_err(CompressionError::from)?;
                e.finish().map_err(CompressionError::from)
            }
            Self::Gzip => {
                let mut e =
                    flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(input).map_err(CompressionError::from)?;
                e.finish().map_err(CompressionError::from)
            }
            Self::Deflate => {
                let mut e =
                    flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
                e.write_all(input).map_err(CompressionError::from)?;
                e.finish().map_err(CompressionError::from)
            }
            Self::Lzma => Err(CompressionError::Unsupported(Self::Lzma)),
        }
    }

    /// Spec §18 — decompresses `input` analogously to [`Self::compress`].
    ///
    /// Output is capped at [`MAX_DECOMPRESSED_BYTES`] — see that
    /// constant's doc for the DoS rationale. Input that decompresses
    /// past the cap is rejected with
    /// [`CompressionError::OutputTooLarge`] rather than allocating
    /// without bound.
    ///
    /// # Errors
    /// I/O error, [`CompressionError::OutputTooLarge`] if the
    /// decompressed size exceeds [`MAX_DECOMPRESSED_BYTES`], or
    /// [`CompressionError::Unsupported`] for LZMA.
    #[cfg(feature = "std")]
    pub fn decompress(self, input: &[u8]) -> Result<Vec<u8>, CompressionError> {
        match self {
            Self::None => {
                if input.len() > MAX_DECOMPRESSED_BYTES {
                    return Err(CompressionError::OutputTooLarge);
                }
                Ok(input.to_vec())
            }
            Self::Zlib => read_bounded(flate2::read::ZlibDecoder::new(input)),
            Self::Gzip => read_bounded(flate2::read::GzDecoder::new(input)),
            Self::Deflate => read_bounded(flate2::read::DeflateDecoder::new(input)),
            Self::Lzma => Err(CompressionError::Unsupported(Self::Lzma)),
        }
    }
}

/// Reads `r` to end, capped at [`MAX_DECOMPRESSED_BYTES`] + 1 bytes so the
/// intermediate buffer never grows unbounded — [`Read::take`] stops the
/// decompressor from producing more than the cap allows, and a full
/// `cap + 1`-byte read is treated as "exceeded the cap" rather than
/// silently truncated output.
#[cfg(feature = "std")]
fn read_bounded<R: std::io::Read>(r: R) -> Result<Vec<u8>, CompressionError> {
    use std::io::Read;
    let mut out = Vec::new();
    let limit = MAX_DECOMPRESSED_BYTES as u64 + 1;
    r.take(limit)
        .read_to_end(&mut out)
        .map_err(CompressionError::from)?;
    if out.len() > MAX_DECOMPRESSED_BYTES {
        return Err(CompressionError::OutputTooLarge);
    }
    Ok(out)
}

/// Error in the compression codec.
#[cfg(feature = "std")]
#[derive(Debug)]
pub enum CompressionError {
    /// Backend I/O error.
    Io(std::io::Error),
    /// The algorithm is not covered in the current build
    /// (decision record: LZMA requires an extra `xz2`/`liblzma` build).
    Unsupported(CompressionAlgorithm),
    /// Decompressed output exceeded [`MAX_DECOMPRESSED_BYTES`] — rejected
    /// before allocating further (DoS cap, see that constant's doc).
    OutputTooLarge,
}

#[cfg(feature = "std")]
impl core::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "compression io: {e}"),
            Self::OutputTooLarge => write!(
                f,
                "decompressed output exceeds cap ({MAX_DECOMPRESSED_BYTES} bytes)"
            ),
            Self::Unsupported(a) => write!(f, "compression unsupported: {a:?}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for CompressionError {}

#[cfg(feature = "std")]
impl From<std::io::Error> for CompressionError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Spec Part 2 §12 — ZIOP (compressed IIOP) configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZiopConfig {
    /// Algorithm.
    pub algorithm: CompressionAlgorithm,
    /// Threshold (bytes) — no compression below this value.
    pub min_size_threshold: u32,
    /// Compression level (0-9 for Zlib/Deflate; 0 = none, 9 = max).
    pub level: u8,
}

impl Default for ZiopConfig {
    fn default() -> Self {
        Self {
            algorithm: CompressionAlgorithm::None,
            min_size_threshold: 1024,
            level: 6,
        }
    }
}

// ===========================================================================
// Part 2 §11 MIOP (Multicast Inter-ORB Protocol)
// ===========================================================================

/// Spec Part 2 §11 — MIOP configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiopConfig {
    /// IPv4 multicast group address.
    pub group_addr_v4: [u8; 4],
    /// Port.
    pub port: u16,
    /// TTL (hop count).
    pub ttl: u8,
    /// Loopback (enable local replication).
    pub loopback: bool,
}

impl Default for MiopConfig {
    fn default() -> Self {
        Self {
            // Spec MIOP — default group in the 239.x range.
            group_addr_v4: [239, 255, 0, 1],
            port: 5683,
            ttl: 1,
            loopback: false,
        }
    }
}

/// Spec Part 2 §11 — MIOP magic bytes (`MIOP` ASCII).
pub const MIOP_MAGIC: [u8; 4] = *b"MIOP";

/// Spec Part 2 §11.4 — MIOP packet version (`0x10` = MIOP/1.0).
pub const MIOP_VERSION_1_0: u8 = 0x10;

/// Spec Part 2 §11.4 — MIOP packet header (10 bytes without magic, plus
/// 4-byte magic = 16 bytes header total). Encapsulates a GIOP frame
/// in a UDP multicast datagram.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MiopFrameHeader {
    /// Spec — packet version. Default `MIOP_VERSION_1_0`.
    pub version: u8,
    /// Spec — flags byte. Bit 0 = endian (0=BE,1=LE), bit 1 = last-frag.
    pub flags: u8,
    /// Spec — packet length (number of body bytes after this header).
    pub packet_length: u16,
    /// Spec — unique identifier for this multi-packet set.
    pub unique_id: u32,
    /// Spec — index of this packet within the set (0-based).
    pub packet_number: u8,
    /// Spec — number of packets in the set (>= 1).
    pub number_of_packets: u8,
}

impl MiopFrameHeader {
    /// Byte length of the header (magic + 10-byte body).
    pub const ENCODED_LEN: usize = 14;

    /// Constructor for a single-packet MIOP frame (typical case:
    /// the GIOP message fits in one UDP datagram).
    #[must_use]
    pub const fn single_packet(unique_id: u32, packet_length: u16, little_endian: bool) -> Self {
        let mut flags: u8 = 0;
        if little_endian {
            flags |= 0x01;
        }
        // Last-frag bit set (bit 1).
        flags |= 0x02;
        Self {
            version: MIOP_VERSION_1_0,
            flags,
            packet_length,
            unique_id,
            packet_number: 0,
            number_of_packets: 1,
        }
    }

    /// Spec Part 2 §11.4 — header encode (16 bytes, big-endian
    /// wire order for the u16/u32 fields).
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&MIOP_MAGIC);
        out.push(self.version);
        out.push(self.flags);
        out.extend_from_slice(&self.packet_length.to_be_bytes());
        out.extend_from_slice(&self.unique_id.to_be_bytes());
        out.push(self.packet_number);
        out.push(self.number_of_packets);
    }

    /// Spec Part 2 §11.4 — header decode from the first 16 bytes.
    ///
    /// # Errors
    /// `MiopError::TooShort`/`InvalidMagic`/`UnsupportedVersion`.
    pub fn decode(input: &[u8]) -> Result<(Self, usize), MiopError> {
        if input.len() < Self::ENCODED_LEN {
            return Err(MiopError::TooShort);
        }
        if input[0..4] != MIOP_MAGIC {
            return Err(MiopError::InvalidMagic);
        }
        let version = input[4];
        if version != MIOP_VERSION_1_0 {
            return Err(MiopError::UnsupportedVersion(version));
        }
        let flags = input[5];
        let packet_length = u16::from_be_bytes([input[6], input[7]]);
        let unique_id = u32::from_be_bytes([input[8], input[9], input[10], input[11]]);
        let packet_number = input[12];
        let number_of_packets = input[13];
        Ok((
            Self {
                version,
                flags,
                packet_length,
                unique_id,
                packet_number,
                number_of_packets,
            },
            Self::ENCODED_LEN,
        ))
    }

    /// Spec — `true` if the last-fragment bit is set.
    #[must_use]
    pub const fn is_last_fragment(&self) -> bool {
        (self.flags & 0x02) != 0
    }

    /// Spec — `true` if the endian bit indicates little-endian.
    #[must_use]
    pub const fn is_little_endian(&self) -> bool {
        (self.flags & 0x01) != 0
    }
}

/// MIOP codec errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MiopError {
    /// Buffer too small for the header.
    TooShort,
    /// Magic bytes != "MIOP".
    InvalidMagic,
    /// Unsupported MIOP version.
    UnsupportedVersion(u8),
}

/// Spec Part 2 §11 — MIOP sender. Adapter trait for the multicast
/// sink. Concrete implementations live in `transport-udp`.
pub trait MulticastSink: Send + Sync {
    /// Sends a UDP datagram to the multicast group.
    ///
    /// # Errors
    /// Implementation-specific (e.g. socket IO).
    fn send_datagram(&self, data: &[u8]) -> Result<(), MulticastSinkError>;
}

/// Multicast sink error (opaque).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MulticastSinkError(pub String);

/// Spec Part 2 §11 — MIOP sender that packs a GIOP message body into
/// MIOP frames and sends them over a `MulticastSink`.
///
/// `mtu` is the UDP datagram size (typically 1472 at a 1500 MTU
/// minus the 28-byte IP/UDP header). If the GIOP bytes do not fit in a
/// single MIOP frame, they are fragmented.
pub struct MiopSender<S: MulticastSink> {
    sink: S,
    mtu: usize,
    next_unique_id: core::sync::atomic::AtomicU32,
}

impl<S: MulticastSink> MiopSender<S> {
    /// Constructor.
    #[must_use]
    pub fn new(sink: S, mtu: usize) -> Self {
        Self {
            sink,
            mtu,
            next_unique_id: core::sync::atomic::AtomicU32::new(1),
        }
    }

    /// Maximum body size per MIOP frame.
    #[must_use]
    pub const fn max_body_per_frame(&self) -> usize {
        self.mtu.saturating_sub(MiopFrameHeader::ENCODED_LEN)
    }

    /// Spec Part 2 §11.4 — sends a GIOP message body as a single- or
    /// multi-part MIOP set.
    ///
    /// # Errors
    /// `MulticastSinkError` on an IO failure of the sink.
    pub fn send_giop(
        &self,
        giop_bytes: &[u8],
        little_endian: bool,
    ) -> Result<(), MulticastSinkError> {
        let max = self.max_body_per_frame();
        let unique_id = self
            .next_unique_id
            .fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        if max == 0 || giop_bytes.len() <= max {
            // Single-packet path.
            let header = MiopFrameHeader::single_packet(
                unique_id,
                u16::try_from(giop_bytes.len()).unwrap_or(u16::MAX),
                little_endian,
            );
            let mut datagram = Vec::with_capacity(MiopFrameHeader::ENCODED_LEN + giop_bytes.len());
            header.encode(&mut datagram);
            datagram.extend_from_slice(giop_bytes);
            return self.sink.send_datagram(&datagram);
        }
        // Multi-packet path.
        let total_len = giop_bytes.len();
        let total_packets = total_len.div_ceil(max);
        let total_packets_u8 = u8::try_from(total_packets).unwrap_or(u8::MAX);
        for (idx, chunk) in giop_bytes.chunks(max).enumerate() {
            let mut flags: u8 = 0;
            if little_endian {
                flags |= 0x01;
            }
            let is_last = idx + 1 == total_packets;
            if is_last {
                flags |= 0x02;
            }
            let header = MiopFrameHeader {
                version: MIOP_VERSION_1_0,
                flags,
                packet_length: u16::try_from(chunk.len()).unwrap_or(u16::MAX),
                unique_id,
                packet_number: u8::try_from(idx).unwrap_or(u8::MAX),
                number_of_packets: total_packets_u8,
            };
            let mut datagram = Vec::with_capacity(MiopFrameHeader::ENCODED_LEN + chunk.len());
            header.encode(&mut datagram);
            datagram.extend_from_slice(chunk);
            self.sink.send_datagram(&datagram)?;
        }
        Ok(())
    }
}

// ===========================================================================
// Part 2 §8 Inter-ORB Bridges
// ===========================================================================

/// Spec Part 2 §8 — bridge mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeMode {
    /// `Inline-Bridge` — embedded in an ORB process.
    Inline,
    /// `Request-Level-Bridge` — a separate bridge process via DSI/DII.
    RequestLevel,
}

/// Spec Part 2 §8 — bridge configuration.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Bridge mode.
    pub mode: BridgeMode,
    /// Source ORB identifier (e.g. `"corba"`).
    pub source_orb: String,
    /// Target ORB identifier (e.g. `"dds"`).
    pub target_orb: String,
}

// ===========================================================================
// Part 2 §9.8/§9.9 Bi-Directional GIOP
// ===========================================================================

/// Spec Part 2 §9.8 — BiDir policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiDirPolicy {
    /// `NORMAL` — standard IIOP, no BiDir.
    Normal,
    /// `BOTH` — the server can also send outgoing requests over the same
    /// connection (callback style).
    Both,
}

/// Spec Part 2 §9.9 — bidirectional service context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BiDirServiceContext {
    /// Spec §9.9.1 — listen points (the client's host:port pairs).
    pub listen_points: Vec<(String, u16)>,
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    // §16 Portable Interceptors
    struct DummyClient;
    impl ClientRequestInterceptor for DummyClient {
        fn name(&self) -> &str {
            "dummy-client"
        }
        fn intercept(&self, _: ClientInterceptionPoint, _: &str) {}
    }

    struct DummyServer;
    impl ServerRequestInterceptor for DummyServer {
        fn name(&self) -> &str {
            "dummy-server"
        }
        fn intercept(&self, _: ServerInterceptionPoint, _: &str) {}
    }

    struct DummyIor;
    impl IorInterceptor for DummyIor {
        fn name(&self) -> &str {
            "dummy-ior"
        }
        fn establish_components(&self) -> Vec<u32> {
            alloc::vec![]
        }
    }

    #[test]
    fn registry_add_increments_counts() {
        let mut r = InterceptorRegistry::new();
        r.add_client(Arc::new(DummyClient) as Arc<dyn ClientRequestInterceptor>);
        r.add_server(Arc::new(DummyServer) as Arc<dyn ServerRequestInterceptor>);
        r.add_ior(Arc::new(DummyIor) as Arc<dyn IorInterceptor>);
        assert_eq!(r.client_count(), 1);
        assert_eq!(r.server_count(), 1);
        assert_eq!(r.ior_count(), 1);
    }

    #[test]
    fn picurrent_set_get_slot() {
        let mut p = PiCurrent::new();
        p.set_slot(7, alloc::vec![0xab, 0xcd]);
        assert_eq!(p.get_slot(7), Some(&[0xab, 0xcd][..]));
        assert!(p.get_slot(99).is_none());
    }

    #[test]
    fn client_interception_points_distinct() {
        assert_ne!(
            ClientInterceptionPoint::SendRequest,
            ClientInterceptionPoint::ReceiveReply
        );
    }

    #[test]
    fn server_interception_points_distinct() {
        assert_ne!(
            ServerInterceptionPoint::ReceiveRequest,
            ServerInterceptionPoint::SendReply
        );
    }

    // §17 Messaging
    #[test]
    fn messaging_policies_distinct() {
        assert_ne!(MessagingPolicy::Rebind, MessagingPolicy::SyncScope);
    }

    #[test]
    fn ami_reply_handler_distinct() {
        assert_ne!(AmiReplyHandler::Callback, AmiReplyHandler::Polling);
    }

    // §18 + Part 2 §12 ZIOP
    #[test]
    fn compression_algorithm_round_trip() {
        for a in [
            CompressionAlgorithm::None,
            CompressionAlgorithm::Zlib,
            CompressionAlgorithm::Gzip,
            CompressionAlgorithm::Lzma,
            CompressionAlgorithm::Deflate,
        ] {
            assert_eq!(CompressionAlgorithm::from_u8(a.to_u8()).expect("ok"), a);
        }
    }

    #[test]
    fn compression_algorithm_unknown_rejected() {
        assert!(CompressionAlgorithm::from_u8(99).is_err());
    }

    #[test]
    fn ziop_config_default_no_compression() {
        let c = ZiopConfig::default();
        assert_eq!(c.algorithm, CompressionAlgorithm::None);
        assert_eq!(c.min_size_threshold, 1024);
    }

    // §18 Compression — Wire-up

    #[cfg(feature = "std")]
    #[test]
    fn compression_none_passes_through() {
        let input = b"hello, corba";
        let out = CompressionAlgorithm::None
            .compress(input)
            .expect("compress ok");
        assert_eq!(out, input);
        let back = CompressionAlgorithm::None
            .decompress(&out)
            .expect("decompress ok");
        assert_eq!(back, input);
    }

    #[cfg(feature = "std")]
    #[test]
    fn compression_zlib_round_trip() {
        let input = b"the quick brown fox jumps over the lazy dog".repeat(8);
        let compressed = CompressionAlgorithm::Zlib
            .compress(&input)
            .expect("compress ok");
        // The Zlib output must be shorter than the repeated input.
        assert!(compressed.len() < input.len());
        let back = CompressionAlgorithm::Zlib
            .decompress(&compressed)
            .expect("decompress ok");
        assert_eq!(back, input);
    }

    #[cfg(feature = "std")]
    #[test]
    fn compression_zlib_decompress_bomb_rejected() {
        // A highly compressible input (all zeros) that expands past
        // MAX_DECOMPRESSED_BYTES on decompress — the classic "zip bomb"
        // shape: tiny wire payload, huge decompressed output. Must be
        // rejected with OutputTooLarge, not allocated without bound.
        let input = vec![0_u8; MAX_DECOMPRESSED_BYTES + 4096];
        let compressed = CompressionAlgorithm::Zlib
            .compress(&input)
            .expect("compress ok");
        assert!(compressed.len() < MAX_DECOMPRESSED_BYTES / 100);
        let err = CompressionAlgorithm::Zlib
            .decompress(&compressed)
            .expect_err("must be rejected as too large");
        assert!(matches!(err, CompressionError::OutputTooLarge));
    }

    #[cfg(feature = "std")]
    #[test]
    fn compression_none_decompress_bomb_rejected() {
        // `None` has no expansion, but the cap must still reject
        // oversized input consistently across all algorithm arms.
        let input = vec![0_u8; MAX_DECOMPRESSED_BYTES + 1];
        let err = CompressionAlgorithm::None
            .decompress(&input)
            .expect_err("must be rejected as too large");
        assert!(matches!(err, CompressionError::OutputTooLarge));
    }

    #[cfg(feature = "std")]
    #[test]
    fn compression_gzip_round_trip() {
        let input = b"OMG-CORBA-3.3 18 Compression spec".repeat(16);
        let compressed = CompressionAlgorithm::Gzip
            .compress(&input)
            .expect("compress ok");
        let back = CompressionAlgorithm::Gzip
            .decompress(&compressed)
            .expect("decompress ok");
        assert_eq!(back, input);
    }

    #[cfg(feature = "std")]
    #[test]
    fn compression_deflate_round_trip() {
        let input = b"deflate raw RFC1951".repeat(32);
        let compressed = CompressionAlgorithm::Deflate
            .compress(&input)
            .expect("compress ok");
        let back = CompressionAlgorithm::Deflate
            .decompress(&compressed)
            .expect("decompress ok");
        assert_eq!(back, input);
    }

    #[cfg(feature = "std")]
    #[test]
    fn compression_lzma_returns_unsupported() {
        let err = CompressionAlgorithm::Lzma
            .compress(b"x")
            .expect_err("must fail");
        assert!(matches!(
            err,
            CompressionError::Unsupported(CompressionAlgorithm::Lzma)
        ));
    }

    #[cfg(feature = "std")]
    #[test]
    fn compression_zlib_handles_large_block() {
        // 10 kB pseudo-random block: byte pattern from a deterministic
        // index function so the test does not depend on rand deps.
        let input: Vec<u8> = (0..10_000_u32)
            .map(|i| (i.wrapping_mul(2654435761) >> 24) as u8)
            .collect();
        let compressed = CompressionAlgorithm::Zlib
            .compress(&input)
            .expect("compress ok");
        let back = CompressionAlgorithm::Zlib
            .decompress(&compressed)
            .expect("decompress ok");
        assert_eq!(back, input);
    }

    // Part 2 §11 MIOP
    #[test]
    fn miop_config_default_uses_239_range() {
        let m = MiopConfig::default();
        assert_eq!(m.group_addr_v4[0], 239);
        assert_eq!(m.port, 5683);
        assert_eq!(m.ttl, 1);
    }

    // Part 2 §8 Bridges
    #[test]
    fn bridge_modes_distinct() {
        assert_ne!(BridgeMode::Inline, BridgeMode::RequestLevel);
    }

    #[test]
    fn bridge_config_construct() {
        let c = BridgeConfig {
            mode: BridgeMode::RequestLevel,
            source_orb: "corba".into(),
            target_orb: "dds".into(),
        };
        assert_eq!(c.source_orb, "corba");
    }

    // Part 2 §9.8/§9.9 BiDir
    #[test]
    fn bidir_policy_distinct() {
        assert_ne!(BiDirPolicy::Normal, BiDirPolicy::Both);
    }

    #[test]
    fn bidir_service_context_listen_points() {
        let sc = BiDirServiceContext {
            listen_points: alloc::vec![("client.example".into(), 8080)],
        };
        assert_eq!(sc.listen_points.len(), 1);
    }

    // §16 Portable Interceptors — Pipeline-Walks
    #[test]
    fn registry_walk_client_invokes_all_client_interceptors() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        struct Counting {
            count: alloc::sync::Arc<AtomicUsize>,
        }
        impl ClientRequestInterceptor for Counting {
            fn name(&self) -> &str {
                "counting"
            }
            fn intercept(&self, _: ClientInterceptionPoint, _: &str) {
                self.count.fetch_add(1, Ordering::Relaxed);
            }
        }
        let count = alloc::sync::Arc::new(AtomicUsize::new(0));
        let mut r = InterceptorRegistry::new();
        r.add_client(alloc::sync::Arc::new(Counting {
            count: count.clone(),
        }) as Arc<dyn ClientRequestInterceptor>);
        r.add_client(alloc::sync::Arc::new(Counting {
            count: count.clone(),
        }) as Arc<dyn ClientRequestInterceptor>);
        r.walk_client(ClientInterceptionPoint::SendRequest, "op");
        assert_eq!(count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn registry_walk_ior_collects_tags() {
        struct EmitTwo;
        impl IorInterceptor for EmitTwo {
            fn name(&self) -> &str {
                "emit-two"
            }
            fn establish_components(&self) -> Vec<u32> {
                alloc::vec![0xAAAA_AAAA, 0xBBBB_BBBB]
            }
        }
        let mut r = InterceptorRegistry::new();
        r.add_ior(Arc::new(EmitTwo) as Arc<dyn IorInterceptor>);
        let tags = r.walk_ior();
        assert_eq!(tags, alloc::vec![0xAAAA_AAAA, 0xBBBB_BBBB]);
    }

    // §17 Messaging — Policy-Type-Wire
    #[test]
    fn messaging_policy_wire_values_match_omg_messaging_idl() {
        assert_eq!(MessagingPolicy::Rebind.policy_type(), 23);
        assert_eq!(MessagingPolicy::SyncScope.policy_type(), 24);
        assert_eq!(MessagingPolicy::Routing.policy_type(), 30);
        assert_eq!(MessagingPolicy::RelativeRoundtripTimeout.policy_type(), 31);
    }

    // §17 AMI — Reply-Dispatch
    struct RecordingSink {
        replies: alloc::sync::Arc<std::sync::Mutex<Vec<(u32, &'static str)>>>,
    }
    impl AmiReplySink for RecordingSink {
        fn handle_reply(&self, request_id: u32, _body: &[u8]) {
            if let Ok(mut g) = self.replies.lock() {
                g.push((request_id, "reply"));
            }
        }
        fn handle_excep(&self, request_id: u32, _body: &[u8]) {
            if let Ok(mut g) = self.replies.lock() {
                g.push((request_id, "excep"));
            }
        }
        fn handle_other(&self, request_id: u32, _body: &[u8]) {
            if let Ok(mut g) = self.replies.lock() {
                g.push((request_id, "other"));
            }
        }
    }

    #[test]
    fn ami_handler_handles_no_exception_reply() {
        use zerodds_corba_giop::{Reply, ReplyStatusType, ServiceContextList};
        let replies = alloc::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = RecordingSink {
            replies: replies.clone(),
        };
        let r = Reply {
            request_id: 42,
            reply_status: ReplyStatusType::NoException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![1, 2, 3],
        };
        dispatch_async_reply(&sink, &r);
        let g = replies.lock().unwrap();
        assert_eq!(*g, alloc::vec![(42_u32, "reply")]);
    }

    #[test]
    fn ami_handler_handles_user_exception_reply() {
        use zerodds_corba_giop::{Reply, ReplyStatusType, ServiceContextList};
        let replies = alloc::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = RecordingSink {
            replies: replies.clone(),
        };
        let r = Reply {
            request_id: 7,
            reply_status: ReplyStatusType::UserException,
            service_context: ServiceContextList::default(),
            body: alloc::vec![],
        };
        dispatch_async_reply(&sink, &r);
        let g = replies.lock().unwrap();
        assert_eq!(*g, alloc::vec![(7_u32, "excep")]);
    }

    // §17.7 persistent request store for TII
    #[cfg(feature = "std")]
    #[test]
    fn persistent_request_store_add_poll_timeout() {
        let s = PersistentRequestStore::new();
        s.add(1, alloc::vec![0xAA], 100);
        s.add(2, alloc::vec![0xBB], 50);
        s.add(3, alloc::vec![0xCC], 200);
        assert_eq!(s.len(), 3);

        // Poll retrieves an entry (removes it from the store).
        let e1 = s.poll(1).expect("present");
        assert_eq!(e1.body, alloc::vec![0xAA]);
        assert_eq!(e1.deadline_secs, 100);
        assert!(s.poll(1).is_none());
        assert_eq!(s.len(), 2);

        // Timeout @ now=120 → request 2 (deadline=50) is expired.
        let expired = s.timeout_expired(120);
        assert_eq!(expired, alloc::vec![2]);
        assert_eq!(s.len(), 1);

        // Request 3 (deadline=200) stays in.
        assert!(s.poll(3).is_some());
    }

    // Part 2 §11 MIOP — Frame-Codec
    #[test]
    fn miop_frame_encode_decode_roundtrip() {
        let h = MiopFrameHeader::single_packet(0xCAFE_BABE, 1234, true);
        let mut bytes = Vec::new();
        h.encode(&mut bytes);
        assert_eq!(bytes.len(), MiopFrameHeader::ENCODED_LEN);
        let (decoded, consumed) = MiopFrameHeader::decode(&bytes).expect("decode");
        assert_eq!(consumed, MiopFrameHeader::ENCODED_LEN);
        assert_eq!(decoded, h);
        assert!(decoded.is_last_fragment());
        assert!(decoded.is_little_endian());
    }

    #[test]
    fn miop_frame_decode_rejects_bad_magic_and_version() {
        let mut bad = alloc::vec![b'X', b'X', b'X', b'X'];
        bad.extend_from_slice(&[0u8; 10]);
        assert_eq!(
            MiopFrameHeader::decode(&bad).unwrap_err(),
            MiopError::InvalidMagic
        );

        let mut wrong_version = MIOP_MAGIC.to_vec();
        wrong_version.push(0xFF); // version
        wrong_version.extend_from_slice(&[0u8; 9]);
        assert_eq!(
            MiopFrameHeader::decode(&wrong_version).unwrap_err(),
            MiopError::UnsupportedVersion(0xFF)
        );

        let too_short = MIOP_MAGIC.to_vec();
        assert_eq!(
            MiopFrameHeader::decode(&too_short).unwrap_err(),
            MiopError::TooShort
        );
    }

    // Part 2 §11 MIOP — Sender (Single + Multi-Packet)
    struct MockSink {
        sent: alloc::sync::Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
    }
    impl MulticastSink for MockSink {
        fn send_datagram(&self, data: &[u8]) -> Result<(), MulticastSinkError> {
            if let Ok(mut g) = self.sent.lock() {
                g.push(data.to_vec());
            }
            Ok(())
        }
    }

    #[test]
    fn miop_sender_single_packet_fits_mtu() {
        let sent = alloc::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = MockSink { sent: sent.clone() };
        let sender = MiopSender::new(sink, 256);
        let payload = alloc::vec![0xAB; 100];
        sender.send_giop(&payload, false).expect("send");

        let g = sent.lock().unwrap();
        assert_eq!(g.len(), 1, "single-packet path produces 1 datagram");
        assert!(g[0].starts_with(&MIOP_MAGIC));
        // Datagram = header (14) + payload (100).
        assert_eq!(g[0].len(), MiopFrameHeader::ENCODED_LEN + 100);
    }

    #[test]
    fn miop_sender_fragments_multi_packet_over_small_mtu() {
        let sent = alloc::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = MockSink { sent: sent.clone() };
        // MTU = 14 (header) + 30 (body) = 44.
        let sender = MiopSender::new(sink, 44);
        let payload = alloc::vec![0xCD; 100];
        sender.send_giop(&payload, true).expect("send");

        let g = sent.lock().unwrap();
        // 100 / 30 -> 4 packets (30+30+30+10).
        assert_eq!(g.len(), 4);
        // The last frame must have the last-fragment bit set.
        let (last_header, _) = MiopFrameHeader::decode(&g[3]).expect("decode");
        assert!(last_header.is_last_fragment());
        assert_eq!(last_header.packet_number, 3);
        assert_eq!(last_header.number_of_packets, 4);
        // First frame: last-fragment NOT set.
        let (first_header, _) = MiopFrameHeader::decode(&g[0]).expect("decode");
        assert!(!first_header.is_last_fragment());
        assert_eq!(first_header.packet_number, 0);
    }
}
