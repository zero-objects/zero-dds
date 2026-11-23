// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The requests waiting for their DDS reply (spec §4.3, §6, §9.2): the
//! reply, the timeout, the deadline, the replay store of `TRANSIENT_LOCAL`,
//! `CancelRequest` and the drain; and the end of every exchange, its log
//! line (§8.1) and its spans (§8.3).

#[allow(unused_imports)]
use super::prelude::*;

use std::collections::VecDeque;
use zerodds_corba_csiv2::sas::SasMessage;

/// What a request carries into its reply, its log line and its spans.
#[derive(Debug, Clone)]
pub(crate) struct ReqCtx {
    /// The client's address.
    pub(crate) peer: String,
    /// The GIOP version of the request.
    pub(crate) version: Version,
    /// Its byte order.
    pub(crate) endianness: Endianness,
    /// Its GIOP request id.
    pub(crate) request_id: u32,
    /// The operation it names.
    pub(crate) operation: String,
    /// The octets of its body.
    pub(crate) bytes: usize,
    /// When it arrived.
    pub(crate) started: Instant,
    /// When it arrived, in nanoseconds since the Unix epoch.
    pub(crate) start_ns: u64,
}

/// A request waiting for its DDS reply.
pub(crate) struct Pending {
    pub(crate) conn: ConnWriter,
    pub(crate) ctx: ReqCtx,
    /// `timeout_ms` after the request.
    pub(crate) deadline: Instant,
    /// Index of the mapping.
    pub(crate) mapping: usize,
    /// §7.2: this request's CSIv2 SAS decision, attached to whichever
    /// reply it eventually gets (the DDS reply, or a TIMEOUT/TRANSIENT).
    pub(crate) sas_reply: Option<SasMessage>,
    /// The DDS write of the request sample, for its span.
    pub(crate) publish: Option<(u64, u64)>,
    /// §6 `TRANSIENT_LOCAL`: the key the reply is kept under.
    pub(crate) replay_key: Option<[u8; 32]>,
}

/// Every request in flight, by the bridge's request id, and the condition
/// the timeout thread waits on.
pub(crate) struct InFlight {
    pub(crate) pending: Mutex<BTreeMap<u64, Pending>>,
    pub(crate) changed: Condvar,
}

/// When a mapping's reply or notify topic last delivered a sample, for its
/// deadline (§6), and whether its missed deadline was reported.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SampleClock {
    pub(crate) last: Option<Instant>,
    pub(crate) reported: bool,
}

/// The name of a GIOP reply status, as the log and the spans give it.
pub(crate) fn status_name(status: ReplyStatusType) -> &'static str {
    match status {
        ReplyStatusType::NoException => "NO_EXCEPTION",
        ReplyStatusType::UserException => "USER_EXCEPTION",
        ReplyStatusType::SystemException => "SYSTEM_EXCEPTION",
        ReplyStatusType::LocationForward => "LOCATION_FORWARD",
        ReplyStatusType::LocationForwardPerm => "LOCATION_FORWARD_PERM",
        ReplyStatusType::NeedsAddressingMode => "NEEDS_ADDRESSING_MODE",
    }
}

/// The exception a reply body names: its repository id, the first string
/// of the body (GIOP §15.4.3.2), shortened to the name for a system
/// exception of `omg.org/CORBA`.
pub(crate) fn exception_name(body: &[u8], endianness: Endianness) -> Option<String> {
    let mut r = zerodds_wire::Reader::new(body, endianness);
    let id = r.read_string().ok()?;
    Some(
        id.strip_prefix("IDL:omg.org/CORBA/")
            .and_then(|s| s.strip_suffix(":1.0"))
            .map_or(id.clone(), str::to_string),
    )
}

