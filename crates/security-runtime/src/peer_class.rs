// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Peer-class matching engine.
//!
//! This layer takes the [`PeerClass`] rules parsed from the
//! governance XML and classifies a remote peer (represented
//! by its [`PeerCapabilities`]) into the matching class. The
//! [`GovernancePolicyEngine`](crate::GovernancePolicyEngine) consults
//! the result and emits the corresponding protection level.
//!
//! # Matching rule
//!
//! * Iteration over `domain_rule.peer_classes` in XML order —
//!   **first match wins**.
//! * A peer class matches if **all** set
//!   `match_criteria` fields are satisfied (AND combination):
//!   * `auth_plugin_class` — exact string comparison. `""` matches
//!     `None` in the peer (legacy peer without a plugin).
//!   * `cert_cn_pattern` — wildcard comparison via
//!     [`cn_pattern_match`](zerodds_security_permissions::cn_pattern_match).
//!     A peer without a cert CN does not match.
//!   * `suite` — the peer must list the suite in its
//!     `supported_suites` (CSV string comparison).
//!   * `require_ocsp` — `PeerCapabilities::has_valid_cert` must be `true`.
//! * Empty `match_criteria` (all fields `None`/`false`) matches
//!   **every** peer — this is the default/fallback class, which
//!   typically is the last entry in the XML.
//!
//! # Spec reference
//!
//! * `docs/architecture/08_heterogeneous_security.md` §5.
//! * Plan DoD §stage 8: "governance example with 4 peer classes
//!   (legacy/fast/secure/HA) is parsed correctly" — `resolve_peer_class`
//!   is the runtime verification of exactly this example.

use alloc::collections::BTreeMap;
use alloc::string::String;

use zerodds_security_permissions::{
    DelegationProfile, PeerClass, ProtectionKind, ValidatedChain, cn_pattern_match, validate_chain,
};
use zerodds_security_pki::SignatureAlgorithm;

use crate::caps::PeerCapabilities;
use crate::policy::SuiteHint;

/// Converts a [`SuiteHint`] value into the string that
/// `<match suite="...">` expects. Must be consistent with
/// [`crate::caps_wire`].
fn suite_hint_name(s: SuiteHint) -> &'static str {
    match s {
        SuiteHint::Aes128Gcm => "AES_128_GCM",
        SuiteHint::Aes256Gcm => "AES_256_GCM",
        SuiteHint::HmacSha256 => "HMAC_SHA256",
    }
}

/// Checks whether a [`PeerClass`] matches the peer capabilities.
#[must_use]
pub fn peer_matches_class(caps: &PeerCapabilities, class: &PeerClass) -> bool {
    let m = &class.match_criteria;

    if let Some(expected) = &m.auth_plugin_class {
        match (expected.as_str(), caps.auth_plugin_class.as_deref()) {
            // Empty string in the XML = peer without a plugin (legacy).
            ("", None) => {}
            // Otherwise the peer must carry exactly this plugin-class string.
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
        // The peer must offer the suite in `supported_suites`.
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

    // The delegation_profile check happens in `peer_matches_class_with_delegation`
    // — the plain `peer_matches_class` deliberately ignores the field, so
    // the RC1 tests keep their existing path and callers that want to
    // actively use delegation must explicitly call the extended function.
    true
}

/// Extended variant of [`peer_matches_class`] that additionally
/// resolves `delegation_profile` references.
///
/// If `class.match_criteria.delegation_profile` is set:
/// 1. The peer MUST have a [`DelegationChain`](zerodds_security_pki::DelegationChain)
///    in its capabilities.
/// 2. The referenced profile MUST exist in `profiles`.
/// 3. [`validate_chain`] against the profile + `now` MUST succeed
///    (trust anchor, algorithm, time, sig, depth, scope).
///
/// `pubkey_resolver` is passed through to [`validate_chain`] and
/// provides pubkeys for sub-hop delegators (>1-hop chains). For a
/// 1-hop chain the trust anchor suffices.
///
/// If `delegation_profile` is `None`, the plain
/// [`peer_matches_class`] path is used — no chain expectation.
///
/// Output: `Ok(Some(ValidatedChain))` if delegation succeeded,
/// `Ok(None)` if the class matched but without a delegation path,
/// `Err(())` if delegation was required but validation fails.
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
        // The class requires no delegation — direct path ok.
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

/// Finds the first matching peer class for a peer. Returns
/// `None` if no class matches — the caller then falls back
/// to the domain default protection.
#[must_use]
pub fn resolve_peer_class<'a>(
    caps: &PeerCapabilities,
    classes: &'a [PeerClass],
) -> Option<&'a PeerClass> {
    classes.iter().find(|c| peer_matches_class(caps, c))
}

/// Extracts the protection level of a matching peer class.
/// `None` if no class matched.
#[must_use]
pub fn resolve_protection(
    caps: &PeerCapabilities,
    classes: &[PeerClass],
) -> Option<ProtectionKind> {
    resolve_peer_class(caps, classes).map(|c| c.protection)
}

/// Allow-list check: may a peer of a given class communicate on
/// the given interface?
///
/// `peer_class_filter` is empty → every class is allowed.
/// Otherwise the `class_name` must appear in the filter list.
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
        // Fast peer: has an auth plugin (otherwise legacy would match
        // first due to the empty auth_plugin_class=""), but uses HMAC
        // only + has a cert CN in the .fast.example namespace.
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

        // Peer with auth but without a suite → no match.
        let only_auth = PeerCapabilities {
            auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
            supported_suites: alloc::vec![],
            ..Default::default()
        };
        assert!(!peer_matches_class(&only_auth, &secure_class()));

        // Peer with a suite but the wrong plugin → no match.
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

        // Same caps but has_valid_cert=false → no match.
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
        // auth_plugin_class="" matches ONLY peers without a plugin.
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
        // Caps with only a cert_cn that matches neither fast nor ha,
        // no plugin, no suite.
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
                auth_plugin_class: Some(String::new()), // edge without its own plugin
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
        // Anchor with the wrong pubkey → validation fails.
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
        // Direct auth path: class without delegation_profile.
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
        // now = 50_000 is well after not_after=9_000
        let err = peer_matches_class_with_delegation(&caps, &class, &profiles, 50_000, |_| None)
            .expect_err("must fail");
        assert!(err.contains("validation"));
    }
}
