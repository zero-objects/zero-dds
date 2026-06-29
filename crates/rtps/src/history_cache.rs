// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `HistoryCache` — ordered sample storage for reliable writer/reader.
//!
//! DDSI-RTPS 2.5 §8.4.8. Both sides (writer + reader) hold their own
//! cache instance:
//!
//! - **Writer cache**: `CacheChange`s stored via `write()`, from which
//!   re-sends happen on AckNack request. Removes samples only
//!   once **all** matched readers have confirmed them via AckNack.
//! - **Reader cache**: received `CacheChange`s in SN order, for
//!   in-order delivery to the application layer. Can be cleared via
//!   `remove_up_to` after delivery.
//!
//! **History QoS (WP 1.4 T3 follow-up):** the cache is configured via
//! [`HistoryKind`] — `KeepAll` (hard-bounded, error on
//! overflow) vs. `KeepLast(depth)` (ring buffer, the oldest sample drops
//! out on overflow). KeepLast is spec-conformant (§8.7.4) and
//! decouples writer-cache GC from reader-ACKNACK progress — a
//! stalled reader thereby no longer prevents other
//! readers from getting further samples ("per-destination queue" model).

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
// portable_atomic instead of core::sync::atomic, so AtomicI64/AtomicU64
// also work on Cortex-M targets without native 64-bit atomics.
// On x86_64/aarch64 Linux this is identical to the stdlib.
use portable_atomic::{AtomicI64, AtomicU64, AtomicUsize, Ordering};

use crate::wire_types::SequenceNumber;

#[cfg(feature = "inspect")]
use alloc::borrow::ToOwned;

#[cfg(feature = "inspect")]
fn dispatch_rtps_tap(label: &str, sn: SequenceNumber, payload: Vec<u8>) {
    let ts_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_nanos()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    #[allow(clippy::cast_sign_loss)]
    let corr = sn.0 as u64;
    let frame = zerodds_inspect_endpoint::Frame::rtps(label.to_owned(), ts_ns, corr, payload);
    zerodds_inspect_endpoint::tap::dispatch(&frame);
}

/// Kind of a cache entry (DDSI-RTPS §8.2.1.2 / §8.7.2.2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// Valid sample, accepted by the DDS DataReader filter.
    Alive,
    /// Valid sample, discarded by the reader-side filter (TIME_BASED_FILTER /
    /// ContentFilteredTopic) — but stays "available" in the NACK
    /// path, so the reliable writer does not re-send it
    /// (filteredCount counter in `ChangeFromWriter`).
    AliveFiltered,
    /// `dispose` marker.
    NotAliveDisposed,
    /// `unregister` marker.
    NotAliveUnregistered,
    /// Combined dispose+unregister.
    NotAliveDisposedUnregistered,
}

impl ChangeKind {
    /// Spec §8.4.10.5 — `is_relevant` true for all live kinds; only
    /// `AliveFiltered` is explicitly *not* relevant for the DDS
    /// user-API path (but counts as "received" in the NACK path).
    #[must_use]
    pub fn is_relevant(self) -> bool {
        !matches!(self, Self::AliveFiltered)
    }

    /// Spec §8.4.10.5 — `is_alive_kind` covers Alive + AliveFiltered.
    #[must_use]
    pub fn is_alive_kind(self) -> bool {
        matches!(self, Self::Alive | Self::AliveFiltered)
    }
}

/// Single cache entry.
///
/// `payload` is held as `Arc<[u8]>` — the cache, the writer build-
/// datagram path and reader delivery share a single
/// allocation. This saves the n-fold `Vec::clone()` per reader proxy
/// in the reliable-writer tick (perf audit F7/F8/F10, ~30-50 %
/// throughput gain for large payloads).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheChange {
    /// Sequence number (writer-locally unique).
    pub sequence_number: SequenceNumber,
    /// Payload (serialized sample), reference-counted.
    pub payload: Arc<[u8]>,
    /// Kind of the event.
    pub kind: ChangeKind,
    /// Optional `PID_KEY_HASH` from the inline QoS (Spec §9.6.4.8).
    /// Reader side: filled for keyed topics + ALIVE samples with an inline hash
    /// or for lifecycle markers (Disposed/Unregistered)
    /// by the reader path. Writer side: set by the writer when the path
    /// propagates the hash along the sample pipeline.
    pub key_hash: Option<[u8; 16]>,
    /// Source timestamp of the write (DDSI-RTPS §8.3.7.9 / §8.7.3). When set,
    /// the writer prepends an INFO_TS submessage before the DATA so a remote
    /// reader can populate `SampleInfo.source_timestamp` and apply
    /// `DESTINATION_ORDER = BY_SOURCE_TIMESTAMP`. `None` ⇒ no INFO_TS (the
    /// reader falls back to reception order). Held on the change so retransmits
    /// carry the *original* timestamp, not the resend wall-clock.
    pub source_timestamp: Option<crate::header_extension::HeTimestamp>,
}

impl CacheChange {
    /// Creates an alive change. Takes `Vec<u8>` and
    /// converts once into `Arc<[u8]>`. For call sites that already have the
    /// allocation as an Arc, there is [`Self::alive_arc`].
    #[must_use]
    pub fn alive(sn: SequenceNumber, payload: Vec<u8>) -> Self {
        Self::alive_arc(sn, Arc::from(payload))
    }