/// The end of an exchange: its log line (§8.1, event `exchange`) and its
/// spans (§8.3).
#[allow(clippy::too_many_arguments)]
pub(crate) fn finish(
    d: &Dispatch,
    x: &ReqCtx,
    mapping: Option<usize>,
    status: Option<ReplyStatusType>,
    exception: Option<&str>,
    publish: Option<(u64, u64)>,
    subscribe: Option<u64>,
    reply: Option<(u64, u64)>,
) {
    let m = mapping.and_then(|i| d.cfg.mappings.get(i));
    // A oneway has no reply.
    let name = status.map_or("NONE", status_name);
    let version = format!("{}.{}", x.version.major, x.version.minor);
    let latency_us = u64::try_from(x.started.elapsed().as_micros()).unwrap_or(u64::MAX);
    let mut fields = vec![
        ("peer", Field::Text(&x.peer)),
        ("giop_version", Field::Text(&version)),
        ("request_id", Field::Number(u64::from(x.request_id))),
        ("operation", Field::Text(&x.operation)),
    ];
    if let Some(m) = m {
        fields.push(("repo_id", Field::Text(&m.repo_id)));
    }
    fields.push(("bytes", Field::Number(x.bytes as u64)));
    fields.push(("reply_status", Field::Text(name)));
    if let Some(e) = exception {
        fields.push(("exception", Field::Text(e)));
    }
    fields.push(("latency_us", Field::Number(latency_us)));
    if let Some(t) = &d.tenant {
        fields.push(("tenant", Field::Text(t)));
    }
    emit("exchange", &fields);
    if d.tracer.enabled() {
        let mut attributes: Vec<(&'static str, String)> = vec![
            ("rpc.system", "corba".into()),
            ("rpc.method", x.operation.clone()),
            ("net.peer", x.peer.clone()),
            ("giop.version", version),
            ("giop.reply_status", name.into()),
        ];
        if let Some(m) = m {
            attributes.push(("corba.repo_id", m.repo_id.clone()));
            attributes.push(("dds.topic", m.request_topic.dds_name.clone()));
        }
        if let Some(e) = exception {
            attributes.push(("corba.exception", e.to_string()));
        }
        if let Some(t) = &d.tenant {
            attributes.push(("zerodds.tenant", t.clone()));
        }
        d.tracer.exchange(&Exchange {
            start_ns: x.start_ns,
            end_ns: unix_ns(),
            ok: matches!(status, None | Some(ReplyStatusType::NoException)),
            attributes,
            publish,
            subscribe,
            reply,
        });
    }
}

/// The key a reply is kept under in the replay store (§6): the SHA-256 of
/// the operation, the byte order and the request body.
pub(crate) fn replay_key(operation: &str, endianness: Endianness, body: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(operation.as_bytes());
    h.update([0, u8::from(matches!(endianness, Endianness::Little))]);
    h.update(body);
    h.finalize().into()
}

/// One kept reply.
#[derive(Debug, Clone)]
pub(crate) struct Replayed {
    pub(crate) key: [u8; 32],
    pub(crate) at: Instant,
    pub(crate) status: ReplyStatusType,
    pub(crate) body: Vec<u8>,
}

/// §6 `TRANSIENT_LOCAL`: the replies of a mapping by request, the last
/// `depth` of them (KEEP_LAST) or all (KEEP_ALL), each for `lifespan`.
#[derive(Debug, Default)]
pub(crate) struct ReplayStore {
    pub(crate) entries: VecDeque<Replayed>,
}

impl ReplayStore {
    /// The kept reply of `key`, if it is younger than `lifespan`; expired
    /// replies go.
    pub(crate) fn get(
        &mut self,
        key: &[u8; 32],
        lifespan: Option<Duration>,
        now: Instant,
    ) -> Option<(ReplyStatusType, Vec<u8>)> {
        if let Some(l) = lifespan {
            self.entries.retain(|e| now.duration_since(e.at) < l);
        }
        self.entries
            .iter()
            .find(|e| &e.key == key)
            .map(|e| (e.status, e.body.clone()))
    }

    /// Keeps the reply of `key`, in place of an older one of it; beyond
    /// `depth` the oldest goes.
    pub(crate) fn put(
        &mut self,
        key: [u8; 32],
        status: ReplyStatusType,
        body: Vec<u8>,
        depth: Option<u32>,
        now: Instant,
    ) {
        self.entries.retain(|e| e.key != key);
        self.entries.push_back(Replayed {
            key,
            at: now,
            status,
            body,
        });
        if let Some(depth) = depth {
            while self.entries.len() > depth as usize {
                self.entries.pop_front();
            }
        }
    }
}

/// Notes a sample on the reply or notify topic of mapping `index` (§6).
pub(crate) fn note_sample(d: &Dispatch, index: usize) {
    if let Some(c) = d.clocks.get(index)
        && let Ok(mut c) = c.lock()
    {
        c.last = Some(Instant::now());
        c.reported = false;
    }
}

