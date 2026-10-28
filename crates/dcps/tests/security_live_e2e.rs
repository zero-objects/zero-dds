// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! FU2 S2 — live verification of the secured handshake over real
//! UDP multicast. Two `DcpsRuntime` in the same process (same domain)
//! with auth plugin + crypto gate find each other via SPDP, perform the
//! PKI handshake and automatically exchange their crypto tokens over
//! the Kx-protected VolatileSecure channel. Success = BOTH gates
//! have registered the respective other peer (`slot_for` = Some).
//!
//! Multicast loopback on macOS is unreliable → Linux-only
//! (verified on the Linux test host).

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

// rtps_protection_kind NOT set (= NONE): SPDP discovery bootstraps in
// plaintext (otherwise classify_inbound discards the beacons on the protected
// domain → no discovery → no handshake). The crypto token exchanged over
// VolatileSecure is payload-level Kx-encrypted (no leak),
// independent of rtps_protection_kind.
fn gov(domain: i32) -> String {
    format!(
        "<dds><domain_access_rules><domain_rule><domains><id>{domain}</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>\
         <topic_access_rules><topic_rule><topic_expression>*</topic_expression>\
         </topic_rule></topic_access_rules></domain_rule></domain_access_rules></dds>"
    )
}

// Like `gov`, but with data_protection_kind=ENCRYPT (user data encrypted,
// SPDP/SEDP stays plaintext since rtps_protection_kind=NONE).
fn gov_data_encrypt(domain: i32) -> String {
    format!(
        "<dds><domain_access_rules><domain_rule><domains><id>{domain}</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>\
         <topic_access_rules><topic_rule><topic_expression>*</topic_expression>\
         <data_protection_kind>ENCRYPT</data_protection_kind>\
         </topic_rule></topic_access_rules></domain_rule></domain_access_rules></dds>"
    )
}

struct PkiProvider(Arc<Mutex<PkiAuthenticationPlugin>>);
impl SharedSecretProvider for PkiProvider {
    fn get_shared_secret(&self, h: SharedSecretHandle) -> Option<Vec<u8>> {
        self.0.lock().ok()?.get_shared_secret(h)
    }
}

