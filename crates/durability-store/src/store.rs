// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The [`DurabilityStore`] trait — the storage abstraction every cold adapter
//! implements (ADR 0009, layer ②b).

use alloc::vec::Vec;
use std::time::SystemTime;

use crate::contract::Contract;
use crate::error::Result;
use crate::model::{DurabilitySample, Page, Selector, StoreStats};

/// Default page size for the [`DurabilityStore::replay_for_topic`] convenience
/// drain when a selector does not set its own `limit`.
pub const DEFAULT_PAGE: usize = 1024;

/// A durable, contract-bounded sample store. Implementations are the cold
/// adapters (sqlite / file / lakehouse / in-memory). All methods are
/// `&self` + `Send + Sync`: the daemon shares one store across threads.
///
/// Retention is bounded ONLY by the topic's `Contract` (ADR 0009 inv. 1):
/// implementations enforce it on [`store`](DurabilityStore::store) (drop-oldest
/// under `KEEP_LAST`, [`StoreError::OutOfResources`](crate::StoreError) under
/// `KEEP_ALL` caps).
pub trait DurabilityStore: Send + Sync {
    /// Registers (or replaces) the retention [`Contract`] for `topic`. The
    /// daemon calls this when it starts serving a topic. Topics without an
    /// explicit contract use the store's default ([`Contract::default`]).
    ///
    /// # Errors
    /// Backend/I/O failure.
    fn set_contract(&self, topic: &str, contract: Contract) -> Result<()>;

    /// Persists `sample` durably, enforcing the topic contract. Returns
    /// [`StoreError::OutOfResources`](crate::StoreError) when a `KEEP_ALL` cap
    /// is hit.
    ///
    /// # Errors
    /// Contract cap exceeded, or a backend/I/O failure.
    fn store(&self, sample: DurabilitySample) -> Result<()>;

    /// Returns one ordered [`Page`] of samples for `topic` matching `selector`.
    /// Ordering is `(instance_key, sequence)`. Drive pagination via
    /// [`Page::next`] + [`Selector::after_cursor`].
    ///
    /// # Errors
    /// Backend/I/O failure.
    fn query(&self, topic: &str, selector: &Selector) -> Result<Page>;

    /// Marks an instance as unregistered at `now`; its samples become eligible
    /// for purge after `now + cleanup_delay` (applied by [`cleanup`]).
    ///
    /// [`cleanup`]: DurabilityStore::cleanup
    ///
    /// # Errors
    /// Backend/I/O failure.
    fn unregister(&self, topic: &str, instance_key: &[u8; 16], now: SystemTime) -> Result<()>;

    /// Purges instances whose `unregister` time + cleanup-delay is `<= now`.
    /// Returns the number of purged instances.
    ///
    /// # Errors
    /// Backend/I/O failure.
    fn cleanup(&self, now: SystemTime) -> Result<usize>;

    /// Aggregate counters for `topic`.
    ///
    /// # Errors
    /// Backend/I/O failure.
    fn stats(&self, topic: &str) -> Result<StoreStats>;

    /// Convenience: drains the full contracted history for `topic` by paging
    /// through [`query`](DurabilityStore::query). Prefer paged `query` for
    /// large contracts; this materializes everything into a `Vec`.
    ///
    /// # Errors
    /// Backend/I/O failure from the underlying `query`.
    fn replay_for_topic(&self, topic: &str) -> Result<Vec<DurabilitySample>> {
        let mut out = Vec::new();
        let mut sel = Selector {
            limit: Some(DEFAULT_PAGE),
            ..Selector::default()
        };
        loop {
            let page = self.query(topic, &sel)?;
            let got = page.samples.len();
            out.extend(page.samples);
            match page.next {
                Some(cursor) if got > 0 => sel = sel.after_cursor(cursor),
                _ => break,
            }
        }
        Ok(out)
    }
}
