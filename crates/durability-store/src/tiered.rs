// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! [`TieredStore`] — a bounded memory hot-cache over any cold
//! [`DurabilityStore`] (ADR 0009, layer ②a).
//!
//! Invariants (ADR 0009):
//! * **Cold is authoritative.** Every `store` is write-through to cold, which
//!   enforces the QoS contract. [`query`](DurabilityStore::query) reads cold —
//!   a late-joiner always gets the FULL contracted history, regardless of how
//!   small the hot budget is.
//! * **The hot budget bounds the cache only.** It evicts the lowest-sequence
//!   samples from RAM when the per-topic byte budget is exceeded — it never
//!   drops a sample from the durable cold store, so it cannot shrink the
//!   contract.

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use std::sync::Mutex;
use std::time::SystemTime;

use crate::contract::Contract;
use crate::error::{Result, StoreError};
use crate::model::{DurabilitySample, Page, Selector, StoreStats};
use crate::store::DurabilityStore;

/// Hot-cache ordering key: `(sequence, instance_key)` so eviction of the
/// smallest entry removes the lowest sequence first (keeps the newest hot).
type HotKey = (u64, [u8; 16]);

#[derive(Default)]
struct HotTopic {
    samples: BTreeMap<HotKey, DurabilitySample>,
    bytes: u64,
}

#[derive(Default)]
struct Hot {
    topics: BTreeMap<String, HotTopic>,
}

/// A memory hot-cache over a cold [`DurabilityStore`]. The hot budget is
/// per-topic, in bytes of payload.
pub struct TieredStore<C: DurabilityStore> {
    cold: C,
    hot: Mutex<Hot>,
    budget_bytes: u64,
}

impl<C: DurabilityStore> TieredStore<C> {
    /// Wraps `cold` with a per-topic hot cache of `budget_bytes` payload bytes.
    /// A `budget_bytes` of 0 disables the hot tier (every read hits cold).
    #[must_use]
    pub fn new(cold: C, budget_bytes: u64) -> Self {
        Self {
            cold,
            hot: Mutex::new(Hot::default()),
            budget_bytes,
        }
    }

    /// Borrows the underlying cold store (e.g. for adapter-native queries).
    pub fn cold(&self) -> &C {
        &self.cold
    }

    fn lock_hot(&self) -> Result<std::sync::MutexGuard<'_, Hot>> {
        self.hot
            .lock()
            .map_err(|_| StoreError::Poisoned("tiered hot cache"))
    }

    /// Inserts into the hot cache and evicts the lowest sequences while over
    /// budget. Never touches cold (the durable copy is independent).
    fn cache(&self, sample: &DurabilitySample) -> Result<()> {
        if self.budget_bytes == 0 {
            return Ok(());
        }
        let mut g = self.lock_hot()?;
        let topic = g.topics.entry(sample.topic.clone()).or_default();
        let hk: HotKey = (sample.sequence, sample.instance_key);
        if let Some(old) = topic.samples.insert(hk, sample.clone()) {
            topic.bytes = topic.bytes.saturating_sub(old.payload.len() as u64);
        }
        topic.bytes += sample.payload.len() as u64;
        while topic.bytes > self.budget_bytes {
            let Some((&k, _)) = topic.samples.iter().next() else {
                break;
            };
            if let Some(removed) = topic.samples.remove(&k) {
                topic.bytes = topic.bytes.saturating_sub(removed.payload.len() as u64);
            }
            // Keep at least one sample hot even if a single sample exceeds the
            // budget (avoids an empty cache that thrashes).
            if topic.samples.len() <= 1 {
                break;
            }
        }
        Ok(())
    }

    /// Hot-only fast path: the newest cached samples for `topic` matching
    /// `selector`, ordered by `(instance_key, sequence)`. May return a
    /// SUBSET of the contract (only what is hot) — for the full history use
    /// [`query`](DurabilityStore::query), which reads cold.
    ///
    /// # Errors
    /// Lock poisoned.
    pub fn recent(&self, topic: &str, selector: &Selector) -> Result<Vec<DurabilitySample>> {
        let g = self.lock_hot()?;
        let Some(t) = g.topics.get(topic) else {
            return Ok(Vec::new());
        };
        let mut out: Vec<DurabilitySample> = t
            .samples
            .values()
            .filter(|s| selector.matches(s))
            .cloned()
            .collect();
        out.sort_by_key(|s| (s.instance_key, s.sequence));
        Ok(out)
    }

    /// Current hot-cache payload bytes for `topic` (diagnostics/tests).
    ///
    /// # Errors
    /// Lock poisoned.
    pub fn hot_bytes(&self, topic: &str) -> Result<u64> {
        Ok(self
            .lock_hot()?
            .topics
            .get(topic)
            .map(|t| t.bytes)
            .unwrap_or(0))
    }
}

impl<C: DurabilityStore> DurabilityStore for TieredStore<C> {
    fn set_contract(&self, topic: &str, contract: Contract) -> Result<()> {
        self.cold.set_contract(topic, contract)
    }

    fn store(&self, sample: DurabilitySample) -> Result<()> {
        // Write-through to cold FIRST: cold is the durable authority and the
        // contract enforcer. Only cache once cold has accepted it.
        self.cold.store(sample.clone())?;
        self.cache(&sample)
    }

    fn query(&self, topic: &str, selector: &Selector) -> Result<Page> {
        // Authoritative + complete: always from cold (ADR 0009 inv. 3).
        self.cold.query(topic, selector)
    }

    fn unregister(&self, topic: &str, instance_key: &[u8; 16], now: SystemTime) -> Result<()> {
        self.cold.unregister(topic, instance_key, now)?;
        if let Ok(mut g) = self.hot.lock() {
            if let Some(t) = g.topics.get_mut(topic) {
                let drop: Vec<HotKey> = t
                    .samples
                    .keys()
                    .filter(|(_, k)| k == instance_key)
                    .copied()
                    .collect();
                for k in drop {
                    if let Some(removed) = t.samples.remove(&k) {
                        t.bytes = t.bytes.saturating_sub(removed.payload.len() as u64);
                    }
                }
            }
        }
        Ok(())
    }

    fn cleanup(&self, now: SystemTime) -> Result<usize> {
        self.cold.cleanup(now)
    }

    fn stats(&self, topic: &str) -> Result<StoreStats> {
        self.cold.stats(topic)
    }
}
