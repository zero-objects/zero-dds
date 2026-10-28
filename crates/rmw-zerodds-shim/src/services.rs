// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! ROS 2 services and clients in the two forms the other RMWs put on the
//! wire (`zerodds-ros2-bridge-1.0.md` §4.2).
//!
//! Both use the topics `rq/<service>Request` and `rr/<service>Reply` with
//! the types `<pkg>::srv::dds_::<Srv>_Request_` / `_Response_`. They
//! correlate differently:
//!
//! * **Cyclone form** (rmw_cyclonedds_cpp 1.3.4, `serdata.hpp`
//!   `cdds_request_header_t`): the CDR body is preceded by `u64 guid` and
//!   `i64 seq`; the server echoes both. The client's writer and reader carry
//!   `clientid=<id>;` in `USER_DATA`, the server's `serviceid=<id>;`, and a
//!   server waits up to 100 ms for the client's reply reader with the same
//!   `clientid` before it answers (`check_for_response_reader`).
//! * **Related-identity form** (rmw_fastrtps 6.2.7, `rmw_request.cpp`,
//!   `rmw_response.cpp`): no header in the payload. The request's inline
//!   QoS carries a related sample identity with the GUID of the client's
//!   reply reader; the reply's names that reader and the RTPS sequence
//!   number of the request. Fast DDS 2.6 reads it only as its
//!   vendor-specific PID 0x800f; this stack also writes 0x0083
//!   (DDS-RPC 1.0 §7.8.2) and uses the form between its own nodes.
//!
//! A server answers each request in the form it came in. A client picks the
//! form per request from the vendor of the servers its request writer is
//! matched with: Cyclone form when every one of them is Cyclone DDS, the
//! related-identity form otherwise.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ffi::{c_char, c_void};
use core::ptr;
use std::ffi::CStr;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{DcpsRuntime, UserSample};
use zerodds_rtps::inline_qos::SampleIdentityBytes;
use zerodds_rtps::wire_types::{EntityId, Guid, VendorId};

use crate::qos::{self, RmwZerodsQos, Side};
use crate::{
    InboxEvent, NodeGraph, RMW_RET_ERROR, RMW_RET_INVALID_ARGUMENT, RMW_RET_OK, RMW_RET_TIMEOUT,
    RmwEventCallback, RmwZerodsNode, WaitNotify, encapsulation_header, graph_register_endpoint,
    graph_unregister_endpoint,
};

/// How long a server waits for a client's reply reader to be matched before
/// it answers: rmw_cyclonedds_cpp waits 100 ms, rmw_fastrtps_cpp the reply
/// writer's `max_blocking_time` (100 ms by default).
const REPLY_READER_WAIT: Duration = Duration::from_millis(100);

/// Requests a server holds for their reply, and related-identity requests a
/// client holds for their reply. Past it the oldest is dropped and logged:
/// a peer that never answers must not grow the table without bound.
const PENDING_CAP: usize = 4096;

/// Length of the Cyclone form's header: `u64 guid`, `i64 seq`.
const CYCLONE_HEADER: usize = 16;

/// The samples a reader has queued, and what an arrival wakes.
struct Inbox {
    rx: Mutex<Receiver<UserSample>>,
    queue: Mutex<VecDeque<UserSample>>,
    event: Arc<Mutex<Option<InboxEvent>>>,
    /// KEEP_LAST depth of the reader (DDS 1.4 §2.2.3.18); `None` = KEEP_ALL.
    keep_last: Option<usize>,
}

impl Inbox {
    fn new(
        rt: &DcpsRuntime,
        reader: EntityId,
        rx: Receiver<UserSample>,
        keep_last: Option<usize>,
        notify: Arc<WaitNotify>,
    ) -> Self {
        let event: Arc<Mutex<Option<InboxEvent>>> = Arc::new(Mutex::new(None));
        let ev = Arc::clone(&event);
        rt.set_user_reader_arrival_hook(
            reader,
            Some(Arc::new(move || {
                notify.notify();
                let cb = ev.lock().ok().and_then(|g| *g);
                if let Some(e) = cb {
                    (e.cb)(e.ud as *const c_void, 1);
                }
            })),
        );
        Self {
            rx: Mutex::new(rx),
            queue: Mutex::new(VecDeque::new()),
            event,
            keep_last,
        }
    }

    /// Moves what arrived from the channel into the queue, trimmed to the
    /// KEEP_LAST depth.
    fn drain(&self) {
        let (Ok(rx), Ok(mut q)) = (self.rx.lock(), self.queue.lock()) else {
            return;
        };
        while let Ok(s) = rx.try_recv() {
            if matches!(s, UserSample::Alive { .. }) {
                q.push_back(s);
            }
        }
        if let Some(depth) = self.keep_last {
            while q.len() > depth.max(1) {
                q.pop_front();
            }
        }
    }

