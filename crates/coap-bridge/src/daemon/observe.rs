// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Observe (RFC 7641) for the topics of `zerodds-coap-bridged` (spec §4.3,
//! §6).
//!
//! Every topic keeps the samples a new observer is given (its replay
//! window: the last sample, or the last N of a TRANSIENT_LOCAL topic with
//! KEEP_LAST N) and its observers. A DDS sample becomes a notification to
//! each observer, in the form the observer asked for. A notification is
//! Confirmable on a RELIABLE topic, and on any topic once the observer has
//! not acknowledged one for `coap.observe_max_age_secs` (RFC 7641 §4.5: at
//! least every 24 hours); it is retransmitted like a Confirmable message
//! (RFC 7252 §4.2), and a newer notification takes the place of one still
//! in flight (RFC 7641 §4.5.2). An observation ends when the client
//! deregisters, answers a notification with a Reset (§3.6), does not
//! acknowledge a Confirmable notification (§4.5), does not answer the
//! daemon's ping (`coap.keep_alive_secs`, spec §6), or the topic misses its
//! deadline (spec §6: `5.03` with Max-Age 0, a non-2.xx notification,
//! §4.2).

use std::collections::{BTreeMap, VecDeque};
use std::net::{SocketAddr, UdpSocket};
use std::string::String;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::vec::Vec;

use crate::codec::encode;
use crate::message::{CoapCode, CoapMessage, MessageType};
use crate::option::{CoapOption, OptionValue, numbers};

use super::oscore_gateway::{OscoreExchange, OscoreGateway};
use super::representation::Form;
use super::server::{Core, Topic, etag_of, representation};

/// The largest Observe sequence number (RFC 7641 §3.4: 24 bits).
const SEQUENCE_MASK: u32 = 0x00FF_FFFF;

/// How many Message IDs of the notifications to an observer are kept, so
/// that a Reset to one of them is matched (RFC 7641 §3.6).
const RECENT_MIDS: usize = 16;

/// The replay window of a KEEP_ALL topic.
pub(super) const KEEP_ALL_WINDOW: usize = 1024;

/// RFC 7641 §4.5: "at least every 24 hours" a notification is
/// Confirmable.
const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// The endpoint of an observer: a UDP peer or a DTLS session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Endpoint {
    /// Plaintext, by the peer's address.
    Udp(SocketAddr),
    /// A DTLS session, by its number.
    #[cfg(feature = "dtls")]
    Dtls(u64),
}

/// How a message reaches an observer.
#[derive(Debug, Clone)]
pub(super) enum Observer {
    /// Plaintext: sent to `addr` on the daemon's UDP socket.
    Udp {
        /// The peer.
        addr: SocketAddr,
        /// The token of the observation.
        token: Vec<u8>,
    },
    /// DTLS: handed to the session's thread, which writes it.
    #[cfg(feature = "dtls")]
    Dtls {
        /// The session.
        session: u64,
        /// The token of the observation.
        token: Vec<u8>,
        /// The session's queue.
        tx: super::server::DtlsTx,
    },
}

impl Observer {
    pub(super) fn token(&self) -> &[u8] {
        match self {
            Observer::Udp { token, .. } => token,
            #[cfg(feature = "dtls")]
            Observer::Dtls { token, .. } => token,
        }
    }

    pub(super) fn endpoint(&self) -> Endpoint {
        match self {
            Observer::Udp { addr, .. } => Endpoint::Udp(*addr),
            #[cfg(feature = "dtls")]
            Observer::Dtls { session, .. } => Endpoint::Dtls(*session),
        }
    }
}

/// A DDS sample as the daemon keeps it.
#[derive(Debug, Clone)]
pub(super) struct Stored {
    /// The body, without the encapsulation header.
    pub(super) body: Arc<[u8]>,
    /// The representation identifier of its header (DDS-XTypes 1.3 Table
    /// 60).
    pub(super) identifier: [u8; 2],
    /// When it arrived.
    pub(super) at: Instant,
}

impl Stored {
    /// RFC 7252 §5.10.5 with spec §6: the Max-Age of the sample, what is
    /// left of the topic's lifespan, rounded up; `None` without a lifespan
    /// (the default of 60 s applies), `Some(0)` once it is spent.
    pub(super) fn max_age(&self, lifespan: Option<Duration>, now: Instant) -> Option<u32> {
        let lifespan = lifespan?;
        let left = lifespan.saturating_sub(now.saturating_duration_since(self.at));
        let millis = left.as_millis();
        Some(u32::try_from(millis.div_ceil(1000)).unwrap_or(u32::MAX))
    }

