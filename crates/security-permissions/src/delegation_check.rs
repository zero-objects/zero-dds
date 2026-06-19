// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Delegation chain validation for permissions sub-CAs.
//!
//! Implements the 7-point validation from
//! `docs/architecture/09_delegation.md` §6:
//!
//! 1. **Chain continuity** — `links[i].delegatee_guid` must
//!    equal `links[i+1].delegator_guid`.
//! 2. **Origin match** — `chain.origin_guid` must equal
//!    `links[0].delegator_guid`.
//! 3. **Trust anchor match** — depending on the [`TrustPolicy`] mode,
//!    the origin delegator is checked against one or more trust anchors.
//! 4. **Signature chain** — each link is verified against the **previous
//!    delegatee pubkey** (the initial link against the
//!    trust-anchor pubkey). This prevents a compromised
//!    intermediate gateway from escalating arbitrarily upward.
//! 5. **Time window** — `link.not_before <= now <= link.not_after`
//!    for **every** link.
//! 6. **Max chain depth** — `chain.depth() <= profile.max_chain_depth`.
//! 7. **Scope intersection** — the effective topic/partition pattern
//!    list is the intersection of all pattern lists along the chain.
//!    This way an interposed gateway can only narrow the scope,
//!    never widen it.
//!
//! Output is [`ValidatedChain`] — passed by the caller (j-d
//! `peer_matches_class`) as an authorization pass to the permissions plugin.

extern crate alloc;

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_security_pki::{DelegationChain, SignatureAlgorithm};

use crate::topic_match::topic_match;

/// Trust policy mode (architecture §4).
///
/// Determines how [`validate_chain`] checks the origin delegator against
/// trust anchors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TrustPolicy {
    /// Gateway-only: the origin cert MUST match the configured
    /// gateway cert exactly. No multi-hop via other
    /// gateways. (Default for vehicle-internal.)
    GatewayOnly,
    /// Direct-or-delegated: the peer is accepted **either** directly
    /// (regular PKI auth, no chain) **or** via delegation.
    /// The hop count may go up to `profile.max_chain_depth`. (Default for
    /// vehicle ↔ C4I.)
    DirectOrDelegated,
    /// Federation: multiple trust anchors (all gateways peered with each
    /// other). The origin delegator MUST be in the trust-anchor list.
    Federation,
    /// Strict-delegated: only delegation allowed — no
    /// direct auth path. Useful for C4I backends that do not want to
    /// admit vehicle edges directly.
    StrictDelegated,
}

/// Trust anchor — public key DER + algorithm + subject GUID.
///
/// The `subject_guid` is the GUID of the trust anchor (typically the
/// hull gateway or C4I root). `verify_public_key` is the DER-bytes
/// format pubkey with which [`DelegationLink::verify`] is called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustAnchor {
    /// 16-byte GUID of the trust anchor.
    pub subject_guid: [u8; 16],
    /// Pubkey DER bytes (algorithm-specific format, see
    /// [`DelegationLink::verify`]).
    pub verify_public_key: Vec<u8>,
    /// Expected signature algorithm of this anchor.
    pub algorithm: SignatureAlgorithm,
}

/// Delegation profile (minimal — the full definition comes in j-h from
/// the governance XML).
///
/// Profile = configuration bundle that defines the trust policy + allowed
/// algorithms + max chain depth. Referenced by name in
/// `PeerClassMatch::delegation_profile` (j-d).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelegationProfile {
    /// Name of the profile (governance-XML reference).
    pub name: String,
    /// Trust policy mode.
    pub trust_policy: TrustPolicy,
    /// Allowed trust anchors. For `GatewayOnly` there must be exactly 1
    /// entry; for `Federation` >=1.
    pub trust_anchors: Vec<TrustAnchor>,
    /// Maximum chain depth (additional to the hard cap from the PKI crate).
    /// Default 3.
    pub max_chain_depth: usize,
    /// Allowed signature algorithms. Others → reject.
    pub allowed_algorithms: BTreeSet<u8>, // SignatureAlgorithm::wire_id
    /// If true: the profile requires an OCSP liveness check for the
    /// trust-anchor cert. Wired in j-h against the governance XML;
    /// in j-b the field is only a marker.
    pub require_ocsp: bool,
}

