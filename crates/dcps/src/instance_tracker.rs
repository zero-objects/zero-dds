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

use alloc::collections::BTreeMap;
use alloc::sync::Arc;

#[cfg(feature = "std")]
use std::sync::Mutex;

use zerodds_cdr::KEY_HASH_LEN;

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
    /// Reader side: source_timestamp of this instance's last sample
    /// delivered to the user API. Spec §2.2.3.12 TIME_BASED_FILTER: a
    /// new sample is filtered out when
    /// `t - last_delivered_ts < minimum_separation`.
    pub last_delivered_ts: Option<Time>,
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
    /// higher `GuidLike` wins. On liveliness loss: reset explicitly via
    /// `clear_owner`.
    pub current_owner: Option<([u8; 16], i32)>,
    /// Stored key holder (bytes), so `get_key_value` can replay the key
    /// without re-decoding. This is the **PLAIN_CDR2-BE** stream, i.e.
    /// exactly the input to `compute_key_hash`.
    pub key_holder: alloc::vec::Vec<u8>,
    /// View state on the reader side. Unused on the writer side.
    pub reader_view_new: bool,
    /// Number of reader samples so far in this instance (a sample_rank
    /// helper value for the next read).
    pub samples_in_cache: u32,
}

impl InstanceState {
    fn fresh(handle: InstanceHandle, key_holder: alloc::vec::Vec<u8>) -> Self {
        Self {
            handle,
            kind: InstanceStateKind::Alive,
            disposed_generation_count: 0,
            no_writers_generation_count: 0,
            writer_count: 0,
            last_sample_timestamp: None,
            last_delivered_ts: None,
            disposed_at: None,
            no_writers_at: None,
            current_owner: None,
            key_holder,
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
}

#[derive(Debug, Default)]
struct TrackerInner {
    by_keyhash: BTreeMap<KeyHash, InstanceState>,
    handle_to_keyhash: BTreeMap<InstanceHandle, KeyHash>,
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
        }
    }

