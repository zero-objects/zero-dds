// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! `InstanceTracker` — the central bookkeeping for keyed topic
//! instances, on both the writer and the reader side.
//!
//! Spec reference: OMG DDS-DCPS 1.4 §2.2.2.4.2.5 (`register_instance`),
//! §2.2.2.4.2.7 (`unregister_instance`), §2.2.2.4.2.10 (`dispose`),
//! §2.2.2.4.2.13 (`get_key_value`), §2.2.2.4.2.14 (`lookup_instance`),
//! §2.2.2.5.1 (`InstanceStateKind`).
//!
//! # Data model
//!
//! We index instances by the **16-byte KeyHash** (XTypes 1.3 §7.6.8).
//! Per instance we keep:
//! * the assigned [`InstanceHandle`],
//! * the lifecycle state ([`InstanceStateKind`]),
//! * the generation counters (`disposed`, `no_writers`),
//! * the number of still-registered writers (reader side),
//! * the last observed sample timestamp.
//!
//! KeyHash → handle is a 1:1 map for the lifetime of the tracker
//! (handles are not recycled, even once the instance is disposed +
//! purged; the spec explicitly allows recycling, but we avoid it for
//! test stability).

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::sync::Arc;

#[cfg(feature = "std")]
use std::sync::Mutex;

use crate::key_hash::KEY_HASH_LEN;

use crate::instance_handle::{InstanceHandle, InstanceHandleAllocator};
use crate::sample_info::InstanceStateKind;
use crate::time::Time;

/// 16-byte KeyHash used as index.
pub type KeyHash = [u8; KEY_HASH_LEN];

/// Per-instance bookkeeping.
#[derive(Debug, Clone)]
pub struct InstanceState {
    /// Local handle (stable across the tracker lifecycle).
    pub handle: InstanceHandle,
    /// Current lifecycle state.
    pub kind: InstanceStateKind,
    /// `NOT_ALIVE_DISPOSED → ALIVE` transitions since the first sample.
    pub disposed_generation_count: i32,
    /// `NOT_ALIVE_NO_WRITERS → ALIVE` transitions since the first sample.
    pub no_writers_generation_count: i32,
    /// Number of writers that currently consider this instance
    /// registered. Reader side: each incoming sample increments the
    /// counter (if it is a new writer); each unregister marker
    /// decrements it. Writer side: 0 or 1 (a single writer's view).
    pub writer_count: u32,
    /// Wall-clock time of the last processed sample (or None).
    pub last_sample_timestamp: Option<Time>,
    /// Reader side: the DESTINATION_ORDER time (§2.2.3.17) of this
    /// instance's last sample delivered to the user API: its source
    /// timestamp under BY_SOURCE_TIMESTAMP, its reception time otherwise.
    pub last_delivered_ts: Option<Time>,
    /// Reader side: the TIME_BASED_FILTER time (§2.2.3.12, on the clock
    /// `reader.time_based_filter_clock` names) of this instance's last
    /// sample delivered to the user API: a new sample is filtered out when
    /// `t - last_filter_ts < minimum_separation`. It shared
    /// `last_delivered_ts`, so under BY_SOURCE_TIMESTAMP the filter
    /// compared the reception time against a source timestamp.
    pub last_filter_ts: Option<Time>,
    /// Reader side: wall-clock time at which the instance transitioned
    /// into `NOT_ALIVE_DISPOSED`. Spec §2.2.3.22
    /// `autopurge_disposed_samples_delay`: samples are purged after the
    /// delay elapses.
    pub disposed_at: Option<Time>,
    /// Reader side: wall-clock time at which the instance transitioned
    /// into `NOT_ALIVE_NO_WRITERS`. Spec §2.2.3.22
    /// `autopurge_no_writer_samples_delay`.
    pub no_writers_at: Option<Time>,
    /// Reader side OWNERSHIP=EXCLUSIVE (Spec §2.2.3.10): current owner
    /// writer as (GuidLike, Strength). On a tie, the lexicographically
    /// lower `GuidLike` wins. On liveliness loss: reset explicitly via
    /// `clear_owner`.
    pub current_owner: Option<([u8; 16], i32)>,
    /// Reader side: when the current owner's last sample of the instance
    /// was accepted, for its DEADLINE (§2.2.3.9.2 (d)).
    pub owner_at: Option<Time>,
    /// Stored key holder (bytes), so `get_key_value` can replay the key
    /// without re-decoding. This is the **PLAIN_CDR2-BE** stream, i.e.
    /// exactly the input to `compute_key_hash`.
    /// `None` means only the hash is known; `Some(Vec::new())` is a known
    /// empty holder, not an unknown sentinel.
    pub key_holder: Option<alloc::vec::Vec<u8>>,
    /// Reader side: GUIDs of the matched DataWriters that have written this
    /// instance. DDS 1.4 §2.2.2.5.1: `NOT_ALIVE_NO_WRITERS_INSTANCE_STATE`
    /// "indicates the instance has been declared as not-alive by the
    /// DataReader because it detected that there are no live DataWriter
    /// objects writing that instance" — so the instance state follows this
    /// set becoming empty, not a global match count. Unused on the writer
    /// side.
    pub writers: BTreeSet<[u8; 16]>,
    /// View state on the reader side. Unused on the writer side.
    pub reader_view_new: bool,
    /// Number of reader samples so far in this instance (a sample_rank
    /// helper value for the next read).
    pub samples_in_cache: u32,
}

impl InstanceState {
    fn fresh(handle: InstanceHandle, key_holder: Option<alloc::vec::Vec<u8>>) -> Self {
        Self {
            handle,
            kind: InstanceStateKind::Alive,
            disposed_generation_count: 0,
            no_writers_generation_count: 0,
            writer_count: 0,
            last_sample_timestamp: None,
            last_delivered_ts: None,
            last_filter_ts: None,
            disposed_at: None,
            no_writers_at: None,
            current_owner: None,
            owner_at: None,
            key_holder,
            writers: BTreeSet::new(),
            reader_view_new: true,
            samples_in_cache: 0,
        }
    }
}

/// Thread-safe tracker — instantiated in both the DataWriter and the
/// DataReader.
#[derive(Debug)]
pub struct InstanceTracker {
    inner: Arc<Mutex<TrackerInner>>,
    allocator: Arc<InstanceHandleAllocator>,
    /// The handle of an instance is `InstanceHandle::from_guid` of its key
    /// hash, which for a built-in topic is the entity's GUID
    /// ([`Self::with_key_handles`]); otherwise the allocator numbers them.
    key_handles: bool,
}

#[derive(Debug, Default)]
struct TrackerInner {
    by_keyhash: BTreeMap<KeyHash, InstanceState>,
    handle_to_keyhash: BTreeMap<InstanceHandle, KeyHash>,
}

/// Pure §2.2.3.12 TIME_BASED_FILTER admission math: `false` if
/// `candidate_ts - last < min_separation_nanos` (drop), `true` otherwise.
/// Factored out of [`InstanceTracker::should_deliver_under_time_based_filter`]
/// so a caller that must not mutate the tracker (a non-consuming `read()`)
/// can run the identical decision against its own scan-local baseline.
#[must_use]
pub(crate) fn time_based_filter_admits(
    last: Time,
    candidate_ts: Time,
    min_separation_nanos: u128,
) -> bool {
    // Nanosecond difference; for a sample in the past (clock skew), never
    // filter — the caller has a reorder problem, not a filter problem.
    let last_nanos =
        u128::from(u64::try_from(last.sec).unwrap_or(0)) * 1_000_000_000 + u128::from(last.nanosec);
    let candidate_nanos = u128::from(u64::try_from(candidate_ts.sec).unwrap_or(0)) * 1_000_000_000
        + u128::from(candidate_ts.nanosec);
    if candidate_nanos < last_nanos {
        return true;
    }
    candidate_nanos - last_nanos >= min_separation_nanos
}

/// Pure §2.2.3.18 DESTINATION_ORDER (`BY_SOURCE_TIMESTAMP`) admission math:
/// `true` only if `candidate_ts` is strictly greater than `last`. Factored
/// out for the same reason as [`time_based_filter_admits`].
#[must_use]
pub(crate) fn destination_order_admits(last: Time, candidate_ts: Time) -> bool {
    let last_nanos =
        u128::from(u64::try_from(last.sec).unwrap_or(0)) * 1_000_000_000 + u128::from(last.nanosec);
    let candidate_nanos = u128::from(u64::try_from(candidate_ts.sec).unwrap_or(0)) * 1_000_000_000
        + u128::from(candidate_ts.nanosec);
    candidate_nanos > last_nanos
}

