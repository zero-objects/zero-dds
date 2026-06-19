// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Exclusive ownership resolution (DDS 1.4 §2.2.3.23 / §2.2.2.5.5).
//!
//! When a topic is configured with `Exclusive` ownership QoS and
//! multiple DataWriters write the same instance, the DataReader MUST
//! deliver samples only from the **currently strongest writer**. The
//! "strongest writer" is the one with the **highest ownership_strength**
//! value; on a tie the GUID decides lexicographically (tie-breaker).
//!
//! This module provides the stateless resolver function. The caller
//! (DataReader.take()) must hold an `OwnershipResolver` state per
//! instance.
//!
//! WP 2.8 (C2.8 — QoS completeness).

extern crate alloc;

use alloc::vec::Vec;

/// 16-byte GUID reference (of a writer). We copy the GUID in as
/// `[u8; 16]` so the module needs no zerodds-rtps dependency.
pub type WriterGuidBytes = [u8; 16];

/// A single candidate writer for exclusive ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OwnershipCandidate {
    /// 16-byte writer GUID.
    pub guid: WriterGuidBytes,
    /// `OwnershipStrengthQosPolicy.value` (i32, default 0).
    pub strength: i32,
    /// True if the writer is currently known as ALIVE (liveliness state).
    /// Inactive writers are excluded from the resolution.
    pub alive: bool,
}

/// Selects from a candidate list the current "strongest writer" per
/// DDS 1.4 §2.2.3.23.4:
///
/// 1. Filter out non-alive writers.
/// 2. Sort by `strength` descending; on a tie by GUID ascending
///    (tie-breaker).
/// 3. Return the first entry (or `None` if all were filtered out).
#[must_use]
pub fn resolve_strongest(candidates: &[OwnershipCandidate]) -> Option<OwnershipCandidate> {
    let mut alive: Vec<OwnershipCandidate> =
        candidates.iter().copied().filter(|c| c.alive).collect();
    if alive.is_empty() {
        return None;
    }
    // Sort: highest strength first; on a tie the smallest GUID first.
    alive.sort_by(|a, b| {
        b.strength
            .cmp(&a.strength)
            .then_with(|| a.guid.cmp(&b.guid))
    });
    alive.first().copied()
}

/// Stateful resolver — held per instance in the DataReader and
/// remembers the currently accepted writer.
///
/// `accept(sample_writer)` returns `true` if the sample should be
/// delivered, `false` if it should be filtered.
#[derive(Debug, Clone, Default)]
pub struct OwnershipResolver {
    /// Known writers with their current strength + alive state.
    candidates: Vec<OwnershipCandidate>,
    /// Cache of the currently strongest writer.
    current: Option<WriterGuidBytes>,
}

impl OwnershipResolver {
    /// New empty resolver.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a writer (or updates its strength/alive).
    /// Recomputes the currently strongest writer.
    pub fn upsert(&mut self, candidate: OwnershipCandidate) {
        if let Some(existing) = self
            .candidates
            .iter_mut()
            .find(|c| c.guid == candidate.guid)
        {
            *existing = candidate;
        } else {
            self.candidates.push(candidate);
        }
        self.recompute();
    }

    /// Removes a writer (e.g. after unmatch or lost liveliness).
    pub fn remove(&mut self, guid: &WriterGuidBytes) {
        self.candidates.retain(|c| &c.guid != guid);
        self.recompute();
    }

    /// True if the writer is currently the strongest.
    #[must_use]
    pub fn is_strongest(&self, guid: &WriterGuidBytes) -> bool {
        self.current.as_ref() == Some(guid)
    }

    /// True if a sample of this writer should be delivered.
    /// Convenience wrapper around `is_strongest`.
    #[must_use]
    pub fn accept(&self, writer_guid: &WriterGuidBytes) -> bool {
        self.is_strongest(writer_guid)
    }

    /// Currently strongest writer.
    #[must_use]
    pub fn current(&self) -> Option<WriterGuidBytes> {
        self.current
    }

    fn recompute(&mut self) {
        self.current = resolve_strongest(&self.candidates).map(|c| c.guid);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn cand(byte: u8, strength: i32, alive: bool) -> OwnershipCandidate {
        OwnershipCandidate {
            guid: [byte; 16],
            strength,
            alive,
        }
    }

    #[test]
    fn resolve_picks_highest_strength() {
        let cs = [
            cand(0xAA, 5, true),
            cand(0xBB, 10, true),
            cand(0xCC, 1, true),
        ];
        assert_eq!(resolve_strongest(&cs).unwrap().guid, [0xBB; 16]);
    }

    #[test]
    fn resolve_ignores_non_alive() {
        let cs = [
            cand(0xAA, 100, false), // highest strength but not alive
            cand(0xBB, 5, true),
        ];
        assert_eq!(resolve_strongest(&cs).unwrap().guid, [0xBB; 16]);
    }

    #[test]
    fn resolve_returns_none_if_all_dead() {
        let cs = [cand(0xAA, 10, false), cand(0xBB, 5, false)];
        assert!(resolve_strongest(&cs).is_none());
    }

    #[test]
    fn tie_breaker_uses_smallest_guid() {
        // Both have strength=5; the smaller GUID wins.
        let cs = [cand(0xBB, 5, true), cand(0xAA, 5, true)];
        assert_eq!(resolve_strongest(&cs).unwrap().guid, [0xAA; 16]);
    }

    #[test]
    fn resolver_upsert_tracks_strongest() {
        let mut r = OwnershipResolver::new();
        r.upsert(cand(0xAA, 5, true));
        r.upsert(cand(0xBB, 10, true));
        assert!(r.is_strongest(&[0xBB; 16]));
        assert!(!r.is_strongest(&[0xAA; 16]));
    }

    #[test]
    fn resolver_strength_change_triggers_swap() {
        let mut r = OwnershipResolver::new();
        r.upsert(cand(0xAA, 5, true));
        r.upsert(cand(0xBB, 10, true));
        assert!(r.is_strongest(&[0xBB; 16]));
        // BB drops below AA
        r.upsert(cand(0xBB, 1, true));
        assert!(r.is_strongest(&[0xAA; 16]));
    }

    #[test]
    fn resolver_remove_triggers_recompute() {
        let mut r = OwnershipResolver::new();
        r.upsert(cand(0xAA, 5, true));
        r.upsert(cand(0xBB, 10, true));
        assert!(r.is_strongest(&[0xBB; 16]));
        r.remove(&[0xBB; 16]);
        assert!(r.is_strongest(&[0xAA; 16]));
    }

    #[test]
    fn resolver_dead_strongest_falls_through() {
        let mut r = OwnershipResolver::new();
        r.upsert(cand(0xAA, 10, true));
        r.upsert(cand(0xBB, 5, true));
        assert!(r.is_strongest(&[0xAA; 16]));
        // AA goes ALIVE→NOT_ALIVE
        r.upsert(cand(0xAA, 10, false));
        assert!(r.is_strongest(&[0xBB; 16]));
    }

    #[test]
    fn resolver_accept_helper() {
        let mut r = OwnershipResolver::new();
        r.upsert(cand(0xAA, 10, true));
        assert!(r.accept(&[0xAA; 16]));
        assert!(!r.accept(&[0xBB; 16]));
    }

    #[test]
    fn empty_resolver_accepts_nothing() {
        let r = OwnershipResolver::new();
        assert!(!r.accept(&[0xAA; 16]));
        assert!(r.current().is_none());
    }
}
