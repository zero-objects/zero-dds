// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! GUID-to-identity bindings cache (C3.8).
//!
//! Spec DDS-Security 1.2 §7.4.3 (identity-cert bind), §9.3.1 + §10.3.4:
//! a malicious peer can attempt to maliciously "squat" a foreign GUID
//! by sending SPDP beacons with the same `GuidPrefix` but a
//! differing IdentityToken. Without bindings tracking the
//! peer cache would treat the squatter exactly like the legitimate
//! original peer, and in the SEDP match procedure a confusion
//! phase begins which is a DoS / cred-exfil vector.
//!
//! The `IdentityBindingCache` here remembers, per `GuidPrefix`, the
//! hash of the **first-seen** IdentityToken. On subsequent
//! announces with the same GuidPrefix the hash is recomputed and
//! compared — a mismatch leads to rejection.
//!
//! # What is not done here
//!
//! - **Identity-to-GUID derivation**: spec §9.3.1.1 allows a
//!   deterministic binding GuidPrefix = Hash(cert public key). That
//!   is a stronger guarantee but only arrives with the full PKI
//!   handshake path (C3.1).
//! - **Time-bounded re-binding**: a peer that rotates its identity cert
//!   (OCSP-revoked → new cert) needs a re-binding path.
//!   Currently it must be evicted from the cache, then once re-discovered
//!   it is accepted with the new binding. Optional spec section
//!   §10.3.3.2 OCSP — arrives with C3.9.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// SHA-256 would also do, but we don't want an additional
/// crypto dep in the cache layer. Instead of a hash we store the
/// full token bytes — worst case ~ 1 KiB per peer
/// and the cache typically holds < 1000 peers. On overflow the
/// caller trims (see [`IdentityBindingCache::with_capacity`]).
type IdentityFingerprint = Vec<u8>;

/// 12-byte `GuidPrefix` from DDSI-RTPS (wire layout).
pub type GuidPrefixBytes = [u8; 12];

/// Cache of the first observed identity binding per `GuidPrefix`.
#[derive(Debug, Default)]
pub struct IdentityBindingCache {
    bindings: BTreeMap<GuidPrefixBytes, IdentityFingerprint>,
    capacity: usize,
}

/// Evaluation of a new IdentityToken announce by the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingDecision {
    /// First binding — recorded.
    NewBinding,
    /// The binding exists and matches — the peer is consistent.
    Reaffirmed,
    /// **Conflict** — same GuidPrefix, differing IdentityToken.
    /// The incoming announce must be rejected.
    SquatterRejected {
        /// Bytes of the previously stored fingerprint (for logging).
        previous: Vec<u8>,
    },
    /// The cache is full and the peer is new — the caller decides
    /// (eviction strategy, otherwise reject).
    CapacityExhausted,
}

impl IdentityBindingCache {
    /// Cache with unlimited capacity (default for tests).
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings: BTreeMap::new(),
            capacity: usize::MAX,
        }
    }

    /// Cache with an explicit cap (DoS protection). On reaching it,
    /// [`Self::observe`] returns `CapacityExhausted` and the caller must
    /// evict or reject.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bindings: BTreeMap::new(),
            capacity,
        }
    }

    /// Current number of bindings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// True if there are no bindings in the cache.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Observes a GuidPrefix-to-IdentityToken binding.
    /// `identity_token_bytes` is the **raw CDR DataHolder blob** from
    /// the `PID_IDENTITY_TOKEN` value (see C3.5).
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

    /// Removes a binding — e.g. when a peer was evicted after a
    /// lease timeout or OCSP revoke. Allows the peer to re-discover
    /// with a new identity.
    pub fn evict(&mut self, guid_prefix: &GuidPrefixBytes) -> bool {
        self.bindings.remove(guid_prefix).is_some()
    }

    /// Reads the current fingerprint for a GuidPrefix (for audit/
    /// logging).
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
        // The cache kept the original fingerprint.
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
        // The cache is full, but re-affirming a known peer must
        // keep working.
        assert_eq!(c.observe(px(0x01), b"a"), BindingDecision::Reaffirmed);
    }

    #[test]
    fn capacity_cap_still_detects_squatter() {
        let mut c = IdentityBindingCache::with_capacity(1);
        c.observe(px(0x01), b"a");
        // The cache is full. A known prefix with a different token is
        // still a squatter — not "CapacityExhausted".
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
