//! E2E: Vehicle-Mesh Doppelstern + C4I-Backend.
//!
//! Architektur-Referenz: `docs/architecture/09_delegation.md` §3
//! (Use-Cases) + §6 (Chain-Validation).
//!
//! Testet die volle Datenmodell-Toolchain ueber alle Stufen j-a..j-h:
//!
//! 1. Wanne-Gateway (Trust-Anchor) erzeugt Upstream-Link zum Turm-GW.
//! 2. Turm-GW (Sub-Bridge mit Upstream) delegiert an `turm-imu` und
//!    `turm-cam`.
//! 3. Wanne-GW delegiert an `wanne-ecu` (1-Hop, gleiche Anchor).
//! 4. Caps mit DelegationChain werden via SPDP-Beacon beim C4I-Node
//!    sichtbar.
//! 5. C4I validiert die Chains gegen sein Profile
//!    (`strict-delegated`, max_chain_depth=3).
//! 6. Rogue-Peer mit gefakter Chain wird abgelehnt.
//!
//! Der Test ist DCPS-Runtime-frei — er deckt das **Datenmodell** und
//! das **Discovery-Wire-Format** ab, also exakt die Schichten, die
//! liefert das Delegation-Vehicle-Subsystem. DCPS-Integration (Sample-Routing mit
//! Re-Sealing am Bridge) ist Phase-2-Folgearbeit.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use ring::rand::SystemRandom;
use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair, KeyPair};
use zerodds_discovery::spdp::{SpdpBeacon, SpdpReader};
use zerodds_rtps::participant_data::{Duration, ParticipantBuiltinTopicData, endpoint_flag};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, ProtocolVersion, VendorId};
use zerodds_security_permissions::{DelegationProfile, TrustAnchor, TrustPolicy, validate_chain};
use zerodds_security_pki::{DelegationChain, DelegationLink, SignatureAlgorithm};
use zerodds_security_runtime::{
    GatewayBridge, GatewayBridgeConfig, PeerCache, PeerCapabilities, ProtectionLevel,
    advertise_security_caps, parse_peer_caps,
};

// ============================================================================
// Topology Setup
// ============================================================================

fn ecdsa_keypair() -> (Vec<u8>, Vec<u8>) {
    let rng = SystemRandom::new();
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
    let sk = pkcs8.as_ref().to_vec();
    let kp = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &sk, &rng).unwrap();
    (sk, kp.public_key().as_ref().to_vec())
}

fn baseline_participant(prefix: u8) -> ParticipantBuiltinTopicData {
    ParticipantBuiltinTopicData {
        guid: Guid::new(GuidPrefix::from_bytes([prefix; 12]), EntityId::PARTICIPANT),
        protocol_version: ProtocolVersion::V2_5,
        vendor_id: VendorId::ZERODDS,
        default_unicast_locator: Some(Locator::udp_v4([127, 0, 0, 1], 7410)),
        default_multicast_locator: Some(Locator::udp_v4([239, 255, 0, 1], 7400)),
        metatraffic_unicast_locator: None,
        metatraffic_multicast_locator: None,
        domain_id: Some(0),
        builtin_endpoint_set: endpoint_flag::PARTICIPANT_ANNOUNCER
            | endpoint_flag::PARTICIPANT_DETECTOR,
        lease_duration: Duration::from_secs(100),
        user_data: Vec::new(),
        properties: Default::default(),
        identity_token: None,
        permissions_token: None,
        identity_status_token: None,
        sig_algo_info: None,
        kx_algo_info: None,
        sym_cipher_algo_info: None,
    }
}

/// Vehicle-Topology: 6 Participant-GUIDs.
struct Topology {
    wanne_gw: [u8; 16],
    turm_gw: [u8; 16],
    wanne_ecu: [u8; 16],
    turm_imu: [u8; 16],
    turm_cam: [u8; 16],
    c4i: [u8; 16],
}

impl Topology {
    fn new() -> Self {
        Self {
            wanne_gw: [0x11; 16],
            turm_gw: [0x22; 16],
            wanne_ecu: [0x33; 16],
            turm_imu: [0x44; 16],
            turm_cam: [0x55; 16],
            c4i: [0x66; 16],
        }
    }
}

/// Test-Setup mit Bridges, Profiles, alle Schluessel.
struct VehicleMesh {
    topo: Topology,
    /// Wanne-GW Bridge (Trust-Root fuer C4I).
    wanne_bridge: GatewayBridge,
    /// Turm-GW Bridge (Sub-Bridge mit Upstream-Link von Wanne).
    turm_bridge: GatewayBridge,
    /// PubKey des Wanne-GW (= Trust-Anchor fuer C4I).
    wanne_pubkey: Vec<u8>,
    /// PubKey des Turm-GW (= Resolver-Output fuer 2-Hop-Verify).
    turm_pubkey: Vec<u8>,
}

