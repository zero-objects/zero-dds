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

use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use std::ffi::CStr;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use zerodds_ros2_rmw::ffi_api::RMW_IMPLEMENTATION_IDENTIFIER;
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
/// receive thread the instant a sample lands and calls [`WaitNotify::notify`],
/// which bumps a generation counter and wakes the condvar. `rmw_wait` blocks on
/// the condvar via [`WaitNotify::wait_until`] and re-evaluates readiness on each
/// wake — no spin loop and no fixed-tick poll (the readiness *truth* is the
/// non-destructive [`subscription_has_data`] peek; this type only supplies the
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
type InboxEntry = (alloc::boxed::Box<[u8]>, u8, u8);

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
    if let Ok(mut q) = inbox.queue.lock() {
        q.push_back((bytes, repr, big_endian));
    }
    inbox.notify.notify();
    // Fire the rmw EventsExecutor callback (snapshot it, then call outside the
    // lock so a re-entrant set_callback cannot deadlock).
    let ev = inbox.event.lock().ok().and_then(|g| *g);
    if let Some(ev) = ev {
        (ev.cb)(ev.ud as *const c_void, 1);
    }
}

/// Attaches a listener-fed inbox to a ZeroDDS reader: registers
/// [`subscription_on_data`] so the reader delivers into the returned inbox and
/// wakes `notify`. Returns the inbox plus the `Arc<SubInbox>` raw pointer handed
/// to the callback as `user_data` — reclaim it with `Arc::from_raw` AFTER the
/// reader is destroyed. `None` if the listener could not be registered.
///
/// # Safety
/// `reader` must come from `zerodds_reader_create*`.
unsafe fn attach_inbox(
    reader: *mut zerodds::ZeroDdsReader,
    notify: Arc<WaitNotify>,
) -> Option<(Arc<SubInbox>, *const SubInbox)> {
    let inbox = Arc::new(SubInbox {
        queue: Mutex::new(alloc::collections::VecDeque::new()),
        notify,
        event: Mutex::new(None),
    });
    let ud = Arc::into_raw(inbox.clone());
    // SAFETY: reader valid; callback + user_data outlive the reader (the caller
    // reclaims `ud` after destroying the reader).
    let set = unsafe {
        zerodds::zerodds_reader_set_data_callback(
            reader,
            Some(subscription_on_data),
            ud as *mut c_void,
        )
    };
    if set == 0 {
        Some((inbox, ud))
    } else {
        // SAFETY: reclaim the extra ref we just leaked.
        drop(unsafe { Arc::from_raw(ud) });
        None
    }
}

/// Pops one sample from `inbox` into a freshly heap-allocated buffer the caller
/// frees via `zerodds_buffer_free`; writes a NULL buffer when the inbox is
/// empty. Always returns `RMW_RET_OK` (empty is not an error).
///
/// # Safety
/// `out_buf` / `out_len` must be valid out pointers.
unsafe fn inbox_take(
    inbox: &SubInbox,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_big_endian: *mut u8,
) -> i32 {
    let popped = inbox.queue.lock().ok().and_then(|mut q| q.pop_front());
    match popped {
        Some((bytes, _repr, be)) if !bytes.is_empty() => {
            let len = bytes.len();
            let ptr = alloc::boxed::Box::into_raw(bytes).cast::<u8>();
            // SAFETY: out pointers valid per caller contract.
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
            // SAFETY: out pointers valid per caller contract.
            unsafe {
                *out_buf = ptr::null_mut();
                *out_len = 0;
                if !out_big_endian.is_null() {
                    *out_big_endian = 0;
                }
            }
            RMW_RET_OK
        }
    }
}

