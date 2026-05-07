// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Delegation-Chain-Validation fuer Permissions-Sub-CAs.
//!
//! Implementiert die 7-Punkte-Validation aus
//! `docs/architecture/09_delegation.md` §6:
//!
//! 1. **Chain-Kontinuitaet** — `links[i].delegatee_guid` muss
//!    `links[i+1].delegator_guid` entsprechen.
//! 2. **Origin-Match** — `chain.origin_guid` muss `links[0].delegator_guid`
//!    entsprechen.
//! 3. **Trust-Anchor-Match** — abhaengig vom [`TrustPolicy`]-Mode wird
//!    der Origin-Delegator gegen einen oder mehrere Trust-Anchors
//!    geprueft.
//! 4. **Signatur-Kette** — jeder Link wird gegen den **vorigen
//!    Delegatee-PubKey** verifiziert (Initial-Link gegen
//!    Trust-Anchor-PubKey). Damit kann ein kompromittierter
//!    Zwischen-Gateway nicht beliebig nach oben skalieren.
//! 5. **Zeitfenster** — `link.not_before <= now <= link.not_after`
//!    fuer **jeden** Link.
//! 6. **Max-Chain-Depth** — `chain.depth() <= profile.max_chain_depth`.
//! 7. **Scope-Intersection** — die effektive Topic-/Partition-Pattern-
//!    Liste ist der Schnitt aller Pattern-Listen entlang der Kette.
//!    Dadurch kann ein zwischengeschalteter Gateway den Scope nur
//!    **enger** ziehen, nie weiter.
//!
//! Output ist [`ValidatedChain`] — wird vom Caller (j-d
//! `peer_matches_class`) als Berechtigung-Pass an den Permissions-Plugin
//! weitergegeben.

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_security_pki::{DelegationChain, SignatureAlgorithm};

use crate::topic_match::topic_match;

/// Trust-Policy-Mode (Architektur §4).
///
/// Bestimmt, wie [`validate_chain`] den Origin-Delegator gegen
/// Trust-Anchors prueft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TrustPolicy {
    /// Gateway-Only: Origin-Cert MUSS exakt mit dem konfigurierten
    /// Gateway-Cert uebereinstimmen. Kein Multi-Hop ueber andere
    /// Gateways. (Default fuer Vehicle-intern.)
    GatewayOnly,
    /// Direct-Or-Delegated: Peer wird **entweder** direkt akzeptiert
    /// (regulaere PKI-Auth, ohne Chain) **oder** ueber Delegation.
    /// Hop-Anzahl darf bis `profile.max_chain_depth`. (Default fuer
    /// Vehicle-↔C4I.)
    DirectOrDelegated,
    /// Federation: mehrere Trust-Anchors (alle Gateways untereinander
    /// peer-ed). Origin-Delegator MUSS in der Trust-Anchor-Liste sein.
    Federation,
    /// Strict-Delegated: ausschliesslich Delegation zugelassen — kein
    /// direkter Auth-Pfad. Sinnvoll fuer C4I-Backends, die keine
    /// Vehicle-Edges direkt zulassen wollen.
    StrictDelegated,
}

/// Trust-Anchor — Public-Key-DER + Algorithmus + Subject-GUID.
///
/// Die `subject_guid` ist die GUID des Trust-Anchors (typisch das
/// Wanne-Gateway oder C4I-Root). `verify_public_key` ist der DER-Bytes-
/// Format-PubKey, mit dem [`DelegationLink::verify`] aufgerufen wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustAnchor {
    /// 16-byte GUID des Trust-Anchors.
    pub subject_guid: [u8; 16],
    /// PubKey-DER-Bytes (algorithm-spezifisches Format, siehe
    /// [`DelegationLink::verify`]).
    pub verify_public_key: Vec<u8>,
    /// Erwarteter Signatur-Algorithmus dieses Anchors.
    pub algorithm: SignatureAlgorithm,
}

