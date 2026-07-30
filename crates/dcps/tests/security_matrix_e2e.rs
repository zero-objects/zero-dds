// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! zero<->zero Security-Matrix (Live UDP, Linux-only).
//!
//! Systematic closeout: NOT only the common selection, but every
//! crypto suite, every ProtectionKind per category, both auth methods and
//! both DataReps must round-trip zero<->zero. Each dimension is its
//! own `#[test]` that walks the whole axis and collects all results
//! (one assertion at the end with the complete error list).
//!
//! Two `DcpsRuntime` in the same process, one own domain per combo (full
//! isolation), find each other via SPDP, handshake + crypto-token exchange over
//! VolatileSecure, then a user-DATA roundtrip over the respective protection
//! Pfad. Multicast-Loopback auf macOS unzuverlaessig -> Linux-only (codepit).

#![cfg(all(target_os = "linux", feature = "security"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::uninlined_format_args,
    missing_docs
)]

use std::sync::atomic::{AtomicU32, Ordering};
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
use zerodds_security_pki::{IdentityConfig, PkiAuthenticationPlugin, PskAuthenticationPlugin};
use zerodds_security_runtime::SharedSecurityGate;

// One own domain per combo -> no SPDP multicast cross-contamination
// between sequential combos (despite #[serial] + shutdown).
static NEXT_DOMAIN: AtomicU32 = AtomicU32::new(40);
fn next_domain() -> u32 {
    NEXT_DOMAIN.fetch_add(1, Ordering::SeqCst)
}

/// ProtectionKind as a governance XML token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum K {
    None,
    Sign,
    Encrypt,
    SignOA,
    EncryptOA,
}
impl K {
    fn xml(self) -> &'static str {
        match self {
            K::None => "NONE",
            K::Sign => "SIGN",
            K::Encrypt => "ENCRYPT",
            K::SignOA => "SIGN_WITH_ORIGIN_AUTHENTICATION",
            K::EncryptOA => "ENCRYPT_WITH_ORIGIN_AUTHENTICATION",
        }
    }
}

/// Builds the governance XML. `rtps`=NONE is needed so SPDP bootstraps plaintext
/// (otherwise no discovery); the other categories are freely selectable.
fn gov_xml(disc: K, live: K, rtps: K, meta: K, data: K, domain: u32) -> String {
    let enable_disc = disc != K::None;
    let enable_live = live != K::None;
    format!(
        "<domain_access_rules><domain_rule><domains><id>{domain}</id></domains>\
         <allow_unauthenticated_participants>false</allow_unauthenticated_participants>\
         <enable_join_access_control>false</enable_join_access_control>\
         <discovery_protection_kind>{}</discovery_protection_kind>\
         <liveliness_protection_kind>{}</liveliness_protection_kind>\
         <rtps_protection_kind>{}</rtps_protection_kind>\
         <topic_access_rules><topic_rule><topic_expression>*</topic_expression>\
         <enable_discovery_protection>{}</enable_discovery_protection>\
         <enable_liveliness_protection>{}</enable_liveliness_protection>\
         <enable_read_access_control>false</enable_read_access_control>\
         <enable_write_access_control>false</enable_write_access_control>\
         <metadata_protection_kind>{}</metadata_protection_kind>\
         <data_protection_kind>{}</data_protection_kind>\
         </topic_rule></topic_access_rules></domain_rule></domain_access_rules>",
        disc.xml(),
        live.xml(),
        rtps.xml(),
        if enable_disc { "true" } else { "false" },
        if enable_live { "true" } else { "false" },
        meta.xml(),
        data.xml(),
    )
}

struct PkiProvider(Arc<Mutex<PkiAuthenticationPlugin>>);
impl SharedSecretProvider for PkiProvider {
    fn get_shared_secret(&self, h: SharedSecretHandle) -> Option<Vec<u8>> {
        self.0.lock().ok()?.get_shared_secret(h)
    }
    fn get_shared_secret_challenges(&self, h: SharedSecretHandle) -> Option<([u8; 32], [u8; 32])> {
        self.0.lock().ok()?.get_shared_secret_challenges(h)
    }
}

