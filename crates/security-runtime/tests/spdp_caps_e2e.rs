//! E2E: Security-Caps durch den SPDP-Beacon-Kanal.
//!
//! Kette: PeerCapabilities → advertise_security_caps → WirePropertyList
//! → ParticipantBuiltinTopicData.properties → SpdpBeacon.serialize() →
//! Datagram → SpdpReader.parse_datagram() → parse_peer_caps() →
//! PeerCapabilities (beim Empfaenger).
//!
//! Damit ist der DoD-Punkt "Participant A annonciert, Participant B
//! sieht die Caps im PeerCache" abgedeckt. Cyclone-Live-Interop
//! folgt separat auf `ssh llvm@llvm`.

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

use zerodds_discovery::spdp::{SpdpBeacon, SpdpReader};
use zerodds_rtps::participant_data::{Duration, ParticipantBuiltinTopicData, endpoint_flag};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, Locator, ProtocolVersion, VendorId};
use zerodds_security_runtime::{
    PeerCache, PeerCapabilities, ProtectionLevel, SuiteHint, advertise_security_caps,
    parse_peer_caps,
};

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

fn secure_caps() -> PeerCapabilities {
    PeerCapabilities {
        auth_plugin_class: Some("DDS:Auth:PKI-DH:1.2".into()),
        access_plugin_class: Some("DDS:Access:Permissions:1.2".into()),
        crypto_plugin_class: Some("DDS:Crypto:AES-GCM-GMAC:1.2".into()),
        supported_suites: vec![SuiteHint::Aes128Gcm, SuiteHint::Aes256Gcm],
        offered_protection: ProtectionLevel::Encrypt,
        has_valid_cert: false,
        validity_window: None,
        vendor_hint: Some("zerodds".into()),
        cert_cn: None,
        delegation_chain: None,
    }
}

#[test]
fn secure_caps_roundtrip_through_spdp_beacon() {
    // --- Sender-Seite ---
    let mut data_a = baseline_participant(0xAA);
    advertise_security_caps(&mut data_a.properties, &secure_caps());

    let mut beacon = SpdpBeacon::new(data_a.clone());
    let datagram = beacon.serialize().expect("serialize beacon");

    // --- Empfaenger-Seite ---
    let discovered = SpdpReader::new()
        .parse_datagram(&datagram)
        .expect("parse beacon");

    // ParticipantBuiltinTopicData kommt byte-konsistent zurueck
    assert_eq!(discovered.data.guid, data_a.guid);
    assert_eq!(discovered.data.properties, data_a.properties);

    // PeerCapabilities werden korrekt geparst
    let caps = parse_peer_caps(&discovered.data.properties);
    assert_eq!(
        caps.auth_plugin_class.as_deref(),
        Some("DDS:Auth:PKI-DH:1.2")
    );
    assert_eq!(caps.offered_protection, ProtectionLevel::Encrypt);
    assert_eq!(
        caps.supported_suites,
        vec![SuiteHint::Aes128Gcm, SuiteHint::Aes256Gcm]
    );
    assert_eq!(caps.vendor_hint.as_deref(), Some("zerodds"));
}

#[test]
fn legacy_peer_without_caps_lands_as_none_in_cache() {
    // Phase-0-Peer oder Cyclone ohne Security schickt ein SPDP ohne
    // PropertyList — Architektur-Doc §2.1 (2): "ein Peer ohne
    // auth_plugin_class in SPDP wird als Legacy klassifiziert,
    // nicht gedroppt".
    let data_legacy = baseline_participant(0x11);
    let mut beacon = SpdpBeacon::new(data_legacy.clone());
    let datagram = beacon.serialize().unwrap();

    let discovered = SpdpReader::new().parse_datagram(&datagram).unwrap();
    let caps = parse_peer_caps(&discovered.data.properties);

    assert!(caps.auth_plugin_class.is_none());
    assert_eq!(caps.offered_protection, ProtectionLevel::None);
}

