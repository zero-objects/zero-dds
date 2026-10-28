// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Per-topic governance on the wire: one governance, two `<topic_rule>`
//! elements, two user topics — one encrypted, one in the clear, in the same
//! domain and the same pair of participants.
//!
//! DDS-Security 1.2 §10.4.1.2.6 hangs every protection attribute off the
//! `<topic_rule>` whose `<topic_expression>` matches the endpoint's topic
//! name. Reading `topic_rules.first()` instead gives every endpoint of the
//! participant the protection of whichever rule the governance author wrote
//! down first — with the rule order below that means `OD_OA_OM_OD` (all
//! `NONE`) decides, and the payload of `PD_RWA_EM_ED` goes out in plaintext.
//!
//! The two topic names are the OMG DDS-Security interop names:
//! `PD_RWA_EM_ED` = protected discovery / read-write access control /
//! encrypted metadata / encrypted data; `OD_OA_OM_OD` = open on all four.
//! The access-control halves are left off here — the gate carries no
//! permissions document, and this test measures the two protection kinds.
//!
//! The assertions here are in-process. The wire-level proof is the tcpdump
//! run described in the module docs of the marker constants: capture UDP on
//! the loopback while this test runs and grep the pcap for the two markers.
//!
//! Multicast loopback on macOS is unreliable → Linux-only.

#![cfg(all(target_os = "linux", feature = "security"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::type_complexity
)]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, VendorId};
use zerodds_security::authentication::{
    AuthenticationPlugin, SharedSecretHandle, SharedSecretProvider,
};
use zerodds_security_crypto::{AesGcmCryptoPlugin, Suite};
use zerodds_security_pki::{IdentityConfig, PkiAuthenticationPlugin};
use zerodds_security_runtime::SharedSecurityGate;

#[path = "common/mod.rs"]
mod common;

/// Encrypted topic (`data_protection_kind=ENCRYPT`).
const TOPIC_PROTECTED: &str = "PD_RWA_EM_ED";
/// Unprotected topic (`data_protection_kind=NONE`).
const TOPIC_OPEN: &str = "OD_OA_OM_OD";

/// Payload written on [`TOPIC_PROTECTED`]. MUST NOT appear in a capture:
/// `tcpdump -i lo -w /tmp/cap.pcap 'udp'` while this test runs, then
/// `grep -c PDRWAEMED-MARKER /tmp/cap.pcap` = 0.
const MARKER_PROTECTED: &[u8] = b"PDRWAEMED-MARKER-ZEROCLEARTEXT";
/// Payload written on [`TOPIC_OPEN`]. MUST appear in the same capture —
/// otherwise the test proves nothing about the capture (a run in which
/// nothing at all was recorded would look identical).
const MARKER_OPEN: &[u8] = b"ODOAOMOD-MARKER-STAYSCLEARTEXT";

/// `OD_OA_OM_OD` FIRST in document order, so that a `topic_rules.first()`
/// lookup resolves to the unprotected rule for BOTH topics.
fn gov_two_topic_rules(domain: i32) -> String {
    format!(
        "<dds><domain_access_rules><domain_rule><domains><id>{domain}</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>\
         <topic_access_rules>\
         <topic_rule><topic_expression>{TOPIC_OPEN}</topic_expression>\
         <metadata_protection_kind>NONE</metadata_protection_kind>\
         <data_protection_kind>NONE</data_protection_kind>\
         </topic_rule>\
         <topic_rule><topic_expression>{TOPIC_PROTECTED}</topic_expression>\
         <metadata_protection_kind>NONE</metadata_protection_kind>\
         <data_protection_kind>ENCRYPT</data_protection_kind>\
         </topic_rule>\
         </topic_access_rules></domain_rule></domain_access_rules></dds>"
    )
}

struct PkiProvider(Arc<Mutex<PkiAuthenticationPlugin>>);
impl SharedSecretProvider for PkiProvider {
    fn get_shared_secret(&self, h: SharedSecretHandle) -> Option<Vec<u8>> {
        self.0.lock().ok()?.get_shared_secret(h)
    }
}