impl Default for InstanceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl InstanceTracker {
    /// New tracker with its own [`InstanceHandleAllocator`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(TrackerInner::default())),
            allocator: Arc::new(InstanceHandleAllocator::new()),
            key_handles: false,
        }
    }

    /// Tracker of a built-in topic's reader: an instance's handle is
    /// `InstanceHandle::from_guid` of its key, the GUID of the entity the
    /// sample describes. DDS 1.4 §2.2.2.2.1.14: the handle `ignore_participant`
    /// takes "is the one that appears in the SampleInfo retrieved when
    /// reading the data-samples available for the built-in DataReader";
    /// §2.2.2.4.2.24 `get_matched_subscriptions`: "These handles match the
    /// ones that appear in the 'instance_handle' field of the SampleInfo".
    /// The built-in readers numbered their instances 1, 2, …, which matched
    /// none of those.
    #[must_use]
    pub fn with_key_handles() -> Self {
        Self {
            key_handles: true,
            ..Self::new()
        }
    }

    /// The handle of a new instance of `keyhash`.
    fn new_handle(&self, keyhash: &KeyHash) -> InstanceHandle {
        if self.key_handles {
            InstanceHandle::from_guid(zerodds_rtps::wire_types::Guid::from_bytes(*keyhash))
        } else {
            self.allocator.allocate()
        }
    }

    /// Tracker with a shared allocator (e.g. when a writer and reader in
    /// the same participant draw their handles from the same pool).
    #[must_use]
    pub fn with_allocator(allocator: Arc<InstanceHandleAllocator>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TrackerInner::default())),
            allocator,
            key_handles: false,
        }
    }

    /// Registers the instance if it is not yet known; otherwise just
    /// reactivates it (Spec §2.2.2.4.2.5).
    ///
    /// Always returns the (stable) [`InstanceHandle`].
    pub fn register(
        &self,
        keyhash: KeyHash,
        key_holder: alloc::vec::Vec<u8>,
        timestamp: Option<Time>,
    ) -> InstanceHandle {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let entry = g.by_keyhash.entry(keyhash).or_insert_with(|| {
            let h = self.new_handle(&keyhash);
            InstanceState::fresh(h, Some(key_holder.clone()))
        });
        entry.key_holder = Some(key_holder);
        // Reactivation from NOT_ALIVE → ALIVE produces a generation
        // bump (Spec §2.2.2.5.1.7/8).
        match entry.kind {
            InstanceStateKind::NotAliveDisposed => {
                entry.disposed_generation_count = entry.disposed_generation_count.saturating_add(1);
                entry.kind = InstanceStateKind::Alive;
            }
            InstanceStateKind::NotAliveNoWriters => {
                entry.no_writers_generation_count =
                    entry.no_writers_generation_count.saturating_add(1);
                entry.kind = InstanceStateKind::Alive;
            }
            InstanceStateKind::Alive => {}
        }
        entry.writer_count = entry.writer_count.saturating_add(1);
        if let Some(ts) = timestamp {
            entry.last_sample_timestamp = Some(ts);
        }
        let handle = entry.handle;
        g.handle_to_keyhash.insert(handle, keyhash);
        handle
    }

    /// Spec §2.2.3.12 TIME_BASED_FILTER — decides whether a sample with
    /// `sample_ts` may be delivered to the user API. `false` if
    /// `t - last_delivered_ts < min_separation` (drop); `true` otherwise
    /// (deliver).
    ///
    /// For an unknown instance or the first sample (no
    /// `last_delivered_ts`) it is always `true`.
    #[must_use]
    pub fn should_deliver_under_time_based_filter(
        &self,
        keyhash: &KeyHash,
        sample_ts: Time,
        min_separation_nanos: u128,
    ) -> bool {
        if min_separation_nanos == 0 {
            return true;
        }
        let last = {
            let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            g.by_keyhash.get(keyhash).and_then(|s| s.last_filter_ts)
        };
        let Some(last) = last else {
            return true;
        };
        time_based_filter_admits(last, sample_ts, min_separation_nanos)
    }

    /// Spec §2.2.3.18 DESTINATION_ORDER — decides whether a sample with
    /// `source_ts` may be delivered to the user API.
    /// `BY_RECEPTION_TIMESTAMP`: always `true`. `BY_SOURCE_TIMESTAMP`:
    /// only if `source_ts` is strictly greater than this instance's
    /// `last_delivered_ts` (the tie-break on equal timestamps via the
    /// writer GUID happens in the typed path).
    #[must_use]
    pub fn should_deliver_under_destination_order(
        &self,
        keyhash: &KeyHash,
        source_ts: Time,
        by_source_timestamp: bool,
    ) -> bool {
        if !by_source_timestamp {
            return true;
        }
        let Some(last) = self.last_delivered_ts(keyhash) else {
            return true;
        };
        destination_order_admits(last, source_ts)
    }

    /// Read-only lookup of this instance's `last_delivered_ts` (the
    /// baseline [`Self::should_deliver_under_time_based_filter`] /
    /// [`Self::should_deliver_under_destination_order`] compare against),
    /// `None` for an unknown instance or one that never had a delivery
    /// recorded. Lets a caller replicate the same admission decision
    /// against a scan-local baseline without mutating this tracker — a
    /// non-consuming read must not move the state a consuming take() would
    /// later read.
    #[must_use]
    pub fn last_delivered_ts(&self, keyhash: &KeyHash) -> Option<Time> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.by_keyhash.get(keyhash).and_then(|s| s.last_delivered_ts)
    }

    /// Marks that a sample with `sample_ts` was delivered to the user
    /// API, on both the TIME_BASED_FILTER and the DESTINATION_ORDER time
    /// (Spec §2.2.3.12, §2.2.3.17 — for the next decision).
    pub fn record_delivery(&self, keyhash: &KeyHash, sample_ts: Time) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = g.by_keyhash.get_mut(keyhash) {
            s.last_delivered_ts = Some(sample_ts);
            s.last_filter_ts = Some(sample_ts);
        }
    }

    /// Ensures a full-key identity exists without incrementing a writer count
    /// on repeated writes. Admission and the finite instance limit are atomic.
    /// A negative limit means unlimited, as in DDS ResourceLimits.
    pub fn ensure_registered_limited(
        &self,
        keyhash: KeyHash,
        max_instances: i32,
    ) -> Option<InstanceHandle> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(entry) = g.by_keyhash.get_mut(&keyhash) {
            match entry.kind {
                InstanceStateKind::NotAliveDisposed => {
                    entry.disposed_generation_count =
                        entry.disposed_generation_count.saturating_add(1);
                    entry.reader_view_new = true;
                }
                InstanceStateKind::NotAliveNoWriters => {
                    entry.no_writers_generation_count =
                        entry.no_writers_generation_count.saturating_add(1);
                    entry.reader_view_new = true;
                }
                InstanceStateKind::Alive => {}
            }
            entry.writer_count = entry.writer_count.max(1);
            entry.kind = InstanceStateKind::Alive;
            return Some(entry.handle);
        }
        if max_instances >= 0 && g.by_keyhash.len() >= max_instances as usize {
            return None;
        }
        let handle = self.new_handle(&keyhash);
        let mut state = InstanceState::fresh(handle, None);
        state.writer_count = 1;
        g.by_keyhash.insert(keyhash, state);
        g.handle_to_keyhash.insert(handle, keyhash);
        Some(handle)
    }

    /// The complete key for a known opaque handle, never reconstructed bytes.
    #[must_use]
    pub fn key_hash_for_handle(&self, handle: InstanceHandle) -> Option<KeyHash> {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .handle_to_keyhash
            .get(&handle)
            .copied()
    }

    /// Lookup without mutation (Spec §2.2.2.4.2.14 `lookup_instance`).
    #[must_use]
    pub fn lookup(&self, keyhash: &KeyHash) -> Option<InstanceHandle> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.by_keyhash.get(keyhash).map(|s| s.handle)
    }

    /// `read` applied to the state of `handle`, under the tracker's lock
    /// and without copying it: [`Self::get_by_handle`] clones the key holder
    /// and the writer set, two allocations, and a reader asked twice per
    /// sample for three numbers.
    pub fn read_by_handle<R>(
        &self,
        handle: InstanceHandle,
        read: impl FnOnce(&InstanceState) -> R,
    ) -> Option<R> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let kh = g.handle_to_keyhash.get(&handle)?;
        g.by_keyhash.get(kh).map(read)
    }

    /// Returns a copy of the state snapshot for a handle.
    #[must_use]
    pub fn get_by_handle(&self, handle: InstanceHandle) -> Option<InstanceState> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let kh = g.handle_to_keyhash.get(&handle)?;
        g.by_keyhash.get(kh).cloned()
    }

    /// The instance state and whether the reader's view of it is NEW, for
    /// each handle in `handles`, under one lock; an unknown handle is left
    /// out. What a read or take puts in the SampleInfo of every sample of an
    /// instance (DDS 1.4 §2.2.2.5.1.3, §2.2.2.5.1.8).
    pub(crate) fn current_states(
        &self,
        handles: impl IntoIterator<Item = InstanceHandle>,
    ) -> alloc::collections::BTreeMap<InstanceHandle, (InstanceStateKind, bool)> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = alloc::collections::BTreeMap::new();
        for handle in handles {
            if out.contains_key(&handle) {
                continue;
            }
            if let Some(s) = g
                .handle_to_keyhash
                .get(&handle)
                .and_then(|kh| g.by_keyhash.get(kh))
            {
                out.insert(handle, (s.kind, s.reader_view_new));
            }
        }
        out
    }

    /// Returns a copy of the state snapshot for a KeyHash.
    #[must_use]
    pub fn get_by_keyhash(&self, keyhash: &KeyHash) -> Option<InstanceState> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.by_keyhash.get(keyhash).cloned()
    }

    /// The handle and the instance state of a key, under one lock and
    /// without copying the rest of the state (its key holder above all).
    #[must_use]
    pub fn handle_and_kind(
        &self,
        keyhash: &KeyHash,
    ) -> Option<(InstanceHandle, InstanceStateKind)> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.by_keyhash.get(keyhash).map(|s| (s.handle, s.kind))
    }

    /// A second handle on the same instances, for what reads them apart
    /// from the endpoint that owns the tracker (a ReadCondition's trigger).
    pub(crate) fn shared(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            allocator: Arc::clone(&self.allocator),
            key_handles: self.key_handles,
        }
    }

    /// Returns the key-holder byte stream for a handle (Spec
    /// §2.2.2.4.2.13 `get_key_value`).
    #[must_use]
    pub fn get_key_holder(&self, handle: InstanceHandle) -> Option<alloc::vec::Vec<u8>> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let kh = g.handle_to_keyhash.get(&handle)?;
        g.by_keyhash.get(kh).and_then(|s| s.key_holder.clone())
    }

    /// Marks the instance as `NOT_ALIVE_DISPOSED` (Spec §2.2.2.4.2.10
    /// `dispose`).
    ///
    /// Returns `false` if the instance is not known.
    pub fn dispose(&self, handle: InstanceHandle, timestamp: Option<Time>) -> bool {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(kh) = g.handle_to_keyhash.get(&handle).copied() else {
            return false;
        };
        if let Some(s) = g.by_keyhash.get_mut(&kh) {
            s.kind = InstanceStateKind::NotAliveDisposed;
            if let Some(ts) = timestamp {
                s.last_sample_timestamp = Some(ts);
                s.disposed_at = Some(ts);
            }
            return true;
        }
        false
    }

    /// Decrements the writer counter; if it drops to 0, the instance
    /// transitions to `NOT_ALIVE_NO_WRITERS` (Spec §2.2.2.4.2.7).
    pub fn unregister(&self, handle: InstanceHandle, timestamp: Option<Time>) -> bool {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(kh) = g.handle_to_keyhash.get(&handle).copied() else {
            return false;
        };
        if let Some(s) = g.by_keyhash.get_mut(&kh) {
            s.writer_count = s.writer_count.saturating_sub(1);
            if s.writer_count == 0 && !matches!(s.kind, InstanceStateKind::NotAliveDisposed) {
                s.kind = InstanceStateKind::NotAliveNoWriters;
                if let Some(ts) = timestamp {
                    s.no_writers_at = Some(ts);
                }
            }
            if let Some(ts) = timestamp {
                s.last_sample_timestamp = Some(ts);
            }
            return true;
        }
        false
    }

    /// Reader side: `writer` unregistered the instance `handle`. It leaves
    /// the set of writers of that instance, and the instance goes
    /// NOT_ALIVE_NO_WRITERS once none is left: DDS 1.4 §2.2.2.5.1.3, "there
    /// are no live DataWriter entities writing that instance". A disposed
    /// instance stays NOT_ALIVE_DISPOSED (§2.2.3.22). The count of
    /// [`Self::unregister`] took every unregister for the last one, also
    /// while another writer still wrote the instance.
    ///
    /// Returns `false` if the instance is not known.
    pub fn unregister_writer(
        &self,
        handle: InstanceHandle,
        writer: [u8; 16],
        timestamp: Option<Time>,
    ) -> bool {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(kh) = g.handle_to_keyhash.get(&handle).copied() else {
            return false;
        };
        let Some(s) = g.by_keyhash.get_mut(&kh) else {
            return false;
        };
        s.writers.remove(&writer);
        if s.writers.is_empty() && matches!(s.kind, InstanceStateKind::Alive) {
            s.kind = InstanceStateKind::NotAliveNoWriters;
            if let Some(ts) = timestamp {
                s.no_writers_at = Some(ts);
            }
        }
        if let Some(ts) = timestamp {
            s.last_sample_timestamp = Some(ts);
        }
        true
    }

    /// Reader side: records that `writer` has written the instance
    /// `handle`. DDS 1.4 §2.2.2.5.1 ties `NOT_ALIVE_NO_WRITERS` to the set
    /// of DataWriters writing *that instance*, so the set is kept per
    /// instance rather than as a single match count.
    pub fn note_writer(&self, handle: InstanceHandle, writer: [u8; 16]) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(kh) = g.handle_to_keyhash.get(&handle).copied() else {
            return;
        };
        if let Some(s) = g.by_keyhash.get_mut(&kh) {
            s.writers.insert(writer);
        }
    }

    /// Reader side: `writer` is gone (its SEDP dispose arrived, its
    /// participant left, or its liveliness lease expired). Removes it from
    /// every instance it was writing and returns the handles of those
    /// instances that now have no writer left at all — DDS 1.4 §2.2.2.5.1:
    /// the DataReader declares such an instance
    /// `NOT_ALIVE_NO_WRITERS_INSTANCE_STATE`.
    ///
    /// An instance already in `NOT_ALIVE_DISPOSED` keeps that state (the
    /// same rule [`Self::unregister`] applies): a disposed instance was
    /// declared not-alive by a DataWriter, and losing the writer does not
    /// undo that declaration.
    pub fn drop_writer(&self, writer: &[u8; 16], timestamp: Option<Time>) -> Vec<InstanceHandle> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut now_writerless = Vec::new();
        for s in g.by_keyhash.values_mut() {
            if !s.writers.remove(writer) {
                continue;
            }
            if !s.writers.is_empty() {
                continue;
            }
            if matches!(s.kind, InstanceStateKind::Alive) {
                s.kind = InstanceStateKind::NotAliveNoWriters;
                if let Some(ts) = timestamp {
                    s.no_writers_at = Some(ts);
                    s.last_sample_timestamp = Some(ts);
                }
                now_writerless.push(s.handle);
            }
        }
        now_writerless
    }

    /// Whether [`Self::drop_writer`] for every writer in `gone` would turn
    /// an ALIVE instance whose view state is in `view_states` NOT_ALIVE_NO_WRITERS
    /// — the instances that then get a marker sample (DDS 1.4
    /// §2.2.2.5.1.3). Changes nothing: a status probe or a listener asks
    /// what the next read or take will find (§2.2.4.2.2).
    pub fn would_leave_writerless(&self, gone: &[[u8; 16]], view_states: u32) -> bool {
        !self.left_writerless(gone, view_states).is_empty()
    }

    /// The handles of the instances [`Self::would_leave_writerless`] finds.
    pub fn left_writerless(&self, gone: &[[u8; 16]], view_states: u32) -> Vec<InstanceHandle> {
        use crate::sample_info::view_state_mask;
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.by_keyhash
            .values()
            .filter(|s| {
                let view = if s.reader_view_new {
                    view_state_mask::NEW
                } else {
                    view_state_mask::NOT_NEW
                };
                matches!(s.kind, InstanceStateKind::Alive)
                    && view_states & view != 0
                    && !s.writers.is_empty()
                    && s.writers.iter().all(|w| gone.contains(w))
            })
            .map(|s| s.handle)
            .collect()
    }

    /// Spec §2.2.3.10 OWNERSHIP=EXCLUSIVE strength selection.
    /// Returns `true` if a sample from the writer with
    /// `(writer_guid, writer_strength)` should be accepted for the
    /// instance `keyhash`.
    ///
    /// Algorithm (Spec §2.2.3.10):
    /// - No current owner → accept + set as owner.
    /// - Strength > current → accept + replace owner.
    /// - Strength == current and guid < current_guid → accept + replace
    ///   owner.
    /// - Strength < current → reject.
    /// - Strength == current and guid > current → reject.
    /// - The owner's own sample → accept.
    ///
    /// On a tie the lower GUID wins. DDS 1.4 §2.2.3.9.2 leaves the rule
    /// open but requires "that all DataReader objects will make the same
    /// choice"; Cyclone DDS takes the lower GUID
    /// (`inst_accepts_sample_by_writer_guid`, `dds_rhc_default.c`, which
    /// names RTI's model as the same), and so does `zerodds-qos`'s
    /// `resolve_strongest`. The DataReader took the higher one.
    pub fn should_accept_sample_under_exclusive_ownership(
        &self,
        keyhash: &KeyHash,
        writer_guid: [u8; 16],
        writer_strength: i32,
    ) -> bool {
        self.accept_under_exclusive_ownership(
            keyhash,
            writer_guid,
            writer_strength,
            Time::ZERO,
            None,
        )
    }

    /// [`Self::should_accept_sample_under_exclusive_ownership`] for a sample
    /// accepted at `now`, with the reader's DEADLINE period in nanoseconds
    /// (`None`: INFINITE). DDS 1.4 §2.2.3.9.2: "Ownership can therefore
    /// change as a result of (a) a DataWriter in the system with a higher
    /// value of the strength that modifies the instance, (b) a change in
    /// the strength value of the DataWriter that owns the instance, (c) a
    /// change in the liveliness of the DataWriter that owns the instance,
    /// and (d) a deadline with which the DataWriter that owns the instance
    /// has committed is missed."
    ///
    /// The owner's own sample is accepted whatever strength it carries, and
    /// its strength is the one it carries (b): one that lowered its strength
    /// was rejected by itself. An owner whose last sample is a deadline
    /// period or more before `now` gives the instance up to the writer of
    /// this sample (d).
    pub fn accept_under_exclusive_ownership(
        &self,
        keyhash: &KeyHash,
        writer_guid: [u8; 16],
        writer_strength: i32,
        now: Time,
        deadline_nanos: Option<u128>,
    ) -> bool {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = g.by_keyhash.get_mut(keyhash) else {
            return true; // unknown instance → accept
        };
        let take_over = match s.current_owner {
            None => true,
            Some((cur_guid, _)) if cur_guid == writer_guid => true,
            Some((cur_guid, cur_str)) => {
                let missed = deadline_nanos.is_some_and(|period| {
                    s.owner_at.is_some_and(|at| {
                        time_nanos(now) - time_nanos(at)
                            >= i128::try_from(period).unwrap_or(i128::MAX)
                    })
                });
                missed
                    || writer_strength > cur_str
                    || (writer_strength == cur_str && writer_guid < cur_guid)
            }
        };
        if take_over {
            s.current_owner = Some((writer_guid, writer_strength));
            s.owner_at = Some(now);
        }
        take_over
    }

    /// The writer `writer` announced the strength `strength` (§2.2.3.9.2
    /// (b)): the instances it owns are held at that strength, so a writer
    /// stronger than it now takes them over with its next sample.
    pub fn update_owner_strength(&self, writer: [u8; 16], strength: i32) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        for s in g.by_keyhash.values_mut() {
            if let Some((owner, held)) = s.current_owner.as_mut() {
                if *owner == writer {
                    *held = strength;
                }
            }
        }
    }

    /// Spec §2.2.3.23 — on liveliness loss of a writer: clear the owner
    /// for all instances whose owner was this writer. The next sample
    /// triggers failover selection.
    pub fn clear_owner_for_writer(&self, writer_guid: [u8; 16]) -> usize {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut cleared = 0;
        for s in g.by_keyhash.values_mut() {
            if let Some((g_, _)) = s.current_owner {
                if g_ == writer_guid {
                    s.current_owner = None;
                    cleared += 1;
                }
            }
        }
        cleared
    }

    /// The writer that owns the EXCLUSIVE instance `keyhash`, if one does.
    #[must_use]
    pub fn owner_of(&self, keyhash: &KeyHash) -> Option<[u8; 16]> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.by_keyhash
            .get(keyhash)
            .and_then(|s| s.current_owner.map(|(writer, _)| writer))
    }

    /// DDS 1.4 §2.2.3.23.3: "if the current owner of an instance
    /// unregisters it, then it will relinquish ownership". Clears the owner
    /// of `keyhash` when it is `writer`; `true` when it was.
    pub fn release_owner_of(&self, keyhash: &KeyHash, writer: [u8; 16]) -> bool {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = g.by_keyhash.get_mut(keyhash) else {
            return false;
        };
        if s.current_owner.is_some_and(|(owner, _)| owner == writer) {
            s.current_owner = None;
            return true;
        }
        false
    }

    /// Like [`Self::clear_owner_for_writer`], but matches on the first
    /// 12 bytes of the GUID (GuidPrefix). Allows failover when only the
    /// participant identity (e.g. via SPDP lease expiry) is known.
    pub fn clear_owner_for_writer_prefix(&self, prefix: [u8; 12]) -> usize {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut cleared = 0;
        for s in g.by_keyhash.values_mut() {
            if let Some((g_, _)) = s.current_owner {
                if g_[..12] == prefix {
                    s.current_owner = None;
                    cleared += 1;
                }
            }
        }
        cleared
    }

    /// Spec §2.2.3.22 READER_DATA_LIFECYCLE — purges instances whose
    /// disposed/no-writer marker is older than the respective delay.
    /// `now` is the caller-side wall-clock; the delays are in
    /// nanoseconds. Returns the number of removed instances.
    ///
    /// Lazy purge: called from the read path (or a background tick). The
    /// spec leaves the strategy open — we delete the affected instance
    /// entirely, so that subsequent read/take no longer see it.
    pub fn autopurge(
        &self,
        now: Time,
        autopurge_disposed_delay_nanos: u128,
        autopurge_nowriter_delay_nanos: u128,
    ) -> usize {
        let now_nanos = u128::from(u64::try_from(now.sec).unwrap_or(0)) * 1_000_000_000
            + u128::from(now.nanosec);
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut to_purge: alloc::vec::Vec<KeyHash> = alloc::vec::Vec::new();
        for (kh, s) in g.by_keyhash.iter() {
            let purge = match s.kind {
                InstanceStateKind::NotAliveDisposed
                    if autopurge_disposed_delay_nanos != u128::MAX =>
                {
                    s.disposed_at.is_some_and(|t| {
                        let t_nanos = u128::from(u64::try_from(t.sec).unwrap_or(0)) * 1_000_000_000
                            + u128::from(t.nanosec);
                        now_nanos.saturating_sub(t_nanos) >= autopurge_disposed_delay_nanos
                    })
                }
                InstanceStateKind::NotAliveNoWriters
                    if autopurge_nowriter_delay_nanos != u128::MAX =>
                {
                    s.no_writers_at.is_some_and(|t| {
                        let t_nanos = u128::from(u64::try_from(t.sec).unwrap_or(0)) * 1_000_000_000
                            + u128::from(t.nanosec);
                        now_nanos.saturating_sub(t_nanos) >= autopurge_nowriter_delay_nanos
                    })
                }
                _ => false,
            };
            if purge {
                to_purge.push(*kh);
            }
        }
        let count = to_purge.len();
        for kh in to_purge {
            if let Some(s) = g.by_keyhash.remove(&kh) {
                g.handle_to_keyhash.remove(&s.handle);
            }
        }
        count
    }

    /// Reader-side hook: marks that a sample has arrived for this
    /// instance. Returns `(handle, was_new)`, where `was_new == true`
    /// means this instance was previously unknown or the reader view was
    /// freshly reset (`view_state = NEW`).
    pub fn observe_sample(
        &self,
        keyhash: KeyHash,
        key_holder: alloc::vec::Vec<u8>,
        timestamp: Option<Time>,
    ) -> (InstanceHandle, bool) {
        self.observe_sample_with_holder(keyhash, Some(key_holder), timestamp)
    }

    /// Observes a hash-only lifecycle identity without inventing key-holder bytes.
    /// A later actual sample can fill the holder; absent bytes never erase it.
    pub(crate) fn observe_sample_with_holder(
        &self,
        keyhash: KeyHash,
        key_holder: Option<alloc::vec::Vec<u8>>,
        timestamp: Option<Time>,
    ) -> (InstanceHandle, bool) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut was_new = false;
        let entry = g.by_keyhash.entry(keyhash).or_insert_with(|| {
            was_new = true;
            let h = self.new_handle(&keyhash);
            InstanceState::fresh(h, key_holder.clone())
        });
        note_observed(entry, key_holder, timestamp);
        let handle = entry.handle;
        g.handle_to_keyhash.insert(handle, keyhash);
        (handle, was_new)
    }

    /// Reader side: the instance a dispose or unregister marker names, made
    /// known if it is not, with the marker counted in the cache. Unlike
    /// [`Self::observe_sample_with_holder`] it does not bring a not-alive
    /// instance back to ALIVE: only a data sample does that (DDS 1.4
    /// §2.2.2.5.1.3, "The instance_state becomes ALIVE as soon as any
    /// DataWriter writes the instance again"). An unregister of a disposed
    /// instance leaves it NOT_ALIVE_DISPOSED (§2.2.3.22: the last sample
    /// shows NOT_ALIVE_DISPOSED "depending on whether the last writer that
    /// had ownership of the instance disposed it"); passed through ALIVE,
    /// it ended NOT_ALIVE_NO_WRITERS with a disposed generation counted
    /// that never was.
    pub(crate) fn observe_lifecycle(
        &self,
        keyhash: KeyHash,
        key_holder: Option<alloc::vec::Vec<u8>>,
        timestamp: Option<Time>,
    ) -> InstanceHandle {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let entry = g
            .by_keyhash
            .entry(keyhash)
            .or_insert_with(|| InstanceState::fresh(self.new_handle(&keyhash), key_holder.clone()));
        if key_holder.is_some() {
            entry.key_holder = key_holder;
        }
        if let Some(ts) = timestamp {
            entry.last_sample_timestamp = Some(ts);
        }
        entry.samples_in_cache = entry.samples_in_cache.saturating_add(1);
        let handle = entry.handle;
        g.handle_to_keyhash.insert(handle, keyhash);
        handle
    }

    /// The admission of a received sample of `keyhash` under one lock:
    /// [`Self::observe_sample_with_holder`] for an instance the tracker
    /// holds or has room for, with the holder asked for only when the
    /// tracker holds none for `keyhash` yet, and the instance's `samples_in_cache` with this sample
    /// counted. `None`, and nothing observed, when `keyhash` is a new
    /// instance and the tracker holds `max_instances` already (a negative
    /// limit is none). A reader asked [`Self::lookup`], the tracker twice
    /// for the observation and [`Self::read_by_handle`] for this: four
    /// locks a sample. The same hash names the same holder, and a reader
    /// that keyed every sample built a holder per sample to replace the
    /// one there.
    pub(crate) fn observe_admitted(
        &self,
        keyhash: KeyHash,
        key_holder: impl FnOnce() -> alloc::vec::Vec<u8>,
        timestamp: Time,
        max_instances: i32,
    ) -> Option<(InstanceHandle, u32)> {
        {
            let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            match g.by_keyhash.get_mut(&keyhash) {
                Some(entry) if entry.key_holder.is_some() => {
                    note_observed(entry, None, Some(timestamp));
                    return Some((entry.handle, entry.samples_in_cache));
                }
                Some(_) => {}
                None => {
                    if max_instances >= 0 && g.by_keyhash.len() >= max_instances as usize {
                        return None;
                    }
                }
            }
        }
        // A new instance, or one known by its hash only: its holder is
        // built without the lock held.
        let holder = key_holder();
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let entry = g.by_keyhash.entry(keyhash).or_insert_with(|| {
            let h = self.new_handle(&keyhash);
            InstanceState::fresh(h, None)
        });
        note_observed(entry, Some(holder), Some(timestamp));
        let (handle, in_cache) = (entry.handle, entry.samples_in_cache);
        g.handle_to_keyhash.insert(handle, keyhash);
        Some((handle, in_cache))
    }

    /// The bookkeeping of a received sample of `keyhash` that admission
    /// let into the cache, under one lock: the DESTINATION_ORDER time
    /// `order_ts` and the TIME_BASED_FILTER time `filter_ts` of the
    /// delivery, [`Self::note_writer`] with `writer`, and the instance's
    /// view state, instance state and generation counts for its
    /// `SampleInfo`. `None` for an unknown instance.
    pub(crate) fn note_admitted(
        &self,
        keyhash: &KeyHash,
        order_ts: Time,
        filter_ts: Time,
        writer: [u8; 16],
    ) -> Option<(bool, InstanceStateKind, i32, i32)> {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let s = g.by_keyhash.get_mut(keyhash)?;
        s.last_delivered_ts = Some(order_ts);
        s.last_filter_ts = Some(filter_ts);
        if !s.writers.contains(&writer) {
            s.writers.insert(writer);
        }
        Some((
            s.reader_view_new,
            s.kind,
            s.disposed_generation_count,
            s.no_writers_generation_count,
        ))
    }

    /// [`Self::mark_view_seen`] and [`Self::drain_samples`] of `n` taken
    /// samples of `handle`, under one lock.
    pub(crate) fn note_taken(&self, handle: InstanceHandle, n: u32) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(kh) = g.handle_to_keyhash.get(&handle).copied() {
            if let Some(s) = g.by_keyhash.get_mut(&kh) {
                s.reader_view_new = false;
                s.samples_in_cache = s.samples_in_cache.saturating_sub(n);
            }
        }
    }
    /// Reader side: after the first `read`/`take` of an instance, its
    /// view state is set to `NOT_NEW`.
    pub fn mark_view_seen(&self, handle: InstanceHandle) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(kh) = g.handle_to_keyhash.get(&handle).copied() {
            if let Some(s) = g.by_keyhash.get_mut(&kh) {
                s.reader_view_new = false;
            }
        }
    }

    /// Reader side: the instance `keyhash` is NEW again for the next
    /// `read` or `take` (DDS 1.4 §2.2.2.3.4: the view_state of a MultiTopic
    /// instance is NEW while one of its constituents is). `false` if the
    /// instance is not known.
    pub fn mark_view_new(&self, keyhash: &KeyHash) -> bool {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        match g.by_keyhash.get_mut(keyhash) {
            Some(s) => {
                s.reader_view_new = true;
                true
            }
            None => false,
        }
    }

    /// Reader side: after `take`, `samples_in_cache` is reduced by `n`.
    pub fn drain_samples(&self, handle: InstanceHandle, n: u32) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(kh) = g.handle_to_keyhash.get(&handle).copied() {
            if let Some(s) = g.by_keyhash.get_mut(&kh) {
                s.samples_in_cache = s.samples_in_cache.saturating_sub(n);
            }
        }
    }

    /// Lists all instance handles in stable order (BTreeMap order by
    /// KeyHash). Spec §2.2.2.5.3.28 `read_next_instance`.
    #[must_use]
    pub fn ordered_handles(&self) -> alloc::vec::Vec<InstanceHandle> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.by_keyhash.values().map(|s| s.handle).collect()
    }

    /// Returns the **first** handle whose sort order lies strictly after
    /// `previous_handle` (or the very first one if
    /// `previous == HANDLE_NIL`). Spec §2.2.2.5.3.28.
    #[must_use]
    pub fn next_handle_after(&self, previous: InstanceHandle) -> Option<InstanceHandle> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if previous.is_nil() {
            return g.by_keyhash.values().next().map(|s| s.handle);
        }
        let prev_kh = g.handle_to_keyhash.get(&previous).copied()?;
        let range: (core::ops::Bound<KeyHash>, core::ops::Bound<KeyHash>) = (
            core::ops::Bound::Excluded(prev_kh),
            core::ops::Bound::Unbounded,
        );
        g.by_keyhash.range(range).next().map(|(_, s)| s.handle)
    }

    /// Number of tracked instances.
    #[must_use]
    pub fn len(&self) -> usize {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.by_keyhash.len()
    }

    /// `true` if no instances are tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drops all per-instance state (HISTORY/RESOURCE_LIMITS counters,
    /// view/sample state, exclusive-ownership winners, key holders).
    /// `Clone`s of this tracker (they share the same inner `Arc`) observe
    /// the reset too — used by builtin-topic readers on
    /// `DomainParticipant::delete_contained_entities` so a reader that has
    /// already delivered pre-delete samples does not carry their
    /// bookkeeping into the "clean" post-delete state the caller expects.
    /// The handle allocator is deliberately left untouched: handles stay
    /// monotonic, matching how a real DataReader's own numbering never
    /// rewinds.
    pub(crate) fn clear(&self) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        *g = TrackerInner::default();
    }
}