/// A reply sample on the reply topic of mapping `index` (§4.3 step 7).
pub(crate) fn on_reply(
    d: &Dispatch,
    index: usize,
    payload: &[u8],
    big_endian: bool,
    identifier: [u8; 2],
) {
    let taken = unix_ns();
    note_sample(d, index);
    let endianness = if big_endian {
        Endianness::Big
    } else {
        Endianness::Little
    };
    if payload.len() < 16 {
        d.metrics.errors_total.inc();
        log("reply_malformed", &[("len", &payload.len().to_string())]);
        return;
    }
    let u64v = |b: &[u8]| {
        let a: [u8; 8] = b.try_into().unwrap_or([0; 8]);
        if big_endian {
            u64::from_be_bytes(a)
        } else {
            u64::from_le_bytes(a)
        }
    };
    let i32v = |b: &[u8]| {
        let a: [u8; 4] = b.try_into().unwrap_or([0; 4]);
        if big_endian {
            i32::from_be_bytes(a)
        } else {
            i32::from_le_bytes(a)
        }
    };
    let id = u64v(&payload[..8]);
    let status = i32v(&payload[8..12]);
    let Some(p) = d
        .in_flight
        .pending
        .lock()
        .ok()
        .and_then(|mut m| m.remove(&id))
    else {
        // Timed out already, cancelled, or another bridge's.
        log("reply_without_request", &[("id", &id.to_string())]);
        return;
    };
    if let Some(om) = d.spec.as_ref().and_then(|s| s.ops.get(p.mapping)) {
        om.received.inc();
    }
    let (status, body) = match (status, plain_cdr(identifier)) {
        (_, false) => {
            log(
                "reply_malformed",
                &[("id", &id.to_string()), ("err", &not_plain_cdr(identifier))],
            );
            (
                ReplyStatusType::SystemException,
                system_exception_body("MARSHAL", 0, Completion::Maybe, endianness),
            )
        }
        (0, true) => (ReplyStatusType::NoException, payload[16..].to_vec()),
        (1, true) => match d.models.get(p.mapping).and_then(Option::as_ref) {
            // §5.2: the sample names the exception by its kind and holds
            // its members; GIOP names it by its repository id.
            Some(model) => {
                let raises = d.cfg.mappings.get(p.mapping).map_or(&[][..], |m| &m.raises);
                match super::typegen::user_exception_body(model, raises, payload, big_endian) {
                    Ok(body) => (ReplyStatusType::UserException, body),
                    Err(e) => {
                        log("reply_malformed", &[("id", &id.to_string()), ("err", &e)]);
                        (
                            ReplyStatusType::SystemException,
                            system_exception_body("MARSHAL", 0, Completion::Maybe, endianness),
                        )
                    }
                }
            }
            None => (ReplyStatusType::UserException, payload[16..].to_vec()),
        },
        (2, _) => (ReplyStatusType::SystemException, payload[16..].to_vec()),
        (other, _) => {
            log(
                "reply_malformed",
                &[
                    ("id", &id.to_string()),
                    ("reply_status", &other.to_string()),
                ],
            );
            (
                ReplyStatusType::SystemException,
                system_exception_body("MARSHAL", 0, Completion::Maybe, endianness),
            )
        }
    };
    log("reply", &[("id", &id.to_string())]);
    d.metrics.dds_samples_in_total.inc();
    count_reply(
        d,
        &p,
        match status {
            ReplyStatusType::NoException => Outcome::Ok,
            ReplyStatusType::UserException => Outcome::User,
            _ => Outcome::System,
        },
    );
    if let Some(key) = p.replay_key
        && matches!(
            status,
            ReplyStatusType::NoException | ReplyStatusType::UserException
        )
        && let Some(store) = d.replay.get(p.mapping)
        && let Ok(mut store) = store.lock()
    {
        let m = &d.cfg.mappings[p.mapping];
        store.put(key, status, body.clone(), m.history_depth, Instant::now());
    }
    let exception = (status != ReplyStatusType::NoException)
        .then(|| exception_name(&body, endianness))
        .flatten();
    let w0 = unix_ns();
    match encode_reply_with_context(
        Version::V1_2,
        endianness,
        p.ctx.request_id,
        status,
        body,
        sas_service_context(p.sas_reply.as_ref(), endianness),
    ) {
        Ok(msg) => {
            d.metrics.frames_out_total.inc();
            d.metrics.bytes_out_total.add(msg.len() as u64);
            if let Err(e) = write_giop(
                &p.conn,
                &msg,
                d.cfg.fragment_size,
                p.ctx.request_id,
                endianness,
            ) {
                log("reply_write_failed", &[("err", &e.to_string())]);
            }
        }
        Err(e) => log("encode_error", &[("err", &e.to_string())]),
    }
    finish(
        d,
        &p.ctx,
        Some(p.mapping),
        Some(status),
        exception.as_deref(),
        p.publish,
        Some(taken),
        Some((w0, unix_ns())),
    );
}

