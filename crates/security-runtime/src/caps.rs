// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Peer-Capabilities und -Cache.
//!
//! [`PeerCapabilities`] ist der Snapshot dessen, was ein Remote-
//! Participant ueber seine Security-Faehigkeiten kommuniziert hat
//! (Auth-/Crypto-/Access-Plugin-Class, akzeptierte Suites,
//! angebotenes Protection-Level, Validity-Window etc.). In RC1
//! wird dieser Snapshot aus SPDP-Properties befuellt; hier
//! definieren wir nur das Datenmodell plus einen in-memory-Cache
//! fuer die Runtime.
//!
//! Der Cache ist bewusst ein schlanker [`alloc::collections::BTreeMap`]-
//! Wrapper: Peers kommen und gehen, Lookups haeufig, das typische
//! Peer-Setup hat dutzende bis niedrige Tausender von Eintraegen
//! — also keine Hash-Overhead-Diskussion.
//!
//! Siehe `docs/architecture/08_heterogeneous_security.md` §3.2
//! und §4.3 (Upgrade-Pfad per `update_partial`).

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_security_pki::DelegationChain;

use crate::policy::{ProtectionLevel, SuiteHint};
use crate::shared::PeerKey;

/// Gueltigkeitsfenster einer Peer-Identity (Unix-Seconds).
///
/// Minimal-Repraesentation ohne chrono-Dep — die konkreten
/// Zeitvergleiche passieren im Authentication-Plugin. Diese Struct
/// dient als transparenter Durchreich-Container in den
/// [`PeerCapabilities`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Validity {
    /// Nicht gueltig vor diesem Zeitpunkt (Unix-Seconds).
    pub not_before: i64,
    /// Nicht gueltig nach diesem Zeitpunkt (Unix-Seconds).
    pub not_after: i64,
}

impl Validity {
    /// Prueft, ob `now` im Validity-Fenster liegt (inklusiv).
    #[must_use]
    pub const fn contains(&self, now: i64) -> bool {
        now >= self.not_before && now <= self.not_after
    }
}

/// Security-relevante Capabilities eines Remote-Peers.
///
/// Wird aus SPDP-Properties (Auth-/Crypto-/Access-Plugin-Class,
/// `zerodds.sec.supported_suites`, `zerodds.sec.offered_protection`)
/// sowie aus SEDP-Permissions-Tokens befuellt. Legacy-Peers ohne
/// Security-Properties landen mit `auth_plugin_class=None` hier —
/// kein Drop, die [`crate::PolicyEngine`] entscheidet pro
/// Domain-Rule, ob Legacy akzeptiert wird.
///
/// Alle Felder sind `Option`-/`Vec`-basiert, damit Partial-Updates
/// (Upgrade-Pfad in §4.3 der Architektur-Doc) sauber moeglich sind.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PeerCapabilities {
    /// `DDS:Auth:PKI-DH:1.2` (Spec 1.2 §10.3.2.1) etc. `None` = Legacy-
    /// Peer ohne Auth-Plugin.
    pub auth_plugin_class: Option<String>,
    /// `DDS:Crypto:AES-GCM-GMAC:1.2` (Spec 1.2 §10.5) etc.
    pub crypto_plugin_class: Option<String>,
    /// `DDS:Access:Permissions:1.2` (Spec 1.2 §10.4) etc.
    pub access_plugin_class: Option<String>,
    /// Suites, die der Peer laut SPDP-Annonce akzeptieren wuerde.
    pub supported_suites: Vec<SuiteHint>,
    /// Protection-Level, das der Peer **selbst** anbietet.
    pub offered_protection: ProtectionLevel,
    /// `true` wenn Cert-Chain + OCSP geprueft und ok — wird vom
    /// Authentication-Plugin gesetzt, nicht aus SPDP.
    pub has_valid_cert: bool,
    /// Validity-Window aus dem Permissions-Token.
    pub validity_window: Option<Validity>,
    /// Vendor-Identifikation (z.B. `"Cyclone DDS"`, `"Fast DDS"`)
    /// fuer Quirks.
    pub vendor_hint: Option<String>,
    /// Subject-Common-Name aus dem Peer-Cert (z.B.
    /// `"writer1.fast.example"`). Wird vom Authentication-Plugin nach
    /// erfolgreichem Handshake gesetzt; **nicht** via SPDP propagiert.
    /// Genutzt fuer `<zerodds:peer_class><match cert_cn_pattern=...>`
    ///.
    pub cert_cn: Option<String>,
    /// Delegation-Chain. Wird vom Edge- oder Sub-Gateway
    /// via SPDP-Property `zerodds.sec.delegation_chain` propagiert.
    /// Validation gegen ein Delegation-Profile passiert in
    /// `peer_matches_class` (j-d). `None` = Peer ohne Chain (= direkt
    /// authentifizierter Peer oder Legacy).
    pub delegation_chain: Option<DelegationChain>,
}

