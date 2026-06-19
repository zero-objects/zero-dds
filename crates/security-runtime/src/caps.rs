// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Peer capabilities and cache.
//!
//! [`PeerCapabilities`] is the snapshot of what a remote
//! participant has communicated about its security capabilities
//! (auth/crypto/access plugin class, accepted suites,
//! offered protection level, validity window, etc.). In RC1
//! this snapshot is populated from SPDP properties; here we
//! only define the data model plus an in-memory cache
//! for the runtime.
//!
//! The cache is deliberately a lean [`alloc::collections::BTreeMap`]
//! wrapper: peers come and go, lookups are frequent, the typical
//! peer setup has dozens to low thousands of entries
//! — so no hash-overhead discussion.
//!
//! See `docs/architecture/08_heterogeneous_security.md` §3.2
//! and §4.3 (upgrade path via `update_partial`).

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_security_pki::DelegationChain;

use crate::policy::{ProtectionLevel, SuiteHint};
use crate::shared::PeerKey;

/// Validity window of a peer identity (Unix seconds).
///
/// Minimal representation without a chrono dep — the actual
/// time comparisons happen in the authentication plugin. This struct
/// serves as a transparent pass-through container inside
/// [`PeerCapabilities`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Validity {
    /// Not valid before this point in time (Unix seconds).
    pub not_before: i64,
    /// Not valid after this point in time (Unix seconds).
    pub not_after: i64,
}

impl Validity {
    /// Checks whether `now` lies within the validity window (inclusive).
    #[must_use]
    pub const fn contains(&self, now: i64) -> bool {
        now >= self.not_before && now <= self.not_after
    }
}

/// Security-relevant capabilities of a remote peer.
///
/// Populated from SPDP properties (auth/crypto/access plugin class,
/// `zerodds.sec.supported_suites`, `zerodds.sec.offered_protection`)
/// as well as from SEDP permissions tokens. Legacy peers without
/// security properties land here with `auth_plugin_class=None` —
/// no drop, the [`crate::PolicyEngine`] decides per
/// domain rule whether legacy is accepted.
///
/// All fields are `Option`/`Vec`-based so that partial updates
/// (upgrade path in §4.3 of the architecture doc) are cleanly possible.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PeerCapabilities {
    /// `DDS:Auth:PKI-DH:1.2` (spec 1.2 §10.3.2.1) etc. `None` = legacy
    /// peer without an auth plugin.
    pub auth_plugin_class: Option<String>,
    /// `DDS:Crypto:AES-GCM-GMAC:1.2` (spec 1.2 §10.5) etc.
    pub crypto_plugin_class: Option<String>,
    /// `DDS:Access:Permissions:1.2` (spec 1.2 §10.4) etc.
    pub access_plugin_class: Option<String>,
    /// Suites the peer would accept according to its SPDP announce.
    pub supported_suites: Vec<SuiteHint>,
    /// Protection level the peer **itself** offers.
    pub offered_protection: ProtectionLevel,
    /// `true` if the cert chain + OCSP were checked and ok — set by the
    /// authentication plugin, not from SPDP.
    pub has_valid_cert: bool,
    /// Validity window from the permissions token.
    pub validity_window: Option<Validity>,
    /// Vendor identification (e.g. `"Cyclone DDS"`, `"Fast DDS"`)
    /// for quirks.
    pub vendor_hint: Option<String>,
    /// Subject common name from the peer cert (e.g.
    /// `"writer1.fast.example"`). Set by the authentication plugin after
    /// a successful handshake; **not** propagated via SPDP.
    /// Used for `<zerodds:peer_class><match cert_cn_pattern=...>`
    ///.
    pub cert_cn: Option<String>,
    /// Delegation chain. Propagated by the edge or sub-gateway
    /// via the SPDP property `zerodds.sec.delegation_chain`.
    /// Validation against a delegation profile happens in
    /// `peer_matches_class` (j-d). `None` = peer without a chain (= directly
    /// authenticated peer or legacy).
    pub delegation_chain: Option<DelegationChain>,
}