struct PskProvider(Arc<Mutex<PskAuthenticationPlugin>>);
impl SharedSecretProvider for PskProvider {
    fn get_shared_secret(&self, h: SharedSecretHandle) -> Option<Vec<u8>> {
        self.0.lock().ok()?.get_shared_secret(h)
    }
}

/// Shared CA + two leaves (cert||CA bundle).
fn mint_ca_two() -> ((Vec<u8>, Vec<u8>), (Vec<u8>, Vec<u8>)) {
    use rcgen::{CertificateParams, KeyPair};
    let mut ca_p = CertificateParams::new(vec!["Matrix CA".to_string()]).unwrap();
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
    (mint("alice-matrix"), mint("bob-matrix"))
}

fn gate_for(
    provider: Arc<dyn SharedSecretProvider>,
    gov: &str,
    suite: Suite,
) -> Arc<SharedSecurityGate> {
    Arc::new(SharedSecurityGate::new(
        gov_domain(gov),
        zerodds_security_permissions::parse_governance_xml(gov).unwrap(),
        Box::new(AesGcmCryptoPlugin::with_secret_provider(suite, provider)),
    ))
}

// Pull the domain id from the gov XML (we wrote it in).
fn gov_domain(gov: &str) -> u32 {
    let a = gov.find("<id>").map(|i| i + 4).unwrap_or(0);
    let b = gov[a..].find("</id>").map(|i| a + i).unwrap_or(a);
    gov[a..b].parse().unwrap_or(0)
}

#[derive(Clone, Copy)]
enum Auth {
    Pki,
    Psk,
}

fn start_secured(
    auth: Auth,
    cert: Vec<u8>,
    key: Vec<u8>,
    guid: [u8; 16],
    prefix: GuidPrefix,
    gov: &str,
    suite: Suite,
) -> (Arc<DcpsRuntime>, Arc<SharedSecurityGate>) {
    let domain = gov_domain(gov);
    let (gate, auth_plugin, local) = match auth {
        Auth::Pki => {
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
            let provider = Arc::new(PkiProvider(pki.clone())) as Arc<dyn SharedSecretProvider>;
            let gate = gate_for(provider, gov, suite);
            let auth_plugin: Arc<Mutex<dyn AuthenticationPlugin>> = pki;
            (gate, auth_plugin, local)
        }
        Auth::Psk => {
            // Both sides share the same PSK under the same identity.
            let psk = Arc::new(Mutex::new(PskAuthenticationPlugin::new()));
            let local = {
                let mut p = psk.lock().unwrap();
                p.register_psk("matrix".into(), vec![0x5Au8; 32]).unwrap();
                p.validate_local_psk_identity("matrix").unwrap()
            };
            let provider = Arc::new(PskProvider(psk.clone())) as Arc<dyn SharedSecretProvider>;
            let gate = gate_for(provider, gov, suite);
            let auth_plugin: Arc<Mutex<dyn AuthenticationPlugin>> = psk;
            (gate, auth_plugin, local)
        }
    };
    let _ = (guid,);
    let rt = DcpsRuntime::start(
        domain as i32,
        prefix,
        RuntimeConfig {
            security: Some(gate.clone()),
            // #11 §9.3.3 guard: security_guid_prefix must equal the prefix passed
            // to start(), else DcpsRuntime::start rejects the secured runtime.
            security_guid_prefix: Some(prefix),
            ..RuntimeConfig::default()
        },
    )
    .expect("start");
    rt.enable_security_builtins_with_auth(VendorId::ZERODDS, auth_plugin, local);
    (rt, gate)
}