    /// Creates an alive change from an already-existing
    /// `Arc<[u8]>` payload. Zero-copy path for callers that share the
    /// allocation (writer ↔ cache ↔ datagram).
    ///
    /// **Crate-internal:** external users should enter via [`Self::alive`] with
    /// `Vec<u8>` — the Arc path is a pure writer/reader-
    /// internal optimization and should not accidentally become part of the
    /// public API.
    #[must_use]
    pub(crate) fn alive_arc(sn: SequenceNumber, payload: Arc<[u8]>) -> Self {
        Self {
            sequence_number: sn,
            payload,
            kind: ChangeKind::Alive,
            key_hash: None,
            source_timestamp: None,
        }
    }

    /// Attaches a source timestamp (builder style). The writer emits an INFO_TS
    /// submessage before the DATA for any change that carries one.
    #[must_use]
    pub fn with_source_timestamp(
        mut self,
        ts: Option<crate::header_extension::HeTimestamp>,
    ) -> Self {
        self.source_timestamp = ts;
        self
    }

    /// Creates a lifecycle marker (Spec §8.2.1.2). `payload` is the
    /// key-only serialization of the disposed/unregistered instance —
    /// exactly what lands as `PID_KEY_HASH` in the inline QoS.
    #[must_use]
    pub fn lifecycle(sn: SequenceNumber, payload: Vec<u8>, kind: ChangeKind) -> Self {
        Self {
            sequence_number: sn,
            payload: Arc::from(payload),
            kind,
            key_hash: None,
            source_timestamp: None,
        }
    }
}

/// History-QoS (DDSI-RTPS §8.7.4, Spec Table 17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryKind {
    /// The cache is hard-bounded; on overflow `insert` returns a
    /// `CapacityExceeded` error. Useful for no-loss scenarios in
    /// which the writer prefers to block rather than discard data
    /// (file transfer, logging).
    KeepAll,
    /// The cache holds at most `depth` newest samples. On overflow the
    /// **oldest** sample drops out automatically — the writer `insert`
    /// never fails due to capacity. The spec default for DDS.
    KeepLast {
        /// Maximum number of samples in the cache.
        depth: usize,
    },
}

/// Error variants for cache operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CacheError {
    /// The `KeepAll` cache has reached its capacity.
    CapacityExceeded,
    /// The same SN was already inserted.
    DuplicateSequenceNumber,
    /// `KeepLast` with `depth == 0` — every insert would be an immediate
    /// drop. This case is rejected at `insert` instead of silently accepted.
    ZeroDepth,
}

/// Sentinel value for "no entry" in the `AtomicI64` stats. Chosen
/// so that every valid `SequenceNumber.0` (>= 1 per spec)
/// is uniquely distinguishable as a positive value.
const STATS_SENTINEL_NO_SN: i64 = i64::MIN;

/// Atomically updated snapshot statistics of a [`HistoryCache`].
///
/// **Note:** the actual `BTreeMap` storage of the cache
/// still needs `&mut self` to mutate (concurrent read/write
/// `BTreeMap` mutation does not exist in `std`), but **statistics values**
/// (length, max/min SN, eviction counter) are carried in parallel in an
/// `Arc<HistoryCacheStats>`. Monitoring threads, SEDP tick
/// loops and telemetry can thus poll lock-free, without taking the
/// writer/reader lock.
///
/// Consistency guarantee: every mutating method of the cache updates the
/// atomics **after** the `BTreeMap` mutation, with `Release` ordering.
/// Readers use `Acquire` ordering — they see a consistent
/// state of the **last** completed cache operation, never a
/// half-updated state of the individual atomics.
///
/// What is *not* guaranteed: cross-field consistency. If a
/// reader reads `len` and then `max_sn`, further inserts can have
/// happened between the loads. That is acceptable for
/// monitoring; for hard wire paths (heartbeat build) the
/// writer lock is still taken.
#[derive(Debug)]
pub struct HistoryCacheStats {
    /// Number of changes in the cache (corresponds to `BTreeMap::len`).
    pub len: AtomicUsize,
    /// Number of samples discarded by `KeepLast` eviction since start.
    pub evicted: AtomicU64,
    /// Highest SN in the cache, or [`STATS_SENTINEL_NO_SN`] if empty.
    pub max_sn: AtomicI64,
    /// Lowest SN in the cache, or [`STATS_SENTINEL_NO_SN`] if empty.
    pub min_sn: AtomicI64,
}

impl Default for HistoryCacheStats {
    fn default() -> Self {
        Self {
            len: AtomicUsize::new(0),
            evicted: AtomicU64::new(0),
            max_sn: AtomicI64::new(STATS_SENTINEL_NO_SN),
            min_sn: AtomicI64::new(STATS_SENTINEL_NO_SN),
        }
    }
}

impl HistoryCacheStats {
    /// Snapshot of the four atomics as a plain-old-data struct. Loaded with
    /// `Acquire` ordering — synchronized with the
    /// `Release` store in [`HistoryCache::insert`] /
    /// [`HistoryCache::remove_up_to`].
    #[must_use]
    pub fn snapshot(&self) -> HistoryCacheSnapshot {
        HistoryCacheSnapshot {
            len: self.len.load(Ordering::Acquire),
            evicted: self.evicted.load(Ordering::Acquire),
            max_sn: decode_sn_atom(self.max_sn.load(Ordering::Acquire)),
            min_sn: decode_sn_atom(self.min_sn.load(Ordering::Acquire)),
        }
    }
}

impl Clone for HistoryCacheStats {
    fn clone(&self) -> Self {
        Self {
            len: AtomicUsize::new(self.len.load(Ordering::Acquire)),
            evicted: AtomicU64::new(self.evicted.load(Ordering::Acquire)),
            max_sn: AtomicI64::new(self.max_sn.load(Ordering::Acquire)),
            min_sn: AtomicI64::new(self.min_sn.load(Ordering::Acquire)),
        }
    }
}

