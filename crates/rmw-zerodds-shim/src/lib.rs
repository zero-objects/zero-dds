// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `rmw_zerodds` — ROS 2 RMW shim over ZeroDDS.
//!
//! Crate `rmw-zerodds-shim`. Safety classification: **STANDARD** (FFI boundary).
//!
//! # Architecture
//!
//! ```text
//!   rclcpp/rclpy
//!       │  rmw API (REP-2007)
//!       ▼
//!   librmw_zerodds.so   ◀── this crate
//!       │  ZeroDDS C-API (zerodds.h)
//!       ▼
//!   libzerodds.so       (crates/zerodds-c-api)
//! ```
//!
//! The REP-2007 mapping comes from `zerodds-ros2-rmw` (topic mangling, QoS
//! profiles, identifier constraints). The underlying wire path is
//! the `zerodds-c-api` runtime — `zerodds_runtime_create`,
//! `zerodds_writer_create`, `zerodds_reader_take`.
//!
//! # Implemented surface
//!
//! The shim implements the RMW surface rclcpp drives — no exported entry
//! point returns `RMW_RET_UNSUPPORTED`:
//! * identifier + serialization format
//! * `_create_init_options` / `_fini`, `_init` / `_shutdown` / `_context_fini`
//! * nodes (`_create_node` / `_destroy_node`)
//! * publishers / subscriptions (`_publish` / `_take`)
//! * services: clients + services, request/response
//!   (`_send_request` / `_take_request` / `_send_response` / `_take_response`)
//! * wait-sets + guard conditions (`_create_wait_set`, `_wait`)
//! * message loaning (`_borrow_loaned_message` / `_publish_loaned_message`
//!   / `_return_loaned_message`) — `Portable` delivery mode
//! * REP-2009 type hash (`_compute_type_hash`)

#![warn(missing_docs)]
#![allow(clippy::missing_safety_doc)]

extern crate alloc;

use core::ffi::{c_char, c_void};
use core::ptr;
use std::ffi::CStr;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use zerodds_ros2_rmw::ffi_api::RMW_IMPLEMENTATION_IDENTIFIER;

mod dynamic;
mod events;
pub mod qos;
pub mod ros2_config;
pub mod services;
pub mod spatial_bridge;
pub use qos::RmwZerodsQos;
// The `zerodds` crate (= `zerodds-c-api` per `[lib] name = "zerodds"`)
// is used below directly via `zerodds::ZeroDdsRuntime` / `zerodds::zerodds_*`.
// The explicit `use zerodds as _;` marker is dropped — the
// real paths are self-explanatory.

/// `rmw_ret_t` aliases as plain int. Spec REP-2007 §4 codes.
#[allow(non_upper_case_globals)]
pub const RMW_RET_OK: i32 = 0;
#[allow(non_upper_case_globals)]
/// `RMW_RET_ERROR`.
pub const RMW_RET_ERROR: i32 = 1;
#[allow(non_upper_case_globals)]
/// `RMW_RET_TIMEOUT`.
pub const RMW_RET_TIMEOUT: i32 = 2;
#[allow(non_upper_case_globals)]
/// `RMW_RET_UNSUPPORTED`.
pub const RMW_RET_UNSUPPORTED: i32 = 3;
#[allow(non_upper_case_globals)]
/// `RMW_RET_BAD_ALLOC`.
pub const RMW_RET_BAD_ALLOC: i32 = 10;
#[allow(non_upper_case_globals)]
/// `RMW_RET_INVALID_ARGUMENT`.
pub const RMW_RET_INVALID_ARGUMENT: i32 = 11;
#[allow(non_upper_case_globals)]
/// `RMW_RET_INCORRECT_RMW_IMPLEMENTATION`.
pub const RMW_RET_INCORRECT_RMW_IMPLEMENTATION: i32 = 12;

// ============================================================================
// Event-driven wakeup (shared by all subscriptions of a context)
// ============================================================================

/// Edge source shared across a context's subscriptions and guard conditions.
///
/// A subscription registers a data callback on its ZeroDDS reader
/// ([`zerodds::zerodds_reader_set_data_callback`]); that callback fires in the
/// receive thread the instant a sample lands and calls `WaitNotify::notify`,
/// which bumps a generation counter and wakes the condvar. `rmw_wait` blocks on
/// the condvar via `WaitNotify::wait_until` and re-evaluates readiness on each
/// wake — no spin loop and no fixed-tick poll (the readiness *truth* is the
/// non-destructive `subscription_has_data` peek; this type only supplies the
/// blocking-until-something-changed edge).
pub struct WaitNotify {
    generation: Mutex<u64>,
    cv: Condvar,
}

impl WaitNotify {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            generation: Mutex::new(0),
            cv: Condvar::new(),
        })
    }

    /// Wakes every blocked `wait_until`. Called from the reader receive thread
    /// (data callback) and from a guard-condition trigger.
    fn notify(&self) {
        if let Ok(mut g) = self.generation.lock() {
            *g = g.wrapping_add(1);
            self.cv.notify_all();
        }
    }

    /// Current generation — snapshot it *before* checking readiness so a notify
    /// racing in between is not lost (it changes the generation, so the
    /// subsequent `wait_until` returns at once).
    fn current(&self) -> u64 {
        self.generation.lock().map(|g| *g).unwrap_or(0)
    }

    /// Blocks until the generation moves past `since` or `deadline` elapses.
    /// Returns `true` if woken by a generation change (a notify fired), `false`
    /// on a plain timeout. The caller uses this to distinguish "something
    /// happened, re-evaluate" from "nothing happened, keep waiting".
    fn wait_until(&self, since: u64, deadline: Instant) -> bool {
        let Ok(mut g) = self.generation.lock() else {
            return false;
        };
        while *g == since {
            let now = Instant::now();
            if now >= deadline {
                return false; // timed out, generation unchanged
            }
            match self.cv.wait_timeout(g, deadline - now) {
                Ok((ng, _)) => g = ng,
                Err(_) => return false,
            }
        }
        true // generation moved → woken by a notify
    }
}

/// Per-subscription inbox. The runtime delivers a subscription's samples
/// EITHER to a data listener OR to its MPSC channel — never both (they are
/// mutually exclusive, `crates/dcps/src/runtime.rs`). The shim therefore makes
/// the listener the subscription's sole delivery path: the callback (fired in
/// the receive thread the instant a sample lands) copies the CDR body here and
/// wakes the context condvar; `take` / `has_data` drain this queue.
/// One parked sample awaiting `take`: `(CDR body, representation byte,
/// big_endian flag)` from the encapsulation header (RTPS 2.5 §10.5).
type InboxEntry = (alloc::boxed::Box<[u8]>, u8, u8, RmwZerodsSampleOrigin);

/// Where a taken sample came from, for `rmw_message_info_t`: the writer's
/// GUID (the publisher GID), its source timestamp in nanoseconds (`-1`
/// unknown) and its RTPS sequence number (`-1` unknown).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct RmwZerodsSampleOrigin {
    /// The writer's GUID.
    pub writer_guid: [u8; 16],
    /// Source timestamp, nanoseconds since the Unix epoch, `-1` unknown.
    pub source_timestamp_ns: i64,
    /// The writer's sequence number of the sample, `-1` unknown.
    pub sequence_number: i64,
}

struct SubInbox {
    // (CDR body, representation, big_endian). `representation`/`big_endian` are
    // the wire byte order from the encapsulation header (RTPS 2.5 §10.5),
    // carried for completeness; the C introspection deserializer is currently
    // CDR_LE-only (REP-2007), so a big-endian remote sample is a known
    // pre-existing limitation rather than honored here.
    queue: Mutex<alloc::collections::VecDeque<InboxEntry>>,
    notify: Arc<WaitNotify>,
    /// Optional rmw event callback (EventsExecutor `on_new_message/request/
    /// response`), fired on each arrival with `number_of_events = 1`.
    event: Mutex<Option<InboxEvent>>,
    /// KEEP_LAST depth of the subscription (DDS 1.4 §2.2.3.18): an arrival
    /// past it drops the oldest queued sample. `None` = KEEP_ALL.
    keep_last: Option<usize>,
}

/// rmw event callback: `(user_data, number_of_events)` — `rmw_event_callback_t`.
pub type RmwEventCallback = extern "C" fn(*const c_void, usize);

/// A stored rmw event callback. `ud` is held as `usize` (the `const void*`
/// user_data) so the struct stays `Send`; the rmw contract requires the caller
/// to keep `user_data` alive until the callback is cleared with NULL.
#[derive(Clone, Copy)]
struct InboxEvent {
    cb: RmwEventCallback,
    ud: usize,
}

/// Sets (or clears with `None`) the inbox event callback. On set, flushes any
/// already-queued messages by invoking the callback once with the backlog count
/// (so a callback registered after data arrived does not miss it).
fn inbox_set_event(inbox: &SubInbox, cb: Option<RmwEventCallback>, ud: *const c_void) {
    let ev = cb.map(|c| InboxEvent {
        cb: c,
        ud: ud as usize,
    });
    {
        let Ok(mut g) = inbox.event.lock() else {
            return;
        };
        *g = ev;
    }
    if let Some(ev) = ev {
        let backlog = inbox.queue.lock().map(|q| q.len()).unwrap_or(0);
        if backlog > 0 {
            (ev.cb)(ev.ud as *const c_void, backlog);
        }
    }
}

/// Data callback registered on each subscription's ZeroDDS reader. `user_data`
/// is an [`Arc<SubInbox>`] raw pointer kept alive by the owning subscription
/// until after the reader is destroyed (see `rmw_zerodds_destroy_subscription`).
extern "C" fn subscription_on_data(
    ud: *mut c_void,
    payload: *const u8,
    len: usize,
    repr: u8,
    big_endian: u8,
    origin: *const zerodds::ZeroDdsSampleOrigin,
) {
    if ud.is_null() || (len != 0 && payload.is_null()) {
        return;
    }
    // SAFETY: `ud` is a live `Arc<SubInbox>` raw pointer; the subscription tears
    // the reader (and thus this callback source) down before reclaiming the Arc,
    // so the referent outlives every callback invocation.
    let inbox = unsafe { &*(ud as *const SubInbox) };
    // SAFETY: `payload`/`len` is the listener's CDR-body slice, valid for the
    // duration of this call (the runtime owns it).
    let bytes: alloc::boxed::Box<[u8]> = if len == 0 {
        alloc::boxed::Box::default()
    } else {
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        unsafe { core::slice::from_raw_parts(payload, len) }.into()
    };
    let origin = if origin.is_null() {
        RmwZerodsSampleOrigin {
            source_timestamp_ns: -1,
            sequence_number: -1,
            ..RmwZerodsSampleOrigin::default()
        }
    } else {
        // SAFETY: valid for the call per the callback contract.
        let o = unsafe { &*origin };
        RmwZerodsSampleOrigin {
            writer_guid: o.writer_guid,
            source_timestamp_ns: o.source_timestamp_ns,
            sequence_number: o.sequence_number,
        }
    };
    if let Ok(mut q) = inbox.queue.lock() {
        q.push_back((bytes, repr, big_endian, origin));
        if let Some(depth) = inbox.keep_last {
            while q.len() > depth.max(1) {
                q.pop_front();
            }
        }
    }
    inbox.notify.notify();
    // Fire the rmw EventsExecutor callback (snapshot it, then call outside the
    // lock so a re-entrant set_callback cannot deadlock).
    let ev = inbox.event.lock().ok().and_then(|g| *g);
    if let Some(ev) = ev {
        (ev.cb)(ev.ud as *const c_void, 1);
    }
}

// ============================================================================
// Node graph (rmw_get_node_names) via the ROS 2 `ros_discovery_info` topic
// ============================================================================
//
// Each participant publishes a `rmw_dds_common::msg::ParticipantEntitiesInfo`
// (participant gid + a sequence of nodes, each with name/namespace) on the
// `ros_discovery_info` topic and subscribes to it; `rmw_get_node_names`
// aggregates the local nodes (tracked here directly — a runtime does not deliver
// to its own reader) with the remote participants' nodes. The well-known CDR
// (XCDR1, LE) is hand-encoded so no rmw_dds_common typesupport linkage is needed.

const DISCOVERY_TOPIC: &str = "ros_discovery_info";
const DISCOVERY_TYPE: &str = "rmw_dds_common::msg::dds_::ParticipantEntitiesInfo_";

/// A node identity `(namespace, name)`.
type NodeId = (alloc::string::String, alloc::string::String);
/// A participant's node list.
type NodeList = alloc::vec::Vec<NodeId>;
/// Remote participants' nodes, keyed by participant gid.
type RemoteMap = alloc::collections::BTreeMap<[u8; 24], NodeList>;
/// A 16-byte endpoint GUID (participant prefix ++ entity id).
type Gid16 = [u8; 16];
/// Decoded `ParticipantEntitiesInfo`: participant gid, its nodes, and each
/// endpoint GUID mapped to its owning node.
type ParticipantInfo = ([u8; 24], NodeList, alloc::vec::Vec<(Gid16, NodeId)>);

/// A local endpoint owned by one of this participant's nodes. Tracked so
/// `ParticipantEntitiesInfo` can carry per-node reader/writer gid sequences,
/// which lets remote peers map an endpoint back to the exact owning node
/// (`rmw_get_publishers/subscriptions_info_by_topic`, `ros2 topic info -v`).
struct LocalEp {
    node: NodeId,
    /// `true` = writer (publication), `false` = reader (subscription).
    writer: bool,
    gid: Gid16,
}

/// Per-context graph state. Shared (Arc) with the discovery reader's listener.
struct NodeGraph {
    /// This participant's 24-byte gid: real DDS participant GUID (first 16 bytes)
    /// zero-padded to 24. The first 12 bytes are the RTPS prefix shared by every
    /// endpoint this participant owns.
    gid: [u8; 24],
    /// Local nodes `(namespace, name)`.
    local: Mutex<NodeList>,
    /// Remote participants' nodes, keyed by participant gid.
    remote: Mutex<RemoteMap>,
    /// Local endpoints (per-node) for endpoint-info resolution + discovery seqs.
    local_eps: Mutex<alloc::vec::Vec<LocalEp>>,
    /// Remote endpoint GUID (16 bytes) -> owning node `(ns, name)`, rebuilt from
    /// the per-node gid sequences in discovery samples.
    remote_eps: Mutex<alloc::collections::BTreeMap<Gid16, NodeId>>,
    /// `ros_discovery_info` writer (raw zerodds writer pointer).
    writer: Mutex<*mut zerodds::ZeroDdsWriter>,
}
// SAFETY: the raw writer pointer is only used behind the mutex; the c-api writer
// is itself Send/Sync-safe to call from any thread.
unsafe impl Send for NodeGraph {}
// SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
unsafe impl Sync for NodeGraph {}

// -- minimal XCDR1-LE writer/reader (origin after the 4-byte encap header) ----

fn cdr_align(buf: &mut alloc::vec::Vec<u8>, a: usize) {
    while (buf.len() - 4) % a != 0 {
        buf.push(0);
    }
}
fn cdr_put_u32(buf: &mut alloc::vec::Vec<u8>, v: u32) {
    cdr_align(buf, 4);
    buf.extend_from_slice(&v.to_le_bytes());
}
fn cdr_put_str(buf: &mut alloc::vec::Vec<u8>, s: &str) {
    cdr_align(buf, 4);
    let n = s.len() as u32 + 1; // length includes the trailing NUL
    buf.extend_from_slice(&n.to_le_bytes());
    buf.extend_from_slice(s.as_bytes());
    buf.push(0);
}

/// Pushes a `Gid.data[24]` (16-byte endpoint GUID zero-padded to 24, octet
/// array, no alignment).
fn cdr_put_gid24(buf: &mut alloc::vec::Vec<u8>, gid16: &Gid16) {
    buf.extend_from_slice(gid16);
    buf.extend_from_slice(&[0u8; 8]);
}

/// Encodes a `ParticipantEntitiesInfo` for `gid` + `nodes`, carrying each node's
/// reader (subscription) and writer (publication) gid sequences from `eps`, so
/// remote peers can resolve an endpoint GUID to the exact owning node.
fn encode_participant_info(
    gid: &[u8; 24],
    nodes: &[NodeId],
    eps: &[LocalEp],
) -> alloc::vec::Vec<u8> {
    let mut b = alloc::vec![0x00u8, 0x01, 0x00, 0x00]; // CDR_LE encapsulation
    b.extend_from_slice(gid); // Gid.data[24] (octet array, no alignment)
    cdr_put_u32(&mut b, nodes.len() as u32); // node_entities_info_seq count
    for (ns, name) in nodes {
        cdr_put_str(&mut b, ns); // node_namespace
        cdr_put_str(&mut b, name); // node_name
        let readers: alloc::vec::Vec<&Gid16> = eps
            .iter()
            .filter(|e| !e.writer && e.node.0 == *ns && e.node.1 == *name)
            .map(|e| &e.gid)
            .collect();
        cdr_put_u32(&mut b, readers.len() as u32); // reader_gid_seq count
        for g in &readers {
            cdr_put_gid24(&mut b, g);
        }
        let writers: alloc::vec::Vec<&Gid16> = eps
            .iter()
            .filter(|e| e.writer && e.node.0 == *ns && e.node.1 == *name)
            .map(|e| &e.gid)
            .collect();
        cdr_put_u32(&mut b, writers.len() as u32); // writer_gid_seq count
        for g in &writers {
            cdr_put_gid24(&mut b, g);
        }
    }
    b
}

struct CdrReader<'a> {
    buf: &'a [u8],
    pos: usize,
}
impl<'a> CdrReader<'a> {
    fn align(&mut self, a: usize) {
        while (self.pos - 4) % a != 0 {
            self.pos += 1;
        }
    }
    fn u32(&mut self) -> Option<u32> {
        self.align(4);
        let e = self.pos + 4;
        let v = self.buf.get(self.pos..e)?;
        self.pos = e;
        Some(u32::from_le_bytes([v[0], v[1], v[2], v[3]]))
    }
    fn gid(&mut self) -> Option<[u8; 24]> {
        let e = self.pos + 24;
        let v = self.buf.get(self.pos..e)?;
        self.pos = e;
        let mut g = [0u8; 24];
        g.copy_from_slice(v);
        Some(g)
    }
    fn string(&mut self) -> Option<alloc::string::String> {
        let n = self.u32()? as usize;
        if n == 0 {
            return Some(alloc::string::String::new());
        }
        let e = self.pos + n;
        let v = self.buf.get(self.pos..e)?;
        self.pos = e;
        // Drop the trailing NUL.
        Some(alloc::string::String::from_utf8_lossy(&v[..n - 1]).into_owned())
    }
}

