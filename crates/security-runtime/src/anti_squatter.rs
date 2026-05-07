// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GUID-zu-Identity-Bindings-Cache (C3.8).
//!
//! Spec DDS-Security 1.2 §7.4.3 (Identity-Cert-Bind), §9.3.1 + §10.3.4:
//! ein boeswilliger Peer kann eine fremde GUID schadhaft "squatting"
//! versuchen, indem er SPDP-Beacons mit gleicher `GuidPrefix` aber
//! abweichendem IdentityToken sendet. Ohne Bindings-Tracking wuerde
//! der Peer-Cache den Squatter genauso behandeln wie den legitimen
//! Original-Peer und im SEDP-Match-Verfahren beginnt eine Confusion-
//! Phase, die ein DoS-/Cred-Exfil-Vektor ist.
//!
//! Der `IdentityBindingCache` hier merkt sich pro `GuidPrefix` den
//! Hash des **erstmals gesehenen** IdentityToken. Bei nachfolgenden
//! Announces mit gleicher GuidPrefix wird der Hash neu berechnet und
//! verglichen — Mismatch fuehrt zur Ablehnung.
//!
//! # Was hier nicht gemacht wird
//!
//! - **Identity-zu-GUID-Ableitung**: Spec §9.3.1.1 erlaubt eine
//!   deterministische Bindung GuidPrefix = Hash(Cert-Public-Key). Das
//!   ist eine staerkere Garantie, kommt aber erst mit dem vollen PKI-
//!   Handshake-Pfad (C3.1).
//! - **Time-bounded Re-Binding**: ein Peer der seine Identity-Cert
//!   rotiert (OCSP-revoked → neues Cert) braucht ein Re-Binding-Path.
//!   Aktuell muss er aus dem Cache evicted werden, dann re-discovered
//!   wird er mit der neuen Bindung akzeptiert. Optionale Spec-Sektion
//!   §10.3.3.2 OCSP — kommt mit C3.9.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// SHA-256 wuerde es auch tun, aber wir wollen keine zusaetzliche
/// Crypto-Dep im Cache-Layer. Statt Hash speichern wir den
/// vollstaendigen Token-Bytes — der ist im Worst-Case ~ 1 KiB pro Peer
/// und der Cache haelt typisch < 1000 Peers. Bei Ueberlauf trim'd der
/// Caller (siehe [`IdentityBindingCache::with_capacity`]).
type IdentityFingerprint = Vec<u8>;

/// 12-byte `GuidPrefix` aus DDSI-RTPS (Wire-Layout).
pub type GuidPrefixBytes = [u8; 12];

/// Cache der ersten beobachteten Identity-Bindung pro `GuidPrefix`.
#[derive(Debug, Default)]
pub struct IdentityBindingCache {
    bindings: BTreeMap<GuidPrefixBytes, IdentityFingerprint>,
    capacity: usize,
}

/// Bewertung eines neuen IdentityToken-Announces durch den Cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingDecision {
    /// Erste Bindung — aufgenommen.
    NewBinding,
    /// Bindung existiert und passt — der Peer ist konsistent.
    Reaffirmed,
    /// **Konflikt** — gleiche GuidPrefix, abweichendes IdentityToken.
    /// Der ankommende Announce muss abgelehnt werden.
    SquatterRejected {
        /// Bytes des bisher gespeicherten Fingerprints (zum Logging).
        previous: Vec<u8>,
    },
    /// Cache ist voll und der Peer ist neu — Caller entscheidet
    /// (Eviction-Strategie, sonst Reject).
    CapacityExhausted,
}