    /// Whether the topic's lifespan has passed.
    pub(super) fn expired(&self, lifespan: Option<Duration>, now: Instant) -> bool {
        lifespan.is_some_and(|l| now.saturating_duration_since(self.at) >= l)
    }
}

/// The retransmission of a Confirmable message (RFC 7252 §4.2): a timeout
/// between ACK_TIMEOUT and ACK_TIMEOUT * 1.5, doubled at each
/// retransmission, MAX_RETRANSMIT retransmissions.
#[derive(Debug, Clone)]
struct Retransmission {
    attempts: u8,
    timeout: Duration,
    next_at: Instant,
}

impl Retransmission {
    fn start(timing: &Timing, now: Instant) -> Self {
        let timeout = initial_timeout(timing.ack_timeout);
        Self {
            attempts: 0,
            timeout,
            next_at: now + timeout,
        }
    }

    /// `Some(true)`: send again now; `Some(false)`: not yet; `None`: the
    /// last timeout has passed, the message is not acknowledged.
    fn due(&mut self, timing: &Timing, now: Instant) -> Option<bool> {
        if now < self.next_at {
            return Some(false);
        }
        if self.attempts >= timing.max_retransmit {
            return None;
        }
        self.attempts += 1;
        self.timeout *= 2;
        self.next_at = now + self.timeout;
        Some(true)
    }
}

/// RFC 7252 §4.2: the initial timeout, a random duration between
/// ACK_TIMEOUT and ACK_TIMEOUT * ACK_RANDOM_FACTOR (1.5).
fn initial_timeout(ack_timeout: Duration) -> Duration {
    let mut b = [0u8; 2];
    let r = if getrandom::getrandom(&mut b).is_ok() {
        u32::from(u16::from_be_bytes(b))
    } else {
        0
    };
    ack_timeout + ack_timeout / 2 * r / 65_536
}

/// A Confirmable notification waiting for its acknowledgement.
#[derive(Debug, Clone)]
struct InFlight {
    mid: u16,
    message: CoapMessage,
    retransmission: Retransmission,
}

/// A registered observer (RFC 7641 §4.1).
#[derive(Debug, Clone)]
pub(super) struct RegisteredObserver {
    pub(super) transport: Observer,
    /// The exchange parameters a notification to it is protected with when
    /// it registered over OSCORE (RFC 8613 §4.1.3.5.2).
    pub(super) oscore: Option<OscoreExchange>,
    /// The largest block a notification carries (RFC 7959 §2.6; spec §4.4,
    /// RFC 7252 §11.3).
    pub(super) block_szx: u8,
    /// The form it asked for with Accept; `None`: the topic's default.
    pub(super) form: Option<Form>,
    /// When it registered or last acknowledged a notification.
    confirmed_at: Instant,
    /// The Message IDs of the last notifications to it.
    recent: VecDeque<u16>,
    in_flight: Option<InFlight>,
}

/// The state of one topic.
#[derive(Debug, Default)]
pub(super) struct TopicState {
    pub(super) observers: Vec<RegisteredObserver>,
    next_seq: u32,
    /// The last samples, oldest first; at least the last one.
    history: VecDeque<Stored>,
    /// When the last sample came; the deadline counts from it.
    last_at: Option<Instant>,
    /// Whether the missed deadline has been reported.
    deadline_missed: bool,
}

impl TopicState {
    fn next_sequence(&mut self) -> u32 {
        self.next_seq = (self.next_seq.wrapping_add(1) & SEQUENCE_MASK).max(2);
        self.next_seq
    }
}

/// The timing of notifications and pings.
#[derive(Debug, Clone, Copy)]
pub(super) struct Timing {
    /// `coap.ack_timeout_ms` (RFC 7252 §4.8 ACK_TIMEOUT).
    pub(super) ack_timeout: Duration,
    /// `coap.max_retransmit` (RFC 7252 §4.8 MAX_RETRANSMIT).
    pub(super) max_retransmit: u8,
    /// `coap.observe_max_age_secs`: the longest an observer goes without
    /// acknowledging a notification, at most 24 h (RFC 7641 §4.5).
    pub(super) max_unconfirmed: Duration,
    /// `coap.keep_alive_secs`: the period of the ping to each observer.
    pub(super) keep_alive: Option<Duration>,
}