/// Decodes a `ParticipantEntitiesInfo` body (incl. the 4-byte encap header) into
/// `(participant_gid, nodes, endpoint_gid -> node)`. The endpoint map carries the
/// first 16 bytes of each reader/writer gid, mapped to its owning node — this is
/// what backs remote endpoint-info resolution. Returns `None` on a malformed
/// buffer.
fn decode_participant_info(body: &[u8]) -> Option<ParticipantInfo> {
    if body.len() < 4 {
        return None;
    }
    let mut r = CdrReader { buf: body, pos: 4 };
    let gid = r.gid()?;
    let count = r.u32()? as usize;
    let mut nodes = alloc::vec::Vec::with_capacity(count.min(4096));
    let mut eps: alloc::vec::Vec<(Gid16, NodeId)> = alloc::vec::Vec::new();
    for _ in 0..count {
        let ns = r.string()?;
        let name = r.string()?;
        let nr = r.u32()? as usize; // reader_gid_seq
        for _ in 0..nr {
            let g = r.gid()?; // 24-byte Gid; first 16 = endpoint GUID
            let mut g16 = [0u8; 16];
            g16.copy_from_slice(&g[..16]);
            eps.push((g16, (ns.clone(), name.clone())));
        }
        let nw = r.u32()? as usize; // writer_gid_seq
        for _ in 0..nw {
            let g = r.gid()?;
            let mut g16 = [0u8; 16];
            g16.copy_from_slice(&g[..16]);
            eps.push((g16, (ns.clone(), name.clone())));
        }
        nodes.push((ns, name));
    }
    Some((gid, nodes, eps))
}

/// Data callback on the discovery reader: decode an incoming
/// `ParticipantEntitiesInfo` and update the remote node map.
extern "C" fn discovery_on_data(
    ud: *mut c_void,
    payload: *const u8,
    len: usize,
    repr: u8,
    big_endian: u8,
) {
    if ud.is_null() || payload.is_null() || len == 0 {
        return;
    }
    // SAFETY: `ud` is a live `Arc<NodeGraph>` raw pointer (reclaimed only after
    // the discovery reader is destroyed); `payload`/`len` is the CDR body.
    let graph = unsafe { &*(ud as *const NodeGraph) };
    // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
    let body = unsafe { core::slice::from_raw_parts(payload, len) };
    // The runtime delivers the body; the decoder reads a CDR stream with its
    // encapsulation header, the form the encoder builds.
    let mut stream = Vec::with_capacity(len + 4);
    stream.extend_from_slice(&encapsulation_header(repr, big_endian));
    stream.extend_from_slice(body);
    if let Some((gid, nodes, eps)) = decode_participant_info(&stream) {
        if gid == graph.gid {
            return; // ignore our own announcement echoed back
        }
        if let Ok(mut m) = graph.remote.lock() {
            m.insert(gid, nodes);
        }
        // Refresh this participant's endpoint→node entries: drop the old ones
        // (same 12-byte prefix) then insert the snapshot's. ParticipantEntitiesInfo
        // is a full snapshot, so this keeps the map free of vanished endpoints.
        if let Ok(mut em) = graph.remote_eps.lock() {
            let prefix = &gid[..12];
            em.retain(|k, _| &k[..12] != prefix);
            for (g16, node) in eps {
                em.insert(g16, node);
            }
        }
    }
}

/// Re-publishes this participant's current `ParticipantEntitiesInfo`.
fn graph_publish(graph: &NodeGraph) {
    let nodes = match graph.local.lock() {
        Ok(g) => g.clone(),
        Err(_) => return,
    };
    let eps: alloc::vec::Vec<LocalEp> = match graph.local_eps.lock() {
        Ok(e) => e
            .iter()
            .map(|e| LocalEp {
                node: e.node.clone(),
                writer: e.writer,
                gid: e.gid,
            })
            .collect(),
        Err(_) => alloc::vec::Vec::new(),
    };
    let stream = encode_participant_info(&graph.gid, &nodes, &eps);
    // The writer stamps its own encapsulation header over the body.
    let body = &stream[4..];
    if let Ok(w) = graph.writer.lock() {
        if !w.is_null() {
            // SAFETY: writer from zerodds_writer_create_qos; body lives for the call.
            unsafe { zerodds::zerodds_writer_write(*w, body.as_ptr(), body.len()) };
        }
    }
}

/// Registers a newly created local endpoint with its owning node, then
/// re-announces `ParticipantEntitiesInfo` so peers learn the endpoint→node link.
fn graph_register_endpoint(graph: &NodeGraph, node: &NodeId, writer: bool, gid: Gid16) {
    if let Ok(mut e) = graph.local_eps.lock() {
        e.push(LocalEp {
            node: node.clone(),
            writer,
            gid,
        });
    }
    graph_publish(graph);
}

/// Removes a local endpoint by GUID (on publisher/subscription destroy) and
/// re-announces.
fn graph_unregister_endpoint(graph: &NodeGraph, gid: &Gid16) {
    if let Ok(mut e) = graph.local_eps.lock() {
        e.retain(|x| x.gid != *gid);
    }
    graph_publish(graph);
}

// ============================================================================
// Implementation identifier
// ============================================================================

/// `rmw_get_implementation_identifier()` — REP-2007 §3.
/// Returns a static NUL-terminated string "rmw_zerodds_cpp".
///
/// # Safety
/// The pointer is `'static` and must not be freed.
#[unsafe(no_mangle)]
pub extern "C" fn rmw_zerodds_get_implementation_identifier() -> *const c_char {
    static IDENT: &[u8] = b"rmw_zerodds_cpp\0";
    IDENT.as_ptr().cast()
}

/// `rmw_get_serialization_format()` — fixed `"cdr"` (XCDR1).
///
/// # Safety
/// The pointer is `'static`.
#[unsafe(no_mangle)]
pub extern "C" fn rmw_zerodds_get_serialization_format() -> *const c_char {
    static FMT: &[u8] = b"cdr\0";
    FMT.as_ptr().cast()
}

// ============================================================================
// Init / Shutdown
// ============================================================================

/// Opaque handle: rmw_zerodds context (1:1 to a domain-participant
/// init). The domain is `rmw_init`'s `options->domain_id`.
pub struct RmwZerodsContext {
    domain_id: u32,
    runtime: *mut c_void, // ZeroDdsRuntime from zerodds.h
    /// Shared wakeup edge for this context's subscriptions / guard conditions.
    notify: Arc<WaitNotify>,
    /// Node graph state (local + remote nodes) for `rmw_get_node_names`.
    graph: Arc<NodeGraph>,
    /// `ros_discovery_info` reader; its listener feeds `graph.remote`.
    discovery_reader: *mut zerodds::ZeroDdsReader,
    /// `Arc<NodeGraph>` raw pointer handed to the discovery reader callback.
    discovery_cb_ud: *const NodeGraph,
    /// The `ros2.*` settings of the runtime configuration: topic mangling,
    /// QoS profiles, per-topic overrides.
    ros2: ros2_config::Ros2Config,
    /// `Arc<WaitNotify>` raw pointer handed to the runtime's status callback,
    /// so a QoS event wakes `rmw_wait`; reclaimed after the runtime is gone.
    status_cb_ud: *const WaitNotify,
    /// Calls the QoS event callbacks (`rmw_event_set_callback`).
    events: Arc<events::EventDispatcher>,
    /// C2(6) — the opt-in `ros2.spatial_bridge` content-translation mode
    /// (`spatial_bridge.rs`). `None` when not configured/enabled, or when
    /// it failed to start (logged at init, never a silent partial daemon).
    spatial_bridge: Option<spatial_bridge::SpatialBridge>,
}

impl Drop for RmwZerodsContext {
    fn drop(&mut self) {
        // The spatial bridge is its own participant + threads; drop it
        // before the runtime it shares the domain with is torn down.
        self.spatial_bridge = None;
        // The event thread reads endpoint status; it stops before anything
        // it could read goes.
        self.events.shutdown();
        // Destroy the discovery reader FIRST (stops its listener), then the
        // writer, then reclaim the callback's Arc<NodeGraph> ref.
        if !self.discovery_reader.is_null() {
            // SAFETY: reader from zerodds_reader_create.
            unsafe { zerodds::zerodds_reader_destroy(self.discovery_reader) };
            self.discovery_reader = ptr::null_mut();
        }
        if let Ok(w) = self.graph.writer.lock() {
            if !w.is_null() {
                // SAFETY: writer from zerodds_writer_create.
                unsafe { zerodds::zerodds_writer_destroy(*w) };
            }
        }
        if !self.discovery_cb_ud.is_null() {
            // SAFETY: discovery_cb_ud is the Arc::into_raw pointer from init.
            drop(unsafe { Arc::from_raw(self.discovery_cb_ud) });
            self.discovery_cb_ud = ptr::null();
        }
        if !self.runtime.is_null() {
            // SAFETY: runtime comes from zerodds_runtime_create.
            unsafe {
                zerodds::zerodds_runtime_destroy(self.runtime as *mut zerodds::ZeroDdsRuntime);
            }
            self.runtime = ptr::null_mut();
        }
        if !self.status_cb_ud.is_null() {
            // SAFETY: the Arc::into_raw pointer from init; the runtime that
            // called it is destroyed above.
            drop(unsafe { Arc::from_raw(self.status_cb_ud) });
            self.status_cb_ud = ptr::null();
        }
    }
}

/// The runtime's status callback: a QoS status of some endpoint moved, so a
/// waiting `rmw_wait` re-reads its events.
extern "C" fn status_on_change(user_data: *mut c_void) {
    // SAFETY: `user_data` is the `Arc<WaitNotify>` pointer registered in init,
    // alive until the context is dropped after the runtime.
    let notify = unsafe { &*(user_data as *const WaitNotify) };
    notify.notify();
}

/// Builds a synthetic, process-unique 24-byte participant gid.
/// Builds the 24-byte ROS 2 participant gid from the runtime's real DDS
/// participant GUID (16 bytes: 12-byte RTPS prefix + ENTITYID_PARTICIPANT),
/// zero-padded to 24. Using the real GUID — not a synthetic id — is what lets
/// `rmw_get_publishers/subscriptions_info_by_topic` map an endpoint's GUID
/// prefix back to the owning node: every endpoint GUID owned by this participant
/// shares the same first 12 bytes. The node-name aggregation stays self-
/// consistent (we publish and match the same gid form).
fn make_participant_gid(rt: *mut zerodds::ZeroDdsRuntime) -> [u8; 24] {
    let mut g = [0u8; 24];
    let mut guid16 = [0u8; 16];
    // SAFETY: rt is the runtime handle from create; out_guid points at 16 bytes.
    let rc = unsafe { zerodds::zerodds_runtime_participant_guid(rt, guid16.as_mut_ptr()) };
    if rc == 0 {
        g[..16].copy_from_slice(&guid16);
    }
    g
}

/// `rmw_init(domain_id) -> *mut RmwZerodsContext`.
/// In current distros it takes a single domain-id parameter.
///
/// # Safety
/// The caller must call `rmw_zerodds_shutdown` with the returned handle,
/// otherwise leak.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_init(domain_id: u32) -> *mut RmwZerodsContext {
    // SAFETY: NULL root path is the "no rcl security" case.
    unsafe { rmw_zerodds_init_with_security(domain_id, ptr::null(), 0) }
}

/// `rmw_init` with the security options rcl resolved
/// (`rmw_security_options_t`): `security_root_path` is the enclave directory
/// rcl found under `ROS_SECURITY_KEYSTORE` for the node's enclave (NULL
/// without `ROS_SECURITY_ENABLE=true`), `enforce` is
/// `ROS_SECURITY_STRATEGY=Enforce`. The RMW used to ignore both: a node
/// started with ROS security switched on ran plaintext unless
/// `security.enclave_dir` was also set.
///
/// # Safety
/// `security_root_path` is NULL or a NUL-terminated string; the caller
/// calls `rmw_zerodds_shutdown` with the returned handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_init_with_security(
    domain_id: u32,
    security_root_path: *const core::ffi::c_char,
    enforce: core::ffi::c_int,
) -> *mut RmwZerodsContext {
    let rcl_enclave = if security_root_path.is_null() {
        None
    } else {
        // SAFETY: non-NULL and NUL-terminated by the caller's contract.
        let s = unsafe { core::ffi::CStr::from_ptr(security_root_path) };
        Some(std::path::PathBuf::from(s.to_string_lossy().into_owned()))
    };
    // The layers of this process: environment, files (`--config` on the
    // node's command line, else the system directory), `--set`. The runtime,
    // the enclave and `ros2.*` all come from them; a refused setting is a
    // failed init, as an invalid runtime setting is.
    let loaded = ros2_config::process_source().and_then(|source| {
        let ros2 =
            ros2_config::Ros2Config::from_config(source.tree()).map_err(|e| e.to_string())?;
        let spatial_bridge_cfg = spatial_bridge::SpatialBridgeConfig::from_config(source.tree())
            .map_err(|e| e.to_string())?;
        Ok((source, ros2, spatial_bridge_cfg))
    });
    let (source, ros2, spatial_bridge_cfg) = match loaded {
        Ok(c) => c,
        Err(e) => {
            #[allow(clippy::print_stderr)]
            {
                eprintln!("rmw_zerodds_init: invalid configuration: {e}");
            }
            return ptr::null_mut();
        }
    };
    // C7 "secure by default": with the `security` feature, if the runtime
    // configuration names an SROS2 enclave (`security.enclave_dir`, env
    // `ZERODDS_SECURITY_DIR`, a file or `--set`), a DDS-Security participant
    // from it; otherwise (or without the feature) the plain participant with
    // the ROS 2 profile (the reader offers XCDR1+XCDR2 so it matches
    // rmw_cyclonedds/rmw_fastrtps XCDR1 writers). A named enclave that fails
    // is a hard error (NULL): security was requested and must not silently
    // downgrade.
    #[cfg(feature = "security")]
    let secured = zerodds::security_ffi::runtime_create_secure_ros_from_source(
        domain_id,
        &source,
        rcl_enclave.as_deref(),
    )
    .and_then(|rt| match rt {
        // ROS_SECURITY_STRATEGY=Enforce needs security; rcl already refuses
        // an enforced node without an enclave, this keeps the RMW from
        // being the place where it is lost.
        None if enforce != 0 => Err(String::from(
            "ROS_SECURITY_STRATEGY=Enforce, but no enclave: neither rcl's \
             security_root_path nor security.enclave_dir",
        )),
        other => Ok(other),
    });
    #[cfg(not(feature = "security"))]
    let secured: Result<Option<*mut zerodds::ZeroDdsRuntime>, String> =
        if rcl_enclave.is_some() || enforce != 0 {
            Err(String::from(
                "ROS security is enabled (rcl passed an enclave or Enforce), but this RMW is \
                 built without the `security` feature",
            ))
        } else {
            Ok(None)
        };
    let rt = match secured.and_then(|rt| match rt {
        Some(rt) => Ok(rt),
        None => zerodds::runtime_create_ros_from_source(domain_id, &source),
    }) {
        Ok(rt) => rt,
        Err(e) => {
            #[allow(clippy::print_stderr)]
            {
                eprintln!("rmw_zerodds_init: {e}");
            }
            return ptr::null_mut();
        }
    };
    if rt.is_null() {
        return ptr::null_mut();
    }
    // C2(6) — the opt-in ros2.spatial_bridge content-translation mode: its
    // own participant on the same domain. A malformed mapping already
    // failed init above (the same `loaded` chain that validates `ros2.*`);
    // a runtime failure to start it here (e.g. the extra participant's
    // sockets do not bind) degrades the same way a discovery-hookup
    // failure below does — logged loudly, the node still starts without it,
    // never a silent no-op.
    let spatial_bridge = if spatial_bridge_cfg.enabled {
        let sink = zerodds_dcps::runtime::observability_sink_from_source(&source);
        #[allow(clippy::cast_possible_wrap)]
        match spatial_bridge::SpatialBridge::start(domain_id as i32, &spatial_bridge_cfg, sink) {
            Ok(bridge) => bridge,
            Err(e) => {
                #[allow(clippy::print_stderr)]
                {
                    eprintln!("rmw_zerodds_init: ros2.spatial_bridge failed to start: {e}");
                }
                None
            }
        }
    } else {
        None
    };
    // Node-graph discovery: a ros_discovery_info writer + reader on this runtime.
    // The reader's listener decodes remote ParticipantEntitiesInfo into the graph.
    let graph = Arc::new(NodeGraph {
        gid: make_participant_gid(rt),
        local: Mutex::new(alloc::vec::Vec::new()),
        remote: Mutex::new(alloc::collections::BTreeMap::new()),
        local_eps: Mutex::new(alloc::vec::Vec::new()),
        remote_eps: Mutex::new(alloc::collections::BTreeMap::new()),
        writer: Mutex::new(ptr::null_mut()),
    });
    let topic_c = std::ffi::CString::new(DISCOVERY_TOPIC).unwrap_or_default();
    let type_c = std::ffi::CString::new(DISCOVERY_TYPE).unwrap_or_default();
    // `ros_discovery_info` as rmw_dds_common has every middleware create it:
    // NoKey, RELIABLE, TRANSIENT_LOCAL, KEEP_LAST(1). A VOLATILE WithKey
    // writer matched no other middleware's reader, so their graphs showed
    // this RMW's endpoints under `_CREATED_BY_BARE_DDS_APP_`.
    let discovery_qos = zerodds::ZeroDdsEndpointQos {
        reliable: 1,
        transient_local: 1,
        history_keep_all: 0,
        history_depth: 1,
        ..zerodds::zerodds_endpoint_qos_default()
    };
    // SAFETY: runtime valid; NUL-terminated strings; the QoS lives across the call.
    let dwriter = unsafe {
        zerodds::zerodds_writer_create_qos(rt, topic_c.as_ptr(), type_c.as_ptr(), 0, &discovery_qos)
    };
    if !dwriter.is_null() {
        if let Ok(mut w) = graph.writer.lock() {
            *w = dwriter;
        }
    }
    // KEEP_ALL on the reader: the data callback sees every announcement.
    let reader_qos = zerodds::ZeroDdsEndpointQos {
        history_keep_all: 1,
        ..discovery_qos
    };
    // SAFETY: runtime valid; NUL-terminated strings; the QoS lives across the call.
    let dreader = unsafe {
        zerodds::zerodds_reader_create_qos(rt, topic_c.as_ptr(), type_c.as_ptr(), 0, &reader_qos)
    };
    let discovery_cb_ud = if dreader.is_null() {
        ptr::null()
    } else {
        let ud = Arc::into_raw(graph.clone());
        // SAFETY: reader valid; callback + user_data outlive the reader (drop
        // tears the reader down before reclaiming the Arc).
        let set = unsafe {
            zerodds::zerodds_reader_set_data_callback(
                dreader,
                Some(discovery_on_data),
                ud as *mut c_void,
            )
        };
        if set == 0 {
            ud
        } else {
            // SAFETY: reclaim the extra ref on registration failure.
            drop(unsafe { Arc::from_raw(ud) });
            ptr::null()
        }
    };
    let notify = WaitNotify::new();
    let status_cb_ud = Arc::into_raw(notify.clone());
    // SAFETY: rt valid; the callback and its user_data outlive the runtime
    // (drop destroys the runtime before reclaiming the Arc).
    let registered = unsafe {
        zerodds::zerodds_runtime_add_status_callback(
            rt,
            Some(status_on_change),
            status_cb_ud as *mut c_void,
        )
    };
    let status_cb_ud = if registered == 0 {
        status_cb_ud
    } else {
        // SAFETY: reclaim the ref the runtime did not take.
        drop(unsafe { Arc::from_raw(status_cb_ud) });
        ptr::null()
    };
    let events = events::EventDispatcher::new(notify.clone());
    Box::into_raw(Box::new(RmwZerodsContext {
        domain_id,
        runtime: rt as *mut c_void,
        notify,
        events,
        graph,
        discovery_reader: dreader,
        discovery_cb_ud,
        ros2,
        status_cb_ud,
        spatial_bridge,
    }))
}