/// What observing a sample does to its instance's state: a holder it
/// brings replaces the one there, a not-alive instance is alive again with
/// its generation count raised, and the sample is counted in the cache.
fn note_observed(
    entry: &mut InstanceState,
    key_holder: Option<alloc::vec::Vec<u8>>,
    timestamp: Option<Time>,
) {
    if key_holder.is_some() {
        entry.key_holder = key_holder;
    }
    // Reactivate the reader view
    if matches!(
        entry.kind,
        InstanceStateKind::NotAliveDisposed | InstanceStateKind::NotAliveNoWriters
    ) {
        // Generation bumps as on the writer side — this matches the
        // spec, because readers derive it from the sample stream.
        match entry.kind {
            InstanceStateKind::NotAliveDisposed => {
                entry.disposed_generation_count = entry.disposed_generation_count.saturating_add(1);
            }
            InstanceStateKind::NotAliveNoWriters => {
                entry.no_writers_generation_count =
                    entry.no_writers_generation_count.saturating_add(1);
            }
            InstanceStateKind::Alive => {}
        }
        entry.kind = InstanceStateKind::Alive;
        entry.reader_view_new = true;
    }
    if let Some(ts) = timestamp {
        entry.last_sample_timestamp = Some(ts);
    }
    entry.samples_in_cache = entry.samples_in_cache.saturating_add(1);
}