impl Timing {
    /// From the configuration.
    pub(super) fn of(cfg: &super::config::DaemonConfig) -> Self {
        let max_unconfirmed = match cfg.observe_max_age_secs {
            0 => DAY,
            s => Duration::from_secs(u64::from(s)).min(DAY),
        };
        Self {
            ack_timeout: Duration::from_millis(u64::from(cfg.ack_timeout_ms.max(1))),
            max_retransmit: cfg.max_retransmit,
            max_unconfirmed,
            keep_alive: (cfg.keep_alive_secs > 0)
                .then(|| Duration::from_secs(u64::from(cfg.keep_alive_secs))),
        }
    }
}

/// The daemon's ping to an endpoint (spec §6, RFC 7252 §4.3).
#[derive(Debug, Clone)]
struct Ping {
    transport: Observer,
    sent_at: Instant,
    in_flight: Option<(u16, Retransmission)>,
}

/// The observe registry of a daemon.
pub(super) struct Registry {
    pub(super) state: Mutex<BTreeMap<String, TopicState>>,
    mid: Arc<AtomicU16>,
    socket: UdpSocket,
    timing: Timing,
    pings: Mutex<BTreeMap<Endpoint, Ping>>,
    started: Instant,
}

/// What a request does to the registry, applied once its response is
/// sent.
#[derive(Debug)]
pub(super) enum ObserveDirective {
    /// Nothing.
    None,
    /// RFC 7641 §4.1: add the observer, or replace the one of the same
    /// endpoint and token; then send it the rest of the replay window.
    Register {
        /// The topic's path.
        path: String,
        /// The token.
        token: Vec<u8>,
        /// OSCORE exchange parameters, when it registered protected.
        oscore: Option<OscoreExchange>,
        /// The largest block of a notification.
        block_szx: u8,
        /// The form it asked for.
        form: Option<Form>,
        /// The samples after the one of the first response.
        replay: Vec<Stored>,
    },
    /// RFC 7641 §3.6: remove it.
    Deregister {
        /// The topic's path.
        path: String,
        /// The token.
        token: Vec<u8>,
    },
}

/// The window a new observer of a topic is given (spec §4.3, §6).
pub(super) struct Window {
    /// The Observe value of the first response.
    pub(super) sequence: u32,
    /// The sample of the first response; `None` on a VOLATILE topic and
    /// before the first.
    pub(super) first: Option<Stored>,
    /// The samples after it.
    pub(super) rest: Vec<Stored>,
}

impl Registry {
    /// A registry sending over `socket`.
    pub(super) fn new(socket: UdpSocket, mid: Arc<AtomicU16>, timing: Timing) -> Self {
        Self {
            state: Mutex::new(BTreeMap::new()),
            mid,
            socket,
            timing,
            pings: Mutex::new(BTreeMap::new()),
            started: Instant::now(),
        }
    }

    /// A Message ID of the daemon's own messages.
    pub(super) fn next_mid(&self) -> u16 {
        self.mid.fetch_add(1, Ordering::SeqCst)
    }

    fn send(&self, transport: &Observer, msg: &CoapMessage) -> bool {
        match transport {
            Observer::Udp { addr, .. } => encode(msg)
                .ok()
                .and_then(|b| self.socket.send_to(&b, addr).ok())
                .is_some(),
            #[cfg(feature = "dtls")]
            Observer::Dtls { tx, .. } => tx.send(msg.clone()),
        }
    }
}

impl Core {
    /// The current sample of the topic at `path`: the last one, unless its
    /// lifespan has passed.
    pub(super) fn current(&self, path: &str, topic: &Topic, now: Instant) -> Option<Stored> {
        let state = self.observe.state.lock().ok()?;
        state
            .get(path)
            .and_then(|t| t.history.back())
            .filter(|s| !s.expired(topic.behavior.lifespan, now))
            .cloned()
    }

    /// Whether the topic at `path` has gone longer than its deadline
    /// without a sample (spec §6); the deadline counts from the start.
    pub(super) fn deadline_missed(&self, path: &str, topic: &Topic, now: Instant) -> bool {
        let Some(deadline) = topic.behavior.deadline else {
            return false;
        };
        let last = self
            .observe
            .state
            .lock()
            .ok()
            .and_then(|s| s.get(path).and_then(|t| t.last_at))
            .unwrap_or(self.observe.started);
        now.saturating_duration_since(last) > deadline
    }