/// `rmw_shutdown(*mut Context)` — logical shutdown only. NULL-safe.
///
/// Per the rmw contract `rmw_shutdown` must NOT deallocate the context: it
/// only signals that no new entities may be created. The runtime, discovery
/// writer/reader and the context struct stay alive so that entities created
/// from this context (nodes, publishers, …) can still be torn down afterwards
/// — `rclcpp::shutdown()` is routinely called while nodes are still in scope,
/// and their later destruction reaches back into the context (e.g.
/// `destroy_node` → `graph_publish` over the discovery writer). The actual
/// free happens in [`rmw_zerodds_context_fini`].
///
/// # Safety
/// `ctx` must come from `rmw_zerodds_init` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_shutdown(ctx: *mut RmwZerodsContext) -> i32 {
    if ctx.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // No teardown here — see the doc comment. The handle stays valid until
    // `rmw_zerodds_context_fini`.
    RMW_RET_OK
}

/// `rmw_context_fini(*mut Context)` — frees the runtime + context. NULL-safe.
///
/// Only valid after `rmw_zerodds_shutdown` and after every entity created from
/// the context has been destroyed (the rcl/rclcpp layer guarantees this
/// ordering). Reclaiming the `Box` runs [`RmwZerodsContext`]'s `Drop`, which
/// destroys the discovery reader/writer and the underlying runtime.
///
/// # Safety
/// `ctx` must come from `rmw_zerodds_init` or be NULL, and must not be used
/// afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_context_fini(ctx: *mut RmwZerodsContext) -> i32 {
    if ctx.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: the Box reclaim + Drop takes care of the runtime + discovery.
    let _ = unsafe { Box::from_raw(ctx) };
    RMW_RET_OK
}

/// Callback for node enumeration: `(user_data, node_name, node_namespace)`.
pub type RmwNodeCallback = extern "C" fn(*mut c_void, *const c_char, *const c_char);

/// Registers a local node `(namespace, name)` on `ctx` and re-announces this
/// participant's `ParticipantEntitiesInfo` on `ros_discovery_info`.
///
/// # Safety
/// `ctx` from `rmw_zerodds_init`; `name`/`namespace_` NUL-terminated or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_node_add(
    ctx: *mut RmwZerodsContext,
    name: *const c_char,
    namespace_: *const c_char,
) -> i32 {
    if ctx.is_null() || name.is_null() || namespace_.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked NUL-terminated C strings.
    let (nm, ns) = unsafe {
        (
            CStr::from_ptr(name).to_string_lossy().into_owned(),
            CStr::from_ptr(namespace_).to_string_lossy().into_owned(),
        )
    };
    // SAFETY: ctx valid.
    let graph = &unsafe { &*ctx }.graph;
    if let Ok(mut g) = graph.local.lock() {
        if !g.iter().any(|(a, b)| *a == ns && *b == nm) {
            g.push((ns, nm));
        }
    }
    graph_publish(graph);
    RMW_RET_OK
}

/// Removes a local node `(namespace, name)` and re-announces.
///
/// # Safety
/// As [`rmw_zerodds_node_add`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_node_remove(
    ctx: *mut RmwZerodsContext,
    name: *const c_char,
    namespace_: *const c_char,
) -> i32 {
    if ctx.is_null() || name.is_null() || namespace_.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked NUL-terminated C strings.
    let (nm, ns) = unsafe {
        (
            CStr::from_ptr(name).to_string_lossy().into_owned(),
            CStr::from_ptr(namespace_).to_string_lossy().into_owned(),
        )
    };
    // SAFETY: ctx valid.
    let graph = &unsafe { &*ctx }.graph;
    if let Ok(mut g) = graph.local.lock() {
        g.retain(|(a, b)| !(*a == ns && *b == nm));
    }
    graph_publish(graph);
    RMW_RET_OK
}

/// Invokes `callback(ud, name, namespace)` for every known node — this
/// participant's local nodes plus every remote participant's nodes.
///
/// # Safety
/// `ctx` from `rmw_zerodds_init` or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_for_each_node(
    ctx: *mut RmwZerodsContext,
    callback: Option<RmwNodeCallback>,
    user_data: *mut c_void,
) -> i32 {
    if ctx.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    let Some(cb) = callback else {
        return RMW_RET_INVALID_ARGUMENT;
    };
    // SAFETY: ctx valid.
    let graph = &unsafe { &*ctx }.graph;
    let mut all: NodeList = alloc::vec::Vec::new();
    if let Ok(g) = graph.local.lock() {
        all.extend(g.iter().cloned());
    }
    if let Ok(m) = graph.remote.lock() {
        for nodes in m.values() {
            all.extend(nodes.iter().cloned());
        }
    }
    for (ns, nm) in all {
        if let (Ok(n), Ok(s)) = (std::ffi::CString::new(nm), std::ffi::CString::new(ns)) {
            cb(user_data, n.as_ptr(), s.as_ptr());
        }
    }
    RMW_RET_OK
}

// ============================================================================
// Node
// ============================================================================

/// Opaque node handle.
pub struct RmwZerodsNode {
    /// Owning context — we borrow the runtime through it.
    ctx: *mut RmwZerodsContext,
    /// Logical node identity (name + namespace + domain).
    pub identity: zerodds_ros2_rmw::ffi_api::RmwNode,
}

/// `rmw_create_node(ctx, name, namespace_)` — REP-2007 §5.1.
///
/// # Safety
/// `ctx` must be live; `name`/`namespace_` NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_node(
    ctx: *mut RmwZerodsContext,
    name: *const c_char,
    namespace_: *const c_char,
) -> *mut RmwZerodsNode {
    if ctx.is_null() || name.is_null() || namespace_.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: ctx NULL-checked.
    let cref = unsafe { &*ctx };
    // SAFETY: NUL-terminated C strings required by the caller contract.
    let n = match unsafe { CStr::from_ptr(name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: NUL-terminated C-string, NULL-checked above.
    let ns = match unsafe { CStr::from_ptr(namespace_) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // Register the node in the graph + announce it on ros_discovery_info.
    if let Ok(mut g) = cref.graph.local.lock() {
        if !g.iter().any(|(a, b)| *a == ns && *b == n) {
            g.push((ns.clone(), n.clone()));
        }
    }
    graph_publish(&cref.graph);
    Box::into_raw(Box::new(RmwZerodsNode {
        ctx,
        identity: zerodds_ros2_rmw::ffi_api::RmwNode {
            implementation_identifier: RMW_IMPLEMENTATION_IDENTIFIER.into(),
            name: n,
            namespace: ns,
            domain_id: cref.domain_id,
        },
    }))
}

/// `rmw_destroy_node(*mut Node)`. NULL-safe.
///
/// # Safety
/// `node` must come from `rmw_zerodds_create_node` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_node(node: *mut RmwZerodsNode) -> i32 {
    if node.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box from_raw with an owned pointer.
    let n = unsafe { Box::from_raw(node) };
    // De-register from the graph + re-announce (the context may outlive the node).
    if !n.ctx.is_null() {
        // SAFETY: ctx valid while the node lives.
        let graph = &unsafe { &*n.ctx }.graph;
        if let Ok(mut g) = graph.local.lock() {
            g.retain(|(a, b)| !(*a == n.identity.namespace && *b == n.identity.name));
        }
        graph_publish(graph);
    }
    RMW_RET_OK
}

// ============================================================================
// Publisher / Subscription
// ============================================================================

/// Opaque publisher handle. Wraps a ZeroDDS writer.
pub struct RmwZerodsPublisher {
    inner: Mutex<*mut zerodds::ZeroDdsWriter>,
    /// ROS-logical topic name (before mangling).
    pub ros_topic: alloc::string::String,
    /// DDS topic name (after the `rt/` prefix).
    pub dds_topic: alloc::string::String,
    /// Type name from the TypeSupport layer.
    pub type_name: alloc::string::String,
    /// Owning context (for graph endpoint un/registration on destroy).
    ctx: *mut RmwZerodsContext,
    /// This writer's 16-byte endpoint GUID (for graph endpoint tracking).
    gid: Gid16,
    /// Its QoS event callbacks.
    events: Arc<events::EndpointEvents>,
}

/// Background "doorbell" for the raw delivery modes (RawSameHost/Iceoryx): a
/// thread parks on `zerodds_reader_raw_wait` and bumps the context condvar on
/// each raw-data arrival, so `rmw_wait` wakes event-driven instead of polling
/// (the raw sources do not fire the RTPS data listener). Joined on destroy.
struct Doorbell {
    stop: Arc<core::sync::atomic::AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

/// `Send` wrapper for the reader pointer handed to the doorbell thread. Safe
/// because the thread is stopped + joined in `destroy_subscription` *before* the
/// reader is destroyed, so the pointer stays valid for the thread's lifetime.
struct ReaderPtr(*mut zerodds::ZeroDdsReader);
// SAFETY: see `Doorbell` — the reader outlives the thread by construction.
unsafe impl Send for ReaderPtr {}

/// Opaque subscription handle. Wraps a ZeroDDS reader.
pub struct RmwZerodsSubscription {
    inner: Mutex<*mut zerodds::ZeroDdsReader>,
    /// ROS-logical topic name (before mangling).
    pub ros_topic: alloc::string::String,
    /// DDS topic name (after the `rt/` prefix).
    pub dds_topic: alloc::string::String,
    /// Type name from the TypeSupport layer.
    pub type_name: alloc::string::String,
    /// Listener-fed delivery queue + the context wakeup edge.
    inbox: Arc<SubInbox>,
    /// `Arc<SubInbox>` raw pointer handed to the reader data callback as its
    /// `user_data`; reclaimed in `destroy_subscription` after the reader is
    /// torn down.
    cb_userdata: *const SubInbox,
    /// Raw-mode doorbell thread (started lazily once the raw source is enabled).
    doorbell: Mutex<Option<Doorbell>>,
    /// Owning context (for graph endpoint un/registration on destroy).
    ctx: *mut RmwZerodsContext,
    /// This reader's 16-byte endpoint GUID (for graph endpoint tracking).
    gid: Gid16,
    /// Its QoS event callbacks.
    events: Arc<events::EndpointEvents>,
    /// The content filter of `rmw_subscription_set_content_filter`.
    filter: Mutex<Option<SubscriptionFilter>>,
}

/// A subscription's content filter: the parsed expression and its
/// parameters, read as DDS 1.4 Annex B literals.
struct SubscriptionFilter {
    expr: zerodds_sql_filter::Expr,
    params: Vec<zerodds_sql_filter::Value>,
}

/// One field of a ROS message as the C side reads it for a filter: `kind`
/// 0 none (no such field, or one a filter cannot compare), 1 bool (`b`),
/// 2 integer (`i`), 3 floating point (`f`), 4 string (`s`, `slen` bytes,
/// valid for the call).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct RmwZerodsFilterValue {
    /// Which member holds the value.
    pub kind: u32,
    /// Boolean.
    pub b: u8,
    /// Integer.
    pub i: i64,
    /// Floating point.
    pub f: f64,
    /// String bytes.
    pub s: *const c_char,
    /// String length.
    pub slen: usize,
}

/// Reads the field named by the NUL-terminated `path` (`a.b` for nested
/// members) of the message behind `ctx` into `out`.
pub type RmwZerodsFieldGetter =
    extern "C" fn(ctx: *const c_void, path: *const c_char, out: *mut RmwZerodsFilterValue) -> i32;

struct CallbackRow {
    ctx: *const c_void,
    getter: RmwZerodsFieldGetter,
}

impl zerodds_sql_filter::RowAccess for CallbackRow {
    fn get(&self, path: &str) -> Option<zerodds_sql_filter::Value> {
        use zerodds_sql_filter::Value;
        let path = alloc::ffi::CString::new(path).ok()?;
        let mut v = RmwZerodsFilterValue {
            kind: 0,
            b: 0,
            i: 0,
            f: 0.0,
            s: ptr::null(),
            slen: 0,
        };
        if (self.getter)(self.ctx, path.as_ptr(), &mut v) == 0 {
            return None;
        }
        match v.kind {
            1 => Some(Value::Bool(v.b != 0)),
            2 => Some(Value::Int(v.i)),
            3 => Some(Value::Float(v.f)),
            4 if !v.s.is_null() => {
                // SAFETY: `s[0..slen]` valid for the call, per the getter.
                let bytes = unsafe { core::slice::from_raw_parts(v.s.cast::<u8>(), v.slen) };
                Some(Value::String(
                    alloc::string::String::from_utf8_lossy(bytes).into_owned(),
                ))
            }
            _ => None,
        }
    }
}

/// `rmw_create_publisher(node, type_name, topic_name, qos)`. The endpoint is
/// created with the resolved [`RmwZerodsQos`] (see [`qos::resolve`]), which
/// is also written to `actual` for `rmw_publisher_get_actual_qos`.
///
/// # Safety
/// Pointer validity as always; strings NUL-terminated; `qos` points at an
/// [`RmwZerodsQos`]; `actual` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_publisher(
    node: *mut RmwZerodsNode,
    type_name: *const c_char,
    topic_name: *const c_char,
    qos: *const RmwZerodsQos,
    actual: *mut RmwZerodsQos,
) -> *mut RmwZerodsPublisher {
    // SAFETY: the caller's contract, passed on; no type hash.
    unsafe {
        rmw_zerodds_create_publisher_hashed(node, type_name, topic_name, qos, actual, ptr::null())
    }
}

/// The USER_DATA a ROS 2 endpoint announces its type hash in:
/// `typehash=RIHS01_<64 hex digits>;`, the form `rmw_dds_common` writes and
/// reads (`encode_type_hash_for_user_data_qos`). `type_hash` is the hash as
/// `rosidl_stringify_type_hash` writes it, NUL-terminated, or NULL for
/// none (a distribution before Iron has no type hashes). `None` for a hash
/// that is not UTF-8.
///
/// # Safety
/// `type_hash` NULL or NUL-terminated.
pub(crate) unsafe fn type_hash_user_data(type_hash: *const c_char) -> Option<Vec<u8>> {
    if type_hash.is_null() {
        return Some(Vec::new());
    }
    // SAFETY: non-NULL and NUL-terminated by the caller's contract.
    let hash = unsafe { CStr::from_ptr(type_hash) }.to_str().ok()?;
    Some(format!("typehash={hash};").into_bytes())
}

/// [`rmw_zerodds_create_publisher`] that announces the type hash in
/// USER_DATA (`type_hash_user_data`), as every RMW from Iron on does;
/// `ros2 topic info -v` and `rmw_get_publishers_info_by_topic` of any RMW
/// read it from there.
///
/// # Safety
/// As [`rmw_zerodds_create_publisher`]; `type_hash` NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_publisher_hashed(
    node: *mut RmwZerodsNode,
    type_name: *const c_char,
    topic_name: *const c_char,
    qos: *const RmwZerodsQos,
    actual: *mut RmwZerodsQos,
    type_hash: *const c_char,
) -> *mut RmwZerodsPublisher {
    if node.is_null() || type_name.is_null() || topic_name.is_null() || qos.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: the caller's contract.
    let Some(user_data) = (unsafe { type_hash_user_data(type_hash) }) else {
        return ptr::null_mut();
    };

    // SAFETY: node NULL-checked above.
    let n = unsafe { &*node };
    // SAFETY: n.ctx must be initialized by rmw_zerodds_init
    // (caller contract of the pub unsafe fn).
    let ctx = unsafe { &*n.ctx };
    // SAFETY: topic_name NULL-checked above; the caller contract requires
    // a NUL-terminated C string.
    let topic_ros = match unsafe { CStr::from_ptr(topic_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: type_name NULL-checked above; caller contract: NUL-
    // terminated C string.
    let typ = match unsafe { CStr::from_ptr(type_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // ROS-2 topic mangling: rt/<topic> for regular topics.
    // The topic's `ros2.topic_overrides` entry replaces what it names of the
    // request, then the request resolves as in qos::resolve.
    // SAFETY: non-null by the check above, readable by the caller's contract.
    let requested = ctx.ros2.effective_qos(&topic_ros, unsafe { &*qos });
    let resolved = match qos::resolve(&requested, qos::Side::Publisher) {
        Ok(r) => r,
        Err(_) => return ptr::null_mut(),
    };
    if !actual.is_null() {
        // SAFETY: non-null and writable by the caller's contract.
        unsafe { *actual = resolved.actual() };
    }
    let topic_dds = ctx
        .ros2
        .dds_topic(&topic_ros, resolved.avoid_ros_namespace_conventions);
    let dds_topic_c = match std::ffi::CString::new(topic_dds.clone()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let typ_c = match std::ffi::CString::new(typ.clone()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let endpoint_qos = resolved.endpoint();
    // NoKey (RTPS §9.3.1.2 Table 9.1, entity kind 0x03/0x04): ROS 2 messages
    // carry no key, and every other ROS middleware creates NoKey endpoints.
    // The untyped create this replaced made WithKey writers (0x02), whose
    // DATA a NoKey Cyclone or Fast DDS reader does not accept.
    // SAFETY: writer_create_qos NULL-tolerant; the QoS lives across the call.
    let writer = unsafe {
        zerodds::zerodds_writer_create_qos_user_data(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            dds_topic_c.as_ptr(),
            typ_c.as_ptr(),
            0,
            &endpoint_qos,
            user_data.as_ptr(),
            user_data.len(),
        )
    };
    if writer.is_null() {
        return ptr::null_mut();
    }
    // Say FINAL rather than leave the ABI's fallback to speak for us.
    //
    // The rmw entry point takes a type *name*; the layer above resolves a
    // `rosidl_typesupport_introspection_c__MessageMembers`, and that struct
    // carries no extensibility — the C side reads `member_count_`, `size_of_`,
    // `members_`, `type_id_` and `offset_`, and there is no sixth field to
    // read. So there is no value in this path to pass on, and the question is
    // what ROS types actually are.
    //
    // Measured 2026-09-05 against the ROS 2 Humble environment used for the
    // interop runs (`ROS_PREFIX`, 224 generated `.idl` files): **not one**
    // carries `@appendable`, `@mutable` or `@final`, and `rosidl` emits no
    // extensibility at all — the only matches for the word in that tree are
    // Fast DDS's own headers. Every ROS type on that path is therefore plain
    // CDR, which is what FINAL frames.
    //
    // Saying it beats inheriting it: under XCDR1 the ABI fallback framed an
    // `@appendable` body with a `CDR_LE` identifier, and OpenDDS then read the
    // DHEADER as the first member — 24 samples delivered with shifted values
    // rather than refused. A stated FINAL cannot drift that way.
    //
    // `tools/ros-extensibility-scan.sh` repeats the measurement. If a ROS
    // package ever ships hand-written IDL with a non-final type, that scan is
    // where it shows up, and this line is what has to change.
    // SAFETY: writer is non-null (checked above) and came from writer_create.
    let _ = unsafe { zerodds::zerodds_writer_set_wire_extensibility(writer, 0) };
    // Capture the writer GUID + register the endpoint with its owning node so
    // ros_discovery_info carries the endpoint→node link (endpoint-info).
    let mut gid = [0u8; 16];
    // SAFETY: writer valid; out_guid points at 16 bytes.
    unsafe { zerodds::zerodds_writer_guid(writer, gid.as_mut_ptr()) };
    let node_id = (n.identity.namespace.clone(), n.identity.name.clone());
    graph_register_endpoint(&ctx.graph, &node_id, true, gid);
    Box::into_raw(Box::new(RmwZerodsPublisher {
        inner: Mutex::new(writer),
        ros_topic: topic_ros,
        dds_topic: topic_dds,
        type_name: typ,
        ctx: n.ctx,
        gid,
        events: events::EndpointEvents::new(events::Probe::Writer(writer)),
    }))
}

/// The publisher's GID: its DDS writer GUID, the value graph queries
/// (`rmw_get_publishers_info_by_topic`) report for it (16 bytes into `out`).
///
/// # Safety
/// `pub_` from `rmw_zerodds_create_publisher` or NULL; `out` writable for
/// 16 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publisher_gid(
    pub_: *mut RmwZerodsPublisher,
    out: *mut u8,
) -> i32 {
    if pub_.is_null() || out.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: both NULL-checked; `out` holds 16 bytes per the caller.
    unsafe { core::ptr::copy_nonoverlapping((*pub_).gid.as_ptr(), out, 16) };
    RMW_RET_OK
}

/// A unicast locator of an endpoint, for `rmw_*_get_network_flow_endpoints`:
/// `kind` 1 UDPv4, 2 UDPv6, 4 TCPv4, 8 TCPv6, others not IP.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct RmwZerodsLocator {
    /// DDSI-RTPS locator kind.
    pub kind: i32,
    /// Port.
    pub port: u32,
    /// Address bytes; IPv4 in the last four.
    pub address: [u8; 16],
}

fn copy_locators(raw: &[zerodds::ZeroDdsLocator], out: *mut RmwZerodsLocator, capacity: usize) {
    if out.is_null() {
        return;
    }
    for (i, l) in raw.iter().take(capacity).enumerate() {
        // SAFETY: `out` holds `capacity` elements per the caller; i < capacity.
        unsafe {
            *out.add(i) = RmwZerodsLocator {
                kind: l.kind,
                port: l.port,
                address: l.address,
            };
        }
    }
}

/// The unicast locators of a publisher's writer; returns the count, copies
/// at most `capacity` into `out` (NULL to ask for the count).
///
/// # Safety
/// `pub_` from `rmw_zerodds_create_publisher` or NULL; `out` writable for
/// `capacity` elements or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publisher_unicast_locators(
    pub_: *mut RmwZerodsPublisher,
    out: *mut RmwZerodsLocator,
    capacity: usize,
) -> usize {
    if pub_.is_null() {
        return 0;
    }
    // SAFETY: NULL-checked; caller pledge on its origin.
    let p = unsafe { &*pub_ };
    let Ok(w) = p.inner.lock() else {
        return 0;
    };
    // SAFETY: writer from zerodds_writer_create; NULL `out` asks for the count.
    let n = unsafe { zerodds::zerodds_writer_unicast_locators(*w, ptr::null_mut(), 0) };
    let mut raw = alloc::vec![zerodds::ZeroDdsLocator::default(); n];
    // SAFETY: `raw` holds `n` elements.
    unsafe { zerodds::zerodds_writer_unicast_locators(*w, raw.as_mut_ptr(), n) };
    copy_locators(&raw, out, capacity);
    n
}

/// [`rmw_zerodds_publisher_unicast_locators`] for a subscription's reader.
///
/// # Safety
/// As [`rmw_zerodds_publisher_unicast_locators`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_unicast_locators(
    sub: *mut RmwZerodsSubscription,
    out: *mut RmwZerodsLocator,
    capacity: usize,
) -> usize {
    if sub.is_null() {
        return 0;
    }
    // SAFETY: NULL-checked; caller pledge on its origin.
    let s = unsafe { &*sub };
    let Ok(r) = s.inner.lock() else {
        return 0;
    };
    // SAFETY: reader from zerodds_reader_create; NULL `out` asks for the count.
    let n = unsafe { zerodds::zerodds_reader_unicast_locators(*r, ptr::null_mut(), 0) };
    let mut raw = alloc::vec![zerodds::ZeroDdsLocator::default(); n];
    // SAFETY: `raw` holds `n` elements.
    unsafe { zerodds::zerodds_reader_unicast_locators(*r, raw.as_mut_ptr(), n) };
    copy_locators(&raw, out, capacity);
    n
}

/// Number of remote subscriptions currently matched to this publisher — the
/// value behind `rmw_publisher_count_matched_subscriptions` /
/// `Publisher.get_subscription_count()`. `0` on NULL.
///
/// # Safety
/// `pub_` must come from `rmw_zerodds_create_publisher` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publisher_matched_count(
    pub_: *mut RmwZerodsPublisher,
) -> usize {
    if pub_.is_null() {
        return 0;
    }
    // SAFETY: pub_ NULL-checked; caller pledge it came from create_publisher.
    let p = unsafe { &*pub_ };
    let Ok(w) = p.inner.lock() else {
        return 0;
    };
    // SAFETY: writer from zerodds_writer_create; NULL-tolerant.
    unsafe { zerodds::zerodds_writer_matched_count(*w) }
}

/// A publisher's communication status, cumulative since it was created
/// (DDS 1.4 §2.2.4.1). `rmw_take_event` reports the change to the values it
/// read last.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct RmwZerodsPublisherStatus {
    /// OFFERED_DEADLINE_MISSED `total_count`.
    pub offered_deadline_missed_total: u64,
    /// LIVELINESS_LOST `total_count`.
    pub liveliness_lost_total: u64,
    /// OFFERED_INCOMPATIBLE_QOS `total_count`.
    pub offered_incompatible_qos_total: u64,
    /// The DDS QosPolicyId of the last incompatible policy, 0 for none.
    pub offered_incompatible_qos_last_policy_id: u32,
    /// Subscriptions matched now.
    pub matched_current: u64,
    /// Readers of an incompatible type refused on the topic.
    pub incompatible_type_total: u64,
}