impl PeerCapabilities {
    /// Mischt nicht-leere Felder aus `other` in `self`. Leere Felder
    /// (`None`, `[]`) bleiben unveraendert — damit sind mehrere
    /// partielle SPDP-Updates idempotent und reihenfolge-tolerant.
    ///
    /// Sonderregeln:
    /// * `offered_protection` wird immer uebernommen (monoton steigend
    ///   via [`ProtectionLevel::stronger`]) — ein Peer kann sein
    ///   Level upgraden, aber nicht still herunterstufen.
    /// * `has_valid_cert=true` ist sticky: einmal validiert, kann es
    ///   nicht zu `false` zurueckfallen (Cert-Rotation erfordert
    ///   explizites [`PeerCache::forget`]).
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

/// In-memory-Cache `PeerKey → PeerCapabilities`.
///
/// Eine Instanz pro Participant; gelebte Lebensdauer entspricht dem
/// Participant selbst. Thread-Safety liefert der Aufrufer via
/// `Arc<Mutex<PeerCache>>` oder `Arc<RwLock<PeerCache>>` — der Cache
/// selbst ist intentional **nicht** thread-safe, weil pro-Tick
/// meist nur eine Writer-/Reader-Task ihn anfasst.
#[derive(Debug, Default, Clone)]
pub struct PeerCache {
    inner: BTreeMap<PeerKey, PeerCapabilities>,
}

impl PeerCache {
    /// Leeren Cache bauen.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Anzahl bekannter Peers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// `true` wenn kein Peer bekannt.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Vollstaendiger Upsert. Ueberschreibt einen vorhandenen Eintrag
    /// mit den neuen Capabilities. Fuer Teil-Updates
    /// [`Self::update_partial`] nehmen.
    pub fn insert(&mut self, key: PeerKey, caps: PeerCapabilities) {
        self.inner.insert(key, caps);
    }

    /// Liest die Capabilities eines Peers.
    #[must_use]
    pub fn get(&self, key: &PeerKey) -> Option<&PeerCapabilities> {
        self.inner.get(key)
    }

    /// Merged neue Informationen in den existierenden Eintrag.
    /// Ist der Peer neu → wird er eingefuegt.
    ///
    /// Semantik siehe [`PeerCapabilities::merge_update`].
    pub fn update_partial(&mut self, key: PeerKey, update: &PeerCapabilities) {
        self.inner
            .entry(key)
            .and_modify(|existing| existing.merge_update(update))
            .or_insert_with(|| update.clone());
    }

    /// Entfernt einen Peer aus dem Cache. Liefert die alten Caps
    /// falls vorhanden — praktisch fuers Logging bei Session-End.
    pub fn forget(&mut self, key: &PeerKey) -> Option<PeerCapabilities> {
        self.inner.remove(key)
    }

    /// Iterator ueber alle `(PeerKey, PeerCapabilities)`-Paare.
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
        // has_valid_cert: default=false darf sticky-true NICHT zuruecksetzen
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
            "downgrade darf offered_protection nicht reduzieren"
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
        assert!(
            base.has_valid_cert,
            "sticky: true darf nicht zu false werden"
        );
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
            "pre-existing Felder bleiben erhalten"
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