#[test]
fn two_participants_land_independently_in_peer_cache() {
    // Zwei verschiedene Sender, ein gemeinsamer PeerCache beim
    // Empfaenger — GuidPrefix unterscheidet die Eintraege.
    let mut data_a = baseline_participant(0xAA);
    advertise_security_caps(&mut data_a.properties, &secure_caps());

    let data_b = baseline_participant(0xBB); // Legacy
    // (kein advertise fuer B)

    let mut beacon_a = SpdpBeacon::new(data_a.clone());
    let mut beacon_b = SpdpBeacon::new(data_b.clone());
    let dg_a = beacon_a.serialize().unwrap();
    let dg_b = beacon_b.serialize().unwrap();

    let reader = SpdpReader::new();
    let disc_a = reader.parse_datagram(&dg_a).unwrap();
    let disc_b = reader.parse_datagram(&dg_b).unwrap();

    let mut cache = PeerCache::new();
    cache.insert(
        disc_a.data.guid.prefix.0,
        parse_peer_caps(&disc_a.data.properties),
    );
    cache.insert(
        disc_b.data.guid.prefix.0,
        parse_peer_caps(&disc_b.data.properties),
    );

    assert_eq!(cache.len(), 2);
    let caps_a = cache.get(&[0xAA; 12]).unwrap();
    let caps_b = cache.get(&[0xBB; 12]).unwrap();
    assert_eq!(caps_a.offered_protection, ProtectionLevel::Encrypt);
    assert_eq!(caps_b.offered_protection, ProtectionLevel::None);
    assert!(caps_b.auth_plugin_class.is_none());
}

#[test]
fn peer_cache_upgrade_path_via_update_partial() {
    // Arch-Doc §4.3: Peer war initial als auth_plugin=None bekannt,
    // schickt dann ein Extended-SPDP mit Security-Caps → Cache wird
    // aktualisiert, nicht ueberschrieben.
    let mut cache = PeerCache::new();
    let key = [0x55u8; 12];

    // Initial: Legacy-Beacon (kein PropertyList)
    let data_legacy = baseline_participant(0x55);
    let mut beacon = SpdpBeacon::new(data_legacy.clone());
    let dg1 = beacon.serialize().unwrap();
    let disc1 = SpdpReader::new().parse_datagram(&dg1).unwrap();
    cache.insert(key, parse_peer_caps(&disc1.data.properties));
    assert!(cache.get(&key).unwrap().auth_plugin_class.is_none());

    // Zweiter Beacon: mit Security-Caps (Peer hat Security nachgezogen)
    let mut data_upgraded = baseline_participant(0x55);
    advertise_security_caps(&mut data_upgraded.properties, &secure_caps());
    let mut beacon2 = SpdpBeacon::new(data_upgraded.clone());
    let dg2 = beacon2.serialize().unwrap();
    let disc2 = SpdpReader::new().parse_datagram(&dg2).unwrap();
    cache.update_partial(key, &parse_peer_caps(&disc2.data.properties));

    // Nach Upgrade sind die Caps im Cache.
    let merged = cache.get(&key).unwrap();
    assert_eq!(merged.offered_protection, ProtectionLevel::Encrypt);
    assert_eq!(
        merged.auth_plugin_class.as_deref(),
        Some("DDS:Auth:PKI-DH:1.2")
    );
}

#[test]
fn extra_zerodds_properties_are_ignored_by_vendor_agnostic_parse() {
    // Wenn ein ZeroDDS-Peer uns unbekannte zerodds.sec.*-Properties
    // schickt, darf parse_peer_caps nicht crashen/droppen —
    // Forward-Compat fuer spaetere Protokoll-Erweiterungen.
    let mut data = baseline_participant(0x22);
    advertise_security_caps(&mut data.properties, &secure_caps());
    data.properties
        .push(zerodds_rtps::property_list::WireProperty::new(
            "zerodds.sec.future_extension",
            "some-opaque-value",
        ));

    let mut beacon = SpdpBeacon::new(data);
    let dg = beacon.serialize().unwrap();
    let disc = SpdpReader::new().parse_datagram(&dg).unwrap();
    let caps = parse_peer_caps(&disc.data.properties);
    assert_eq!(caps.offered_protection, ProtectionLevel::Encrypt);
}

// ============================================================================
// Delegation-Chain via SPDP
// ============================================================================

#[test]
fn delegation_chain_zero_links_treated_as_none() {
    // Empty Chain wird im Cap-Modell als None gehandhabt — der
    // wire-Encode laesst delegation_chain=None weg, parse liefert None.
    let mut data = baseline_participant(0x33);
    advertise_security_caps(&mut data.properties, &secure_caps());
    let mut beacon = SpdpBeacon::new(data);
    let dg = beacon.serialize().unwrap();
    let disc = SpdpReader::new().parse_datagram(&dg).unwrap();
    let caps = parse_peer_caps(&disc.data.properties);
    assert!(caps.delegation_chain.is_none());
}