/// `1` if `inbox` has a sample ready, `0` if empty, `RMW_RET_ERROR` if poisoned.
fn inbox_has_data(inbox: &SubInbox) -> i32 {
    match inbox.queue.lock() {
        Ok(q) => i32::from(!q.is_empty()),
        Err(_) => RMW_RET_ERROR,
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
    _repr: u8,
    _big_endian: u8,
) {
    if ud.is_null() || payload.is_null() || len < 4 {
        return;
    }
    // SAFETY: `ud` is a live `Arc<NodeGraph>` raw pointer (reclaimed only after
    // the discovery reader is destroyed); `payload`/`len` is the CDR body.
    let graph = unsafe { &*(ud as *const NodeGraph) };
    // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
    let body = unsafe { core::slice::from_raw_parts(payload, len) };
    if let Some((gid, nodes, eps)) = decode_participant_info(body) {
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
    let body = encode_participant_info(&graph.gid, &nodes, &eps);
    if let Ok(w) = graph.writer.lock() {
        if !w.is_null() {
            // SAFETY: writer from zerodds_writer_create; body lives for the call.
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
/// init). Hardcoded domain 0 in current distros; later from init_options.
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
}

impl Drop for RmwZerodsContext {
    fn drop(&mut self) {
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
    }
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
    // C7 "secure by default": with the `security` feature, if the SROS2
    // enclave env `ZERODDS_SECURITY_DIR` is set, create a DDS-Security
    // participant from the enclave; otherwise (or without the feature) the
    // plain participant. A set-but-failed enclave is a hard error (NULL) —
    // security was requested and must not silently downgrade.
    #[cfg(feature = "security")]
    let rt = if std::env::var_os("ZERODDS_SECURITY_DIR").is_some() {
        // SAFETY: no pointer args; NULL on missing/invalid enclave.
        unsafe { zerodds::security_ffi::zerodds_runtime_create_secure_from_env(domain_id) }
    } else {
        // A2/A5: ROS-2 out-of-the-box profile — the reader offers XCDR1+XCDR2 so
        // it matches rmw_cyclonedds/rmw_fastrtps XCDR1 writers config-free.
        // SAFETY: NULL-tolerant + heap-allocated.
        unsafe { zerodds::zerodds_runtime_create_ros_defaults(domain_id) }
    };
    #[cfg(not(feature = "security"))]
    // A2/A5: ROS-2 out-of-the-box profile (XCDR1+XCDR2 offer), config-free interop.
    // SAFETY: NULL-tolerant + heap-allocated.
    let rt = unsafe { zerodds::zerodds_runtime_create_ros_defaults(domain_id) };
    if rt.is_null() {
        return ptr::null_mut();
    }
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
    // SAFETY: runtime valid; NUL-terminated strings.
    let dwriter =
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        unsafe { zerodds::zerodds_writer_create(rt, topic_c.as_ptr(), type_c.as_ptr(), 1) };
    if !dwriter.is_null() {
        if let Ok(mut w) = graph.writer.lock() {
            *w = dwriter;
        }
    }
    // SAFETY: runtime valid; NUL-terminated strings.
    let dreader =
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        unsafe { zerodds::zerodds_reader_create(rt, topic_c.as_ptr(), type_c.as_ptr(), 1) };
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
    Box::into_raw(Box::new(RmwZerodsContext {
        domain_id,
        runtime: rt as *mut c_void,
        notify: WaitNotify::new(),
        graph,
        discovery_reader: dreader,
        discovery_cb_ud,
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
    // SAFETY: NUL-terminierter C-String, NULL-checked oben.
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
}

/// `rmw_create_publisher(node, type_name, topic_name, reliable)`.
///
/// # Safety
/// Pointer validity as always; strings NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_publisher(
    node: *mut RmwZerodsNode,
    type_name: *const c_char,
    topic_name: *const c_char,
    reliable: c_int,
) -> *mut RmwZerodsPublisher {
    if node.is_null() || type_name.is_null() || topic_name.is_null() {
        return ptr::null_mut();
    }
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
    let topic_dds = zerodds_ros2_rmw::topic_mangling::mangle_topic_name(
        &topic_ros,
        zerodds_ros2_rmw::topic_mangling::RosKind::Topic,
    )
    .unwrap_or_else(|_| topic_ros.clone());
    let dds_topic_c = match std::ffi::CString::new(topic_dds.clone()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let typ_c = match std::ffi::CString::new(typ.clone()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: writer_create NULL-tolerant.
    let writer = unsafe {
        zerodds::zerodds_writer_create(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            dds_topic_c.as_ptr(),
            typ_c.as_ptr(),
            reliable,
        )
    };
    if writer.is_null() {
        return ptr::null_mut();
    }
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
    }))
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
    if let Ok(g) = p.inner.lock() {
        // SAFETY: writer comes from zerodds_writer_create + Box owns.
        unsafe { zerodds::zerodds_writer_destroy(*g) };
    }
    RMW_RET_OK
}

/// `rmw_publish(pub, payload, len)` — schreibt CDR-encoded bytes.
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
    // SAFETY: payload + len contract via FFI; writer from create.
    let rc = unsafe { zerodds::zerodds_writer_write(writer, payload, len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// A2 — resolve the TIME_BASED_FILTER `minimum_separation` (nanoseconds) for a
/// ROS topic from the `ZERODDS_TIME_BASED_FILTER` env. ROS 2 has no
/// `rmw_qos_profile_t` field for TIME_BASED_FILTER, so this is the
/// vendor-idiomatic config seam (cf. `CYCLONEDDS_URI` / Fast DDS XML).
///
/// Value = comma-separated entries; each is either `<seconds>` (a bare number =
/// global default for every subscription) or `<topic>=<seconds>` (per-topic
/// override, where `<topic>` is the ROS topic name, e.g. `/scan`). A per-topic
/// entry wins over the global default. `Some(0)`/absent/unparseable → no filter.
///
/// Examples: `ZERODDS_TIME_BASED_FILTER=0.1` (10 Hz cap on all subs);
/// `ZERODDS_TIME_BASED_FILTER=/scan=0.2,/image=0.5` (per-topic).
fn tbf_min_separation_nanos_for(topic_ros: &str) -> Option<u64> {
    let raw = std::env::var("ZERODDS_TIME_BASED_FILTER").ok()?;
    parse_tbf_env(&raw, topic_ros)
}

/// Pure parser for [`tbf_min_separation_nanos_for`] (no env access, unit-testable).
fn parse_tbf_env(raw: &str, topic_ros: &str) -> Option<u64> {
    let to_ns = |secs: f64| -> Option<u64> {
        if secs.is_finite() && secs > 0.0 {
            Some((secs * 1_000_000_000.0) as u64)
        } else {
            None
        }
    };
    let mut global: Option<u64> = None;
    for entry in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match entry.split_once('=') {
            Some((topic, secs)) => {
                if topic.trim() == topic_ros {
                    // Per-topic override takes precedence; return immediately.
                    return secs.trim().parse::<f64>().ok().and_then(to_ns);
                }
            }
            None => {
                if let Ok(secs) = entry.parse::<f64>() {
                    global = to_ns(secs);
                }
            }
        }
    }
    global
}

/// `rmw_create_subscription(node, type, topic, reliable)`.
///
/// # Safety
/// Wie create_publisher.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_subscription(
    node: *mut RmwZerodsNode,
    type_name: *const c_char,
    topic_name: *const c_char,
    reliable: c_int,
) -> *mut RmwZerodsSubscription {
    if node.is_null() || type_name.is_null() || topic_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: node NULL-checked oben.
    let n = unsafe { &*node };
    // SAFETY: n.ctx must be initialized by rmw_zerodds_init.
    let ctx = unsafe { &*n.ctx };
    // SAFETY: topic_name NULL-checked oben; caller-Kontrakt: NUL-
    // terminierter C-String.
    let topic_ros = match unsafe { CStr::from_ptr(topic_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: type_name NULL-checked oben; caller-Kontrakt: NUL-
    // terminierter C-String.
    let typ = match unsafe { CStr::from_ptr(type_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    let topic_dds = zerodds_ros2_rmw::topic_mangling::mangle_topic_name(
        &topic_ros,
        zerodds_ros2_rmw::topic_mangling::RosKind::Topic,
    )
    .unwrap_or_else(|_| topic_ros.clone());
    let dds_topic_c = match std::ffi::CString::new(topic_dds.clone()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let typ_c = match std::ffi::CString::new(typ.clone()) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: reader_create NULL-tolerant.
    let reader = unsafe {
        zerodds::zerodds_reader_create(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            dds_topic_c.as_ptr(),
            typ_c.as_ptr(),
            reliable,
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
    });
    let cb_userdata = Arc::into_raw(inbox.clone());
    // SAFETY: reader from reader_create; callback + user_data outlive the reader.
    let set = unsafe {
        zerodds::zerodds_reader_set_data_callback(
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
    // the subscription rate-limit is set per-topic via `ZERODDS_TIME_BASED_FILTER`.
    if let Some(ns) = tbf_min_separation_nanos_for(&topic_ros) {
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
    if let Ok(g) = s.inner.lock() {
        // Destroy the reader (after the doorbell is gone) — this also stops the
        // receive thread from firing the data callback, so the callback's
        // `user_data` Arc can be reclaimed race-free below.
        // SAFETY: reader kommt aus zerodds_reader_create.
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

/// `rmw_take(sub, *mut buf, *mut len)` — versucht ein Sample zu lesen.
/// Caller MUSS `rmw_zerodds_buffer_free(buf, len)` aufrufen.
///
/// # Safety
/// Pointer-Validitaet; out_buf/out_len NULL-checked unten.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_take(
    sub: *mut RmwZerodsSubscription,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_big_endian: *mut u8,
) -> i32 {
    if sub.is_null() || out_buf.is_null() || out_len.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let s = unsafe { &*sub };
    let popped = s.inbox.queue.lock().ok().and_then(|mut q| q.pop_front());
    match popped {
        Some((bytes, _repr, be)) if !bytes.is_empty() => {
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
/// Wie zerodds_buffer_free.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_buffer_free(buf: *mut u8, len: usize) {
    // SAFETY: Pass-through.
    unsafe { zerodds::zerodds_buffer_free(buf, len) }
}

// ============================================================================
// Phase-B: Service-Layer + Wait-Sets + Loaning + REP-2009 Type-Hash
// ============================================================================

/// Opaque-Handle: rmw_zerodds Client (request-Pub + reply-Sub auf
/// `<service>_Request` / `<service>_Reply`).
pub struct RmwZerodsClient {
    /// Underlying writer auf `<service>_Request`.
    request_writer: Mutex<*mut zerodds::ZeroDdsWriter>,
    /// Underlying reader auf `<service>_Reply`.
    reply_reader: Mutex<*mut zerodds::ZeroDdsReader>,
    /// Listener-fed inbox for replies (event-driven `has_data` + wait wake).
    reply_inbox: Arc<SubInbox>,
    /// `Arc<SubInbox>` raw pointer handed to the reply reader's data callback.
    reply_cb_ud: *const SubInbox,
    /// Service name (before topic mangling).
    pub service_name: alloc::string::String,
}

/// Opaque-Handle: rmw_zerodds Service (request-Sub + reply-Pub).
pub struct RmwZerodsService {
    /// Underlying reader auf `<service>_Request`.
    request_reader: Mutex<*mut zerodds::ZeroDdsReader>,
    /// Underlying writer auf `<service>_Reply`.
    reply_writer: Mutex<*mut zerodds::ZeroDdsWriter>,
    /// Listener-fed inbox for requests (event-driven `has_data` + wait wake).
    request_inbox: Arc<SubInbox>,
    /// `Arc<SubInbox>` raw pointer handed to the request reader's data callback.
    request_cb_ud: *const SubInbox,
    /// Service name (before topic mangling).
    pub service_name: alloc::string::String,
}

/// `rmw_create_client(node, service, type_name)`.
///
/// # Safety
/// Pointer-Validitaet wie Publisher/Subscription-create.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_client(
    node: *mut RmwZerodsNode,
    service_name: *const c_char,
    type_name: *const c_char,
) -> *mut RmwZerodsClient {
    if node.is_null() || service_name.is_null() || type_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: node NULL-checked.
    let n = unsafe { &*node };
    // SAFETY: ctx pointer from the RmwZerodsNode construct + lives
    // as long as node lives.
    let ctx = unsafe { &*n.ctx };
    // SAFETY: NUL-terminierter C-String, NULL-checked oben.
    let service = match unsafe { CStr::from_ptr(service_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: NUL-terminierter C-String, NULL-checked oben.
    let typ = match unsafe { CStr::from_ptr(type_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    if zerodds_rpc::topic_naming::validate_service_name(&service).is_err() {
        return ptr::null_mut();
    }
    let req_topic = match zerodds_rpc::topic_naming::request_topic_name(&service) {
        Ok(t) => t,
        Err(_) => return ptr::null_mut(),
    };
    let rep_topic = match zerodds_rpc::topic_naming::reply_topic_name(&service) {
        Ok(t) => t,
        Err(_) => return ptr::null_mut(),
    };
    let req_topic_c = match std::ffi::CString::new(req_topic) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let rep_topic_c = match std::ffi::CString::new(rep_topic) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let typ_c = match std::ffi::CString::new(typ) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: NULL-tolerant FFI calls; cleanup on the error path.
    let writer = unsafe {
        zerodds::zerodds_writer_create(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            req_topic_c.as_ptr(),
            typ_c.as_ptr(),
            1, // services are reliable
        )
    };
    // SAFETY: NULL-tolerante FFI; Cleanup-Pfad below.
    let reader = unsafe {
        zerodds::zerodds_reader_create(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            rep_topic_c.as_ptr(),
            typ_c.as_ptr(),
            1,
        )
    };
    if writer.is_null() || reader.is_null() {
        if !writer.is_null() {
            // SAFETY: writer from writer_create.
            unsafe { zerodds::zerodds_writer_destroy(writer) };
        }
        if !reader.is_null() {
            // SAFETY: reader aus reader_create.
            unsafe { zerodds::zerodds_reader_destroy(reader) };
        }
        return ptr::null_mut();
    }
    // Event-driven replies: route the reply reader through a listener inbox so
    // the client's reply is observable via has_data + wakes the executor wait.
    // SAFETY: reader from reader_create.
    let (reply_inbox, reply_cb_ud) = match unsafe { attach_inbox(reader, ctx.notify.clone()) } {
        Some(v) => v,
        None => {
            // SAFETY: tear both endpoints down on listener-registration failure.
            unsafe {
                zerodds::zerodds_writer_destroy(writer);
                zerodds::zerodds_reader_destroy(reader);
            }
            return ptr::null_mut();
        }
    };
    Box::into_raw(Box::new(RmwZerodsClient {
        request_writer: Mutex::new(writer),
        reply_reader: Mutex::new(reader),
        reply_inbox,
        reply_cb_ud,
        service_name: service,
    }))
}

/// `rmw_destroy_client(*mut Client)`.
///
/// # Safety
/// `client` from `rmw_zerodds_create_client` or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_client(client: *mut RmwZerodsClient) -> i32 {
    if client.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box pointer from above.
    let c = unsafe { Box::from_raw(client) };
    // Destroy the reply reader FIRST (stops its listener) before reclaiming the
    // inbox callback ref.
    if let Ok(g) = c.reply_reader.lock() {
        // SAFETY: reader aus create_client.
        unsafe { zerodds::zerodds_reader_destroy(*g) };
    }
    if !c.reply_cb_ud.is_null() {
        // SAFETY: reply_cb_ud is the Arc::into_raw pointer from attach_inbox.
        drop(unsafe { Arc::from_raw(c.reply_cb_ud) });
    }
    if let Ok(g) = c.request_writer.lock() {
        // SAFETY: writer from create_client.
        unsafe { zerodds::zerodds_writer_destroy(*g) };
    }
    RMW_RET_OK
}

/// `rmw_send_request(client, payload, len)`.
///
/// # Safety
/// `client` valid; payload + len lives during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_send_request(
    client: *mut RmwZerodsClient,
    payload: *const u8,
    len: usize,
) -> i32 {
    if client.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let c = unsafe { &*client };
    let writer = match c.request_writer.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: writer from create_client.
    let rc = unsafe { zerodds::zerodds_writer_write(writer, payload, len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

/// `rmw_take_response(client, *mut buf, *mut len)`.
///
/// # Safety
/// Wie rmw_zerodds_take.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_take_response(
    client: *mut RmwZerodsClient,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_big_endian: *mut u8,
) -> i32 {
    if client.is_null() || out_buf.is_null() || out_len.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked; out pointers validated above.
    let c = unsafe { &*client };
    // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
    unsafe { inbox_take(&c.reply_inbox, out_buf, out_len, out_big_endian) }
}

/// `1` if a reply is queued for `client`, `0` if none, negative on a bad handle.
///
/// # Safety
/// `client` must come from `rmw_zerodds_create_client` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_client_has_data(client: *mut RmwZerodsClient) -> i32 {
    if client.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    inbox_has_data(&unsafe { &*client }.reply_inbox)
}

/// `1` if the matching service server is available (the client's request writer
/// has a matched reader AND its reply reader has a matched writer), else `0`.
/// Non-blocking (uses a zero-timeout matched check).
///
/// # Safety
/// `client` must come from `rmw_zerodds_create_client` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_client_server_available(client: *mut RmwZerodsClient) -> i32 {
    if client.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let c = unsafe { &*client };
    let w = match c.request_writer.lock() {
        Ok(g) => *g,
        Err(_) => return 0,
    };
    let r = match c.reply_reader.lock() {
        Ok(g) => *g,
        Err(_) => return 0,
    };
    // SAFETY: writer/reader from create_client; zero timeout = non-blocking peek.
    let w_ok = unsafe { zerodds::zerodds_writer_wait_for_matched(w, 1, 0) } == 0;
    // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
    let r_ok = unsafe { zerodds::zerodds_reader_wait_for_matched(r, 1, 0) } == 0;
    i32::from(w_ok && r_ok)
}

/// `rmw_create_service(node, service, type_name)` — Server-side.
///
/// # Safety
/// Wie rmw_zerodds_create_client.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_service(
    node: *mut RmwZerodsNode,
    service_name: *const c_char,
    type_name: *const c_char,
) -> *mut RmwZerodsService {
    if node.is_null() || service_name.is_null() || type_name.is_null() {
        return ptr::null_mut();
    }
    // SAFETY: node NULL-checked.
    let n = unsafe { &*node };
    // SAFETY: ctx pointer from the RmwZerodsNode construct.
    let ctx = unsafe { &*n.ctx };
    // SAFETY: NUL-terminierter C-String.
    let service = match unsafe { CStr::from_ptr(service_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: NUL-terminierter C-String.
    let typ = match unsafe { CStr::from_ptr(type_name) }.to_str() {
        Ok(s) => s.to_string(),
        Err(_) => return ptr::null_mut(),
    };
    if zerodds_rpc::topic_naming::validate_service_name(&service).is_err() {
        return ptr::null_mut();
    }
    let req_topic = match zerodds_rpc::topic_naming::request_topic_name(&service) {
        Ok(t) => t,
        Err(_) => return ptr::null_mut(),
    };
    let rep_topic = match zerodds_rpc::topic_naming::reply_topic_name(&service) {
        Ok(t) => t,
        Err(_) => return ptr::null_mut(),
    };
    let req_topic_c = match std::ffi::CString::new(req_topic) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let rep_topic_c = match std::ffi::CString::new(rep_topic) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let typ_c = match std::ffi::CString::new(typ) {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    // SAFETY: NULL-tolerante FFI-Calls.
    let reader = unsafe {
        zerodds::zerodds_reader_create(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            req_topic_c.as_ptr(),
            typ_c.as_ptr(),
            1,
        )
    };
    // SAFETY: NULL-tolerante FFI-Calls.
    let writer = unsafe {
        zerodds::zerodds_writer_create(
            ctx.runtime as *mut zerodds::ZeroDdsRuntime,
            rep_topic_c.as_ptr(),
            typ_c.as_ptr(),
            1,
        )
    };
    if writer.is_null() || reader.is_null() {
        if !writer.is_null() {
            // SAFETY: writer from writer_create.
            unsafe { zerodds::zerodds_writer_destroy(writer) };
        }
        if !reader.is_null() {
            // SAFETY: reader aus reader_create.
            unsafe { zerodds::zerodds_reader_destroy(reader) };
        }
        return ptr::null_mut();
    }
    // Event-driven requests: route the request reader through a listener inbox.
    // SAFETY: reader from reader_create.
    let (request_inbox, request_cb_ud) = match unsafe { attach_inbox(reader, ctx.notify.clone()) } {
        Some(v) => v,
        None => {
            // SAFETY: tear both endpoints down on listener-registration failure.
            unsafe {
                zerodds::zerodds_writer_destroy(writer);
                zerodds::zerodds_reader_destroy(reader);
            }
            return ptr::null_mut();
        }
    };
    Box::into_raw(Box::new(RmwZerodsService {
        request_reader: Mutex::new(reader),
        reply_writer: Mutex::new(writer),
        request_inbox,
        request_cb_ud,
        service_name: service,
    }))
}

/// `rmw_destroy_service(*mut Service)`.
///
/// # Safety
/// `service` from `rmw_zerodds_create_service` or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_service(service: *mut RmwZerodsService) -> i32 {
    if service.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box-Pointer.
    let s = unsafe { Box::from_raw(service) };
    // Destroy the request reader FIRST (stops its listener), then reclaim the
    // inbox callback ref.
    if let Ok(g) = s.request_reader.lock() {
        // SAFETY: reader aus create_service.
        unsafe { zerodds::zerodds_reader_destroy(*g) };
    }
    if !s.request_cb_ud.is_null() {
        // SAFETY: request_cb_ud is the Arc::into_raw pointer from attach_inbox.
        drop(unsafe { Arc::from_raw(s.request_cb_ud) });
    }
    if let Ok(g) = s.reply_writer.lock() {
        // SAFETY: writer from create_service.
        unsafe { zerodds::zerodds_writer_destroy(*g) };
    }
    RMW_RET_OK
}

/// `rmw_take_request(service, *mut buf, *mut len)`.
///
/// # Safety
/// Pointer-Validitaet wie rmw_zerodds_take.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_take_request(
    service: *mut RmwZerodsService,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_big_endian: *mut u8,
) -> i32 {
    if service.is_null() || out_buf.is_null() || out_len.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked; out pointers validated above.
    let s = unsafe { &*service };
    // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
    unsafe { inbox_take(&s.request_inbox, out_buf, out_len, out_big_endian) }
}

/// `1` if a request is queued for `service`, `0` if none, negative on bad handle.
///
/// # Safety
/// `service` must come from `rmw_zerodds_create_service` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_service_has_data(service: *mut RmwZerodsService) -> i32 {
    if service.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    inbox_has_data(&unsafe { &*service }.request_inbox)
}

/// `rmw_send_response(service, payload, len)`.
///
/// # Safety
/// Wie rmw_zerodds_publish.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_send_response(
    service: *mut RmwZerodsService,
    payload: *const u8,
    len: usize,
) -> i32 {
    if service.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let s = unsafe { &*service };
    let writer = match s.reply_writer.lock() {
        Ok(g) => *g,
        Err(_) => return RMW_RET_ERROR,
    };
    // SAFETY: writer from create_service.
    let rc = unsafe { zerodds::zerodds_writer_write(writer, payload, len) };
    if rc == 0 { RMW_RET_OK } else { RMW_RET_ERROR }
}

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
/// `ws` + `sub` muessen valid sein.
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
/// [`WaitNotify::wait_until`] — no spin loop, no fixed-tick poll. The generation
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
/// `pub_` valid; `ptr`/`len` aus rmw_zerodds_borrow_loaned_message.
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
/// Wie rmw_zerodds_publish_loaned_message.
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
    // SAFETY: NUL-terminierter C-String.
    let s = match unsafe { CStr::from_ptr(type_str) }.to_bytes_with_nul() {
        b if b.len() > 1 => &b[..b.len() - 1],
        _ => return RMW_RET_INVALID_ARGUMENT,
    };
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(s);
    let result = h.finalize();
    // SAFETY: out_hash NULL-checked; Caller-Kontrakt: 32-byte-Buffer.
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

/// Sets the rmw `on_new_request` callback on a service (request inbox).
///
/// # Safety
/// `service` must come from `rmw_zerodds_create_service` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_service_set_event_callback(
    service: *mut RmwZerodsService,
    cb: Option<RmwEventCallback>,
    user_data: *const c_void,
) -> i32 {
    if service.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    inbox_set_event(&unsafe { &*service }.request_inbox, cb, user_data);
    RMW_RET_OK
}

/// Sets the rmw `on_new_response` callback on a client (reply inbox).
///
/// # Safety
/// `client` must come from `rmw_zerodds_create_client` or be NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_client_set_event_callback(
    client: *mut RmwZerodsClient,
    cb: Option<RmwEventCallback>,
    user_data: *const c_void,
) -> i32 {
    if client.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    inbox_set_event(&unsafe { &*client }.reply_inbox, cb, user_data);
    RMW_RET_OK
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // tests may use unwrap.
mod tests {
    use super::*;

    #[test]
    fn tbf_env_parse_global_and_per_topic() {
        // Bare number = global default for every topic.
        assert_eq!(parse_tbf_env("0.1", "/scan"), Some(100_000_000));
        assert_eq!(parse_tbf_env("0.1", "/anything"), Some(100_000_000));
        // Per-topic override wins over the global default.
        assert_eq!(
            parse_tbf_env("0.1,/scan=0.5", "/scan"),
            Some(500_000_000),
            "per-topic must override global"
        );
        assert_eq!(
            parse_tbf_env("0.1,/scan=0.5", "/image"),
            Some(100_000_000),
            "non-matching topic falls back to global"
        );
        // No global, only a per-topic entry → other topics get nothing.
        assert_eq!(parse_tbf_env("/scan=0.2", "/scan"), Some(200_000_000));
        assert_eq!(parse_tbf_env("/scan=0.2", "/image"), None);
        // Zero / negative / junk = disabled.
        assert_eq!(parse_tbf_env("0", "/x"), None);
        assert_eq!(parse_tbf_env("-1", "/x"), None);
        assert_eq!(parse_tbf_env("nonsense", "/x"), None);
        assert_eq!(parse_tbf_env("", "/x"), None);
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
        let r = unsafe { rmw_zerodds_destroy_client(ptr::null_mut()) };
        assert_eq!(r, RMW_RET_INVALID_ARGUMENT);
    }

    #[test]
    fn destroy_service_null_returns_invalid_argument() {
        // SAFETY: NULL-tolerant.
        let r = unsafe { rmw_zerodds_destroy_service(ptr::null_mut()) };
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
        // SAFETY: ein Null-Arg pruefen.
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
            unsafe { rmw_zerodds_create_publisher(node_pub, typ.as_ptr(), topic.as_ptr(), 1) };
        assert!(!publ.is_null());
        let sub =
            // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
            unsafe { rmw_zerodds_create_subscription(node_sub, typ.as_ptr(), topic.as_ptr(), 1) };
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

    /// In-process service round-trip at the bridge byte level: client
    /// `send_request` → service inbox (`service_has_data`) → `take_request` →
    /// `send_response` → client inbox (`client_has_data`) → `take_response`.
    /// Verifies the event-driven service delivery path (listener inboxes).
    #[test]
    fn service_request_reply_roundtrip_inprocess() {
        let svc = std::ffi::CString::new("rmw_srv_it").unwrap();
        let typ = std::ffi::CString::new("test::Srv").unwrap();
        let name = std::ffi::CString::new("n").unwrap();
        let ns = std::ffi::CString::new("/").unwrap();

        // SAFETY: FFI bring-up on a private domain with valid C strings.
        let ctx_cli = unsafe { rmw_zerodds_init(92) };
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        let ctx_srv = unsafe { rmw_zerodds_init(92) };
        assert!(!ctx_cli.is_null() && !ctx_srv.is_null());
        // SAFETY: discovery before endpoints.
        unsafe {
            let rc = (*ctx_cli).runtime as *mut zerodds::ZeroDdsRuntime;
            let rs = (*ctx_srv).runtime as *mut zerodds::ZeroDdsRuntime;
            assert_eq!(zerodds::zerodds_runtime_wait_for_peers(rc, 1, 5_000), 0);
            assert_eq!(zerodds::zerodds_runtime_wait_for_peers(rs, 1, 5_000), 0);
        }
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        let node_cli = unsafe { rmw_zerodds_create_node(ctx_cli, name.as_ptr(), ns.as_ptr()) };
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        let node_srv = unsafe { rmw_zerodds_create_node(ctx_srv, name.as_ptr(), ns.as_ptr()) };
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        let client = unsafe { rmw_zerodds_create_client(node_cli, svc.as_ptr(), typ.as_ptr()) };
        // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
        let service = unsafe { rmw_zerodds_create_service(node_srv, svc.as_ptr(), typ.as_ptr()) };
        assert!(!client.is_null(), "client create");
        assert!(!service.is_null(), "service create");

        // Send requests until the service inbox sees one (discovery + match).
        let req: [u8; 6] = [1, 2, 3, 4, 5, 6];
        let mut got_req = false;
        for _ in 0..50 {
            // SAFETY: client valid; req lives across the call.
            let _ = unsafe { rmw_zerodds_send_request(client, req.as_ptr(), req.len()) };
            // SAFETY: service valid.
            if unsafe { rmw_zerodds_service_has_data(service) } == 1 {
                got_req = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(
            got_req,
            "service must receive the request (event-driven inbox)"
        );

        let mut rbuf: *mut u8 = ptr::null_mut();
        let mut rlen: usize = 0;
        // SAFETY: valid out pointers; service valid.
        assert_eq!(
            unsafe { rmw_zerodds_take_request(service, &mut rbuf, &mut rlen, &mut 0u8) },
            RMW_RET_OK
        );
        assert!(!rbuf.is_null() && rlen >= 1, "request bytes delivered");
        // SAFETY: from take.
        unsafe { rmw_zerodds_buffer_free(rbuf, rlen) };

        // Server replies until the client inbox sees it.
        let rep: [u8; 3] = [9, 8, 7];
        let mut got_rep = false;
        for _ in 0..50 {
            // SAFETY: service valid; rep lives across the call.
            let _ = unsafe { rmw_zerodds_send_response(service, rep.as_ptr(), rep.len()) };
            // SAFETY: client valid.
            if unsafe { rmw_zerodds_client_has_data(client) } == 1 {
                got_rep = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(
            got_rep,
            "client must receive the reply (event-driven inbox)"
        );

        let mut pbuf: *mut u8 = ptr::null_mut();
        let mut plen: usize = 0;
        // SAFETY: valid out pointers; client valid.
        assert_eq!(
            unsafe { rmw_zerodds_take_response(client, &mut pbuf, &mut plen, &mut 0u8) },
            RMW_RET_OK
        );
        assert!(!pbuf.is_null() && plen >= 1, "reply bytes delivered");
        // SAFETY: from take.
        unsafe { rmw_zerodds_buffer_free(pbuf, plen) };

        // SAFETY: all handles valid + owned.
        unsafe {
            let _ = rmw_zerodds_destroy_client(client);
            let _ = rmw_zerodds_destroy_service(service);
            let _ = rmw_zerodds_destroy_node(node_cli);
            let _ = rmw_zerodds_destroy_node(node_srv);
            let _ = rmw_zerodds_shutdown(ctx_cli);
            let _ = rmw_zerodds_shutdown(ctx_srv);
            let _ = rmw_zerodds_context_fini(ctx_cli);
            let _ = rmw_zerodds_context_fini(ctx_srv);
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
            unsafe { rmw_zerodds_create_publisher(node_pub, typ.as_ptr(), topic.as_ptr(), 1) };
        let sub =
            // SAFETY: validity upheld by the surrounding contract (NULL/bounds checked where applicable).
            unsafe { rmw_zerodds_create_subscription(node_sub, typ.as_ptr(), topic.as_ptr(), 1) };
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
