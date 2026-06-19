// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! In-memory [`DurabilityStore`] — the cold store for `TRANSIENT` (no disk)
//! and the reference/test vehicle for the trait semantics.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use std::sync::Mutex;
use std::time::SystemTime;

use zerodds_qos::policies::history::HistoryKind;

use crate::contract::Contract;
use crate::error::{Result, StoreError};
use crate::model::{Cursor, DurabilitySample, Page, Selector, StoreStats};
use crate::store::{DEFAULT_PAGE, DurabilityStore};

/// `(topic, instance_key)` map key. Ranging over a topic prefix yields all of
/// its instances in `O(log n + k)`.
type Key = (String, [u8; 16]);

#[derive(Default)]
struct Slot {
    /// Samples kept sorted by `sequence`.
    samples: Vec<DurabilitySample>,
    /// `unregister` wall-clock time, if any.
    unregistered_at: Option<SystemTime>,
}

#[derive(Default)]
struct Inner {
    default_contract: Contract,
    contracts: BTreeMap<String, Contract>,
    by_key: BTreeMap<Key, Slot>,
}

impl Inner {
    fn contract_for(&self, topic: &str) -> Contract {
        self.contracts
            .get(topic)
            .copied()
            .unwrap_or(self.default_contract)
    }

    /// Inclusive `(topic, ..)` range bounds for the BTreeMap.
    fn topic_range(topic: &str) -> (Key, Key) {
        (
            (topic.to_string(), [0u8; 16]),
            (topic.to_string(), [0xffu8; 16]),
        )
    }

    fn topic_sample_count(&self, topic: &str) -> usize {
        let (lo, hi) = Self::topic_range(topic);
        self.by_key
            .range(lo..=hi)
            .map(|(_, s)| s.samples.len())
            .sum()
    }

    fn topic_instance_count(&self, topic: &str) -> usize {
        let (lo, hi) = Self::topic_range(topic);
        self.by_key.range(lo..=hi).count()
    }
}

/// In-memory, contract-bounded durability store. Thread-safe via a single
/// mutex. Used directly as the `TRANSIENT` cold store, and as the hot tier
/// inside [`crate::TieredStore`].
#[derive(Default)]
pub struct InMemoryStore {
    inner: Mutex<Inner>,
}