/// Delegation-Profile (minimal — Voll-Definition kommt in j-h aus
/// Governance-XML).
///
/// Profile = Konfigurations-Bundle, das Trust-Policy + erlaubte
/// Algorithmen + max-Chain-Depth definiert. Wird per Name in
/// `PeerClassMatch::delegation_profile` referenziert (j-d).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegationProfile {
    /// Name des Profiles (Governance-XML-Referenz).
    pub name: String,
    /// Trust-Policy-Modus.
    pub trust_policy: TrustPolicy,
    /// Erlaubte Trust-Anchors. Bei `GatewayOnly` muss exakt 1 Eintrag
    /// drin sein; bei `Federation` >=1.
    pub trust_anchors: Vec<TrustAnchor>,
    /// Maximale Chain-Tiefe (zusaetzlich zum hard-cap aus PKI-Crate).
    /// Default 3.
    pub max_chain_depth: usize,
    /// Erlaubte Signatur-Algorithmen. Andere → Reject.
    pub allowed_algorithms: BTreeSet<u8>, // SignatureAlgorithm::wire_id
    /// Wenn true: Profil verlangt OCSP-Liveness-Check fuer
    /// Trust-Anchor-Cert. Wird in j-h gegen das Governance-XML
    /// gehaengt; in j-b ist das Feld nur ein Marker.
    pub require_ocsp: bool,
}

impl DelegationProfile {
    /// Convenience-Konstruktor mit `max_chain_depth=3`,
    /// `trust_policy=DirectOrDelegated`, alle 4 Algorithmen erlaubt.
    #[must_use]
    pub fn default_with_anchor(name: String, anchor: TrustAnchor) -> Self {
        let mut algos = BTreeSet::new();
        for a in [
            SignatureAlgorithm::EcdsaP256,
            SignatureAlgorithm::EcdsaP384,
            SignatureAlgorithm::RsaPss2048,
            SignatureAlgorithm::Ed25519,
        ] {
            algos.insert(a.wire_id());
        }
        Self {
            name,
            trust_policy: TrustPolicy::DirectOrDelegated,
            trust_anchors: alloc::vec![anchor],
            max_chain_depth: 3,
            allowed_algorithms: algos,
            require_ocsp: false,
        }
    }
}

/// Errors aus der Chain-Validation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DelegationCheckError {
    /// Chain ist leer.
    EmptyChain,
    /// `links[i].delegatee_guid != links[i+1].delegator_guid`.
    ChainBroken {
        /// Index des fehlerhaften Links (i).
        index: usize,
    },
    /// `chain.origin_guid != links[0].delegator_guid`.
    OriginMismatch,
    /// Origin-Delegator entspricht keinem Trust-Anchor.
    UntrustedDelegator,
    /// Link-Signatur ist ungueltig.
    SignatureInvalid {
        /// Index des Links.
        index: usize,
        /// Diagnose-String aus dem PKI-Crate.
        reason: String,
    },
    /// Link ausserhalb seines Zeitfensters.
    LinkExpired {
        /// Index des Links.
        index: usize,
        /// Aktueller Zeit-Tick.
        now: i64,
        /// `link.not_before`.
        not_before: i64,
        /// `link.not_after`.
        not_after: i64,
    },
    /// Chain ist tiefer als `profile.max_chain_depth`.
    ChainTooDeep {
        /// Tatsaechliche Tiefe.
        depth: usize,
        /// Profil-Limit.
        max: usize,
    },
    /// Verwendeter Signatur-Algorithmus ist nicht in
    /// `profile.allowed_algorithms`.
    AlgorithmRejected {
        /// Index des Links.
        index: usize,
        /// Algorithm-Wire-Id.
        algorithm: u8,
    },
    /// Profil verlangt mindestens einen Trust-Anchor, hat aber keinen.
    NoTrustAnchor,
    /// Trust-Anchor-Liste hat einen Eintrag mit Algorithm-Mismatch zum
    /// Initial-Link (Defensive-Check).
    AnchorAlgorithmMismatch,
}