/// A subscription's communication status, cumulative since it was created.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct RmwZerodsSubscriptionStatus {
    /// REQUESTED_DEADLINE_MISSED `total_count`.
    pub requested_deadline_missed_total: u64,
    /// Writers seen becoming alive.
    pub liveliness_alive: u64,
    /// Writers seen becoming not alive.
    pub liveliness_not_alive: u64,
    /// REQUESTED_INCOMPATIBLE_QOS `total_count`.
    pub requested_incompatible_qos_total: u64,
    /// The DDS QosPolicyId of the last incompatible policy, 0 for none.
    pub requested_incompatible_qos_last_policy_id: u32,
    /// SAMPLE_LOST `total_count`.
    pub sample_lost_total: u64,
    /// Publishers matched now.
    pub matched_current: u64,
    /// Writers of an incompatible type refused on the topic.
    pub incompatible_type_total: u64,
}

/// The communication status of a publisher's writer, cumulative; the C side
/// turns it into the change `rmw_take_event` reports.
///
/// # Safety
/// `pub_` from `rmw_zerodds_create_publisher` or NULL; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publisher_get_status(
    pub_: *mut RmwZerodsPublisher,
    out: *mut RmwZerodsPublisherStatus,
) -> i32 {
    if pub_.is_null() || out.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: pub_ NULL-checked; caller pledge it came from create_publisher.
    let p = unsafe { &*pub_ };
    let Ok(w) = p.inner.lock() else {
        return RMW_RET_ERROR;
    };
    let mut st = zerodds::ZeroDdsWriterStatus::default();
    // SAFETY: writer from zerodds_writer_create; `st` is a local.
    if unsafe { zerodds::zerodds_writer_get_status(*w, &mut st) } != 0 {
        return RMW_RET_ERROR;
    }
    // SAFETY: out NULL-checked, writable per the caller.
    unsafe {
        *out = RmwZerodsPublisherStatus {
            offered_deadline_missed_total: st.offered_deadline_missed_total,
            liveliness_lost_total: st.liveliness_lost_total,
            offered_incompatible_qos_total: st.offered_incompatible_qos_total,
            offered_incompatible_qos_last_policy_id: st.offered_incompatible_qos_last_policy_id,
            matched_current: st.matched_current,
            incompatible_type_total: st.incompatible_type_total,
        };
    }
    RMW_RET_OK
}

/// The communication status of a subscription's reader, cumulative; the C
/// side turns it into the change `rmw_take_event` reports.
///
/// # Safety
/// `sub` from `rmw_zerodds_create_subscription` or NULL; `out` writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_get_status(
    sub: *mut RmwZerodsSubscription,
    out: *mut RmwZerodsSubscriptionStatus,
) -> i32 {
    if sub.is_null() || out.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: sub NULL-checked; caller pledge it came from create_subscription.
    let s = unsafe { &*sub };
    let Ok(r) = s.inner.lock() else {
        return RMW_RET_ERROR;
    };
    let mut st = zerodds::ZeroDdsReaderStatus::default();
    // SAFETY: reader from zerodds_reader_create; `st` is a local.
    if unsafe { zerodds::zerodds_reader_get_status(*r, &mut st) } != 0 {
        return RMW_RET_ERROR;
    }
    // SAFETY: out NULL-checked, writable per the caller.
    unsafe {
        *out = RmwZerodsSubscriptionStatus {
            requested_deadline_missed_total: st.requested_deadline_missed_total,
            liveliness_alive: st.liveliness_alive,
            liveliness_not_alive: st.liveliness_not_alive,
            requested_incompatible_qos_total: st.requested_incompatible_qos_total,
            requested_incompatible_qos_last_policy_id: st.requested_incompatible_qos_last_policy_id,
            sample_lost_total: st.sample_lost_total,
            matched_current: st.matched_current,
            incompatible_type_total: st.incompatible_type_total,
        };
    }
    RMW_RET_OK
}

/// Number of remote publishers currently matched to this subscription — the
/// value behind `rmw_subscription_count_matched_publishers` /
/// `Subscription.get_publisher_count()`. `0` on NULL.
///
/// # Safety
/// `sub` must come from `rmw_zerodds_create_subscription` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_matched_count(
    sub: *mut RmwZerodsSubscription,
) -> usize {
    if sub.is_null() {
        return 0;
    }
    // SAFETY: sub NULL-checked; caller pledge it came from create_subscription.
    let s = unsafe { &*sub };
    let Ok(r) = s.inner.lock() else {
        return 0;
    };
    // SAFETY: reader from zerodds_reader_create; NULL-tolerant.
    unsafe { zerodds::zerodds_reader_matched_count(*r) }
}

/// `rmw_destroy_publisher(*mut Publisher)`.
///
/// # Safety
/// `pub_` must come from `rmw_zerodds_create_publisher` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_publisher(pub_: *mut RmwZerodsPublisher) -> i32 {
    if pub_.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box-Pointer.
    let p = unsafe { Box::from_raw(pub_) };
    // Unregister the endpoint from the node graph + re-announce.
    if !p.ctx.is_null() {
        // SAFETY: ctx set at create, valid for the publisher's lifetime.
        graph_unregister_endpoint(&unsafe { &*p.ctx }.graph, &p.gid);
    }
    // The event thread stops reading this writer before it goes.
    p.events.close();
    if let Ok(g) = p.inner.lock() {
        // SAFETY: writer comes from zerodds_writer_create + Box owns.
        unsafe { zerodds::zerodds_writer_destroy(*g) };
    }
    RMW_RET_OK
}

/// `rmw_publish(pub, payload, len)` — writes CDR-encoded bytes.
///
/// # Safety
/// `pub_` valid; `payload` of `len` bytes lives during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publish(
    pub_: *mut RmwZerodsPublisher,
    payload: *const u8,
    len: usize,
) -> i32 {
    if pub_.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: pub_ NULL-checked.
    let p = unsafe { &*pub_ };
    let writer = match p.inner.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // The C layer hands a CDR stream with its encapsulation header (the
    // form ROS serialized messages have); the writer stamps its own header
    // over the body, so the stream's is taken off here. It used to go out as
    // body bytes: a second header, which ZeroDDS readers skipped and every
    // other middleware decoded as data (an Int32 5 arrived as 256).
    if payload.is_null() || len < 4 {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: payload non-null, `len` bytes readable by the caller's contract.
    let stream = unsafe { core::slice::from_raw_parts(payload, len) };
    // CDR_LE (0x0001) is what the introspection serializer and a
    // little-endian ROS serialized message carry; another header would be
    // restamped as CDR_LE over a body it does not describe.
    if stream[0..2] != [0x00, 0x01] {
        return RMW_RET_ERROR;
    }
    let body = &stream[4..];
    // SAFETY: body within the caller's buffer; writer from create.
    let rc = unsafe { zerodds::zerodds_writer_write(writer, body.as_ptr(), body.len()) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// The encapsulation header (RTPS 2.5 §10.5) of a body the runtime delivered
/// without one: XCDR1 `CDR_LE`/`CDR_BE`, XCDR2 `PLAIN_CDR2_LE`/`_BE`.
fn encapsulation_header(repr: u8, big_endian: u8) -> [u8; 4] {
    match (repr, big_endian) {
        (0, 0) => [0x00, 0x01, 0x00, 0x00],
        (0, _) => [0x00, 0x00, 0x00, 0x00],
        (_, 0) => [0x00, 0x07, 0x00, 0x00],
        (_, _) => [0x00, 0x06, 0x00, 0x00],
    }
}

/// A2 — the TIME_BASED_FILTER `minimum_separation` (nanoseconds) for a ROS
/// topic, from the runtime's `ros2.time_based_filter` (env
/// `ZERODDS_TIME_BASED_FILTER`, or a file). ROS 2 has no `rmw_qos_profile_t`
/// field for TIME_BASED_FILTER, so this is the vendor-idiomatic config seam
/// (cf. `CYCLONEDDS_URI` / Fast DDS XML). A misspelled value refuses runtime
/// creation instead of silently not filtering.
fn tbf_min_separation_nanos_for(runtime: *mut c_void, topic_ros: &str) -> Option<u64> {
    // SAFETY: the context's runtime, alive for the context's lifetime.
    let config = unsafe { zerodds::runtime_config(runtime as *const zerodds::ZeroDdsRuntime) }?;
    config
        .time_based_filter_for(topic_ros)
        .map(|d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
}

/// The delivery mode the RMW's C layer selects for new publishers and
/// subscriptions: the runtime's `delivery.mode` (env `ZERODDS_DELIVERY_MODE`).
/// 0 Portable, 1 RawSameHost, 2 Iceoryx; 0 for a NULL context.
///
/// # Safety
/// `ctx` must be NULL or come from `rmw_zerodds_init` and still be alive.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_delivery_mode(ctx: *const RmwZerodsContext) -> i32 {
    if ctx.is_null() {
        return 0;
    }
    // SAFETY: non-null and live by the caller's contract.
    let runtime = unsafe { (*ctx).runtime };
    // SAFETY: the context's runtime, alive for the context's lifetime.
    unsafe { zerodds::runtime_config(runtime as *const zerodds::ZeroDdsRuntime) }
        .map_or(0, |config| config.delivery_mode.as_c())
}

/// `rmw_publisher_assert_liveliness`: a MANUAL_BY_TOPIC liveliness assertion
/// (DDS 1.4 §2.2.3.11). The C layer returned OK without asserting.
///
/// # Safety
/// `pub_` from `rmw_zerodds_create_publisher` or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publisher_assert_liveliness(
    pub_: *mut RmwZerodsPublisher,
) -> i32 {
    if pub_.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: non-null by the check above.
    let p = unsafe { &*pub_ };
    let Ok(writer) = p.inner.lock().map(|g| *g) else {
        return RMW_RET_ERROR;
    };
    // SAFETY: writer from writer_create_qos, alive while the publisher is.
    match unsafe { zerodds::zerodds_writer_assert_liveliness(writer) } {
        0 => RMW_RET_OK,
        _ => RMW_RET_ERROR,
    }
}

/// `rmw_publisher_wait_for_all_acked`: blocks until every matched
/// subscription acknowledged every sample, or `timeout_ns` elapses
/// (`RMW_RET_TIMEOUT`). The C layer returned OK without waiting.
///
/// # Safety
/// As [`rmw_zerodds_publisher_assert_liveliness`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publisher_wait_for_all_acked(
    pub_: *mut RmwZerodsPublisher,
    timeout_ns: u64,
) -> i32 {
    if pub_.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: non-null by the check above.
    let p = unsafe { &*pub_ };
    let Ok(writer) = p.inner.lock().map(|g| *g) else {
        return RMW_RET_ERROR;
    };
    // SAFETY: writer from writer_create_qos, alive while the publisher is.
    match unsafe { zerodds::zerodds_writer_wait_for_acknowledgments(writer, timeout_ns) } {
        0 => RMW_RET_OK,
        1 => RMW_RET_TIMEOUT,
        _ => RMW_RET_ERROR,
    }
}

/// `rmw_create_subscription(node, type, topic, qos)`; the QoS as for
/// [`rmw_zerodds_create_publisher`], KEEP_LAST bounding the inbox.
///
/// # Safety
/// Like create_publisher.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_subscription(
    node: *mut RmwZerodsNode,
    type_name: *const c_char,
    topic_name: *const c_char,
    qos: *const RmwZerodsQos,
    actual: *mut RmwZerodsQos,
) -> *mut RmwZerodsSubscription {
    // SAFETY: the caller's contract, passed on; no type hash.
    unsafe {
        rmw_zerodds_create_subscription_hashed(
            node,
            type_name,
            topic_name,
            qos,
            actual,
            ptr::null(),
        )
    }
}