    fn pop(&self) -> Option<UserSample> {
        self.drain();
        self.queue.lock().ok().and_then(|mut q| q.pop_front())
    }

    fn has_data(&self) -> bool {
        self.drain();
        self.queue.lock().map(|q| !q.is_empty()).unwrap_or(false)
    }

    fn set_event(&self, cb: Option<RmwEventCallback>, ud: *const c_void) {
        let ev = cb.map(|c| InboxEvent {
            cb: c,
            ud: ud as usize,
        });
        if let Ok(mut g) = self.event.lock() {
            *g = ev;
        }
        if let Some(ev) = ev {
            self.drain();
            let backlog = self.queue.lock().map(|q| q.len()).unwrap_or(0);
            if backlog > 0 {
                (ev.cb)(ev.ud as *const c_void, backlog);
            }
        }
    }
}

fn guid16(rt: &DcpsRuntime, eid: EntityId) -> [u8; 16] {
    Guid::new(rt.guid_prefix, eid).to_bytes()
}

fn prefix_of(guid: &[u8; 16]) -> zerodds_rtps::wire_types::GuidPrefix {
    let mut p = [0u8; 12];
    p.copy_from_slice(&guid[..12]);
    zerodds_rtps::wire_types::GuidPrefix::from_bytes(p)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The value of `key` in a `k=v;k=v;` `USER_DATA` string, as
/// rmw_cyclonedds_cpp's `get_user_data_key` reads it.
fn user_data_key(user_data: &[u8], key: &str) -> Option<String> {
    let text = core::str::from_utf8(user_data).ok()?;
    text.split(';').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| v.to_string())
    })
}

