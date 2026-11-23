// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The DDS side of the bridge daemon (spec §4.2, §4.3, §6): the participant,
//! the DataWriter of each topic carried into DDS, a DataReader per
//! `Subscribe` stream, the retained samples of TRANSIENT_LOCAL topics and the
//! join of an `initial_burst` with a stream's live reader.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use zerodds_dcps::DdsError;
use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_qos::{LivelinessKind, LivelinessQosPolicy};
use zerodds_rtps::header_extension::HeTimestamp;
use zerodds_rtps::history_cache::ChangeKind;
use zerodds_rtps::wire_types::{EntityId, GuidPrefix};

use super::TopicMapping;

/// Type name advertised by the bridge's opaque-bytes writer/reader. A
/// peer DataReader must register the same `(topic, type)` pair to match.
pub const BRIDGE_TYPE_NAME: &str = "ZeroDdsBridgeBytes";

/// The DataReader of one `Subscribe` stream (spec §4.3: the daemon
/// registers a DataReader for each), with its delivery channel. Every
/// stream has its own, so two clients subscribed to one topic each get
/// every sample.
pub struct StreamReader {
    eid: EntityId,
    rx: Mutex<std::sync::mpsc::Receiver<UserSample>>,
}

/// What a stream's reader delivers (spec §4.2 `Sample`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Delivered {
    /// The CDR bytes without the encapsulation header; empty for a
    /// dispose or unregister.
    pub payload: Vec<u8>,
    /// The representation identifier of the encapsulation header.
    pub identifier: [u8; 2],
    /// `Sample.flags` (`FLAG_DISPOSE`, `FLAG_UNREGISTER`).
    pub flags: u32,
    /// The KeyHash of the instance, when the sample names one.
    pub key_hash: Option<[u8; 16]>,
    /// The source timestamp in nanoseconds since the Unix epoch.
    pub source_ts_ns: Option<u64>,
    /// The GUID of the DataWriter and the RTPS sequence number of the
    /// change (DDSI-RTPS 2.5 §8.3.5.4): the identity of the sample
    /// over all readers; `None` where the delivery path knows no
    /// sequence number.
    pub origin: Option<([u8; 16], i64)>,
}

/// The identity of a sample delivered with `writer_guid` and
/// `source_sequence_number`.
fn origin_of(writer_guid: [u8; 16], sn: i64) -> Option<([u8; 16], i64)> {
    (sn >= 0).then_some((writer_guid, sn))
}

/// One sample a `Publish` writes (spec §4.2 `Sample`, checked).
#[derive(Debug, Clone, Copy)]
pub struct Outgoing<'a> {
    /// The encapsulation identifier and the CDR bytes after the header.
    pub identifier: [u8; 2],
    pub body: &'a [u8],
    /// `Sample.flags`.
    pub flags: u32,
    pub key_hash: Option<[u8; 16]>,
    /// `Sample.source_ts_ns` as the RTPS timestamp of the write.
    pub source_ts: Option<HeTimestamp>,
    /// Spec §6 Lifespan: the lifespan of the sample in milliseconds
    /// (`zerodds-lifespan-ms`, else `grpc-timeout`); `None` is the
    /// topic's `qos.lifespan_ms`.
    pub lifespan_ms: Option<u64>,
}

/// The DataWriters of other lifespans a topic may have beside its own
/// (spec §6 Lifespan: one per distinct `zerodds-lifespan-ms`).
pub const MAX_LIFESPAN_WRITERS: usize = 8;

/// The samples the daemon retains of all TRANSIENT_LOCAL topics of a
/// participant together keep below this; beyond it the oldest go.
pub const RETAINED_LIMIT: usize = 10_000;

/// How long a new DataWriter of another lifespan waits to match the
/// DataReaders the topic's own DataWriter matched, before its first
/// write; and how long a DataWriter of another lifespan that makes
/// room for a new one waits for its readers' acknowledgements.
const MATCH_WAIT: Duration = Duration::from_secs(1);

/// Spec §6 TRANSIENT_LOCAL: the DataReader of the topic's durability
/// that follows a retained topic from the start, and the depth per
/// instance.
struct Retained {
    reader: StreamReader,
    depth: usize,
}

/// One retained sample and its place in the order of arrival over all
/// retained topics of the participant (from 1).
#[derive(Debug, Clone)]
struct Kept {
    at: u64,
    sample: Delivered,
}

/// Spec §6 Durability, History: the samples the daemon retains of the
/// TRANSIENT_LOCAL topics of one participant — per topic in the order
/// they arrived, per instance the last `depth`, at most `limit` of all
/// topics together; beyond it the oldest of all topics goes first.
#[derive(Debug)]
pub struct RetainedStore {
    topics: HashMap<String, VecDeque<Kept>>,
    /// The arrival number of the last sample taken.
    last: u64,
    total: usize,
    limit: usize,
}

impl RetainedStore {
    /// An empty store of at most `limit` samples.
    #[must_use]
    pub fn new(limit: usize) -> Self {
        Self {
            topics: HashMap::new(),
            last: 0,
            total: 0,
            limit: limit.max(1),
        }
    }

    /// Takes a sample of `topic` a retained DataReader delivered: a
    /// dispose or unregister removes the instance; an instance keeps
    /// its last `depth`; above `limit` the oldest of all topics goes.
    pub fn take(&mut self, topic: &str, depth: usize, d: Delivered) {
        let q = self.topics.entry(topic.to_string()).or_default();
        if d.flags != 0 {
            let before = q.len();
            q.retain(|k| k.sample.key_hash != d.key_hash);
            self.total -= before - q.len();
            return;
        }
        let same = q.iter().filter(|k| k.sample.key_hash == d.key_hash).count();
        if same >= depth.max(1)
            && let Some(at) = q.iter().position(|k| k.sample.key_hash == d.key_hash)
        {
            q.remove(at);
            self.total -= 1;
        }
        self.last += 1;
        q.push_back(Kept {
            at: self.last,
            sample: d,
        });
        self.total += 1;
        while self.total > self.limit {
            let oldest = self
                .topics
                .iter()
                .filter_map(|(name, q)| q.front().map(|k| (k.at, name.clone())))
                .min();
            let Some((_, name)) = oldest else {
                break;
            };
            if let Some(q) = self.topics.get_mut(&name) {
                q.pop_front();
                self.total -= 1;
            }
        }
    }

    /// Spec §6 `initial_burst`: the arrival number just before the
    /// last `count` retained samples of `topic` — where a stream that
    /// sends them starts reading ([`Self::after`]).
    #[must_use]
    pub fn burst_start(&self, topic: &str, count: usize) -> u64 {
        let Some(q) = self.topics.get(topic) else {
            return self.last;
        };
        match q.len().checked_sub(count) {
            Some(skip) if skip < q.len() => q[skip].at - 1,
            Some(_) => self.last,
            None => q.front().map_or(self.last, |k| k.at - 1),
        }
    }

    /// The first retained sample of `topic` that arrived after
    /// `cursor`, with its arrival number.
    #[must_use]
    pub fn after(&self, topic: &str, cursor: u64) -> Option<(u64, Delivered)> {
        let q = self.topics.get(topic)?;
        let at = q.partition_point(|k| k.at <= cursor);
        q.get(at).map(|k| (k.at, k.sample.clone()))
    }

    /// The retained samples of `topic`, oldest first.
    #[cfg(test)]
    #[must_use]
    pub fn samples(&self, topic: &str) -> Vec<Delivered> {
        self.topics
            .get(topic)
            .map(|q| q.iter().map(|k| k.sample.clone()).collect())
            .unwrap_or_default()
    }

    /// The number of retained samples of all topics.
    #[cfg(test)]
    #[must_use]
    pub fn held(&self) -> usize {
        self.total
    }
}

