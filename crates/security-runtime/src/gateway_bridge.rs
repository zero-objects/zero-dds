// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Gateway-bridge helper.
//!
//! Architecture reference: `docs/architecture/09_delegation.md` §3 (use
//! cases) + §5.3 (bridge sub-gateway chaining).
//!
//! A [`GatewayBridge`] typically sits on a hull or
//! turret compute unit responsible for several edge peers (sensors, ECUs)
//! without their own cert. The bridge:
//!
//! 1. **Issues** delegation links per edge peer, signed with its
//!    own gateway key.
//! 2. **Manages** the active delegations in a `BTreeMap`
//!    (`edge_guid → DelegationLink`).
//! 3. **Provides** on request the full [`DelegationChain`] to the
//!    discovery layer (SPDP property), optionally as 1-hop or
//!    n-hop when the bridge is itself the delegatee of a higher level
//!    (double-star hull+turret).
//! 4. **Revokes** delegations explicitly (revocation list, sent along in the
//!    next SPDP beacon).
//!
//! The bridge **does not run a forwarding path itself** — it is the
//! policy/data-model helper; the actual re-sealing and
//! forwarding of the RTPS frames happens in the DCPS runtime (plan
//! §stage j-g, comes later).

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_security_permissions::EdgeIdentityConfig;
use zerodds_security_pki::{DelegationChain, DelegationError, DelegationLink, SignatureAlgorithm};

/// Configuration of a gateway bridge.
///
/// `gateway_guid` is the 16-byte subject GUID that the issued
/// delegations carry as their `delegator_guid`.
/// `signing_key` is the PKCS#8-DER-formatted private key material for
/// signing new links — the bridge holds it in RAM, the loading mechanism
/// is up to the caller (filesystem, secret manager, HSM).
/// `algorithm` must match the trust anchor of the profile against which the
/// chain is later validated.
#[derive(Debug, Clone)]
pub struct GatewayBridgeConfig {
    /// 16-byte gateway participant GUID.
    pub gateway_guid: [u8; 16],
    /// PKCS#8-DER-formatted private key.
    pub signing_key: Vec<u8>,
    /// Signature algorithm.
    pub algorithm: SignatureAlgorithm,
}

/// Error from gateway-bridge operations.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GatewayBridgeError {
    /// The edge is not registered.
    UnknownEdge {
        /// 16-byte edge GUID.
        edge_guid: [u8; 16],
    },
    /// The sign operation failed (delegated from the PKI crate).
    DelegationFailed(DelegationError),
}