/// A 16-byte GUID as the Cyclone form's `u64 guid` (FNV-1a-64): the server
/// echoes it, only the client compares it.
fn cyclone_id(guid: &[u8; 16]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in guid {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Strips the `CDR_LE` encapsulation header off the stream the C layer
/// hands over; the writer stamps its own.
fn body_of(stream: &[u8]) -> Option<&[u8]> {
    (stream.len() >= 4 && stream[0..2] == [0x00, 0x01]).then(|| &stream[4..])
}

/// `[encap][body]` into a heap buffer the C layer frees with
/// `rmw_zerodds_buffer_free`.
///
/// # Safety
/// The out pointers must be valid.
unsafe fn hand_out(
    representation: u8,
    big_endian: bool,
    body: &[u8],
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_big_endian: *mut u8,
) {
    let mut v = Vec::with_capacity(4 + body.len());
    v.extend_from_slice(&encapsulation_header(representation, u8::from(big_endian)));
    v.extend_from_slice(body);
    let boxed = v.into_boxed_slice();
    let len = boxed.len();
    let p = alloc::boxed::Box::into_raw(boxed).cast::<u8>();
    // SAFETY: valid out pointers by the caller's contract.
    unsafe {
        *out_buf = p;
        *out_len = len;
        if !out_big_endian.is_null() {
            *out_big_endian = u8::from(big_endian);
        }
    }
}

/// Reads the `u64 guid` and `i64 seq` of the Cyclone form.
fn cyclone_header(payload: &[u8], big_endian: bool) -> Option<(u64, i64)> {
    if payload.len() < CYCLONE_HEADER {
        return None;
    }
    let mut g = [0u8; 8];
    let mut s = [0u8; 8];
    g.copy_from_slice(&payload[0..8]);
    s.copy_from_slice(&payload[8..16]);
    Some(if big_endian {
        (u64::from_be_bytes(g), i64::from_be_bytes(s))
    } else {
        (u64::from_le_bytes(g), i64::from_le_bytes(s))
    })
}

/// Inserts into a bounded table; past [`PENDING_CAP`] the oldest entry goes,
/// logged.
fn insert_bounded<K: Ord + Clone, V>(
    map: &mut BTreeMap<K, (Instant, V)>,
    key: K,
    value: V,
    what: &str,
) {
    if map.len() >= PENDING_CAP {
        if let Some(oldest) = map
            .iter()
            .min_by_key(|(_, (t, _))| *t)
            .map(|(k, _)| k.clone())
        {
            map.remove(&oldest);
            #[allow(clippy::print_stderr)]
            {
                eprintln!("[rmw_zerodds] {what}: {PENDING_CAP} unanswered, dropping the oldest");
            }
        }
    }
    map.insert(key, (Instant::now(), value));
}

/// The endpoints of a client or a service.
struct Endpoints {
    rt: Arc<DcpsRuntime>,
    writer: EntityId,
    reader: EntityId,
    inbox: Inbox,
    /// The context's node graph: both endpoints are announced in
    /// `ros_discovery_info` under their node, as the other RMWs announce
    /// their service endpoints, so `ros2 service list`/`ros2 node info` on
    /// any RMW map them to the node.
    graph: Arc<NodeGraph>,
}

impl Endpoints {
    /// Creates the writer on `writer_topic` and the reader on `reader_topic`,
    /// both with `user_data`.
    #[allow(clippy::too_many_arguments)]
    fn create(
        node: &RmwZerodsNode,
        writer_topic: &str,
        writer_type: &str,
        reader_topic: &str,
        reader_type: &str,
        requested: &RmwZerodsQos,
        user_data: &dyn Fn(&[u8; 16]) -> Vec<u8>,
        actual: *mut RmwZerodsQos,
    ) -> Option<Self> {
        // SAFETY: the node's context outlives the node.
        let ctx = unsafe { &*node.ctx };
        // SAFETY: the context's runtime handle is live for the context.
        let rt = unsafe { zerodds::runtime_arc(ctx.runtime as *const zerodds::ZeroDdsRuntime) }?;
        let wq = qos::resolve(requested, Side::Publisher).ok()?;
        let rq = qos::resolve(requested, Side::Subscription).ok()?;
        if !actual.is_null() {
            // SAFETY: caller-provided out pointer.
            unsafe { *actual = wq.actual() };
        }
        // The id both endpoints announce goes into their USER_DATA, so it is
        // fixed before either is registered: the participant prefix and a
        // process-wide counter, unique per client or service.
        static NEXT_ID: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(1);
        let wcfg =
            zerodds::user_writer_config(writer_topic, writer_type, &wq.endpoint(), Vec::new())
                .ok()?;
        let id_seed = {
            let mut seed = [0u8; 16];
            seed[..12].copy_from_slice(&rt.guid_prefix.to_bytes());
            seed[12..].copy_from_slice(&NEXT_ID.fetch_add(1, Ordering::Relaxed).to_be_bytes());
            seed
        };
        let ud = user_data(&id_seed);
        let wcfg = zerodds_dcps::runtime::UserWriterConfig {
            user_data: ud.clone(),
            ..wcfg
        };
        let writer = rt.register_user_writer_kind(wcfg, false).ok()?;
        // ROS types are FINAL, as the publishers state (`CDR_LE`, no DHEADER).
        if zerodds::apply_user_writer_qos(&rt, writer, &wq.endpoint()).is_err()
            || rt
                .set_user_writer_wire_extensibility(
                    writer,
                    zerodds_types::qos::ExtensibilityForRepr::Final,
                )
                .is_err()
        {
            rt.remove_user_writer(writer);
            return None;
        }
        let Ok((rcfg, keep_last)) =
            zerodds::user_reader_config(reader_topic, reader_type, &rq.endpoint(), ud)
        else {
            rt.remove_user_writer(writer);
            return None;
        };
        let Ok((reader, rx)) = rt.register_user_reader_kind(rcfg, false) else {
            rt.remove_user_writer(writer);
            return None;
        };
        let inbox = Inbox::new(&rt, reader, rx, keep_last, Arc::clone(&ctx.notify));
        let node_id = (node.identity.namespace.clone(), node.identity.name.clone());
        graph_register_endpoint(&ctx.graph, &node_id, true, guid16(&rt, writer));
        graph_register_endpoint(&ctx.graph, &node_id, false, guid16(&rt, reader));
        Some(Self {
            rt,
            writer,
            reader,
            inbox,
            graph: Arc::clone(&ctx.graph),
        })
    }
}

impl Drop for Endpoints {
    fn drop(&mut self) {
        graph_unregister_endpoint(&self.graph, &guid16(&self.rt, self.writer));
        graph_unregister_endpoint(&self.graph, &guid16(&self.rt, self.reader));
        self.rt.set_user_reader_arrival_hook(self.reader, None);
        self.rt.remove_user_reader(self.reader);
        self.rt.remove_user_writer(self.writer);
    }
}

fn names(
    node: *mut RmwZerodsNode,
    service_name: *const c_char,
    request_type: *const c_char,
    response_type: *const c_char,
    qos: *const RmwZerodsQos,
) -> Option<(
    &'static RmwZerodsNode,
    String,
    String,
    String,
    String,
    String,
    RmwZerodsQos,
)> {
    if node.is_null()
        || service_name.is_null()
        || request_type.is_null()
        || response_type.is_null()
        || qos.is_null()
    {
        return None;
    }
    // SAFETY: NULL-checked; the node outlives its clients and services.
    let n: &'static RmwZerodsNode = unsafe { &*node };
    // SAFETY: NUL-terminated C strings by the caller's contract.
    let (svc, req_t, rep_t) = unsafe {
        (
            CStr::from_ptr(service_name).to_str().ok()?.to_string(),
            CStr::from_ptr(request_type).to_str().ok()?.to_string(),
            CStr::from_ptr(response_type).to_str().ok()?.to_string(),
        )
    };
    // SAFETY: NULL-checked.
    let requested = unsafe { *qos };
    // SAFETY: the node's context outlives the node.
    let ctx = unsafe { &*n.ctx };
    let requested = ctx.ros2.effective_qos(&svc, &requested);
    let (rq, rr) = ctx
        .ros2
        .dds_service_topics(&svc, requested.avoid_ros_namespace_conventions != 0);
    Some((n, svc, rq, rr, req_t, rep_t, requested))
}

// ============================================================================
// Client
// ============================================================================

/// A ROS 2 service client: request writer on `rq/…Request`, reply reader on
/// `rr/…Reply`.
pub struct RmwZerodsClient {
    ep: Endpoints,
    writer_guid: [u8; 16],
    reader_guid: [u8; 16],
    /// Cyclone form: the `guid` the server echoes.
    cyclone_id: u64,
    /// rmw sequence ids, from 1.
    next_seq: AtomicI64,
    /// Related-identity form: the RTPS sequence number of a request → its
    /// rmw sequence id.
    in_flight: Mutex<BTreeMap<i64, (Instant, i64)>>,
    /// The ROS service name.
    pub service_name: String,
}

impl RmwZerodsClient {
    /// Whether every server the request writer is matched with is Cyclone
    /// DDS (and there is one).
    fn servers_are_cyclone(&self) -> bool {
        let readers = self.ep.rt.user_writer_matched_reader_guids(self.ep.writer);
        !readers.is_empty()
            && readers
                .iter()
                .all(|g| self.ep.rt.participant_vendor(prefix_of(g)) == Some(VendorId::CYCLONE))
    }
}

/// `rmw_create_client`: `service_name` is the ROS name (`/add_two_ints`),
/// the types are the DDS type names of request and response
/// (`example_interfaces::srv::dds_::AddTwoInts_Request_`). `actual` (may be
/// NULL) gets the QoS the endpoints were created with.
///
/// # Safety
/// `node` live; strings NUL-terminated; `qos` valid; `actual` NULL or valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_client(
    node: *mut RmwZerodsNode,
    service_name: *const c_char,
    request_type: *const c_char,
    response_type: *const c_char,
    qos: *const RmwZerodsQos,
    actual: *mut RmwZerodsQos,
) -> *mut RmwZerodsClient {
    // SAFETY: the caller's contract, passed on; no type hash.
    unsafe {
        rmw_zerodds_create_client_hashed(
            node,
            service_name,
            request_type,
            response_type,
            qos,
            actual,
            ptr::null(),
        )
    }
}

/// [`rmw_zerodds_create_client`] whose two endpoints announce the
/// service's type hash after the client id (`clientid=…;typehash=…;`),
/// as `rmw_cyclonedds_cpp` from Iron on writes it.
///
/// # Safety
/// As [`rmw_zerodds_create_client`]; `type_hash` NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_client_hashed(
    node: *mut RmwZerodsNode,
    service_name: *const c_char,
    request_type: *const c_char,
    response_type: *const c_char,
    qos: *const RmwZerodsQos,
    actual: *mut RmwZerodsQos,
    type_hash: *const c_char,
) -> *mut RmwZerodsClient {
    // SAFETY: the caller's contract.
    let Some(hash) = (unsafe { crate::type_hash_user_data(type_hash) }) else {
        return ptr::null_mut();
    };
    let Some((n, svc, rq, rr, req_t, rep_t, requested)) =
        names(node, service_name, request_type, response_type, qos)
    else {
        return ptr::null_mut();
    };
    let Some(ep) = Endpoints::create(
        n,
        &rq,
        &req_t,
        &rr,
        &rep_t,
        &requested,
        &|seed| {
            let mut ud = format!("clientid={};", hex(seed)).into_bytes();
            ud.extend_from_slice(&hash);
            ud
        },
        actual,
    ) else {
        return ptr::null_mut();
    };
    let writer_guid = guid16(&ep.rt, ep.writer);
    let reader_guid = guid16(&ep.rt, ep.reader);
    alloc::boxed::Box::into_raw(alloc::boxed::Box::new(RmwZerodsClient {
        cyclone_id: cyclone_id(&writer_guid),
        writer_guid,
        reader_guid,
        ep,
        next_seq: AtomicI64::new(1),
        in_flight: Mutex::new(BTreeMap::new()),
        service_name: svc,
    }))
}