/// A DataWriter of another lifespan than the topic's (spec §6
/// Lifespan) and when it last wrote.
#[derive(Clone, Copy)]
struct LifespanWriter {
    eid: EntityId,
    used: Instant,
}

/// A place in the pool of DataWriters of other lifespans. A call that
/// makes a writer, or that waits for the acknowledgements of the one it
/// replaces, does so outside the pool's lock; a call for the same lifespan
/// waits for the end ([`BridgeDds::lifespan_writer`]).
enum LifespanSlot {
    Ready(LifespanWriter),
    /// Being made.
    Making,
    /// Making room for another: waiting for its readers' acknowledgements.
    Evicting(LifespanWriter),
}

impl LifespanSlot {
    /// The DataWriter that exists for the slot.
    fn eid(&self) -> Option<EntityId> {
        match self {
            Self::Ready(w) | Self::Evicting(w) => Some(w.eid),
            Self::Making => None,
        }
    }
}

/// Holds the DDS participant, the per-topic writers and the mappings the
/// streams' readers are made from.
pub struct BridgeDds {
    rt: Arc<DcpsRuntime>,
    /// `dds_name → ingress DataWriter` (gRPC `Publish` → DDS).
    writers: HashMap<String, EntityId>,
    /// `(dds_name, lifespan in ms) → DataWriter` of a lifespan other
    /// than the topic's (spec §6 Lifespan).
    lifespan_writers: Mutex<HashMap<(String, u64), LifespanSlot>>,
    /// Signalled when a slot of `lifespan_writers` stops being made or
    /// evicted.
    lifespan_settled: Condvar,
    /// `dds_name → the mapping` of each topic carried into DDS.
    inbound: HashMap<String, TopicMapping>,
    /// `dds_name → the retained DataReader` of each TRANSIENT_LOCAL
    /// topic carried out of DDS (spec §6).
    retained: HashMap<String, Retained>,
    /// The samples those DataReaders delivered (spec §6).
    store: Mutex<RetainedStore>,
    /// The representation overrides of a writer and its write are one
    /// step, and the sequence number of the write is read in it: the
    /// connections write from threads of their own.
    write_lock: Mutex<()>,
    /// `dds_name → the mapping` of each topic carried out of DDS
    /// (gRPC `Subscribe`).
    outbound: HashMap<String, TopicMapping>,
}

/// Nanoseconds since the Unix epoch of an RTPS timestamp (DDSI-RTPS
/// 2.5 §9.3.2.2: seconds and 2^-32 fractions).
#[must_use]
pub fn nanos_of(ts: HeTimestamp) -> u64 {
    u64::from(ts.seconds) * 1_000_000_000 + ((u64::from(ts.fraction) * 1_000_000_000) >> 32)
}

/// The largest `Sample.source_ts_ns` an RTPS timestamp holds: the
/// last nanosecond of second 2^32 - 1 (DDSI-RTPS 2.5 §9.3.2.1, the
/// unsigned 32-bit seconds).
pub const MAX_SOURCE_TS_NS: u64 = (u32::MAX as u64 + 1) * 1_000_000_000 - 1;

/// The RTPS timestamp of nanoseconds since the Unix epoch; `None`
/// beyond the 32-bit seconds of the wire format
/// ([`MAX_SOURCE_TS_NS`]).
#[must_use]
pub fn timestamp_of(ns: u64) -> Option<HeTimestamp> {
    let seconds = u32::try_from(ns / 1_000_000_000).ok()?;
    let rest = ns % 1_000_000_000;
    let fraction = u32::try_from((rest << 32) / 1_000_000_000).ok()?;
    Some(HeTimestamp { seconds, fraction })
}

impl BridgeDds {
    /// Starts the DDS participant for `domain` and registers, per DDS
    /// topic, a writer when a mapping carries it into DDS (`direction`
    /// `in`/`bidir`). A topic carried out of DDS (`out`/`bidir`) gets a
    /// reader per `Subscribe` stream ([`Self::open_reader`]); with
    /// `bidir` that reader observes what `Publish` writes.
    pub fn start(
        domain: i32,
        topics: &[TopicMapping],
        runtime: RuntimeConfig,
    ) -> Result<Self, String> {
        let rt = DcpsRuntime::start(domain, guid_prefix(), runtime)
            .map_err(|e| format!("DcpsRuntime::start: {e:?}"))?;
        Self::on(rt, topics)
    }

    /// The DDS side of `topics` on the running participant `rt`.
    pub fn on(rt: Arc<DcpsRuntime>, topics: &[TopicMapping]) -> Result<Self, String> {
        let mut writers = HashMap::new();
        let mut inbound = HashMap::new();
        let mut outbound = HashMap::new();
        let mut retained = HashMap::new();
        for t in topics {
            if t.dds_name.is_empty() {
                continue;
            }
            if super::bridged_into_dds(t) && !writers.contains_key(&t.dds_name) {
                let weid = register_writer(&rt, t, None)?;
                writers.insert(t.dds_name.clone(), weid);
                inbound.insert(t.dds_name.clone(), t.clone());
            }
            if super::bridged_out_of_dds(t) && !outbound.contains_key(&t.dds_name) {
                outbound.insert(t.dds_name.clone(), t.clone());
                if t.durability != zerodds_qos::DurabilityKind::Volatile {
                    let (eid, rx) = rt
                        .register_user_reader_kind(reader_cfg(t), t.keyed)
                        .map_err(|e| format!("register_user_reader({}): {e:?}", t.dds_name))?;
                    retained.insert(
                        t.dds_name.clone(),
                        Retained {
                            reader: StreamReader {
                                eid,
                                rx: Mutex::new(rx),
                            },
                            // `keep_all` (`usize::MAX`) and no depth:
                            // all, bounded by the shared RETAINED_LIMIT.
                            depth: t.history_depth.unwrap_or(RETAINED_LIMIT).max(1),
                        },
                    );
                }
            }
        }
        Ok(Self {
            rt,
            writers,
            lifespan_writers: Mutex::new(HashMap::new()),
            lifespan_settled: Condvar::new(),
            inbound,
            retained,
            store: Mutex::new(RetainedStore::new(RETAINED_LIMIT)),
            write_lock: Mutex::new(()),
            outbound,
        })
    }

    /// The DataWriter of `dds_name`.
    #[cfg(test)]
    pub fn writer_of(&self, dds_name: &str) -> Option<EntityId> {
        self.writers.get(dds_name).copied()
    }

    /// Registers the DataReader of a new `Subscribe` stream of
    /// `dds_name` (spec §4.3) in `partitions` (spec §6:
    /// `SubscribeReq.partition_filter`; empty is the default
    /// partition).
    ///
    /// # Errors
    /// The topic is not carried out of DDS, or the runtime refused the
    /// reader.
    pub fn open_reader(
        &self,
        dds_name: &str,
        partitions: Vec<String>,
        live_only: bool,
    ) -> Result<StreamReader, String> {
        let t = self
            .outbound
            .get(dds_name)
            .ok_or_else(|| format!("{dds_name} is not bridged out of DDS"))?;
        let mut cfg = reader_cfg(t);
        cfg.partition = partitions;
        // Spec §6: a stream with an `initial_burst` takes the retained
        // samples from the daemon and the live ones from a VOLATILE
        // reader, so that none comes twice.
        if live_only {
            cfg.durability = zerodds_qos::DurabilityKind::Volatile;
        }
        let (eid, rx) = self
            .rt
            .register_user_reader_kind(cfg, t.keyed)
            .map_err(|e| format!("register_user_reader({dds_name}): {e:?}"))?;
        Ok(StreamReader {
            eid,
            rx: Mutex::new(rx),
        })
    }