impl PeerCapabilities {
    /// Merges non-empty fields from `other` into `self`. Empty fields
    /// (`None`, `[]`) stay unchanged — so multiple
    /// partial SPDP updates are idempotent and order-tolerant.
    ///
    /// Special rules:
    /// * `offered_protection` is always taken over (monotonically increasing
    ///   via [`ProtectionLevel::stronger`]) — a peer can upgrade its
    ///   level but not silently downgrade it.
    /// * `has_valid_cert=true` is sticky: once validated, it cannot
    ///   fall back to `false` (cert rotation requires an
    ///   explicit [`PeerCache::forget`]).
    pub fn merge_update(&mut self, other: &PeerCapabilities) {
        if other.auth_plugin_class.is_some() {
            self.auth_plugin_class = other.auth_plugin_class.clone();
        }
        if other.crypto_plugin_class.is_some() {
            self.crypto_plugin_class = other.crypto_plugin_class.clone();
        }
        if other.access_plugin_class.is_some() {
            self.access_plugin_class = other.access_plugin_class.clone();
        }
        if !other.supported_suites.is_empty() {
            self.supported_suites = other.supported_suites.clone();
        }
        self.offered_protection = self.offered_protection.stronger(other.offered_protection);
        if other.has_valid_cert {
            self.has_valid_cert = true;
        }
        if other.validity_window.is_some() {
            self.validity_window = other.validity_window;
        }
        if other.vendor_hint.is_some() {
            self.vendor_hint = other.vendor_hint.clone();
        }
        if other.cert_cn.is_some() {
            self.cert_cn = other.cert_cn.clone();
        }
        if other.delegation_chain.is_some() {
            self.delegation_chain = other.delegation_chain.clone();
        }
    }
}

/// In-memory cache `PeerKey → PeerCapabilities`.
///
/// One instance per participant; its lifetime matches the
/// participant itself. Thread safety is provided by the caller via
/// `Arc<Mutex<PeerCache>>` or `Arc<RwLock<PeerCache>>` — the cache
/// itself is intentionally **not** thread-safe, because per tick
/// usually only one writer/reader task touches it.
#[derive(Debug, Default, Clone)]
pub struct PeerCache {
    inner: BTreeMap<PeerKey, PeerCapabilities>,
}

impl PeerCache {
    /// Build an empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of known peers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// `true` if no peer is known.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Full upsert. Overwrites an existing entry
    /// with the new capabilities. For partial updates
    /// use [`Self::update_partial`].
    pub fn insert(&mut self, key: PeerKey, caps: PeerCapabilities) {
        self.inner.insert(key, caps);
    }

    /// Reads the capabilities of a peer.
    #[must_use]
    pub fn get(&self, key: &PeerKey) -> Option<&PeerCapabilities> {
        self.inner.get(key)
    }

    /// Merges new information into the existing entry.
    /// If the peer is new → it is inserted.
    ///
    /// For the semantics see [`PeerCapabilities::merge_update`].
    pub fn update_partial(&mut self, key: PeerKey, update: &PeerCapabilities) {
        self.inner
            .entry(key)
            .and_modify(|existing| existing.merge_update(update))
            .or_insert_with(|| update.clone());
    }

    /// Removes a peer from the cache. Returns the old caps
    /// if present — handy for logging at session end.
    pub fn forget(&mut self, key: &PeerKey) -> Option<PeerCapabilities> {
        self.inner.remove(key)
    }