impl DelegationProfile {
    /// Convenience constructor with `max_chain_depth=3`,
    /// `trust_policy=DirectOrDelegated`, all 4 algorithms allowed.
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

/// Errors from the chain validation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DelegationCheckError {
    /// Chain is empty.
    EmptyChain,
    /// `links[i].delegatee_guid != links[i+1].delegator_guid`.
    ChainBroken {
        /// Index of the faulty link (i).
        index: usize,
    },
    /// `chain.origin_guid != links[0].delegator_guid`.
    OriginMismatch,
    /// The origin delegator matches no trust anchor.
    UntrustedDelegator,
    /// The link signature is invalid.
    SignatureInvalid {
        /// Index of the link.
        index: usize,
        /// Diagnostic string from the PKI crate.
        reason: String,
    },
    /// Link outside its time window.
    LinkExpired {
        /// Index of the link.
        index: usize,
        /// Current time tick.
        now: i64,
        /// `link.not_before`.
        not_before: i64,
        /// `link.not_after`.
        not_after: i64,
    },
    /// Chain is deeper than `profile.max_chain_depth`.
    ChainTooDeep {
        /// Actual depth.
        depth: usize,
        /// Profile limit.
        max: usize,
    },
    /// The signature algorithm used is not in
    /// `profile.allowed_algorithms`.
    AlgorithmRejected {
        /// Index of the link.
        index: usize,
        /// Algorithm wire id.
        algorithm: u8,
    },
    /// The profile requires at least one trust anchor but has none.
    NoTrustAnchor,
    /// The trust-anchor list has an entry with an algorithm mismatch to the
    /// initial link (defensive check).
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

/// Result alias.
pub type DelegationCheckResult<T> = Result<T, DelegationCheckError>;

/// Validated chain — output of [`validate_chain`].
///
/// The effective pattern lists are the result of the scope intersection
/// of all links: `effective = intersect(links[0].patterns, ..., links[N-1].patterns)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedChain {
    /// 16-byte GUID of the origin participant.
    pub origin_guid: [u8; 16],
    /// 16-byte GUID of the edge peer (= last delegatee).
    pub edge_guid: [u8; 16],
    /// Actual chain depth.
    pub chain_depth: usize,
    /// Effective topic patterns (intersection over all links).
    pub effective_topic_patterns: Vec<String>,
    /// Effective partition patterns (intersection over all links).
    pub effective_partition_patterns: Vec<String>,
}

impl ValidatedChain {
    /// True if `topic_name` is covered by the effective pattern list.
    /// Empty list = no topic whitelist (match
    /// false — explicit, safe default).
    #[must_use]
    pub fn allows_topic(&self, topic_name: &str) -> bool {
        if self.effective_topic_patterns.is_empty() {
            return false;
        }
        self.effective_topic_patterns
            .iter()
            .any(|p| topic_match(p, topic_name))
    }

    /// True if `partition_name` is covered by the effective partition
    /// pattern list. Empty list = only the default partition `""`.
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

/// 7-point chain validation.
///
/// Order of the checks (early return):
/// 1. Chain non-empty
/// 2. Profile has trust anchors (unless TrustPolicy::DirectOrDelegated with an empty chain)
/// 3. Origin match
/// 4. Chain continuity
/// 5. Per link: algorithm filter
/// 6. Per link: time window
/// 7. Per link: signature (trust-anchor pubkey for the initial, previous delegatee for follow-up links)
/// 8. Trust-anchor match
/// 9. Chain depth against the profile
/// 10. Scope intersection
///
/// **Note on point 7 (signature chain):** follow-up links cannot be
/// verified without access to the **delegator cert** of the intermediate
/// hop — because no pubkey can be derived from the GUID alone. j-b
/// solves this as follows: the previous `link.delegatee_guid` is at the
/// same time the next `link.delegator_guid`. We trust the
/// **sub-gateway bridge** (j-e) to supply the matching pubkey via SPDP.
/// In j-b we expand `pubkey_resolver: impl Fn(&[u8;16])
/// -> Option<(Vec<u8>, SignatureAlgorithm)>` as a closure hook — the
/// default resolver matches only the trust anchor + the initial link.
///
/// # Errors
/// See [`DelegationCheckError`].
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

    // Point 6: chain depth.
    if chain.depth() > profile.max_chain_depth {
        return Err(DelegationCheckError::ChainTooDeep {
            depth: chain.depth(),
            max: profile.max_chain_depth,
        });
    }

    // Point 2: origin match.
    if chain.origin_guid != chain.links[0].delegator_guid {
        return Err(DelegationCheckError::OriginMismatch);
    }

    // Point 1: chain continuity.
    for i in 0..chain.links.len() - 1 {
        if chain.links[i].delegatee_guid != chain.links[i + 1].delegator_guid {
            return Err(DelegationCheckError::ChainBroken { index: i });
        }
    }