impl InMemoryStore {
    /// New empty store with the given default contract (used for topics
    /// without an explicit [`set_contract`](DurabilityStore::set_contract)).
    #[must_use]
    pub fn with_default_contract(default_contract: Contract) -> Self {
        Self {
            inner: Mutex::new(Inner {
                default_contract,
                ..Inner::default()
            }),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Inner>> {
        self.inner
            .lock()
            .map_err(|_| StoreError::Poisoned("in-memory store"))
    }
}

/// Inserts `sample` into `slot`, enforcing the per-instance history policy.
/// `KEEP_LAST` evicts the lowest sequence(s); `KEEP_ALL` rejects past the cap.
/// Returns the signed change in the slot's sample count.
fn push_into_slot(slot: &mut Slot, sample: DurabilitySample, contract: &Contract) -> Result<isize> {
    match contract.history_kind {
        HistoryKind::KeepAll => {
            // A re-send of an existing sequence is an idempotent replace — it
            // does not grow the slot, so the cap must not reject it.
            let is_resend = slot
                .samples
                .binary_search_by(|s| s.sequence.cmp(&sample.sequence))
                .is_ok();
            if !is_resend
                && contract.per_instance_bounded()
                && slot.samples.len() >= contract.max_samples_per_instance as usize
            {
                return Err(StoreError::OutOfResources("max_samples_per_instance"));
            }
            let before = slot.samples.len();
            insert_sorted(&mut slot.samples, sample);
            Ok(slot.samples.len() as isize - before as isize)
        }
        HistoryKind::KeepLast => {
            let before = slot.samples.len();
            insert_sorted(&mut slot.samples, sample);
            let depth = contract.effective_depth();
            while slot.samples.len() > depth {
                slot.samples.remove(0); // lowest sequence (sorted)
            }
            Ok(slot.samples.len() as isize - before as isize)
        }
    }
}

/// Inserts keeping `samples` sorted by `sequence` (duplicates by sequence
/// replace in place — a re-sent sample is idempotent).
fn insert_sorted(samples: &mut Vec<DurabilitySample>, sample: DurabilitySample) {
    match samples.binary_search_by(|s| s.sequence.cmp(&sample.sequence)) {
        Ok(pos) => samples[pos] = sample,
        Err(pos) => samples.insert(pos, sample),
    }
}

impl DurabilityStore for InMemoryStore {
    fn set_contract(&self, topic: &str, contract: Contract) -> Result<()> {
        self.lock()?.contracts.insert(topic.to_string(), contract);
        Ok(())
    }

    fn store(&self, sample: DurabilitySample) -> Result<()> {
        let mut g = self.lock()?;
        let contract = g.contract_for(&sample.topic);
        let topic = sample.topic.clone();
        let key: Key = (topic.clone(), sample.instance_key);

        // A re-send of an existing (topic, instance, sequence) is an idempotent
        // replace — it grows nothing, so no cap may reject it.
        let is_resend = g
            .by_key
            .get(&key)
            .map(|slot| {
                slot.samples
                    .binary_search_by(|s| s.sequence.cmp(&sample.sequence))
                    .is_ok()
            })
            .unwrap_or(false);

        // Global per-topic caps (only meaningful under KEEP_ALL; KEEP_LAST
        // self-bounds via depth but max_samples still applies as a ceiling).
        if !is_resend
            && contract.samples_bounded()
            && g.topic_sample_count(&topic) >= contract.max_samples as usize
            && matches!(contract.history_kind, HistoryKind::KeepAll)
        {
            return Err(StoreError::OutOfResources("max_samples"));
        }
        let new_instance = !g.by_key.contains_key(&key);
        if new_instance
            && contract.instances_bounded()
            && g.topic_instance_count(&topic) >= contract.max_instances as usize
        {
            return Err(StoreError::OutOfResources("max_instances"));
        }
        let slot = g.by_key.entry(key).or_default();
        push_into_slot(slot, sample, &contract)?;
        Ok(())
    }

    fn query(&self, topic: &str, selector: &Selector) -> Result<Page> {
        let g = self.lock()?;
        let (lo, hi) = Inner::topic_range(topic);
        let mut matched: Vec<DurabilitySample> = g
            .by_key
            .range(lo..=hi)
            .flat_map(|(_, s)| s.samples.iter())
            .filter(|s| selector.matches(s))
            .cloned()
            .collect();
        matched.sort_by_key(|s| (s.instance_key, s.sequence));
        let limit = selector.limit.unwrap_or(DEFAULT_PAGE);
        let exhausted = matched.len() <= limit;
        matched.truncate(limit);
        let next: Option<Cursor> = if exhausted {
            None
        } else {
            matched.last().map(|s| (s.instance_key, s.sequence))
        };
        Ok(Page {
            samples: matched,
            next,
        })
    }

    fn unregister(&self, topic: &str, instance_key: &[u8; 16], now: SystemTime) -> Result<()> {
        let mut g = self.lock()?;
        if let Some(slot) = g.by_key.get_mut(&(topic.to_string(), *instance_key)) {
            slot.unregistered_at = Some(now);
        }
        Ok(())
    }

    fn cleanup(&self, now: SystemTime) -> Result<usize> {
        let mut g = self.lock()?;
        let due: Vec<Key> = g
            .by_key
            .iter()
            .filter_map(|(k, slot)| {
                let ts = slot.unregistered_at?;
                let delay = g.contract_for(&k.0).cleanup_delay;
                let deadline = ts.checked_add(delay)?;
                if now >= deadline {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect();
        let removed = due.len();
        for k in due {
            g.by_key.remove(&k);
        }
        Ok(removed)
    }

    fn stats(&self, topic: &str) -> Result<StoreStats> {
        let g = self.lock()?;
        let (lo, hi) = Inner::topic_range(topic);
        let mut stats = StoreStats::default();
        for (_, slot) in g.by_key.range(lo..=hi) {
            stats.instances += 1;
            stats.samples += slot.samples.len();
            stats.bytes += slot
                .samples
                .iter()
                .map(|s| s.payload.len() as u64)
                .sum::<u64>();
        }
        Ok(stats)
    }
}
