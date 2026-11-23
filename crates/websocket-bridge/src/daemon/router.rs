// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Topic router for the `zerodds-ws-bridged` daemon.
//!
//! The router holds:
//! * A list of subscribed connection IDs per topic.
//! * The send queue of every connection ([`ConnQueue`]), bounded by
//!   `max_send_queue_per_connection` (Spec §6).
//! * Per topic the behaviour of §6 ([`WsBehavior`]): what a full queue does
//!   (RELIABLE waits, BEST_EFFORT drops the oldest sample), the samples a
//!   durable topic keeps for a connection that subscribes later, and the
//!   deadline of its instances.
//!
//! The router is `Send + Sync` and can be consumed via `Arc<Mutex<...>>`
//! by all worker threads. A sample is routed under the lock and handed to
//! the queues outside it ([`Route::deliver`]), so that a RELIABLE topic
//! waiting for a slow client does not hold the lock.

use std::collections::{BTreeMap, VecDeque};
use std::string::String;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use std::vec::Vec;

use super::qos_translation::{BackpressureMode, DeadlineMode, ReplayMode, WsBehavior};

/// What happened to an instance with a sample (DDS 1.4 §2.2.2.5.1.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleKind {
    /// A sample of the instance.
    Alive,
    /// The instance is disposed.
    Disposed,
    /// Its writer unregistered it.
    Unregistered,
    /// Both at once.
    DisposedUnregistered,
}

/// A DDS sample for the clients of a topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdsSample {
    /// The CDR body without the encapsulation header; for a dispose or an
    /// unregister the key holder (DDS-XTypes 1.3 §7.6.8), when known.
    pub payload: Vec<u8>,
    /// The representation identifier of the header `payload` came under
    /// (DDSI-RTPS 2.5 §10.5, DDS-XTypes 1.3 Table 60): version, format and
    /// byte order; `00 06` (CDR2_BE) for a key holder.
    pub representation_identifier: [u8; 2],
    /// What the sample does to its instance.
    pub kind: SampleKind,
    /// The instance's KeyHash (DDSI-RTPS 2.5 §9.6.4.8), when known.
    pub key_hash: Option<[u8; 16]>,
    /// The source time in nanoseconds since the Unix epoch, when known.
    pub timestamp_ns: Option<i64>,
    /// Whether the writer wrote the sample inside a coherent set (DDS 1.4
    /// §2.2.2.4.1.8, DDSI-RTPS 2.5 §8.7.6 `PID_COHERENT_SET`): flag bit 1
    /// of a binary frame (Spec §4.3).
    pub coherent_set: bool,
}

impl DdsSample {
    /// A sample of a topic without key, under `representation_identifier`.
    #[must_use]
    pub fn alive(payload: Vec<u8>, representation_identifier: [u8; 2]) -> Self {
        Self {
            payload,
            representation_identifier,
            kind: SampleKind::Alive,
            key_hash: None,
            timestamp_ns: None,
            coherent_set: false,
        }
    }
}

/// Message to a connection writer thread.
#[derive(Debug, Clone)]
pub enum RouterMsg {
    /// DDS sample on a topic — push as a WS frame.
    Sample {
        /// DDS topic name.
        topic: String,
        /// The sample.
        sample: DdsSample,
    },
    /// The samples a durable topic keeps, oldest first, for a connection
    /// that subscribes to it (Spec §6); sent before its live samples.
    Burst {
        /// DDS topic name.
        topic: String,
        /// The kept samples.
        samples: Vec<DdsSample>,
    },
    /// A control message of the daemon (§4.4), a text frame.
    Text(String),
    /// A control message about a topic (`deadline_missed`, §6), a text
    /// frame. Like a sample it reaches the client only while the read ACL
    /// allows the topic (§7.3).
    TopicText {
        /// DDS topic name.
        topic: String,
        /// The text of the frame.
        text: String,
    },
    /// Daemon shutdown — close the connection with code 1001.
    Shutdown,
}

/// What handing a sample to a connection's queue did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Push {
    /// The sample is queued.
    Queued,
    /// The queue was full: the oldest queued sample of a BEST_EFFORT topic
    /// was dropped for it (Spec §6).
    DroppedOldest,
    /// The queue was full of samples of RELIABLE topics, which are never
    /// dropped: the new BEST_EFFORT sample was dropped (Spec §6).
    DroppedNew,
    /// The connection is gone, or the daemon stops.
    Closed,
}

/// How many control messages (`subscribed`, `deadline_missed`) may wait in
/// the send queue of one connection (Spec §6). A client that lets more pile
/// up, by subscribing again and again without reading, overflows its queue:
/// the connection gets a Close 1008 and is closed.
pub const MAX_QUEUED_CONTROLS: usize = 1024;

