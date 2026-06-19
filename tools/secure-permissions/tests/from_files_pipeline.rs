// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! End-to-end proof of the dds4iobroker #2 -> #3 pipeline:
//!
//!   1. (#2) `zerodds-secure-permissions` signs governance + permissions XML
//!      with a Permissions CA.
//!   2. (#3) `SecurityProfile::from_files` — the per-adapter secured-participant
//!      entry — loads an adapter's identity certificate together with those
//!      signed documents and builds a ready security gate.
//!
//! This shows that two adapters with *distinct* identities each produce a valid,
//! distinct security profile, exactly as the per-adapter-identity guide
//! describes. Fully offline (no network / no handshake) — the live multi-identity
//! handshake is covered by `crates/dcps/tests/security_live_e2e.rs`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use rcgen::{CertificateParams, IsCa, KeyPair};
use zerodds_secure_permissions::sign_xml_opaque;
use zerodds_security_runtime::{SecurityProfile, SecurityProfileConfig};

const GOVERNANCE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<domain_access_rules><domain_rule><domains><id>0</id></domains>
<topic_access_rules><topic_rule><topic_expression>*</topic_expression></topic_rule></topic_access_rules>
</domain_rule></domain_access_rules>"#;

fn permissions_xml(common_name: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<dds><permissions><grant name="{common_name}">
  <subject_name>CN={common_name}</subject_name>
  <allow_rule><publish><topic>State</topic></publish></allow_rule>
  <default>DENY</default>
</grant></permissions></dds>"#
    )
}