/// `rmw_destroy_client`.
///
/// # Safety
/// `client` from `rmw_zerodds_create_client` or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_client(client: *mut RmwZerodsClient) -> i32 {
    if client.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box pointer from create.
    drop(unsafe { alloc::boxed::Box::from_raw(client) });
    RMW_RET_OK
}

/// `rmw_send_request`: `stream` is `[CDR_LE encapsulation][body]`; the rmw
/// sequence id of the request goes to `out_seq`.
///
/// # Safety
/// `client` valid; `stream[..len]` readable; `out_seq` valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_send_request(
    client: *mut RmwZerodsClient,
    stream: *const u8,
    len: usize,
    out_seq: *mut i64,
) -> i32 {
    if client.is_null() || stream.is_null() || out_seq.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked; readable by the caller's contract.
    let (c, stream) = unsafe { (&*client, core::slice::from_raw_parts(stream, len)) };
    let Some(body) = body_of(stream) else {
        return RMW_RET_ERROR;
    };
    let seq = c.next_seq.fetch_add(1, Ordering::Relaxed);
    let written = if c.servers_are_cyclone() {
        let mut payload = Vec::with_capacity(CYCLONE_HEADER + body.len());
        payload.extend_from_slice(&c.cyclone_id.to_le_bytes());
        payload.extend_from_slice(&seq.to_le_bytes());
        payload.extend_from_slice(body);
        c.ep.rt.write_user_sample_borrowed(c.ep.writer, &payload)
    } else {
        // Fast DDS reads the reply reader's GUID from here; its sequence
        // number is unknown (`SequenceNumber_t::unknown()`, high -1, low 0).
        let related = SampleIdentityBytes::new(c.reader_guid, 0xFFFF_FFFF_0000_0000);
        let Ok(mut in_flight) = c.in_flight.lock() else {
            return RMW_RET_ERROR;
        };
        match c
            .ep
            .rt
            .write_user_sample_related(c.ep.writer, body, related)
        {
            Ok(sn) => {
                insert_bounded(&mut in_flight, sn, seq, "client requests");
                Ok(())
            }
            Err(e) => Err(e),
        }
    };
    if written.is_err() {
        return RMW_RET_ERROR;
    }
    // SAFETY: NULL-checked.
    unsafe { *out_seq = seq };
    RMW_RET_OK
}