impl VehicleMesh {
    fn build() -> Self {
        let topo = Topology::new();
        let (sk_wanne, pk_wanne) = ecdsa_keypair();
        let (sk_turm, pk_turm) = ecdsa_keypair();

        // Wanne-Bridge.
        let wanne_cfg = GatewayBridgeConfig {
            gateway_guid: topo.wanne_gw,
            signing_key: sk_wanne.clone(),
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        let wanne_bridge = GatewayBridge::new(wanne_cfg);

        // Wanne signiert Upstream-Link an Turm-GW (gibt Turm Recht
        // weiter zu delegieren).
        let mut upstream = DelegationLink::new(
            topo.wanne_gw,
            topo.turm_gw,
            vec!["*".to_string()],
            vec![],
            0,
            9_000,
            SignatureAlgorithm::EcdsaP256,
        )
        .unwrap();
        upstream.sign(&sk_wanne).unwrap();
        let upstream_chain = DelegationChain::new(topo.wanne_gw, vec![upstream]).unwrap();

        // Turm-Bridge mit Upstream.
        let turm_cfg = GatewayBridgeConfig {
            gateway_guid: topo.turm_gw,
            signing_key: sk_turm,
            algorithm: SignatureAlgorithm::EcdsaP256,
        };
        let mut turm_bridge = GatewayBridge::new(turm_cfg);
        turm_bridge.with_upstream(upstream_chain);

        Self {
            topo,
            wanne_bridge,
            turm_bridge,
            wanne_pubkey: pk_wanne,
            turm_pubkey: pk_turm,
        }
    }

    fn issue_all_delegations(&mut self) {
        // 1-Hop: Wanne-ECU direkt unter Wanne-GW.
        self.wanne_bridge
            .delegate_for(
                self.topo.wanne_ecu,
                vec!["wanne/*".to_string(), "telemetry/*".to_string()],
                vec![],
                100,
                8_000,
            )
            .unwrap();

        // 2-Hop: Turm-IMU + Turm-Cam unter Turm-GW (mit Upstream).
        self.turm_bridge
            .delegate_for(
                self.topo.turm_imu,
                vec!["sensor/imu".to_string()],
                vec![],
                100,
                8_000,
            )
            .unwrap();
        self.turm_bridge
            .delegate_for(
                self.topo.turm_cam,
                vec!["sensor/cam".to_string()],
                vec![],
                100,
                8_000,
            )
            .unwrap();
    }

    /// C4I-Profile: strict-delegated + Trust-Anchor=Wanne-GW.
    fn c4i_profile(&self) -> DelegationProfile {
        use std::collections::BTreeSet;
        let mut algos = BTreeSet::new();
        algos.insert(SignatureAlgorithm::EcdsaP256.wire_id());
        DelegationProfile {
            name: "c4i-via-wanne".to_string(),
            trust_policy: TrustPolicy::StrictDelegated,
            trust_anchors: vec![TrustAnchor {
                subject_guid: self.topo.wanne_gw,
                verify_public_key: self.wanne_pubkey.clone(),
                algorithm: SignatureAlgorithm::EcdsaP256,
            }],
            max_chain_depth: 3,
            allowed_algorithms: algos,
            require_ocsp: false,
        }
    }

    /// Resolver fuer Sub-Hop-PubKeys. Fuer Turm-Sensoren liefert er
    /// pk_turm wenn nach turm_gw gefragt wird.
    fn pubkey_resolver(&self) -> impl Fn(&[u8; 16]) -> Option<(Vec<u8>, SignatureAlgorithm)> + '_ {
        move |g: &[u8; 16]| {
            if g == &self.topo.turm_gw {
                Some((self.turm_pubkey.clone(), SignatureAlgorithm::EcdsaP256))
            } else {
                None
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[test]
fn one_hop_wanne_ecu_passes_c4i_validation() {
    let mut mesh = VehicleMesh::build();
    mesh.issue_all_delegations();
    let chain = mesh.wanne_bridge.chain_for(&mesh.topo.wanne_ecu).unwrap();
    assert_eq!(chain.depth(), 1);
    let validated = validate_chain(&chain, &mesh.c4i_profile(), 5_000, mesh.pubkey_resolver())
        .expect("c4i must accept wanne-ecu");
    assert_eq!(validated.edge_guid, mesh.topo.wanne_ecu);
    assert_eq!(validated.origin_guid, mesh.topo.wanne_gw);
}

#[test]
fn two_hop_turm_imu_passes_c4i_validation() {
    let mut mesh = VehicleMesh::build();
    mesh.issue_all_delegations();
    let chain = mesh.turm_bridge.chain_for(&mesh.topo.turm_imu).unwrap();
    assert_eq!(chain.depth(), 2);
    let validated = validate_chain(&chain, &mesh.c4i_profile(), 5_000, mesh.pubkey_resolver())
        .expect("c4i must accept turm-imu via 2-hop");
    assert_eq!(validated.edge_guid, mesh.topo.turm_imu);
    assert_eq!(validated.origin_guid, mesh.topo.wanne_gw);
    // Scope-Intersection: "*" ∩ "sensor/imu" → "sensor/imu".
    assert!(
        validated
            .effective_topic_patterns
            .contains(&"sensor/imu".to_string())
    );
}

#[test]
fn two_hop_turm_cam_passes_c4i_validation_with_distinct_scope() {
    let mut mesh = VehicleMesh::build();
    mesh.issue_all_delegations();
    let chain = mesh.turm_bridge.chain_for(&mesh.topo.turm_cam).unwrap();
    let validated = validate_chain(&chain, &mesh.c4i_profile(), 5_000, mesh.pubkey_resolver())
        .expect("c4i must accept turm-cam");
    // Cam darf nur "sensor/cam", nicht "sensor/imu" (Scope-Per-Edge).
    assert!(validated.allows_topic("sensor/cam"));
    assert!(!validated.allows_topic("sensor/imu"));
}

#[test]
fn rogue_peer_with_self_signed_chain_rejected_by_c4i() {
    // Rogue: kennt nicht den Wanne-GW-Schluessel, signiert eine
    // Chain mit eigenem Key und behauptet zur wanne_gw zu gehoeren.
    let mesh = VehicleMesh::build();
    let (sk_rogue, _pk_rogue) = ecdsa_keypair();
    let rogue_edge = [0xFF; 16];
    let mut rogue_link = DelegationLink::new(
        mesh.topo.wanne_gw,
        rogue_edge,
        vec!["*".to_string()],
        vec![],
        0,
        9_000,
        SignatureAlgorithm::EcdsaP256,
    )
    .unwrap();
    rogue_link.sign(&sk_rogue).unwrap();
    let rogue_chain = DelegationChain::new(mesh.topo.wanne_gw, vec![rogue_link]).unwrap();

    let err = validate_chain(
        &rogue_chain,
        &mesh.c4i_profile(),
        5_000,
        mesh.pubkey_resolver(),
    )
    .expect_err("c4i must reject rogue");
    // SignatureInvalid weil rogue-key ≠ trust-anchor pk.
    assert!(matches!(
        err,
        zerodds_security_permissions::DelegationCheckError::SignatureInvalid { .. }
    ));
}

#[test]
fn rogue_peer_with_unknown_origin_rejected() {
    // Rogue mit Origin = unbekannte GUID → UntrustedDelegator.
    let mesh = VehicleMesh::build();
    let (sk_rogue, _pk_rogue) = ecdsa_keypair();
    let rogue_origin = [0xEE; 16];
    let rogue_edge = [0xFF; 16];
    let mut rogue_link = DelegationLink::new(
        rogue_origin,
        rogue_edge,
        vec!["*".to_string()],
        vec![],
        0,
        9_000,
        SignatureAlgorithm::EcdsaP256,
    )
    .unwrap();
    rogue_link.sign(&sk_rogue).unwrap();
    let rogue_chain = DelegationChain::new(rogue_origin, vec![rogue_link]).unwrap();
    let err = validate_chain(
        &rogue_chain,
        &mesh.c4i_profile(),
        5_000,
        mesh.pubkey_resolver(),
    )
    .expect_err("c4i must reject");
    assert!(matches!(
        err,
        zerodds_security_permissions::DelegationCheckError::UntrustedDelegator
    ));
}

#[test]
fn full_pipeline_through_spdp_beacon() {
    // Volle Schichten-Kette:
    //   Bridge.chain_for → caps.delegation_chain → advertise_security_caps
    //   → SpdpBeacon.serialize → Datagram → SpdpReader.parse_datagram
    //   → parse_peer_caps → validate_chain.
    let mut mesh = VehicleMesh::build();
    mesh.issue_all_delegations();

    // Wanne-ECU annonciert sein Beacon.
    let mut data = baseline_participant(0x33);
    let chain = mesh.wanne_bridge.chain_for(&mesh.topo.wanne_ecu).unwrap();
    let mut caps = PeerCapabilities {
        auth_plugin_class: None, // edge ohne eigenen plugin
        crypto_plugin_class: None,
        access_plugin_class: None,
        supported_suites: vec![],
        offered_protection: ProtectionLevel::None,
        has_valid_cert: false,
        validity_window: None,
        vendor_hint: Some("zerodds-edge".into()),
        cert_cn: None,
        delegation_chain: Some(chain.clone()),
    };
    // sanity-check: caps haengt
    let _ = &caps;
    advertise_security_caps(&mut data.properties, &caps);
    let mut beacon = SpdpBeacon::new(data);
    let datagram = beacon.serialize().unwrap();

    // C4I empfaengt + parsed.
    let disc = SpdpReader::new().parse_datagram(&datagram).unwrap();
    let parsed_caps = parse_peer_caps(&disc.data.properties);
    let parsed_chain = parsed_caps
        .delegation_chain
        .as_ref()
        .expect("chain present")
        .clone();
    assert_eq!(parsed_chain, chain);

    // C4I validiert.
    let validated = validate_chain(
        &parsed_chain,
        &mesh.c4i_profile(),
        5_000,
        mesh.pubkey_resolver(),
    )
    .expect("c4i validates chain");
    assert_eq!(validated.edge_guid, mesh.topo.wanne_ecu);

    // mesh.topo.c4i ist nur fuer Bookkeeping, nicht fuer Wire-Adresse.
    assert_eq!(mesh.topo.c4i, [0x66; 16]);

    // PeerCache-Eintrag wuerde dieselbe Chain enthalten.
    let mut cache = PeerCache::new();
    cache.insert(disc.data.guid.prefix.0, parsed_caps.clone());
    let cached = cache.get(&disc.data.guid.prefix.0).unwrap();
    assert!(cached.delegation_chain.is_some());

    // caps-noop just to keep variable used:
    caps.delegation_chain = None;
}

#[test]
fn doppelstern_full_topology_three_chains_independent() {
    let mut mesh = VehicleMesh::build();
    mesh.issue_all_delegations();

    // Alle drei Edges produzieren ihre Chain.
    let ch_ecu = mesh.wanne_bridge.chain_for(&mesh.topo.wanne_ecu).unwrap();
    let ch_imu = mesh.turm_bridge.chain_for(&mesh.topo.turm_imu).unwrap();
    let ch_cam = mesh.turm_bridge.chain_for(&mesh.topo.turm_cam).unwrap();

    assert_eq!(ch_ecu.depth(), 1);
    assert_eq!(ch_imu.depth(), 2);
    assert_eq!(ch_cam.depth(), 2);
    // Origin ist immer Wanne-GW (Trust-Root).
    assert_eq!(ch_ecu.origin_guid, mesh.topo.wanne_gw);
    assert_eq!(ch_imu.origin_guid, mesh.topo.wanne_gw);
    assert_eq!(ch_cam.origin_guid, mesh.topo.wanne_gw);

    let profile = mesh.c4i_profile();
    let resolver = mesh.pubkey_resolver();

    // Per Edge ein eigener resolver-Aufruf — alle drei muessen
    // unabhaengig validieren.
    let v1 = validate_chain(&ch_ecu, &profile, 5_000, &resolver).unwrap();
    let v2 = validate_chain(&ch_imu, &profile, 5_000, &resolver).unwrap();
    let v3 = validate_chain(&ch_cam, &profile, 5_000, &resolver).unwrap();

    // Edges sind unterschiedlich, Origins identisch.
    assert_ne!(v1.edge_guid, v2.edge_guid);
    assert_ne!(v2.edge_guid, v3.edge_guid);
    assert_eq!(v1.origin_guid, v3.origin_guid);

    // Scope-Per-Edge: keine Vermischung der Patterns.
    assert!(v2.allows_topic("sensor/imu"));
    assert!(!v2.allows_topic("sensor/cam"));
    assert!(v3.allows_topic("sensor/cam"));
    assert!(!v3.allows_topic("sensor/imu"));
}

#[test]
fn revoked_delegation_no_longer_chains() {
    let mut mesh = VehicleMesh::build();
    mesh.issue_all_delegations();
    // Turm-IMU wird widerrufen.
    mesh.turm_bridge
        .revoke_delegation(mesh.topo.turm_imu)
        .unwrap();
    // chain_for liefert None nach Revoke.
    assert!(mesh.turm_bridge.chain_for(&mesh.topo.turm_imu).is_none());
    // Aber Cam und Wanne-ECU bleiben.
    assert!(mesh.turm_bridge.chain_for(&mesh.topo.turm_cam).is_some());
    assert!(mesh.wanne_bridge.chain_for(&mesh.topo.wanne_ecu).is_some());
    // Revocation-Liste enthaelt den IMU-Eintrag fuer den naechsten
    // SPDP-Beacon.
    let revs = mesh.turm_bridge.take_revocations();
    assert_eq!(revs, vec![mesh.topo.turm_imu]);
}