impl core::fmt::Display for DelegationCheckError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyChain => write!(f, "delegation chain is empty"),
            Self::ChainBroken { index } => write!(f, "chain broken at link {index}"),
            Self::OriginMismatch => write!(f, "origin_guid != links[0].delegator_guid"),
            Self::UntrustedDelegator => write!(f, "origin delegator not in trust anchors"),
            Self::SignatureInvalid { index, reason } => {
                write!(f, "link {index} signature invalid: {reason}")
            }
            Self::LinkExpired {
                index,
                now,
                not_before,
                not_after,
            } => write!(
                f,
                "link {index} expired (now={now}, window=[{not_before}, {not_after}])"
            ),
            Self::ChainTooDeep { depth, max } => write!(f, "chain depth {depth} > max {max}"),
            Self::AlgorithmRejected { index, algorithm } => {
                write!(f, "link {index} algorithm {algorithm} rejected by profile")
            }
            Self::NoTrustAnchor => write!(f, "profile has no trust anchors"),
            Self::AnchorAlgorithmMismatch => {
                write!(f, "trust anchor algorithm differs from initial link")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DelegationCheckError {}

/// Result-Alias.
pub type DelegationCheckResult<T> = Result<T, DelegationCheckError>;

/// Validierte Chain — Output von [`validate_chain`].
///
/// Effektive Pattern-Listen sind das Ergebnis der Scope-Intersection
/// aller Links: `effective = intersect(links[0].patterns, ..., links[N-1].patterns)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedChain {
    /// 16-byte GUID des Origin-Participants.
    pub origin_guid: [u8; 16],
    /// 16-byte GUID des Edge-Peers (= letzter Delegatee).
    pub edge_guid: [u8; 16],
    /// Tatsaechliche Chain-Tiefe.
    pub chain_depth: usize,
    /// Effektive Topic-Patterns (Intersection ueber alle Links).
    pub effective_topic_patterns: Vec<String>,
    /// Effektive Partition-Patterns (Intersection ueber alle Links).
    pub effective_partition_patterns: Vec<String>,
}

impl ValidatedChain {
    /// True wenn `topic_name` von der effektiven Pattern-Liste
    /// abgedeckt wird. Empty-List = keine Topic-Whitelist (Match
    /// false — explizit, sicherer Default).
    #[must_use]
    pub fn allows_topic(&self, topic_name: &str) -> bool {
        if self.effective_topic_patterns.is_empty() {
            return false;
        }
        self.effective_topic_patterns
            .iter()
            .any(|p| topic_match(p, topic_name))
    }

    /// True wenn `partition_name` von der effektiven Partition-Pattern-
    /// Liste abgedeckt wird. Empty-List = nur Default-Partition `""`.
    #[must_use]
    pub fn allows_partition(&self, partition_name: &str) -> bool {
        if self.effective_partition_patterns.is_empty() {
            return partition_name.is_empty();
        }
        self.effective_partition_patterns
            .iter()
            .any(|p| topic_match(p, partition_name))
    }
}