/// [`rmw_zerodds_create_subscription`] that announces the type hash in
/// USER_DATA, as [`rmw_zerodds_create_publisher_hashed`].
///
/// # Safety
/// As [`rmw_zerodds_create_subscription`]; `type_hash` NULL or
/// NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_subscription_hashed(
    node: *mut RmwZerodsNode,
    type_name: *const c_char,
    topic_name: *const c_char,
    qos: *const RmwZerodsQos,
    actual: *mut RmwZerodsQos,
    type_hash: *const c_char,
) -> *mut RmwZerodsSubscription {
    if node.is_null() || type_name.is_null() || topic_name.is_null() || qos.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: the caller's contract.
    let Some(user_data) = (unsafe { type_hash_user_data(type_hash) }) else {
        return ptr::null_mut();
    };

    // SAFETY: node NULL-checked above.
    let n = unsafe { &*node };
    // SAFETY: n.ctx must be initialized by rmw_zerodds_init.
    let ctx = unsafe { &*n.ctx };
    // SAFETY: topic_name NULL-checked above; caller contract: NUL-
    // terminierter C-String.
    let topic_ros = match unsafe { CStr::from_ptr(topic_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: type_name NULL-checked above; caller contract: NUL-
    // terminierter C-String.
    let typ = match unsafe { CStr::from_ptr(type_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // The topic's `ros2.topic_overrides` entry replaces what it names of the
    // request, then the request resolves as in qos::resolve.
    // SAFETY: non-null by the check above, readable by the caller's contract.
    let requested = ctx.ros2.effective_qos(&topic_ros, unsafe { &*qos });
    let resolved = match qos::resolve(&requested, qos::Side::Subscription) {
        Ok(r) => r,
        Err(_) => return ptr::null_mut(),
    };
    if !actual.is_null() {
        // SAFETY: non-null and writable by the caller's contract.
        unsafe { *actual = resolved.actual() };
    }
    let topic_dds = ctx
        .ros2
        .dds_topic(&topic_ros, resolved.avoid_ros_namespace_conventions);
    let dds_topic_c = match std::ffi::CString::new(topic_dds.clone()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let typ_c = match std::ffi::CString::new(typ.clone()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let endpoint_qos = resolved.endpoint();
    // NoKey, as for the publisher.
    // SAFETY: reader_create_qos NULL-tolerant; the QoS lives across the call.
    let reader = unsafe {
        zerodds::zerodds_reader_create_qos_user_data(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            dds_topic_c.as_ptr(),
            typ_c.as_ptr(),
            0,
            &endpoint_qos,
            user_data.as_ptr(),
            user_data.len(),
        )
    };
    if reader.is_null() {
        return ptr::null_mut();
    }
    // Event-driven delivery: route the reader's samples to a data callback that
    // queues them and wakes the context's shared condvar. `user_data` is an
    // extra `Arc<SubInbox>` strong ref, reclaimed in destroy after the reader
    // (and thus the callback) is gone.
    let inbox = Arc::new(SubInbox {
        queue: Mutex::new(alloc::collections::VecDeque::new()),
        notify: ctx.notify.clone(),
        event: Mutex::new(None),
        keep_last: resolved.keep_last.map(|d| d as usize),
    });
    let cb_userdata = Arc::into_raw(inbox.clone());
    // SAFETY: reader from reader_create; callback + user_data outlive the reader.
    let set = unsafe {
        zerodds::zerodds_reader_set_data_callback_ex(
            reader,
            Some(subscription_on_data),
            cb_userdata as *mut c_void,
        )
    };
    if set != 0 {
        // No listener means no delivery path for this subscription — fail hard
        // rather than create a silently-dead subscription.
        // SAFETY: reclaim the extra ref; destroy the reader we just created.
        drop(unsafe { Arc::from_raw(cb_userdata) });
        // SAFETY: reader from reader_create.
        unsafe { zerodds::zerodds_reader_destroy(reader) };
        return ptr::null_mut();
    }
    // A2 — TIME_BASED_FILTER: ROS 2's `rmw_qos_profile_t` has no field for it, so
    // the subscription rate-limit is set per-topic via `ros2.time_based_filter`.
    if let Some(ns) = tbf_min_separation_nanos_for(ctx.runtime, &topic_ros) {
        // SAFETY: reader valid (created above + callback set); NULL-tolerant.
        unsafe { zerodds::zerodds_reader_set_time_based_filter_ns(reader, ns) };
    }
    // Capture the reader GUID + register the endpoint with its owning node.
    let mut gid = [0u8; 16];
    // SAFETY: reader valid; out_guid points at 16 bytes.
    unsafe { zerodds::zerodds_reader_guid(reader, gid.as_mut_ptr()) };
    let node_id = (n.identity.namespace.clone(), n.identity.name.clone());
    graph_register_endpoint(&ctx.graph, &node_id, false, gid);
    Box::into_raw(Box::new(RmwZerodsSubscription {
        inner: Mutex::new(reader),
        ros_topic: topic_ros,
        dds_topic: topic_dds,
        type_name: typ,
        inbox,
        cb_userdata,
        doorbell: Mutex::new(None),
        ctx: n.ctx,
        gid,
        events: events::EndpointEvents::new(events::Probe::Reader(reader)),
        filter: Mutex::new(None),
    }))
}

/// `rmw_destroy_subscription(*mut Subscription)`.
///
/// # Safety
/// `sub` must come from `rmw_zerodds_create_subscription` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_subscription(sub: *mut RmwZerodsSubscription) -> i32 {
    if sub.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box-Pointer.
    let s = unsafe { Box::from_raw(sub) };
    // Unregister the endpoint from the node graph + re-announce.
    if !s.ctx.is_null() {
        // SAFETY: ctx set at create, valid for the subscription's lifetime.
        graph_unregister_endpoint(&unsafe { &*s.ctx }.graph, &s.gid);
    }
    // Stop + join the raw-mode doorbell FIRST — it holds the reader pointer and
    // must not outlive the reader.
    if let Ok(mut g) = s.doorbell.lock() {
        if let Some(mut db) = g.take() {
            db.stop.store(true, core::sync::atomic::Ordering::Relaxed);
            if let Some(j) = db.join.take() {
                let _ = j.join();
            }
        }
    }
    // The event thread stops reading this reader before it goes.
    s.events.close();
    if let Ok(g) = s.inner.lock() {
        // Destroy the reader (after the doorbell is gone) — this also stops the
        // receive thread from firing the data callback, so the callback's
        // `user_data` Arc can be reclaimed race-free below.
        // SAFETY: reader comes from zerodds_reader_create.
        unsafe { zerodds::zerodds_reader_destroy(*g) };
    }
    // Reclaim the data-callback's extra Arc<SubInbox> ref (no callback can fire
    // now that the reader is gone). The queued Box<[u8]> samples drop with it.
    if !s.cb_userdata.is_null() {
        // SAFETY: cb_userdata is the Arc::into_raw pointer from create.
        drop(unsafe { Arc::from_raw(s.cb_userdata) });
    }
    RMW_RET_OK
}

// ============================================================================
// Same-host zero-copy loaning bridge — delivery mode `RawSameHost`
// (`zerodds-delivery-modes-1.0` §3.2/§7). The C ABI layer drives these from the
// loaned-message API once it selects `RawSameHost` (env `ZERODDS_DELIVERY_MODE`)
// for a fixed-POD type; the default `Portable` path never touches them.
// ============================================================================

/// Switches a publisher to `RawSameHost` and creates its POSIX SHM loan segment
/// (`slots` × `cap` bytes at flink path `name`). Afterwards the publisher's loan
/// hands back a pointer into a shared slot and commit delivers same-host only
/// (no RTPS, no serialization).
///
/// # Safety
/// `pub_` from `rmw_zerodds_create_publisher`; `name` a NUL-terminated C string.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publisher_enable_raw_loan(
    pub_: *mut RmwZerodsPublisher,
    name: *const c_char,
    slots: usize,
    cap: usize,
) -> i32 {
    if pub_.is_null() || name.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: pub_ NULL-checked.
    let p = unsafe { &*pub_ };
    let Ok(w) = p.inner.lock() else {
        return RMW_RET_ERROR;
    };
    if w.is_null() {
        return RMW_RET_ERROR;
    }
    // RawSameHost = 1 (`ZeroDdsDeliveryMode`).
    // SAFETY: writer valid; FFI NULL-tolerant.
    if unsafe { zerodds::zerodds_writer_set_delivery_mode(*w, 1) } != 0 {
        return RMW_RET_ERROR;
    }
    // SAFETY: writer valid; `name` is a NUL-terminated C string.
    if unsafe { zerodds::zerodds_writer_enable_shm_loan(*w, name, slots, cap) } != 0 {
        return RMW_RET_ERROR;
    }
    RMW_RET_OK
}

/// Loans a `len`-byte slot from the publisher's SHM segment; the caller writes
/// the message struct directly into `*out_ptr` (zero-copy, zero-serialize).
///
/// # Safety
/// `pub_` valid; `out_ptr` a valid out pointer.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publisher_loan(
    pub_: *mut RmwZerodsPublisher,
    len: usize,
    out_ptr: *mut *mut u8,
) -> i32 {
    if pub_.is_null() || out_ptr.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: pub_ NULL-checked.
    let p = unsafe { &*pub_ };
    let Ok(w) = p.inner.lock() else {
        return RMW_RET_ERROR;
    };
    if w.is_null() {
        return RMW_RET_ERROR;
    }
    let mut got: usize = 0;
    // SAFETY: writer valid; `out_ptr` + `&mut got` are valid out pointers.
    let rc = unsafe { zerodds::zerodds_writer_loan_message(*w, len, out_ptr, &mut got) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// Commits a loaned slot (`ptr`, `len`) — finalizes it in place and delivers
/// same-host (no serialization, no RTPS in `RawSameHost`).
///
/// # Safety
/// `pub_` valid; `ptr`/`len` from a prior `rmw_zerodds_publisher_loan`.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publisher_commit(
    pub_: *mut RmwZerodsPublisher,
    ptr: *mut u8,
    len: usize,
) -> i32 {
    if pub_.is_null() || ptr.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: pub_ NULL-checked.
    let p = unsafe { &*pub_ };
    let Ok(w) = p.inner.lock() else {
        return RMW_RET_ERROR;
    };
    if w.is_null() {
        return RMW_RET_ERROR;
    }
    // SAFETY: writer valid; `ptr`/`len` from a prior loan.
    let rc = unsafe { zerodds::zerodds_writer_commit_loan(*w, ptr, len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// Discards a loaned-but-unpublished slot.
///
/// # Safety
/// `pub_` valid; `ptr`/`len` from a prior `rmw_zerodds_publisher_loan`.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publisher_discard(
    pub_: *mut RmwZerodsPublisher,
    ptr: *mut u8,
    len: usize,
) -> i32 {
    if pub_.is_null() || ptr.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: pub_ NULL-checked.
    let p = unsafe { &*pub_ };
    let Ok(w) = p.inner.lock() else {
        return RMW_RET_ERROR;
    };
    if w.is_null() {
        return RMW_RET_ERROR;
    }
    // SAFETY: writer valid; `ptr`/`len` from a prior loan.
    let rc = unsafe { zerodds::zerodds_writer_discard_loan(*w, ptr, len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// Maps the writer's SHM segment on a subscription's reader for zero-copy takes.
///
/// # Safety
/// `sub` valid; `name` a NUL-terminated C string.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_enable_shm(
    sub: *mut RmwZerodsSubscription,
    name: *const c_char,
    reader_index: u8,
) -> i32 {
    if sub.is_null() || name.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: sub NULL-checked.
    let s = unsafe { &*sub };
    let Ok(r) = s.inner.lock() else {
        return RMW_RET_ERROR;
    };
    if r.is_null() {
        return RMW_RET_ERROR;
    }
    // SAFETY: reader valid; `name` is a NUL-terminated C string.
    let rc = unsafe { zerodds::zerodds_reader_enable_shm(*r, name, reader_index) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// Zero-copy take: returns a read-only pointer into the writer's slot + the slot
/// index (for release). `RMW_RET_OK` with data, `RMW_RET_TIMEOUT` when empty.
///
/// # Safety
/// `sub` valid; `out_ptr`/`out_len`/`out_slot` valid out pointers.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_take_shm(
    sub: *mut RmwZerodsSubscription,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
    out_slot: *mut u32,
) -> i32 {
    if sub.is_null() || out_ptr.is_null() || out_len.is_null() || out_slot.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: sub NULL-checked.
    let s = unsafe { &*sub };
    let Ok(r) = s.inner.lock() else {
        return RMW_RET_ERROR;
    };
    if r.is_null() {
        return RMW_RET_ERROR;
    }
    // SAFETY: reader valid; out pointers valid.
    let rc = unsafe { zerodds::zerodds_reader_take_shm(*r, out_ptr, out_len, out_slot) };
    // `ZeroDdsStatus::Ok` == 0 → data; anything else (NoData/…) → no sample.
    if rc == 0 { RMW_RET_OK } else { RMW_RET_TIMEOUT }
}

/// Non-consuming readiness peek: `RMW_RET_OK` if a sample is available. Uses the
/// idempotent `take_shm` (it does not advance the read cursor until
/// `release_shm`), so this never consumes the sample. Used by `rmw_wait` to
/// report a `RawSameHost` subscription ready.
///
/// # Safety
/// `sub` valid.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_has_shm_data(
    sub: *mut RmwZerodsSubscription,
) -> i32 {
    if sub.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: sub NULL-checked.
    let s = unsafe { &*sub };
    let Ok(r) = s.inner.lock() else {
        return RMW_RET_ERROR;
    };
    if r.is_null() {
        return RMW_RET_ERROR;
    }
    let mut p: *const u8 = ptr::null();
    let mut l: usize = 0;
    let mut slot: u32 = 0;
    // SAFETY: reader valid; locals are valid out pointers.
    let rc = unsafe { zerodds::zerodds_reader_take_shm(*r, &mut p, &mut l, &mut slot) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_TIMEOUT }
}

/// Releases a slot returned by `rmw_zerodds_subscription_take_shm`.
///
/// # Safety
/// `sub` valid; `slot_index` from a prior take.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_release_shm(
    sub: *mut RmwZerodsSubscription,
    slot_index: u32,
) -> i32 {
    if sub.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: sub NULL-checked.
    let s = unsafe { &*sub };
    let Ok(r) = s.inner.lock() else {
        return RMW_RET_ERROR;
    };
    if r.is_null() {
        return RMW_RET_ERROR;
    }
    // SAFETY: reader valid; `slot_index` from a prior take.
    let rc = unsafe { zerodds::zerodds_reader_release_shm(*r, slot_index) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// Starts the raw-mode doorbell thread for this subscription (idempotent). The
/// thread parks on `zerodds_reader_raw_wait` and bumps the context condvar on
/// each raw-data arrival, so `rmw_wait` wakes event-driven. Call it once the raw
/// source (SHM segment / iceoryx service) is enabled on the reader.
///
/// # Safety
/// `sub` from `rmw_zerodds_create_subscription`.
#[cfg(feature = "flatdata-loan")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_start_doorbell(
    sub: *mut RmwZerodsSubscription,
) -> i32 {
    if sub.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: sub NULL-checked.
    let s = unsafe { &*sub };
    let Ok(mut guard) = s.doorbell.lock() else {
        return RMW_RET_ERROR;
    };
    if guard.is_some() {
        return RMW_RET_OK; // already running
    }
    let reader = {
        let Ok(r) = s.inner.lock() else {
            return RMW_RET_ERROR;
        };
        ReaderPtr(*r)
    };
    if reader.0.is_null() {
        return RMW_RET_ERROR;
    }
    let notify = s.inbox.notify.clone();
    let stop = Arc::new(core::sync::atomic::AtomicBool::new(false));
    let stop_t = stop.clone();
    let join = std::thread::spawn(move || {
        let rdr = reader; // move the Send wrapper into the thread
        while !stop_t.load(core::sync::atomic::Ordering::Relaxed) {
            // Park up to 250 ms (bounds stop latency); bump the wakeup edge only
            // on a real signal (`ZeroDdsStatus::Ok` == 0 → raw data arrived), so
            // an idle doorbell never wakes `rmw_wait`.
            // SAFETY: `rdr.0` stays valid — the thread is joined before the
            // reader is destroyed (see destroy_subscription).
            let rc = unsafe { zerodds::zerodds_reader_raw_wait(rdr.0, 250) };
            if rc == 0 {
                notify.notify();
            }
        }
    });
    *guard = Some(Doorbell {
        stop,
        join: Some(join),
    });
    RMW_RET_OK
}

/// Switches a publisher to `Iceoryx` (delivery mode 2) and routes its loan over
/// the iceoryx2 service `name` (max `max_len` bytes/sample). The same
/// loan/commit/take_shm/release_shm surface then drives the iceoryx2 ports.
/// Returns `RMW_RET_UNSUPPORTED` when the shim is built without
/// `delivery-iceoryx` (the caller then keeps `Portable`).
///
/// # Safety
/// `pub_` from `rmw_zerodds_create_publisher`; `name` a NUL-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publisher_enable_iceoryx(
    pub_: *mut RmwZerodsPublisher,
    name: *const c_char,
    max_len: usize,
) -> i32 {
    if pub_.is_null() || name.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    #[cfg(feature = "delivery-iceoryx")]
    {
        // SAFETY: pub_ NULL-checked.
        let p = unsafe { &*pub_ };
        let Ok(w) = p.inner.lock() else {
            return RMW_RET_ERROR;
        };
        if w.is_null() {
            return RMW_RET_ERROR;
        }
        // Iceoryx = 2 (`ZeroDdsDeliveryMode`).
        // SAFETY: writer valid; FFI NULL-tolerant.
        if unsafe { zerodds::zerodds_writer_set_delivery_mode(*w, 2) } != 0 {
            return RMW_RET_ERROR;
        }
        // SAFETY: writer valid; `name` is a NUL-terminated C string.
        if unsafe { zerodds::zerodds_writer_enable_iceoryx(*w, name, max_len) } != 0 {
            return RMW_RET_ERROR;
        }
        RMW_RET_OK
    }
    #[cfg(not(feature = "delivery-iceoryx"))]
    {
        let _ = max_len;
        RMW_RET_UNSUPPORTED
    }
}

/// Subscribes a subscription's reader to the iceoryx2 service `name`; samples are
/// then taken via `rmw_zerodds_subscription_take_shm` / `_release_shm`. Returns
/// `RMW_RET_UNSUPPORTED` without `delivery-iceoryx`.
///
/// # Safety
/// `sub` valid; `name` a NUL-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_enable_iceoryx(
    sub: *mut RmwZerodsSubscription,
    name: *const c_char,
) -> i32 {
    if sub.is_null() || name.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    #[cfg(feature = "delivery-iceoryx")]
    {
        // SAFETY: sub NULL-checked.
        let s = unsafe { &*sub };
        let Ok(r) = s.inner.lock() else {
            return RMW_RET_ERROR;
        };
        if r.is_null() {
            return RMW_RET_ERROR;
        }
        // SAFETY: reader valid; `name` is a NUL-terminated C string.
        let rc = unsafe { zerodds::zerodds_reader_enable_iceoryx(*r, name) };
        if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
    }
    #[cfg(not(feature = "delivery-iceoryx"))]
    {
        let _ = name;
        RMW_RET_UNSUPPORTED
    }
}

/// `rmw_take(sub, *mut buf, *mut len)` — tries to read a sample.
/// Caller MUST call `rmw_zerodds_buffer_free(buf, len)`.
///
/// # Safety
/// Pointer validity; out_buf/out_len NULL-checked below.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_take(
    sub: *mut RmwZerodsSubscription,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_big_endian: *mut u8,
) -> i32 {
    // SAFETY: forwarded unchanged; a NULL origin is not written.
    unsafe { rmw_zerodds_take_with_origin(sub, out_buf, out_len, out_big_endian, ptr::null_mut()) }
}

/// [`rmw_zerodds_take`] that also reports where the sample came from into
/// `out_origin` (NULL: not reported).
///
/// # Safety
/// As [`rmw_zerodds_take`]; `out_origin` writable or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_take_with_origin(
    sub: *mut RmwZerodsSubscription,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_big_endian: *mut u8,
    out_origin: *mut RmwZerodsSampleOrigin,
) -> i32 {
    if sub.is_null() || out_buf.is_null() || out_len.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let s = unsafe { &*sub };
    let popped = s.inbox.queue.lock().ok().and_then(|mut q| q.pop_front());
    match popped {
        Some((body, repr, be, origin)) if !body.is_empty() => {
            if !out_origin.is_null() {
                // SAFETY: writable per the caller.
                unsafe { *out_origin = origin };
            }
            // A CDR stream with its encapsulation header, as the C layer and
            // ROS serialized messages read it; the runtime delivers the body.
            let mut stream = Vec::with_capacity(body.len() + 4);
            stream.extend_from_slice(&encapsulation_header(repr, be));
            stream.extend_from_slice(&body);
            let bytes = stream.into_boxed_slice();
            let len = bytes.len();
            let ptr = alloc::boxed::Box::into_raw(bytes).cast::<u8>();
            // SAFETY: out_buf/out_len NULL-checked above; caller frees the
            // buffer via rmw_zerodds_buffer_free → zerodds_buffer_free, which
            // reconstructs the same `Box<[u8]>` of `len`.
            unsafe {
                *out_buf = ptr;
                *out_len = len;
                if !out_big_endian.is_null() {
                    *out_big_endian = be;
                }
            }
            RMW_RET_OK
        }
        _ => {
            // No data (or an empty payload): report none; the executor treats a
            // NULL buffer as `taken = false`.
            // SAFETY: out_buf/out_len NULL-checked above.
            unsafe {
                if !out_big_endian.is_null() {
                    *out_big_endian = 0;
                }
                *out_buf = ptr::null_mut();
                *out_len = 0;
            }
            RMW_RET_OK
        }
    }
}

/// Readiness query: `1` if the subscription has a sample ready to take, `0` if
/// not, negative `RMW_RET_*` on a bad handle. Pure inbox inspection — no
/// consumption, no allocation. Used by `rmw_wait` to report ready
/// subscriptions.
///
/// # Safety
/// `sub` must come from `rmw_zerodds_create_subscription` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_has_data(sub: *mut RmwZerodsSubscription) -> i32 {
    if sub.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let s = unsafe { &*sub };
    match s.inbox.queue.lock() {
        Ok(q) => i32::from(!q.is_empty()),
        Err(_) => RMW_RET_ERROR,
    }
}

/// Blocks until a subscription of `ctx` signals new data (its reader data
/// callback fired) or `timeout_ms` elapses. Event-driven: parks on the
/// context's shared condvar — no spin, no fixed-tick poll.
///
/// Returns `1` when woken by a notify (a generation change — data arrived, a
/// guard was triggered, or a cancel), `0` on a plain timeout. The caller
/// re-checks readiness after a return (the wake is an edge, the per-entity peek
/// is the truth); on a wake with nothing ready it must still let its own
/// executor re-evaluate (e.g. a `cancel`), rather than block indefinitely.
///
/// # Safety
/// `ctx` must come from `rmw_zerodds_init` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_context_wait_block(
    ctx: *mut RmwZerodsContext,
    since_gen: u64,
    timeout_ms: u64,
) -> i32 {
    if ctx.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let c = unsafe { &*ctx };
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    i32::from(c.notify.wait_until(since_gen, deadline))
}

/// Snapshots the context's wakeup generation. `rmw_wait` reads this *before*
/// checking readiness, then passes it to `rmw_zerodds_context_wait_block`, so an
/// event arriving between the check and the block is not lost.
///
/// # Safety
/// `ctx` must come from `rmw_zerodds_init` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_context_wait_generation(ctx: *mut RmwZerodsContext) -> u64 {
    if ctx.is_null() {
        return 0;
    }
    // SAFETY: NULL-checked.
    unsafe { &*ctx }.notify.current()
}

/// Wakes any `rmw_wait` blocked on `ctx` (e.g. after a guard condition is
/// triggered so the executor re-evaluates immediately).
///
/// # Safety
/// `ctx` must come from `rmw_zerodds_init` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_context_notify(ctx: *mut RmwZerodsContext) -> i32 {
    if ctx.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    unsafe { &*ctx }.notify.notify();
    RMW_RET_OK
}

