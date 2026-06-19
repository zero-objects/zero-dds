// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! End-to-end test for the DDS-Security 1.2 C-FFI:
//!
//! Generates via the `openssl` CLI an ECDSA-P256 test CA + identity cert +
//! key + CMS-signed governance/permissions, analogous to
//! `tests/perf/dds-roundtrip-bench/security/gen.sh`, then calls the FFI:
//!
//!   1. `zerodds_security_config_create()`
//!   2. all 6 setters with the freshly generated paths
//!   3. `zerodds_runtime_create_secure(0, cfg)`
//!
//! Expectation: a non-NULL runtime pointer + a clean `zerodds_runtime_destroy`.
//!
//! If `openssl` is not in `PATH` (e.g. a minimal container), the
//! test is skipped with `Ok` and a marker println is emitted — we
//! do not disguise that we skipped.

#![cfg(feature = "security")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout
)]

use std::ffi::CString;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use zerodds::security_ffi::*;
use zerodds::{ZeroDdsRuntime, zerodds_runtime_destroy};

// Keep openssl calls serial — avoids a race on the shared
// `.srl` counter file and sub-process startup storms on macOS.
static OPENSSL_LOCK: Mutex<()> = Mutex::new(());

fn openssl_available() -> bool {
    Command::new("openssl")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn must_run(label: &str, mut cmd: Command) {
    let out = cmd
        .output()
        .unwrap_or_else(|e| panic!("openssl {label}: spawn failed: {e}"));
    if !out.status.success() {
        panic!(
            "openssl {label}: exit={:?}\nstdout: {}\nstderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

// `ca_self_sign`: true → governance/permissions are signed directly with the
// CA cert (variant A, the Cyclone/FastDDS/OMG real-world pattern
// — tests the self-trust-anchor path in the CmsPkcs7Verifier). false →
// signed with the EE cert "alice" (variant B, RFC-5280 chain path).
fn gen_test_fixtures(dir: &Path, ca_self_sign: bool) {
    let _g = OPENSSL_LOCK.lock().unwrap();
    let certs = dir.join("certs");
    std::fs::create_dir_all(&certs).unwrap();

    // 1. Identity-CA (ECDSA P-256).
    let ca_key = certs.join("identity_ca_key.pem");
    let ca_cert = certs.join("identity_ca.pem");
    let mut c = Command::new("openssl");
    c.args([
        "ecparam",
        "-name",
        "prime256v1",
        "-genkey",
        "-noout",
        "-out",
    ])
    .arg(&ca_key);
    must_run("ecparam ca-key", c);

    let mut c = Command::new("openssl");
    c.args(["req", "-x509", "-new", "-nodes", "-key"])
        .arg(&ca_key)
        .args(["-days", "30", "-subj", "/CN=ZeroDDS Test FFI CA", "-out"])
        .arg(&ca_cert);
    must_run("ca self-sign", c);

    // 2. Identity cert "alice" (CA-signed). The key is generated directly in the
    //    PKCS#8-PEM format via `openssl genpkey`, because the ZeroDDS PKI
    //    plugin (and, by the way, also the DDS-Security-1.2 spec model
    //    OpenSSL EVP) expects PKCS#8, not traditional EC.
    let alice_key = certs.join("alice_key.pem");
    let alice_csr = certs.join("alice.csr");
    let alice_cert = certs.join("alice_cert.pem");
    let mut c = Command::new("openssl");
    c.args([
        "genpkey",
        "-algorithm",
        "EC",
        "-pkeyopt",
        "ec_paramgen_curve:P-256",
        "-out",
    ])
    .arg(&alice_key);
    must_run("genpkey alice-key", c);

    let mut c = Command::new("openssl");
    c.args(["req", "-new", "-key"])
        .arg(&alice_key)
        .args([
            "-subj",
            "/CN=zerodds-bench-alice/emailAddress=alice@bench.local",
            "-out",
        ])
        .arg(&alice_csr);
    must_run("alice csr", c);

    let mut c = Command::new("openssl");
    c.args(["x509", "-req", "-in"])
        .arg(&alice_csr)
        .args(["-CA"])
        .arg(&ca_cert)
        .args(["-CAkey"])
        .arg(&ca_key)
        .args(["-CAcreateserial", "-days", "30", "-out"])
        .arg(&alice_cert);
    must_run("alice ca-sign", c);

    // 3. Governance + Permissions XML.
    std::fs::write(
        dir.join("governance.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-SECURITY/20170801/omg_shared_ca_governance">
  <domain_access_rules>
    <domain_rule>
      <domains><id>0</id></domains>
      <allow_unauthenticated_participants>false</allow_unauthenticated_participants>
      <enable_join_access_control>true</enable_join_access_control>
      <discovery_protection_kind>ENCRYPT</discovery_protection_kind>
      <liveliness_protection_kind>ENCRYPT</liveliness_protection_kind>
      <rtps_protection_kind>ENCRYPT</rtps_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>*</topic_expression>
          <enable_discovery_protection>true</enable_discovery_protection>
          <enable_liveliness_protection>true</enable_liveliness_protection>
          <!-- read/write-AC=false => the domain is JOINABLE per DDS-Security
               table 63; otherwise zerodds_runtime_create_secure correctly
               rejects the fully-locked-down (join-AC + all topics read+write-AC)
               governance — exactly like OpenDDS-self (check_create_participant
               "No governance exists for this domain"). The encryption
               (discovery/liveliness/data/metadata = ENCRYPT) stays fully active;
               this test checks the create_secure happy path. -->
          <enable_read_access_control>false</enable_read_access_control>
          <enable_write_access_control>false</enable_write_access_control>
          <metadata_protection_kind>ENCRYPT</metadata_protection_kind>
          <data_protection_kind>ENCRYPT</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
"#,
    )
    .unwrap();

    std::fs::write(
        dir.join("permissions.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-SECURITY/20170801/omg_shared_ca_permissions">
  <permissions>
    <grant>
      <subject_name>CN=zerodds-bench-alice,emailAddress=alice@bench.local</subject_name>
      <validity>
        <not_before>2025-01-01T00:00:00</not_before>
        <not_after>2099-01-01T00:00:00</not_after>
      </validity>
      <allow_rule>
        <domains><id>0</id></domains>
        <publish><topics><topic>*</topic></topics></publish>
        <subscribe><topics><topic>*</topic></topics></subscribe>
      </allow_rule>
      <default>DENY</default>
    </grant>
  </permissions>
</dds>
"#,
    )
    .unwrap();

    // 4. CMS-Sign governance + permissions.
    //    - ca_self_sign=true: signer = CA cert (variant A). This is the
    //      Cyclone/FastDDS/OpenDDS/OMG real-world pattern — the
    //      CmsPkcs7Verifier must recognize the signer as a trust anchor
    //      and skip EE chain validation.
    //    - ca_self_sign=false: signer = EE cert "alice" (variant B,
    //      RFC-5280 chain). webpki validates the chain alice → CA.
    let (signer, signer_key) = if ca_self_sign {
        (&ca_cert, &ca_key)
    } else {
        (&alice_cert, &alice_key)
    };
    for name in ["governance", "permissions"] {
        let xml = dir.join(format!("{name}.xml"));
        let p7s = dir.join(format!("{name}.p7s"));
        let mut c = Command::new("openssl");
        c.args(["smime", "-sign", "-in"])
            .arg(&xml)
            .args(["-text", "-out"])
            .arg(&p7s)
            .args(["-signer"])
            .arg(signer)
            .args(["-inkey"])
            .arg(signer_key);
        must_run(&format!("cms-sign {name}"), c);
    }
}

/// Variant B: EE-signed permissions (RFC-5280-Chain alice → CA).
#[test]
fn ffi_runtime_create_secure_ee_signed() {
    run_e2e(false);
}

/// Variant A: CA-self-signed permissions (Cyclone/FastDDS/OMG pattern).
/// Validiert den self-trust-anchor-Pfad im CmsPkcs7Verifier.
#[test]
fn ffi_runtime_create_secure_ca_self_signed() {
    run_e2e(true);
}

fn run_e2e(ca_self_sign: bool) {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — security FFI e2e cannot run");
        return;
    }

    let tmp = tempdir_unique("zerodds_sec_ffi_");
    gen_test_fixtures(&tmp, ca_self_sign);

    let certs = tmp.join("certs");
    let cfg = zerodds_security_config_create();
    assert!(!cfg.is_null());

    let to_c = |p: std::path::PathBuf| CString::new(p.to_string_lossy().as_ref()).unwrap();
    let identity_ca = to_c(certs.join("identity_ca.pem"));
    let identity_cert = to_c(certs.join("alice_cert.pem"));
    let identity_key = to_c(certs.join("alice_key.pem"));
    let permissions_ca = to_c(certs.join("identity_ca.pem")); // = identity_ca im Test
    let governance = to_c(tmp.join("governance.p7s"));
    let permissions = to_c(tmp.join("permissions.p7s"));

    // SAFETY: cfg comes from zerodds_security_config_create and is non-null;
    // all *.as_ptr() point to live CStrings (held until the end of the function).
    unsafe {
        assert_eq!(
            zerodds_security_set_identity_ca_path(cfg, identity_ca.as_ptr()),
            0
        );
        assert_eq!(
            zerodds_security_set_identity_cert_path(cfg, identity_cert.as_ptr()),
            0
        );
        assert_eq!(
            zerodds_security_set_private_key_path(cfg, identity_key.as_ptr()),
            0
        );
        assert_eq!(
            zerodds_security_set_permissions_ca_path(cfg, permissions_ca.as_ptr()),
            0
        );
        assert_eq!(
            zerodds_security_set_governance_path(cfg, governance.as_ptr()),
            0
        );
        assert_eq!(
            zerodds_security_set_permissions_path(cfg, permissions.as_ptr()),
            0
        );
    }

    // Domain 0 matches the `<domain_rule>` in governance.xml.
    // SAFETY: cfg is a valid builder pointer; create_secure does not consume cfg.
    let rt: *mut ZeroDdsRuntime = unsafe { zerodds_runtime_create_secure(0, cfg) };
    assert!(
        !rt.is_null(),
        "zerodds_runtime_create_secure must return a runtime"
    );

    // SAFETY: rt comes from create_secure, cfg from config_create — both not yet
    // freed; each pointer is destroyed exactly once.
    unsafe {
        zerodds_runtime_destroy(rt);
        zerodds_security_config_destroy(cfg);
    }
    // The tmpdir is deliberately not auto-cleaned — manual inspection on failure.
    // On success Cargo deletes `target/` regularly.
}

fn tempdir_unique(prefix: &str) -> std::path::PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let p = std::env::temp_dir().join(format!("{prefix}{now}_{}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}