/// 7-Punkte-Chain-Validation.
///
/// Reihenfolge der Checks (early-return):
/// 1. Chain non-empty
/// 2. Profile hat Trust-Anchors (sofern nicht TrustPolicy::DirectOrDelegated mit Empty-Chain)
/// 3. Origin-Match
/// 4. Chain-Kontinuitaet
/// 5. Pro Link: Algorithm-Filter
/// 6. Pro Link: Zeitfenster
/// 7. Pro Link: Signatur (Trust-Anchor-PubKey fuer initial, vorigen Delegatee fuer Folgelinks)
/// 8. Trust-Anchor-Match
/// 9. Chain-Depth gegen Profile
/// 10. Scope-Intersection
///
/// **Anmerkung zu Punkt 7 (Signatur-Kette):** Folgelinks koennen wir
/// nicht ohne Zugriff auf den **delegator-Cert** des Zwischen-Hops
/// verifizieren — denn aus der GUID allein laesst sich kein PubKey
/// ableiten. j-b loest das so: der vorige `link.delegatee_guid` ist
/// gleichzeitig der naechste `link.delegator_guid`. Wir vertrauen
/// der **Sub-Gateway-Bridge** (j-e), den passenden PubKey via SPDP
/// mitzuliefern. In j-b expandieren wir `pubkey_resolver: impl Fn(&[u8;16])
/// -> Option<(Vec<u8>, SignatureAlgorithm)>` als Closure-Hook — der
/// Default-Resolver matched nur den Trust-Anchor + den Initial-Link.
///
/// # Errors
/// Siehe [`DelegationCheckError`].
pub fn validate_chain<F>(
    chain: &DelegationChain,
    profile: &DelegationProfile,
    now: i64,
    pubkey_resolver: F,
) -> DelegationCheckResult<ValidatedChain>
where
    F: Fn(&[u8; 16]) -> Option<(Vec<u8>, SignatureAlgorithm)>,
{
    if chain.links.is_empty() {
        return Err(DelegationCheckError::EmptyChain);
    }
    if profile.trust_anchors.is_empty() {
        return Err(DelegationCheckError::NoTrustAnchor);
    }

    // Punkt 6: Chain-Depth.
    if chain.depth() > profile.max_chain_depth {
        return Err(DelegationCheckError::ChainTooDeep {
            depth: chain.depth(),
            max: profile.max_chain_depth,
        });
    }

    // Punkt 2: Origin-Match.
    if chain.origin_guid != chain.links[0].delegator_guid {
        return Err(DelegationCheckError::OriginMismatch);
    }

    // Punkt 1: Chain-Kontinuitaet.
    for i in 0..chain.links.len() - 1 {
        if chain.links[i].delegatee_guid != chain.links[i + 1].delegator_guid {
            return Err(DelegationCheckError::ChainBroken { index: i });
        }
    }

    // Punkt 3: Trust-Anchor-Match (Origin gegen Anchors-Liste).
    let initial = &chain.links[0];
    let anchor = match profile.trust_policy {
        TrustPolicy::GatewayOnly => {
            // Exakt 1 Anchor erlaubt.
            if profile.trust_anchors.len() != 1 {
                return Err(DelegationCheckError::AnchorAlgorithmMismatch);
            }
            let a = &profile.trust_anchors[0];
            if a.subject_guid != initial.delegator_guid {
                return Err(DelegationCheckError::UntrustedDelegator);
            }
            a
        }
        TrustPolicy::Federation | TrustPolicy::DirectOrDelegated | TrustPolicy::StrictDelegated => {
            profile
                .trust_anchors
                .iter()
                .find(|a| a.subject_guid == initial.delegator_guid)
                .ok_or(DelegationCheckError::UntrustedDelegator)?
        }
    };

    // Loop ueber alle Links: Algorithm + Time + Signatur.
    for (idx, link) in chain.links.iter().enumerate() {
        // Punkt 5a: Algorithm-Filter.
        if !profile
            .allowed_algorithms
            .contains(&link.algorithm.wire_id())
        {
            return Err(DelegationCheckError::AlgorithmRejected {
                index: idx,
                algorithm: link.algorithm.wire_id(),
            });
        }
        // Punkt 5b: Zeitfenster.
        if now < link.not_before || now > link.not_after {
            return Err(DelegationCheckError::LinkExpired {
                index: idx,
                now,
                not_before: link.not_before,
                not_after: link.not_after,
            });
        }
        // Punkt 4: Signatur. Initial-Link gegen Trust-Anchor, sonst
        // gegen pubkey_resolver(delegator_guid) — der Caller stellt das
        // ueber den letzten Link's delegatee bereit.
        let (verify_pk, expected_algo) = if idx == 0 {
            (anchor.verify_public_key.clone(), anchor.algorithm)
        } else {
            // Vorheriger delegatee == aktueller delegator.
            pubkey_resolver(&link.delegator_guid).ok_or_else(|| {
                DelegationCheckError::SignatureInvalid {
                    index: idx,
                    reason: alloc::format!("no public key for delegator {:?}", link.delegator_guid),
                }
            })?
        };
        // Defensive: Anchor-Algo soll zum Initial-Link passen.
        if idx == 0 && expected_algo != link.algorithm {
            return Err(DelegationCheckError::AnchorAlgorithmMismatch);
        }
        link.verify(&verify_pk)
            .map_err(|e| DelegationCheckError::SignatureInvalid {
                index: idx,
                reason: alloc::format!("{e}"),
            })?;
    }

    // Punkt 7: Scope-Intersection.
    let mut effective_topics = chain.links[0].allowed_topic_patterns.clone();
    let mut effective_parts = chain.links[0].allowed_partition_patterns.clone();
    for link in chain.links.iter().skip(1) {
        effective_topics = scope_intersect(&effective_topics, &link.allowed_topic_patterns);
        effective_parts = scope_intersect(&effective_parts, &link.allowed_partition_patterns);
    }

    let edge_guid = chain
        .edge_guid()
        .unwrap_or(chain.links[chain.links.len() - 1].delegatee_guid);

    Ok(ValidatedChain {
        origin_guid: chain.origin_guid,
        edge_guid,
        chain_depth: chain.depth(),
        effective_topic_patterns: effective_topics,
        effective_partition_patterns: effective_parts,
    })
}