/// `rmw_take_response`: the next reply to this client as
/// `[encapsulation][body]` and its request's rmw sequence id. A NULL buffer
/// means none; replies to other clients are skipped.
///
/// # Safety
/// `client` valid; out pointers valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_take_response(
    client: *mut RmwZerodsClient,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_big_endian: *mut u8,
    out_seq: *mut i64,
) -> i32 {
    if client.is_null() || out_buf.is_null() || out_len.is_null() || out_seq.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let c = unsafe { &*client };
    // SAFETY: valid out pointers.
    unsafe {
        *out_buf = ptr::null_mut();
        *out_len = 0;
    }
    while let Some(s) = c.ep.inbox.pop() {
        let UserSample::Alive {
            payload,
            representation,
            big_endian,
            related_sample_identity,
            ..
        } = s
        else {
            continue;
        };
        let payload = payload.as_ref();
        let (seq, body) = if let Some(rel) = related_sample_identity {
            if rel.writer_guid != c.reader_guid && rel.writer_guid != c.writer_guid {
                continue;
            }
            let sn = rel.sequence_number as i64;
            let Some((_, seq)) = c.in_flight.lock().ok().and_then(|mut m| m.remove(&sn)) else {
                continue;
            };
            (seq, payload)
        } else {
            match cyclone_header(payload, big_endian) {
                Some((guid, seq)) if guid == c.cyclone_id => (seq, &payload[CYCLONE_HEADER..]),
                _ => continue,
            }
        };
        // SAFETY: valid out pointers.
        unsafe {
            hand_out(
                representation,
                big_endian,
                body,
                out_buf,
                out_len,
                out_big_endian,
            );
            *out_seq = seq;
        }
        return RMW_RET_OK;
    }
    RMW_RET_OK
}

/// `1` if a reply is queued for `client` (possibly one for another client of
/// the service, which a take skips), `0` if none.
///
/// # Safety
/// `client` from `rmw_zerodds_create_client` or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_client_has_data(client: *mut RmwZerodsClient) -> i32 {
    if client.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    i32::from(unsafe { &*client }.ep.inbox.has_data())
}