    /// The window of a new observer of the topic at `path`: the first
    /// response's sample and the ones after it, with a fresh Observe value.
    pub(super) fn window(&self, path: &str, topic: &Topic, now: Instant) -> Window {
        let Ok(mut state) = self.observe.state.lock() else {
            return Window {
                sequence: 2,
                first: None,
                rest: Vec::new(),
            };
        };
        let entry = state.entry(path.to_string()).or_default();
        let sequence = entry.next_sequence();
        let depth = topic.behavior.replay_depth;
        let mut samples: Vec<Stored> = entry
            .history
            .iter()
            .filter(|s| !s.expired(topic.behavior.lifespan, now))
            .cloned()
            .collect();
        if depth == 0 {
            samples.clear();
        } else if samples.len() > depth {
            samples.drain(..samples.len() - depth);
        }
        let mut samples = samples.into_iter();
        Window {
            sequence,
            first: samples.next(),
            rest: samples.collect(),
        }
    }

    /// Applies what a request did to the registry, after its response is
    /// sent; `make_observer` builds the transport of the client's endpoint.
    pub(super) fn apply_observe(
        &self,
        directive: ObserveDirective,
        make_observer: &dyn Fn(Vec<u8>) -> Observer,
    ) {
        match directive {
            ObserveDirective::None => {}
            ObserveDirective::Register {
                path,
                token,
                oscore,
                block_szx,
                form,
                replay,
            } => {
                let transport = make_observer(token);
                let observer = RegisteredObserver {
                    transport,
                    oscore,
                    block_szx,
                    form,
                    confirmed_at: Instant::now(),
                    recent: VecDeque::new(),
                    in_flight: None,
                };
                let replaced = match self.observe.state.lock() {
                    Ok(mut st) => {
                        let entry = st.entry(path.clone()).or_default();
                        let same = |o: &RegisteredObserver| {
                            o.transport.endpoint() == observer.transport.endpoint()
                                && o.transport.token() == observer.transport.token()
                        };
                        let replaced = entry.observers.iter().any(same);
                        entry.observers.retain(|o| !same(o));
                        entry.observers.push(observer.clone());
                        replaced
                    }
                    Err(_) => return,
                };
                if !replaced {
                    if let Some(topic) = self.topics.get(&path) {
                        self.metrics.observer_added(&topic.label);
                    }
                }
                if let Some(topic) = self.topics.get(&path) {
                    for stored in replay {
                        self.notify_one(&path, topic, &observer, &stored);
                    }
                }
            }
            ObserveDirective::Deregister { path, token } => {
                let endpoint = make_observer(token.clone()).endpoint();
                self.remove_observers(|p, o| {
                    p == path && o.transport.endpoint() == endpoint && o.transport.token() == token
                });
            }
        }
    }

    /// Removes the observers `which` names; returns them.
    fn remove_observers(
        &self,
        which: impl Fn(&str, &RegisteredObserver) -> bool,
    ) -> Vec<(String, RegisteredObserver)> {
        let mut removed = Vec::new();
        if let Ok(mut st) = self.observe.state.lock() {
            for (path, entry) in st.iter_mut() {
                let mut kept = Vec::with_capacity(entry.observers.len());
                for o in entry.observers.drain(..) {
                    if which(path, &o) {
                        removed.push((path.clone(), o));
                    } else {
                        kept.push(o);
                    }
                }
                entry.observers = kept;
            }
        }
        for (path, _) in &removed {
            if let Some(topic) = self.topics.get(path) {
                self.metrics.observer_removed(&topic.label);
            }
        }
        removed
    }

    /// The observers of a DTLS session that ended.
    #[cfg(feature = "dtls")]
    pub(super) fn drop_endpoint(&self, endpoint: Endpoint) {
        self.remove_observers(|_, o| o.transport.endpoint() == endpoint);
        if let Ok(mut pings) = self.observe.pings.lock() {
            pings.remove(&endpoint);
        }
    }

    /// A DDS sample of the topic at `path`: kept, and a notification to
    /// every observer.
    pub(super) fn on_sample(&self, path: &str, stored: Stored) {
        let Some(topic) = self.topics.get(path) else {
            return;
        };
        let keep = topic.behavior.replay_depth.max(1);
        let observers = match self.observe.state.lock() {
            Ok(mut st) => {
                let entry = st.entry(path.to_string()).or_default();
                entry.history.push_back(stored.clone());
                while entry.history.len() > keep {
                    entry.history.pop_front();
                }
                entry.last_at = Some(stored.at);
                entry.deadline_missed = false;
                entry.observers.clone()
            }
            Err(_) => return,
        };
        for observer in &observers {
            self.notify_one(path, topic, observer, &stored);
        }
    }