/// Scope-Intersection ueber Wildcard-Pattern-Listen.
///
/// Ein Pattern aus `a` bleibt im Schnitt, wenn es **mindestens einem
/// Pattern in `b` Subset ist** (im Sinne des Wildcard-Match: jedes
/// `topic_match(b_pat, a_pat)` ist genau die Subset-Relation zwischen
/// Pattern-Sprachen, weil `a_pat` als Topic-Name von `b_pat` gematched
/// werden muesste — wir approximieren das durch:
///
/// * `a_pat` bleibt drin, wenn `b` ein Pattern enthaelt, das `a_pat`
///   matched (z.B. `b="*"` matched alles).
/// * Andersrum dazu: konkrete `b_pat`-Strings die kein Wildcard sind
///   bleiben drin, wenn `a` ein Pattern enthaelt das `b_pat` matched.
///
/// Das ist bewusst konservativ — bei Unsicherheit lieber das engere
/// Set behalten. Special-Case: Wenn `b` `"*"` enthaelt, ist alles aus
/// `a` erlaubt (b ist "alles"). Wenn `a` `"*"` enthaelt, alles aus `b`.
#[must_use]
pub fn scope_intersect(a: &[String], b: &[String]) -> Vec<String> {
    // Special-Cases fuer Empty oder Allow-All.
    if a.is_empty() {
        return b.to_vec();
    }
    if b.is_empty() {
        return a.to_vec();
    }
    if a.iter().any(|p| p == "*") {
        return b.to_vec();
    }
    if b.iter().any(|p| p == "*") {
        return a.to_vec();
    }
    let mut out: Vec<String> = Vec::new();
    for pa in a {
        let pa_in_b = b.iter().any(|pb| topic_match(pb, pa));
        if pa_in_b && !out.contains(pa) {
            out.push(pa.clone());
        }
    }
    for pb in b {
        let pb_in_a = a.iter().any(|pa| topic_match(pa, pb));
        if pb_in_a && !out.contains(pb) {
            out.push(pb.clone());
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use ring::rand::SystemRandom;
    use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};
    use zerodds_security_pki::DelegationLink;

    fn ecdsa_keys() -> (Vec<u8>, Vec<u8>) {
        let rng = SystemRandom::new();
        let pkcs8 =
            EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).expect("gen");
        let pkcs8_vec = pkcs8.as_ref().to_vec();
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &pkcs8_vec, &rng)
            .expect("parse");
        (pkcs8_vec, key.public_key().as_ref().to_vec())
    }

    fn make_link(
        delegator: [u8; 16],
        delegatee: [u8; 16],
        topics: &[&str],
        signing_pkcs8: &[u8],
    ) -> DelegationLink {
        let mut l = DelegationLink::new(
            delegator,
            delegatee,
            topics.iter().map(|s| s.to_string()).collect(),
            alloc::vec![],
            1_000,
            2_000,
            SignatureAlgorithm::EcdsaP256,
        )
        .expect("new link");
        l.sign(signing_pkcs8).expect("sign");
        l
    }

    fn profile_with(
        anchor: TrustAnchor,
        policy: TrustPolicy,
        max_depth: usize,
    ) -> DelegationProfile {
        let mut algos = BTreeSet::new();
        algos.insert(SignatureAlgorithm::EcdsaP256.wire_id());
        algos.insert(SignatureAlgorithm::EcdsaP384.wire_id());
        algos.insert(SignatureAlgorithm::Ed25519.wire_id());
        DelegationProfile {
            name: "test".to_string(),
            trust_policy: policy,
            trust_anchors: alloc::vec![anchor],
            max_chain_depth: max_depth,
            allowed_algorithms: algos,
            require_ocsp: false,
        }
    }

    #[test]
    fn one_hop_chain_validates() {
        let (sk, pk) = ecdsa_keys();
        let gateway = [0xAA; 16];
        let edge = [0xBB; 16];
        let link = make_link(gateway, edge, &["sensor/*"], &sk);
        let chain = DelegationChain::new(gateway, alloc::vec![link]).expect("chain");
        let anchor = TrustAnchor {
            subject_guid: gateway,
            verify_public_key: pk,
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        let profile = profile_with(anchor, TrustPolicy::GatewayOnly, 3);
        let validated = validate_chain(&chain, &profile, 1_500, |_| None).expect("validate");
        assert_eq!(validated.origin_guid, gateway);
        assert_eq!(validated.edge_guid, edge);
        assert_eq!(validated.chain_depth, 1);
        assert_eq!(
            validated.effective_topic_patterns,
            alloc::vec!["sensor/*".to_string()]
        );
    }

    #[test]
    fn empty_chain_rejects() {
        let (_, pk) = ecdsa_keys();
        let anchor = TrustAnchor {
            subject_guid: [0; 16],
            verify_public_key: pk,
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        let profile = profile_with(anchor, TrustPolicy::GatewayOnly, 3);
        let chain = DelegationChain {
            origin_guid: [0; 16],
            links: alloc::vec![],
        };
        let err = validate_chain(&chain, &profile, 1_500, |_| None).expect_err("must fail");
        assert!(matches!(err, DelegationCheckError::EmptyChain));
    }

    #[test]
    fn chain_too_deep_rejects() {
        let (sk, pk) = ecdsa_keys();
        let gw = [0xAA; 16];
        let mid = [0xCC; 16];
        let edge = [0xBB; 16];
        let l1 = make_link(gw, mid, &["sensor/*"], &sk);
        let l2 = make_link(mid, edge, &["sensor/lidar"], &sk); // sig wird im check fehlschlagen, vorher schon depth-fail
        let chain = DelegationChain::new(gw, alloc::vec![l1, l2]).expect("chain");
        let anchor = TrustAnchor {
            subject_guid: gw,
            verify_public_key: pk,
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        let mut profile = profile_with(anchor, TrustPolicy::GatewayOnly, 1);
        profile.max_chain_depth = 1;
        let err = validate_chain(&chain, &profile, 1_500, |_| None).expect_err("must fail");
        assert!(matches!(
            err,
            DelegationCheckError::ChainTooDeep { depth: 2, max: 1 }
        ));
    }

    #[test]
    fn origin_mismatch_rejects() {
        let (sk, pk) = ecdsa_keys();
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let link = make_link(gw, edge, &["sensor/*"], &sk);
        // origin != links[0].delegator
        let chain = DelegationChain {
            origin_guid: [0xFF; 16],
            links: alloc::vec![link],
        };
        let anchor = TrustAnchor {
            subject_guid: gw,
            verify_public_key: pk,
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        let profile = profile_with(anchor, TrustPolicy::GatewayOnly, 3);
        let err = validate_chain(&chain, &profile, 1_500, |_| None).expect_err("must fail");
        assert!(matches!(err, DelegationCheckError::OriginMismatch));
    }

    #[test]
    fn untrusted_delegator_rejects() {
        let (sk, _pk_sk) = ecdsa_keys();
        let (_sk2, pk_anchor) = ecdsa_keys(); // anchor ist anderer Key
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let link = make_link(gw, edge, &["sensor/*"], &sk);
        let chain = DelegationChain::new(gw, alloc::vec![link]).expect("chain");
        let anchor = TrustAnchor {
            subject_guid: [0x99; 16], // nicht gw
            verify_public_key: pk_anchor,
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        let profile = profile_with(anchor, TrustPolicy::GatewayOnly, 3);
        let err = validate_chain(&chain, &profile, 1_500, |_| None).expect_err("must fail");
        assert!(matches!(err, DelegationCheckError::UntrustedDelegator));
    }

    #[test]
    fn link_expired_rejects() {
        let (sk, pk) = ecdsa_keys();
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let link = make_link(gw, edge, &["sensor/*"], &sk);
        let chain = DelegationChain::new(gw, alloc::vec![link]).expect("chain");
        let anchor = TrustAnchor {
            subject_guid: gw,
            verify_public_key: pk,
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        let profile = profile_with(anchor, TrustPolicy::GatewayOnly, 3);
        // now nach not_after=2_000
        let err = validate_chain(&chain, &profile, 5_000, |_| None).expect_err("must fail");
        assert!(matches!(err, DelegationCheckError::LinkExpired { .. }));
    }

    #[test]
    fn algorithm_rejected_by_profile() {
        let (sk, pk) = ecdsa_keys();
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let link = make_link(gw, edge, &["sensor/*"], &sk);
        let chain = DelegationChain::new(gw, alloc::vec![link]).expect("chain");
        let anchor = TrustAnchor {
            subject_guid: gw,
            verify_public_key: pk,
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        let mut profile = profile_with(anchor, TrustPolicy::GatewayOnly, 3);
        // ECDSA-P256 nicht in Whitelist:
        profile.allowed_algorithms.clear();
        profile
            .allowed_algorithms
            .insert(SignatureAlgorithm::Ed25519.wire_id());
        let err = validate_chain(&chain, &profile, 1_500, |_| None).expect_err("must fail");
        assert!(matches!(
            err,
            DelegationCheckError::AlgorithmRejected { .. }
        ));
    }

    #[test]
    fn signature_invalid_rejects() {
        let (sk, _pk_sk) = ecdsa_keys();
        let (_sk2, pk_anchor_other) = ecdsa_keys();
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let link = make_link(gw, edge, &["sensor/*"], &sk);
        let chain = DelegationChain::new(gw, alloc::vec![link]).expect("chain");
        // Trust-Anchor zeigt auf gw, hat aber falschen PubKey.
        let anchor = TrustAnchor {
            subject_guid: gw,
            verify_public_key: pk_anchor_other,
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        let profile = profile_with(anchor, TrustPolicy::GatewayOnly, 3);
        let err = validate_chain(&chain, &profile, 1_500, |_| None).expect_err("must fail");
        assert!(matches!(
            err,
            DelegationCheckError::SignatureInvalid { index: 0, .. }
        ));
    }

    #[test]
    fn two_hop_chain_via_resolver() {
        // gw -> mid -> edge. Initial-Link ist gw->mid (signiert mit sk_gw).
        // Folge-Link mid->edge (signiert mit sk_mid). Resolver liefert
        // pk_mid wenn nach mid gefragt.
        let rng = SystemRandom::new();
        let pkcs8_gw =
            EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).expect("gw");
        let sk_gw = pkcs8_gw.as_ref().to_vec();
        let pk_gw = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &sk_gw, &rng)
            .expect("parse")
            .public_key()
            .as_ref()
            .to_vec();
        let pkcs8_mid =
            EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).expect("mid");
        let sk_mid = pkcs8_mid.as_ref().to_vec();
        let pk_mid = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &sk_mid, &rng)
            .expect("parse")
            .public_key()
            .as_ref()
            .to_vec();

        let gw = [0xAA; 16];
        let mid = [0xCC; 16];
        let edge = [0xBB; 16];
        let l1 = make_link(gw, mid, &["sensor/*"], &sk_gw);
        let l2 = make_link(mid, edge, &["sensor/lidar"], &sk_mid);
        let chain = DelegationChain::new(gw, alloc::vec![l1, l2]).expect("chain");

        let anchor = TrustAnchor {
            subject_guid: gw,
            verify_public_key: pk_gw,
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        let profile = profile_with(anchor, TrustPolicy::DirectOrDelegated, 3);

        // Resolver liefert pk_mid fuer mid (== aktueller delegator des
        // 2. Links).
        let resolver = |g: &[u8; 16]| -> Option<(Vec<u8>, SignatureAlgorithm)> {
            if g == &mid {
                Some((pk_mid.clone(), SignatureAlgorithm::EcdsaP256))
            } else {
                None
            }
        };
        let validated = validate_chain(&chain, &profile, 1_500, resolver).expect("validate");
        assert_eq!(validated.chain_depth, 2);
        assert_eq!(validated.edge_guid, edge);
        // Scope-Intersection: enger von "sensor/*" und "sensor/lidar" ist "sensor/lidar"
        assert!(
            validated
                .effective_topic_patterns
                .contains(&"sensor/lidar".to_string())
        );
    }

    #[test]
    fn chain_broken_rejects() {
        let (sk, pk) = ecdsa_keys();
        let gw = [0xAA; 16];
        let mid = [0xCC; 16];
        let edge = [0xBB; 16];
        let l1 = make_link(gw, mid, &["sensor/*"], &sk);
        // l2 hat falschen delegator (nicht mid):
        let l2 = make_link([0xDD; 16], edge, &["sensor/lidar"], &sk);
        let chain = DelegationChain::new(gw, alloc::vec![l1, l2]).expect("chain");
        let anchor = TrustAnchor {
            subject_guid: gw,
            verify_public_key: pk,
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        let profile = profile_with(anchor, TrustPolicy::DirectOrDelegated, 3);
        let err = validate_chain(&chain, &profile, 1_500, |_| None).expect_err("must fail");
        assert!(matches!(
            err,
            DelegationCheckError::ChainBroken { index: 0 }
        ));
    }

    #[test]
    fn federation_finds_anchor_in_list() {
        let (sk1, pk1) = ecdsa_keys();
        let (_sk2, pk2) = ecdsa_keys();
        let gw1 = [0x11; 16];
        let gw2 = [0x22; 16];
        let edge = [0xBB; 16];
        let link = make_link(gw2, edge, &["sensor/*"], &sk1); // signed mit sk1
        let chain = DelegationChain::new(gw2, alloc::vec![link]).expect("chain");

        let mut profile = profile_with(
            TrustAnchor {
                subject_guid: gw1,
                verify_public_key: pk1.clone(),
                algorithm: SignatureAlgorithm::EcdsaP256,
            },
            TrustPolicy::Federation,
            3,
        );
        profile.trust_anchors.push(TrustAnchor {
            subject_guid: gw2,
            verify_public_key: pk2,
            algorithm: SignatureAlgorithm::EcdsaP256,
        });
        // Link wurde mit sk1 signiert, anchor fuer gw2 hat aber pk2 →
        // SignatureInvalid (aber UntrustedDelegator wird vorher
        // behoben weil gw2 in der Liste ist).
        let err = validate_chain(&chain, &profile, 1_500, |_| None).expect_err("must fail");
        assert!(matches!(err, DelegationCheckError::SignatureInvalid { .. }));

        // Korrekt-Fix: anchor fuer gw2 mit pk1 (matches signing key).
        profile.trust_anchors[1].verify_public_key = pk1;
        let validated = validate_chain(&chain, &profile, 1_500, |_| None).expect("validate");
        assert_eq!(validated.origin_guid, gw2);
    }

    #[test]
    fn no_trust_anchor_rejects() {
        let (sk, _) = ecdsa_keys();
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let link = make_link(gw, edge, &["sensor/*"], &sk);
        let chain = DelegationChain::new(gw, alloc::vec![link]).expect("chain");
        let mut algos = BTreeSet::new();
        algos.insert(SignatureAlgorithm::EcdsaP256.wire_id());
        let profile = DelegationProfile {
            name: "no-anchor".to_string(),
            trust_policy: TrustPolicy::DirectOrDelegated,
            trust_anchors: alloc::vec![],
            max_chain_depth: 3,
            allowed_algorithms: algos,
            require_ocsp: false,
        };
        let err = validate_chain(&chain, &profile, 1_500, |_| None).expect_err("must fail");
        assert!(matches!(err, DelegationCheckError::NoTrustAnchor));
    }

    #[test]
    fn validated_chain_topic_match() {
        let v = ValidatedChain {
            origin_guid: [0; 16],
            edge_guid: [0; 16],
            chain_depth: 1,
            effective_topic_patterns: alloc::vec!["sensor/*".to_string()],
            effective_partition_patterns: alloc::vec![],
        };
        assert!(v.allows_topic("sensor/lidar"));
        assert!(!v.allows_topic("actuator/x"));
        // Empty partition list = nur default partition
        assert!(v.allows_partition(""));
        assert!(!v.allows_partition("public"));
    }

    #[test]
    fn validated_chain_partition_match_with_patterns() {
        let v = ValidatedChain {
            origin_guid: [0; 16],
            edge_guid: [0; 16],
            chain_depth: 1,
            effective_topic_patterns: alloc::vec!["*".to_string()],
            effective_partition_patterns: alloc::vec!["pub_*".to_string()],
        };
        assert!(v.allows_partition("pub_alpha"));
        assert!(!v.allows_partition("priv_x"));
    }

    #[test]
    fn scope_intersect_empty_treats_as_allow_all() {
        let a: Vec<String> = alloc::vec![];
        let b: Vec<String> = alloc::vec!["sensor/*".to_string()];
        assert_eq!(scope_intersect(&a, &b), alloc::vec!["sensor/*".to_string()]);
    }

    #[test]
    fn scope_intersect_star_is_neutral() {
        let a = alloc::vec!["*".to_string()];
        let b = alloc::vec!["sensor/lidar".to_string(), "sensor/cam".to_string()];
        assert_eq!(scope_intersect(&a, &b), b);
    }

    #[test]
    fn scope_intersect_narrows() {
        let a = alloc::vec!["sensor/*".to_string()];
        let b = alloc::vec!["sensor/lidar".to_string()];
        let isec = scope_intersect(&a, &b);
        assert!(isec.contains(&"sensor/lidar".to_string()));
    }

    #[test]
    fn scope_intersect_disjoint() {
        let a = alloc::vec!["sensor/*".to_string()];
        let b = alloc::vec!["actuator/*".to_string()];
        let isec = scope_intersect(&a, &b);
        assert!(isec.is_empty());
    }

    #[test]
    fn ed25519_default_anchor_constructor() {
        let pk = alloc::vec![0u8; 32];
        let anchor = TrustAnchor {
            subject_guid: [1; 16],
            verify_public_key: pk,
            algorithm: SignatureAlgorithm::Ed25519,
        };
        let profile = DelegationProfile::default_with_anchor("default".to_string(), anchor);
        assert_eq!(profile.max_chain_depth, 3);
        assert!(
            profile
                .allowed_algorithms
                .contains(&SignatureAlgorithm::Ed25519.wire_id())
        );
        assert!(matches!(
            profile.trust_policy,
            TrustPolicy::DirectOrDelegated
        ));
    }
}