    /// Whether the daemon retains samples of `dds_name` (spec §6: a
    /// TRANSIENT_LOCAL topic carried out of DDS).
    pub fn retains(&self, dds_name: &str) -> bool {
        self.retained.contains_key(dds_name)
    }

    /// Takes what the retained DataReaders delivered into the retained
    /// samples ([`RetainedStore::take`]): per instance (`key_hash`) the
    /// last `depth`; a dispose or unregister removes the instance;
    /// above [`RETAINED_LIMIT`] of all topics the oldest go.
    pub fn refresh_retained(&self) {
        let mut store = self.store.lock().unwrap_or_else(|e| e.into_inner());
        for (name, r) in &self.retained {
            while let Some(d) = Self::next_sample(&r.reader, true) {
                store.take(name, r.depth, d);
            }
        }
    }

    /// Spec §6 `initial_burst`: where a stream that starts with the
    /// last `count` retained samples of `dds_name` starts reading them
    /// ([`Self::retained_after`]); `None` when the daemon retains none
    /// of the topic.
    pub fn burst_start(&self, dds_name: &str, count: usize) -> Option<u64> {
        if !self.retained.contains_key(dds_name) {
            return None;
        }
        self.refresh_retained();
        let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
        Some(store.burst_start(dds_name, count))
    }

    /// The first retained sample of `dds_name` that arrived after
    /// `cursor`, with its arrival number; with `refresh` the retained
    /// DataReaders are read first.
    pub fn retained_after(
        &self,
        dds_name: &str,
        cursor: u64,
        refresh: bool,
    ) -> Option<(u64, Delivered)> {
        if refresh {
            self.refresh_retained();
        }
        let store = self.store.lock().unwrap_or_else(|e| e.into_inner());
        store.after(dds_name, cursor)
    }

    /// Spec §6 Deadline: the requested deadlines a stream's DataReader
    /// missed (DDS 1.4 §2.2.4.1 `REQUESTED_DEADLINE_MISSED.total_count`).
    pub fn deadlines_missed(&self, reader: &StreamReader) -> i32 {
        self.rt
            .user_reader_requested_deadline_missed_status(reader.eid)
            .map_or(0, |s| s.total_count)
    }

    /// The DataWriter of `dds_name` with the lifespan `ms` (spec §6:
    /// `zerodds-lifespan-ms`), made at its first use; it waits up to
    /// [`MATCH_WAIT`] to match the DataReaders the topic's own
    /// DataWriter matched, so that its first sample reaches them. When
    /// the topic has [`MAX_LIFESPAN_WRITERS`] of them, the one that
    /// wrote least recently makes room, once its reliable readers
    /// acknowledged its samples (waiting up to [`MATCH_WAIT`]) and,
    /// for a durability other than VOLATILE, its last sample expired.
    ///
    /// The waits run outside the pool's lock: a call for a writer that
    /// exists does not wait for them; a call for the lifespan being made
    /// or evicted waits for that to end.
    ///
    /// # Errors
    /// `OutOfResources` when none of the topic's DataWriters of other
    /// lifespans can make room; the runtime's error.
    fn lifespan_writer(&self, dds_name: &str, ms: u64) -> Result<EntityId, DdsError> {
        self.lifespan_writer_with(dds_name, ms, |t| self.make_lifespan_writer(dds_name, t, ms))
    }