/// The send queue of one connection (Spec §6). At most `capacity` samples
/// wait in it; a burst, a control message and the shutdown are not counted
/// there. At most one burst of each topic waits, of at most
/// `max_burst_samples` samples: a newer burst of the topic replaces it. At
/// most [`MAX_QUEUED_CONTROLS`] control messages wait; one more overflows
/// the queue ([`Self::overflowed`]).
#[derive(Debug)]
pub struct ConnQueue {
    state: Mutex<QueueState>,
    ready: Condvar,
    space: Condvar,
    capacity: usize,
}

#[derive(Debug, Default)]
struct QueueState {
    items: VecDeque<Entry>,
    samples: usize,
    /// The control messages among `items`.
    controls: usize,
    closed: bool,
    /// A control message found the queue holding [`MAX_QUEUED_CONTROLS`].
    overflowed: bool,
}

/// A queued message and whether it may be dropped for a newer sample: only
/// a sample of a BEST_EFFORT topic may (Spec §6). One connection holds the
/// samples of all its topics in one queue, so the mode travels with each
/// sample.
#[derive(Debug)]
struct Entry {
    msg: RouterMsg,
    droppable: bool,
}

impl ConnQueue {
    /// A queue for at most `capacity` samples (at least 1).
    #[must_use]
    pub fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(QueueState::default()),
            ready: Condvar::new(),
            space: Condvar::new(),
            capacity: capacity.max(1),
        })
    }

    fn lock(&self) -> MutexGuard<'_, QueueState> {
        match self.state.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    /// Hands a sample of a topic with backpressure `mode` to the queue.
    /// With room it is queued. On a full queue [`BackpressureMode::Block`]
    /// waits until the writer takes one, the connection closes or `stop` is
    /// set; [`BackpressureMode::DropOldest`] drops the oldest queued sample
    /// of a BEST_EFFORT topic for it, and the new sample itself when the
    /// queue holds samples of RELIABLE topics only.
    pub fn push_sample(&self, msg: RouterMsg, mode: BackpressureMode, stop: &AtomicBool) -> Push {
        let droppable = mode == BackpressureMode::DropOldest;
        let mut st = self.lock();
        loop {
            if st.closed {
                return Push::Closed;
            }
            if st.samples < self.capacity {
                st.items.push_back(Entry { msg, droppable });
                st.samples += 1;
                self.ready.notify_one();
                return Push::Queued;
            }
            match mode {
                BackpressureMode::DropOldest => {
                    let Some(i) = st.items.iter().position(|e| e.droppable) else {
                        return Push::DroppedNew;
                    };
                    st.items.remove(i);
                    st.items.push_back(Entry { msg, droppable });
                    self.ready.notify_one();
                    return Push::DroppedOldest;
                }
                BackpressureMode::Block => {
                    if stop.load(Ordering::SeqCst) {
                        return Push::Closed;
                    }
                    st = match self.space.wait_timeout(st, Duration::from_millis(50)) {
                        Ok((g, _)) => g,
                        Err(p) => p.into_inner().0,
                    };
                }
            }
        }
    }

    /// Queues a burst, a control message or the shutdown, whatever the
    /// number of waiting samples; `false` when the connection is gone or the
    /// queue overflowed. A burst replaces the burst of its topic that still
    /// waits. A control message that finds [`MAX_QUEUED_CONTROLS`] waiting
    /// is not queued; the queue has overflowed and takes nothing more.
    pub fn push_control(&self, msg: RouterMsg) -> bool {
        let mut st = self.lock();
        if st.closed || st.overflowed {
            return false;
        }
        match &msg {
            RouterMsg::Burst { topic, .. } => {
                let waiting = st.items.iter().position(
                    |e| matches!(&e.msg, RouterMsg::Burst { topic: t, .. } if t == topic),
                );
                if let Some(i) = waiting {
                    st.items.remove(i);
                }
            }
            RouterMsg::Text(_) | RouterMsg::TopicText { .. } => {
                if st.controls >= MAX_QUEUED_CONTROLS {
                    st.overflowed = true;
                    self.ready.notify_all();
                    return false;
                }
                st.controls += 1;
            }
            RouterMsg::Sample { .. } | RouterMsg::Shutdown => {}
        }
        st.items.push_back(Entry {
            msg,
            droppable: false,
        });
        self.ready.notify_one();
        true
    }

    /// The next message, waiting at most `timeout`; `None` when none came
    /// or the queue is closed and empty.
    pub fn pop(&self, timeout: Duration) -> Option<RouterMsg> {
        let deadline = Instant::now() + timeout;
        let mut st = self.lock();
        loop {
            if let Some(Entry { msg, .. }) = st.items.pop_front() {
                match msg {
                    RouterMsg::Sample { .. } => {
                        st.samples -= 1;
                        self.space.notify_one();
                    }
                    RouterMsg::Text(_) | RouterMsg::TopicText { .. } => st.controls -= 1,
                    RouterMsg::Burst { .. } | RouterMsg::Shutdown => {}
                }
                return Some(msg);
            }
            if st.closed {
                return None;
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return None;
            }
            st = match self.ready.wait_timeout(st, left) {
                Ok((g, _)) => g,
                Err(p) => p.into_inner().0,
            };
        }
    }

    /// The connection is gone: pushes fail from now on, a waiting push and
    /// the writer wake up.
    pub fn close(&self) {
        self.lock().closed = true;
        self.ready.notify_all();
        self.space.notify_all();
    }

    /// Whether [`Self::close`] was called.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.lock().closed
    }

    /// Whether a control message found [`MAX_QUEUED_CONTROLS`] waiting: the
    /// connection's client does not read what it asks for (Spec §6).
    #[must_use]
    pub fn overflowed(&self) -> bool {
        self.lock().overflowed
    }

    /// The number of samples waiting.
    #[must_use]
    pub fn samples(&self) -> usize {
        self.lock().samples
    }
}

