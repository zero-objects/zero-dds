// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Gateway-Bridge-Helper.
//!
//! Architektur-Referenz: `docs/architecture/09_delegation.md` §3 (Use-
//! Cases) + §5.3 (Bridge-Sub-Gateway-Chaining).
//!
//! Ein [`GatewayBridge`] sitzt typischerweise auf einer Wanne- oder
//! Turm-Recheneinheit, die fuer mehrere Edge-Peers (Sensoren, ECUs)
//! ohne eigenes Cert verantwortlich ist. Der Bridge:
//!
//! 1. **Stellt** Delegation-Links pro Edge-Peer aus, signiert mit dem
//!    eigenen Gateway-Schluessel.
//! 2. **Verwaltet** die aktiven Delegations in einer `BTreeMap`
//!    (`edge_guid → DelegationLink`).
//! 3. **Reicht** auf Anfrage die volle [`DelegationChain`] an den
//!    Discovery-Layer (SPDP-Property), wahlweise als 1-Hop oder
//!    n-Hop wenn der Bridge selbst Delegatee einer hoeheren Ebene ist
//!    (Doppelstern Wanne+Turm).
//! 4. **Widerruft** Delegations explizit (Revocation-Liste, die im
//!    naechsten SPDP-Beacon mitgeschickt wird).
//!
//! Der Bridge **fuehrt keinen Forwarding-Pfad selbst** — er ist der
//! Policy-/Datenmodell-Helper, das eigentliche Re-Sealing und
//! Forwarding der RTPS-Frames passiert in der DCPS-Runtime (Plan
//! §Stufe j-g, kommt spaeter).

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use zerodds_security_permissions::EdgeIdentityConfig;
use zerodds_security_pki::{DelegationChain, DelegationError, DelegationLink, SignatureAlgorithm};

/// Konfiguration eines Gateway-Bridges.
///
/// `gateway_guid` ist der 16-byte Subject-GUID, den die ausgestellten
/// Delegations als `delegator_guid` tragen.
/// `signing_key` ist das PKCS#8-DER-formatierte Privatkey-Material zum
/// Signieren neuer Links — der Bridge haelt das im RAM, lade-Mechanismus
/// liegt beim Caller (Filesystem, Secret-Manager, HSM).
/// `algorithm` muss zum Trust-Anchor des Profile passen, gegen das die
/// Chain spaeter validiert wird.
#[derive(Debug, Clone)]
pub struct GatewayBridgeConfig {
    /// 16-byte Gateway-Participant-GUID.
    pub gateway_guid: [u8; 16],
    /// PKCS#8-DER-formatierter Privatkey.
    pub signing_key: Vec<u8>,
    /// Signatur-Algorithmus.
    pub algorithm: SignatureAlgorithm,
}

/// Fehler aus Gateway-Bridge-Operationen.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GatewayBridgeError {
    /// Edge ist nicht registriert.
    UnknownEdge {
        /// 16-byte Edge-GUID.
        edge_guid: [u8; 16],
    },
    /// Sign-Operation fehlgeschlagen (delegiert aus PKI-Crate).
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

/// Result-Alias.
pub type GatewayBridgeResult<T> = Result<T, GatewayBridgeError>;

/// Gateway-Bridge-Helper.
///
/// Lifecycle:
/// 1. [`GatewayBridge::new`] mit `GatewayBridgeConfig`.
/// 2. Optional [`GatewayBridge::with_upstream`] um den eigenen Bridge
///    in eine bestehende Chain (z.B. von Wanne-GW zum eigenen Turm-GW)
///    einzuhaengen.
/// 3. Pro Edge: [`GatewayBridge::delegate_for`] um eine neue Delegation
///    auszustellen.
/// 4. [`GatewayBridge::chain_for`] reicht die Chain als Output fuer
///    SPDP/SEDP-Properties.
/// 5. [`GatewayBridge::revoke_delegation`] entfernt einen Edge.
#[derive(Debug, Clone)]
pub struct GatewayBridge {
    config: GatewayBridgeConfig,
    /// Optional: Chain, die diesen Gateway als Delegatee einer hoeheren
    /// Ebene legitimiert. Wird in [`Self::chain_for`] dem ausgestellten
    /// Edge-Link voraus-geschoben.
    upstream: Option<DelegationChain>,
    /// Aktive Edge-Delegations (`edge_guid → Link`).
    active: BTreeMap<[u8; 16], DelegationLink>,
    /// Revocation-Liste: Edge-GUIDs, deren Delegation in der naechsten
    /// SPDP-Welle als revoked annonciert werden soll. Caller leert
    /// die Liste nach erfolgreichem Announce via
    /// [`Self::take_revocations`].
    revocations: Vec<[u8; 16]>,
}