/// `rmw_zerodds_buffer_free` — dual to zerodds_buffer_free, for
/// CDR bytes from rmw_zerodds_take.
///
/// # Safety
/// Like zerodds_buffer_free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_buffer_free(buf: *mut u8, len: usize) {
    // SAFETY: Pass-through.
    unsafe { zerodds::zerodds_buffer_free(buf, len) }
}

// ============================================================================
// Phase-B: Service-Layer + Wait-Sets + Loaning + REP-2009 Type-Hash
// ============================================================================

// Services and clients: `services.rs`.

// ----- Wait-Set (Phase-B): poll-based ---------------------------------------

/// Wait-set handle. Phase-B implemented: poll-based, not
/// edge-triggered. The caller adds subscriptions, calls `wait`
/// and gets back which indices have data ready.
pub struct RmwZerodsWaitSet {
    /// Pointer to the subscriptions we poll.
    subscriptions: Mutex<Vec<*mut RmwZerodsSubscription>>,
}

/// `rmw_create_wait_set()`.
///
/// # Safety
/// The result pointer is heap-allocated; the caller must call
/// `rmw_zerodds_destroy_wait_set`.
#[unsafe(no_mangle)]
pub extern "C" fn rmw_zerodds_create_wait_set() -> *mut RmwZerodsWaitSet {
    Box::into_raw(Box::new(RmwZerodsWaitSet {
        subscriptions: Mutex::new(Vec::new()),
    }))
}

/// `rmw_destroy_wait_set(*mut WaitSet)`.
///
/// # Safety
/// `ws` from `rmw_zerodds_create_wait_set` or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_wait_set(ws: *mut RmwZerodsWaitSet) -> i32 {
    if ws.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box-Pointer.
    let _ = unsafe { Box::from_raw(ws) };
    RMW_RET_OK
}

/// Adds a subscription to the wait set.
///
/// # Safety
/// `ws` + `sub` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_wait_set_add_subscription(
    ws: *mut RmwZerodsWaitSet,
    sub: *mut RmwZerodsSubscription,
) -> i32 {
    if ws.is_null() || sub.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let w = unsafe { &*ws };
    if let Ok(mut g) = w.subscriptions.lock() {
        g.push(sub);
        RMW_RET_OK
    } else {
        RMW_RET_ERROR
    }
}

/// Event-driven wait over the wait set's subscriptions: blocks until any
/// subscription has a sample ready (its reader data callback fired) or
/// `timeout_ms` elapses. Returns `RMW_RET_OK` if at least one subscription is
/// ready, `RMW_RET_TIMEOUT` otherwise.
///
/// Readiness is the non-destructive [`rmw_zerodds_subscription_has_data`] peek;
/// the blocking is parked on the context's shared condvar via
/// `WaitNotify::wait_until` — no spin loop, no fixed-tick poll. The generation
/// is snapshotted before the readiness check so a sample arriving in between
/// wakes the very next block instead of being missed.
///
/// # Safety
/// `ws` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_wait(ws: *mut RmwZerodsWaitSet, timeout_ms: u64) -> i32 {
    if ws.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let w = unsafe { &*ws };
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        // Snapshot the subscription pointers + the shared wakeup edge.
        let subs: Vec<*mut RmwZerodsSubscription> = {
            let Ok(g) = w.subscriptions.lock() else {
                return RMW_RET_ERROR;
            };
            g.iter().copied().filter(|p| !p.is_null()).collect()
        };
        // The wakeup edge is shared by all subscriptions of a context; take it
        // from the first. With no subscriptions there is nothing to wait on.
        let notify = match subs.first() {
            Some(p) => {
                // SAFETY: pointer from a live subscription created by this crate.
                let sref: &RmwZerodsSubscription = unsafe { &**p };
                sref.inbox.notify.clone()
            }
            None => return RMW_RET_TIMEOUT,
        };
        // Read the generation BEFORE checking readiness (race-free edge).
        let cur_gen = notify.current();
        let any_ready = subs.iter().any(|p| {
            // SAFETY: non-null subscription pointer from this crate.
            unsafe { rmw_zerodds_subscription_has_data(*p) == 1 }
        });
        if any_ready {
            return RMW_RET_OK;
        }
        if Instant::now() >= deadline {
            return RMW_RET_TIMEOUT;
        }
        notify.wait_until(cur_gen, deadline);
    }
}

// ----- Loaning (Phase-C: malloc-backed; Phase-D: SHM-backed) ---------------

/// `rmw_borrow_loaned_message(publisher, len, *out_ptr, *out_len)` —
/// reserves a buffer at the writer for zero-copy publish.
/// Phase-C: malloc-backed (not real zero-copy, but the code path is
/// stable). Phase-D switches to an SHM buffer pool when the
/// writer sits on an SHM transport.
///
/// # Safety
/// `pub_` must come from rmw_zerodds_create_publisher.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_borrow_loaned_message(
    pub_: *mut RmwZerodsPublisher,
    len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    if pub_.is_null() || out_ptr.is_null() || out_len.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let p = unsafe { &*pub_ };
    let writer = match p.inner.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: writer from create_publisher.
    let rc = unsafe { zerodds::zerodds_writer_loan_message(writer, len, out_ptr, out_len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// Commit path for a loan — writes the buffer as a sample and
/// frees it.
///
/// # Safety
/// `pub_` valid; `ptr`/`len` from rmw_zerodds_borrow_loaned_message.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publish_loaned_message(
    pub_: *mut RmwZerodsPublisher,
    ptr: *mut u8,
    len: usize,
) -> i32 {
    if pub_.is_null() || ptr.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let p = unsafe { &*pub_ };
    let writer = match p.inner.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: writer + ptr from borrow.
    let rc = unsafe { zerodds::zerodds_writer_commit_loan(writer, ptr, len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// Discards a loan without publishing it.
///
/// # Safety
/// Like rmw_zerodds_publish_loaned_message.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_return_loaned_message(
    pub_: *mut RmwZerodsPublisher,
    ptr: *mut u8,
    len: usize,
) -> i32 {
    if pub_.is_null() || ptr.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let p = unsafe { &*pub_ };
    let writer = match p.inner.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: writer + ptr from borrow.
    let rc = unsafe { zerodds::zerodds_writer_discard_loan(writer, ptr, len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

// ----- REP-2009 Type-Hash -------------------------------------------------

/// REP-2009 type hash: SHA-256 over the IDL type string.
/// Returns exactly 32 bytes in `out_hash` (must be a 32-byte buffer).
///
/// # Safety
/// `type_str` NUL-terminated C string; `out_hash` points to a
/// 32-byte buffer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_compute_type_hash(
    type_str: *const c_char,
    out_hash: *mut u8,
) -> i32 {
    if type_str.is_null() || out_hash.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NUL-terminated C-string.
    let s = match unsafe { CStr::from_ptr(type_str) }.to_bytes_with_nul() {
        b if b.len() > 1 => &b[..b.len() - 1],
        _ => return RMW_RET_INVALID_ARGUMENT,
    };
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s);
    let result = h.finalize();
    // SAFETY: out_hash NULL-checked; caller contract: 32-byte buffer.
    unsafe {
        core::ptr::copy_nonoverlapping(result.as_ptr(), out_hash, 32);
    }
    RMW_RET_OK
}

// ----- Graph introspection (P4a): discovered topics via the node's runtime ---

/// Invokes `callback(user_data, topic, type)` once per discovered remote
/// publication on `node`'s domain (raw DDS topic/type strings). Bridges to
/// [`zerodds::zerodds_runtime_for_each_publication`] on the node's runtime — the
/// C ABI cannot reach the runtime through the opaque context.
///
/// # Safety
/// `node` must come from `rmw_zerodds_create_node` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_node_for_each_publication(
    node: *mut RmwZerodsNode,
    callback: Option<zerodds::ZeroDdsTopicCallback>,
    user_data: *mut c_void,
) -> i32 {
    if node.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: node NULL-checked; ctx set at create.
    let rt = unsafe { (*(*node).ctx).runtime } as *mut zerodds::ZeroDdsRuntime;
    // SAFETY: rt from the runtime; callback is an FFI fn pointer.
    unsafe { zerodds::zerodds_runtime_for_each_publication(rt, callback, user_data) }
}

/// As [`rmw_zerodds_node_for_each_publication`] but for discovered remote
/// subscriptions.
///
/// # Safety
/// `node` must come from `rmw_zerodds_create_node` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_node_for_each_subscription(
    node: *mut RmwZerodsNode,
    callback: Option<zerodds::ZeroDdsTopicCallback>,
    user_data: *mut c_void,
) -> i32 {
    if node.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: node NULL-checked; ctx set at create.
    let rt = unsafe { (*(*node).ctx).runtime } as *mut zerodds::ZeroDdsRuntime;
    // SAFETY: rt from the runtime; callback is an FFI fn pointer.
    unsafe { zerodds::zerodds_runtime_for_each_subscription(rt, callback, user_data) }
}

/// Invokes `callback(user_data, &info)` once per **publication** endpoint on
/// `node`'s domain, with full per-endpoint info (GUID + QoS). Backs
/// `rmw_get_publishers_info_by_topic` (`ros2 topic info -v`). The C side
/// filters by topic and resolves the node name from the GUID prefix.
///
/// # Safety
/// `node` must come from `rmw_zerodds_create_node` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_node_for_each_publication_endpoint(
    node: *mut RmwZerodsNode,
    callback: Option<zerodds::ZeroDdsEndpointCallback>,
    user_data: *mut c_void,
) -> i32 {
    if node.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: node NULL-checked; ctx set at create.
    let rt = unsafe { (*(*node).ctx).runtime } as *mut zerodds::ZeroDdsRuntime;
    // SAFETY: rt from the runtime; callback is an FFI fn pointer.
    unsafe { zerodds::zerodds_runtime_for_each_publication_endpoint(rt, callback, user_data) }
}

/// As [`rmw_zerodds_node_for_each_publication_endpoint`] but for **subscription**
/// endpoints. Backs `rmw_get_subscriptions_info_by_topic`.
///
/// # Safety
/// `node` must come from `rmw_zerodds_create_node` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_node_for_each_subscription_endpoint(
    node: *mut RmwZerodsNode,
    callback: Option<zerodds::ZeroDdsEndpointCallback>,
    user_data: *mut c_void,
) -> i32 {
    if node.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: node NULL-checked; ctx set at create.
    let rt = unsafe { (*(*node).ctx).runtime } as *mut zerodds::ZeroDdsRuntime;
    // SAFETY: rt from the runtime; callback is an FFI fn pointer.
    unsafe { zerodds::zerodds_runtime_for_each_subscription_endpoint(rt, callback, user_data) }
}

/// Resolves a 16-byte endpoint GUID to its owning node `(namespace, name)` via
/// the node graph (local endpoints first, then remote endpoints learned from
/// `ros_discovery_info`). Writes NUL-terminated strings into the caller buffers
/// (truncated to fit). Returns `RMW_RET_OK` if found, `RMW_RET_ERROR` if the
/// GUID is unknown (caller should leave node fields empty). Backs the node-name
/// column of `rmw_get_publishers/subscriptions_info_by_topic`.
///
/// # Safety
/// `node` from `rmw_zerodds_create_node` or NULL; `gid16` points at 16 bytes;
/// the out buffers hold at least their cap bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_node_resolve_endpoint(
    node: *mut RmwZerodsNode,
    gid16: *const u8,
    out_ns: *mut c_char,
    ns_cap: usize,
    out_name: *mut c_char,
    name_cap: usize,
) -> i32 {
    if node.is_null() || gid16.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: node NULL-checked; ctx set at create.
    let graph = &unsafe { &*(*node).ctx }.graph;
    let mut key = [0u8; 16];
    // SAFETY: gid16 points at 16 readable bytes (caller contract).
    key.copy_from_slice(unsafe { core::slice::from_raw_parts(gid16, 16) });

    let found: Option<NodeId> = {
        let local = graph
            .local_eps
            .lock()
            .ok()
            .and_then(|e| e.iter().find(|x| x.gid == key).map(|x| x.node.clone()));
        local.or_else(|| {
            graph
                .remote_eps
                .lock()
                .ok()
                .and_then(|m| m.get(&key).cloned())
        })
    };
    let Some((ns, name)) = found else {
        return RMW_RET_ERROR;
    };
    // SAFETY: out buffers hold >= their cap bytes (caller contract).
    unsafe {
        write_c_str(out_ns, ns_cap, &ns);
        write_c_str(out_name, name_cap, &name);
    }
    RMW_RET_OK
}