/// A participant GUID prefix bound to the identity in `cert_pem`
/// (DDS-Security 1.2 §10.3.3 Table 59) for the candidate `seed`: the
/// handshake refuses a `c.pdata` GUID that is not.
fn bound_prefix(cert_pem: &[u8], seed: u8) -> GuidPrefix {
    let der = zerodds_security_pki::first_cert_der(cert_pem).unwrap();
    GuidPrefix::from_bytes(
        zerodds_security_pki::adjust_participant_guid_prefix(&[seed; 16], &der).unwrap(),
    )
}

fn mint_ca_two() -> ((Vec<u8>, Vec<u8>), (Vec<u8>, Vec<u8>)) {
    use rcgen::{CertificateParams, KeyPair};
    let mut ca_p = CertificateParams::new(vec!["TopicRule CA".to_string()]).unwrap();
    ca_p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca_cert = ca_p.self_signed(&ca_key).unwrap();
    let ca_pem = ca_cert.pem().into_bytes();
    let mint = |name: &str| -> (Vec<u8>, Vec<u8>) {
        let mut p = CertificateParams::new(vec![name.to_string()]).unwrap();
        p.is_ca = rcgen::IsCa::NoCa;
        let k = KeyPair::generate().unwrap();
        let c = p.signed_by(&k, &ca_cert, &ca_key).unwrap();
        (
            [c.pem().into_bytes(), b"\n".to_vec(), ca_pem.clone()].concat(),
            k.serialize_pem().into_bytes(),
        )
    };
    (mint("alice-topicrule"), mint("bob-topicrule"))
}

fn start_secured(
    domain: i32,
    cert: Vec<u8>,
    key: Vec<u8>,
    guid: [u8; 16],
    prefix: GuidPrefix,
    gov: &str,
) -> (Arc<DcpsRuntime>, Arc<SharedSecurityGate>) {
    let pki = Arc::new(Mutex::new(PkiAuthenticationPlugin::new()));
    let local = pki
        .lock()
        .unwrap()
        .validate_with_config(
            IdentityConfig {
                identity_cert_pem: cert.clone(),
                identity_ca_pem: cert,
                identity_key_pem: Some(key),
            },
            guid,
        )
        .unwrap();
    let gate = Arc::new(SharedSecurityGate::new(
        u32::try_from(domain).expect("domain id is non-negative"),
        zerodds_security_permissions::parse_governance_xml(gov).unwrap(),
        Box::new(AesGcmCryptoPlugin::with_secret_provider(
            Suite::Aes128Gcm,
            Arc::new(PkiProvider(pki.clone())) as Arc<dyn SharedSecretProvider>,
        )),
    ));
    let rt = DcpsRuntime::start(
        domain,
        prefix,
        RuntimeConfig {
            security: Some(gate.clone()),
            security_guid_prefix: Some(prefix),
            ..RuntimeConfig::default()
        },
    )
    .expect("start");
    let auth: Arc<Mutex<dyn AuthenticationPlugin>> = pki.clone();
    rt.enable_security_builtins_with_auth(VendorId::ZERODDS, auth, local);
    (rt, gate)
}

fn writer_cfg(topic: &str) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        ..Default::default()
    }
}

fn reader_cfg(topic: &str) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        ..Default::default()
    }
}