#[test]
fn delegation_chain_one_hop_roundtrip_via_spdp() {
    use ring::rand::SystemRandom;
    use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair};
    use zerodds_security_pki::{DelegationChain, DelegationLink, SignatureAlgorithm};

    let rng = SystemRandom::new();
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
    let sk = pkcs8.as_ref().to_vec();
    let _kp = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &sk, &rng).unwrap();

    let gw = [0xAA; 16];
    let edge = [0xBB; 16];
    let mut link = DelegationLink::new(
        gw,
        edge,
        vec!["sensor/*".into()],
        vec![],
        1_000,
        9_000,
        SignatureAlgorithm::EcdsaP256,
    )
    .unwrap();
    link.sign(&sk).unwrap();
    let chain = DelegationChain::new(gw, vec![link]).unwrap();

    let mut caps = secure_caps();
    caps.delegation_chain = Some(chain.clone());

    let mut data = baseline_participant(0xBB);
    advertise_security_caps(&mut data.properties, &caps);

    let mut beacon = SpdpBeacon::new(data);
    let dg = beacon.serialize().unwrap();
    let disc = SpdpReader::new().parse_datagram(&dg).unwrap();
    let parsed_caps = parse_peer_caps(&disc.data.properties);
    let parsed_chain = parsed_caps.delegation_chain.expect("chain present");
    assert_eq!(parsed_chain, chain);
    assert_eq!(parsed_chain.depth(), 1);
    assert_eq!(parsed_chain.edge_guid(), Some(edge));
}

#[test]
fn delegation_chain_two_hops_roundtrip_via_spdp() {
    use ring::rand::SystemRandom;
    use ring::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair};
    use zerodds_security_pki::{DelegationChain, DelegationLink, SignatureAlgorithm};

    let rng = SystemRandom::new();
    let pk1 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
    let pk2 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
    let sk1 = pk1.as_ref().to_vec();
    let sk2 = pk2.as_ref().to_vec();

    let gw = [0xAA; 16];
    let mid = [0xCC; 16];
    let edge = [0xBB; 16];
    let mut l1 = DelegationLink::new(
        gw,
        mid,
        vec!["*".into()],
        vec![],
        1_000,
        9_000,
        SignatureAlgorithm::EcdsaP256,
    )
    .unwrap();
    l1.sign(&sk1).unwrap();
    let mut l2 = DelegationLink::new(
        mid,
        edge,
        vec!["sensor/lidar".into()],
        vec![],
        1_000,
        9_000,
        SignatureAlgorithm::EcdsaP256,
    )
    .unwrap();
    l2.sign(&sk2).unwrap();
    let chain = DelegationChain::new(gw, vec![l1, l2]).unwrap();

    let mut caps = secure_caps();
    caps.delegation_chain = Some(chain.clone());

    let mut data = baseline_participant(0xBB);
    advertise_security_caps(&mut data.properties, &caps);
    let mut beacon = SpdpBeacon::new(data);
    let dg = beacon.serialize().unwrap();
    let disc = SpdpReader::new().parse_datagram(&dg).unwrap();
    let parsed_caps = parse_peer_caps(&disc.data.properties);
    let parsed_chain = parsed_caps.delegation_chain.expect("chain present");
    assert_eq!(parsed_chain.depth(), 2);
    assert_eq!(parsed_chain, chain);
}

#[test]
fn malformed_delegation_chain_property_treated_as_none() {
    // Property mit garbage-Wert darf parse_peer_caps nicht crashen
    // und das chain-Feld muss None bleiben (DoS-Defense).
    let mut data = baseline_participant(0xDD);
    advertise_security_caps(&mut data.properties, &secure_caps());
    data.properties
        .push(zerodds_rtps::property_list::WireProperty::new(
            "zerodds.sec.delegation_chain",
            "this-is-not-base64-or-valid-chain",
        ));
    let mut beacon = SpdpBeacon::new(data);
    let dg = beacon.serialize().unwrap();
    let disc = SpdpReader::new().parse_datagram(&dg).unwrap();
    let caps = parse_peer_caps(&disc.data.properties);
    assert!(caps.delegation_chain.is_none());
}