/// A SEPARATE CA + one leaf under it (cert||own-CA bundle) — a
/// peer that the others do NOT trust (negative control).
fn mint_foreign() -> (Vec<u8>, Vec<u8>) {
    use rcgen::{CertificateParams, KeyPair};
    let mut ca_p = CertificateParams::new(vec!["Foreign CA".to_string()]).unwrap();
    ca_p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca_cert = ca_p.self_signed(&ca_key).unwrap();
    let mut p = CertificateParams::new(vec!["eve-foreign".to_string()]).unwrap();
    p.is_ca = rcgen::IsCa::NoCa;
    let k = KeyPair::generate().unwrap();
    let c = p.signed_by(&k, &ca_cert, &ca_key).unwrap();
    (
        [
            c.pem().into_bytes(),
            b"\n".to_vec(),
            ca_cert.pem().into_bytes(),
        ]
        .concat(),
        k.serialize_pem().into_bytes(),
    )
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

/// Shared CA + two leaf identities (each cert||CA as a bundle).
fn mint_ca_two() -> ((Vec<u8>, Vec<u8>), (Vec<u8>, Vec<u8>)) {
    use rcgen::{CertificateParams, KeyPair};
    let mut ca_p = CertificateParams::new(vec!["FU2 Live CA".to_string()]).unwrap();
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
    (mint("alice-live"), mint("bob-live"))
}

fn gate_for(
    domain: i32,
    pki: &Arc<Mutex<PkiAuthenticationPlugin>>,
    gov: &str,
) -> Arc<SharedSecurityGate> {
    // The gate MUST run on the same domain the governance is written for.
    // `unique_domain()` leases a domain per test run, so a hardcoded 0 makes
    // `find_domain_rule` miss, `data_protection` read None, and every
    // assertion about encrypted user DATA pass on plaintext.
    Arc::new(SharedSecurityGate::new(
        u32::try_from(domain).expect("domain id is non-negative"),
        zerodds_security_permissions::parse_governance_xml(gov).unwrap(),
        Box::new(AesGcmCryptoPlugin::with_secret_provider(
            Suite::Aes128Gcm,
            Arc::new(PkiProvider(pki.clone())) as Arc<dyn SharedSecretProvider>,
        )),
    ))
}

/// Starts a secured runtime (auth + gate sharing pki) and returns
/// (runtime, gate). Does NOT wait for the handshake.
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
    let gate = gate_for(domain, &pki, gov);
    let rt = DcpsRuntime::start(
        domain,
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
    let auth: Arc<Mutex<dyn AuthenticationPlugin>> = pki.clone();
    rt.enable_security_builtins_with_auth(VendorId::ZERODDS, auth, local);
    (rt, gate)
}

/// Waits until both gates have registered the respective other peer
/// (handshake + token exchange over UDP complete).
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

#[serial_test::file_serial(security_live)]
#[test]
fn secured_handshake_completes_live_over_udp() {
    let domain = common::unique_domain();
    let ((a_cert, a_key), (b_cert, b_key)) = mint_ca_two();
    let a_prefix = bound_prefix(&a_cert, 0x2A);
    let b_prefix = bound_prefix(&b_cert, 0x1B);
    let a_guid = Guid::new(a_prefix, EntityId::PARTICIPANT).to_bytes();
    let b_guid = Guid::new(b_prefix, EntityId::PARTICIPANT).to_bytes();

    let (rt_a, gate_a) = start_secured(domain, a_cert, a_key, a_guid, a_prefix, &gov(domain));
    let (rt_b, gate_b) = start_secured(domain, b_cert, b_key, b_guid, b_prefix, &gov(domain));
    assert!(
        wait_handshake(
            &gate_a,
            &gate_b,
            a_prefix.to_bytes(),
            b_prefix.to_bytes(),
            Duration::from_secs(30),
        ),
        "live handshake: both gates must register the peer (a_disc={})",
        rt_a.discovered_participants().len(),
    );
    rt_a.shutdown();
    rt_b.shutdown();
}

/// FU2 S3 — live secured **user DATA** over UDP: two secured runtimes
/// (data_protection_kind=ENCRYPT) perform the handshake, match
/// writer↔reader via SEDP, and a user sample round-trips encrypted
/// (force-encrypt from the governance → SRTPS on the wire; B only reads because
/// it decrypts with A's token).
#[serial_test::file_serial(security_live)]
#[test]
fn secured_user_data_round_trips_live_over_udp() {
    let domain = common::unique_domain();
    let ((a_cert, a_key), (b_cert, b_key)) = mint_ca_two();
    let a_prefix = bound_prefix(&a_cert, 0x3C);
    let b_prefix = bound_prefix(&b_cert, 0x4D);
    let a_guid = Guid::new(a_prefix, EntityId::PARTICIPANT).to_bytes();
    let b_guid = Guid::new(b_prefix, EntityId::PARTICIPANT).to_bytes();
    let (rt_a, gate_a) = start_secured(
        domain,
        a_cert,
        a_key,
        a_guid,
        a_prefix,
        &gov_data_encrypt(domain),
    );
    let (rt_b, gate_b) = start_secured(
        domain,
        b_cert,
        b_key,
        b_guid,
        b_prefix,
        &gov_data_encrypt(domain),
    );

    // 1. Handshake + token exchange.
    assert!(
        wait_handshake(
            &gate_a,
            &gate_b,
            a_prefix.to_bytes(),
            b_prefix.to_bytes(),
            Duration::from_secs(30),
        ),
        "secured user-data: handshake not finished"
    );

    // 2. Writer (A) + reader (B), wait for the SEDP match (on both sides).
    let writer_eid = rt_a
        .register_user_writer(writer_cfg("SecuredData"))
        .unwrap();
    let (reader_eid, rx) = rt_b
        .register_user_reader(reader_cfg("SecuredData"))
        .unwrap();
    let mut matched = false;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if rt_b.user_reader_matched_count(reader_eid) >= 1
            && rt_a.user_writer_matched_count(writer_eid) >= 1
        {
            matched = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(matched, "SEDP match (secured) did not come about");

    // 3. A writes → B reads over the data_protection=ENCRYPT path.
    let payload = b"secured-user-payload-0xC0FFEE".to_vec();
    let mut got = None;
    for _ in 0..20 {
        rt_a.write_user_sample(writer_eid, payload.clone()).unwrap();
        if let Ok(s) = rx.recv_timeout(Duration::from_millis(500)) {
            got = Some(s);
            break;
        }
    }
    match got.expect("B must receive the secured sample") {
        UserSample::Alive { payload: bytes, .. } => {
            assert_eq!(bytes.as_slice(), payload.as_slice())
        }
        other => panic!("expected Alive, got {:?}", std::mem::discriminant(&other)),
    }

    rt_a.shutdown();
    rt_b.shutdown();
}

/// FU2 S3 — negative control: A writes secured DATA; B (same CA,
/// authenticated) decrypts + reads; C (FOREIGN CA, handshake with A fails)
/// is not matched at all and reads NOTHING. DDS-Security 1.2 §9.8.3.2: with
/// `allow_unauthenticated_participants` FALSE a participant that fails the
/// authentication "shall be rejected". C was matched by SEDP before any
/// authentication, and the test relied on it to show that C cannot decrypt.
#[serial_test::file_serial(security_live)]
#[test]
fn secured_user_data_unreadable_by_unauthenticated_peer_live() {
    let domain = common::unique_domain();
    let ((a_cert, a_key), (b_cert, b_key)) = mint_ca_two();
    let (c_cert, c_key) = mint_foreign();
    let a_prefix = bound_prefix(&a_cert, 0x5E);
    let b_prefix = bound_prefix(&b_cert, 0x6F);
    let c_prefix = bound_prefix(&c_cert, 0x7A);
    let a_guid = Guid::new(a_prefix, EntityId::PARTICIPANT).to_bytes();
    let b_guid = Guid::new(b_prefix, EntityId::PARTICIPANT).to_bytes();
    let c_guid = Guid::new(c_prefix, EntityId::PARTICIPANT).to_bytes();
    let (rt_a, gate_a) = start_secured(
        domain,
        a_cert,
        a_key,
        a_guid,
        a_prefix,
        &gov_data_encrypt(domain),
    );
    let (rt_b, gate_b) = start_secured(
        domain,
        b_cert,
        b_key,
        b_guid,
        b_prefix,
        &gov_data_encrypt(domain),
    );
    let (rt_c, _gate_c) = start_secured(
        domain,
        c_cert,
        c_key,
        c_guid,
        c_prefix,
        &gov_data_encrypt(domain),
    );

    // A<->B authenticate (same CA). A<->C fails (foreign CA).
    assert!(
        wait_handshake(
            &gate_a,
            &gate_b,
            a_prefix.to_bytes(),
            b_prefix.to_bytes(),
            Duration::from_secs(30),
        ),
        "A<->B handshake must finish"
    );

    let writer_eid = rt_a.register_user_writer(writer_cfg("SecuredNeg")).unwrap();
    let (_b_reader, b_rx) = rt_b.register_user_reader(reader_cfg("SecuredNeg")).unwrap();
    let (_c_reader, c_rx) = rt_c.register_user_reader(reader_cfg("SecuredNeg")).unwrap();
    // A matches B, the authenticated one, and not C.
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline && rt_a.user_writer_matched_count(writer_eid) < 1 {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        rt_a.user_writer_matched_count(writer_eid),
        1,
        "A matches B; C, which did not authenticate, is rejected"
    );

    let payload = b"only-authenticated-may-read".to_vec();
    let mut b_got = false;
    for _ in 0..20 {
        rt_a.write_user_sample(writer_eid, payload.clone()).unwrap();
        if b_rx.recv_timeout(Duration::from_millis(400)).is_ok() {
            b_got = true;
            break;
        }
    }
    assert!(b_got, "B (authenticated) must decrypt + read the sample");
    assert_eq!(
        rt_a.user_writer_matched_count(writer_eid),
        1,
        "C is still not matched"
    );
    // C (not authenticated) must read NOTHING.
    let c_got = c_rx.recv_timeout(Duration::from_secs(2)).is_ok();
    assert!(
        !c_got,
        "C (foreign CA) must NOT read the secured sample — proof that user DATA is encrypted. \
         Diagnostics: gov_data={:?}, a_registered_c={} (should be false — otherwise the A<->C handshake succeeded despite the foreign CA)",
        gate_a.data_protection("SecuredNeg"),
        gate_a
            .slot_for(&c_prefix.to_bytes())
            .ok()
            .flatten()
            .is_some(),
    );

    rt_a.shutdown();
    rt_b.shutdown();
    rt_c.shutdown();
}

/// Leases a DDS domain no other process on this host holds — the
/// domain number is the UDP port number, so it is a host-wide resource.
#[path = "common/mod.rs"]
mod common;