fn decode_sn_atom(v: i64) -> Option<SequenceNumber> {
    if v == STATS_SENTINEL_NO_SN {
        None
    } else {
        Some(SequenceNumber(v))
    }
}

fn encode_sn_atom(sn: Option<SequenceNumber>) -> i64 {
    sn.map_or(STATS_SENTINEL_NO_SN, |s| s.0)
}

/// Plain-old-data snapshot of the `HistoryCache` statistics at a
/// single point in time. Produced by [`HistoryCacheStats::snapshot`];
/// each component is loaded with `Acquire` ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryCacheSnapshot {
    /// Number of changes.
    pub len: usize,
    /// Number of samples discarded by `KeepLast` eviction.
    pub evicted: u64,
    /// Highest SN, if present.
    pub max_sn: Option<SequenceNumber>,
    /// Lowest SN, if present.
    pub min_sn: Option<SequenceNumber>,
}

/// Ordered sample storage.
///
/// Internal storage: `BTreeMap` for O(log n) insert/lookup and an efficient
/// range iterator.
///
/// **Note:** the writing methods still need
/// `&mut self` (BTreeMap is not concurrent-safe), but stats are
/// available in parallel in an `Arc<HistoryCacheStats>` — see
/// [`stats`](Self::stats).
#[derive(Debug)]
pub struct HistoryCache {
    changes: BTreeMap<SequenceNumber, CacheChange>,
    kind: HistoryKind,
    max_samples: usize,
    evicted_count: u64,
    stats: Arc<HistoryCacheStats>,
    /// Optional label for the inspect-endpoint tap (R-026). Set only with
    /// the cargo feature `inspect`; otherwise phantom.
    #[cfg(feature = "inspect")]
    inspect_label: Option<alloc::string::String>,
}

impl Clone for HistoryCache {
    fn clone(&self) -> Self {
        // Each clone gets its **own** `Arc<HistoryCacheStats>`,
        // so mutations on the clone do not corrupt the stats of the
        // original. The initial value is taken from the original,
        // so a Cache.clone() shows an equivalent
        // snapshot.
        Self {
            changes: self.changes.clone(),
            kind: self.kind,
            max_samples: self.max_samples,
            evicted_count: self.evicted_count,
            stats: Arc::new((*self.stats).clone()),
            #[cfg(feature = "inspect")]
            inspect_label: self.inspect_label.clone(),
        }
    }
}

impl HistoryCache {
    /// Creates a new cache. `max_samples` is the upper bound:
    /// with `KeepAll` exceeding it leads to `CapacityExceeded`, with
    /// `KeepLast` to LRU eviction of the oldest sample.
    #[must_use]
    pub fn new_with_kind(kind: HistoryKind, max_samples: usize) -> Self {
        Self {
            changes: BTreeMap::new(),
            kind,
            max_samples,
            evicted_count: 0,
            stats: Arc::new(HistoryCacheStats::default()),
            #[cfg(feature = "inspect")]
            inspect_label: None,
        }
    }

    /// Sets the inspect-endpoint label for the RTPS-layer tap.
    /// No-op if the `inspect` feature is off.
    #[cfg(feature = "inspect")]
    pub fn set_inspect_label(&mut self, label: alloc::string::String) {
        self.inspect_label = Some(label);
    }

    /// Shared stats handle for **lock-free** monitoring.
    ///
    /// Consumers hold an `Arc<HistoryCacheStats>` and poll the
    /// atomics with `Acquire` ordering. Values always reflect a
    /// completed cache mutation; cross-field consistency between
    /// `len` and `max_sn` is not guaranteed (tear risk).
    #[must_use]
    pub fn stats(&self) -> Arc<HistoryCacheStats> {
        Arc::clone(&self.stats)
    }

    /// Synchronizes the atomics with the current `BTreeMap` state.
    /// Called at the end of all mutating methods.
    fn refresh_stats(&self) {
        self.stats.len.store(self.changes.len(), Ordering::Release);
        self.stats
            .evicted
            .store(self.evicted_count, Ordering::Release);
        let max = self.changes.keys().next_back().copied();
        let min = self.changes.keys().next().copied();
        self.stats
            .max_sn
            .store(encode_sn_atom(max), Ordering::Release);
        self.stats
            .min_sn
            .store(encode_sn_atom(min), Ordering::Release);
    }

    /// Legacy constructor — creates a `KeepAll` cache with a hard
    /// capacity limit. For new users prefer [`new_with_kind`].
    #[must_use]
    pub fn new(max_samples: usize) -> Self {
        Self::new_with_kind(HistoryKind::KeepAll, max_samples)
    }

    /// History kind of this cache.
    #[must_use]
    pub fn kind(&self) -> HistoryKind {
        self.kind
    }

    /// **Expert-only**: sets the history kind and `max_samples` cap at runtime.
    ///
    /// Usage: short-term expansion for a backend replay burst
    /// (DurabilityService §2.2.3.5), when KeepLast(1) would collapse the
    /// replay window. The caller must restore the original kind
    /// afterwards.
    ///
    /// This method moves **no** existing samples. If the
    /// new cap is smaller than the current sample count, the
    /// existing samples stay visible — the next `insert` then
    /// evicts by KeepLast rules.
    pub fn set_kind_and_max(&mut self, kind: HistoryKind, max_samples: usize) {
        self.kind = kind;
        self.max_samples = max_samples;
    }