    /// One notification of `stored` to `observer`, Confirmable where the
    /// topic or the time since the observer's last acknowledgement asks
    /// for it.
    fn notify_one(
        &self,
        path: &str,
        topic: &Topic,
        observer: &RegisteredObserver,
        stored: &Stored,
    ) {
        let now = Instant::now();
        if stored.expired(topic.behavior.lifespan, now) {
            return;
        }
        let (payload, format) = match self.represent(topic, stored, observer.form) {
            Ok(r) => r,
            Err(e) => {
                self.metrics.errors_total.inc();
                super::log::event(super::log::Level::Warn, "notify_skipped")
                    .field("coap_uri", path)
                    .field("dds_topic", &topic.dds_name)
                    .field("reason", &e.to_string())
                    .emit();
                return;
            }
        };
        let confirmable = topic.behavior.confirmable
            || now.saturating_duration_since(observer.confirmed_at)
                >= self.observe.timing.max_unconfirmed;
        let sequence = match self.observe.state.lock() {
            Ok(mut st) => st.entry(path.to_string()).or_default().next_sequence(),
            Err(_) => return,
        };
        let mut msg = CoapMessage {
            version: 1,
            message_type: if confirmable {
                MessageType::Confirmable
            } else {
                MessageType::NonConfirmable
            },
            code: CoapCode::CONTENT,
            message_id: self.observe.next_mid(),
            token: observer.transport.token().to_vec(),
            options: vec![
                CoapOption::observe(sequence),
                CoapOption::content_format(format),
            ],
            payload: Vec::new(),
        };
        if let Some(age) = stored.max_age(topic.behavior.lifespan, now) {
            msg.options.push(CoapOption::new(
                numbers::MAX_AGE,
                OptionValue::Uint(u64::from(age)),
            ));
        }
        // RFC 7959 §2.6: a representation larger than a block goes out as
        // its first block, the client fetches the rest with GET.
        let probe = CoapMessage::new(MessageType::NonConfirmable, CoapCode::GET, 0);
        let cut = representation(&probe, payload, Some(observer.block_szx));
        msg.payload = cut.payload;
        msg.options.extend(
            cut.options
                .into_iter()
                .filter(|o| o.number != numbers::ETAG),
        );
        msg.options.push(CoapOption::new(
            numbers::ETAG,
            OptionValue::Opaque(etag_of(&stored.body)),
        ));
        let Some(msg) = protect(observer, &self.oscore, msg) else {
            self.metrics.errors_total.inc();
            return;
        };
        let mid = msg.message_id;
        // Remember the Message ID, and a Confirmable one as in flight,
        // before it leaves, so that a quick answer finds it; a newer
        // notification takes the place of one still in flight with its
        // retransmission state (RFC 7641 §4.5.2).
        if let Ok(mut st) = self.observe.state.lock() {
            if let Some(o) = st.get_mut(path).and_then(|e| {
                e.observers.iter_mut().find(|o| {
                    o.transport.endpoint() == observer.transport.endpoint()
                        && o.transport.token() == observer.transport.token()
                })
            }) {
                o.recent.push_back(mid);
                while o.recent.len() > RECENT_MIDS {
                    o.recent.pop_front();
                }
                if confirmable {
                    let retransmission = o.in_flight.take().map_or_else(
                        || Retransmission::start(&self.observe.timing, now),
                        |f| f.retransmission,
                    );
                    o.in_flight = Some(InFlight {
                        mid,
                        message: msg.clone(),
                        retransmission,
                    });
                }
            }
        }
        let sent = self.observe.send(&observer.transport, &msg);
        if sent {
            self.metrics.frames_out_total.inc();
            self.metrics.notified(&topic.label);
        }
        if super::log::enabled(super::log::Level::Debug) {
            super::log::event(super::log::Level::Debug, "notify")
                .field("peer", &endpoint_text(observer.transport.endpoint()))
                .field("coap_uri", path)
                .field("dds_topic", &topic.dds_name)
                .number("mid", u64::from(mid))
                .token(observer.transport.token())
                .number("bytes", msg.payload.len() as u64)
                .field("type", if confirmable { "CON" } else { "NON" })
                .field("sent", if sent { "yes" } else { "no" })
                .emit();
        }
    }