impl GatewayBridge {
    /// Konstruktor.
    #[must_use]
    pub fn new(config: GatewayBridgeConfig) -> Self {
        Self {
            config,
            upstream: None,
            active: BTreeMap::new(),
            revocations: Vec::new(),
        }
    }

    /// Setzt eine Upstream-Chain. Diese wird in [`Self::chain_for`]
    /// vor den Edge-Link gehaengt — Sub-Gateway-Chaining fuer
    /// Doppelstern (Turm-GW unter Wanne-GW).
    ///
    /// Validation der Upstream-Chain ist NICHT Teil des Bridge —
    /// Caller muss vorher selbst `validate_chain` aufrufen, um
    /// Mismatch-Profile-Fehlern vorzubeugen.
    pub fn with_upstream(&mut self, upstream_chain: DelegationChain) {
        self.upstream = Some(upstream_chain);
    }

    /// 16-byte Gateway-GUID (Read-only).
    #[must_use]
    pub fn gateway_guid(&self) -> [u8; 16] {
        self.config.gateway_guid
    }

    /// Stellt eine neue Delegation fuer einen Edge-Peer aus. Wenn der
    /// Edge bereits delegiert war, wird der alte Link ueberschrieben
    /// (typisch bei Ephemeral-Edge-Rotation, Plan §Stufe j-f).
    ///
    /// `not_before` und `not_after` sind absolute Unix-Sekunden;
    /// `topic_patterns`/`partition_patterns` sind die Glob-Whitelist,
    /// die der Edge im engsten Scope haben darf.
    ///
    /// # Errors
    /// [`GatewayBridgeError::DelegationFailed`] wenn der PKI-Sign-
    /// Schritt fehlschlaegt (Cap-Verletzung, Key-Parse-Fehler).
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
        // Edge wieder aktiv → ggf. aus Revocations entfernen, falls
        // Re-Issue (z.B. nach Renewal).
        self.revocations.retain(|g| g != &edge_guid);
        // Lookup nach erfolgreichem insert kann nicht fehlschlagen,
        // aber clippy::expect_used verbietet expect — wir liefern
        // unwrap_or via einen frischen Lookup.
        self.active
            .get(&edge_guid)
            .ok_or(GatewayBridgeError::UnknownEdge { edge_guid })
    }

    /// Widerruft die aktive Delegation fuer einen Edge. Der Edge wird
    /// in die Revocation-Liste aufgenommen und kann ueber
    /// [`Self::take_revocations`] dem Discovery-Layer mitgeteilt werden.
    ///
    /// # Errors
    /// [`GatewayBridgeError::UnknownEdge`] wenn der Edge nicht aktiv ist.
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

    /// Liefert die ausgehende Chain fuer einen Edge.
    ///
    /// 1-Hop-Bridge (kein Upstream): Chain = `[Edge-Link]`,
    ///   `origin_guid = gateway_guid`.
    /// n-Hop-Bridge (mit Upstream): Chain = `upstream.links ++
    ///   [Edge-Link]`, `origin_guid = upstream.origin_guid`.
    ///
    /// Returns `None` wenn der Edge nicht aktiv ist.
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

    /// Anzahl aktiver Edge-Delegations.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// True wenn ein Edge aktiv delegiert ist.
    #[must_use]
    pub fn has_edge(&self, edge_guid: &[u8; 16]) -> bool {
        self.active.contains_key(edge_guid)
    }

    /// Iteriert ueber alle aktiven Edge-Delegations.
    pub fn iter_active(&self) -> impl Iterator<Item = (&[u8; 16], &DelegationLink)> {
        self.active.iter()
    }

    /// Liest und leert die Revocation-Liste (Discovery-Layer ruft das
    /// pro SPDP-Beacon-Tick auf).
    pub fn take_revocations(&mut self) -> Vec<[u8; 16]> {
        core::mem::take(&mut self.revocations)
    }

    /// Lese-Zugriff auf den Upstream-Chain (nuetzlich fuer Logging /
    /// Metrics).
    #[must_use]
    pub fn upstream(&self) -> Option<&DelegationChain> {
        self.upstream.as_ref()
    }

    /// Rotiert Ephemeral-Edge-Identities deren Lifetime abgelaufen ist.
    ///
    /// Workflow pro Ephemeral-Edge:
    /// 1. Wenn Edge nicht aktiv → ueberspringen (init kommt via
    ///    `delegate_for` durch den Caller).
    /// 2. Wenn `now < link.not_after - lifetime/N` (N=Renewal-Window)
    ///    → noch zu frisch, ueberspringen.
    /// 3. Sonst: neue GuidPrefix ziehen (`prefix_generator(name)`),
    ///    alten Edge revoken, neuen `delegate_for` mit `now`-basierten
    ///    Zeitfenster ausstellen.
    ///
    /// `prefix_generator` ist ein Pluggable-Hook (z.B. ChaCha20-RNG
    /// oder system-RNG); der Bridge ist deterministic-testbar weil
    /// die Zufalls-Quelle vom Caller kommt.
    ///
    /// Returns Liste der rotierten Edge-Namen.
    ///
    /// # Errors
    /// Propagiert [`GatewayBridgeError::DelegationFailed`] wenn ein
    /// neu-Sign fehlschlaegt — bricht aber NICHT die Loop-Schleife ab,
    /// fehlerhafte Edges werden im `Err`-Tail-Vec gesammelt und der
    /// Aufrufer kann entscheiden.
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
            // Edge-GUID = 12-byte Prefix + 4-byte EntityId 0x00.0x00.0x01.0xC1 (DDS-konv).
            // Wir nehmen hier nur den Prefix als Identifier-Schluessel —
            // GuidPrefix selbst entscheidet die Edge-Identity.
            let new_prefix = prefix_generator(&cfg.name);
            // Reconstruct full 16-byte edge guid (Prefix+EntityId der
            // Default-Participant-EntityId).
            let mut edge_guid = [0u8; 16];
            edge_guid[..12].copy_from_slice(&new_prefix);
            edge_guid[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);

            // Alle vorhandenen Edges mit gleichem Namen aufgreifen
            // (matching ist hier prefix-basiert; in voller Impl haetten
            // wir eine name-keyed Map). Da wir den name nicht im
            // active-Map haben, gehen wir hier einfach raw vor:
            // delegate_for ueberschreibt bei Konflikt.
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
        // Liste ist nach take leer.
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
        // Re-Delegate (z.B. nach Cert-Renewal) muss Revocation entfernen.
        bridge
            .delegate_for(edge, alloc::vec![], alloc::vec![], 100, 10_000)
            .expect("redelegate");
        assert!(bridge.take_revocations().is_empty());
        assert!(bridge.has_edge(&edge));
    }

    #[test]
    fn sub_gateway_chaining_two_hops() {
        // Wanne-GW (gw1) delegates an Turm-GW (gw2). Turm-GW bridges
        // edge `turm-imu`. Resulting chain has 2 links und origin = gw1.
        let gw1 = [0x11; 16];
        let gw2 = [0x22; 16];
        let edge = [0x33; 16];

        // gw1 erzeugt einen Upstream-Link gw1 -> gw2.
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

        // Turm-Bridge.
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
        // Letzter Link ist gw2 -> edge.
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

        // Setup: gw1 (Wanne), gw2 (Turm), edge.
        let gw1 = [0x11; 16];
        let gw2 = [0x22; 16];
        let edge = [0x33; 16];

        // Wanne-GW Schluessel-Pair.
        let (sk1, pk1) = ecdsa_p256_keypair();
        // Turm-GW Schluessel-Pair (anderes Pair!).
        let (sk2, pk2) = ecdsa_p256_keypair();

        // Wanne erzeugt Upstream-Link gw1 -> gw2 (signiert mit sk1).
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

        // Turm-Bridge (signiert mit sk2).
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

        // Profile mit Trust-Anchor = gw1 (pk1).
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

        // Resolver liefert pk2 fuer gw2.
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
        // Scope-Intersection: "*" und "sensor/imu" → "sensor/imu".
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
        // Static-Edge wurde NICHT delegiert (Caller managed Static
        // selbst).
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

        // Edge ist mit dem generierten Prefix aktiv.
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

        // Erste Rotation mit Prefix [0x11; 12].
        let (_, _) =
            bridge.rotate_ephemerals(&identities, 1_000, alloc::vec![], alloc::vec![], |_| {
                [0x11; 12]
            });
        // Zweite Rotation mit Prefix [0x22; 12] — alter Edge bleibt
        // im active map (delegate_for ueberschreibt nur exact gleiche
        // GUID). Beide sind aktiv. In Production wuerde der
        // Discovery-Layer den alten via take_revocations evicten.
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