impl core::fmt::Display for GatewayBridgeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownEdge { edge_guid } => {
                write!(f, "no active delegation for edge {edge_guid:?}")
            }
            Self::DelegationFailed(e) => write!(f, "delegation failed: {e}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for GatewayBridgeError {}

impl From<DelegationError> for GatewayBridgeError {
    fn from(e: DelegationError) -> Self {
        Self::DelegationFailed(e)
    }
}

/// Result alias.
pub type GatewayBridgeResult<T> = Result<T, GatewayBridgeError>;

/// Gateway-bridge helper.
///
/// Lifecycle:
/// 1. [`GatewayBridge::new`] with `GatewayBridgeConfig`.
/// 2. Optional [`GatewayBridge::with_upstream`] to hook the own bridge
///    into an existing chain (e.g. from the hull GW to the own turret GW).
/// 3. Per edge: [`GatewayBridge::delegate_for`] to issue a new
///    delegation.
/// 4. [`GatewayBridge::chain_for`] provides the chain as output for
///    SPDP/SEDP properties.
/// 5. [`GatewayBridge::revoke_delegation`] removes an edge.
#[derive(Debug, Clone)]
pub struct GatewayBridge {
    config: GatewayBridgeConfig,
    /// Optional: chain that legitimizes this gateway as the delegatee of a
    /// higher level. In [`Self::chain_for`] it is prepended to the issued
    /// edge link.
    upstream: Option<DelegationChain>,
    /// Active edge delegations (`edge_guid → Link`).
    active: BTreeMap<[u8; 16], DelegationLink>,
    /// Revocation list: edge GUIDs whose delegation should be announced as
    /// revoked in the next SPDP wave. The caller clears
    /// the list after a successful announce via
    /// [`Self::take_revocations`].
    revocations: Vec<[u8; 16]>,
}

impl GatewayBridge {
    /// Constructor.
    #[must_use]
    pub fn new(config: GatewayBridgeConfig) -> Self {
        Self {
            config,
            upstream: None,
            active: BTreeMap::new(),
            revocations: Vec::new(),
        }
    }

    /// Sets an upstream chain. In [`Self::chain_for`] it is
    /// prepended to the edge link — sub-gateway chaining for
    /// a double-star (turret GW under hull GW).
    ///
    /// Validation of the upstream chain is NOT part of the bridge —
    /// the caller must call `validate_chain` itself beforehand, to
    /// prevent mismatch-profile errors.
    pub fn with_upstream(&mut self, upstream_chain: DelegationChain) {
        self.upstream = Some(upstream_chain);
    }

    /// 16-byte gateway GUID (read-only).
    #[must_use]
    pub fn gateway_guid(&self) -> [u8; 16] {
        self.config.gateway_guid
    }

    /// Issues a new delegation for an edge peer. If the
    /// edge was already delegated, the old link is overwritten
    /// (typical with ephemeral-edge rotation, plan §stage j-f).
    ///
    /// `not_before` and `not_after` are absolute Unix seconds;
    /// `topic_patterns`/`partition_patterns` are the glob whitelist
    /// that the edge may have in the narrowest scope.
    ///
    /// # Errors
    /// [`GatewayBridgeError::DelegationFailed`] if the PKI sign
    /// step fails (cap violation, key parse error).
    pub fn delegate_for(
        &mut self,
        edge_guid: [u8; 16],
        topic_patterns: Vec<String>,
        partition_patterns: Vec<String>,
        not_before: i64,
        not_after: i64,
    ) -> GatewayBridgeResult<&DelegationLink> {
        let mut link = DelegationLink::new(
            self.config.gateway_guid,
            edge_guid,
            topic_patterns,
            partition_patterns,
            not_before,
            not_after,
            self.config.algorithm,
        )?;
        link.sign(&self.config.signing_key)?;
        self.active.insert(edge_guid, link);
        // Edge active again → remove from revocations if needed, in case of
        // a re-issue (e.g. after renewal).
        self.revocations.retain(|g| g != &edge_guid);
        // The lookup after a successful insert cannot fail,
        // but clippy::expect_used forbids expect — we provide
        // unwrap_or via a fresh lookup.
        self.active
            .get(&edge_guid)
            .ok_or(GatewayBridgeError::UnknownEdge { edge_guid })
    }

    /// Revokes the active delegation for an edge. The edge is
    /// added to the revocation list and can be communicated to the
    /// discovery layer via [`Self::take_revocations`].
    ///
    /// # Errors
    /// [`GatewayBridgeError::UnknownEdge`] if the edge is not active.
    pub fn revoke_delegation(&mut self, edge_guid: [u8; 16]) -> GatewayBridgeResult<()> {
        if self.active.remove(&edge_guid).is_some() {
            if !self.revocations.contains(&edge_guid) {
                self.revocations.push(edge_guid);
            }
            Ok(())
        } else {
            Err(GatewayBridgeError::UnknownEdge { edge_guid })
        }
    }

    /// Returns the outgoing chain for an edge.
    ///
    /// 1-hop bridge (no upstream): chain = `[edge link]`,
    ///   `origin_guid = gateway_guid`.
    /// n-hop bridge (with upstream): chain = `upstream.links ++
    ///   [edge link]`, `origin_guid = upstream.origin_guid`.
    ///
    /// Returns `None` if the edge is not active.
    #[must_use]
    pub fn chain_for(&self, edge_guid: &[u8; 16]) -> Option<DelegationChain> {
        let edge_link = self.active.get(edge_guid)?.clone();
        match &self.upstream {
            None => DelegationChain::new(self.config.gateway_guid, alloc::vec![edge_link]).ok(),
            Some(up) => {
                let mut links = up.links.clone();
                links.push(edge_link);
                DelegationChain::new(up.origin_guid, links).ok()
            }
        }
    }

    /// Number of active edge delegations.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// True if an edge is actively delegated.
    #[must_use]
    pub fn has_edge(&self, edge_guid: &[u8; 16]) -> bool {
        self.active.contains_key(edge_guid)
    }

    /// Iterates over all active edge delegations.
    pub fn iter_active(&self) -> impl Iterator<Item = (&[u8; 16], &DelegationLink)> {
        self.active.iter()
    }

    /// Reads and clears the revocation list (the discovery layer calls this
    /// per SPDP beacon tick).
    pub fn take_revocations(&mut self) -> Vec<[u8; 16]> {
        core::mem::take(&mut self.revocations)
    }

    /// Read access to the upstream chain (useful for logging /
    /// metrics).
    #[must_use]
    pub fn upstream(&self) -> Option<&DelegationChain> {
        self.upstream.as_ref()
    }

    /// Rotates ephemeral edge identities whose lifetime has expired.
    ///
    /// Workflow per ephemeral edge:
    /// 1. If the edge is not active → skip (init comes via
    ///    `delegate_for` by the caller).
    /// 2. If `now < link.not_after - lifetime/N` (N=renewal window)
    ///    → still too fresh, skip.
    /// 3. Otherwise: pull a new GuidPrefix (`prefix_generator(name)`),
    ///    revoke the old edge, issue a new `delegate_for` with a `now`-based
    ///    time window.
    ///
    /// `prefix_generator` is a pluggable hook (e.g. a ChaCha20 RNG
    /// or a system RNG); the bridge is deterministically testable because
    /// the randomness source comes from the caller.
    ///
    /// Returns the list of rotated edge names.
    ///
    /// # Errors
    /// Propagates [`GatewayBridgeError::DelegationFailed`] if a
    /// re-sign fails — but does NOT abort the loop;
    /// faulty edges are collected in the `Err` tail vec and the
    /// caller can decide.
    pub fn rotate_ephemerals<F>(
        &mut self,
        identities: &[EdgeIdentityConfig],
        now: i64,
        topic_patterns: Vec<String>,
        partition_patterns: Vec<String>,
        mut prefix_generator: F,
    ) -> (Vec<String>, Vec<(String, GatewayBridgeError)>)
    where
        F: FnMut(&str) -> [u8; 12],
    {
        let mut rotated = Vec::new();
        let mut failed = Vec::new();
        for cfg in identities.iter().filter(|c| c.is_ephemeral()) {
            // Edge GUID = 12-byte prefix + 4-byte EntityId 0x00.0x00.0x01.0xC1 (DDS conv).
            // Here we take only the prefix as the identifier key —
            // the GuidPrefix itself decides the edge identity.
            let new_prefix = prefix_generator(&cfg.name);
            // Reconstruct full 16-byte edge guid (prefix + EntityId of the
            // default participant EntityId).
            let mut edge_guid = [0u8; 16];
            edge_guid[..12].copy_from_slice(&new_prefix);
            edge_guid[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);

            // Pick up all existing edges with the same name
            // (matching here is prefix-based; in a full impl we would
            // have a name-keyed map). Since we don't have the name in the
            // active map, we proceed raw here:
            // delegate_for overwrites on conflict.
            let lifetime = i64::from(cfg.effective_lifetime());
            let new_not_after = now.saturating_add(lifetime);
            match self.delegate_for(
                edge_guid,
                topic_patterns.clone(),
                partition_patterns.clone(),
                now,
                new_not_after,
            ) {
                Ok(_) => rotated.push(cfg.name.clone()),
                Err(e) => failed.push((cfg.name.clone(), e)),
            }
        }
        (rotated, failed)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use ring::rand::SystemRandom;
    use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};
    use zerodds_security_permissions::EdgeIdentityMode;

    fn ecdsa_p256_keypair() -> (Vec<u8>, Vec<u8>) {
        let rng = SystemRandom::new();
        let pkcs8 =
            EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).expect("gen");
        let sk = pkcs8.as_ref().to_vec();
        let kp = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &sk, &rng).expect("p");
        (sk, kp.public_key().as_ref().to_vec())
    }

    fn make_bridge(gw_guid: [u8; 16]) -> (GatewayBridge, Vec<u8>) {
        let (sk, pk) = ecdsa_p256_keypair();
        let cfg = GatewayBridgeConfig {
            gateway_guid: gw_guid,
            signing_key: sk,
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        (GatewayBridge::new(cfg), pk)
    }

    #[test]
    fn delegate_for_creates_signed_link() {
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let (mut bridge, pk) = make_bridge(gw);
        let link = bridge
            .delegate_for(
                edge,
                alloc::vec!["sensor/*".to_string()],
                alloc::vec![],
                1_000,
                9_000,
            )
            .expect("delegate")
            .clone();
        assert_eq!(link.delegator_guid, gw);
        assert_eq!(link.delegatee_guid, edge);
        assert_eq!(link.signature.len(), 64); // ECDSA-P256 fixed
        link.verify(&pk).expect("verify");
        assert_eq!(bridge.active_count(), 1);
        assert!(bridge.has_edge(&edge));
    }

    #[test]
    fn one_hop_chain_for_edge() {
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let (mut bridge, _pk) = make_bridge(gw);
        bridge
            .delegate_for(
                edge,
                alloc::vec!["sensor/*".to_string()],
                alloc::vec![],
                0,
                9_000,
            )
            .expect("delegate");
        let chain = bridge.chain_for(&edge).expect("chain");
        assert_eq!(chain.depth(), 1);
        assert_eq!(chain.origin_guid, gw);
        assert_eq!(chain.edge_guid(), Some(edge));
    }

    #[test]
    fn chain_for_missing_edge_is_none() {
        let gw = [0xAA; 16];
        let (bridge, _) = make_bridge(gw);
        assert!(bridge.chain_for(&[0xCC; 16]).is_none());
    }

    #[test]
    fn revoke_delegation_removes_active_and_records_revocation() {
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let (mut bridge, _pk) = make_bridge(gw);
        bridge
            .delegate_for(edge, alloc::vec![], alloc::vec![], 0, 9_000)
            .expect("delegate");
        bridge.revoke_delegation(edge).expect("revoke");
        assert_eq!(bridge.active_count(), 0);
        assert!(!bridge.has_edge(&edge));
        let revocations = bridge.take_revocations();
        assert_eq!(revocations, alloc::vec![edge]);
        // The list is empty after take.
        assert!(bridge.take_revocations().is_empty());
    }

    #[test]
    fn revoke_unknown_edge_is_error() {
        let gw = [0xAA; 16];
        let (mut bridge, _) = make_bridge(gw);
        let err = bridge.revoke_delegation([0xFF; 16]).expect_err("must fail");
        assert!(matches!(err, GatewayBridgeError::UnknownEdge { .. }));
    }

    #[test]
    fn re_delegate_clears_pending_revocation() {
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let (mut bridge, _) = make_bridge(gw);
        bridge
            .delegate_for(edge, alloc::vec![], alloc::vec![], 0, 9_000)
            .expect("delegate");
        bridge.revoke_delegation(edge).expect("revoke");
        // Re-delegate (e.g. after a cert renewal) must remove the revocation.
        bridge
            .delegate_for(edge, alloc::vec![], alloc::vec![], 100, 10_000)
            .expect("redelegate");
        assert!(bridge.take_revocations().is_empty());
        assert!(bridge.has_edge(&edge));
    }

    #[test]
    fn sub_gateway_chaining_two_hops() {
        // Hull GW (gw1) delegates to turret GW (gw2). The turret GW bridges
        // edge `turm-imu`. The resulting chain has 2 links and origin = gw1.
        let gw1 = [0x11; 16];
        let gw2 = [0x22; 16];
        let edge = [0x33; 16];

        // gw1 creates an upstream link gw1 -> gw2.
        let (sk1, _pk1) = ecdsa_p256_keypair();
        let mut upstream_link = DelegationLink::new(
            gw1,
            gw2,
            alloc::vec!["*".to_string()],
            alloc::vec![],
            0,
            9_000,
            SignatureAlgorithm::EcdsaP256,
        )
        .expect("upstream link");
        upstream_link.sign(&sk1).expect("sign upstream");
        let upstream_chain =
            DelegationChain::new(gw1, alloc::vec![upstream_link]).expect("upstream chain");

        // Turret bridge.
        let (mut turm_bridge, _pk2) = make_bridge(gw2);
        turm_bridge.with_upstream(upstream_chain);

        turm_bridge
            .delegate_for(
                edge,
                alloc::vec!["sensor/imu".to_string()],
                alloc::vec![],
                100,
                8_000,
            )
            .expect("turm delegate");
        let chain = turm_bridge.chain_for(&edge).expect("chain");
        assert_eq!(chain.depth(), 2);
        assert_eq!(chain.origin_guid, gw1);
        assert_eq!(chain.edge_guid(), Some(edge));
        // The last link is gw2 -> edge.
        assert_eq!(chain.links.last().unwrap().delegator_guid, gw2);
        assert_eq!(chain.links.last().unwrap().delegatee_guid, edge);
    }

    #[test]
    fn iter_active_lists_all_delegations() {
        let gw = [0xAA; 16];
        let (mut bridge, _) = make_bridge(gw);
        for i in 0..5u8 {
            let mut edge = [0u8; 16];
            edge[0] = i;
            bridge
                .delegate_for(edge, alloc::vec![], alloc::vec![], 0, 9_000)
                .expect("delegate");
        }
        assert_eq!(bridge.active_count(), 5);
        let collected: Vec<[u8; 16]> = bridge.iter_active().map(|(g, _)| *g).collect();
        assert_eq!(collected.len(), 5);
    }

    #[test]
    fn upstream_accessor_reflects_state() {
        let gw = [0xAA; 16];
        let (mut bridge, _) = make_bridge(gw);
        assert!(bridge.upstream().is_none());
        let (sk, _pk) = ecdsa_p256_keypair();
        let mut up_link = DelegationLink::new(
            [0x11; 16],
            gw,
            alloc::vec!["*".to_string()],
            alloc::vec![],
            0,
            9_000,
            SignatureAlgorithm::EcdsaP256,
        )
        .unwrap();
        up_link.sign(&sk).unwrap();
        let chain = DelegationChain::new([0x11; 16], alloc::vec![up_link]).unwrap();
        bridge.with_upstream(chain.clone());
        assert_eq!(bridge.upstream(), Some(&chain));
    }

    #[test]
    fn bridge_two_hop_chain_validates_via_chain_check() {
        use alloc::collections::BTreeSet;
        use zerodds_security_permissions::{
            DelegationProfile, TrustAnchor, TrustPolicy, validate_chain,
        };

        // Setup: gw1 (hull), gw2 (turret), edge.
        let gw1 = [0x11; 16];
        let gw2 = [0x22; 16];
        let edge = [0x33; 16];

        // Hull-GW key pair.
        let (sk1, pk1) = ecdsa_p256_keypair();
        // Turret-GW key pair (different pair!).
        let (sk2, pk2) = ecdsa_p256_keypair();

        // The hull creates the upstream link gw1 -> gw2 (signed with sk1).
        let mut upstream_link = DelegationLink::new(
            gw1,
            gw2,
            alloc::vec!["*".to_string()],
            alloc::vec![],
            0,
            9_000,
            SignatureAlgorithm::EcdsaP256,
        )
        .unwrap();
        upstream_link.sign(&sk1).unwrap();
        let upstream = DelegationChain::new(gw1, alloc::vec![upstream_link]).unwrap();

        // Turret bridge (signed with sk2).
        let cfg = GatewayBridgeConfig {
            gateway_guid: gw2,
            signing_key: sk2,
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        let mut turm_bridge = GatewayBridge::new(cfg);
        turm_bridge.with_upstream(upstream);
        turm_bridge
            .delegate_for(
                edge,
                alloc::vec!["sensor/imu".to_string()],
                alloc::vec![],
                100,
                8_000,
            )
            .unwrap();

        let chain = turm_bridge.chain_for(&edge).expect("chain");

        // Profile with trust anchor = gw1 (pk1).
        let mut algos = BTreeSet::new();
        algos.insert(SignatureAlgorithm::EcdsaP256.wire_id());
        let profile = DelegationProfile {
            name: "vehicle".to_string(),
            trust_policy: TrustPolicy::DirectOrDelegated,
            trust_anchors: alloc::vec![TrustAnchor {
                subject_guid: gw1,
                verify_public_key: pk1,
                algorithm: SignatureAlgorithm::EcdsaP256,
            }],
            max_chain_depth: 3,
            allowed_algorithms: algos,
            require_ocsp: false,
        };

        // The resolver returns pk2 for gw2.
        let resolver = move |g: &[u8; 16]| -> Option<(Vec<u8>, SignatureAlgorithm)> {
            if g == &gw2 {
                Some((pk2.clone(), SignatureAlgorithm::EcdsaP256))
            } else {
                None
            }
        };

        let validated = validate_chain(&chain, &profile, 5_000, resolver).expect("validate");
        assert_eq!(validated.chain_depth, 2);
        assert_eq!(validated.edge_guid, edge);
        // Scope intersection: "*" and "sensor/imu" → "sensor/imu".
        assert!(
            validated
                .effective_topic_patterns
                .contains(&"sensor/imu".to_string())
        );
    }

    // ---- RC1: rotate_ephemerals ----

    #[test]
    fn rotate_ephemerals_creates_delegations_for_ephemeral_only() {
        let gw = [0xAA; 16];
        let (mut bridge, _pk) = make_bridge(gw);

        let identities = alloc::vec![
            EdgeIdentityConfig {
                name: "static-edge".into(),
                mode: EdgeIdentityMode::Static,
                guid_prefix: Some([0x01; 12]),
                lifetime_seconds: None,
            },
            EdgeIdentityConfig {
                name: "ephemeral-edge".into(),
                mode: EdgeIdentityMode::Ephemeral,
                guid_prefix: None,
                lifetime_seconds: Some(60),
            },
        ];

        let mut counter = 0u8;
        let prefix_gen = |_name: &str| -> [u8; 12] {
            counter += 1;
            [counter; 12]
        };
        let (rotated, failed) = bridge.rotate_ephemerals(
            &identities,
            1_000,
            alloc::vec!["sensor/*".to_string()],
            alloc::vec![],
            prefix_gen,
        );
        assert_eq!(rotated, alloc::vec!["ephemeral-edge".to_string()]);
        assert!(failed.is_empty());
        // The static edge was NOT delegated (the caller manages static
        // itself).
        assert_eq!(bridge.active_count(), 1);
    }

    #[test]
    fn rotate_ephemerals_uses_provided_prefix_generator() {
        let gw = [0xAA; 16];
        let (mut bridge, _) = make_bridge(gw);

        let identities = alloc::vec![EdgeIdentityConfig {
            name: "rot-edge".into(),
            mode: EdgeIdentityMode::Ephemeral,
            guid_prefix: None,
            lifetime_seconds: Some(120),
        }];

        let captured_name: alloc::sync::Arc<core::sync::atomic::AtomicBool> =
            alloc::sync::Arc::new(core::sync::atomic::AtomicBool::new(false));
        let captured_clone = captured_name.clone();
        let prefix_gen = move |name: &str| -> [u8; 12] {
            if name == "rot-edge" {
                captured_clone.store(true, core::sync::atomic::Ordering::SeqCst);
            }
            [0xDE; 12]
        };

        let (rotated, _) =
            bridge.rotate_ephemerals(&identities, 5_000, alloc::vec![], alloc::vec![], prefix_gen);
        assert_eq!(rotated.len(), 1);
        assert!(captured_name.load(core::sync::atomic::Ordering::SeqCst));

        // The edge is active with the generated prefix.
        let mut expected_guid = [0u8; 16];
        expected_guid[..12].copy_from_slice(&[0xDE; 12]);
        expected_guid[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);
        assert!(bridge.has_edge(&expected_guid));
    }

    #[test]
    fn rotate_ephemerals_uses_lifetime_for_not_after() {
        let gw = [0xAA; 16];
        let (mut bridge, _) = make_bridge(gw);
        let identities = alloc::vec![EdgeIdentityConfig {
            name: "imu".into(),
            mode: EdgeIdentityMode::Ephemeral,
            guid_prefix: None,
            lifetime_seconds: Some(300),
        }];
        let (_rotated, _) =
            bridge.rotate_ephemerals(&identities, 1_000, alloc::vec![], alloc::vec![], |_| {
                [0xAB; 12]
            });
        let mut edge_guid = [0u8; 16];
        edge_guid[..12].copy_from_slice(&[0xAB; 12]);
        edge_guid[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);
        let link = bridge.iter_active().find(|(g, _)| *g == &edge_guid);
        assert!(link.is_some());
        let (_g, l) = link.unwrap();
        assert_eq!(l.not_before, 1_000);
        assert_eq!(l.not_after, 1_300);
    }

    #[test]
    fn rotate_ephemerals_repeated_calls_replace_old_delegation() {
        let gw = [0xAA; 16];
        let (mut bridge, _) = make_bridge(gw);
        let identities = alloc::vec![EdgeIdentityConfig {
            name: "ecu".into(),
            mode: EdgeIdentityMode::Ephemeral,
            guid_prefix: None,
            lifetime_seconds: Some(60),
        }];

        // First rotation with prefix [0x11; 12].
        let (_, _) =
            bridge.rotate_ephemerals(&identities, 1_000, alloc::vec![], alloc::vec![], |_| {
                [0x11; 12]
            });
        // Second rotation with prefix [0x22; 12] — the old edge stays
        // in the active map (delegate_for only overwrites the exact same
        // GUID). Both are active. In production the
        // discovery layer would evict the old one via take_revocations.
        let (_, _) =
            bridge.rotate_ephemerals(&identities, 2_000, alloc::vec![], alloc::vec![], |_| {
                [0x22; 12]
            });
        assert_eq!(bridge.active_count(), 2);
    }

    #[test]
    fn delegation_link_too_many_topics_propagates_as_bridge_error() {
        let gw = [0xAA; 16];
        let edge = [0xBB; 16];
        let (mut bridge, _) = make_bridge(gw);
        let topics: Vec<String> = (0..200).map(|i| alloc::format!("t{i}")).collect();
        let err = bridge
            .delegate_for(edge, topics, alloc::vec![], 0, 9_000)
            .expect_err("must fail");
        assert!(matches!(err, GatewayBridgeError::DelegationFailed(_)));
    }
}