/// Two topics, one governance, two rules — the protected topic must resolve
/// to `ENCRYPT` and the open one to `NONE`, on the SAME gate. This is the
/// assertion `topic_rules.first()` cannot satisfy: it returns the same rule
/// for both names.
#[serial_test::file_serial(security_live)]
#[test]
fn two_topic_rules_resolve_per_topic() {
    let domain = common::unique_domain();
    let ((a_cert, a_key), _) = mint_ca_two();
    let a_prefix = bound_prefix(&a_cert, 0x5E);
    let a_guid = Guid::new(a_prefix, EntityId::PARTICIPANT).to_bytes();
    let (rt_a, gate_a) = start_secured(
        domain,
        a_cert,
        a_key,
        a_guid,
        a_prefix,
        &gov_two_topic_rules(domain),
    );

    use zerodds_security_runtime::ProtectionLevel;
    assert_eq!(
        gate_a.data_protection(TOPIC_PROTECTED).unwrap(),
        ProtectionLevel::Encrypt,
        "{TOPIC_PROTECTED} carries data_protection_kind=ENCRYPT"
    );
    assert_eq!(
        gate_a.data_protection(TOPIC_OPEN).unwrap(),
        ProtectionLevel::None,
        "{TOPIC_OPEN} carries data_protection_kind=NONE — in the SAME governance"
    );

    // The same split has to hold for every other per-topic accessor the
    // SEDP `EndpointSecurityInfo` mask is built from — `user_endpoint_
    // security_info` calls all of them with the endpoint's own topic name.
    assert_eq!(
        gate_a.metadata_protection(TOPIC_PROTECTED).unwrap(),
        ProtectionLevel::None,
        "both rules leave metadata unprotected here — only the DATA kinds differ"
    );
    assert!(!gate_a.topic_discovery_protected(TOPIC_PROTECTED).unwrap());
    assert!(!gate_a.topic_read_protected(TOPIC_OPEN).unwrap());
    assert!(!gate_a.topic_write_protected(TOPIC_OPEN).unwrap());

    rt_a.shutdown();
}

/// Live over UDP: both topics round-trip between two secured participants,
/// with the protected payload encrypted and the open one in the clear.
/// The capture-side proof runs tcpdump around this test and greps the pcap
/// for [`MARKER_PROTECTED`] (0 hits) and [`MARKER_OPEN`] (> 0 hits).
#[serial_test::file_serial(security_live)]
#[test]
fn both_topics_round_trip_live_over_udp() {
    let domain = common::unique_domain();
    let ((a_cert, a_key), (b_cert, b_key)) = mint_ca_two();
    let a_prefix = bound_prefix(&a_cert, 0x6F);
    let b_prefix = bound_prefix(&b_cert, 0x7A);
    let a_guid = Guid::new(a_prefix, EntityId::PARTICIPANT).to_bytes();
    let b_guid = Guid::new(b_prefix, EntityId::PARTICIPANT).to_bytes();
    let gov = gov_two_topic_rules(domain);
    let (rt_a, gate_a) = start_secured(domain, a_cert, a_key, a_guid, a_prefix, &gov);
    let (rt_b, gate_b) = start_secured(domain, b_cert, b_key, b_guid, b_prefix, &gov);

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut shook = false;
    while Instant::now() < deadline {
        if gate_a
            .slot_for(&b_prefix.to_bytes())
            .ok()
            .flatten()
            .is_some()
            && gate_b
                .slot_for(&a_prefix.to_bytes())
                .ok()
                .flatten()
                .is_some()
        {
            shook = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(shook, "handshake did not finish");

    for (topic, marker) in [
        (TOPIC_PROTECTED, MARKER_PROTECTED),
        (TOPIC_OPEN, MARKER_OPEN),
    ] {
        let w = rt_a.register_user_writer(writer_cfg(topic)).unwrap();
        let (r, rx) = rt_b.register_user_reader(reader_cfg(topic)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut matched = false;
        while Instant::now() < deadline {
            if rt_b.user_reader_matched_count(r) >= 1 && rt_a.user_writer_matched_count(w) >= 1 {
                matched = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(matched, "SEDP match on {topic} did not come about");

        let mut got = None;
        for _ in 0..20 {
            rt_a.write_user_sample(w, marker.to_vec()).unwrap();
            if let Ok(s) = rx.recv_timeout(Duration::from_millis(500)) {
                got = Some(s);
                break;
            }
        }
        match got.unwrap_or_else(|| panic!("B must receive the sample on {topic}")) {
            UserSample::Alive { payload, .. } => assert_eq!(payload.as_slice(), marker),
            other => panic!(
                "expected Alive on {topic}, got {:?}",
                core::mem::discriminant(&other)
            ),
        }
    }

    rt_a.shutdown();
    rt_b.shutdown();
}