/// `1` if a server is available: the request writer has a matched reader
/// and the reply reader a matched writer.
///
/// # Safety
/// `client` from `rmw_zerodds_create_client` or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_client_server_available(client: *mut RmwZerodsClient) -> i32 {
    if client.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let c = unsafe { &*client };
    i32::from(
        c.ep.rt.user_writer_matched_count(c.ep.writer) > 0
            && c.ep.rt.user_reader_matched_count(c.ep.reader) > 0,
    )
}

/// The client's GID: the GUID of its request writer, the `writer_guid` a
/// server's `rmw_request_id_t` names (16 bytes into `out`).
///
/// # Safety
/// `client` from `rmw_zerodds_create_client` or NULL; `out` writable for
/// 16 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_client_gid(client: *mut RmwZerodsClient, out: *mut u8) -> i32 {
    if client.is_null() || out.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: both NULL-checked; `out` holds 16 bytes per the caller.
    unsafe { core::ptr::copy_nonoverlapping((*client).writer_guid.as_ptr(), out, 16) };
    RMW_RET_OK
}

/// Sets the rmw `on_new_response` callback.
///
/// # Safety
/// `client` from `rmw_zerodds_create_client` or NULL.
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
    unsafe { &*client }.ep.inbox.set_event(cb, user_data);
    RMW_RET_OK
}

// ============================================================================
// Service
// ============================================================================

/// How the reply to a request goes out.
#[derive(Debug, Clone, Copy)]
enum Reply {
    /// Cyclone form: echo `guid` and `seq`; the client is the one whose
    /// reply reader carries the request writer's `clientid`.
    Cyclone {
        guid: u64,
        seq: i64,
        request_writer: [u8; 16],
    },
    /// Related-identity form: the reply names `related`; its writer GUID is
    /// the client's reply reader when the client named one.
    Related {
        related: SampleIdentityBytes,
        request_writer: [u8; 16],
    },
}

/// The request id rcl holds: writer GUID and sequence number.
type RequestId = ([u8; 16], i64);

/// A ROS 2 service: request reader on `rq/…Request`, reply writer on
/// `rr/…Reply`.
pub struct RmwZerodsService {
    ep: Endpoints,
    /// Requests taken and not answered yet, by the request id rcl holds.
    pending: Mutex<BTreeMap<RequestId, (Instant, Reply)>>,
    /// The ROS service name.
    pub service_name: String,
}

/// `rmw_create_service`; arguments as [`rmw_zerodds_create_client`].
///
/// # Safety
/// As [`rmw_zerodds_create_client`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_service(
    node: *mut RmwZerodsNode,
    service_name: *const c_char,
    request_type: *const c_char,
    response_type: *const c_char,
    qos: *const RmwZerodsQos,
    actual: *mut RmwZerodsQos,
) -> *mut RmwZerodsService {
    // SAFETY: the caller's contract, passed on; no type hash.
    unsafe {
        rmw_zerodds_create_service_hashed(
            node,
            service_name,
            request_type,
            response_type,
            qos,
            actual,
            ptr::null(),
        )
    }
}

/// [`rmw_zerodds_create_service`] whose two endpoints announce the
/// service's type hash after the service id, as
/// [`rmw_zerodds_create_client_hashed`].
///
/// # Safety
/// As [`rmw_zerodds_create_service`]; `type_hash` NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_create_service_hashed(
    node: *mut RmwZerodsNode,
    service_name: *const c_char,
    request_type: *const c_char,
    response_type: *const c_char,
    qos: *const RmwZerodsQos,
    actual: *mut RmwZerodsQos,
    type_hash: *const c_char,
) -> *mut RmwZerodsService {
    // SAFETY: the caller's contract.
    let Some(hash) = (unsafe { crate::type_hash_user_data(type_hash) }) else {
        return ptr::null_mut();
    };
    let Some((n, svc, rq, rr, req_t, rep_t, requested)) =
        names(node, service_name, request_type, response_type, qos)
    else {
        return ptr::null_mut();
    };
    let Some(ep) = Endpoints::create(
        n,
        &rr,
        &rep_t,
        &rq,
        &req_t,
        &requested,
        &|seed| {
            let mut ud = format!("serviceid={};", hex(seed)).into_bytes();
            ud.extend_from_slice(&hash);
            ud
        },
        actual,
    ) else {
        return ptr::null_mut();
    };
    alloc::boxed::Box::into_raw(alloc::boxed::Box::new(RmwZerodsService {
        ep,
        pending: Mutex::new(BTreeMap::new()),
        service_name: svc,
    }))
}

/// `rmw_destroy_service`.
///
/// # Safety
/// `service` from `rmw_zerodds_create_service` or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_destroy_service(service: *mut RmwZerodsService) -> i32 {
    if service.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: Box pointer from create.
    drop(unsafe { alloc::boxed::Box::from_raw(service) });
    RMW_RET_OK
}

