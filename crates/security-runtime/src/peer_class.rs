// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Peer-Class-Matching-Engine.
//!
//! Diese Schicht nimmt die aus dem Governance-XML geparsten
//! [`PeerClass`]-Regeln und klassifiziert einen Remote-Peer (repraesentiert
//! durch seine [`PeerCapabilities`]) in die passende Klasse. Die
//! [`GovernancePolicyEngine`](crate::GovernancePolicyEngine) konsultiert
//! das Resultat und gibt das entsprechende Protection-Level aus.
//!
//! # Matching-Regel
//!
//! * Iteration ueber `domain_rule.peer_classes` in XML-Reihenfolge —
//!   **first match wins**.
//! * Eine Peer-Klasse passt, wenn **alle** gesetzten
//!   `match_criteria`-Felder erfuellt sind (UND-Verknuepfung):
//!   * `auth_plugin_class` — exakter String-Vergleich. `""` matcht
//!     `None` im Peer (Legacy-Peer ohne Plugin).
//!   * `cert_cn_pattern` — Wildcard-Vergleich via
//!     [`cn_pattern_match`](zerodds_security_permissions::cn_pattern_match).
//!     Ein Peer ohne Cert-CN matcht nicht.
//!   * `suite` — der Peer muss die Suite in seinen
//!     `supported_suites` listen (CSV-String-Vergleich).
//!   * `require_ocsp` — `PeerCapabilities::has_valid_cert` muss `true`
//!     sein.
//! * Leere `match_criteria` (alle Felder `None`/`false`) matcht
//!   **jeden** Peer — das ist die Default-/Fallback-Klasse, die
//!   typischerweise als letzter Eintrag im XML steht.
//!
//! # Spec-Referenz
//!
//! * `docs/architecture/08_heterogeneous_security.md` §5.
//! * Plan-DoD §Stufe 8: "Governance-Beispiel mit 4 Peer-Classes
//!   (Legacy/Fast/Secure/HA) wird korrekt geparst" — `resolve_peer_class`
//!   ist die Runtime-Verifikation genau dieses Beispiels.

use alloc::collections::BTreeMap;
use alloc::string::String;

use zerodds_security_permissions::{
    DelegationProfile, PeerClass, ProtectionKind, ValidatedChain, cn_pattern_match, validate_chain,
};
use zerodds_security_pki::SignatureAlgorithm;

use crate::caps::PeerCapabilities;
use crate::policy::SuiteHint;

/// Konvertiert einen [`SuiteHint`]-Wert in den String, den
/// `<match suite="...">` erwartet. Muss konsistent mit
/// [`crate::caps_wire`] sein.
fn suite_hint_name(s: SuiteHint) -> &'static str {
    match s {
        SuiteHint::Aes128Gcm => "AES_128_GCM",
        SuiteHint::Aes256Gcm => "AES_256_GCM",
        SuiteHint::HmacSha256 => "HMAC_SHA256",
    }
}

/// Prueft ob eine [`PeerClass`] zu den Peer-Capabilities passt.
#[must_use]
pub fn peer_matches_class(caps: &PeerCapabilities, class: &PeerClass) -> bool {
    let m = &class.match_criteria;

    if let Some(expected) = &m.auth_plugin_class {
        match (expected.as_str(), caps.auth_plugin_class.as_deref()) {
            // Leerer String im XML = Peer ohne Plugin (Legacy).
            ("", None) => {}
            // Sonst muss der Peer genau diesen Plugin-Class-String tragen.
            (_, Some(actual)) if actual == expected => {}
            _ => return false,
        }
    }

    if let Some(pat) = &m.cert_cn_pattern {
        match caps.cert_cn.as_deref() {
            Some(cn) if cn_pattern_match(pat, cn) => {}
            _ => return false,
        }
    }

    if let Some(required) = &m.suite {
        // Der Peer muss die Suite in `supported_suites` anbieten.
        let offers_suite = caps
            .supported_suites
            .iter()
            .any(|s| suite_hint_name(*s) == required.as_str());
        if !offers_suite {
            return false;
        }
    }

    if m.require_ocsp && !caps.has_valid_cert {
        return false;
    }

    // delegation_profile-Check passiert in `peer_matches_class_with_delegation`
    // — der einfache `peer_matches_class` ignoriert das Feld bewusst, damit
    // die RC1-Tests ihren bisherigen Pfad behalten und Caller, die
    // Delegation aktiv nutzen wollen, explizit die erweiterte Funktion
    // aufrufen muessen.
    true
}