impl Clone for InstanceTracker {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            allocator: Arc::clone(&self.allocator),
            key_handles: self.key_handles,
        }
    }
}

/// `t` in nanoseconds.
fn time_nanos(t: Time) -> i128 {
    i128::from(t.sec) * 1_000_000_000 + i128::from(t.nanosec)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn kh(byte: u8) -> KeyHash {
        let mut k = [0u8; KEY_HASH_LEN];
        k[0] = byte;
        k
    }

    #[test]
    fn admission_observes_counts_and_refuses_an_instance_past_the_limit() {
        let t = InstanceTracker::new();
        let now = Time::new(1, 0);
        let (a, in_cache) = t
            .observe_admitted(kh(1), || alloc::vec![1], now, 1)
            .expect("room for the first instance");
        assert_eq!(in_cache, 1);
        assert_eq!(t.get_key_holder(a), Some(alloc::vec![1]));
        // A known instance is admitted at the limit, and its holder is not
        // built again.
        let (again, in_cache) = t
            .observe_admitted(kh(1), || panic!("the holder is known"), now, 1)
            .expect("a known instance");
        assert_eq!((again, in_cache), (a, 2));
        assert!(
            t.observe_admitted(kh(2), || alloc::vec![2], now, 1)
                .is_none()
        );
        assert_eq!(t.len(), 1, "a refused instance is not observed");

        let writer = [7u8; 16];
        let (view_new, kind, disposed, no_writers) = t
            .note_admitted(&kh(1), now, now, writer)
            .expect("a known instance");
        assert!(view_new);
        assert_eq!(
            (kind, disposed, no_writers),
            (InstanceStateKind::Alive, 0, 0)
        );
        assert_eq!(t.last_delivered_ts(&kh(1)), Some(now));
        assert_eq!(t.drop_writer(&writer, None), alloc::vec![a]);

        t.note_taken(a, 2);
        let state = t.get_by_handle(a).expect("state");
        assert!(!state.reader_view_new);
        assert_eq!(state.samples_in_cache, 0);
    }

    #[test]
    fn the_filter_and_the_order_keep_their_own_time() {
        // DESTINATION_ORDER (§2.2.3.17) and TIME_BASED_FILTER (§2.2.3.12)
        // each compare against their own last delivery: a source timestamp
        // far behind the filter's time moves neither into the other.
        let t = InstanceTracker::new();
        let now = Time::new(1, 0);
        t.observe_admitted(kh(1), || alloc::vec![1], now, 1)
            .expect("a new instance");
        let order = Time::new(5, 0);
        let filter = Time::new(1_000, 0);
        t.note_admitted(&kh(1), order, filter, [7u8; 16])
            .expect("a known instance");
        assert_eq!(t.last_delivered_ts(&kh(1)), Some(order));
        assert!(!t.should_deliver_under_time_based_filter(
            &kh(1),
            Time::new(1_000, 500_000_000),
            1_000_000_000
        ));
        assert!(t.should_deliver_under_time_based_filter(
            &kh(1),
            Time::new(1_001, 0),
            1_000_000_000
        ));
        assert!(t.should_deliver_under_destination_order(&kh(1), Time::new(6, 0), true));
    }

    #[test]
    fn register_assigns_stable_handle() {
        let t = InstanceTracker::new();
        let h1 = t.register(kh(1), alloc::vec![1], None);
        let h2 = t.register(kh(1), alloc::vec![1], None);
        assert_eq!(h1, h2);
        assert!(!h1.is_nil());
    }

    #[test]
    fn lookup_returns_handle_for_known_key() {
        let t = InstanceTracker::new();
        let h = t.register(kh(2), alloc::vec![2], None);
        assert_eq!(t.lookup(&kh(2)), Some(h));
        assert_eq!(t.lookup(&kh(99)), None);
    }

    #[test]
    fn dispose_transitions_to_disposed() {
        let t = InstanceTracker::new();
        let h = t.register(kh(3), alloc::vec![3], None);
        assert_eq!(t.get_by_handle(h).unwrap().kind, InstanceStateKind::Alive);
        assert!(t.dispose(h, None));
        assert_eq!(
            t.get_by_handle(h).unwrap().kind,
            InstanceStateKind::NotAliveDisposed
        );
    }

    #[test]
    fn unregister_decrements_writer_count() {
        let t = InstanceTracker::new();
        let h = t.register(kh(4), alloc::vec![4], None);
        // Two writers have registered.
        let _ = t.register(kh(4), alloc::vec![4], None);
        assert_eq!(t.get_by_handle(h).unwrap().writer_count, 2);
        assert!(t.unregister(h, None));
        assert_eq!(t.get_by_handle(h).unwrap().kind, InstanceStateKind::Alive);
        assert!(t.unregister(h, None));
        // now 0 writers → no_writers
        assert_eq!(
            t.get_by_handle(h).unwrap().kind,
            InstanceStateKind::NotAliveNoWriters
        );
    }

    #[test]
    fn re_register_after_dispose_bumps_disposed_generation() {
        let t = InstanceTracker::new();
        let h = t.register(kh(5), alloc::vec![5], None);
        t.dispose(h, None);
        let _ = t.register(kh(5), alloc::vec![5], None);
        let s = t.get_by_handle(h).unwrap();
        assert_eq!(s.kind, InstanceStateKind::Alive);
        assert_eq!(s.disposed_generation_count, 1);
    }

    #[test]
    fn observe_sample_creates_new_instance_on_first_call() {
        let t = InstanceTracker::new();
        let (h, was_new) = t.observe_sample(kh(6), alloc::vec![6], None);
        assert!(was_new);
        assert!(t.get_by_handle(h).unwrap().reader_view_new);
        let (h2, was_new2) = t.observe_sample(kh(6), alloc::vec![6], None);
        assert_eq!(h, h2);
        assert!(!was_new2);
    }

    #[test]
    fn ordered_handles_iterates_in_keyhash_order() {
        let t = InstanceTracker::new();
        let h_b = t.register(kh(2), alloc::vec![2], None);
        let h_a = t.register(kh(1), alloc::vec![1], None);
        let h_c = t.register(kh(3), alloc::vec![3], None);
        assert_eq!(t.ordered_handles(), alloc::vec![h_a, h_b, h_c]);
    }

    #[test]
    fn next_handle_after_walks_in_order() {
        let t = InstanceTracker::new();
        let h_a = t.register(kh(1), alloc::vec![1], None);
        let h_b = t.register(kh(2), alloc::vec![2], None);
        let h_c = t.register(kh(3), alloc::vec![3], None);
        assert_eq!(t.next_handle_after(crate::HANDLE_NIL), Some(h_a));
        assert_eq!(t.next_handle_after(h_a), Some(h_b));
        assert_eq!(t.next_handle_after(h_b), Some(h_c));
        assert_eq!(t.next_handle_after(h_c), None);
    }

    #[test]
    fn get_key_holder_returns_stored_bytes() {
        let t = InstanceTracker::new();
        let h = t.register(kh(7), alloc::vec![1, 2, 3], None);
        assert_eq!(t.get_key_holder(h), Some(alloc::vec![1u8, 2, 3]));
    }

    #[test]
    fn mark_view_seen_clears_new_flag() {
        let t = InstanceTracker::new();
        let (h, _) = t.observe_sample(kh(8), alloc::vec![8], None);
        assert!(t.get_by_handle(h).unwrap().reader_view_new);
        t.mark_view_seen(h);
        assert!(!t.get_by_handle(h).unwrap().reader_view_new);
    }

    #[test]
    fn observe_after_dispose_bumps_disposed_generation() {
        let t = InstanceTracker::new();
        let (h, _) = t.observe_sample(kh(9), alloc::vec![9], None);
        t.dispose(h, None);
        let (_, _) = t.observe_sample(kh(9), alloc::vec![9], None);
        assert_eq!(t.get_by_handle(h).unwrap().disposed_generation_count, 1);
    }

    #[test]
    fn drain_samples_decrements_count() {
        let t = InstanceTracker::new();
        let (h, _) = t.observe_sample(kh(10), alloc::vec![10], None);
        let (_, _) = t.observe_sample(kh(10), alloc::vec![10], None);
        assert_eq!(t.get_by_handle(h).unwrap().samples_in_cache, 2);
        t.drain_samples(h, 2);
        assert_eq!(t.get_by_handle(h).unwrap().samples_in_cache, 0);
    }

    // ---- §2.2.3.12 TIME_BASED_FILTER Reader-Drop ----

    #[test]
    fn time_based_filter_first_sample_passes() {
        let t = InstanceTracker::new();
        // New instance — no last_delivered_ts, so deliver.
        let _ = t.observe_sample(kh(20), alloc::vec![20], Some(Time::new(1, 0)));
        let pass = t.should_deliver_under_time_based_filter(
            &kh(20),
            Time::new(1, 0),
            100_000_000, // 100ms
        );
        assert!(pass);
    }

    #[test]
    fn time_based_filter_too_close_drops() {
        let t = InstanceTracker::new();
        let _ = t.observe_sample(kh(20), alloc::vec![20], None);
        t.record_delivery(&kh(20), Time::new(1, 0));
        // 50ms later — drop at min_separation=100ms.
        let pass = t.should_deliver_under_time_based_filter(
            &kh(20),
            Time::new(1, 50_000_000),
            100_000_000,
        );
        assert!(!pass, "50ms < 100ms separation -> drop");
    }

    #[test]
    fn time_based_filter_far_enough_passes() {
        let t = InstanceTracker::new();
        let _ = t.observe_sample(kh(20), alloc::vec![20], None);
        t.record_delivery(&kh(20), Time::new(1, 0));
        // 150ms later — pass.
        let pass = t.should_deliver_under_time_based_filter(
            &kh(20),
            Time::new(1, 150_000_000),
            100_000_000,
        );
        assert!(pass, "150ms > 100ms separation -> deliver");
    }

    #[test]
    fn time_based_filter_zero_separation_always_passes() {
        let t = InstanceTracker::new();
        let _ = t.observe_sample(kh(20), alloc::vec![20], None);
        t.record_delivery(&kh(20), Time::new(1, 0));
        let pass = t.should_deliver_under_time_based_filter(&kh(20), Time::new(1, 0), 0);
        assert!(pass, "min_separation=0 -> no filter");
    }

    #[test]
    fn time_based_filter_per_instance_isolation() {
        // The filter is per-instance; instance A's last_delivered does
        // not affect instance B.
        let t = InstanceTracker::new();
        let _ = t.observe_sample(kh(1), alloc::vec![1], None);
        let _ = t.observe_sample(kh(2), alloc::vec![2], None);
        t.record_delivery(&kh(1), Time::new(5, 0));
        // Instance 2 has no delivery yet → pass.
        let pass =
            t.should_deliver_under_time_based_filter(&kh(2), Time::new(5, 10_000_000), 100_000_000);
        assert!(pass);
    }

    #[test]
    fn time_based_filter_unknown_instance_passes() {
        let t = InstanceTracker::new();
        let pass = t.should_deliver_under_time_based_filter(&kh(99), Time::new(1, 0), 100_000_000);
        assert!(pass, "unknown instance -> pass");
    }

    // ---- §2.2.3.22 READER_DATA_LIFECYCLE autopurge ----

    #[test]
    fn autopurge_disposed_after_delay() {
        let t = InstanceTracker::new();
        let h = t.register(kh(30), alloc::vec![30], None);
        // Dispose with ts=t1.
        t.dispose(h, Some(Time::new(10, 0)));
        // Now=t1+5s, delay=3s → purge.
        let purged = t.autopurge(Time::new(15, 0), 3_000_000_000, u128::MAX);
        assert_eq!(purged, 1);
        // After purge: lookup returns None.
        assert!(t.lookup(&kh(30)).is_none());
    }

    #[test]
    fn autopurge_disposed_before_delay_keeps_instance() {
        let t = InstanceTracker::new();
        let h = t.register(kh(31), alloc::vec![31], None);
        t.dispose(h, Some(Time::new(10, 0)));
        // Now=t1+1s, delay=5s → keep.
        let purged = t.autopurge(Time::new(11, 0), 5_000_000_000, u128::MAX);
        assert_eq!(purged, 0);
        assert!(t.lookup(&kh(31)).is_some());
    }

    #[test]
    fn autopurge_no_writers_after_delay() {
        let t = InstanceTracker::new();
        let h = t.register(kh(32), alloc::vec![32], None);
        // unregister sets writer_count=0 → NotAliveNoWriters with timestamp.
        t.unregister(h, Some(Time::new(20, 0)));
        let purged = t.autopurge(Time::new(25, 0), u128::MAX, 3_000_000_000);
        assert_eq!(purged, 1);
        assert!(t.lookup(&kh(32)).is_none());
    }

    #[test]
    fn autopurge_alive_instance_never_purged() {
        let t = InstanceTracker::new();
        let _h = t.register(kh(33), alloc::vec![33], None);
        // ALIVE → purge ignored.
        let purged = t.autopurge(Time::new(1000, 0), 0, 0);
        assert_eq!(purged, 0);
        assert!(t.lookup(&kh(33)).is_some());
    }

    #[test]
    fn autopurge_infinity_delay_never_purges() {
        // u128::MAX as delay = INFINITE = never purge (spec default).
        let t = InstanceTracker::new();
        let h = t.register(kh(34), alloc::vec![34], None);
        t.dispose(h, Some(Time::new(10, 0)));
        let purged = t.autopurge(Time::new(99999, 0), u128::MAX, u128::MAX);
        assert_eq!(purged, 0);
    }

    // ---- §2.2.3.10 OWNERSHIP=EXCLUSIVE Strength-Selection ----

    fn guid(byte: u8) -> [u8; 16] {
        [byte; 16]
    }

    #[test]
    fn exclusive_first_writer_wins() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(40), alloc::vec![40], None);
        // No owner → first writer accepted, becomes owner.
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(40), guid(1), 10));
        let s = t.get_by_keyhash(&kh(40)).unwrap();
        assert_eq!(s.current_owner, Some((guid(1), 10)));
    }

    #[test]
    fn exclusive_higher_strength_wins() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(41), alloc::vec![41], None);
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(41), guid(1), 10));
        // Stronger writer arrives → replaces owner.
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(41), guid(2), 20));
        let s = t.get_by_keyhash(&kh(41)).unwrap();
        assert_eq!(s.current_owner, Some((guid(2), 20)));
    }

    #[test]
    fn exclusive_lower_strength_rejected() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(42), alloc::vec![42], None);
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(42), guid(2), 20));
        // Weaker writer rejected.
        assert!(!t.should_accept_sample_under_exclusive_ownership(&kh(42), guid(1), 5));
        let s = t.get_by_keyhash(&kh(42)).unwrap();
        assert_eq!(s.current_owner, Some((guid(2), 20)));
    }

    #[test]
    fn exclusive_tie_break_by_lower_guid() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(43), alloc::vec![43], None);
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(43), guid(2), 10));
        // Same strength, lower guid → wins, as in Cyclone DDS.
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(43), guid(1), 10));
    }

    #[test]
    fn exclusive_tie_break_higher_guid_rejected() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(44), alloc::vec![44], None);
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(44), guid(1), 10));
        // Same strength, higher guid → reject.
        assert!(!t.should_accept_sample_under_exclusive_ownership(&kh(44), guid(2), 10));
    }

    /// §2.2.3.9.2 (b): the owner's own sample is accepted at the strength
    /// it carries, and a writer stronger than that takes the instance over.
    /// An owner that lowered its strength was rejected by itself.
    #[test]
    fn an_owner_that_lowers_its_strength_keeps_writing_until_a_stronger_one() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(46), alloc::vec![46], None);
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(46), guid(1), 10));
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(46), guid(1), 3));
        assert!(!t.should_accept_sample_under_exclusive_ownership(&kh(46), guid(2), 2));
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(46), guid(2), 5));
        assert_eq!(
            t.get_by_keyhash(&kh(46)).unwrap().current_owner,
            Some((guid(2), 5))
        );
    }

    /// §2.2.3.9.2 (b) without a sample of the owner: the strength it
    /// announced holds the instances it owns.
    #[test]
    fn an_announced_strength_holds_the_owned_instances() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(47), alloc::vec![47], None);
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(47), guid(1), 10));
        assert!(!t.should_accept_sample_under_exclusive_ownership(&kh(47), guid(2), 5));
        t.update_owner_strength(guid(1), 3);
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(47), guid(2), 5));
    }

    /// §2.2.3.9.2 (d): an owner whose last sample is a deadline period or
    /// more ago gives the instance up to the next writer, weaker or not.
    #[test]
    fn an_owner_that_missed_its_deadline_gives_the_instance_up() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(48), alloc::vec![48], None);
        let at = |ms: u32| Time::new(100, ms * 1_000_000);
        let period = Some(200_000_000u128);
        assert!(t.accept_under_exclusive_ownership(&kh(48), guid(1), 10, at(0), period));
        assert!(!t.accept_under_exclusive_ownership(&kh(48), guid(2), 5, at(150), period));
        assert!(t.accept_under_exclusive_ownership(&kh(48), guid(2), 5, at(250), period));
        // The stronger writer takes it back with its next sample.
        assert!(t.accept_under_exclusive_ownership(&kh(48), guid(1), 10, at(260), period));
        assert!(!t.accept_under_exclusive_ownership(&kh(48), guid(2), 5, at(270), period));
    }

    #[test]
    fn exclusive_same_writer_always_accepted() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(45), alloc::vec![45], None);
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(45), guid(7), 10));
        // Same writer, same strength → always accepted.
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(45), guid(7), 10));
    }

    // ---- §2.2.3.23 Liveliness-driven Failover ----

    #[test]
    fn clear_owner_for_writer_resets_owner() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(50), alloc::vec![50], None);
        let _ = t.register(kh(51), alloc::vec![51], None);
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(50), guid(9), 100));
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(51), guid(9), 100));
        // Liveliness lost on writer guid(9) → clear from both instances.
        let cleared = t.clear_owner_for_writer(guid(9));
        assert_eq!(cleared, 2);
        let s50 = t.get_by_keyhash(&kh(50)).unwrap();
        let s51 = t.get_by_keyhash(&kh(51)).unwrap();
        assert!(s50.current_owner.is_none());
        assert!(s51.current_owner.is_none());
    }

    #[test]
    fn failover_after_clear_accepts_weaker_writer() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(52), alloc::vec![52], None);
        // Strong writer becomes owner.
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(52), guid(9), 100));
        // Weaker writer normally rejected.
        assert!(!t.should_accept_sample_under_exclusive_ownership(&kh(52), guid(1), 10));
        // Clear (liveliness loss) → weaker writer can take over.
        t.clear_owner_for_writer(guid(9));
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(52), guid(1), 10));
    }

    #[test]
    fn clear_owner_for_writer_prefix_matches_first_12_bytes() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(60), alloc::vec![60], None);
        // GUID = prefix [1;12] + entityId [9,9,9,9]
        let mut full_a = [9u8; 16];
        full_a[..12].fill(1);
        let mut full_b = [9u8; 16];
        full_b[..12].fill(2);
        // owner with prefix=1
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(60), full_a, 50));
        // Clear by mismatching prefix → no clear.
        assert_eq!(t.clear_owner_for_writer_prefix([2u8; 12]), 0);
        let s = t.get_by_keyhash(&kh(60)).unwrap();
        assert!(s.current_owner.is_some());
        // Clear by matching prefix → cleared.
        assert_eq!(t.clear_owner_for_writer_prefix([1u8; 12]), 1);
        let s2 = t.get_by_keyhash(&kh(60)).unwrap();
        assert!(s2.current_owner.is_none());
        // Now full_b can take ownership (prefix=2, weaker).
        let _ = full_b;
    }

    #[test]
    fn clear_owner_for_writer_prefix_multi_instance() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(70), alloc::vec![70], None);
        let _ = t.register(kh(71), alloc::vec![71], None);
        let _ = t.register(kh(72), alloc::vec![72], None);
        let mut g = [0u8; 16];
        g[..12].fill(7);
        // Owner = same prefix on 3 instances.
        for k in [kh(70), kh(71), kh(72)] {
            assert!(t.should_accept_sample_under_exclusive_ownership(&k, g, 1));
        }
        // One participant disappears (lease lost) → all 3 cleared.
        let cleared = t.clear_owner_for_writer_prefix([7u8; 12]);
        assert_eq!(cleared, 3);
    }

    /// Round 3: `clear()` drops every tracked instance, and a `Clone`
    /// (sharing the same inner `Arc`, as `DataReader::__instances_handle`
    /// hands to a builtin-reader clone) observes the reset too — the
    /// property `DomainParticipant::delete_contained_entities` relies on
    /// to give a builtin reader a genuinely clean post-delete state.
    #[test]
    fn clear_drops_every_instance_and_is_visible_through_a_clone() {
        let t = InstanceTracker::new();
        let clone = t.clone();
        let _ = t.register(kh(80), alloc::vec![80], None);
        let _ = t.register(kh(81), alloc::vec![81], None);
        assert_eq!(t.len(), 2);
        assert_eq!(clone.len(), 2, "Clone shares the same inner state");

        t.clear();

        assert!(t.is_empty());
        assert!(
            clone.is_empty(),
            "a Clone observes the reset -- it shares the Arc, not a snapshot"
        );
        assert_eq!(t.lookup(&kh(80)), None);
        assert!(t.get_by_keyhash(&kh(80)).is_none());
    }
}