fn wait_handshake(
    gate_a: &Arc<SharedSecurityGate>,
    gate_b: &Arc<SharedSecurityGate>,
    a_key: [u8; 12],
    b_key: [u8; 12],
    timeout: Duration,
) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if gate_a.slot_for(&b_key).ok().flatten().is_some()
            && gate_b.slot_for(&a_key).ok().flatten().is_some()
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

fn writer_cfg(topic: &str, repr: Option<i16>) -> UserWriterConfig {
    UserWriterConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        reliable: true,
        durability: zerodds_qos::DurabilityKind::Volatile,
        deadline: Default::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        lifespan: Default::default(),
        liveliness: Default::default(),
        ownership: zerodds_qos::OwnershipKind::Shared,
        ownership_strength: 0,
        presentation: Default::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        data_representation_offer: repr.map(|r| vec![r]),
    }
}

fn reader_cfg(topic: &str, repr: Option<i16>) -> UserReaderConfig {
    UserReaderConfig {
        topic_name: topic.into(),
        type_name: "RawBytes".into(),
        reliable: true,
        durability: zerodds_qos::DurabilityKind::Volatile,
        deadline: Default::default(),
        latency_budget: Default::default(),
        destination_order: Default::default(),
        liveliness: Default::default(),
        ownership: zerodds_qos::OwnershipKind::Shared,
        presentation: Default::default(),
        partition: vec![],
        user_data: vec![],
        topic_data: vec![],
        group_data: vec![],
        type_identifier: zerodds_types::TypeIdentifier::None,
        type_consistency: zerodds_types::qos::TypeConsistencyEnforcement::default(),
        data_representation_offer: repr.map(|r| vec![r]),
    }
}