/// Writes `s` as a NUL-terminated C string into `buf` (capacity `cap`),
/// truncating if needed. No-op if `buf` is NULL or `cap == 0`.
///
/// # Safety
/// `buf` must point at `cap` writable bytes.
unsafe fn write_c_str(buf: *mut c_char, cap: usize, s: &str) {
    if buf.is_null() || cap == 0 {
        return;
    }
    let bytes = s.as_bytes();
    let n = bytes.len().min(cap - 1);
    // SAFETY: n < cap; buf has cap writable bytes.
    unsafe {
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), buf.cast::<u8>(), n);
        *buf.add(n) = 0;
    }
}

// ----- on_new_* Event-Callbacks (EventsExecutor) ----------------------------

/// Sets the rmw `on_new_message` callback on a subscription. `cb = None` clears
/// it. Fired on each arrival (and once with the backlog count on set).
///
/// # Safety
/// `sub` must come from `rmw_zerodds_create_subscription` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_set_event_callback(
    sub: *mut RmwZerodsSubscription,
    cb: Option<RmwEventCallback>,
    user_data: *const c_void,
) -> i32 {
    if sub.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    inbox_set_event(&unsafe { &*sub }.inbox, cb, user_data);
    RMW_RET_OK
}

/// Sets a subscription's content filter (`rmw_subscription_set_content_filter`),
/// or clears it with an empty `expression`; the filter is announced in the
/// reader's SEDP subscription. `RMW_RET_INVALID_ARGUMENT` for an expression
/// that does not parse or names a parameter beyond `count`.
///
/// # Safety
/// `sub` from `rmw_zerodds_create_subscription` or NULL; the strings
/// NUL-terminated; `params[0..count]` valid or `count == 0`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_set_filter(
    sub: *mut RmwZerodsSubscription,
    expression: *const c_char,
    params: *const *const c_char,
    count: usize,
) -> i32 {
    if sub.is_null() || expression.is_null() || (count > 0 && params.is_null()) {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked; caller pledge on its origin.
    let s = unsafe { &*sub };
    // SAFETY: NUL-terminated per the caller.
    let Ok(text) = unsafe { CStr::from_ptr(expression) }.to_str() else {
        return RMW_RET_INVALID_ARGUMENT;
    };
    let Ok(reader) = s.inner.lock() else {
        return RMW_RET_ERROR;
    };
    if text.trim().is_empty() {
        if let Ok(mut f) = s.filter.lock() {
            *f = None;
        }
        // SAFETY: reader from zerodds_reader_create; NULL withdraws.
        unsafe {
            zerodds::zerodds_reader_set_content_filter(
                *reader,
                ptr::null(),
                ptr::null(),
                ptr::null(),
                ptr::null(),
                0,
            )
        };
        return RMW_RET_OK;
    }
    let Ok(expr) = zerodds_sql_filter::parse(text) else {
        return RMW_RET_INVALID_ARGUMENT;
    };
    let mut raw: Vec<&str> = Vec::with_capacity(count);
    for i in 0..count {
        // SAFETY: `params[0..count]` readable, each NUL-terminated.
        let p = unsafe { *params.add(i) };
        if p.is_null() {
            return RMW_RET_INVALID_ARGUMENT;
        }
        // SAFETY: NUL-terminated per the caller.
        let Ok(p) = unsafe { CStr::from_ptr(p) }.to_str() else {
            return RMW_RET_INVALID_ARGUMENT;
        };
        raw.push(p);
    }
    if expr
        .collect_param_indices()
        .iter()
        .any(|&i| i as usize >= raw.len())
    {
        return RMW_RET_INVALID_ARGUMENT;
    }
    let values = raw
        .iter()
        .map(|p| zerodds_sql_filter::Value::from_parameter(p))
        .collect();
    let Ok(name) = alloc::ffi::CString::new(alloc::format!("{}_filtered", s.dds_topic)) else {
        return RMW_RET_ERROR;
    };
    let Ok(related) = alloc::ffi::CString::new(s.dds_topic.clone()) else {
        return RMW_RET_ERROR;
    };
    // SAFETY: reader valid; strings and `params[0..count]` valid for the call.
    let announced = unsafe {
        zerodds::zerodds_reader_set_content_filter(
            *reader,
            name.as_ptr(),
            related.as_ptr(),
            expression,
            params,
            count,
        )
    };
    if announced != 0 {
        return RMW_RET_ERROR;
    }
    if let Ok(mut f) = s.filter.lock() {
        *f = Some(SubscriptionFilter {
            expr,
            params: values,
        });
    }
    RMW_RET_OK
}

/// Whether the message behind `ctx` passes the subscription's content filter
/// (1) or not (0); 1 without a filter. A message the expression cannot be
/// evaluated against (a field the getter does not answer, a type mismatch)
/// does not pass (DDS 1.4 §2.2.2.3.3).
///
/// # Safety
/// `sub` from `rmw_zerodds_create_subscription` or NULL; `getter` callable
/// with `ctx` for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_filter_accepts(
    sub: *mut RmwZerodsSubscription,
    ctx: *const c_void,
    getter: Option<RmwZerodsFieldGetter>,
) -> i32 {
    if sub.is_null() {
        return 1;
    }
    // SAFETY: NULL-checked; caller pledge on its origin.
    let s = unsafe { &*sub };
    let Ok(filter) = s.filter.lock() else {
        return 0;
    };
    let Some(filter) = filter.as_ref() else {
        return 1;
    };
    let Some(getter) = getter else {
        return 0;
    };
    let row = CallbackRow { ctx, getter };
    i32::from(filter.expr.evaluate(&row, &filter.params).unwrap_or(false))
}

/// Sets (or with NULL clears) the callback of a publisher QoS event —
/// `rmw_event_set_callback` for an event of `rmw_publisher_event_init`.
/// `kind` is the shim's number for the event (`events::EventKind`); the
/// callback gets `user_data` and the number of new events.
///
/// # Safety
/// `pub_` from `rmw_zerodds_create_publisher` or NULL; `user_data` valid
/// until the callback is cleared.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_publisher_set_status_callback(
    pub_: *mut RmwZerodsPublisher,
    kind: u32,
    cb: Option<RmwEventCallback>,
    user_data: *const c_void,
) -> i32 {
    let Some(kind) = events::EventKind::from_u32(kind).filter(|k| k.is_publisher()) else {
        return RMW_RET_UNSUPPORTED;
    };
    if pub_.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked; caller pledge on its origin.
    let p = unsafe { &*pub_ };
    p.events.set(kind, cb, user_data);
    if cb.is_some() && !p.ctx.is_null() {
        // SAFETY: ctx set at create, valid for the publisher's lifetime.
        unsafe { &*p.ctx }.events.watch(&p.events);
    }
    RMW_RET_OK
}