/// Erweiterte Variante von [`peer_matches_class`], die zusaetzlich
/// `delegation_profile`-Referenzen aufloest.
///
/// Wenn `class.match_criteria.delegation_profile` gesetzt ist:
/// 1. Der Peer MUSS eine [`DelegationChain`](zerodds_security_pki::DelegationChain)
///    in seinen Capabilities haben.
/// 2. Das referenzierte Profile MUSS in `profiles` existieren.
/// 3. [`validate_chain`] gegen Profile + `now` MUSS Erfolg liefern
///    (Trust-Anchor, Algorithm, Time, Sig, Depth, Scope).
///
/// `pubkey_resolver` wird an [`validate_chain`] durchgereicht und
/// liefert PubKeys fuer Sub-Hop-Delegators (>1-Hop-Chains). Bei
/// 1-Hop-Chain reicht der Trust-Anchor.
///
/// Wenn `delegation_profile` `None` ist, wird der reine
/// [`peer_matches_class`]-Pfad genutzt — keine Chain-Erwartung.
///
/// Output: `Ok(Some(ValidatedChain))` wenn Delegation erfolgreich,
/// `Ok(None)` wenn Class matched aber ohne Delegation-Path,
/// `Err(())` wenn Delegation gefordert aber Validation fehlschlaegt.
pub fn peer_matches_class_with_delegation<F>(
    caps: &PeerCapabilities,
    class: &PeerClass,
    profiles: &BTreeMap<String, DelegationProfile>,
    now: i64,
    pubkey_resolver: F,
) -> Result<Option<ValidatedChain>, &'static str>
where
    F: Fn(&[u8; 16]) -> Option<(alloc::vec::Vec<u8>, SignatureAlgorithm)>,
{
    if !peer_matches_class(caps, class) {
        return Err("class match criteria failed");
    }
    let Some(profile_name) = &class.match_criteria.delegation_profile else {
        // Klasse fordert keine Delegation — direkter Pfad ok.
        return Ok(None);
    };
    let profile = profiles
        .get(profile_name.as_str())
        .ok_or("delegation_profile referenced but not in governance.delegation_profiles")?;
    let chain = caps
        .delegation_chain
        .as_ref()
        .ok_or("class requires delegation_profile but peer has no chain")?;
    validate_chain(chain, profile, now, pubkey_resolver)
        .map(Some)
        .map_err(|_| "delegation chain failed validation")
}

/// Sucht die erste matchende Peer-Klasse fuer einen Peer. Gibt
/// `None` zurueck wenn keine Klasse passt — der Caller faellt dann
/// auf die Domain-Default-Protection zurueck.
#[must_use]
pub fn resolve_peer_class<'a>(
    caps: &PeerCapabilities,
    classes: &'a [PeerClass],
) -> Option<&'a PeerClass> {
    classes.iter().find(|c| peer_matches_class(caps, c))
}

/// Extrahiert das Protection-Level einer matchenden Peer-Class.
/// `None` wenn keine Klasse matched.
#[must_use]
pub fn resolve_protection(
    caps: &PeerCapabilities,
    classes: &[PeerClass],
) -> Option<ProtectionKind> {
    resolve_peer_class(caps, classes).map(|c| c.protection)
}