/// The state of one topic: its behaviour, the samples it keeps for late
/// subscribers, the deadline of each instance.
#[derive(Debug, Default)]
struct TopicState {
    behavior: WsBehavior,
    /// Whether the topic's type has a key (its `idl_file` declares one):
    /// then a sample without a KeyHash names no instance.
    keyed: bool,
    kept: VecDeque<DdsSample>,
    /// Instance (its KeyHash, `None` for a topic without key) → when its
    /// next sample is due.
    due: BTreeMap<Option<[u8; 16]>, Instant>,
}

impl TopicState {
    /// Keeps `sample` as the replay mode says, at most `max_burst`
    /// samples in all.
    fn keep(&mut self, sample: &DdsSample, max_burst: usize) {
        match self.behavior.replay {
            ReplayMode::None => return,
            ReplayMode::LastN(n) => {
                let n = usize::try_from(n).unwrap_or(usize::MAX).max(1);
                let same = self
                    .kept
                    .iter()
                    .filter(|k| k.key_hash == sample.key_hash)
                    .count();
                if same >= n {
                    if let Some(i) = self.kept.iter().position(|k| k.key_hash == sample.key_hash) {
                        self.kept.remove(i);
                    }
                }
            }
            ReplayMode::All => {}
        }
        self.kept.push_back(sample.clone());
        while self.kept.len() > max_burst {
            self.kept.pop_front();
        }
    }

    /// A sample of an instance restarts its deadline; a dispose or an
    /// unregister ends it (the instance is no longer alive).
    fn observe(&mut self, sample: &DdsSample, now: Instant) {
        if let DeadlineMode::Emit { period_ms } = self.behavior.deadline {
            if sample.kind == SampleKind::Alive {
                self.due
                    .insert(sample.key_hash, now + Duration::from_millis(period_ms));
            } else {
                self.due.remove(&sample.key_hash);
            }
        }
    }
}

/// A sample routed under the router's lock, to be handed to its
/// connections outside it.
#[derive(Debug)]
pub struct Route {
    topic: String,
    sample: DdsSample,
    targets: Vec<(u64, Arc<ConnQueue>)>,
    mode: BackpressureMode,
    instance_unknown: bool,
}

/// What [`Route::deliver`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Delivery {
    /// Connections that got the sample.
    pub queued: usize,
    /// Samples dropped on a full queue (BEST_EFFORT): the oldest
    /// BEST_EFFORT sample of a connection that got this one, or this sample
    /// for a connection whose queue holds RELIABLE samples only.
    pub dropped: usize,
    /// The connections that dropped a sample, one entry per drop (the
    /// `connection_id` of `send_queue_drops_total`, Spec §8.2).
    pub dropped_on: Vec<u64>,
}

impl Route {
    /// Whether the sample is of a topic with key and carries no KeyHash:
    /// it names no instance, so it was neither kept for late subscribers
    /// nor counted for an instance's deadline (Spec §6). It still goes to
    /// the subscribed connections.
    #[must_use]
    pub fn instance_unknown(&self) -> bool {
        self.instance_unknown
    }

    /// Hands the sample to every subscribed connection: RELIABLE waits for
    /// room in a full queue (until the connection closes or `stop` is
    /// set), BEST_EFFORT drops that queue's oldest BEST_EFFORT sample, or
    /// this one when there is none.
    pub fn deliver(self, stop: &AtomicBool) -> Delivery {
        let mut d = Delivery::default();
        for (id, q) in &self.targets {
            let msg = RouterMsg::Sample {
                topic: self.topic.clone(),
                sample: self.sample.clone(),
            };
            match q.push_sample(msg, self.mode, stop) {
                Push::Queued => d.queued += 1,
                Push::DroppedOldest => {
                    d.queued += 1;
                    d.dropped += 1;
                    d.dropped_on.push(*id);
                }
                Push::DroppedNew => {
                    d.dropped += 1;
                    d.dropped_on.push(*id);
                }
                Push::Closed => {}
            }
        }
        d
    }
}