/// [`rmw_zerodds_publisher_set_status_callback`] for a subscription event.
///
/// # Safety
/// `sub` from `rmw_zerodds_create_subscription` or NULL; `user_data` valid
/// until the callback is cleared.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_subscription_set_status_callback(
    sub: *mut RmwZerodsSubscription,
    kind: u32,
    cb: Option<RmwEventCallback>,
    user_data: *const c_void,
) -> i32 {
    let Some(kind) = events::EventKind::from_u32(kind).filter(|k| !k.is_publisher()) else {
        return RMW_RET_UNSUPPORTED;
    };
    if sub.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked; caller pledge on its origin.
    let s = unsafe { &*sub };
    s.events.set(kind, cb, user_data);
    if cb.is_some() && !s.ctx.is_null() {
        // SAFETY: ctx set at create, valid for the subscription's lifetime.
        unsafe { &*s.ctx }.events.watch(&s.events);
    }
    RMW_RET_OK
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests may use unwrap.
mod tests {
    use super::*;

    /// RELIABLE, VOLATILE, KEEP_ALL: what the untyped endpoints these tests
    /// were written against delivered.
    fn test_qos() -> RmwZerodsQos {
        RmwZerodsQos {
            history: 2,
            reliability: 1,
            durability: 2,
            ..RmwZerodsQos::default()
        }
    }

    #[test]
    fn rmw_codes_match_rep_2007() {
        assert_eq!(RMW_RET_OK, 0);
        assert_eq!(RMW_RET_ERROR, 1);
        assert_eq!(RMW_RET_TIMEOUT, 2);
        assert_eq!(RMW_RET_UNSUPPORTED, 3);
        assert_eq!(RMW_RET_BAD_ALLOC, 10);
        assert_eq!(RMW_RET_INVALID_ARGUMENT, 11);
        assert_eq!(RMW_RET_INCORRECT_RMW_IMPLEMENTATION, 12);
    }

    #[test]
    fn impl_identifier_is_static_cstring() {
        let p = rmw_zerodds_get_implementation_identifier();
        assert!(!p.is_null());
        // SAFETY: static byte-array, CStr-checked.
        let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap();
        assert_eq!(s, "rmw_zerodds_cpp");
    }

    /// ParticipantEntitiesInfo with per-node reader/writer gid sequences must
    /// round-trip: node names AND the endpoint→node map (REP-2009 endpoint-info).
    #[test]
    fn participant_info_roundtrips_endpoint_gids() {
        let gid = [7u8; 24];
        let nodes: NodeList = alloc::vec![
            ("/".to_string(), "talker".to_string()),
            ("/ns".to_string(), "listener".to_string()),
        ];
        let w_gid: Gid16 = [0xAA; 16];
        let r_gid: Gid16 = [0xBB; 16];
        let eps = alloc::vec![
            LocalEp {
                node: ("/".to_string(), "talker".to_string()),
                writer: true,
                gid: w_gid
            },
            LocalEp {
                node: ("/ns".to_string(), "listener".to_string()),
                writer: false,
                gid: r_gid
            },
        ];
        let body = encode_participant_info(&gid, &nodes, &eps);
        let (dgid, dnodes, deps) = decode_participant_info(&body).unwrap();
        assert_eq!(dgid, gid);
        assert_eq!(dnodes, nodes); // node-names path unchanged
        // Endpoint→node map: writer gid -> talker, reader gid -> listener.
        let map: alloc::collections::BTreeMap<Gid16, NodeId> = deps.into_iter().collect();
        assert_eq!(
            map.get(&w_gid),
            Some(&("/".to_string(), "talker".to_string()))
        );
        assert_eq!(
            map.get(&r_gid),
            Some(&("/ns".to_string(), "listener".to_string()))
        );
    }

    /// Empty endpoint sequences (a node with no pub/sub) must still decode the
    /// node names — guards the node-names regression path.
    #[test]
    fn participant_info_roundtrips_without_endpoints() {
        let gid = [3u8; 24];
        let nodes: NodeList = alloc::vec![("/".to_string(), "solo".to_string())];
        let body = encode_participant_info(&gid, &nodes, &[]);
        let (_, dnodes, deps) = decode_participant_info(&body).unwrap();
        assert_eq!(dnodes, nodes);
        assert!(deps.is_empty());
    }

    #[test]
    fn serialization_format_is_cdr() {
        let p = rmw_zerodds_get_serialization_format();
        // SAFETY: static array.
        let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap();
        assert_eq!(s, "cdr");
    }

    #[test]
    fn shutdown_null_returns_invalid_argument() {
        // SAFETY: NULL-tolerant behavior is part of the contract.
        let r = unsafe { rmw_zerodds_shutdown(ptr::null_mut()) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn destroy_node_null_returns_invalid_argument() {
        // SAFETY: NULL-tolerant.
        let r = unsafe { rmw_zerodds_destroy_node(ptr::null_mut()) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn context_fini_null_returns_invalid_argument() {
        // SAFETY: NULL-tolerant.
        let r = unsafe { rmw_zerodds_context_fini(ptr::null_mut()) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    /// Regression: `rmw_shutdown` must NOT free the context — destroying a node
    /// AFTER shutdown (the `rclcpp::shutdown()`-then-node-out-of-scope order)
    /// reaches back into the context (`destroy_node` → `graph_publish`). Before
    /// the fix this was a use-after-free (segfault). The actual free happens in
    /// `context_fini`, which is called last.
    #[test]
    fn destroy_node_after_shutdown_is_safe() {
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        unsafe {
            let ctx = rmw_zerodds_init(0);
            assert!(!ctx.is_null());
            let name = std::ffi::CString::new("n").unwrap();
            let ns = std::ffi::CString::new("/").unwrap();
            let node = rmw_zerodds_create_node(ctx, name.as_ptr(), ns.as_ptr());
            assert!(!node.is_null());
            // Logical shutdown while the node is still alive …
            assert_eq!(rmw_zerodds_shutdown(ctx), RMW_RET_OK);
            // … then the node teardown must still reach a live context.
            assert_eq!(rmw_zerodds_destroy_node(node), RMW_RET_OK);
            // Only now the context is freed.
            assert_eq!(rmw_zerodds_context_fini(ctx), RMW_RET_OK);
        }
    }

    #[test]
    fn loaning_null_args_rejected() {
        let mut p: *mut u8 = ptr::null_mut();
        let mut l: usize = 0;
        // SAFETY: NULL-tolerant.
        let r = unsafe { rmw_zerodds_borrow_loaned_message(ptr::null_mut(), 64, &mut p, &mut l) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn return_loan_null_args_rejected() {
        // SAFETY: NULL-tolerant.
        let r = unsafe { rmw_zerodds_return_loaned_message(ptr::null_mut(), ptr::null_mut(), 0) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn destroy_client_null_returns_invalid_argument() {
        // SAFETY: NULL-tolerant.
        let r = unsafe { services::rmw_zerodds_destroy_client(ptr::null_mut()) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn destroy_service_null_returns_invalid_argument() {
        // SAFETY: NULL-tolerant.
        let r = unsafe { services::rmw_zerodds_destroy_service(ptr::null_mut()) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn wait_set_create_destroy_smoke() {
        let ws = rmw_zerodds_create_wait_set();
        assert!(!ws.is_null());
        // SAFETY: ws from create_wait_set.
        let r = unsafe { rmw_zerodds_destroy_wait_set(ws) };
        assert_eq!(r, RMW_RET_OK);
    }

    #[test]
    fn wait_returns_invalid_for_null() {
        // SAFETY: NULL-tolerant.
        let r = unsafe { rmw_zerodds_wait(ptr::null_mut(), 100) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn type_hash_sha256_is_deterministic() {
        let s = std::ffi::CString::new("std_msgs/msg/String").unwrap();
        let mut h1 = [0u8; 32];
        let mut h2 = [0u8; 32];
        // SAFETY: NUL-Strings + 32-byte buffers.
        let rc1 = unsafe { rmw_zerodds_compute_type_hash(s.as_ptr(), h1.as_mut_ptr()) };
        // SAFETY: NUL strings + 32-byte buffers (same contract).
        let rc2 = unsafe { rmw_zerodds_compute_type_hash(s.as_ptr(), h2.as_mut_ptr()) };
        assert_eq!(rc1, RMW_RET_OK);
        assert_eq!(rc2, RMW_RET_OK);
        assert_eq!(h1, h2);
        // First bytes != 0 (hash non-empty).
        assert_ne!(h1, [0u8; 32]);
    }

    #[test]
    fn type_hash_differs_for_different_types() {
        let a = std::ffi::CString::new("std_msgs/msg/String").unwrap();
        let b = std::ffi::CString::new("std_msgs/msg/Int32").unwrap();
        let mut ha = [0u8; 32];
        let mut hb = [0u8; 32];
        // SAFETY: NUL-Strings + Buffer.
        let _ = unsafe { rmw_zerodds_compute_type_hash(a.as_ptr(), ha.as_mut_ptr()) };
        // SAFETY: NUL strings + buffer (same contract).
        let _ = unsafe { rmw_zerodds_compute_type_hash(b.as_ptr(), hb.as_mut_ptr()) };
        assert_ne!(ha, hb);
    }

    #[test]
    fn type_hash_null_args_rejected() {
        let mut h = [0u8; 32];
        // SAFETY: check for a null arg.
        let r1 = unsafe { rmw_zerodds_compute_type_hash(ptr::null(), h.as_mut_ptr()) };
        assert_eq!(r1, RMW_RET_INVALID_ARGUMENT);
        let s = std::ffi::CString::new("x").unwrap();
        // SAFETY: out_hash NULL.
        let r2 = unsafe { rmw_zerodds_compute_type_hash(s.as_ptr(), ptr::null_mut()) };
        assert_eq!(r2, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn has_data_and_context_block_null_safe() {
        // SAFETY: NULL-tolerant contract.
        assert_eq!(
            unsafe { rmw_zerodds_subscription_has_data(ptr::null_mut()) },
            RMW_RET_INVALID_ARGUMENT
        );
        // SAFETY: NULL-tolerant.
        assert_eq!(
            unsafe { rmw_zerodds_context_wait_block(ptr::null_mut(), 0, 10) },
            RMW_RET_INVALID_ARGUMENT
        );
        // SAFETY: NULL-tolerant.
        assert_eq!(
            unsafe { rmw_zerodds_context_wait_generation(ptr::null_mut()) },
            0
        );
    }

    #[test]
    fn wait_notify_blocks_then_wakes_on_notify() {
        // A wait_until that times out returns; a concurrent notify wakes it
        // before the deadline. Pure unit test of the event edge (no DDS).
        let n = WaitNotify::new();
        let gen0 = n.current();
        // Times out cleanly when nothing notifies.
        let t0 = Instant::now();
        n.wait_until(gen0, t0 + Duration::from_millis(50));
        assert!(t0.elapsed() >= Duration::from_millis(40));
        // A notify from another thread wakes it well before a long deadline.
        let n2 = n.clone();
        let h = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            n2.notify();
        });
        let t1 = Instant::now();
        n.wait_until(gen0, t1 + Duration::from_secs(5));
        assert!(
            t1.elapsed() < Duration::from_secs(2),
            "must wake on notify, not on the 5s deadline"
        );
        h.join().unwrap();
        assert!(n.current() > gen0);
    }

    /// In-process integration: publish → reader data callback wakes the context
    /// condvar → `rmw_zerodds_wait` reports ready (event-driven, no poll) →
    /// `has_data` is true and `take` returns the bytes; then an empty `wait`
    /// times out cleanly instead of hanging.
    #[test]
    fn event_driven_wait_roundtrip_inprocess() {
        let topic = std::ffi::CString::new("rmw_wait_it").unwrap();
        let typ = std::ffi::CString::new("test::T").unwrap();
        let name = std::ffi::CString::new("n").unwrap();
        let ns = std::ffi::CString::new("/").unwrap();

        // Two participants (publisher node + subscriber node) on one domain —
        // the real cross-node rmw shape; a single runtime does not loop a
        // sample back to its own reader.
        // SAFETY: FFI bring-up with valid C strings on a private domain.
        let ctx_pub = unsafe { rmw_zerodds_init(91) };
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        let ctx_sub = unsafe { rmw_zerodds_init(91) };
        assert!(!ctx_pub.is_null() && !ctx_sub.is_null());
        // SPDP discovery must complete BEFORE endpoints are created, otherwise
        // they wire up against an empty locator list (see the c-api smoke test).
        // SAFETY: runtime pointers from rmw_zerodds_init.
        unsafe {
            let rp = (*ctx_pub).runtime as *mut zerodds::ZeroDdsRuntime;
            let rs = (*ctx_sub).runtime as *mut zerodds::ZeroDdsRuntime;
            assert_eq!(zerodds::zerodds_runtime_wait_for_peers(rp, 1, 5_000), 0);
            assert_eq!(zerodds::zerodds_runtime_wait_for_peers(rs, 1, 5_000), 0);
        }
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        let node_pub = unsafe { rmw_zerodds_create_node(ctx_pub, name.as_ptr(), ns.as_ptr()) };
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        let node_sub = unsafe { rmw_zerodds_create_node(ctx_sub, name.as_ptr(), ns.as_ptr()) };
        assert!(!node_pub.is_null() && !node_sub.is_null());
        let publ =
            // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
            unsafe { rmw_zerodds_create_publisher(node_pub, typ.as_ptr(), topic.as_ptr(), &test_qos(), ptr::null_mut()) };
        assert!(!publ.is_null());
        let sub =
            // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
            unsafe { rmw_zerodds_create_subscription(node_sub, typ.as_ptr(), topic.as_ptr(), &test_qos(), ptr::null_mut()) };
        assert!(!sub.is_null());

        let ws = rmw_zerodds_create_wait_set();
        // SAFETY: ws + sub valid.
        assert_eq!(
            unsafe { rmw_zerodds_wait_set_add_subscription(ws, sub) },
            RMW_RET_OK
        );

        // Publish until the wait reports ready (covers discovery + match latency).
        let payload: [u8; 9] = [0x00, 0x01, 0x00, 0x00, b'h', b'e', b'l', b'l', b'o'];
        let mut woke = false;
        for _ in 0..50 {
            // SAFETY: publ valid; payload lives across the call.
            let _ = unsafe { rmw_zerodds_publish(publ, payload.as_ptr(), payload.len()) };
            // SAFETY: ws valid.
            if unsafe { rmw_zerodds_wait(ws, 200) } == RMW_RET_OK {
                woke = true;
                break;
            }
        }
        assert!(woke, "event-driven wait must report ready after a publish");

        // SAFETY: sub valid.
        assert_eq!(unsafe { rmw_zerodds_subscription_has_data(sub) }, 1);
        let mut buf: *mut u8 = ptr::null_mut();
        let mut len: usize = 0;
        // SAFETY: out pointers valid; sub valid.
        let tr = unsafe { rmw_zerodds_take(sub, &mut buf, &mut len, &mut 0u8) };
        assert_eq!(tr, RMW_RET_OK);
        assert!(!buf.is_null() && len >= 4, "take returns the parked sample");
        // SAFETY: buf/len from take.
        unsafe { rmw_zerodds_buffer_free(buf, len) };

        // Drain any straggler reliable retransmits so the sub is genuinely empty.
        for _ in 0..8 {
            // SAFETY: sub valid.
            if unsafe { rmw_zerodds_subscription_has_data(sub) } != 1 {
                break;
            }
            let mut b2: *mut u8 = ptr::null_mut();
            let mut l2: usize = 0;
            // SAFETY: valid out pointers.
            let _ = unsafe { rmw_zerodds_take(sub, &mut b2, &mut l2, &mut 0u8) };
            if !b2.is_null() {
                // SAFETY: from take.
                unsafe { rmw_zerodds_buffer_free(b2, l2) };
            }
        }
        // An empty wait must TIME OUT (bounded) — not hang, not busy-spin.
        let t = Instant::now();
        // SAFETY: ws valid.
        let empty = unsafe { rmw_zerodds_wait(ws, 150) };
        assert_eq!(empty, RMW_RET_TIMEOUT);
        assert!(
            t.elapsed() >= Duration::from_millis(120),
            "wait honours the timeout"
        );

        // SAFETY: all handles valid + owned.
        unsafe {
            let _ = rmw_zerodds_destroy_wait_set(ws);
            let _ = rmw_zerodds_destroy_subscription(sub);
            let _ = rmw_zerodds_destroy_publisher(publ);
            let _ = rmw_zerodds_destroy_node(node_pub);
            let _ = rmw_zerodds_destroy_node(node_sub);
            let _ = rmw_zerodds_shutdown(ctx_pub);
            let _ = rmw_zerodds_shutdown(ctx_sub);
            let _ = rmw_zerodds_context_fini(ctx_pub);
            let _ = rmw_zerodds_context_fini(ctx_sub);
        }
    }

    /// Service qos of REP-2009 (`rmw_qos_profile_services_default`).
    fn services_qos() -> RmwZerodsQos {
        RmwZerodsQos {
            history: 1,
            depth: 10,
            reliability: 1,
            durability: 2,
            liveliness: 1,
            ..RmwZerodsQos::default()
        }
    }

    /// Sends `body` until the service has a request, takes it, answers
    /// `answer` and returns what the client takes: the reply body and the
    /// sequence id, next to the one `send_request` gave.
    ///
    /// # Safety
    /// Handles valid.
    unsafe fn round_trip(
        client: *mut services::RmwZerodsClient,
        service: *mut services::RmwZerodsService,
        body: &[u8],
        answer: &[u8],
    ) -> (Vec<u8>, Vec<u8>, i64, i64) {
        let stream = |b: &[u8]| {
            let mut v = alloc::vec![0x00, 0x01, 0x00, 0x00];
            v.extend_from_slice(b);
            v
        };
        let req = stream(body);
        let mut sent_seq = 0i64;
        let mut got = false;
        for _ in 0..50 {
            // SAFETY: handles valid by the caller.
            unsafe {
                assert_eq!(
                    services::rmw_zerodds_send_request(
                        client,
                        req.as_ptr(),
                        req.len(),
                        &mut sent_seq
                    ),
                    RMW_RET_OK
                );
                if services::rmw_zerodds_service_has_data(service) == 1 {
                    got = true;
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(got, "the service receives a request");
        let (mut buf, mut len, mut guid, mut seq) = (ptr::null_mut(), 0usize, [0u8; 16], 0i64);
        // SAFETY: valid out pointers.
        unsafe {
            assert_eq!(
                services::rmw_zerodds_take_request(
                    service,
                    &mut buf,
                    &mut len,
                    ptr::null_mut(),
                    guid.as_mut_ptr(),
                    &mut seq,
                ),
                RMW_RET_OK
            );
        }
        assert!(!buf.is_null());
        // SAFETY: from take.
        let request = unsafe { core::slice::from_raw_parts(buf, len) }.to_vec();
        // SAFETY: from take.
        unsafe { rmw_zerodds_buffer_free(buf, len) };
        let rep = stream(answer);
        // SAFETY: handles valid.
        assert_eq!(
            unsafe {
                services::rmw_zerodds_send_response(
                    service,
                    guid.as_ptr(),
                    seq,
                    rep.as_ptr(),
                    rep.len(),
                )
            },
            RMW_RET_OK
        );
        let mut reply = Vec::new();
        let mut reply_seq = -1i64;
        for _ in 0..50 {
            let (mut pbuf, mut plen) = (ptr::null_mut(), 0usize);
            // SAFETY: valid out pointers.
            unsafe {
                services::rmw_zerodds_take_response(
                    client,
                    &mut pbuf,
                    &mut plen,
                    ptr::null_mut(),
                    &mut reply_seq,
                );
            }
            if !pbuf.is_null() {
                // SAFETY: from take.
                reply = unsafe { core::slice::from_raw_parts(pbuf, plen) }.to_vec();
                // SAFETY: from take.
                unsafe { rmw_zerodds_buffer_free(pbuf, plen) };
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        (request, reply, sent_seq, reply_seq)
    }

    /// Two runtimes: the request reaches the service with its
    /// encapsulation, the reply the client with the sequence id of the last
    /// request it sent, and the service topics are the `rq`/`rr` names.
    #[test]
    fn a_service_call_between_two_runtimes_correlates_by_the_related_identity() {
        let svc = std::ffi::CString::new("/rmw_srv_it").unwrap();
        let req_t = std::ffi::CString::new("test::srv::dds_::Srv_Request_").unwrap();
        let rep_t = std::ffi::CString::new("test::srv::dds_::Srv_Response_").unwrap();
        let name = std::ffi::CString::new("n").unwrap();
        let ns = std::ffi::CString::new("/").unwrap();
        let q = services_qos();
        // SAFETY: FFI bring-up on a private domain with valid C strings.
        unsafe {
            let ctx_cli = rmw_zerodds_init(92);
            let ctx_srv = rmw_zerodds_init(92);
            assert!(!ctx_cli.is_null() && !ctx_srv.is_null());
            let rc = (*ctx_cli).runtime as *mut zerodds::ZeroDdsRuntime;
            let rs = (*ctx_srv).runtime as *mut zerodds::ZeroDdsRuntime;
            assert_eq!(zerodds::zerodds_runtime_wait_for_peers(rc, 1, 5_000), 0);
            assert_eq!(zerodds::zerodds_runtime_wait_for_peers(rs, 1, 5_000), 0);
            let node_cli = rmw_zerodds_create_node(ctx_cli, name.as_ptr(), ns.as_ptr());
            let node_srv = rmw_zerodds_create_node(ctx_srv, name.as_ptr(), ns.as_ptr());
            let client = services::rmw_zerodds_create_client(
                node_cli,
                svc.as_ptr(),
                req_t.as_ptr(),
                rep_t.as_ptr(),
                &q,
                ptr::null_mut(),
            );
            let service = services::rmw_zerodds_create_service(
                node_srv,
                svc.as_ptr(),
                req_t.as_ptr(),
                rep_t.as_ptr(),
                &q,
                ptr::null_mut(),
            );
            assert!(!client.is_null() && !service.is_null());
            let topics = zerodds::runtime_arc(rs)
                .unwrap()
                .discovered_subscription_topics();
            assert!(
                topics
                    .iter()
                    .any(|(t, ty)| t == "rq/rmw_srv_itRequest"
                        && ty == "test::srv::dds_::Srv_Request_")
                    || zerodds::runtime_arc(rc)
                        .unwrap()
                        .discovered_subscription_topics()
                        .iter()
                        .any(|(t, _)| t == "rq/rmw_srv_itRequest"),
                "request topic: {topics:?}"
            );
            let (request, reply, sent, got) = round_trip(client, service, &[1, 2, 3, 4], &[9, 8]);
            assert_eq!(request, [0, 1, 0, 0, 1, 2, 3, 4]);
            assert_eq!(reply, [0, 1, 0, 0, 9, 8]);
            // The client may have sent several requests until the service
            // matched; the service answered the one it took first.
            assert!(got >= 1 && got <= sent, "reply to {got}, sent up to {sent}");
            let _ = services::rmw_zerodds_destroy_client(client);
            let _ = services::rmw_zerodds_destroy_service(service);
            let _ = rmw_zerodds_destroy_node(node_cli);
            let _ = rmw_zerodds_destroy_node(node_srv);
            let _ = rmw_zerodds_shutdown(ctx_cli);
            let _ = rmw_zerodds_shutdown(ctx_srv);
            let _ = rmw_zerodds_context_fini(ctx_cli);
            let _ = rmw_zerodds_context_fini(ctx_srv);
        }
    }

    /// One runtime, two clients of one service: each takes only the reply to
    /// its own request (the same-runtime path carries the sequence number and
    /// the related identity as the wire does).
    #[test]
    fn two_clients_in_one_runtime_each_get_their_own_reply() {
        let svc = std::ffi::CString::new("/rmw_srv_same").unwrap();
        let req_t = std::ffi::CString::new("test::srv::dds_::Srv_Request_").unwrap();
        let rep_t = std::ffi::CString::new("test::srv::dds_::Srv_Response_").unwrap();
        let name = std::ffi::CString::new("n").unwrap();
        let ns = std::ffi::CString::new("/").unwrap();
        let q = services_qos();
        // SAFETY: FFI bring-up with valid C strings.
        unsafe {
            let ctx = rmw_zerodds_init(93);
            assert!(!ctx.is_null());
            let node = rmw_zerodds_create_node(ctx, name.as_ptr(), ns.as_ptr());
            let mk_client = || {
                services::rmw_zerodds_create_client(
                    node,
                    svc.as_ptr(),
                    req_t.as_ptr(),
                    rep_t.as_ptr(),
                    &q,
                    ptr::null_mut(),
                )
            };
            let a = mk_client();
            let b = mk_client();
            let service = services::rmw_zerodds_create_service(
                node,
                svc.as_ptr(),
                req_t.as_ptr(),
                rep_t.as_ptr(),
                &q,
                ptr::null_mut(),
            );
            assert!(!a.is_null() && !b.is_null() && !service.is_null());
            let (_, reply_a, _, _) = round_trip(a, service, &[0xA], &[0xAA]);
            assert_eq!(reply_a, [0, 1, 0, 0, 0xAA]);
            // B saw A's reply on the shared reply topic and skips it.
            let (mut pbuf, mut plen, mut seq) = (ptr::null_mut(), 0usize, 0i64);
            services::rmw_zerodds_take_response(b, &mut pbuf, &mut plen, ptr::null_mut(), &mut seq);
            assert!(pbuf.is_null(), "B takes nothing of A's");
            let (_, reply_b, sent_b, got_b) = round_trip(b, service, &[0xB], &[0xBB]);
            assert_eq!(reply_b, [0, 1, 0, 0, 0xBB]);
            assert_eq!(got_b, sent_b);
            let _ = services::rmw_zerodds_destroy_client(a);
            let _ = services::rmw_zerodds_destroy_client(b);
            let _ = services::rmw_zerodds_destroy_service(service);
            let _ = rmw_zerodds_destroy_node(node);
            let _ = rmw_zerodds_shutdown(ctx);
            let _ = rmw_zerodds_context_fini(ctx);
        }
    }

    extern "C" fn count_events(ud: *const c_void, n: usize) {
        // SAFETY: ud is the &AtomicUsize passed as user_data; alive for the test.
        let c = unsafe { &*(ud as *const core::sync::atomic::AtomicUsize) };
        c.fetch_add(n, core::sync::atomic::Ordering::SeqCst);
    }

    /// O1: the on_new_message event callback fires on arrival.
    #[test]
    fn event_callback_fires_on_arrival() {
        use core::sync::atomic::{AtomicUsize, Ordering};
        let counter = AtomicUsize::new(0);
        let topic = std::ffi::CString::new("rmw_evcb_it").unwrap();
        let typ = std::ffi::CString::new("test::T").unwrap();
        let name = std::ffi::CString::new("n").unwrap();
        let ns = std::ffi::CString::new("/").unwrap();

        // SAFETY: FFI bring-up.
        let ctx_pub = unsafe { rmw_zerodds_init(93) };
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        let ctx_sub = unsafe { rmw_zerodds_init(93) };
        assert!(!ctx_pub.is_null() && !ctx_sub.is_null());
        // SAFETY: discovery before endpoints.
        unsafe {
            let rp = (*ctx_pub).runtime as *mut zerodds::ZeroDdsRuntime;
            let rs = (*ctx_sub).runtime as *mut zerodds::ZeroDdsRuntime;
            assert_eq!(zerodds::zerodds_runtime_wait_for_peers(rp, 1, 5_000), 0);
            assert_eq!(zerodds::zerodds_runtime_wait_for_peers(rs, 1, 5_000), 0);
        }
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        let node_pub = unsafe { rmw_zerodds_create_node(ctx_pub, name.as_ptr(), ns.as_ptr()) };
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        let node_sub = unsafe { rmw_zerodds_create_node(ctx_sub, name.as_ptr(), ns.as_ptr()) };
        let publ =
            // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
            unsafe { rmw_zerodds_create_publisher(node_pub, typ.as_ptr(), topic.as_ptr(), &test_qos(), ptr::null_mut()) };
        let sub =
            // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
            unsafe { rmw_zerodds_create_subscription(node_sub, typ.as_ptr(), topic.as_ptr(), &test_qos(), ptr::null_mut()) };
        assert!(!publ.is_null() && !sub.is_null());

        // SAFETY: counter outlives the subscription (dropped after destroy below).
        let r = unsafe {
            rmw_zerodds_subscription_set_event_callback(
                sub,
                Some(count_events),
                core::ptr::addr_of!(counter).cast(),
            )
        };
        assert_eq!(r, RMW_RET_OK);

        let payload: [u8; 5] = [0x00, 0x01, 0x00, 0x00, 7];
        for _ in 0..50 {
            // SAFETY: publ valid.
            let _ = unsafe { rmw_zerodds_publish(publ, payload.as_ptr(), payload.len()) };
            if counter.load(Ordering::SeqCst) > 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(
            counter.load(Ordering::SeqCst) > 0,
            "event callback must fire on arrival"
        );

        // SAFETY: destroy the subscription (stops the listener) BEFORE `counter`
        // drops, so no callback can fire into freed memory.
        unsafe {
            let _ = rmw_zerodds_destroy_subscription(sub);
            let _ = rmw_zerodds_destroy_publisher(publ);
            let _ = rmw_zerodds_destroy_node(node_pub);
            let _ = rmw_zerodds_destroy_node(node_sub);
            let _ = rmw_zerodds_shutdown(ctx_pub);
            let _ = rmw_zerodds_shutdown(ctx_sub);
            let _ = rmw_zerodds_context_fini(ctx_pub);
            let _ = rmw_zerodds_context_fini(ctx_sub);
        }
    }
}