/// Answers a pending request with the system exception `name`, completion
/// MAYBE (the backend has the request): `TIMEOUT` after `timeout_ms`,
/// `TRANSIENT` after a missed deadline or at the end of the drain.
pub(crate) fn answer(d: &Dispatch, p: &Pending, name: &str) {
    count_reply(d, p, Outcome::System);
    let body = system_exception_body(name, 0, Completion::Maybe, Endianness::Little);
    let service_context = sas_service_context(p.sas_reply.as_ref(), Endianness::Little);
    let w0 = unix_ns();
    if let Ok(msg) = encode_reply_with_context(
        Version::V1_2,
        Endianness::Little,
        p.ctx.request_id,
        ReplyStatusType::SystemException,
        body,
        service_context,
    ) {
        let _ = write_giop(
            &p.conn,
            &msg,
            d.cfg.fragment_size,
            p.ctx.request_id,
            Endianness::Little,
        );
    }
    finish(
        d,
        &p.ctx,
        Some(p.mapping),
        Some(ReplyStatusType::SystemException),
        Some(name),
        p.publish,
        None,
        Some((w0, unix_ns())),
    );
}

/// Whether the reply topic of `p`'s mapping missed its deadline while `p`
/// waited (§6): `p` has waited `deadline_ms`, and no reply sample came in
/// the last `deadline_ms`.
pub(crate) fn deadline_missed(d: &Dispatch, p: &Pending, now: Instant) -> bool {
    let Some(ms) = d.cfg.mappings.get(p.mapping).and_then(|m| m.deadline_ms) else {
        return false;
    };
    let period = Duration::from_millis(ms);
    if now.duration_since(p.ctx.started) < period {
        return false;
    }
    let last = d
        .clocks
        .get(p.mapping)
        .and_then(|c| c.lock().ok().map(|c| c.last))
        .flatten();
    last.is_none_or(|t| now.duration_since(t) >= period)
}

/// §6: a notify topic that went `deadline_ms` without a sample is reported
/// once per gap.
pub(crate) fn check_notify_deadlines(d: &Dispatch, now: Instant) {
    for (i, m) in d.cfg.mappings.iter().enumerate() {
        let (Some(ms), true) = (m.deadline_ms, m.sends_giop()) else {
            continue;
        };
        let Some(Ok(mut c)) = d.clocks.get(i).map(|c| c.lock()) else {
            continue;
        };
        let since = c.last.unwrap_or(d.started);
        if !c.reported && now.duration_since(since) >= Duration::from_millis(ms) {
            c.reported = true;
            log(
                "deadline_missed",
                &[
                    ("topic", &m.notify_topic.dds_name),
                    ("deadline_ms", &ms.to_string()),
                ],
            );
        }
    }
}

/// Answers every request whose `timeout_ms` passed with `CORBA::TIMEOUT`,
/// and every one whose reply topic missed its deadline with
/// `CORBA::TRANSIENT`.
pub(crate) fn timeout_loop(d: &Dispatch, shutdown: &AtomicBool) {
    let Ok(mut pending) = d.in_flight.pending.lock() else {
        return;
    };
    while !shutdown.load(Ordering::SeqCst) {
        let now = Instant::now();
        let due: Vec<(u64, &str)> = pending
            .iter()
            .filter_map(|(id, p)| {
                if p.deadline <= now {
                    Some((*id, "TIMEOUT"))
                } else if deadline_missed(d, p, now) {
                    Some((*id, "TRANSIENT"))
                } else {
                    None
                }
            })
            .collect();
        for (id, name) in due {
            if let Some(p) = pending.remove(&id) {
                if name == "TIMEOUT" {
                    log("timeout", &[("id", &id.to_string())]);
                    if let Some(om) = d.spec.as_ref().and_then(|s| s.ops.get(p.mapping)) {
                        om.timeouts.inc();
                    }
                } else {
                    log(
                        "deadline_missed",
                        &[
                            ("id", &id.to_string()),
                            ("topic", &d.cfg.mappings[p.mapping].reply_topic.dds_name),
                        ],
                    );
                }
                answer(d, &p, name);
            }
        }
        check_notify_deadlines(d, now);
        let wait = pending
            .values()
            .map(|p| p.deadline.saturating_duration_since(Instant::now()))
            .min()
            .unwrap_or(Duration::from_millis(500))
            .min(Duration::from_millis(if d.has_deadlines {
                50
            } else {
                500
            }));
        pending = match d.in_flight.changed.wait_timeout(pending, wait) {
            Ok((g, _)) => g,
            Err(_) => return,
        };
    }
}