    /// [`Self::lifespan_writer`] with `make`, which registers the new
    /// DataWriter and waits for its matches.
    fn lifespan_writer_with(
        &self,
        dds_name: &str,
        ms: u64,
        make: impl FnOnce(&TopicMapping) -> Result<EntityId, DdsError>,
    ) -> Result<EntityId, DdsError> {
        let t = self
            .inbound
            .get(dds_name)
            .ok_or(DdsError::PreconditionNotMet {
                reason: "the topic has no DataWriter",
            })?;
        let key = (dds_name.to_string(), ms);
        let mut pool = self.lifespan_pool();
        // A call that makes or evicts the slot ends within its two waits.
        let give_up = Instant::now() + 3 * MATCH_WAIT;
        loop {
            match pool.get_mut(&key) {
                Some(LifespanSlot::Ready(w)) => {
                    w.used = Instant::now();
                    return Ok(w.eid);
                }
                Some(LifespanSlot::Making | LifespanSlot::Evicting(_)) => {
                    let left = give_up.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Err(DdsError::OutOfResources {
                            what: "a DataWriter of another lifespan another call makes or replaces (3 s)",
                        });
                    }
                    pool = self
                        .lifespan_settled
                        .wait_timeout(pool, left)
                        .unwrap_or_else(|e| e.into_inner())
                        .0;
                }
                None => break,
            }
        }
        let now = Instant::now();
        let mut evicted = None;
        if pool.keys().filter(|(n, _)| n == dds_name).count() >= MAX_LIFESPAN_WRITERS {
            let volatile = t.durability == zerodds_qos::DurabilityKind::Volatile;
            let least = pool
                .iter()
                .filter_map(|((n, life), slot)| match slot {
                    LifespanSlot::Ready(w)
                        if n == dds_name
                            && (volatile || w.used + Duration::from_millis(*life) <= now) =>
                    {
                        Some((*life, *w))
                    }
                    _ => None,
                })
                .min_by_key(|(_, w)| w.used);
            let Some((life, w)) = least else {
                return Err(DdsError::OutOfResources {
                    what: "the DataWriters of other lifespans of the topic (8), none of them expired",
                });
            };
            pool.insert((dds_name.to_string(), life), LifespanSlot::Evicting(w));
            evicted = Some((life, w));
        }
        pool.insert(key.clone(), LifespanSlot::Making);
        drop(pool);
        if let Some((life, w)) = evicted {
            let acknowledged = self
                .rt
                .await_user_writer_acknowledged(w.eid, Instant::now() + MATCH_WAIT);
            let mut pool = self.lifespan_pool();
            if !acknowledged {
                pool.insert((dds_name.to_string(), life), LifespanSlot::Ready(w));
                pool.remove(&key);
                drop(pool);
                self.lifespan_settled.notify_all();
                return Err(DdsError::OutOfResources {
                    what: "the DataWriters of other lifespans of the topic (8), the least recently used not acknowledged",
                });
            }
            self.rt.remove_user_writer(w.eid);
            pool.remove(&(dds_name.to_string(), life));
            drop(pool);
            self.lifespan_settled.notify_all();
        }
        let made = make(t);
        let mut pool = self.lifespan_pool();
        match made {
            Ok(eid) => {
                pool.insert(
                    key,
                    LifespanSlot::Ready(LifespanWriter {
                        eid,
                        used: Instant::now(),
                    }),
                );
            }
            Err(_) => {
                pool.remove(&key);
            }
        }
        drop(pool);
        self.lifespan_settled.notify_all();
        made
    }

    /// Registers the DataWriter of `dds_name` with the lifespan `ms` and
    /// waits up to [`MATCH_WAIT`] until it matched as many DataReaders as
    /// the topic's own DataWriter.
    fn make_lifespan_writer(
        &self,
        dds_name: &str,
        t: &TopicMapping,
        ms: u64,
    ) -> Result<EntityId, DdsError> {
        let eid = register_writer(&self.rt, t, Some(ms)).map_err(|_| DdsError::OutOfResources {
            what: "a DataWriter of another lifespan",
        })?;
        let own = self
            .writers
            .get(dds_name)
            .map_or(0, |w| self.rt.user_writer_matched_count(*w));
        let until = Instant::now() + MATCH_WAIT;
        while self.rt.user_writer_matched_count(eid) < own && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(eid)
    }

    /// Whether the DataWriter of `dds_name` with the lifespan `ms` is
    /// being evicted: a call waits for its readers' acknowledgements.
    #[cfg(test)]
    fn lifespan_evicting(&self, dds_name: &str, ms: u64) -> bool {
        matches!(
            self.lifespan_pool().get(&(dds_name.to_string(), ms)),
            Some(LifespanSlot::Evicting(_))
        )
    }

    /// The pool of DataWriters of other lifespans, locked.
    fn lifespan_pool(&self) -> MutexGuard<'_, HashMap<(String, u64), LifespanSlot>> {
        self.lifespan_writers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    /// Spec §9.2 "Drain pending DDS writes": waits until the reliable
    /// readers of every DataWriter of the participant — the topics' own and
    /// those of other lifespans — acknowledged what it wrote, or `until`
    /// passes (DDS 1.4 §2.2.2.4.2.15 `wait_for_acknowledgments`). `true`
    /// when all did; a best-effort writer has nothing to wait for.
    pub fn await_writes(&self, until: Instant) -> bool {
        let lifespan: Vec<EntityId> = self
            .lifespan_pool()
            .values()
            .filter_map(LifespanSlot::eid)
            .collect();
        let mut all = true;
        for eid in self.writers.values().copied().chain(lifespan) {
            all &= self.rt.await_user_writer_acknowledged(eid, until);
        }
        all
    }

    /// Spec §9.2 "Clean up DDS entities": deletes the DataWriters and the
    /// retained DataReaders — the runtime disposes their SEDP
    /// announcements (DDSI-RTPS 2.5 §8.5.4) — and stops the participant,
    /// which announces its departure (§8.5.3.2).
    pub fn close(self) {
        let lifespan: Vec<EntityId> = self
            .lifespan_pool()
            .values()
            .filter_map(LifespanSlot::eid)
            .collect();
        for eid in self.writers.values().copied().chain(lifespan) {
            self.rt.remove_user_writer(eid);
        }
        for r in self.retained.values() {
            self.rt.remove_user_reader(r.reader.eid);
        }
        self.rt.shutdown();
    }

    /// Deletes the DataReader of a stream that ended; the runtime
    /// disposes its SEDP subscription (spec §4.3: `grpc-status` 0 on
    /// reader cleanup).
    pub fn close_reader(&self, reader: StreamReader) {
        self.rt.remove_user_reader(reader.eid);
    }

    /// Spec §4.5: the liveliness of the DDS side of a stream is lost —
    /// matched DataWriters are known and none of them is alive
    /// (`LIVELINESS_CHANGED`, DDS 1.4 §2.2.4.1: `alive_count` 0,
    /// `not_alive_count` above 0).
    pub fn liveliness_lost(&self, reader: &StreamReader) -> bool {
        let (any_alive, _, not_alive) = self.rt.user_reader_liveliness_status(reader.eid);
        !any_alive && not_alive > 0
    }

    /// Writes one sample to the DataWriter of `dds_name`: the CDR bytes
    /// under the encapsulation header they came with (spec §4.4;
    /// DDS-XTypes 1.3 §7.6.3.1.2, Table 60: its XCDR version, its
    /// format and its byte order), keyed by `key_hash`, stamped with the
    /// source timestamp; a dispose or unregister as the lifecycle
    /// marker of the instance (DDSI-RTPS 2.5 §9.6.4.9). Returns the RTPS
    /// sequence number the write took (DDSI-RTPS 2.5 §8.3.5.4).
    ///
    /// # Errors
    /// `None` when the topic has no writer; the DDS error of the write.
    pub fn publish(&self, dds_name: &str, s: Outgoing<'_>) -> Option<Result<i64, DdsError>> {
        let own = *self.writers.get(dds_name)?;
        let configured = self.inbound.get(dds_name).and_then(|t| t.lifespan_ms);
        let eid = match s.lifespan_ms {
            Some(ms) if Some(ms) != configured => match self.lifespan_writer(dds_name, ms) {
                Ok(eid) => eid,
                Err(e) => return Some(Err(e)),
            },
            _ => own,
        };
        let _guard = self.write_lock.lock().unwrap_or_else(|e| e.into_inner());
        let ts = s.source_ts;
        let written = if s.flags != 0 {
            use zerodds_rtps::inline_qos::status_info;
            let mut bits = 0;
            if s.flags & crate::messages::FLAG_DISPOSE != 0 {
                bits |= status_info::DISPOSED;
            }
            if s.flags & crate::messages::FLAG_UNREGISTER != 0 {
                bits |= status_info::UNREGISTERED;
            }
            let key = s.key_hash.unwrap_or([0; 16]);
            self.rt.write_user_lifecycle(eid, key, None, bits, ts)
        } else {
            let _ = self
                .rt
                .set_user_writer_data_rep_override(eid, Some(vec![super::offer_of(s.identifier)]));
            let _ = self
                .rt
                .set_user_writer_byte_order_override(eid, s.identifier[1] & 0x01 == 0);
            let _ = self.rt.set_user_writer_wire_extensibility(
                eid,
                super::wire_extensibility_of(s.identifier),
            );
            match (s.key_hash, ts) {
                (Some(key), ts) => self.rt.write_user_sample_keyed(eid, s.body, key, ts),
                (None, Some(ts)) => self.rt.write_user_sample_stamped(eid, s.body, ts),
                (None, None) => self.rt.write_user_sample_borrowed(eid, s.body),
            }
        };
        Some(written.map(|()| {
            self.rt
                .user_writer_next_sequence_number(eid)
                .map_or(0, |next| next - 1)
        }))
    }

    /// The next sample of a stream's reader: an Alive sample with the
    /// representation identifier of the header it came under, its key
    /// and its source timestamp; with `include_dispose` also a dispose
    /// or unregister (spec §4.2 `SubscribeReq.include_dispose`). The
    /// ECS marker of a coherent set carries no user data (DDSI-RTPS
    /// 2.5 §8.7.6) and is not a sample. `None` when the reader holds
    /// nothing.
    pub fn next_sample(reader: &StreamReader, include_dispose: bool) -> Option<Delivered> {
        use crate::messages::{FLAG_DISPOSE, FLAG_UNREGISTER};
        let rx = reader.rx.lock().ok()?;
        loop {
            match rx.try_recv() {
                Ok(UserSample::Alive {
                    payload,
                    representation_identifier,
                    key_hash,
                    source_timestamp,
                    writer_guid,
                    source_sequence_number,
                    ..
                }) => {
                    return Some(Delivered {
                        payload: payload.to_vec(),
                        identifier: representation_identifier,
                        flags: 0,
                        key_hash,
                        source_ts_ns: source_timestamp.map(nanos_of),
                        origin: origin_of(writer_guid, source_sequence_number),
                    });
                }
                Ok(UserSample::Lifecycle {
                    key_hash,
                    kind,
                    source_timestamp,
                    writer_guid,
                    source_sequence_number,
                    ..
                }) => {
                    if !include_dispose {
                        continue;
                    }
                    let flags = match kind {
                        ChangeKind::NotAliveDisposed => FLAG_DISPOSE,
                        ChangeKind::NotAliveUnregistered => FLAG_UNREGISTER,
                        ChangeKind::NotAliveDisposedUnregistered => FLAG_DISPOSE | FLAG_UNREGISTER,
                        ChangeKind::Alive | ChangeKind::AliveFiltered => continue,
                    };
                    return Some(Delivered {
                        payload: Vec::new(),
                        identifier: [0, 0],
                        flags,
                        key_hash: Some(key_hash),
                        source_ts_ns: source_timestamp.map(nanos_of),
                        origin: origin_of(writer_guid, source_sequence_number),
                    });
                }
                Ok(UserSample::CoherentSetEnd { .. }) => continue,
                Err(_) => return None,
            }
        }
    }
}

