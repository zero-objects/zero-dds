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

use zerodds::security_ffi::*;
use zerodds::{ZeroDdsRuntime, zerodds_runtime_destroy};

// The identity CA and the leaf are minted by the shared helper: the
// recipe carries the X.509-v3 rule, and a rule in three test files is a
// rule that gets corrected in two of them. `#[path]` across the crate
// boundary follows this tree's own convention for a shared test module
// (`crates/dcps/tests/*.rs` uses it for `common/cross_vendor.rs`).
#[path = "../../dcps/tests/common/pki.rs"]
mod pki;

use pki::{must_run, openssl_available};

// `ca_self_sign`: true → governance/permissions are signed directly with the
// CA cert (variant A, the Cyclone/FastDDS/OMG real-world pattern
// — tests the self-trust-anchor path in the CmsPkcs7Verifier). false →
// signed with the EE cert "alice" (variant B, RFC-5280 chain path).
fn gen_test_fixtures(dir: &Path, ca_self_sign: bool, full_lockdown: bool) {
    let _g = pki::OPENSSL_LOCK.lock().unwrap();
    let minted = pki::mint_ca_and_leaves(
        dir,
        "/CN=ZeroDDS Test FFI CA",
        "/CN=zerodds-bench-{name}/emailAddress={name}@bench.local",
        &["alice"],
    );
    let certs = dir.join("certs");
    let ca_key = certs.join("identity_ca_key.pem");
    let ca_cert = certs.join("identity_ca.pem");
    let alice_cert = minted[0].cert.clone();
    let alice_key = minted[0].key.clone();

    // 3. Governance + Permissions XML.
    //
    // `full_lockdown=false`: read/write-AC off — the create_secure happy path
    // with protection kinds only (like the 9 green ZeroDDS↔OpenDDS profiles).
    //
    // `full_lockdown=true`: join-AC + read-AC + write-AC all TRUE on every
    // topic — the `all-enc`/`sros2-full` governance shape. Per DDS-Security 1.2
    // §9.8.1 step 3 + Table 70 (`check_create_participant`, builtin
    // DDS:Access:Permissions) such a governance IS joinable by a participant
    // whose permissions grant covers the domain (here: alice's <allow_rule>
    // <domains><id>0</id>). Cyclone DDS and Fast DDS join it; SROS2
    // full-lockdown relies on it. Only OpenDDS self-rejects (its
    // AccessControlBuiltInImpl reads the governance topology only) — an
    // OpenDDS-specific stance, not binding on ZeroDDS. See
    // internal/security/opendds-secure-matrix-closeout.md.
    let read_write_ac = if full_lockdown { "true" } else { "false" };
    std::fs::write(
        dir.join("governance.xml"),
        format!(
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
          <enable_read_access_control>{read_write_ac}</enable_read_access_control>
          <enable_write_access_control>{read_write_ac}</enable_write_access_control>
          <metadata_protection_kind>ENCRYPT</metadata_protection_kind>
          <data_protection_kind>ENCRYPT</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
"#
        ),
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
/// Validates the self-trust-anchor path in the CmsPkcs7Verifier.
#[test]
fn ffi_runtime_create_secure_ca_self_signed() {
    run_e2e(true);
}

/// GitHub #11 regression: the §9.3.3 GUID-prefix guard added in
/// `DcpsRuntime::start` must NOT break the secured FFI path. `finish_secure_runtime`
/// now sets `security_guid_prefix` to the identity-adjusted prefix it also passes
/// to `start`, so the guard is satisfied and `create_secure` still returns a
/// runtime. If the FFI wiring regressed, `rt` would be NULL here.
#[test]
fn ffi_secure_runtime_constructs_post_guid_guard_gh11() {
    with_secure_runtime(false, |rt| {
        assert!(
            !rt.is_null(),
            "GH#11: §9.3.3 GUID-prefix guard must not break the secured FFI path"
        );
    });
}

/// Full-lockdown governance (`enable_join_access_control` + read-AC + write-AC
/// all TRUE, everything ENCRYPT — the `all-enc`/`sros2-full` shape) MUST be
/// joinable with a permissions grant covering the domain. DDS-Security 1.2
/// §9.8.1 step 3 + Table 70 `check_create_participant`; Cyclone/Fast DDS join
/// the same governance. Regression guard for the earlier topology-only gate
/// that denied every fully-locked domain unconditionally
/// (internal/security/opendds-secure-matrix-closeout.md, "Correction of an
/// earlier mis-fix").
#[test]
fn ffi_full_lockdown_governance_joins_with_matching_grant() {
    with_secure_runtime_on(false, true, 0, |rt| {
        assert!(
            !rt.is_null(),
            "full-AC governance + grant for domain 0 must create the participant \
             (§8.8.7 step 3 / Table 70 check_create_participant)"
        );
    });
}

/// Negative: a domain with NO governance `<domain_rule>` must be denied
/// (`check_create_participant` → FALSE → create_secure returns NULL).
#[test]
fn ffi_create_secure_denied_without_domain_rule() {
    with_secure_runtime_on(false, true, 7, |rt| {
        assert!(
            rt.is_null(),
            "domain 7 has no governance domain rule — create_secure must return NULL"
        );
    });
}

fn run_e2e(ca_self_sign: bool) {
    with_secure_runtime(ca_self_sign, |rt| {
        assert!(
            !rt.is_null(),
            "zerodds_runtime_create_secure must return a runtime"
        );
    });
}

/// Mint fixtures, build a secure config, call `zerodds_runtime_create_secure`,
/// hand the (possibly NULL) runtime pointer to `check`, then destroy both the
/// runtime and the config. Skips (without calling `check`) when openssl is
/// unavailable, so the whole flow is Linux/openssl-gated.
fn with_secure_runtime(ca_self_sign: bool, check: impl FnOnce(*mut ZeroDdsRuntime)) {
    with_secure_runtime_on(ca_self_sign, false, 0, check);
}

/// Like [`with_secure_runtime`], plus `full_lockdown` (governance shape, see
/// [`gen_test_fixtures`]) and `domain_id` (create_secure target domain).
fn with_secure_runtime_on(
    ca_self_sign: bool,
    full_lockdown: bool,
    domain_id: u32,
    check: impl FnOnce(*mut ZeroDdsRuntime),
) {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — security FFI e2e cannot run");
        return;
    }

    let tmp = tempdir_unique("zerodds_sec_ffi_");
    gen_test_fixtures(&tmp, ca_self_sign, full_lockdown);

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

    // Domain 0 matches the `<domain_rule>` in governance.xml; other ids have
    // no domain rule (negative tests).
    // SAFETY: cfg is a valid builder pointer; create_secure does not consume cfg.
    let rt: *mut ZeroDdsRuntime = unsafe { zerodds_runtime_create_secure(domain_id, cfg) };
    check(rt);

    // SAFETY: rt comes from create_secure, cfg from config_create — both not yet
    // freed; each pointer is destroyed exactly once. `zerodds_runtime_destroy`
    // tolerates a NULL pointer, so this is sound even if `create_secure` failed.
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