    /// An Acknowledgement or Reset from `endpoint` for Message ID `mid`:
    /// a Confirmable notification is acknowledged, a Reset to any recent
    /// notification ends the observation (RFC 7641 §3.6), a ping is
    /// answered. Returns whether the message was the registry's.
    pub(super) fn confirm(&self, endpoint: Endpoint, mid: u16, reset: bool) -> bool {
        let mut matched = false;
        if let Ok(mut pings) = self.observe.pings.lock() {
            if let Some(p) = pings.get_mut(&endpoint) {
                if p.in_flight.as_ref().is_some_and(|(m, _)| *m == mid) {
                    p.in_flight = None;
                    matched = true;
                }
            }
        }
        if reset {
            let removed = self.remove_observers(|_, o| {
                o.transport.endpoint() == endpoint && o.recent.contains(&mid)
            });
            for (path, _) in &removed {
                super::log::event(super::log::Level::Info, "observe_reset")
                    .field("peer", &endpoint_text(endpoint))
                    .field("coap_uri", path)
                    .field("mid", &mid.to_string())
                    .emit();
            }
            return matched || !removed.is_empty();
        }
        if let Ok(mut st) = self.observe.state.lock() {
            for entry in st.values_mut() {
                for o in &mut entry.observers {
                    if o.transport.endpoint() == endpoint
                        && o.in_flight.as_ref().is_some_and(|f| f.mid == mid)
                    {
                        o.in_flight = None;
                        o.confirmed_at = Instant::now();
                        matched = true;
                    }
                }
            }
        }
        matched
    }

    /// The registry's timers: retransmissions, pings, deadlines.
    pub(super) fn tick(&self, now: Instant) {
        self.retransmit(now);
        self.ping(now);
        self.check_deadlines(now);
    }

    fn retransmit(&self, now: Instant) {
        let timing = self.observe.timing;
        let mut resend: Vec<(Observer, CoapMessage)> = Vec::new();
        let mut gone: Vec<(Endpoint, Vec<u8>)> = Vec::new();
        if let Ok(mut st) = self.observe.state.lock() {
            for entry in st.values_mut() {
                for o in &mut entry.observers {
                    let Some(f) = o.in_flight.as_mut() else {
                        continue;
                    };
                    match f.retransmission.due(&timing, now) {
                        Some(true) => resend.push((o.transport.clone(), f.message.clone())),
                        Some(false) => {}
                        None => gone.push((o.transport.endpoint(), o.transport.token().to_vec())),
                    }
                }
            }
        }
        for (transport, msg) in &resend {
            if self.observe.send(transport, msg) {
                self.metrics.frames_out_total.inc();
            }
        }
        if !gone.is_empty() {
            // RFC 7641 §4.5: a Confirmable notification that times out
            // ends the observation.
            let removed = self.remove_observers(|_, o| {
                gone.iter()
                    .any(|(e, t)| *e == o.transport.endpoint() && t == o.transport.token())
            });
            for (path, o) in removed {
                super::log::event(super::log::Level::Info, "observe_timeout")
                    .field("peer", &endpoint_text(o.transport.endpoint()))
                    .field("coap_uri", &path)
                    .emit();
            }
        }
    }

    fn ping(&self, now: Instant) {
        let timing = self.observe.timing;
        let Some(period) = timing.keep_alive else {
            return;
        };
        // The endpoints with an observation, each with a way to reach it.
        let endpoints: BTreeMap<Endpoint, Observer> = match self.observe.state.lock() {
            Ok(st) => st
                .values()
                .flat_map(|e| e.observers.iter())
                .map(|o| (o.transport.endpoint(), o.transport.clone()))
                .collect(),
            Err(_) => return,
        };
        let mut send: Vec<(Observer, u16)> = Vec::new();
        let mut dead: Vec<Endpoint> = Vec::new();
        if let Ok(mut pings) = self.observe.pings.lock() {
            pings.retain(|e, _| endpoints.contains_key(e));
            for (endpoint, transport) in &endpoints {
                let p = pings.entry(*endpoint).or_insert_with(|| Ping {
                    transport: transport.clone(),
                    sent_at: now,
                    in_flight: None,
                });
                match p.in_flight.as_mut() {
                    Some((mid, r)) => match r.due(&timing, now) {
                        Some(true) => send.push((p.transport.clone(), *mid)),
                        Some(false) => {}
                        None => dead.push(*endpoint),
                    },
                    None if now.saturating_duration_since(p.sent_at) >= period => {
                        let mid = self.observe.next_mid();
                        p.sent_at = now;
                        p.in_flight = Some((mid, Retransmission::start(&timing, now)));
                        send.push((p.transport.clone(), mid));
                    }
                    None => {}
                }
            }
            for e in &dead {
                pings.remove(e);
            }
        }
        for (transport, mid) in send {
            // RFC 7252 §4.3: an empty Confirmable message, answered with a
            // Reset.
            let ping = CoapMessage::new(MessageType::Confirmable, CoapCode::EMPTY, mid);
            if self.observe.send(&transport, &ping) {
                self.metrics.frames_out_total.inc();
            }
        }
        for endpoint in dead {
            let removed = self.remove_observers(|_, o| o.transport.endpoint() == endpoint);
            super::log::event(super::log::Level::Info, "ping_unanswered")
                .field("peer", &endpoint_text(endpoint))
                .field("bytes", &removed.len().to_string())
                .emit();
        }
    }