/// `rmw_take_request`: the next request as `[encapsulation][body]` and the
/// request id rcl passes back to [`rmw_zerodds_send_response`]. A NULL
/// buffer means none.
///
/// # Safety
/// `service` valid; out pointers valid, `out_writer_guid` 16 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_take_request(
    service: *mut RmwZerodsService,
    out_buf: *mut *mut u8,
    out_len: *mut usize,
    out_big_endian: *mut u8,
    out_writer_guid: *mut u8,
    out_seq: *mut i64,
) -> i32 {
    if service.is_null()
        || out_buf.is_null()
        || out_len.is_null()
        || out_writer_guid.is_null()
        || out_seq.is_null()
    {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    let s = unsafe { &*service };
    // SAFETY: valid out pointers.
    unsafe {
        *out_buf = ptr::null_mut();
        *out_len = 0;
    }
    while let Some(sample) = s.ep.inbox.pop() {
        let UserSample::Alive {
            payload,
            writer_guid,
            representation,
            big_endian,
            source_sequence_number,
            related_sample_identity,
            ..
        } = sample
        else {
            continue;
        };
        let payload = payload.as_ref();
        let cyclone_client = related_sample_identity.is_none()
            && s.ep.rt.participant_vendor(prefix_of(&writer_guid)) == Some(VendorId::CYCLONE);
        let (id, reply, body) = if cyclone_client {
            let Some((guid, seq)) = cyclone_header(payload, big_endian) else {
                continue;
            };
            let mut id = [0u8; 16];
            id[..8].copy_from_slice(&guid.to_le_bytes());
            id[8..].copy_from_slice(&writer_guid[8..16]);
            (
                (id, seq),
                Reply::Cyclone {
                    guid,
                    seq,
                    request_writer: writer_guid,
                },
                &payload[CYCLONE_HEADER..],
            )
        } else {
            // rmw_fastrtps `__rmw_take_request`: the request's own identity,
            // its writer GUID replaced by the reply reader's when the client
            // named one.
            let named = related_sample_identity
                .map(|r| r.writer_guid)
                .filter(|g| *g != [0u8; 16]);
            let related = SampleIdentityBytes::new(
                named.unwrap_or(writer_guid),
                source_sequence_number as u64,
            );
            (
                (related.writer_guid, source_sequence_number),
                Reply::Related {
                    related,
                    request_writer: writer_guid,
                },
                payload,
            )
        };
        if let Ok(mut p) = s.pending.lock() {
            insert_bounded(&mut p, id, reply, "service requests");
        }
        // SAFETY: valid out pointers; `out_writer_guid` holds 16 bytes.
        unsafe {
            hand_out(
                representation,
                big_endian,
                body,
                out_buf,
                out_len,
                out_big_endian,
            );
            core::ptr::copy_nonoverlapping(id.0.as_ptr(), out_writer_guid, 16);
            *out_seq = id.1;
        }
        return RMW_RET_OK;
    }
    RMW_RET_OK
}

/// Whether the client of `reply` can receive the reply now: `Some(true)`
/// yes, `Some(false)` not yet, `None` gone (its request writer is no longer
/// matched).
fn client_present(s: &RmwZerodsService, reply: &Reply) -> Option<bool> {
    let rt = &s.ep.rt;
    let request_writer = match reply {
        Reply::Cyclone { request_writer, .. } | Reply::Related { request_writer, .. } => {
            *request_writer
        }
    };
    if !rt
        .user_reader_matched_writer_guids(s.ep.reader)
        .contains(&request_writer)
    {
        return None;
    }
    let readers = rt.user_writer_matched_reader_guids(s.ep.writer);
    Some(match reply {
        // DDSI-RTPS 2.5 §9.3.1.2: reader entity kinds have bit 0x04 set;
        // a named writer GUID is the old fallback, nothing to wait for.
        Reply::Related { related, .. } if related.writer_guid[15] & 0x04 != 0 => {
            readers.contains(&related.writer_guid)
        }
        Reply::Related { .. } => true,
        Reply::Cyclone { .. } => {
            let Some(id) = rt
                .discovered_endpoint_user_data(request_writer)
                .and_then(|ud| user_data_key(&ud, "clientid"))
            else {
                // A client without a client id: rmw_cyclonedds assumes all
                // is well.
                return Some(true);
            };
            readers.iter().any(|g| {
                rt.discovered_endpoint_user_data(*g)
                    .and_then(|ud| user_data_key(&ud, "clientid"))
                    .is_some_and(|c| c == id)
            })
        }
    })
}

/// `rmw_send_response`: `stream` is `[CDR_LE encapsulation][body]`, the
/// request id the one [`rmw_zerodds_take_request`] gave. Waits up to 100 ms
/// for the client's reply reader; `RMW_RET_TIMEOUT` when it does not appear,
/// `RMW_RET_OK` without sending when the client is gone (as both other
/// RMWs), `RMW_RET_ERROR` for an unknown request id.
///
/// # Safety
/// `service` valid; `writer_guid` 16 bytes; `stream[..len]` readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_send_response(
    service: *mut RmwZerodsService,
    writer_guid: *const u8,
    seq: i64,
    stream: *const u8,
    len: usize,
) -> i32 {
    if service.is_null() || writer_guid.is_null() || stream.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked; readable by the caller's contract.
    let (s, stream, guid) = unsafe {
        let mut g = [0u8; 16];
        core::ptr::copy_nonoverlapping(writer_guid, g.as_mut_ptr(), 16);
        (&*service, core::slice::from_raw_parts(stream, len), g)
    };
    let Some(body) = body_of(stream) else {
        return RMW_RET_ERROR;
    };
    let Some((_, reply)) = s
        .pending
        .lock()
        .ok()
        .and_then(|mut p| p.remove(&(guid, seq)))
    else {
        return RMW_RET_ERROR;
    };
    let deadline = Instant::now() + REPLY_READER_WAIT;
    loop {
        match client_present(s, &reply) {
            None => return RMW_RET_OK,
            Some(true) => break,
            Some(false) if Instant::now() >= deadline => {
                #[allow(clippy::print_stderr)]
                {
                    eprintln!(
                        "[rmw_zerodds] {}: the client's reply reader is not matched; reply not sent",
                        s.service_name
                    );
                }
                return RMW_RET_TIMEOUT;
            }
            Some(false) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
    let written = match reply {
        Reply::Cyclone { guid, seq, .. } => {
            let mut payload = Vec::with_capacity(CYCLONE_HEADER + body.len());
            payload.extend_from_slice(&guid.to_le_bytes());
            payload.extend_from_slice(&seq.to_le_bytes());
            payload.extend_from_slice(body);
            s.ep.rt.write_user_sample_borrowed(s.ep.writer, &payload)
        }
        Reply::Related { related, .. } => {
            s.ep.rt
                .write_user_sample_related(s.ep.writer, body, related)
                .map(|_| ())
        }
    };
    if written.is_ok() {
        RMW_RET_OK
    } else {
        RMW_RET_ERROR
    }
}

/// `1` if a request is queued for `service`, `0` if none.
///
/// # Safety
/// `service` from `rmw_zerodds_create_service` or NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rmw_zerodds_service_has_data(service: *mut RmwZerodsService) -> i32 {
    if service.is_null() {
        return RMW_RET_INVALID_ARGUMENT;
    }
    // SAFETY: NULL-checked.
    i32::from(unsafe { &*service }.ep.inbox.has_data())
}

/// Sets the rmw `on_new_request` callback.
///
/// # Safety
/// `service` from `rmw_zerodds_create_service` or NULL.
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
    unsafe { &*service }.ep.inbox.set_event(cb, user_data);
    RMW_RET_OK
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_cyclone_header_reads_in_both_byte_orders() {
        let mut le = Vec::new();
        le.extend_from_slice(&7u64.to_le_bytes());
        le.extend_from_slice(&(-3i64).to_le_bytes());
        le.push(0xAA);
        assert_eq!(cyclone_header(&le, false), Some((7, -3)));
        let mut be = Vec::new();
        be.extend_from_slice(&7u64.to_be_bytes());
        be.extend_from_slice(&(-3i64).to_be_bytes());
        assert_eq!(cyclone_header(&be, true), Some((7, -3)));
        assert_eq!(cyclone_header(&le[..15], false), None);
    }

    #[test]
    fn user_data_keys_read_like_rmw_cyclonedds() {
        let ud = b"clientid=0a0b;other=x;";
        assert_eq!(user_data_key(ud, "clientid").as_deref(), Some("0a0b"));
        assert_eq!(user_data_key(ud, "other").as_deref(), Some("x"));
        assert_eq!(user_data_key(ud, "serviceid"), None);
    }

    #[test]
    fn a_full_table_drops_its_oldest_entry() {
        let mut m: BTreeMap<i64, (Instant, i64)> = BTreeMap::new();
        for k in 0..PENDING_CAP as i64 {
            insert_bounded(&mut m, k, k, "test");
        }
        insert_bounded(&mut m, -1, -1, "test");
        assert_eq!(m.len(), PENDING_CAP);
        assert!(!m.contains_key(&0), "the oldest went");
        assert!(m.contains_key(&-1));
    }
}