/// Erlaubt-Liste-Check: darf ein Peer einer bestimmten Klasse auf
/// dem gegebenen Interface kommunizieren?
///
/// `peer_class_filter` ist leer → jede Klasse ist erlaubt.
/// Sonst muss der `class_name` in der Filterliste auftauchen.
#[must_use]
pub fn interface_accepts_class(class_name: &str, peer_class_filter: &[String]) -> bool {
    peer_class_filter.is_empty() || peer_class_filter.iter().any(|f| f == class_name)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use zerodds_security_permissions::{PeerClass, PeerClassMatch, ProtectionKind};

    fn legacy_class() -> PeerClass {
        PeerClass {
            name: "legacy".into(),
            protection: ProtectionKind::None,
            match_criteria: PeerClassMatch {
                auth_plugin_class: Some(String::new()),
                ..Default::default()
            },
        }
    }

    fn fast_class() -> PeerClass {
        PeerClass {
            name: "fast".into(),
            protection: ProtectionKind::Sign,
            match_criteria: PeerClassMatch {
                cert_cn_pattern: Some("*.fast.example".into()),
                ..Default::default()
            },
        }
    }

    fn secure_class() -> PeerClass {
        PeerClass {
            name: "secure".into(),
            protection: ProtectionKind::Encrypt,
            match_criteria: PeerClassMatch {
                auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
                suite: Some("AES_128_GCM".into()),
                ..Default::default()
            },
        }
    }

    fn ha_class() -> PeerClass {
        PeerClass {
            name: "highassurance".into(),
            protection: ProtectionKind::Encrypt,
            match_criteria: PeerClassMatch {
                cert_cn_pattern: Some("*.ha.*".into()),
                suite: Some("AES_256_GCM".into()),
                require_ocsp: true,
                ..Default::default()
            },
        }
    }

    fn legacy_caps() -> PeerCapabilities {
        PeerCapabilities::default()
    }

    fn fast_caps() -> PeerCapabilities {
        // Fast-Peer: hat Auth-Plugin (sonst wuerde legacy zuerst
        // matchen wegen leerem auth_plugin_class=""), aber nutzt HMAC-
        // only + hat einen Cert-CN im .fast.example-Namensraum.
        PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            cert_cn: Some("writer1.fast.example".into()),
            supported_suites: alloc::vec![SuiteHint::HmacSha256],
            ..Default::default()
        }
    }

    fn secure_caps() -> PeerCapabilities {
        PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            supported_suites: alloc::vec![SuiteHint::Aes128Gcm],
            ..Default::default()
        }
    }

    fn ha_caps() -> PeerCapabilities {
        PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            cert_cn: Some("writer.ha.corp".into()),
            supported_suites: alloc::vec![SuiteHint::Aes256Gcm],
            has_valid_cert: true,
            ..Default::default()
        }
    }

    // ---- peer_matches_class ----

    #[test]
    fn legacy_caps_match_legacy_class() {
        assert!(peer_matches_class(&legacy_caps(), &legacy_class()));
    }

    #[test]
    fn fast_caps_match_fast_cn_pattern() {
        assert!(peer_matches_class(&fast_caps(), &fast_class()));
    }

    #[test]
    fn secure_caps_need_both_auth_and_suite() {
        assert!(peer_matches_class(&secure_caps(), &secure_class()));

        // Peer mit Auth aber ohne Suite → kein Match.
        let only_auth = PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            supported_suites: alloc::vec![],
            ..Default::default()
        };
        assert!(!peer_matches_class(&only_auth, &secure_class()));

        // Peer mit Suite aber falschem Plugin → kein Match.
        let wrong_auth = PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:Custom".into()),
            supported_suites: alloc::vec![SuiteHint::Aes128Gcm],
            ..Default::default()
        };
        assert!(!peer_matches_class(&wrong_auth, &secure_class()));
    }

    #[test]
    fn ha_caps_need_ocsp() {
        assert!(peer_matches_class(&ha_caps(), &ha_class()));

        // Gleiche Caps, aber has_valid_cert=false → kein Match.
        let no_ocsp = PeerCapabilities {
            has_valid_cert: false,
            ..ha_caps()
        };
        assert!(!peer_matches_class(&no_ocsp, &ha_class()));
    }

    #[test]
    fn peer_without_cn_does_not_match_cn_pattern_class() {
        assert!(!peer_matches_class(&legacy_caps(), &fast_class()));
    }

    #[test]
    fn empty_match_criteria_matches_every_peer() {
        let fallback = PeerClass {
            name: "fallback".into(),
            protection: ProtectionKind::Sign,
            match_criteria: PeerClassMatch::default(),
        };
        assert!(peer_matches_class(&legacy_caps(), &fallback));
        assert!(peer_matches_class(&fast_caps(), &fallback));
        assert!(peer_matches_class(&secure_caps(), &fallback));
    }

    #[test]
    fn legacy_class_rejects_peer_with_plugin() {
        // auth_plugin_class="" matcht NUR Peers ohne Plugin.
        let secured = secure_caps();
        assert!(!peer_matches_class(&secured, &legacy_class()));
    }

    // ---- resolve_peer_class (first-match-wins) ----

    #[test]
    fn resolve_peer_class_first_match_wins() {
        let classes = alloc::vec![legacy_class(), fast_class(), secure_class(), ha_class(),];

        assert_eq!(
            resolve_peer_class(&legacy_caps(), &classes).map(|c| c.name.as_str()),
            Some("legacy")
        );
        assert_eq!(
            resolve_peer_class(&fast_caps(), &classes).map(|c| c.name.as_str()),
            Some("fast")
        );
        assert_eq!(
            resolve_peer_class(&secure_caps(), &classes).map(|c| c.name.as_str()),
            Some("secure")
        );
        assert_eq!(
            resolve_peer_class(&ha_caps(), &classes).map(|c| c.name.as_str()),
            Some("highassurance")
        );
    }

    #[test]
    fn resolve_peer_class_no_match_returns_none() {
        // Caps mit ausschliesslich cert_cn der weder fast noch ha
        // matcht, kein Plugin, keine Suite.
        let caps = PeerCapabilities {
            cert_cn: Some("misc.corp".into()),
            ..Default::default()
        };
        let classes = alloc::vec![fast_class(), secure_class(), ha_class()];
        assert!(resolve_peer_class(&caps, &classes).is_none());
    }

    #[test]
    fn resolve_protection_maps_to_class_protection() {
        let classes = alloc::vec![legacy_class(), secure_class()];
        assert_eq!(
            resolve_protection(&legacy_caps(), &classes),
            Some(ProtectionKind::None)
        );
        assert_eq!(
            resolve_protection(&secure_caps(), &classes),
            Some(ProtectionKind::Encrypt)
        );
    }

    // ---- interface_accepts_class ----

    #[test]
    fn interface_accepts_any_class_when_filter_empty() {
        assert!(interface_accepts_class("legacy", &[]));
        assert!(interface_accepts_class("highassurance", &[]));
    }

    #[test]
    fn interface_accepts_only_listed_classes() {
        let filter = alloc::vec!["secure".into(), "highassurance".into()];
        assert!(interface_accepts_class("secure", &filter));
        assert!(interface_accepts_class("highassurance", &filter));
        assert!(!interface_accepts_class("legacy", &filter));
        assert!(!interface_accepts_class("fast", &filter));
    }

    // ---- RC1: peer_matches_class_with_delegation ----

    use zerodds_security_permissions::{DelegationProfile, TrustAnchor};
    use zerodds_security_pki::{DelegationChain, DelegationLink};

    fn make_chain_signed_by(
        gw: [u8; 16],
        edge: [u8; 16],
        topics: &[&str],
    ) -> (DelegationChain, alloc::vec::Vec<u8>) {
        use ring::rand::SystemRandom;
        use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};
        let rng = SystemRandom::new();
        let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
        let sk = pkcs8.as_ref().to_vec();
        let kp = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &sk, &rng).unwrap();
        let pk = kp.public_key().as_ref().to_vec();
        let mut link = DelegationLink::new(
            gw,
            edge,
            topics.iter().map(|s| s.to_string()).collect(),
            alloc::vec![],
            1_000,
            9_000,
            SignatureAlgorithm::EcdsaP256,
        )
        .unwrap();
        link.sign(&sk).unwrap();
        (DelegationChain::new(gw, alloc::vec![link]).unwrap(), pk)
    }

    fn delegated_class(profile_name: &str) -> PeerClass {
        PeerClass {
            name: "delegated-edge".into(),
            protection: ProtectionKind::Encrypt,
            match_criteria: PeerClassMatch {
                auth_plugin_class: Some(String::new()), // edge ohne eigenes plugin
                delegation_profile: Some(profile_name.into()),
                ..Default::default()
            },
        }
    }

    #[test]
    fn delegation_class_match_with_valid_chain() {
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let (chain, pk) = make_chain_signed_by(gw, edge, &["sensor/*"]);

        let mut profiles = BTreeMap::new();
        profiles.insert(
            "vehicle-edges".to_string(),
            DelegationProfile::default_with_anchor(
                "vehicle-edges".to_string(),
                TrustAnchor {
                    subject_guid: gw,
                    verify_public_key: pk,
                    algorithm: SignatureAlgorithm::EcdsaP256,
                },
            ),
        );

        let caps = PeerCapabilities {
            delegation_chain: Some(chain),
            ..Default::default()
        };
        let class = delegated_class("vehicle-edges");
        let result = peer_matches_class_with_delegation(&caps, &class, &profiles, 5_000, |_| None);
        let validated = result.expect("must validate").expect("chain produced");
        assert_eq!(validated.edge_guid, edge);
        assert_eq!(validated.chain_depth, 1);
    }

    #[test]
    fn delegation_class_rejects_peer_without_chain() {
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let (_chain, pk) = make_chain_signed_by(gw, edge, &["sensor/*"]);

        let mut profiles = BTreeMap::new();
        profiles.insert(
            "vehicle-edges".to_string(),
            DelegationProfile::default_with_anchor(
                "vehicle-edges".to_string(),
                TrustAnchor {
                    subject_guid: gw,
                    verify_public_key: pk,
                    algorithm: SignatureAlgorithm::EcdsaP256,
                },
            ),
        );

        let caps = PeerCapabilities {
            delegation_chain: None,
            ..Default::default()
        };
        let class = delegated_class("vehicle-edges");
        let err = peer_matches_class_with_delegation(&caps, &class, &profiles, 5_000, |_| None)
            .expect_err("must fail");
        assert!(err.contains("no chain"));
    }

    #[test]
    fn delegation_class_rejects_unknown_profile_reference() {
        let caps = PeerCapabilities::default();
        let class = delegated_class("nonexistent-profile");
        let profiles: BTreeMap<String, DelegationProfile> = BTreeMap::new();
        let err = peer_matches_class_with_delegation(&caps, &class, &profiles, 5_000, |_| None)
            .expect_err("must fail");
        assert!(err.contains("not in governance"));
    }

    #[test]
    fn delegation_class_rejects_invalid_chain() {
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let (chain, _pk_correct) = make_chain_signed_by(gw, edge, &["sensor/*"]);
        // Anchor mit falschem PubKey → Validation fehlschlaegt.
        let mut profiles = BTreeMap::new();
        profiles.insert(
            "vehicle-edges".to_string(),
            DelegationProfile::default_with_anchor(
                "vehicle-edges".to_string(),
                TrustAnchor {
                    subject_guid: gw,
                    verify_public_key: alloc::vec![0u8; 65],
                    algorithm: SignatureAlgorithm::EcdsaP256,
                },
            ),
        );

        let caps = PeerCapabilities {
            delegation_chain: Some(chain),
            ..Default::default()
        };
        let class = delegated_class("vehicle-edges");
        let err = peer_matches_class_with_delegation(&caps, &class, &profiles, 5_000, |_| None)
            .expect_err("must fail");
        assert!(err.contains("validation"));
    }

    #[test]
    fn class_without_delegation_profile_returns_ok_none() {
        // Direkt-Auth-Pfad: Klasse ohne delegation_profile.
        let caps = PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            supported_suites: alloc::vec![SuiteHint::Aes128Gcm],
            ..Default::default()
        };
        let class = secure_class();
        let profiles: BTreeMap<String, DelegationProfile> = BTreeMap::new();
        let result = peer_matches_class_with_delegation(&caps, &class, &profiles, 5_000, |_| None);
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn delegation_class_rejects_chain_outside_time_window() {
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let (chain, pk) = make_chain_signed_by(gw, edge, &["sensor/*"]);

        let mut profiles = BTreeMap::new();
        profiles.insert(
            "vehicle-edges".to_string(),
            DelegationProfile::default_with_anchor(
                "vehicle-edges".to_string(),
                TrustAnchor {
                    subject_guid: gw,
                    verify_public_key: pk,
                    algorithm: SignatureAlgorithm::EcdsaP256,
                },
            ),
        );

        let caps = PeerCapabilities {
            delegation_chain: Some(chain),
            ..Default::default()
        };
        let class = delegated_class("vehicle-edges");
        // now = 50_000 ist weit nach not_after=9_000
        let err = peer_matches_class_with_delegation(&caps, &class, &profiles, 50_000, |_| None)
            .expect_err("must fail");
        assert!(err.contains("validation"));
    }
}
