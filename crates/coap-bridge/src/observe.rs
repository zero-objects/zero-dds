// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Observe pattern — RFC 7641.
//!
//! Spec §2: a client signals observe interest with option
//! `Observe = 0` (register) or `Observe = 1` (deregister). The server
//! delivers notifications with a monotonically increasing `Observe`
//! sequence number.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// Observe option number (RFC 7641 §2 — registered in the IANA table
/// as 6).
pub const OBSERVE_OPTION_NUMBER: u16 = 6;

/// Observer entry per (resource path, token).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserverEntry {
    /// Resource path (e.g. `dds/Trade/AAPL`).
    pub path: alloc::string::String,
    /// Token bytes of the registering client.
    pub token: Vec<u8>,
    /// Current `Observe` sequence counter (spec §3.4).
    pub seq: u32,
    /// Optional caller reference (e.g. UDP endpoint encoded bytes).
    pub endpoint: Vec<u8>,
}

/// Observe registry — server side.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObserveRegistry {
    by_path: BTreeMap<alloc::string::String, Vec<ObserverEntry>>,
}

impl ObserveRegistry {
    /// Constructor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §3.1: register request with `Observe = 0`.
    pub fn register(&mut self, path: alloc::string::String, token: Vec<u8>, endpoint: Vec<u8>) {
        let list = self.by_path.entry(path.clone()).or_default();
        if let Some(existing) = list.iter_mut().find(|e| e.token == token) {
            existing.endpoint = endpoint;
            return;
        }
        list.push(ObserverEntry {
            path,
            token,
            seq: 0,
            endpoint,
        });
    }

    /// Spec §3.6: deregister via `Observe = 1` or Reset on NON.
    pub fn deregister(&mut self, path: &str, token: &[u8]) -> bool {
        let Some(list) = self.by_path.get_mut(path) else {
            return false;
        };
        let before = list.len();
        list.retain(|e| e.token != token);
        before != list.len()
    }

    /// List of all observers for a path. The caller iterates to
    /// emit notifications.
    #[must_use]
    pub fn observers_of(&self, path: &str) -> Vec<&ObserverEntry> {
        self.by_path
            .get(path)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Increments the `Observe` sequence for all observers of a
    /// path. Spec §3.4: notifications must be numbered monotonically
    /// increasing (modulo 2^24).
    pub fn next_seq(&mut self, path: &str) -> Vec<(Vec<u8>, u32, Vec<u8>)> {
        let Some(list) = self.by_path.get_mut(path) else {
            return Vec::new();
        };
        list.iter_mut()
            .map(|e| {
                e.seq = (e.seq + 1) & 0x00ff_ffff;
                (e.token.clone(), e.seq, e.endpoint.clone())
            })
            .collect()
    }

    /// Number of observers across all paths.
    #[must_use]
    pub fn observer_count(&self) -> usize {
        self.by_path.values().map(Vec::len).sum()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn register_then_observers_lists_one() {
        let mut r = ObserveRegistry::new();
        r.register("path/a".into(), alloc::vec![1], alloc::vec![]);
        assert_eq!(r.observers_of("path/a").len(), 1);
    }

    #[test]
    fn re_register_with_same_token_updates_endpoint() {
        let mut r = ObserveRegistry::new();
        r.register("p".into(), alloc::vec![1], alloc::vec![1]);
        r.register("p".into(), alloc::vec![1], alloc::vec![2]);
        assert_eq!(r.observer_count(), 1);
        assert_eq!(r.observers_of("p")[0].endpoint, alloc::vec![2]);
    }

    #[test]
    fn deregister_removes_specific_token() {
        let mut r = ObserveRegistry::new();
        r.register("p".into(), alloc::vec![1], alloc::vec![]);
        r.register("p".into(), alloc::vec![2], alloc::vec![]);
        assert!(r.deregister("p", &[1]));
        assert_eq!(r.observer_count(), 1);
    }

    #[test]
    fn deregister_unknown_returns_false() {
        let mut r = ObserveRegistry::new();
        assert!(!r.deregister("nope", &[0]));
    }

    #[test]
    fn next_seq_increments_per_observer() {
        let mut r = ObserveRegistry::new();
        r.register("p".into(), alloc::vec![1], alloc::vec![]);
        r.register("p".into(), alloc::vec![2], alloc::vec![]);
        let s1 = r.next_seq("p");
        assert_eq!(s1.len(), 2);
        assert!(s1.iter().all(|(_, seq, _)| *seq == 1));
        let s2 = r.next_seq("p");
        assert!(s2.iter().all(|(_, seq, _)| *seq == 2));
    }

    #[test]
    fn next_seq_wraps_at_24_bits() {
        let mut r = ObserveRegistry::new();
        r.register("p".into(), alloc::vec![1], alloc::vec![]);
        // Force sequence to 2^24-1 manually.
        if let Some(list) = r.by_path.get_mut("p") {
            list[0].seq = 0x00ff_ffff;
        }
        let s = r.next_seq("p");
        assert_eq!(s[0].1, 0, "wrap to 0 per Spec §3.4");
    }

    #[test]
    fn observers_of_unknown_returns_empty() {
        let r = ObserveRegistry::new();
        assert!(r.observers_of("nope").is_empty());
    }
}
