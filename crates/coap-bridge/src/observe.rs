// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Observe-Pattern — RFC 7641.
//!
//! Spec §2: ein Client signalisiert Observe-Interesse mit Option
//! `Observe = 0` (register) bzw. `Observe = 1` (deregister). Server
//! liefert Notifications mit monoton steigender `Observe`-Sequenz-
//! Nummer.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// Observe-Option-Number (RFC 7641 §2 — registriert in IANA-Tabelle
/// als 6).
pub const OBSERVE_OPTION_NUMBER: u16 = 6;

/// Observer-Eintrag pro (Resource-Path, Token).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObserverEntry {
    /// Resource-Path (z.B. `dds/Trade/AAPL`).
    pub path: alloc::string::String,
    /// Token-Bytes des registrierenden Clients.
    pub token: Vec<u8>,
    /// Aktueller `Observe`-Sequenz-Counter (Spec §3.4).
    pub seq: u32,
    /// Optional Caller-Reference (z.B. UDP-Endpoint-Encoded-Bytes).
    pub endpoint: Vec<u8>,
}

/// Observe-Registry — Server-Side.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ObserveRegistry {
    by_path: BTreeMap<alloc::string::String, Vec<ObserverEntry>>,
}

impl ObserveRegistry {
    /// Konstruktor.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Spec §3.1: Register-Request mit `Observe = 0`.
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

    /// Spec §3.6: Deregister via `Observe = 1` oder Reset auf NON.
    pub fn deregister(&mut self, path: &str, token: &[u8]) -> bool {
        let Some(list) = self.by_path.get_mut(path) else {
            return false;
        };
        let before = list.len();
        list.retain(|e| e.token != token);
        before != list.len()
    }

    /// Liste aller Observer fuer einen Pfad. Caller iteriert um
    /// Notifications zu emittieren.
    #[must_use]
    pub fn observers_of(&self, path: &str) -> Vec<&ObserverEntry> {
        self.by_path
            .get(path)
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    }

    /// Inkrementiert die `Observe`-Sequenz fuer alle Observer eines
    /// Pfads. Spec §3.4: Notifications muessen monoton steigend
    /// nummeriert sein (modulo 2^24).
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

    /// Anzahl Observer ueber alle Pfade.
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