/// Router-State.
#[derive(Debug)]
pub struct Router {
    /// `topic → list-of-connection-ids`.
    subs: BTreeMap<String, Vec<u64>>,
    /// `connection-id → send queue`.
    conns: BTreeMap<u64, Arc<ConnQueue>>,
    /// The behaviour and the kept samples of each configured topic.
    topics: BTreeMap<String, TopicState>,
    /// `max_burst_samples` (Spec §6).
    max_burst: usize,
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    /// A router without topic behaviours: every topic waits on a full
    /// queue and keeps nothing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            subs: BTreeMap::new(),
            conns: BTreeMap::new(),
            topics: BTreeMap::new(),
            max_burst: usize::MAX,
        }
    }

    /// At most `max_burst` samples are kept per topic and sent to a
    /// connection that subscribes (`max_burst_samples`, Spec §6).
    #[must_use]
    pub fn with_max_burst(mut self, max_burst: usize) -> Self {
        self.max_burst = max_burst;
        self
    }

    /// The behaviour of §6 for `topic`.
    pub fn configure_topic(&mut self, topic: &str, behavior: WsBehavior) {
        self.topics.entry(topic.to_string()).or_default().behavior = behavior;
    }

    /// `topic` has a type with key: its instances are told apart by the
    /// KeyHash of each sample (Spec §6).
    pub fn set_keyed(&mut self, topic: &str, keyed: bool) {
        self.topics.entry(topic.to_string()).or_default().keyed = keyed;
    }

    /// Registers a new connection with its send queue.
    pub fn register_connection(&mut self, id: u64, queue: Arc<ConnQueue>) {
        self.conns.insert(id, queue);
    }

    /// Removes a connection (cleanup on disconnect) and closes its queue.
    pub fn deregister_connection(&mut self, id: u64) {
        if let Some(q) = self.conns.remove(&id) {
            q.close();
        }
        for subs in self.subs.values_mut() {
            subs.retain(|c| *c != id);
        }
    }

    /// Subscribes a connection to a topic. A connection that was not
    /// subscribed gets the samples the topic keeps (Spec §6), before any
    /// live sample.
    pub fn subscribe(&mut self, conn_id: u64, topic: String) {
        let entry = self.subs.entry(topic.clone()).or_default();
        if entry.contains(&conn_id) {
            return;
        }
        entry.push(conn_id);
        let kept = self
            .topics
            .get(&topic)
            .map(|t| t.kept.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        if !kept.is_empty() {
            if let Some(q) = self.conns.get(&conn_id) {
                q.push_control(RouterMsg::Burst {
                    topic,
                    samples: kept,
                });
            }
        }
    }

    /// Unsubscribe.
    pub fn unsubscribe(&mut self, conn_id: u64, topic: &str) {
        if let Some(list) = self.subs.get_mut(topic) {
            list.retain(|c| *c != conn_id);
        }
    }

    /// Takes a DDS sample of `topic`: keeps it for late subscribers, restarts
    /// its instance's deadline, and names the connections to hand it to.
    pub fn route(&mut self, topic: &str, sample: DdsSample) -> Route {
        self.route_at(topic, sample, Instant::now())
    }

    /// [`Self::route`] at the time `now`.
    pub fn route_at(&mut self, topic: &str, sample: DdsSample, now: Instant) -> Route {
        let max_burst = self.max_burst;
        let state = self.topics.entry(topic.to_string()).or_default();
        // A sample of a topic with key that names no instance is not merged
        // into one: it is neither kept nor counted for a deadline.
        let instance_unknown = state.keyed && sample.key_hash.is_none();
        if !instance_unknown {
            state.keep(&sample, max_burst);
            state.observe(&sample, now);
        }
        let mode = state.behavior.backpressure;
        self.conns.retain(|_, q| !q.is_closed());
        let targets = self
            .subs
            .get(topic)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.conns.get(id).map(|q| (*id, Arc::clone(q))))
                    .collect()
            })
            .unwrap_or_default();
        Route {
            topic: topic.to_string(),
            sample,
            targets,
            mode,
            instance_unknown,
        }
    }

    /// Sends `{"op":"deadline_missed"}` (Spec §6) to the subscribers of
    /// each topic with an instance whose deadline passed by `now`, once per
    /// missed period. Returns the number of missed deadlines.
    pub fn deadlines_missed(&mut self, now: Instant) -> usize {
        let mut missed = 0;
        for (topic, state) in &mut self.topics {
            let DeadlineMode::Emit { period_ms } = state.behavior.deadline else {
                continue;
            };
            let period = Duration::from_millis(period_ms);
            for (key, due) in &mut state.due {
                if *due > now {
                    continue;
                }
                missed += 1;
                *due = (*due + period).max(now + period / 2);
                let text = super::json_frames::deadline_missed(topic, key.as_ref());
                for id in self.subs.get(topic).into_iter().flatten() {
                    if let Some(q) = self.conns.get(id) {
                        q.push_control(RouterMsg::TopicText {
                            topic: topic.clone(),
                            text: text.clone(),
                        });
                    }
                }
            }
        }
        missed
    }

    /// Queues a control message for one connection.
    pub fn send_text(&self, conn_id: u64, text: String) -> bool {
        self.conns
            .get(&conn_id)
            .is_some_and(|q| q.push_control(RouterMsg::Text(text)))
    }

    /// Sends `Shutdown` to all connections (graceful drain).
    pub fn broadcast_shutdown(&self) {
        for q in self.conns.values() {
            q.push_control(RouterMsg::Shutdown);
        }
    }

    /// Number of active connections.
    #[must_use]
    pub fn connection_count(&self) -> usize {
        self.conns.len()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    static NO_STOP: AtomicBool = AtomicBool::new(false);

    fn sample(n: u8, key: Option<u8>) -> DdsSample {
        let mut s = DdsSample::alive(vec![n], [0x00, 0x07]);
        s.key_hash = key.map(|k| [k; 16]);
        s
    }

    fn payloads(msgs: &[RouterMsg]) -> Vec<u8> {
        msgs.iter()
            .flat_map(|m| match m {
                RouterMsg::Sample { sample, .. } => vec![sample.payload[0]],
                RouterMsg::Burst { samples, .. } => samples.iter().map(|s| s.payload[0]).collect(),
                _ => Vec::new(),
            })
            .collect()
    }

    fn drain(q: &ConnQueue) -> Vec<RouterMsg> {
        core::iter::from_fn(|| q.pop(Duration::from_millis(1))).collect()
    }

    fn behavior(backpressure: BackpressureMode, replay: ReplayMode) -> WsBehavior {
        WsBehavior {
            backpressure,
            replay,
            ..WsBehavior::default()
        }
    }

    #[test]
    fn wsbridge_5_1_dispatch_to_subscribed_connection() {
        let mut router = Router::new();
        let q = ConnQueue::new(8);
        router.register_connection(1, Arc::clone(&q));
        router.subscribe(1, "Trade".to_string());
        let s = DdsSample::alive(b"PAYLOAD".to_vec(), [0x00, 0x07]);
        let d = router.route("Trade", s.clone()).deliver(&NO_STOP);
        assert_eq!(d.queued, 1);
        match q.pop(Duration::from_millis(10)).unwrap() {
            RouterMsg::Sample { topic, sample: got } => {
                assert_eq!(topic, "Trade");
                assert_eq!(got, s);
            }
            other => panic!("unexpected msg {other:?}"),
        }
    }

    #[test]
    fn dispatch_to_no_subscribers_is_zero() {
        let mut router = Router::new();
        let d = router.route("Empty", sample(1, None)).deliver(&NO_STOP);
        assert_eq!(d.queued, 0);
    }

    #[test]
    fn unsubscribe_stops_delivery() {
        let mut router = Router::new();
        let q = ConnQueue::new(8);
        router.register_connection(2, Arc::clone(&q));
        router.subscribe(2, "T".to_string());
        router.unsubscribe(2, "T");
        assert_eq!(
            router.route("T", sample(1, None)).deliver(&NO_STOP).queued,
            0
        );
        assert!(q.pop(Duration::from_millis(1)).is_none());
    }

    #[test]
    fn deregister_removes_subscription() {
        let mut router = Router::new();
        let q = ConnQueue::new(8);
        router.register_connection(3, Arc::clone(&q));
        router.subscribe(3, "T".to_string());
        router.deregister_connection(3);
        assert_eq!(router.connection_count(), 0);
        assert!(q.is_closed());
    }

    #[test]
    fn wsbridge_9_2_shutdown_broadcasts_to_all() {
        let mut router = Router::new();
        let q = ConnQueue::new(1);
        router.register_connection(7, Arc::clone(&q));
        router.subscribe(7, "T".to_string());
        // The shutdown reaches a connection whose queue is full.
        router.route("T", sample(1, None)).deliver(&NO_STOP);
        router.broadcast_shutdown();
        let msgs = drain(&q);
        assert!(matches!(msgs.last(), Some(RouterMsg::Shutdown)), "{msgs:?}");
    }

    /// Spec §6: BEST_EFFORT drops on a full send queue, the oldest sample
    /// first, and the queue never holds more than
    /// `max_send_queue_per_connection` samples.
    #[test]
    fn wsbridge_6_best_effort_drops_on_a_full_queue() {
        let mut router = Router::new();
        router.configure_topic(
            "T",
            behavior(BackpressureMode::DropOldest, ReplayMode::None),
        );
        let q = ConnQueue::new(3);
        router.register_connection(1, Arc::clone(&q));
        router.subscribe(1, "T".to_string());
        let mut dropped = 0;
        for n in 1..=5 {
            dropped += router.route("T", sample(n, None)).deliver(&NO_STOP).dropped;
        }
        assert_eq!(dropped, 2);
        assert_eq!(q.samples(), 3);
        assert_eq!(payloads(&drain(&q)), [3, 4, 5]);
    }

    /// Spec §6: one connection subscribed to a RELIABLE and a BEST_EFFORT
    /// topic shares one queue; a BEST_EFFORT sample on a full queue drops
    /// the oldest BEST_EFFORT sample, never a RELIABLE one, and itself when
    /// the queue holds RELIABLE samples only. Every drop is counted.
    #[test]
    fn wsbridge_6_a_best_effort_sample_never_drops_a_reliable_one() {
        let mut router = Router::new();
        router.configure_topic("R", behavior(BackpressureMode::Block, ReplayMode::None));
        router.configure_topic(
            "B",
            behavior(BackpressureMode::DropOldest, ReplayMode::None),
        );
        let q = ConnQueue::new(3);
        router.register_connection(7, Arc::clone(&q));
        router.subscribe(7, "R".to_string());
        router.subscribe(7, "B".to_string());
        // Queue: R1, B2, R3 (full).
        for (topic, n) in [("R", 1), ("B", 2), ("R", 3)] {
            let d = router.route(topic, sample(n, None)).deliver(&NO_STOP);
            assert_eq!((d.queued, d.dropped), (1, 0));
        }
        // B4 drops B2, the only BEST_EFFORT sample: R1, R3, B4.
        let d = router.route("B", sample(4, None)).deliver(&NO_STOP);
        assert_eq!((d.queued, d.dropped), (1, 1));
        // §8.2: the drop is counted under the connection that dropped.
        assert_eq!(d.dropped_on, [7]);
        assert_eq!(payloads(&drain(&q)), [1, 3, 4]);
        // Queue full of RELIABLE samples: R5, R6, R7. B8 is dropped itself.
        for n in 5..=7 {
            router.route("R", sample(n, None)).deliver(&NO_STOP);
        }
        let d = router.route("B", sample(8, None)).deliver(&NO_STOP);
        assert_eq!((d.queued, d.dropped), (0, 1));
        assert_eq!(d.dropped_on, [7]);
        assert_eq!(q.samples(), 3);
        assert_eq!(payloads(&drain(&q)), [5, 6, 7]);
    }

    /// Spec §6: RELIABLE drops nothing: a full queue makes the pump wait
    /// until the writer takes a sample; the stop flag ends the wait.
    #[test]
    fn wsbridge_6_reliable_blocks_on_a_full_queue() {
        let router = Arc::new(Mutex::new(Router::new()));
        let q = ConnQueue::new(2);
        {
            let mut r = router.lock().unwrap();
            r.configure_topic("T", behavior(BackpressureMode::Block, ReplayMode::None));
            r.register_connection(1, Arc::clone(&q));
            r.subscribe(1, "T".to_string());
        }
        let r2 = Arc::clone(&router);
        let pump = std::thread::spawn(move || {
            let stop = AtomicBool::new(false);
            for n in 1..=4 {
                let route = r2.lock().unwrap().route("T", sample(n, None));
                assert_eq!(route.deliver(&stop).dropped, 0);
            }
        });
        std::thread::sleep(Duration::from_millis(200));
        // Two samples fit; the pump waits with the third, and the router
        // is not locked while it waits.
        assert_eq!(q.samples(), 2);
        assert!(!pump.is_finished());
        assert_eq!(router.lock().unwrap().connection_count(), 1);
        let mut got = Vec::new();
        while got.len() < 4 {
            if let Some(m) = q.pop(Duration::from_millis(500)) {
                got.push(m);
            } else {
                panic!("the pump stopped after {got:?}");
            }
        }
        pump.join().unwrap();
        assert_eq!(payloads(&got), [1, 2, 3, 4]);
        // The stop flag ends a wait.
        let full = ConnQueue::new(1);
        let stop = AtomicBool::new(false);
        let first = RouterMsg::Sample {
            topic: "T".into(),
            sample: sample(8, None),
        };
        assert_eq!(
            full.push_sample(first, BackpressureMode::Block, &stop),
            Push::Queued
        );
        stop.store(true, Ordering::SeqCst);
        let msg = RouterMsg::Sample {
            topic: "T".into(),
            sample: sample(9, None),
        };
        assert_eq!(
            full.push_sample(msg, BackpressureMode::Block, &stop),
            Push::Closed
        );
    }

    /// Spec §6: a TRANSIENT_LOCAL topic keeps the last N samples of each
    /// instance (KEEP_LAST) or all (KEEP_ALL), at most `max_burst_samples`,
    /// and sends them to a connection that subscribes, before live ones.
    #[test]
    fn wsbridge_6_transient_local_bursts_the_last_n_on_subscribe() {
        let mut router = Router::new().with_max_burst(4);
        router.configure_topic(
            "Keyed",
            behavior(BackpressureMode::Block, ReplayMode::LastN(2)),
        );
        router.configure_topic("All", behavior(BackpressureMode::Block, ReplayMode::All));
        router.configure_topic("Volatile", WsBehavior::default());
        // Instance 1: 1, 2, 3; instance 2: 4. Two per instance stay.
        for (n, key) in [(1, 1), (2, 1), (4, 2), (3, 1)] {
            router
                .route("Keyed", sample(n, Some(key)))
                .deliver(&NO_STOP);
        }
        for n in 1..=6 {
            router.route("All", sample(n, None)).deliver(&NO_STOP);
            router.route("Volatile", sample(n, None)).deliver(&NO_STOP);
        }
        let q = ConnQueue::new(16);
        router.register_connection(1, Arc::clone(&q));
        for topic in ["Keyed", "All", "Volatile"] {
            router.subscribe(1, topic.to_string());
        }
        // A second subscribe sends no second burst.
        router.subscribe(1, "Keyed".to_string());
        router.route("Keyed", sample(7, Some(2))).deliver(&NO_STOP);
        let msgs = drain(&q);
        assert!(matches!(&msgs[0], RouterMsg::Burst { topic, .. } if topic == "Keyed"));
        assert!(matches!(&msgs[1], RouterMsg::Burst { topic, .. } if topic == "All"));
        assert_eq!(msgs.len(), 3, "{msgs:?}");
        // Keyed: 2 (inst 1), 4 (inst 2), 3 (inst 1) in arrival order, then
        // the live 7. All: the last four of six.
        assert_eq!(payloads(&msgs), [2, 4, 3, 3, 4, 5, 6, 7]);
    }

    /// Spec §6: on a topic with key, a sample without a KeyHash names no
    /// instance: it reaches the subscribers, but it is not kept in the
    /// bucket of another instance and starts no deadline; the route says
    /// so, and the pump counts it.
    #[test]
    fn wsbridge_6_a_keyed_sample_without_a_key_hash_is_not_merged() {
        let mut router = Router::new();
        router.configure_topic(
            "K",
            WsBehavior {
                replay: ReplayMode::LastN(1),
                deadline: DeadlineMode::Emit { period_ms: 100 },
                ..WsBehavior::default()
            },
        );
        router.set_keyed("K", true);
        let t0 = Instant::now();
        let q = ConnQueue::new(8);
        router.register_connection(1, Arc::clone(&q));
        router.subscribe(1, "K".to_string());
        let first = router.route_at("K", sample(1, Some(1)), t0);
        assert!(!first.instance_unknown());
        first.deliver(&NO_STOP);
        let route = router.route_at("K", sample(2, None), t0);
        assert!(route.instance_unknown());
        assert_eq!(route.deliver(&NO_STOP).queued, 1);
        assert_eq!(payloads(&drain(&q)), [1, 2]);
        // Only instance 1 is due; the sample without key started nothing.
        assert_eq!(router.deadlines_missed(t0 + Duration::from_millis(100)), 1);
        // A late subscriber gets instance 1's sample, not the one without
        // key in its place.
        let late = ConnQueue::new(8);
        router.register_connection(2, Arc::clone(&late));
        router.subscribe(2, "K".to_string());
        assert_eq!(payloads(&drain(&late)), [1]);
        // On a topic without key, no KeyHash is the one instance.
        router.configure_topic("U", behavior(BackpressureMode::Block, ReplayMode::LastN(1)));
        assert!(!router.route("U", sample(3, None)).instance_unknown());
    }

    /// Spec §6: an instance without a sample for the deadline period gets
    /// one `deadline_missed` per missed period; a sample restarts it, a
    /// dispose ends it.
    #[test]
    fn wsbridge_6_a_missed_deadline_is_sent_once_per_period() {
        let mut router = Router::new();
        router.configure_topic(
            "T",
            WsBehavior {
                deadline: DeadlineMode::Emit { period_ms: 100 },
                ..WsBehavior::default()
            },
        );
        let q = ConnQueue::new(8);
        router.register_connection(1, Arc::clone(&q));
        router.subscribe(1, "T".to_string());
        let t0 = Instant::now();
        // No instance yet: nothing is due.
        assert_eq!(router.deadlines_missed(t0 + Duration::from_secs(1)), 0);
        router
            .route_at("T", sample(1, Some(5)), t0)
            .deliver(&NO_STOP);
        drain(&q);
        assert_eq!(router.deadlines_missed(t0 + Duration::from_millis(50)), 0);
        assert_eq!(router.deadlines_missed(t0 + Duration::from_millis(100)), 1);
        assert_eq!(router.deadlines_missed(t0 + Duration::from_millis(150)), 0);
        assert_eq!(router.deadlines_missed(t0 + Duration::from_millis(200)), 1);
        let texts: Vec<_> = drain(&q)
            .into_iter()
            .map(|m| match m {
                RouterMsg::TopicText { topic, text } => {
                    assert_eq!(topic, "T");
                    text
                }
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(texts.len(), 2);
        assert!(texts[0].contains("\"op\":\"deadline_missed\""));
        assert!(texts[0].contains("\"topic\":\"T\""));
        assert!(texts[0].contains(&"05".repeat(16)));
        // A sample restarts the period.
        let t1 = t0 + Duration::from_millis(250);
        router
            .route_at("T", sample(2, Some(5)), t1)
            .deliver(&NO_STOP);
        assert_eq!(router.deadlines_missed(t1 + Duration::from_millis(90)), 0);
        // A dispose ends the instance's deadline.
        let mut dispose = sample(3, Some(5));
        dispose.kind = SampleKind::Disposed;
        router.route_at("T", dispose, t1).deliver(&NO_STOP);
        assert_eq!(router.deadlines_missed(t1 + Duration::from_secs(5)), 0);
    }

    /// Spec §6: a connection holds at most one burst of a topic. A client
    /// that unsubscribes and subscribes again while the burst waits gets
    /// the newer burst in its place, so the bursts of a client that does
    /// not read do not pile up.
    #[test]
    fn wsbridge_6_a_newer_burst_of_a_topic_replaces_the_waiting_one() {
        let mut router = Router::new().with_max_burst(4);
        router.configure_topic("T", behavior(BackpressureMode::Block, ReplayMode::All));
        router.configure_topic("U", behavior(BackpressureMode::Block, ReplayMode::All));
        router.route("T", sample(1, None)).deliver(&NO_STOP);
        router.route("U", sample(9, None)).deliver(&NO_STOP);
        let q = ConnQueue::new(4);
        router.register_connection(1, Arc::clone(&q));
        router.subscribe(1, "T".to_string());
        router.subscribe(1, "U".to_string());
        for n in 2..=100 {
            router.unsubscribe(1, "T");
            router.route("T", sample(n, None)).deliver(&NO_STOP);
            router.subscribe(1, "T".to_string());
        }
        let msgs = drain(&q);
        // The burst of U, then the newest burst of T: the last four of T.
        assert_eq!(msgs.len(), 2, "{msgs:?}");
        assert!(matches!(&msgs[0], RouterMsg::Burst { topic, .. } if topic == "U"));
        assert!(matches!(&msgs[1], RouterMsg::Burst { topic, .. } if topic == "T"));
        assert_eq!(payloads(&msgs), [9, 97, 98, 99, 100]);
    }

    /// Spec §6: at most `MAX_QUEUED_CONTROLS` control messages wait in a
    /// connection's queue. One more overflows it: it is not queued, the
    /// queue takes nothing more, and the writer reads the overflow. Taking
    /// a control message makes room before the bound is reached.
    #[test]
    fn wsbridge_6_control_messages_over_the_bound_overflow_the_queue() {
        let router = {
            let mut r = Router::new();
            r.register_connection(1, ConnQueue::new(1));
            r
        };
        let q = Arc::clone(router.conns.get(&1).unwrap());
        for n in 0..MAX_QUEUED_CONTROLS {
            assert!(router.send_text(1, n.to_string()), "message {n}");
        }
        // Taking one makes room for one.
        assert!(matches!(q.pop(Duration::from_millis(1)), Some(RouterMsg::Text(t)) if t == "0"));
        assert!(router.send_text(1, "again".to_string()));
        assert!(!q.overflowed());
        // One over the bound.
        assert!(!router.send_text(1, "over".to_string()));
        assert!(q.overflowed());
        // A queue that overflowed takes nothing more, also no burst.
        assert!(!router.send_text(1, "later".to_string()));
        assert!(!q.push_control(RouterMsg::Burst {
            topic: "T".to_string(),
            samples: vec![sample(1, None)],
        }));
        let msgs = drain(&q);
        assert_eq!(msgs.len(), MAX_QUEUED_CONTROLS);
        assert!(matches!(msgs.last(), Some(RouterMsg::Text(t)) if t == "again"));
    }
}