    /// `max_samples` cap of the cache.
    #[must_use]
    pub fn max_samples(&self) -> usize {
        self.max_samples
    }

    /// Number of samples discarded by `KeepLast` eviction since
    /// start.
    #[must_use]
    pub fn evicted_count(&self) -> u64 {
        self.evicted_count
    }

    /// Inserts a change.
    ///
    /// # Errors
    /// - `CapacityExceeded`: only with `KeepAll`, cache full.
    /// - `DuplicateSequenceNumber`: SN already present.
    /// - `ZeroDepth`: `KeepLast { depth: 0 }`.
    pub fn insert(&mut self, change: CacheChange) -> Result<(), CacheError> {
        // Backward-compat wrapper — callers that want the evicted sample back for
        // their payload pool use
        // `insert_returning_evicted` directly.
        self.insert_returning_evicted(change).map(|_| ())
    }

    /// Like [`Self::insert`], but returns the evicted `CacheChange`
    /// (or `None` if nothing was evicted). Allows the caller to
    /// recycle the payload `Arc<[u8]>` instead of dropping it — see
    /// the `ReliableWriter::stage_sample` hot-path pool.
    ///
    /// # Errors
    /// As [`Self::insert`].
    pub fn insert_returning_evicted(
        &mut self,
        change: CacheChange,
    ) -> Result<Option<CacheChange>, CacheError> {
        if self.changes.contains_key(&change.sequence_number) {
            return Err(CacheError::DuplicateSequenceNumber);
        }
        let cap = self.effective_max_samples()?;
        let mut evicted: Option<CacheChange> = None;
        if self.changes.len() >= cap {
            match self.kind {
                HistoryKind::KeepAll => return Err(CacheError::CapacityExceeded),
                HistoryKind::KeepLast { .. } => {
                    // Oldest sample out (LRU by SN order).
                    if let Some((&oldest, _)) = self.changes.iter().next() {
                        evicted = self.changes.remove(&oldest);
                        self.evicted_count = self.evicted_count.saturating_add(1);
                    }
                }
            }
        }
        #[cfg(feature = "inspect")]
        let tap_view = self.inspect_label.as_ref().map(|label| {
            (
                label.clone(),
                change.sequence_number,
                change.payload.to_vec(),
            )
        });
        self.changes.insert(change.sequence_number, change);
        self.refresh_stats();
        #[cfg(feature = "inspect")]
        if let Some((label, sn, payload)) = tap_view {
            dispatch_rtps_tap(&label, sn, payload);
        }
        Ok(evicted)
    }

    /// Effective max-samples = min(max_samples, depth).
    fn effective_max_samples(&self) -> Result<usize, CacheError> {
        match self.kind {
            HistoryKind::KeepAll => Ok(self.max_samples),
            HistoryKind::KeepLast { depth } => {
                if depth == 0 {
                    return Err(CacheError::ZeroDepth);
                }
                Ok(core::cmp::min(depth, self.max_samples))
            }
        }
    }

    /// Fetches a change by SN.
    #[must_use]
    pub fn get(&self, sn: SequenceNumber) -> Option<&CacheChange> {
        self.changes.get(&sn)
    }

    /// Removes all changes with SN ≤ `sn`.
    /// Returns the number of removed entries.
    pub fn remove_up_to(&mut self, sn: SequenceNumber) -> usize {
        let keep = self.changes.split_off(&SequenceNumber(sn.0 + 1));
        let removed = self.changes.len();
        self.changes = keep;
        self.refresh_stats();
        removed
    }

    /// Iterates in SN order over changes in the range `[lo, hi]`
    /// (both inclusive).
    pub fn iter_range(
        &self,
        lo: SequenceNumber,
        hi: SequenceNumber,
    ) -> impl Iterator<Item = &CacheChange> + '_ {
        self.changes.range(lo..=hi).map(|(_, v)| v)
    }

    /// Smallest SN in the cache.
    #[must_use]
    pub fn min_sn(&self) -> Option<SequenceNumber> {
        self.changes.keys().next().copied()
    }

    /// Largest SN in the cache.
    #[must_use]
    pub fn max_sn(&self) -> Option<SequenceNumber> {
        self.changes.keys().next_back().copied()
    }

    /// Number of changes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// True if no changes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Maximum capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.max_samples
    }
}

// ============================================================================
// LockFreeReadHistoryCache — // ============================================================================

/// `HistoryCache` variant with a lock-free read path via RCU/copy-on-write.
///
/// **Note:** readers (e.g. the SEDP tick, heartbeat build,
/// resend iteration) see an `Arc`-stable snapshot of the
/// `BTreeMap` storage; they access it without a further lock touch.
/// Writers serialize over an internal [`RcuCell`] mutex and
/// publish copy-on-write.
///
/// # Trade-offs
///
/// * **Read path**: 1× mutex acquire for the refcount inc + 0 further
///   locks. Concurrent readers see the same Arc; read iteration is
///   essentially lock-free.
/// * **Write path**: O(n) per insert/remove, because the `BTreeMap`
///   content must be cloned. Acceptable for small caches
///   (<= 1000 samples). For write-heavy paths keep using [`HistoryCache`].
/// * **Memory**: active snapshots consume memory until they are released
///   (reader lifetime). Cache mutations do not invalidate
///   existing reader snapshots.
///
/// # When to use
///
/// * Discovery caches that are read frequently by the SEDP tick + match loops,
///   but mutated only on `announce_publication`/`subscription`.
/// * Monitoring/tooling paths that want to iterate over the cache
///   without a lock touch.
///
/// # When NOT to use
///
/// * Reliable-writer cache with a high insert rate (every insert clones
///   the whole BTreeMap). There [`HistoryCache`] stays better.
///
/// Persistent data structures (`im::OrdMap`) would push the
/// write-cost effort to O(log n) — that is the
/// natural follow-up optimization once this variant lands in production.
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct LockFreeReadHistoryCache {
    inner: zerodds_foundation::rcu::RcuCell<LockFreeInner>,
    stats: Arc<HistoryCacheStats>,
}