impl IdentityBindingCache {
    /// Cache mit unlimitierter Kapazitaet (default fuer Tests).
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings: BTreeMap::new(),
            capacity: usize::MAX,
        }
    }

    /// Cache mit explizitem Cap (DoS-Schutz). Bei Erreichen liefert
    /// [`Self::observe`] `CapacityExhausted` und der Caller muss
    /// evictieren oder ablehnen.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bindings: BTreeMap::new(),
            capacity,
        }
    }

    /// Aktuelle Anzahl der Bindings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// True wenn keine Bindings im Cache sind.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Beobachtet eine GuidPrefix-zu-IdentityToken-Bindung.
    /// `identity_token_bytes` ist der **rohe CDR-DataHolder-Blob** aus
    /// dem `PID_IDENTITY_TOKEN`-Wert (siehe C3.5).
    pub fn observe(
        &mut self,
        guid_prefix: GuidPrefixBytes,
        identity_token_bytes: &[u8],
    ) -> BindingDecision {
        if let Some(existing) = self.bindings.get(&guid_prefix) {
            return if existing.as_slice() == identity_token_bytes {
                BindingDecision::Reaffirmed
            } else {
                BindingDecision::SquatterRejected {
                    previous: existing.clone(),
                }
            };
        }
        if self.bindings.len() >= self.capacity {
            return BindingDecision::CapacityExhausted;
        }
        self.bindings
            .insert(guid_prefix, identity_token_bytes.to_vec());
        BindingDecision::NewBinding
    }

    /// Entfernt eine Bindung — z.B. wenn ein Peer evicted wurde nach
    /// Lease-Timeout oder OCSP-Revoke. Erlaubt dem Peer, mit neuer
    /// Identity zurueck-zu-discoveren.
    pub fn evict(&mut self, guid_prefix: &GuidPrefixBytes) -> bool {
        self.bindings.remove(guid_prefix).is_some()
    }

    /// Liest den aktuellen Fingerprint zu einer GuidPrefix (fuer Audit/
    /// Logging).
    #[must_use]
    pub fn fingerprint_for(&self, guid_prefix: &GuidPrefixBytes) -> Option<&[u8]> {
        self.bindings.get(guid_prefix).map(Vec::as_slice)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    fn px(byte: u8) -> GuidPrefixBytes {
        [byte; 12]
    }

    #[test]
    fn first_observe_is_new_binding() {
        let mut c = IdentityBindingCache::new();
        let d = c.observe(px(0xAA), b"identity-token-A");
        assert_eq!(d, BindingDecision::NewBinding);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn second_observe_with_same_token_is_reaffirmed() {
        let mut c = IdentityBindingCache::new();
        c.observe(px(0xAA), b"identity-token-A");
        let d = c.observe(px(0xAA), b"identity-token-A");
        assert_eq!(d, BindingDecision::Reaffirmed);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn squatter_with_diff_token_is_rejected() {
        let mut c = IdentityBindingCache::new();
        c.observe(px(0xAA), b"identity-token-A");
        let d = c.observe(px(0xAA), b"identity-token-B");
        match d {
            BindingDecision::SquatterRejected { previous } => {
                assert_eq!(previous, b"identity-token-A");
            }
            other => panic!("expected SquatterRejected, got {other:?}"),
        }
        // Cache hat den Original-Fingerprint behalten.
        assert_eq!(c.fingerprint_for(&px(0xAA)), Some(&b"identity-token-A"[..]));
    }

    #[test]
    fn distinct_prefixes_are_independent() {
        let mut c = IdentityBindingCache::new();
        assert_eq!(
            c.observe(px(0xAA), b"alice-token"),
            BindingDecision::NewBinding
        );
        assert_eq!(
            c.observe(px(0xBB), b"bob-token"),
            BindingDecision::NewBinding
        );
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn evict_allows_rebinding_with_new_token() {
        let mut c = IdentityBindingCache::new();
        c.observe(px(0xAA), b"old-token");
        assert!(c.evict(&px(0xAA)));
        let d = c.observe(px(0xAA), b"new-token");
        assert_eq!(d, BindingDecision::NewBinding);
    }

    #[test]
    fn evict_unknown_prefix_returns_false() {
        let mut c = IdentityBindingCache::new();
        assert!(!c.evict(&px(0xCC)));
    }

    #[test]
    fn capacity_cap_rejects_new_bindings() {
        let mut c = IdentityBindingCache::with_capacity(2);
        assert_eq!(c.observe(px(0x01), b"a"), BindingDecision::NewBinding);
        assert_eq!(c.observe(px(0x02), b"b"), BindingDecision::NewBinding);
        assert_eq!(
            c.observe(px(0x03), b"c"),
            BindingDecision::CapacityExhausted
        );
        assert_eq!(c.len(), 2);
    }

    #[test]
    fn capacity_cap_still_allows_reaffirm() {
        let mut c = IdentityBindingCache::with_capacity(1);
        c.observe(px(0x01), b"a");
        // Cache ist voll, aber re-affirm eines bekannten Peers darf
        // weiter funktionieren.
        assert_eq!(c.observe(px(0x01), b"a"), BindingDecision::Reaffirmed);
    }

    #[test]
    fn capacity_cap_still_detects_squatter() {
        let mut c = IdentityBindingCache::with_capacity(1);
        c.observe(px(0x01), b"a");
        // Cache ist voll. Bekannter Prefix mit anderem Token ist
        // weiterhin Squatter — nicht "CapacityExhausted".
        match c.observe(px(0x01), b"b") {
            BindingDecision::SquatterRejected { .. } => {}
            other => panic!("expected SquatterRejected, got {other:?}"),
        }
    }

    #[test]
    fn empty_token_is_distinct_from_some_token() {
        let mut c = IdentityBindingCache::new();
        c.observe(px(0x01), b"");
        let d = c.observe(px(0x01), b"non-empty");
        assert!(matches!(d, BindingDecision::SquatterRejected { .. }));
    }
}