    // Point 3: trust-anchor match (origin against the anchors list).
    let initial = &chain.links[0];
    let anchor = match profile.trust_policy {
        TrustPolicy::GatewayOnly => {
            // Exactly 1 anchor allowed.
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

    // Loop over all links: algorithm + time + signature.
    for (idx, link) in chain.links.iter().enumerate() {
        // Point 5a: algorithm filter.
        if !profile
            .allowed_algorithms
            .contains(&link.algorithm.wire_id())
        {
            return Err(DelegationCheckError::AlgorithmRejected {
                index: idx,
                algorithm: link.algorithm.wire_id(),
            });
        }
        // Point 5b: time window.
        if now < link.not_before || now > link.not_after {
            return Err(DelegationCheckError::LinkExpired {
                index: idx,
                now,
                not_before: link.not_before,
                not_after: link.not_after,
            });
        }
        // Point 4: signature. Initial link against the trust anchor, else
        // against pubkey_resolver(delegator_guid) — the caller provides it
        // via the last link's delegatee.
        let (verify_pk, expected_algo) = if idx == 0 {
            (anchor.verify_public_key.clone(), anchor.algorithm)
        } else {
            // Previous delegatee == current delegator.
            pubkey_resolver(&link.delegator_guid).ok_or_else(|| {
                DelegationCheckError::SignatureInvalid {
                    index: idx,
                    reason: alloc::format!("no public key for delegator {:?}", link.delegator_guid),
                }
            })?
        };
        // Defensive: the anchor algo should match the initial link.
        if idx == 0 && expected_algo != link.algorithm {
            return Err(DelegationCheckError::AnchorAlgorithmMismatch);
        }
        link.verify(&verify_pk)
            .map_err(|e| DelegationCheckError::SignatureInvalid {
                index: idx,
                reason: alloc::format!("{e}"),
            })?;
    }

    // Point 7: scope intersection.
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

/// Scope intersection over wildcard pattern lists.
///
/// A pattern from `a` stays in the intersection if it is **a subset of at
/// least one pattern in `b`** (in the sense of the wildcard match: every
/// `topic_match(b_pat, a_pat)` is exactly the subset relation between
/// pattern languages, because `a_pat` would have to be matched as a topic
/// name by `b_pat` — we approximate this by:
///
/// * `a_pat` stays in if `b` contains a pattern that matches `a_pat`
///   (e.g. `b="*"` matches everything).
/// * Conversely: concrete `b_pat` strings that are not a wildcard
///   stay in if `a` contains a pattern that matches `b_pat`.
///
/// This is intentionally conservative — when in doubt, keep the narrower
/// set. Special case: if `b` contains `"*"`, everything from
/// `a` is allowed (b is "everything"). If `a` contains `"*"`, everything from `b`.
#[must_use]
pub fn scope_intersect(a: &[String], b: &[String]) -> Vec<String> {
    // Special cases for empty or allow-all.
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
        let l2 = make_link(mid, edge, &["sensor/lidar"], &sk); // sig will fail in the check, but depth-fail happens first
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
        let (_sk2, pk_anchor) = ecdsa_keys(); // anchor is a different key
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let link = make_link(gw, edge, &["sensor/*"], &sk);
        let chain = DelegationChain::new(gw, alloc::vec![link]).expect("chain");
        let anchor = TrustAnchor {
            subject_guid: [0x99; 16], // not gw
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
        // now after not_after=2_000
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
        // ECDSA-P256 not in whitelist:
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
        // Trust anchor points to gw but has the wrong pubkey.
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
        // gw -> mid -> edge. The initial link is gw->mid (signed with sk_gw).
        // The follow-up link mid->edge (signed with sk_mid). The resolver
        // returns pk_mid when asked for mid.
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

        // The resolver returns pk_mid for mid (== current delegator of the
        // 2nd link).
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
        // Scope intersection: the narrower of "sensor/*" and "sensor/lidar" is "sensor/lidar"
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
        // l2 has the wrong delegator (not mid):
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
        let link = make_link(gw2, edge, &["sensor/*"], &sk1); // signed with sk1
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
        // The link was signed with sk1, but the anchor for gw2 has pk2 →
        // SignatureInvalid (but UntrustedDelegator is resolved earlier
        // because gw2 is in the list).
        let err = validate_chain(&chain, &profile, 1_500, |_| None).expect_err("must fail");
        assert!(matches!(err, DelegationCheckError::SignatureInvalid { .. }));

        // Correct fix: anchor for gw2 with pk1 (matches signing key).
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
        // Empty partition list = only the default partition
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