    /// Iterator over all `(PeerKey, PeerCapabilities)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&PeerKey, &PeerCapabilities)> {
        self.inner.iter()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn caps_secure() -> PeerCapabilities {
        PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".to_string()),
            crypto_plugin_class: Some("DDS:Crypto:AES-GCM-GMAC:1.2".to_string()),
            access_plugin_class: Some("DDS:Access:Permissions:1.2".to_string()),
            supported_suites: alloc::vec![SuiteHint::Aes128Gcm, SuiteHint::Aes256Gcm],
            offered_protection: ProtectionLevel::Encrypt,
            has_valid_cert: true,
            validity_window: Some(Validity {
                not_before: 0,
                not_after: 2_000_000_000,
            }),
            vendor_hint: Some("zerodds".to_string()),
            cert_cn: Some("writer1.fast.example".to_string()),
            delegation_chain: None,
        }
    }

    // ---- Validity ----

    #[test]
    fn validity_contains_inside_window() {
        let v = Validity {
            not_before: 100,
            not_after: 200,
        };
        assert!(v.contains(100));
        assert!(v.contains(150));
        assert!(v.contains(200));
    }

    #[test]
    fn validity_rejects_outside_window() {
        let v = Validity {
            not_before: 100,
            not_after: 200,
        };
        assert!(!v.contains(99));
        assert!(!v.contains(201));
    }

    // ---- PeerCapabilities::merge_update ----

    #[test]
    fn merge_fills_empty_fields() {
        let mut base = PeerCapabilities::default();
        base.merge_update(&caps_secure());
        assert_eq!(base, caps_secure());
    }

    #[test]
    fn merge_preserves_existing_when_update_is_empty() {
        let mut base = caps_secure();
        let orig = base.clone();
        base.merge_update(&PeerCapabilities::default());
        // offered_protection default = None, stronger(Encrypt, None) = Encrypt
        // has_valid_cert: default=false must NOT reset the sticky-true
        assert_eq!(base, orig);
    }

    #[test]
    fn merge_upgrades_protection_monotonically() {
        let mut base = PeerCapabilities {
            offered_protection: ProtectionLevel::Sign,
            ..Default::default()
        };
        let upgrade = PeerCapabilities {
            offered_protection: ProtectionLevel::Encrypt,
            ..Default::default()
        };
        base.merge_update(&upgrade);
        assert_eq!(base.offered_protection, ProtectionLevel::Encrypt);
    }

    #[test]
    fn merge_does_not_downgrade_protection() {
        let mut base = PeerCapabilities {
            offered_protection: ProtectionLevel::Encrypt,
            ..Default::default()
        };
        let downgrade = PeerCapabilities {
            offered_protection: ProtectionLevel::Sign,
            ..Default::default()
        };
        base.merge_update(&downgrade);
        assert_eq!(
            base.offered_protection,
            ProtectionLevel::Encrypt,
            "downgrade must not reduce offered_protection"
        );
    }

    #[test]
    fn merge_has_valid_cert_is_sticky_true() {
        let mut base = PeerCapabilities {
            has_valid_cert: true,
            ..Default::default()
        };
        base.merge_update(&PeerCapabilities {
            has_valid_cert: false,
            ..Default::default()
        });
        assert!(base.has_valid_cert, "sticky: true must not become false");
    }

    #[test]
    fn merge_supported_suites_replaces_when_update_has_any() {
        let mut base = PeerCapabilities {
            supported_suites: alloc::vec![SuiteHint::Aes128Gcm],
            ..Default::default()
        };
        base.merge_update(&PeerCapabilities {
            supported_suites: alloc::vec![SuiteHint::Aes256Gcm, SuiteHint::HmacSha256],
            ..Default::default()
        });
        assert_eq!(
            base.supported_suites,
            alloc::vec![SuiteHint::Aes256Gcm, SuiteHint::HmacSha256]
        );
    }

    // ---- PeerCache ----

    #[test]
    fn cache_new_is_empty() {
        let c = PeerCache::new();
        assert!(c.is_empty());
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn cache_insert_and_get() {
        let mut c = PeerCache::new();
        let key: PeerKey = [1; 12];
        c.insert(key, caps_secure());
        assert_eq!(c.len(), 1);
        assert_eq!(c.get(&key).unwrap(), &caps_secure());
    }

    #[test]
    fn cache_insert_overwrites() {
        let mut c = PeerCache::new();
        let key: PeerKey = [2; 12];
        c.insert(key, PeerCapabilities::default());
        c.insert(key, caps_secure());
        assert_eq!(c.get(&key).unwrap(), &caps_secure());
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn cache_update_partial_inserts_when_missing() {
        let mut c = PeerCache::new();
        let key: PeerKey = [3; 12];
        c.update_partial(key, &caps_secure());
        assert_eq!(c.get(&key).unwrap(), &caps_secure());
    }

    #[test]
    fn cache_update_partial_merges_into_existing() {
        let mut c = PeerCache::new();
        let key: PeerKey = [4; 12];
        c.insert(
            key,
            PeerCapabilities {
                auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".to_string()),
                offered_protection: ProtectionLevel::Sign,
                ..Default::default()
            },
        );
        c.update_partial(
            key,
            &PeerCapabilities {
                offered_protection: ProtectionLevel::Encrypt,
                has_valid_cert: true,
                ..Default::default()
            },
        );
        let merged = c.get(&key).unwrap();
        assert_eq!(merged.offered_protection, ProtectionLevel::Encrypt);
        assert!(merged.has_valid_cert);
        assert_eq!(
            merged.auth_plugin_class.as_deref(),
            Some("DDS:Auth:PKI-DH:1.2"),
            "pre-existing fields are preserved"
        );
    }

    #[test]
    fn cache_forget_removes_and_returns_caps() {
        let mut c = PeerCache::new();
        let key: PeerKey = [5; 12];
        c.insert(key, caps_secure());
        let removed = c.forget(&key);
        assert_eq!(removed, Some(caps_secure()));
        assert!(c.get(&key).is_none());
        assert!(c.is_empty());
    }

    #[test]
    fn cache_forget_unknown_returns_none() {
        let mut c = PeerCache::new();
        assert!(c.forget(&[9; 12]).is_none());
    }

    #[test]
    fn cache_iter_yields_all_entries() {
        let mut c = PeerCache::new();
        c.insert([1; 12], caps_secure());
        c.insert([2; 12], PeerCapabilities::default());
        let count = c.iter().count();
        assert_eq!(count, 2);
    }
}