    fn check_deadlines(&self, now: Instant) {
        let mut ended: Vec<(String, RegisteredObserver)> = Vec::new();
        if let Ok(mut st) = self.observe.state.lock() {
            for (path, topic) in &self.topics {
                let Some(deadline) = topic.behavior.deadline else {
                    continue;
                };
                let entry = st.entry(path.clone()).or_default();
                let last = entry.last_at.unwrap_or(self.observe.started);
                if entry.deadline_missed || now.saturating_duration_since(last) <= deadline {
                    continue;
                }
                entry.deadline_missed = true;
                for o in entry.observers.drain(..) {
                    ended.push((path.clone(), o));
                }
            }
        }
        for (path, o) in &ended {
            if let Some(topic) = self.topics.get(path) {
                self.metrics.observer_removed(&topic.label);
            }
            // Spec §6: 5.03 with Max-Age 0; a notification with a code
            // other than 2.xx ends the observation (RFC 7641 §4.2).
            let msg = CoapMessage {
                version: 1,
                message_type: MessageType::NonConfirmable,
                code: CoapCode::new(5, 3),
                message_id: self.observe.next_mid(),
                token: o.transport.token().to_vec(),
                options: vec![CoapOption::new(numbers::MAX_AGE, OptionValue::Uint(0))],
                payload: b"deadline missed".to_vec(),
            };
            if let Some(msg) = protect(o, &self.oscore, msg) {
                if self.observe.send(&o.transport, &msg) {
                    self.metrics.frames_out_total.inc();
                }
            }
            super::log::event(super::log::Level::Warn, "deadline_missed")
                .field("peer", &endpoint_text(o.transport.endpoint()))
                .field("coap_uri", path)
                .emit();
        }
    }

    /// Spec §9.2: every observer gets `5.03 Service Unavailable` without an
    /// Observe option, which ends its observation (RFC 7641 §4.2), and the
    /// registry is emptied.
    pub(super) fn end_all(&self) {
        let observers = self.remove_observers(|_, _| true);
        for (_, o) in &observers {
            let msg = CoapMessage {
                version: 1,
                message_type: MessageType::NonConfirmable,
                code: CoapCode::new(5, 3),
                message_id: self.observe.next_mid(),
                token: o.transport.token().to_vec(),
                options: Vec::new(),
                payload: b"shutting down".to_vec(),
            };
            if let Some(msg) = protect(o, &self.oscore, msg) {
                // A DTLS session sends what was queued before it ends.
                self.observe.send(&o.transport, &msg);
            }
        }
        if !observers.is_empty() {
            super::log::event(super::log::Level::Info, "observations_ended")
                .field("bytes", &observers.len().to_string())
                .emit();
        }
    }
}

/// A notification to an OSCORE observer leaves protected or not at all
/// (RFC 8613 §4.1.3.5.2, the daemon's own Partial IV).
#[allow(clippy::unnecessary_wraps)]
fn protect(
    observer: &RegisteredObserver,
    gateway: &Option<Arc<OscoreGateway>>,
    msg: CoapMessage,
) -> Option<CoapMessage> {
    match (&observer.oscore, gateway.as_ref()) {
        (None, _) => Some(msg),
        #[cfg(feature = "oscore")]
        (Some(ex), Some(gw)) => gw.protect_notification(ex, &msg).ok(),
        (Some(_), _) => None,
    }
}