/// One combo: two secured runtimes, handshake, SEDP match, user DATA-
/// Roundtrip. `Ok(())` = sample received correctly.
fn run_combo(auth: Auth, suite: Suite, gov: &str, repr: Option<i16>) -> Result<(), String> {
    let ((a_cert, a_key), (b_cert, b_key)) = mint_ca_two();
    let a_prefix = GuidPrefix::from_bytes([0x2A; 12]);
    let b_prefix = GuidPrefix::from_bytes([0x4B; 12]);
    let a_guid = Guid::new(a_prefix, EntityId::PARTICIPANT).to_bytes();
    let b_guid = Guid::new(b_prefix, EntityId::PARTICIPANT).to_bytes();
    let (rt_a, gate_a) = start_secured(auth, a_cert, a_key, a_guid, a_prefix, gov, suite);
    let (rt_b, gate_b) = start_secured(auth, b_cert, b_key, b_guid, b_prefix, gov, suite);

    let res = (|| -> Result<(), String> {
        if !wait_handshake(
            &gate_a,
            &gate_b,
            a_prefix.to_bytes(),
            b_prefix.to_bytes(),
            Duration::from_secs(30),
        ) {
            return Err("handshake/token-exchange timeout".into());
        }
        let writer_eid = rt_a
            .register_user_writer(writer_cfg("MatrixData", repr))
            .map_err(|e| format!("register_writer: {e:?}"))?;
        let (reader_eid, rx) = rt_b
            .register_user_reader(reader_cfg("MatrixData", repr))
            .map_err(|e| format!("register_reader: {e:?}"))?;
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut matched = false;
        while Instant::now() < deadline {
            if rt_b.user_reader_matched_count(reader_eid) >= 1
                && rt_a.user_writer_matched_count(writer_eid) >= 1
            {
                matched = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        if !matched {
            return Err("SEDP-match timeout".into());
        }
        let payload = b"matrix-secured-payload-0xC0FFEE".to_vec();
        for _ in 0..30 {
            rt_a.write_user_sample(writer_eid, payload.clone())
                .map_err(|e| format!("write: {e:?}"))?;
            if let Ok(s) = rx.recv_timeout(Duration::from_millis(400)) {
                return match s {
                    UserSample::Alive { payload: bytes, .. }
                        if bytes.as_slice() == payload.as_slice() =>
                    {
                        Ok(())
                    }
                    UserSample::Alive { .. } => Err("payload mismatch".into()),
                    other => Err(format!(
                        "non-alive sample {:?}",
                        std::mem::discriminant(&other)
                    )),
                };
            }
        }
        Err("no sample received".into())
    })();

    rt_a.shutdown();
    rt_b.shutdown();
    std::thread::sleep(Duration::from_millis(300)); // settle before the next combo
    res
}

// =====================================================================
// #32 — crypto-suite matrix: both AES suites (128/256) each with ENCRYPT
// (GCM transform) AND SIGN (GMAC transform; sign-only comes from the
// ProtectionKind, not from a separate suite enum).
// =====================================================================
#[serial_test::serial(security_live)]
#[test]
fn matrix_all_crypto_suites() {
    let cases = [
        ("Aes128Gcm/ENCRYPT", Suite::Aes128Gcm, K::Encrypt),
        ("Aes256Gcm/ENCRYPT", Suite::Aes256Gcm, K::Encrypt),
        ("Aes128Gcm/SIGN(GMAC)", Suite::Aes128Gcm, K::Sign),
        ("Aes256Gcm/SIGN(GMAC)", Suite::Aes256Gcm, K::Sign),
    ];
    let mut fails = Vec::new();
    for (name, suite, data_kind) in cases {
        let d = next_domain();
        let gov = gov_xml(K::None, K::None, K::None, K::None, data_kind, d);
        if let Err(e) = run_combo(Auth::Pki, suite, &gov, None) {
            fails.push(format!("suite {name}: {e}"));
        }
    }
    assert!(
        fails.is_empty(),
        "suite matrix error:\n  {}",
        fails.join("\n  ")
    );
}

// =====================================================================
// #33 — data_protection_kind: all 5 kinds (the GCM suite can do all).
// =====================================================================
#[serial_test::serial(security_live)]
#[test]
fn matrix_data_protection_all_kinds() {
    let mut fails = Vec::new();
    for kind in [K::None, K::Sign, K::Encrypt, K::SignOA, K::EncryptOA] {
        let d = next_domain();
        let gov = gov_xml(K::None, K::None, K::None, K::None, kind, d);
        if let Err(e) = run_combo(Auth::Pki, Suite::Aes128Gcm, &gov, None) {
            fails.push(format!("data_protection={:?}: {e}", kind));
        }
    }
    assert!(
        fails.is_empty(),
        "data_protection matrix error:\n  {}",
        fails.join("\n  ")
    );
}

// =====================================================================
// #33 — metadata_protection_kind: all 5 kinds (SEDP submessage protection).
// =====================================================================
#[serial_test::serial(security_live)]
#[test]
fn matrix_metadata_protection_all_kinds() {
    let mut fails = Vec::new();
    for kind in [K::None, K::Sign, K::Encrypt, K::SignOA, K::EncryptOA] {
        let d = next_domain();
        let gov = gov_xml(K::None, K::None, K::None, kind, K::Encrypt, d);
        if let Err(e) = run_combo(Auth::Pki, Suite::Aes128Gcm, &gov, None) {
            fails.push(format!("metadata_protection={:?}: {e}", kind));
        }
    }
    assert!(
        fails.is_empty(),
        "metadata_protection matrix error:\n  {}",
        fails.join("\n  ")
    );
}

// =====================================================================
// #33 — discovery_protection_kind: secure-SEDP (NONE + ENCRYPT).
// =====================================================================
#[serial_test::serial(security_live)]
#[test]
fn matrix_discovery_protection() {
    // discovery × data, each axis NONE/ENCRYPT — all four combinations.
    let mut fails = Vec::new();
    for disc in [K::None, K::Encrypt] {
        for data in [K::None, K::Encrypt] {
            let d = next_domain();
            let gov = gov_xml(disc, K::None, K::None, K::None, data, d);
            if let Err(e) = run_combo(Auth::Pki, Suite::Aes128Gcm, &gov, None) {
                fails.push(format!("discovery={:?} data={:?}: {e}", disc, data));
            }
        }
    }
    assert!(
        fails.is_empty(),
        "discovery_protection matrix error:\n  {}",
        fails.join("\n  ")
    );
}

/// Full secure profile (sros2 style): discovery + metadata + data all ENCRYPT
/// at the same time — the per-endpoint submessage path under secure SEDP. Plus the
/// edge combo data=ENCRYPT with metadata=NONE under discovery=ENCRYPT, which exercises the
/// 3-way dispatch (data_protection -> per-endpoint key instead of message-level
/// Participant-Fallback key_id=0) absichert.
#[serial_test::serial(security_live)]
#[test]
fn matrix_full_secure_profile() {
    let mut fails = Vec::new();
    let cases = [
        ("discovery+metadata+data=ENCRYPT", K::Encrypt, K::Encrypt),
        ("discovery+data=ENCRYPT, metadata=NONE", K::None, K::Encrypt),
    ];
    for (name, meta, data) in cases {
        let d = next_domain();
        let gov = gov_xml(K::Encrypt, K::None, K::None, meta, data, d);
        if let Err(e) = run_combo(Auth::Pki, Suite::Aes128Gcm, &gov, None) {
            fails.push(format!("{name}: {e}"));
        }
    }
    assert!(
        fails.is_empty(),
        "full-secure profile error:\n  {}",
        fails.join("\n  ")
    );
}

// =====================================================================
// #33 — liveliness_protection_kind.
// =====================================================================
#[serial_test::serial(security_live)]
#[test]
fn matrix_liveliness_protection() {
    let mut fails = Vec::new();
    for kind in [K::None, K::Sign, K::Encrypt] {
        let d = next_domain();
        let gov = gov_xml(K::None, kind, K::None, K::None, K::Encrypt, d);
        if let Err(e) = run_combo(Auth::Pki, Suite::Aes128Gcm, &gov, None) {
            fails.push(format!("liveliness_protection={:?}: {e}", kind));
        }
    }
    assert!(
        fails.is_empty(),
        "liveliness_protection matrix error:\n  {}",
        fails.join("\n  ")
    );
}

// =====================================================================
// #34 — Auth PKI + PSK (both under data=ENCRYPT).
// =====================================================================
#[serial_test::serial(security_live)]
#[test]
fn matrix_auth_methods() {
    let mut fails = Vec::new();
    for (name, auth) in [("PKI", Auth::Pki), ("PSK", Auth::Psk)] {
        let d = next_domain();
        let gov = gov_xml(K::None, K::None, K::None, K::None, K::Encrypt, d);
        if let Err(e) = run_combo(auth, Suite::Aes128Gcm, &gov, None) {
            fails.push(format!("auth {name}: {e}"));
        }
    }
    assert!(
        fails.is_empty(),
        "Auth matrix error:\n  {}",
        fails.join("\n  ")
    );
}

// =====================================================================
// #34 — DataRep XCDR1 + XCDR2 (under data=ENCRYPT).
// =====================================================================
#[serial_test::serial(security_live)]
#[test]
fn matrix_data_representation() {
    let mut fails = Vec::new();
    for (name, repr) in [("XCDR1", 0i16), ("XCDR2", 2i16)] {
        let d = next_domain();
        let gov = gov_xml(K::None, K::None, K::None, K::None, K::Encrypt, d);
        if let Err(e) = run_combo(Auth::Pki, Suite::Aes128Gcm, &gov, Some(repr)) {
            fails.push(format!("data_rep {name}: {e}"));
        }
    }
    assert!(
        fails.is_empty(),
        "DataRep matrix error:\n  {}",
        fails.join("\n  ")
    );
}