/// A duration of milliseconds as a QoS duration; the reader of the
/// configuration keeps the values within `i32`.
fn millis(ms: u64) -> zerodds_qos::Duration {
    zerodds_qos::Duration::from_millis(i32::try_from(ms).unwrap_or(i32::MAX))
}

/// Registers the DataWriter of `t` with the lifespan `lifespan_ms`, or
/// the topic's `qos.lifespan_ms`, and its history: the KEEP_LAST depth,
/// KEEP_ALL for `keep_all` (`usize::MAX`), else the runtime's default.
fn register_writer(
    rt: &DcpsRuntime,
    t: &TopicMapping,
    lifespan_ms: Option<u64>,
) -> Result<EntityId, String> {
    let weid = rt
        .register_user_writer_kind(writer_cfg(t, lifespan_ms), t.keyed)
        .map_err(|e| format!("register_user_writer({}): {e:?}", t.dds_name))?;
    if let Some(depth) = t.history_depth {
        rt.set_user_writer_history_depth(weid, depth)
            .map_err(|e| format!("set_user_writer_history_depth({}): {e:?}", t.dds_name))?;
    }
    Ok(weid)
}

/// Spec §6: the writer offers the topic's reliability, durability,
/// deadline (`qos.deadline_ms`) and lifespan.
fn writer_cfg(t: &TopicMapping, lifespan_ms: Option<u64>) -> UserWriterConfig {
    let mut cfg = UserWriterConfig {
        topic_name: t.dds_name.clone(),
        type_name: t.dds_type.clone(),
        reliable: t.reliable,
        durability: t.durability,
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        type_identifier: Default::default(),
        ..Default::default()
    };
    if let Some(ms) = lifespan_ms.or(t.lifespan_ms) {
        cfg.lifespan.duration = millis(ms);
    }
    if let Some(ms) = t.deadline_ms {
        cfg.deadline.period = millis(ms);
    }
    cfg
}

/// Spec §6: the reader requests the topic's reliability, durability and
/// deadline (`qos.deadline_ms`).
fn reader_cfg(t: &TopicMapping) -> UserReaderConfig {
    let mut cfg = UserReaderConfig {
        topic_name: t.dds_name.clone(),
        type_name: t.dds_type.clone(),
        reliable: t.reliable,
        durability: t.durability,
        liveliness: LivelinessQosPolicy {
            kind: LivelinessKind::Automatic,
            ..Default::default()
        },
        type_identifier: Default::default(),
        ..Default::default()
    };
    if let Some(ms) = t.deadline_ms {
        cfg.deadline.period = millis(ms);
    }
    cfg
}

fn guid_prefix() -> GuidPrefix {
    GuidPrefix::zerodds(
        zerodds_dcps::participant::host_id_bytes(),
        zerodds_dcps::participant::next_process_unique(),
    )
}

/// Spec §6 `initial_burst`: a stream that starts with the last N retained
/// samples reads the retained samples from where the burst starts — the
/// burst, then what arrives in them while its VOLATILE live reader has
/// not yet matched the topic's DataWriters — and its live reader. A
/// sample goes out once: by the identity of a change (writer GUID and
/// sequence number, DDSI-RTPS 2.5 §8.3.5.4), per writer and instance the
/// highest sequence number sent from the retained samples is kept; a live
/// sample at or below it was sent. A writer whose live samples go beyond
/// what the retained samples gave is followed by the live reader alone.
#[derive(Debug, Default)]
pub(crate) struct BurstJoin {
    /// The arrival number of the last retained sample taken
    /// ([`RetainedStore`]).
    pub(crate) cursor: u64,
    /// Per writer and instance the highest sequence number sent from the
    /// retained samples.
    pub(crate) sent: HashMap<([u8; 16], Option<[u8; 16]>), i64>,
    /// Per writer the highest of them.
    pub(crate) writer_max: HashMap<[u8; 16], i64>,
    /// The writers the live reader follows alone.
    pub(crate) live: std::collections::HashSet<[u8; 16]>,
    /// A live sample taken while retained samples before it were still to
    /// be sent; it goes after them.
    pub(crate) held: Option<Delivered>,
}

impl BurstJoin {
    /// Whether a retained sample goes out, recorded as sent when it does.
    pub(crate) fn take_retained(&mut self, d: &Delivered) -> bool {
        let Some((writer, sn)) = d.origin else {
            return true;
        };
        if self.live.contains(&writer) {
            return false;
        }
        let key = (writer, d.key_hash);
        if self.sent.get(&key).is_some_and(|s| sn <= *s) {
            return false;
        }
        self.sent.insert(key, sn);
        let max = self.writer_max.entry(writer).or_insert(sn);
        *max = (*max).max(sn);
        true
    }

    /// Whether a live sample was sent from the retained samples already;
    /// `None` when that depends on retained samples not yet taken (the
    /// writer is not followed by the live reader alone yet).
    pub(crate) fn live_seen(&self, d: &Delivered) -> Option<bool> {
        let Some((writer, sn)) = d.origin else {
            return Some(false);
        };
        if self.live.contains(&writer) {
            return Some(false);
        }
        if self
            .sent
            .get(&(writer, d.key_hash))
            .is_some_and(|s| sn <= *s)
        {
            return Some(true);
        }
        None
    }