/// Inner state of the [`LockFreeReadHistoryCache`]. Handed out by
/// [`LockFreeReadHistoryCache::snapshot`] as an `Arc<LockFreeInner>`
/// — readers iterate over `changes` directly.
#[cfg(feature = "std")]
#[derive(Debug, Clone)]
pub struct LockFreeInner {
    /// Sample-Map keyed by SequenceNumber.
    pub changes: BTreeMap<SequenceNumber, CacheChange>,
    /// History QoS kind.
    pub kind: HistoryKind,
    /// Cap from QoS.
    pub max_samples: usize,
    /// Eviction counter.
    pub evicted_count: u64,
}

#[cfg(feature = "std")]
impl LockFreeInner {
    fn effective_max_samples(&self) -> Result<usize, CacheError> {
        match self.kind {
            HistoryKind::KeepAll => Ok(self.max_samples),
            HistoryKind::KeepLast { depth } => {
                if depth == 0 {
                    return Err(CacheError::ZeroDepth);
                }
                Ok(core::cmp::min(depth, self.max_samples))
            }
        }
    }
}

#[cfg(feature = "std")]
impl LockFreeReadHistoryCache {
    /// Creates a new lock-free read cache.
    #[must_use]
    pub fn new_with_kind(kind: HistoryKind, max_samples: usize) -> Self {
        Self {
            inner: zerodds_foundation::rcu::RcuCell::new(LockFreeInner {
                changes: BTreeMap::new(),
                kind,
                max_samples,
                evicted_count: 0,
            }),
            stats: Arc::new(HistoryCacheStats::default()),
        }
    }

    /// Legacy constructor — `KeepAll`.
    #[must_use]
    pub fn new(max_samples: usize) -> Self {
        Self::new_with_kind(HistoryKind::KeepAll, max_samples)
    }

    /// Lock-free read snapshot of stats.
    #[must_use]
    pub fn stats(&self) -> Arc<HistoryCacheStats> {
        Arc::clone(&self.stats)
    }

    /// Returns an `Arc` snapshot of the current cache state for
    /// lock-free iteration.
    #[must_use]
    pub fn snapshot(&self) -> Arc<LockFreeInner> {
        self.inner.read()
    }

    /// History kind.
    #[must_use]
    pub fn kind(&self) -> HistoryKind {
        self.inner.read().kind
    }

    /// Number of samples discarded by `KeepLast` eviction.
    #[must_use]
    pub fn evicted_count(&self) -> u64 {
        self.stats.evicted.load(Ordering::Acquire)
    }

    /// Number of changes (Acquire load of the atomic).
    #[must_use]
    pub fn len(&self) -> usize {
        self.stats.len.load(Ordering::Acquire)
    }

    /// True if no changes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Smallest SN from the atom — lock-free.
    #[must_use]
    pub fn min_sn(&self) -> Option<SequenceNumber> {
        decode_sn_atom(self.stats.min_sn.load(Ordering::Acquire))
    }

    /// Largest SN from the atom — lock-free.
    #[must_use]
    pub fn max_sn(&self) -> Option<SequenceNumber> {
        decode_sn_atom(self.stats.max_sn.load(Ordering::Acquire))
    }