/// `CancelRequest` (GIOP §15.4.4): the request `request_id` of `conn` is
/// not answered any more. Returns how many were cancelled (0 or 1).
pub(crate) fn cancel(d: &Dispatch, conn: &ConnWriter, request_id: u32) -> usize {
    let Ok(mut p) = d.in_flight.pending.lock() else {
        return 0;
    };
    let conn_ptr = Arc::as_ptr(conn);
    let mut cancelled: Vec<usize> = Vec::new();
    p.retain(|_, v| {
        let keep = !(v.ctx.request_id == request_id && Arc::as_ptr(&v.conn) == conn_ptr);
        if !keep {
            cancelled.push(v.mapping);
        }
        keep
    });
    for index in &cancelled {
        if let Some(om) = d.spec.as_ref().and_then(|s| s.ops.get(*index)) {
            om.pending.dec();
        }
    }
    if !cancelled.is_empty() {
        log("cancelled", &[("request_id", &request_id.to_string())]);
    }
    cancelled.len()
}

/// The shutdown of §9.2: new requests are refused with `TRANSIENT`, the
/// pending ones get their reply or their `TIMEOUT` for at most `limit`,
/// what is left then gets `TRANSIENT` (completion MAYBE: the backend has the
/// request). Then every connection gets `CloseConnection` and is closed.
pub(crate) fn drain(d: &Dispatch, limit: Duration) {
    d.draining.store(true, Ordering::SeqCst);
    let pending = d.in_flight.pending.lock().map(|p| p.len()).unwrap_or(0);
    log("draining", &[("pending", &pending.to_string())]);
    let deadline = Instant::now() + limit;
    loop {
        let left = d.in_flight.pending.lock().map(|p| p.len()).unwrap_or(0);
        if left == 0 || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let left: Vec<Pending> = d
        .in_flight
        .pending
        .lock()
        .map(|mut p| core::mem::take(&mut *p).into_values().collect())
        .unwrap_or_default();
    if !left.is_empty() {
        log("drain_expired", &[("pending", &left.len().to_string())]);
    }
    for p in &left {
        answer(d, p, "TRANSIENT");
    }
    let conns: Vec<Arc<Conn>> = d
        .conns
        .lock()
        .map(|c| c.values().cloned().collect())
        .unwrap_or_default();
    for c in conns {
        let version = c.version.lock().map(|v| *v).unwrap_or(Version::V1_2);
        if let Ok(bytes) = encode_message(
            version,
            Endianness::Little,
            false,
            &Message::CloseConnection(zerodds_corba_giop::CloseConnection),
        ) && let Ok(mut w) = c.writer.lock()
        {
            let _ = w.write_all(&bytes).and_then(|()| w.flush());
        }
        let _ = c.sock.shutdown(std::net::Shutdown::Both);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::bridge_daemon::server::tests::{Sink, fixture, pending, replies};

    const YAML: &str =
        "mappings:\n  - repo_id: \"IDL:x/Calc:1.0\"\n    operation: add\n    timeout_ms: 60000\n";

    fn reply_sample(id: u64, status: i32, body: &[u8]) -> Vec<u8> {
        let mut s = id.to_le_bytes().to_vec();
        s.extend_from_slice(&status.to_le_bytes());
        s.extend_from_slice(&0u32.to_le_bytes());
        s.extend_from_slice(body);
        s
    }

    /// §4.3 step 7: the reply sample with the request's id becomes the GIOP
    /// reply of that request, its status and body.
    #[test]
    fn corbabridge_4_3_a_reply_sample_answers_its_request() {
        let (d, _lease) = fixture(YAML);
        let sink = Sink::default();
        pending(&d, 5, 77, sink.writer());
        on_reply(&d, 0, &reply_sample(5, 0, &[42, 0, 0, 0]), false, [0, 1]);
        let r = replies(&sink);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].request_id, 77);
        assert_eq!(r[0].reply_status, ReplyStatusType::NoException);
        assert_eq!(r[0].body, [42, 0, 0, 0]);
        assert!(d.in_flight.pending.lock().unwrap().is_empty());
    }

    /// A user exception goes through as one, an unknown `reply_status` and
    /// a sample that is not plain CDR are `MARSHAL`.
    #[test]
    fn corbabridge_4_3_the_reply_status_and_the_representation_decide() {
        let (d, _lease) = fixture(YAML);
        let sink = Sink::default();
        pending(&d, 1, 10, sink.writer());
        pending(&d, 2, 11, sink.writer());
        pending(&d, 3, 12, sink.writer());
        on_reply(
            &d,
            0,
            &reply_sample(1, 1, b"\x08\0\0\0IDL:E:1\0"),
            false,
            [0, 1],
        );
        on_reply(&d, 0, &reply_sample(2, 9, &[]), false, [0, 1]);
        on_reply(&d, 0, &reply_sample(3, 0, &[]), false, [0, 3]);
        let r = replies(&sink);
        assert_eq!(r[0].reply_status, ReplyStatusType::UserException);
        for x in &r[1..] {
            assert_eq!(x.reply_status, ReplyStatusType::SystemException);
            assert_eq!(
                exception_name(&x.body, Endianness::Little).as_deref(),
                Some("MARSHAL")
            );
        }
    }

    /// §5.2: with the type of the mapping from `idl_file`, the reply sample
    /// of a user exception holds its `exception_kind` and its members; the
    /// GIOP reply names it by its repository id (41 characters and the NUL,
    /// so the members begin at 46 and `numerator` is padded to 48). A kind
    /// the operation does not raise is `MARSHAL`.
    #[test]
    fn corbabridge_5_2_a_user_exception_sample_is_the_giop_exception() {
        let yaml = format!(
            "mappings:\n  - repo_id: \"IDL:zerodds_test/Divider:1.0\"\n    operation: divide\n    idl_file: \"{}/tests/omniorb/Div.idl\"\n    timeout_ms: 60000\n",
            env!("CARGO_MANIFEST_DIR")
        );
        let (d, _lease) = fixture(&yaml);
        let sink = Sink::default();
        for (id, request_id) in [(1, 10), (2, 11), (3, 12)] {
            pending(&d, id, request_id, sink.writer());
        }
        let sample = |id: u64, kind: u32, members: &[u8]| {
            let mut s = id.to_le_bytes().to_vec();
            s.extend_from_slice(&1i32.to_le_bytes());
            s.extend_from_slice(&kind.to_le_bytes());
            s.extend_from_slice(members);
            s
        };
        let mut members = 7i64.to_le_bytes().to_vec();
        members.extend_from_slice(&5u32.to_le_bytes());
        members.extend_from_slice(b"none\0");
        on_reply(&d, 0, &sample(1, 0, &members), false, [0, 1]);
        on_reply(&d, 0, &sample(2, 1, &[]), false, [0, 1]);
        on_reply(&d, 0, &sample(3, 2, &[]), false, [0, 1]);
        let r = replies(&sink);
        assert_eq!(r[0].reply_status, ReplyStatusType::UserException);
        let mut expected = 42u32.to_le_bytes().to_vec();
        expected.extend_from_slice(b"IDL:zerodds_test/Divider/DivideByZero:1.0\0");
        expected.extend_from_slice(&[0, 0]);
        expected.extend_from_slice(&members);
        assert_eq!(r[0].body, expected);
        assert_eq!(r[1].reply_status, ReplyStatusType::UserException);
        let mut expected = 41u32.to_le_bytes().to_vec();
        expected.extend_from_slice(b"IDL:zerodds_test/Divider/Unsupported:1.0\0");
        assert_eq!(r[1].body, expected);
        assert_eq!(r[2].reply_status, ReplyStatusType::SystemException);
        assert_eq!(
            exception_name(&r[2].body, Endianness::Little).as_deref(),
            Some("MARSHAL")
        );
    }

    /// A reply without its request (timed out, cancelled, another bridge's)
    /// is dropped.
    #[test]
    fn corbabridge_4_3_a_reply_without_its_request_is_dropped() {
        let (d, _lease) = fixture(YAML);
        let sink = Sink::default();
        pending(&d, 1, 10, sink.writer());
        on_reply(&d, 0, &reply_sample(99, 0, &[]), false, [0, 1]);
        assert!(replies(&sink).is_empty());
        assert_eq!(d.in_flight.pending.lock().unwrap().len(), 1);
    }

    /// `CancelRequest` removes the request of its connection only.
    #[test]
    fn corbabridge_4_3_cancel_request_removes_the_request_of_its_connection() {
        let (d, _lease) = fixture(YAML);
        let (a, b) = (Sink::default(), Sink::default());
        let wa = a.writer();
        pending(&d, 1, 10, Arc::clone(&wa));
        pending(&d, 2, 10, b.writer());
        assert_eq!(cancel(&d, &wa, 10), 1);
        assert_eq!(cancel(&d, &wa, 10), 0);
        let left: Vec<u64> = d
            .in_flight
            .pending
            .lock()
            .unwrap()
            .keys()
            .copied()
            .collect();
        assert_eq!(left, [2]);
        on_reply(&d, 0, &reply_sample(1, 0, &[]), false, [0, 1]);
        assert!(replies(&a).is_empty(), "no reply to a cancelled request");
    }

    /// §9.2: what is still pending when the drain ends gets `TRANSIENT`
    /// with completion MAYBE.
    #[test]
    fn corbabridge_9_2_the_drain_answers_what_is_left_with_transient_maybe() {
        let (d, _lease) = fixture(YAML);
        let sink = Sink::default();
        pending(&d, 1, 10, sink.writer());
        drain(&d, Duration::from_millis(50));
        let r = replies(&sink);
        assert_eq!(r.len(), 1);
        assert_eq!(
            exception_name(&r[0].body, Endianness::Little).as_deref(),
            Some("TRANSIENT")
        );
        let mut rd = zerodds_wire::Reader::new(&r[0].body, Endianness::Little);
        let _ = rd.read_string().unwrap();
        let _minor = rd.read_u32().unwrap();
        assert_eq!(rd.read_u32().unwrap(), 2, "COMPLETED_MAYBE");
        assert!(d.draining.load(Ordering::SeqCst));
    }

    /// §6: the replay store keeps the last `depth` replies, each for the
    /// lifespan, one per key.
    #[test]
    fn corbabridge_6_the_replay_store_keeps_the_last_replies_for_their_lifespan() {
        let mut s = ReplayStore::default();
        let t0 = Instant::now();
        let k = |b: u8| replay_key("op", Endianness::Little, &[b]);
        s.put(k(1), ReplyStatusType::NoException, vec![1], Some(2), t0);
        s.put(k(2), ReplyStatusType::NoException, vec![2], Some(2), t0);
        s.put(k(1), ReplyStatusType::NoException, vec![11], Some(2), t0);
        s.put(k(3), ReplyStatusType::UserException, vec![3], Some(2), t0);
        assert!(s.get(&k(2), None, t0).is_none(), "depth 2: the oldest went");
        assert_eq!(s.get(&k(1), None, t0).unwrap().1, [11]);
        let later = t0 + Duration::from_millis(30);
        assert!(
            s.get(&k(3), Some(Duration::from_millis(10)), later)
                .is_none()
        );
        assert_ne!(
            replay_key("op", Endianness::Little, &[1]),
            replay_key("op", Endianness::Big, &[1])
        );
    }

    /// §6: a reply topic without a sample for `deadline_ms` answers the
    /// requests that waited that long with `TRANSIENT`.
    #[test]
    fn corbabridge_6_a_missed_deadline_answers_with_transient() {
        let (d, _lease) = fixture(
            "mappings:\n  - repo_id: \"IDL:x/Calc:1.0\"\n    operation: add\n    timeout_ms: 60000\n    qos:\n      deadline_ms: 50\n",
        );
        let sink = Sink::default();
        pending(&d, 1, 10, sink.writer());
        let p = d.in_flight.pending.lock().unwrap().remove(&1).unwrap();
        assert!(!deadline_missed(&d, &p, p.ctx.started));
        let later = p.ctx.started + Duration::from_millis(60);
        assert!(deadline_missed(&d, &p, later));
        note_sample(&d, 0);
        assert!(
            !deadline_missed(&d, &p, Instant::now()),
            "a sample just came"
        );
    }
}