    /// A live sample goes out after every retained sample before it was
    /// taken: a writer whose live samples go beyond the retained ones is
    /// followed by the live reader alone from now on.
    pub(crate) fn live_sent(&mut self, d: &Delivered) {
        let Some((writer, sn)) = d.origin else {
            return;
        };
        if self.writer_max.get(&writer).is_none_or(|m| sn > *m) {
            self.live.insert(writer);
            self.sent.retain(|(w, _), _| *w != writer);
            self.writer_max.remove(&writer);
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::bridge_daemon::test_support::topic;
    use crate::bridge_daemon::*;

    /// RTPS timestamps (DDSI-RTPS 2.5 §9.3.2.2) and nanoseconds.
    #[test]
    fn grpcbridge_4_2_source_ts_ns_and_the_rtps_timestamp() {
        let ns = 1_700_000_000_500_000_000u64;
        let ts = dds_pump::timestamp_of(ns).unwrap();
        assert_eq!(ts.seconds, 1_700_000_000);
        assert_eq!(ts.fraction, 1 << 31);
        assert_eq!(dds_pump::nanos_of(ts), ns);
        assert!(dds_pump::timestamp_of(u64::MAX).is_none());
    }

    /// A DDS participant of a test on a leased domain.
    fn runtime(
        lease: &zerodds_test_domains::DomainLease,
    ) -> Arc<zerodds_dcps::runtime::DcpsRuntime> {
        let mut prefix = [0u8; 12];
        prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
        prefix[4..8].copy_from_slice(&lease.id().to_be_bytes());
        prefix[11] = 0x42;
        zerodds_dcps::runtime::DcpsRuntime::start(
            lease.id(),
            zerodds_rtps::wire_types::GuidPrefix::from_bytes(prefix),
            zerodds_dcps::runtime::RuntimeConfig::default(),
        )
        .unwrap()
    }

    /// §4.5 "send queue full / DDS backpressure": a TRANSIENT_LOCAL writer
    /// whose RESOURCE_LIMITS are reached refuses the write, which is
    /// RESOURCE_EXHAUSTED; §4.2 the writes before it took the sequence
    /// numbers 1 and 2.
    #[test]
    fn grpcbridge_4_5_a_full_writer_history_is_resource_exhausted() {
        let lease = zerodds_test_domains::lease_domain();
        let rt = runtime(&lease);
        let t = TopicMapping {
            direction: "in".into(),
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            ..topic("Limited::Topic")
        };
        let dds = BridgeDds::on(Arc::clone(&rt), std::slice::from_ref(&t)).unwrap();
        let eid = dds.writer_of("Limited::Topic").unwrap();
        rt.set_user_writer_history_depth(eid, usize::MAX).unwrap();
        rt.set_user_writer_resource_limits(
            eid,
            zerodds_qos::ResourceLimitsQosPolicy {
                max_samples: 2,
                max_instances: -1,
                max_samples_per_instance: -1,
            },
            zerodds_qos::Duration::from_millis(50),
        )
        .unwrap();
        let sample = Sample {
            cdr_payload: vec![0, 1, 0, 0, 1, 2, 3, 4],
            ..Sample::default()
        };
        let write = || dds.publish("Limited::Topic", sample_to_publish(&t, &sample).unwrap());
        assert_eq!(write().unwrap().unwrap(), 1);
        assert_eq!(write().unwrap().unwrap(), 2);
        let third = write().unwrap().unwrap_err();
        assert_eq!(
            status_of_write_error(&third),
            Status::ResourceExhausted,
            "{third:?}"
        );
        rt.shutdown();
    }

    /// Spec §3, §6 History: `qos.history` is the history of the topic's
    /// DataWriter into DDS and of its DataWriters of other lifespans:
    /// `keep_all` KEEP_ALL (`usize::MAX`), `keep_last` its depth, without
    /// a depth the runtime's default KEEP_LAST 1.
    #[test]
    fn grpcbridge_6_the_history_of_a_topic_is_the_history_of_its_writers() {
        let lease = zerodds_test_domains::lease_domain();
        let rt = runtime(&lease);
        let with = |name: &str, depth: Option<usize>| TopicMapping {
            direction: "in".into(),
            history_depth: depth,
            ..topic(name)
        };
        let topics = [
            with("All::Topic", Some(usize::MAX)),
            with("Last::Topic", Some(7)),
            with("Default::Topic", None),
        ];
        let dds = BridgeDds::on(Arc::clone(&rt), &topics).unwrap();
        let depth = |eid| rt.user_writer_history_and_limits(eid).unwrap().0;
        assert_eq!(depth(dds.writer_of("All::Topic").unwrap()), usize::MAX);
        assert_eq!(depth(dds.writer_of("Last::Topic").unwrap()), 7);
        assert_eq!(depth(dds.writer_of("Default::Topic").unwrap()), 1);
        assert_eq!(
            depth(dds.lifespan_writer("All::Topic", 1500).unwrap()),
            usize::MAX
        );
        assert_eq!(depth(dds.lifespan_writer("Last::Topic", 1500).unwrap()), 7);
        rt.shutdown();
    }

    /// §4.2: a stream's `Sample` names the key, the source timestamp, the
    /// type and the topic of the DDS sample; with `include_dispose` a
    /// dispose arrives with its flag and key and no payload.
    #[test]
    fn grpcbridge_4_2_a_subscribe_sample_names_key_hash_time_type_and_topic() {
        let lease = zerodds_test_domains::lease_domain();
        let rt = runtime(&lease);
        let t = TopicMapping {
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            ..topic("Keyed::Topic")
        };
        let dds = BridgeDds::on(Arc::clone(&rt), std::slice::from_ref(&t)).unwrap();
        let reader = dds.open_reader("Keyed::Topic", Vec::new(), false).unwrap();
        let ts = 1_700_000_000_250_000_000u64;
        let alive = Sample {
            cdr_payload: vec![0, 1, 0, 0, 5, 6],
            key_hash: vec![7; 16],
            source_ts_ns: ts,
            ..Sample::default()
        };
        dds.publish("Keyed::Topic", sample_to_publish(&t, &alive).unwrap())
            .unwrap()
            .unwrap();
        let dispose = Sample {
            flags: 1,
            key_hash: vec![7; 16],
            source_ts_ns: ts + 1_000_000,
            ..Sample::default()
        };
        dds.publish("Keyed::Topic", sample_to_publish(&t, &dispose).unwrap())
            .unwrap()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut got = Vec::new();
        while got.len() < 2 {
            assert!(Instant::now() < deadline, "{got:?}");
            match BridgeDds::next_sample(&reader, true) {
                Some(d) => got.push(sample_of(&t, &d)),
                None => std::thread::sleep(Duration::from_millis(20)),
            }
        }
        assert_eq!(got[0].cdr_payload, [0, 1, 0, 0, 5, 6]);
        assert_eq!(got[0].key_hash, vec![7; 16]);
        // 2^-32 s steps: within a nanosecond.
        assert!(
            got[0].source_ts_ns.abs_diff(ts) <= 1,
            "{}",
            got[0].source_ts_ns
        );
        assert_eq!(got[0].type_name, "T::Type");
        assert_eq!(got[0].topic_name, "Keyed::Topic");
        assert_eq!(got[0].flags, 0);
        assert_eq!(got[1].flags, crate::messages::FLAG_DISPOSE);
        assert_eq!(got[1].key_hash, vec![7; 16]);
        assert!(got[1].cdr_payload.is_empty());
        dds.close_reader(reader);
        rt.shutdown();
    }

    /// A retained sample of `writer` with sequence number `sn`, instance
    /// `key` and the payload `body`.
    fn kept(writer: u8, sn: i64, key: Option<u8>, body: u8) -> Delivered {
        Delivered {
            payload: vec![body],
            identifier: [0, 1],
            flags: 0,
            key_hash: key.map(|k| [k; 16]),
            source_ts_ns: None,
            origin: Some(([writer; 16], sn)),
        }
    }

    /// Spec §6 Durability/History: the retained samples of the topics of a
    /// participant keep below one limit together; the oldest of all
    /// topics goes first, whichever topic it belongs to.
    #[test]
    fn grpcbridge_6_the_retained_limit_is_shared_by_the_topics_of_a_participant() {
        let mut store = dds_pump::RetainedStore::new(3);
        store.take("A", 10, kept(1, 1, None, 1));
        store.take("B", 10, kept(2, 1, None, 2));
        store.take("A", 10, kept(1, 2, None, 3));
        assert_eq!(store.held(), 3);
        // A fourth sample of B: the oldest of both topics (A's first) goes.
        store.take("B", 10, kept(2, 2, None, 4));
        assert_eq!(store.held(), 3);
        let bodies = |store: &dds_pump::RetainedStore, t: &str| -> Vec<u8> {
            store.samples(t).iter().map(|d| d.payload[0]).collect()
        };
        assert_eq!(bodies(&store, "A"), [3]);
        assert_eq!(bodies(&store, "B"), [2, 4]);
        // Then B's first, the oldest left.
        store.take("A", 10, kept(1, 3, None, 5));
        assert_eq!(bodies(&store, "A"), [3, 5]);
        assert_eq!(bodies(&store, "B"), [4]);
        // The arrival numbers run over both topics: a stream of A that
        // starts with its last sample reads 5 and nothing after it.
        let start = store.burst_start("A", 1);
        let (at, d) = store.after("A", start).expect("the burst");
        assert_eq!(d.payload, [5]);
        assert!(store.after("A", at).is_none());
        assert_eq!(store.burst_start("A", 0), at);
    }

    /// Spec §6 Durability/History: per instance the last `depth`; a
    /// dispose removes the instance.
    #[test]
    fn grpcbridge_6_the_retained_samples_keep_depth_per_instance() {
        let mut store = dds_pump::RetainedStore::new(100);
        for (sn, key, body) in [(1, 1, 1), (2, 2, 2), (3, 1, 3), (4, 2, 4), (5, 1, 5)] {
            store.take("T", 2, kept(9, sn, Some(key), body));
        }
        let bodies = |store: &dds_pump::RetainedStore| -> Vec<u8> {
            store.samples("T").iter().map(|d| d.payload[0]).collect()
        };
        assert_eq!(bodies(&store), [2, 3, 4, 5]);
        let mut dispose = kept(9, 6, Some(2), 0);
        dispose.flags = crate::messages::FLAG_DISPOSE;
        dispose.payload.clear();
        store.take("T", 2, dispose);
        assert_eq!(bodies(&store), [3, 5]);
        assert_eq!(store.held(), 2);
    }

    /// Spec §6 `initial_burst`: a change goes out once — from the retained
    /// samples or from the live reader — and a writer whose live samples go
    /// beyond the retained ones is followed by the live reader alone.
    #[test]
    fn grpcbridge_6_a_burst_join_sends_each_change_once() {
        let mut join = BurstJoin::default();
        // The burst and what arrived in the retained samples after it.
        for sn in 1..=3 {
            assert!(join.take_retained(&kept(1, sn, None, 0)));
        }
        assert!(!join.take_retained(&kept(1, 2, None, 0)), "twice");
        // The live reader matched late: 2 and 3 were sent, 4 is new.
        assert_eq!(join.live_seen(&kept(1, 2, None, 0)), Some(true));
        assert_eq!(join.live_seen(&kept(1, 3, None, 0)), Some(true));
        assert_eq!(join.live_seen(&kept(1, 4, None, 0)), None);
        join.live_sent(&kept(1, 4, None, 0));
        // From now on the live reader alone: retained copies do not go.
        assert!(!join.take_retained(&kept(1, 4, None, 0)));
        assert!(!join.take_retained(&kept(1, 5, None, 0)));
        assert_eq!(join.live_seen(&kept(1, 5, None, 0)), Some(false));
        // Per instance: a change of another instance below the writer's
        // highest is no repeat (a dispose the retained samples do not hold).
        assert!(join.take_retained(&kept(2, 7, Some(1), 0)));
        assert_eq!(join.live_seen(&kept(2, 6, Some(2), 0)), None);
        join.live_sent(&kept(2, 6, Some(2), 0));
        assert!(
            join.take_retained(&kept(2, 8, Some(1), 0)),
            "writer 2 is not live alone yet"
        );
        assert_eq!(join.live_seen(&kept(2, 7, Some(1), 0)), Some(true));
    }

    /// A second participant of a test on the leased domain: the DDS peer
    /// of the bridge's.
    fn peer(lease: &zerodds_test_domains::DomainLease) -> Arc<zerodds_dcps::runtime::DcpsRuntime> {
        let mut prefix = [0u8; 12];
        prefix[..4].copy_from_slice(&std::process::id().to_be_bytes());
        prefix[4..8].copy_from_slice(&lease.id().to_be_bytes());
        prefix[11] = 0x43;
        zerodds_dcps::runtime::DcpsRuntime::start(
            lease.id(),
            zerodds_rtps::wire_types::GuidPrefix::from_bytes(prefix),
            zerodds_dcps::runtime::RuntimeConfig::default(),
        )
        .unwrap()
    }

    /// The reliable opaque-bytes DataReader of `topic` on `rt`.
    fn peer_reader(
        rt: &zerodds_dcps::runtime::DcpsRuntime,
        topic: &str,
    ) -> (
        zerodds_rtps::wire_types::EntityId,
        std::sync::mpsc::Receiver<zerodds_dcps::runtime::UserSample>,
    ) {
        rt.register_user_reader(zerodds_dcps::runtime::UserReaderConfig {
            topic_name: topic.into(),
            type_name: "T::Type".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap()
    }

    /// Waits until `count()` reaches `n`, at most 10 s.
    fn until(what: &str, n: usize, count: impl Fn() -> usize) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while count() != n {
            assert!(Instant::now() < deadline, "{what}: {} of {n}", count());
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The Alive samples `rx` delivers within `wait` each, until none
    /// comes.
    fn alive(
        rx: &std::sync::mpsc::Receiver<zerodds_dcps::runtime::UserSample>,
        wait: Duration,
    ) -> Vec<(Vec<u8>, [u8; 2])> {
        let mut out = Vec::new();
        while let Ok(s) = rx.recv_timeout(wait) {
            if let zerodds_dcps::runtime::UserSample::Alive {
                payload,
                representation_identifier,
                ..
            } = s
            {
                out.push((payload.to_vec(), representation_identifier));
            }
        }
        out
    }

    /// Spec §9.2 "Drain pending DDS writes": `await_writes` returns once the
    /// reliable reader acknowledged every sample — the writer reports all
    /// acknowledged at that moment and the reader holds them — and, with a
    /// matched reader that never acknowledges, waits to the limit and says
    /// so.
    #[test]
    fn grpcbridge_9_2_await_writes_waits_for_the_acknowledgements() {
        let lease = zerodds_test_domains::lease_domain();
        let rt = runtime(&lease);
        let other = peer(&lease);
        let t = TopicMapping {
            direction: "in".into(),
            ..topic("Drain::Topic")
        };
        let dds = BridgeDds::on(Arc::clone(&rt), std::slice::from_ref(&t)).unwrap();
        let eid = dds.writer_of("Drain::Topic").unwrap();
        let (_, rx) = peer_reader(&other, "Drain::Topic");
        until("matched", 1, || rt.user_writer_matched_count(eid));
        let body = vec![7u8; 4096];
        let mut cdr = vec![0, 1, 0, 0];
        cdr.extend_from_slice(&body);
        let sample = Sample {
            cdr_payload: cdr,
            ..Sample::default()
        };
        for _ in 0..50 {
            dds.publish("Drain::Topic", sample_to_publish(&t, &sample).unwrap())
                .unwrap()
                .unwrap();
        }
        assert!(dds.await_writes(Instant::now() + Duration::from_secs(10)));
        assert!(
            rt.user_writer_all_acknowledged(eid),
            "await_writes returned before the acknowledgements"
        );
        assert_eq!(alive(&rx, Duration::from_millis(200)).len(), 50);
        // A reader that stops without notice stays matched and never
        // acknowledges the next sample: the wait runs to its limit.
        other.__vanish();
        dds.publish("Drain::Topic", sample_to_publish(&t, &sample).unwrap())
            .unwrap()
            .unwrap();
        let started = Instant::now();
        assert!(!dds.await_writes(started + Duration::from_millis(300)));
        assert!(started.elapsed() >= Duration::from_millis(280));
        rt.shutdown();
    }

    /// Spec §6 Lifespan: the waits for a DataWriter of another lifespan run
    /// outside the pool's lock. While a ninth lifespan waits for the
    /// acknowledgements of the least recently used writer (its reader
    /// vanished, so they never come), a call for a writer that exists
    /// returns at once, and a call for the lifespan being replaced waits for
    /// the end and gets its writer back when the replacement fails.
    #[test]
    fn grpcbridge_6_a_lifespan_writer_waits_outside_the_pool_lock() {
        let lease = zerodds_test_domains::lease_domain();
        let rt = runtime(&lease);
        let other = peer(&lease);
        let t = TopicMapping {
            direction: "in".into(),
            ..topic("Lifespan::Pool")
        };
        let dds = BridgeDds::on(Arc::clone(&rt), std::slice::from_ref(&t)).unwrap();
        let own = dds.writer_of("Lifespan::Pool").unwrap();
        let (_, _rx) = peer_reader(&other, "Lifespan::Pool");
        until("matched", 1, || rt.user_writer_matched_count(own));
        let lifespans: Vec<u64> = (1..=8).map(|i| i * 10_000).collect();
        let mut eids = HashMap::new();
        for ms in &lifespans {
            eids.insert(*ms, dds.lifespan_writer("Lifespan::Pool", *ms).unwrap());
        }
        // The reader stops without notice: the sample of the first writer
        // stays unacknowledged; the other seven are used after it.
        other.__vanish();
        let sample = Sample {
            cdr_payload: vec![0, 1, 0, 0, 9],
            ..Sample::default()
        };
        let mut out = sample_to_publish(&t, &sample).unwrap();
        out.lifespan_ms = Some(10_000);
        dds.publish("Lifespan::Pool", out).unwrap().unwrap();
        for ms in &lifespans[1..] {
            dds.lifespan_writer("Lifespan::Pool", *ms).unwrap();
        }
        std::thread::scope(|scope| {
            let ninth = scope.spawn(|| {
                let started = Instant::now();
                let r = dds.lifespan_writer("Lifespan::Pool", 90_000);
                (r, started.elapsed())
            });
            // The ninth call has put the least recently used writer into
            // eviction before the two timed calls.
            let bound = Instant::now() + Duration::from_secs(5);
            while !dds.lifespan_evicting("Lifespan::Pool", 10_000) {
                assert!(
                    Instant::now() < bound,
                    "the ninth lifespan did not start the eviction"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
            let started = Instant::now();
            assert_eq!(
                dds.lifespan_writer("Lifespan::Pool", 50_000).unwrap(),
                eids[&50_000]
            );
            assert!(
                started.elapsed() < Duration::from_millis(300),
                "an existing writer waited {:?}",
                started.elapsed()
            );
            let started = Instant::now();
            assert_eq!(
                dds.lifespan_writer("Lifespan::Pool", 10_000).unwrap(),
                eids[&10_000],
                "the writer being replaced comes back"
            );
            assert!(
                started.elapsed() >= Duration::from_millis(500),
                "{:?}",
                started.elapsed()
            );
            let (r, took) = ninth.join().unwrap();
            assert!(
                matches!(r, Err(zerodds_dcps::DdsError::OutOfResources { .. })),
                "{r:?}"
            );
            assert!(took >= Duration::from_millis(900), "{took:?}");
        });
        rt.shutdown();
    }

    /// Spec §4.2, §9.1: a DataWriter for each topic carried into DDS
    /// (`in`, `bidir`), none for `out`; a DataReader per stream only for a
    /// topic carried out of DDS (`out`, `bidir`), deleted with
    /// `close_reader` (the peer's writer loses the match).
    #[test]
    fn grpcbridge_12_1_dds_pump_the_entities_follow_the_direction() {
        let lease = zerodds_test_domains::lease_domain();
        let rt = runtime(&lease);
        let other = peer(&lease);
        let mapping = |name: &str, direction: &str| TopicMapping {
            direction: direction.into(),
            ..topic(name)
        };
        let topics = [
            mapping("Dir::In", "in"),
            mapping("Dir::Out", "out"),
            mapping("Dir::Both", "bidir"),
        ];
        let dds = BridgeDds::on(Arc::clone(&rt), &topics).unwrap();
        assert!(dds.writer_of("Dir::In").is_some());
        assert!(dds.writer_of("Dir::Both").is_some());
        assert!(dds.writer_of("Dir::Out").is_none());
        let sample = Sample {
            cdr_payload: vec![0, 1, 0, 0, 1],
            ..Sample::default()
        };
        assert!(
            dds.publish("Dir::Out", sample_to_publish(&topics[1], &sample).unwrap())
                .is_none()
        );
        assert!(dds.open_reader("Dir::In", Vec::new(), false).is_err());
        let w = other
            .register_user_writer(zerodds_dcps::runtime::UserWriterConfig {
                topic_name: "Dir::Out".into(),
                type_name: "T::Type".into(),
                reliable: true,
                ..Default::default()
            })
            .unwrap();
        let first = dds.open_reader("Dir::Out", Vec::new(), false).unwrap();
        let second = dds.open_reader("Dir::Out", Vec::new(), false).unwrap();
        until("two stream readers", 2, || {
            other.user_writer_matched_count(w)
        });
        dds.close_reader(first);
        until("one stream reader", 1, || {
            other.user_writer_matched_count(w)
        });
        dds.close_reader(second);
        until("no stream reader", 0, || other.user_writer_matched_count(w));
        rt.shutdown();
        other.shutdown();
    }

    /// Spec §4.4: the encapsulation identifier of `cdr_payload` reaches the
    /// DDS reader — the XCDR version, the format and the byte order of the
    /// body the client sent.
    #[test]
    fn grpcbridge_12_1_dds_pump_the_encapsulation_reaches_the_reader() {
        let lease = zerodds_test_domains::lease_domain();
        let rt = runtime(&lease);
        let other = peer(&lease);
        let t = TopicMapping {
            direction: "in".into(),
            ..topic("Enc::Topic")
        };
        let dds = BridgeDds::on(Arc::clone(&rt), std::slice::from_ref(&t)).unwrap();
        let eid = dds.writer_of("Enc::Topic").unwrap();
        let (_, rx) = peer_reader(&other, "Enc::Topic");
        until("matched", 1, || rt.user_writer_matched_count(eid));
        let sent = [([0x00, 0x00], 1u8), ([0x00, 0x07], 2), ([0x00, 0x01], 3)];
        for (identifier, body) in sent {
            let sample = Sample {
                cdr_payload: vec![identifier[0], identifier[1], 0, 0, body, 0, 0, 0],
                ..Sample::default()
            };
            dds.publish("Enc::Topic", sample_to_publish(&t, &sample).unwrap())
                .unwrap()
                .unwrap();
        }
        let got = alive(&rx, Duration::from_millis(500));
        assert_eq!(
            got.iter().map(|(p, id)| (*id, p[0])).collect::<Vec<_>>(),
            sent.to_vec()
        );
        rt.shutdown();
        other.shutdown();
    }

    /// Spec §9.2 "Clean up DDS entities": `close` deletes the DataWriters
    /// and stops the participant; the peer's reader loses the match.
    #[test]
    fn grpcbridge_9_2_close_deletes_the_entities() {
        let lease = zerodds_test_domains::lease_domain();
        let rt = runtime(&lease);
        let other = peer(&lease);
        let t = TopicMapping {
            direction: "in".into(),
            ..topic("Close::Topic")
        };
        let dds = BridgeDds::on(Arc::clone(&rt), std::slice::from_ref(&t)).unwrap();
        let (reader, _rx) = peer_reader(&other, "Close::Topic");
        until("matched", 1, || other.user_reader_matched_count(reader));
        dds.close();
        until("unmatched", 0, || other.user_reader_matched_count(reader));
        other.shutdown();
    }

    /// Spec §3 `keyed`, §10: the DataWriter and the stream readers of a
    /// topic with `keyed: false` are NO_KEY endpoints (DDSI-RTPS 2.5
    /// §9.3.1.2: writer 0x03, reader 0x04), those of a keyed topic WITH_KEY
    /// endpoints (0x02, 0x07).
    #[test]
    fn grpcbridge_10_the_endpoints_of_a_keyless_topic_are_no_key_endpoints() {
        use zerodds_rtps::wire_types::EntityKind;
        let lease = zerodds_test_domains::lease_domain();
        let rt = runtime(&lease);
        let keyless = TopicMapping {
            keyed: false,
            ..topic("Keyless::Topic")
        };
        let dds = BridgeDds::on(Arc::clone(&rt), &[keyless, topic("Keyed::Topic")]).unwrap();
        assert_eq!(
            dds.writers["Keyless::Topic"].entity_kind,
            EntityKind::UserWriterNoKey
        );
        assert_eq!(
            dds.writers["Keyed::Topic"].entity_kind,
            EntityKind::UserWriterWithKey
        );
        let r = dds
            .open_reader("Keyless::Topic", Vec::new(), false)
            .unwrap();
        assert_eq!(r.eid.entity_kind, EntityKind::UserReaderNoKey);
        let r = dds.open_reader("Keyed::Topic", Vec::new(), false).unwrap();
        assert_eq!(r.eid.entity_kind, EntityKind::UserReaderWithKey);
        dds.close();
    }
}