    /// Maximum capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.inner.read().max_samples
    }

    /// Fetches a change by SN — cloned (CacheChange is Arc-payload-
    /// wrapped, so a refcount inc).
    #[must_use]
    pub fn get(&self, sn: SequenceNumber) -> Option<CacheChange> {
        self.inner.read().changes.get(&sn).cloned()
    }

    /// Sample snapshot in the SN range `[lo, hi]`. Returns a Vec
    /// — we cannot return an Iter `<'a>` over an Arc snapshot
    /// when the snapshot is not referenced.
    #[must_use]
    pub fn iter_range_snapshot(&self, lo: SequenceNumber, hi: SequenceNumber) -> Vec<CacheChange> {
        let snap = self.inner.read();
        snap.changes
            .range(lo..=hi)
            .map(|(_, v)| v.clone())
            .collect()
    }

    /// Inserts a change. Copy-on-write of the BTreeMap.
    ///
    /// # Errors
    /// As [`HistoryCache::insert`].
    pub fn insert(&self, change: CacheChange) -> Result<(), CacheError> {
        // Pre-check: avoid the BTreeMap clone if we already know
        // the insert fails (Duplicate, ZeroDepth, KeepAll-full).
        let dup_or_full: Result<(), CacheError> = {
            let snap = self.inner.read();
            if snap.changes.contains_key(&change.sequence_number) {
                Err(CacheError::DuplicateSequenceNumber)
            } else {
                let cap = snap.effective_max_samples()?;
                if snap.changes.len() >= cap {
                    if matches!(snap.kind, HistoryKind::KeepAll) {
                        Err(CacheError::CapacityExceeded)
                    } else {
                        Ok(()) // KeepLast: evict in the write-with
                    }
                } else {
                    Ok(())
                }
            }
        };
        dup_or_full?;
        self.inner.modify(|inner| {
            // Eviction logic: KeepLast drops the oldest.
            let cap = match inner.effective_max_samples() {
                Ok(c) => c,
                Err(_) => return,
            };
            if inner.changes.len() >= cap {
                if let HistoryKind::KeepLast { .. } = inner.kind {
                    if let Some((&oldest, _)) = inner.changes.iter().next() {
                        inner.changes.remove(&oldest);
                        inner.evicted_count = inner.evicted_count.saturating_add(1);
                    }
                }
            }
            inner.changes.insert(change.sequence_number, change.clone());
        });
        self.refresh_stats();
        Ok(())
    }

    /// Removes all changes with SN ≤ `sn`. Returns the number removed.
    pub fn remove_up_to(&self, sn: SequenceNumber) -> usize {
        let mut removed = 0;
        self.inner.modify(|inner| {
            let keep = inner.changes.split_off(&SequenceNumber(sn.0 + 1));
            removed = inner.changes.len();
            inner.changes = keep;
        });
        self.refresh_stats();
        removed
    }

    fn refresh_stats(&self) {
        let snap = self.inner.read();
        self.stats.len.store(snap.changes.len(), Ordering::Release);
        self.stats
            .evicted
            .store(snap.evicted_count, Ordering::Release);
        let max = snap.changes.keys().next_back().copied();
        let min = snap.changes.keys().next().copied();
        self.stats
            .max_sn
            .store(encode_sn_atom(max), Ordering::Release);
        self.stats
            .min_sn
            .store(encode_sn_atom(min), Ordering::Release);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sn(n: i64) -> SequenceNumber {
        SequenceNumber(n)
    }

    fn alive(n: i64) -> CacheChange {
        CacheChange::alive(sn(n), alloc::vec![n as u8])
    }

    #[test]
    fn new_cache_is_empty() {
        let c = HistoryCache::new(10);
        assert_eq!(c.len(), 0);
        assert!(c.is_empty());
        assert_eq!(c.min_sn(), None);
        assert_eq!(c.max_sn(), None);
    }

    #[test]
    fn insert_and_get() {
        let mut c = HistoryCache::new(10);
        c.insert(alive(1)).expect("insert");
        c.insert(alive(2)).expect("insert");
        assert_eq!(
            c.get(sn(1)).map(|ch| ch.payload.as_ref().to_vec()),
            Some(alloc::vec![1])
        );
        assert_eq!(c.get(sn(3)), None);
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn insert_duplicate_is_err() {
        let mut c = HistoryCache::new(10);
        c.insert(alive(1)).expect("insert");
        assert_eq!(c.insert(alive(1)), Err(CacheError::DuplicateSequenceNumber));
    }

    #[test]
    fn insert_at_capacity_is_err() {
        let mut c = HistoryCache::new(2);
        c.insert(alive(1)).expect("insert");
        c.insert(alive(2)).expect("insert");
        assert_eq!(c.insert(alive(3)), Err(CacheError::CapacityExceeded));
    }

    #[test]
    fn min_max_sn_reflect_content() {
        let mut c = HistoryCache::new(10);
        c.insert(alive(5)).unwrap();
        c.insert(alive(3)).unwrap();
        c.insert(alive(7)).unwrap();
        assert_eq!(c.min_sn(), Some(sn(3)));
        assert_eq!(c.max_sn(), Some(sn(7)));
    }

    #[test]
    fn remove_up_to_inclusive() {
        let mut c = HistoryCache::new(10);
        for i in 1..=5 {
            c.insert(alive(i)).unwrap();
        }
        let removed = c.remove_up_to(sn(3));
        assert_eq!(removed, 3);
        assert_eq!(c.len(), 2);
        assert_eq!(c.min_sn(), Some(sn(4)));
    }

    #[test]
    fn remove_up_to_with_no_matches_is_noop() {
        let mut c = HistoryCache::new(10);
        c.insert(alive(10)).unwrap();
        assert_eq!(c.remove_up_to(sn(5)), 0);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn iter_range_is_ordered() {
        let mut c = HistoryCache::new(10);
        for i in [5, 1, 3, 8, 2] {
            c.insert(alive(i)).unwrap();
        }
        let collected: alloc::vec::Vec<i64> = c
            .iter_range(sn(2), sn(5))
            .map(|ch| ch.sequence_number.0)
            .collect();
        assert_eq!(collected, alloc::vec![2, 3, 5]);
    }

    #[test]
    fn iter_range_empty_when_no_overlap() {
        let mut c = HistoryCache::new(10);
        c.insert(alive(1)).unwrap();
        c.insert(alive(2)).unwrap();
        assert_eq!(c.iter_range(sn(10), sn(20)).count(), 0);
    }

    #[test]
    fn capacity_accessor() {
        let c = HistoryCache::new(42);
        assert_eq!(c.capacity(), 42);
    }

    #[test]
    fn cache_change_alive_constructor() {
        let ch = CacheChange::alive(sn(1), alloc::vec![1, 2, 3]);
        assert_eq!(ch.kind, ChangeKind::Alive);
        assert_eq!(ch.sequence_number, sn(1));
        assert_eq!(ch.payload.as_ref(), &[1, 2, 3][..]);
    }

    // ---- §8.2.1.2 ChangeKind_t all 4 spec variants + AliveFiltered ----

    #[test]
    fn change_kind_alive_is_relevant_and_alive() {
        assert!(ChangeKind::Alive.is_relevant());
        assert!(ChangeKind::Alive.is_alive_kind());
    }

    #[test]
    fn change_kind_alive_filtered_is_alive_but_not_relevant() {
        // Spec §8.4.10.5: AliveFiltered is alive_kind but !is_relevant.
        assert!(ChangeKind::AliveFiltered.is_alive_kind());
        assert!(!ChangeKind::AliveFiltered.is_relevant());
    }

    #[test]
    fn change_kind_not_alive_kinds_are_not_alive() {
        for k in [
            ChangeKind::NotAliveDisposed,
            ChangeKind::NotAliveUnregistered,
            ChangeKind::NotAliveDisposedUnregistered,
        ] {
            assert!(!k.is_alive_kind(), "{k:?}");
            assert!(k.is_relevant(), "{k:?}");
        }
    }

    #[test]
    fn change_kind_distinct_variants() {
        // Identity sanity — all 5 variants are distinct.
        let v = [
            ChangeKind::Alive,
            ChangeKind::AliveFiltered,
            ChangeKind::NotAliveDisposed,
            ChangeKind::NotAliveUnregistered,
            ChangeKind::NotAliveDisposedUnregistered,
        ];
        for (i, a) in v.iter().enumerate() {
            for (j, b) in v.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b);
                }
            }
        }
    }

    // ========================================================================
    // D.4 Phase A — Atomic Stats / Lock-Free Snapshot Tests
    // ========================================================================

    #[test]
    fn stats_default_is_empty_with_no_sn() {
        let c = HistoryCache::new(10);
        let snap = c.stats().snapshot();
        assert_eq!(snap.len, 0);
        assert_eq!(snap.evicted, 0);
        assert_eq!(snap.max_sn, None);
        assert_eq!(snap.min_sn, None);
    }

    #[test]
    fn stats_track_insert_and_remove() {
        let mut c = HistoryCache::new(10);
        c.insert(alive(3)).unwrap();
        c.insert(alive(5)).unwrap();
        c.insert(alive(7)).unwrap();
        let snap = c.stats().snapshot();
        assert_eq!(snap.len, 3);
        assert_eq!(snap.min_sn, Some(sn(3)));
        assert_eq!(snap.max_sn, Some(sn(7)));
        assert_eq!(snap.evicted, 0);

        c.remove_up_to(sn(5));
        let snap = c.stats().snapshot();
        assert_eq!(snap.len, 1);
        assert_eq!(snap.min_sn, Some(sn(7)));
        assert_eq!(snap.max_sn, Some(sn(7)));
    }

    #[test]
    fn stats_track_keeplast_eviction() {
        let mut c = HistoryCache::new_with_kind(HistoryKind::KeepLast { depth: 2 }, 100);
        c.insert(alive(1)).unwrap();
        c.insert(alive(2)).unwrap();
        c.insert(alive(3)).unwrap(); // evicts alive(1)
        let snap = c.stats().snapshot();
        assert_eq!(snap.len, 2);
        assert_eq!(snap.evicted, 1);
        assert_eq!(snap.min_sn, Some(sn(2)));
        assert_eq!(snap.max_sn, Some(sn(3)));
    }

    #[test]
    fn stats_arc_is_shared_across_clones_of_handle() {
        // Multiple `cache.stats()` calls return the same Arc — so that
        // a reader that pulls the handle once sees all subsequent
        // cache mutations.
        let mut c = HistoryCache::new(10);
        let s1 = c.stats();
        let s2 = c.stats();
        assert!(Arc::ptr_eq(&s1, &s2));
        c.insert(alive(1)).unwrap();
        assert_eq!(s1.snapshot().len, 1);
        assert_eq!(s2.snapshot().len, 1);
    }

    #[test]
    fn stats_reader_thread_sees_inserts_concurrently() {
        // Lock-free read from a second thread while the writer
        // mutated. Correctness test for the Acquire/Release ordering.
        use std::sync::Arc as StdArc;
        use std::sync::Mutex as StdMutex;
        use std::thread;
        use std::time::Duration;

        let cache = StdArc::new(StdMutex::new(HistoryCache::new(2_000)));
        let stats = cache.lock().expect("init lock").stats();

        let writer_cache = StdArc::clone(&cache);
        let writer = thread::spawn(move || {
            for i in 1..=1_000 {
                let mut c = writer_cache.lock().expect("write lock");
                c.insert(alive(i)).expect("insert");
            }
        });

        let reader_stats = StdArc::clone(&stats);
        let reader = thread::spawn(move || {
            // Read stats 100x without taking the writer lock.
            for _ in 0..100 {
                let snap = reader_stats.snapshot();
                // len may only grow monotonically while the writer runs.
                assert!(snap.len <= 1_000);
                if let Some(max) = snap.max_sn {
                    assert!(max.0 >= 1 && max.0 <= 1_000);
                }
                thread::sleep(Duration::from_micros(50));
            }
        });

        writer.join().expect("writer joined");
        reader.join().expect("reader joined");

        let final_snap = stats.snapshot();
        assert_eq!(final_snap.len, 1_000);
        assert_eq!(final_snap.max_sn, Some(sn(1_000)));
        assert_eq!(final_snap.min_sn, Some(sn(1)));
    }

    #[test]
    fn clone_creates_independent_stats_handles() {
        // Cache.clone() must not share the stats Arc of the original,
        // otherwise mutations on the clone would corrupt the original.
        let mut a = HistoryCache::new(10);
        a.insert(alive(1)).unwrap();
        let b = a.clone();
        assert!(!Arc::ptr_eq(&a.stats(), &b.stats()));
        assert_eq!(a.stats().snapshot().len, 1);
        assert_eq!(b.stats().snapshot().len, 1);

        let mut a_mut = a;
        a_mut.insert(alive(2)).unwrap();
        assert_eq!(a_mut.stats().snapshot().len, 2);
        assert_eq!(b.stats().snapshot().len, 1, "clone unaffected");
    }

    // ========================================================================
    // D.4 Phase C — LockFreeReadHistoryCache Tests
    // ========================================================================

    #[cfg(feature = "std")]
    mod lock_free_tests {
        use super::*;

        #[test]
        fn lock_free_new_is_empty() {
            let c = LockFreeReadHistoryCache::new(10);
            assert_eq!(c.len(), 0);
            assert!(c.is_empty());
            assert_eq!(c.min_sn(), None);
            assert_eq!(c.max_sn(), None);
        }

        #[test]
        fn lock_free_insert_and_get() {
            let c = LockFreeReadHistoryCache::new(10);
            // Insert without &mut self — pure interior mutability.
            c.insert(alive(1)).unwrap();
            c.insert(alive(2)).unwrap();
            assert_eq!(
                c.get(sn(1)).map(|ch| ch.payload.as_ref().to_vec()),
                Some(alloc::vec![1])
            );
            assert_eq!(c.get(sn(3)), None);
            assert_eq!(c.len(), 2);
        }

        #[test]
        fn lock_free_min_max_lock_free_loads() {
            let c = LockFreeReadHistoryCache::new(10);
            c.insert(alive(5)).unwrap();
            c.insert(alive(3)).unwrap();
            c.insert(alive(7)).unwrap();
            assert_eq!(c.min_sn(), Some(sn(3)));
            assert_eq!(c.max_sn(), Some(sn(7)));
        }

        #[test]
        fn lock_free_keeplast_evicts_oldest() {
            let c =
                LockFreeReadHistoryCache::new_with_kind(HistoryKind::KeepLast { depth: 2 }, 100);
            c.insert(alive(1)).unwrap();
            c.insert(alive(2)).unwrap();
            c.insert(alive(3)).unwrap(); // evicts 1
            assert_eq!(c.len(), 2);
            assert_eq!(c.min_sn(), Some(sn(2)));
            assert_eq!(c.max_sn(), Some(sn(3)));
            assert_eq!(c.evicted_count(), 1);
        }

        #[test]
        fn lock_free_keepall_full_rejects() {
            let c = LockFreeReadHistoryCache::new(2);
            c.insert(alive(1)).unwrap();
            c.insert(alive(2)).unwrap();
            assert_eq!(c.insert(alive(3)), Err(CacheError::CapacityExceeded));
        }

        #[test]
        fn lock_free_duplicate_sn_rejected() {
            let c = LockFreeReadHistoryCache::new(10);
            c.insert(alive(1)).unwrap();
            assert_eq!(c.insert(alive(1)), Err(CacheError::DuplicateSequenceNumber));
        }

        #[test]
        fn lock_free_remove_up_to() {
            let c = LockFreeReadHistoryCache::new(10);
            for i in 1..=5 {
                c.insert(alive(i)).unwrap();
            }
            let removed = c.remove_up_to(sn(3));
            assert_eq!(removed, 3);
            assert_eq!(c.len(), 2);
            assert_eq!(c.min_sn(), Some(sn(4)));
        }

        #[test]
        fn lock_free_iter_range_snapshot() {
            let c = LockFreeReadHistoryCache::new(10);
            for i in 1..=5 {
                c.insert(alive(i)).unwrap();
            }
            let mid: alloc::vec::Vec<_> = c
                .iter_range_snapshot(sn(2), sn(4))
                .iter()
                .map(|ch| ch.sequence_number)
                .collect();
            assert_eq!(mid, alloc::vec![sn(2), sn(3), sn(4)]);
        }

        #[test]
        fn lock_free_snapshot_outlives_writes() {
            // Snapshot API guarantee: a reader Arc stays unchanged,
            // even if the writer mutates the cache later.
            let c = LockFreeReadHistoryCache::new(10);
            c.insert(alive(1)).unwrap();
            let snap = c.snapshot();
            assert_eq!(snap.changes.len(), 1);

            c.insert(alive(2)).unwrap();
            c.insert(alive(3)).unwrap();
            // Original snapshot: still only SN 1.
            assert_eq!(snap.changes.len(), 1);
            assert!(snap.changes.contains_key(&sn(1)));
            // Live cache: 3 entries.
            assert_eq!(c.len(), 3);
        }

        #[test]
        fn lock_free_concurrent_readers_writers_smoke() {
            use std::sync::Arc as StdArc;
            use std::thread;

            let cache: StdArc<LockFreeReadHistoryCache> =
                StdArc::new(LockFreeReadHistoryCache::new(2_000));
            let cache_w = StdArc::clone(&cache);
            let writer = thread::spawn(move || {
                for i in 1..=500 {
                    cache_w.insert(alive(i)).expect("insert");
                }
            });

            let cache_r = StdArc::clone(&cache);
            let reader = thread::spawn(move || {
                for _ in 0..200 {
                    let snap = cache_r.snapshot();
                    // The snapshot is internally consistent: changes.len matches
                    // the range between min and max.
                    if let (Some(min), Some(max)) = (
                        snap.changes.keys().next().copied(),
                        snap.changes.keys().next_back().copied(),
                    ) {
                        let inferred_count = (max.0 - min.0 + 1) as usize;
                        assert!(
                            snap.changes.len() <= inferred_count,
                            "snapshot inkonsistent"
                        );
                    }
                }
            });

            writer.join().expect("writer joined");
            reader.join().expect("reader joined");

            assert_eq!(cache.len(), 500);
            assert_eq!(cache.max_sn(), Some(sn(500)));
        }
    }
}