fn endpoint_text(endpoint: Endpoint) -> String {
    match endpoint {
        Endpoint::Udp(addr) => addr.to_string(),
        #[cfg(feature = "dtls")]
        Endpoint::Dtls(session) => format!("dtls:{session}"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn timing() -> Timing {
        Timing {
            ack_timeout: Duration::from_millis(100),
            max_retransmit: 2,
            max_unconfirmed: Duration::from_secs(60),
            keep_alive: None,
        }
    }

    /// RFC 7252 §4.2: the first timeout is between ACK_TIMEOUT and 1.5
    /// times it; each retransmission doubles it; after MAX_RETRANSMIT
    /// retransmissions the message is given up once the timeout passes.
    #[test]
    fn coap_4_2_a_confirmable_message_is_retransmitted_with_back_off() {
        let t = timing();
        let start = Instant::now();
        let mut r = Retransmission::start(&t, start);
        assert!(r.timeout >= t.ack_timeout && r.timeout < t.ack_timeout * 3 / 2);
        let first = r.timeout;
        assert_eq!(r.due(&t, start), Some(false));
        let mut now = r.next_at;
        assert_eq!(r.due(&t, now), Some(true));
        assert_eq!(r.timeout, first * 2);
        now = r.next_at;
        assert_eq!(r.due(&t, now), Some(true));
        assert_eq!(r.timeout, first * 4);
        now = r.next_at;
        assert_eq!(r.due(&t, now), None, "two retransmissions, then given up");
    }

    /// RFC 7641 §3.4: the Observe value is 24 bits; it wraps and never
    /// takes the values 0 and 1 of the register and deregister requests.
    #[test]
    fn rfc7641_3_4_the_sequence_number_is_24_bits() {
        let mut t = TopicState {
            next_seq: SEQUENCE_MASK - 1,
            ..TopicState::default()
        };
        assert_eq!(t.next_sequence(), SEQUENCE_MASK);
        assert_eq!(t.next_sequence(), 2);
        assert_eq!(t.next_sequence(), 3);
    }

    /// Spec §6 with RFC 7252 §5.10.5: the Max-Age of a sample is what is
    /// left of the lifespan, rounded up; without a lifespan there is none.
    #[test]
    fn coapbridge_6_the_max_age_is_what_is_left_of_the_lifespan() {
        let at = Instant::now();
        let s = Stored {
            body: Arc::from(&[1u8][..]),
            identifier: [0, 7],
            at,
        };
        let life = Some(Duration::from_millis(2500));
        assert_eq!(s.max_age(life, at), Some(3));
        assert_eq!(s.max_age(life, at + Duration::from_millis(1600)), Some(1));
        assert_eq!(s.max_age(life, at + Duration::from_secs(3)), Some(0));
        assert!(s.expired(life, at + Duration::from_secs(3)));
        assert!(!s.expired(life, at + Duration::from_secs(2)));
        assert_eq!(s.max_age(None, at), None);
        assert!(!s.expired(None, at + DAY));
    }

    /// RFC 7641 §4.1: an observer is its endpoint and its token; a UDP
    /// observer's endpoint is the peer's address.
    #[test]
    fn rfc7641_4_1_an_observer_is_named_by_endpoint_and_token() {
        let addr: SocketAddr = "127.0.0.1:5683".parse().unwrap();
        let o = Observer::Udp {
            addr,
            token: vec![1, 2],
        };
        assert_eq!(o.endpoint(), Endpoint::Udp(addr));
        assert_eq!(o.token(), &[1, 2]);
        assert_ne!(
            Endpoint::Udp(addr),
            Endpoint::Udp("127.0.0.1:5684".parse().unwrap())
        );
        assert_eq!(endpoint_text(o.endpoint()), "127.0.0.1:5683");
    }

    /// RFC 7641 §4.5: at least every 24 hours a notification is
    /// Confirmable; `observe_max_age_secs` sets a shorter bound, 0 leaves
    /// the 24 hours.
    #[test]
    fn rfc7641_4_5_the_bound_of_unconfirmed_notifications() {
        let mut cfg = super::super::config::DaemonConfig::default_for_dev();
        assert_eq!(Timing::of(&cfg).max_unconfirmed, Duration::from_secs(60));
        cfg.observe_max_age_secs = 0;
        assert_eq!(Timing::of(&cfg).max_unconfirmed, DAY);
        cfg.observe_max_age_secs = 3 * 24 * 60 * 60;
        assert_eq!(Timing::of(&cfg).max_unconfirmed, DAY);
        assert_eq!(Timing::of(&cfg).keep_alive, None);
        cfg.keep_alive_secs = 5;
        assert_eq!(Timing::of(&cfg).keep_alive, Some(Duration::from_secs(5)));
    }
}