/// A unique temp directory for one test (process-id + monotonic counter).
fn unique_dir(tag: &str) -> PathBuf {
    static N: AtomicU32 = AtomicU32::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("zerodds_secperm_{tag}_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// rcgen P-256 self-signed CA -> (ca_cert, ca_key).
fn mint_ca(name: &str) -> (rcgen::Certificate, KeyPair) {
    let mut params = CertificateParams::new(vec![name.to_string()]).unwrap();
    params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let key = KeyPair::generate().unwrap();
    let cert = params.self_signed(&key).unwrap();
    (cert, key)
}

/// An EE identity leaf under `ca` with subject `CN=<name>`.
fn mint_leaf(name: &str, ca: &rcgen::Certificate, ca_key: &KeyPair) -> (String, String) {
    let mut params = CertificateParams::new(vec![name.to_string()]).unwrap();
    params.is_ca = IsCa::NoCa;
    let key = KeyPair::generate().unwrap();
    let cert = params.signed_by(&key, ca, ca_key).unwrap();
    (cert.pem(), key.serialize_pem())
}

/// Build a full per-adapter `SecurityProfileConfig` on disk and return it.
/// Uses our own signer (#2) for governance + permissions.
fn build_adapter(
    dir: &std::path::Path,
    identity_ca_cert_pem: &str,
    perm_ca_cert_pem: &str,
    perm_ca_key_pem: &str,
    adapter_cn: &str,
    identity_ca: &rcgen::Certificate,
    identity_ca_key: &KeyPair,
) -> SecurityProfileConfig {
    let (leaf_cert_pem, leaf_key_pem) = mint_leaf(adapter_cn, identity_ca, identity_ca_key);

    // governance is shared; permissions are per-adapter (its own grant).
    let gov_p7s = sign_xml_opaque(GOVERNANCE_XML.as_bytes(), perm_ca_cert_pem, perm_ca_key_pem)
        .expect("sign governance");
    let perms_xml = permissions_xml(adapter_cn);
    let perms_p7s = sign_xml_opaque(perms_xml.as_bytes(), perm_ca_cert_pem, perm_ca_key_pem)
        .expect("sign permissions");

    let w = |name: &str, bytes: &[u8]| -> PathBuf {
        let p = dir.join(format!("{adapter_cn}_{name}"));
        std::fs::write(&p, bytes).unwrap();
        p
    };

    SecurityProfileConfig {
        domain_id: 0,
        identity_ca_pem: w("identity_ca.pem", identity_ca_cert_pem.as_bytes()),
        identity_cert_pem: w("identity.pem", leaf_cert_pem.as_bytes()),
        identity_key_pem: w("identity_key.pem", leaf_key_pem.as_bytes()),
        permissions_ca_pem: w("permissions_ca.pem", perm_ca_cert_pem.as_bytes()),
        governance_p7s: w("governance.p7s", &gov_p7s),
        permissions_p7s: w("permissions.p7s", &perms_p7s),
    }
}

#[test]
fn two_adapters_get_distinct_secured_profiles() {
    let dir = unique_dir("pipeline");

    let (identity_ca, identity_ca_key) = mint_ca("dds4iobroker Identity CA");
    let identity_ca_pem = identity_ca.pem();

    // Permissions CA via rcgen too; its PKCS#8 key + cert drive our signer.
    let (perm_ca, perm_ca_key) = mint_ca("dds4iobroker Permissions CA");
    let perm_ca_pem = perm_ca.pem();
    let perm_ca_key_pem = perm_ca_key.serialize_pem();

    // A distinct seed GUID per adapter (the broker would derive this from the
    // adapter id); from_files replaces the prefix with the identity-bound one.
    let seed_a = [0xA1u8; 16];
    let seed_b = [0xB2u8; 16];

    let cfg_a = build_adapter(
        &dir,
        &identity_ca_pem,
        &perm_ca_pem,
        &perm_ca_key_pem,
        "adapter1",
        &identity_ca,
        &identity_ca_key,
    );
    let cfg_b = build_adapter(
        &dir,
        &identity_ca_pem,
        &perm_ca_pem,
        &perm_ca_key_pem,
        "adapter2",
        &identity_ca,
        &identity_ca_key,
    );

    let profile_a = SecurityProfile::from_files(&cfg_a, seed_a)
        .expect("adapter1 profile builds from our signed docs");
    let profile_b = SecurityProfile::from_files(&cfg_b, seed_b)
        .expect("adapter2 profile builds from our signed docs");

    // §9.3.3: the GUID prefix is rebound to the identity (so it is NOT the raw
    // seed), and the two adapters end up with DISTINCT participant GUIDs.
    assert_ne!(
        profile_a.adjusted_participant_guid, seed_a,
        "GUID prefix must be identity-bound, not the raw seed"
    );
    assert_ne!(
        profile_a.adjusted_participant_guid, profile_b.adjusted_participant_guid,
        "two distinct identities must yield two distinct participant GUIDs"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn from_files_rejects_permissions_signed_by_wrong_ca() {
    let dir = unique_dir("wrongca");

    let (identity_ca, identity_ca_key) = mint_ca("Identity CA");
    let identity_ca_pem = identity_ca.pem();

    // The real Permissions CA the participant trusts...
    let (perm_ca, _perm_ca_key) = mint_ca("Permissions CA");
    let perm_ca_pem = perm_ca.pem();
    // ...but the documents are signed by a DIFFERENT (rogue) CA.
    let (rogue_ca, rogue_ca_key) = mint_ca("Rogue CA");
    let rogue_ca_key_pem = rogue_ca_key.serialize_pem();
    let rogue_ca_pem = rogue_ca.pem();

    let cfg = build_adapter(
        &dir,
        &identity_ca_pem,
        &rogue_ca_pem,
        &rogue_ca_key_pem,
        "adapter1",
        &identity_ca,
        &identity_ca_key,
    );
    // Overwrite the permissions-CA trust anchor with the REAL one — now the
    // rogue-signed docs must fail to verify.
    std::fs::write(&cfg.permissions_ca_pem, perm_ca_pem.as_bytes()).unwrap();

    let res = SecurityProfile::from_files(&cfg, [0xC3u8; 16]);
    assert!(
        res.is_err(),
        "permissions signed by an untrusted CA must be rejected"
    );

    std::fs::remove_dir_all(&dir).ok();
}