    /// Tracker with a shared allocator (e.g. when a writer and reader in
    /// the same participant draw their handles from the same pool).
    #[must_use]
    pub fn with_allocator(allocator: Arc<InstanceHandleAllocator>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TrackerInner::default())),
            allocator,
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
            let h = self.allocator.allocate();
            InstanceState::fresh(h, key_holder.clone())
        });
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
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = g.by_keyhash.get(keyhash) else {
            return true;
        };
        let Some(last) = s.last_delivered_ts else {
            return true;
        };
        // Nanosecond difference; for a sample in the past (clock skew),
        // never filter — the caller has a reorder problem, not a filter
        // problem.
        let last_nanos = u128::from(u64::try_from(last.sec).unwrap_or(0)) * 1_000_000_000
            + u128::from(last.nanosec);
        let sample_nanos = u128::from(u64::try_from(sample_ts.sec).unwrap_or(0)) * 1_000_000_000
            + u128::from(sample_ts.nanosec);
        if sample_nanos < last_nanos {
            return true;
        }
        sample_nanos - last_nanos >= min_separation_nanos
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
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = g.by_keyhash.get(keyhash) else {
            return true;
        };
        let Some(last) = s.last_delivered_ts else {
            return true;
        };
        let last_nanos = u128::from(u64::try_from(last.sec).unwrap_or(0)) * 1_000_000_000
            + u128::from(last.nanosec);
        let src_nanos = u128::from(u64::try_from(source_ts.sec).unwrap_or(0)) * 1_000_000_000
            + u128::from(source_ts.nanosec);
        src_nanos > last_nanos
    }

    /// Marks that a sample with `sample_ts` was delivered to the user
    /// API (Spec §2.2.3.12 — for the next filter decision).
    pub fn record_delivery(&self, keyhash: &KeyHash, sample_ts: Time) {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(s) = g.by_keyhash.get_mut(keyhash) {
            s.last_delivered_ts = Some(sample_ts);
        }
    }

    /// Lookup without mutation (Spec §2.2.2.4.2.14 `lookup_instance`).
    #[must_use]
    pub fn lookup(&self, keyhash: &KeyHash) -> Option<InstanceHandle> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.by_keyhash.get(keyhash).map(|s| s.handle)
    }

    /// Returns a copy of the state snapshot for a handle.
    #[must_use]
    pub fn get_by_handle(&self, handle: InstanceHandle) -> Option<InstanceState> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let kh = g.handle_to_keyhash.get(&handle)?;
        g.by_keyhash.get(kh).cloned()
    }

    /// Returns a copy of the state snapshot for a KeyHash.
    #[must_use]
    pub fn get_by_keyhash(&self, keyhash: &KeyHash) -> Option<InstanceState> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        g.by_keyhash.get(keyhash).cloned()
    }

    /// Returns the key-holder byte stream for a handle (Spec
    /// §2.2.2.4.2.13 `get_key_value`).
    #[must_use]
    pub fn get_key_holder(&self, handle: InstanceHandle) -> Option<alloc::vec::Vec<u8>> {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let kh = g.handle_to_keyhash.get(&handle)?;
        g.by_keyhash.get(kh).map(|s| s.key_holder.clone())
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

    /// Spec §2.2.3.10 OWNERSHIP=EXCLUSIVE strength selection.
    /// Returns `true` if a sample from the writer with
    /// `(writer_guid, writer_strength)` should be accepted for the
    /// instance `keyhash`.
    ///
    /// Algorithm (Spec §2.2.3.10):
    /// - No current owner → accept + set as owner.
    /// - Strength > current → accept + replace owner.
    /// - Strength == current and guid > current_guid → accept
    ///   (spec tie-break via the lexicographically higher guid).
    /// - Strength < current → reject.
    /// - Strength == current and guid < current → reject.
    /// - Strength == current and guid == current → accept
    ///   (same writer).
    pub fn should_accept_sample_under_exclusive_ownership(
        &self,
        keyhash: &KeyHash,
        writer_guid: [u8; 16],
        writer_strength: i32,
    ) -> bool {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(s) = g.by_keyhash.get_mut(keyhash) else {
            return true; // unknown instance → accept
        };
        match s.current_owner {
            None => {
                s.current_owner = Some((writer_guid, writer_strength));
                true
            }
            Some((cur_guid, cur_str)) => {
                if writer_strength > cur_str
                    || (writer_strength == cur_str && writer_guid > cur_guid)
                {
                    s.current_owner = Some((writer_guid, writer_strength));
                    true
                } else {
                    writer_strength == cur_str && writer_guid == cur_guid
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
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let mut was_new = false;
        let entry = g.by_keyhash.entry(keyhash).or_insert_with(|| {
            was_new = true;
            let h = self.allocator.allocate();
            InstanceState::fresh(h, key_holder.clone())
        });
        // Reactivate the reader view
        if matches!(
            entry.kind,
            InstanceStateKind::NotAliveDisposed | InstanceStateKind::NotAliveNoWriters
        ) {
            // Generation bumps as on the writer side — this matches the
            // spec, because readers derive it from the sample stream.
            match entry.kind {
                InstanceStateKind::NotAliveDisposed => {
                    entry.disposed_generation_count =
                        entry.disposed_generation_count.saturating_add(1);
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
        let handle = entry.handle;
        g.handle_to_keyhash.insert(handle, keyhash);
        (handle, was_new)
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
}

impl Clone for InstanceTracker {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            allocator: Arc::clone(&self.allocator),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn kh(byte: u8) -> KeyHash {
        let mut k = [0u8; KEY_HASH_LEN];
        k[0] = byte;
        k
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
    fn exclusive_tie_break_by_higher_guid() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(43), alloc::vec![43], None);
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(43), guid(1), 10));
        // Same strength, higher guid → wins.
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(43), guid(2), 10));
    }

    #[test]
    fn exclusive_tie_break_lower_guid_rejected() {
        let t = InstanceTracker::new();
        let _ = t.register(kh(44), alloc::vec![44], None);
        assert!(t.should_accept_sample_under_exclusive_ownership(&kh(44), guid(2), 10));
        // Same strength, lower guid → reject.
        assert!(!t.should_accept_sample_under_exclusive_ownership(&kh(44), guid(1), 10));
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
}
