// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! #11 fix verification — DDS-Security §9.3.3 identity-adjusted
//! participant GUID prefix, wired onto the **public** `zerodds-dcps` Rust
//! API path.
//!
//! `adjust_participant_guid_prefix` (`security-pki`) and
//! `SecurityProfile::from_files` (`security-runtime`) compute the
//! identity-adjusted GUID correctly, byte-identical to OpenDDS/Cyclone.
//! But before this fix, `RuntimeConfig::with_security_bundle` copied the
//! profile's `gate` into `RuntimeConfig.security` and silently dropped
//! `profile.adjusted_participant_guid`; `DomainParticipant::new_with_runtime`
//! always called `random_guid_prefix()` regardless. Only the FFI path
//! (`zerodds-c-api::security_ffi::finish_secure_runtime`) used the adjusted
//! prefix. A participant created through the public
//! `DomainParticipantFactory` under security would announce a random wire
//! GUID that a peer's §9.3.3 handshake check (`c.pdata` GUID must derive
//! from the identity cert) rejects — the "13/13 cross-vendor security"
//! claim was FFI/rmw-backed only, never exercised on this path.
//!
//! This test generates real identities via the `openssl` CLI + CMS-signed
//! governance/permissions (same recipe as
//! `zerodds-c-api/tests/security_ffi_e2e.rs`), builds a `SecurityProfile`
//! via `SecurityProfile::from_files` (the same constructor the FFI path
//! uses), wires it through `SecurityBundle` + `RuntimeConfig::
//! with_security_bundle`, and creates the participant via the public
//! `DomainParticipantFactory` — no hardcoded GUID prefix
//! (`GuidPrefix::from_bytes([..])` like `security_live_e2e.rs`/
//! `security_matrix_e2e.rs` use), no `zerodds-c-api` FFI. It asserts the
//! actual on-wire participant GUID (`DomainParticipant::participant_handle`,
//! backed by `DcpsRuntime.guid_prefix`) equals a §9.3.3 value recomputed
//! independently from the cert bytes, then runs two such participants
//! through a real secured discovery + auth handshake over UDP to prove the
//! wiring holds end-to-end on the public API.
//!
//! Skips (with a loud `[skip]` marker, not a silent pass) if `openssl` is
//! not on `PATH`.

#![cfg(feature = "security")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos, InstanceHandle};
use zerodds_rtps::wire_types::{EntityId, Guid, GuidPrefix, VendorId};
use zerodds_security_pki::{adjust_participant_guid_prefix, first_cert_der};
use zerodds_security_runtime::{SecurityBundle, SecurityProfile, SecurityProfileConfig};

// Keep openssl calls serial — avoids a race on the shared `.srl` counter
// file and sub-process startup storms (mirrors
// `zerodds-c-api/tests/security_ffi_e2e.rs`).
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

fn tempdir_unique(prefix: &str) -> PathBuf {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let p = std::env::temp_dir().join(format!("{prefix}{now}_{}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// Generates a test CA + two leaf identities (`alice`, `bob`) plus a
/// CA-self-signed CMS governance.p7s/permissions.p7s (Cyclone/FastDDS/OMG
/// real-world pattern), analogous to `security_ffi_e2e.rs::gen_test_fixtures`
/// but with two identities so a real 2-participant secured discovery can
/// run. Governance carries NO protection kinds (SPDP/data stay plaintext) —
/// mirrors `security_live_e2e.rs::GOV`: the point of this test is the GUID
/// wiring + discovery, not the crypto-transform path (that is covered
/// elsewhere).
fn gen_fixtures(dir: &Path) {
    let _g = OPENSSL_LOCK.lock().unwrap();
    let certs = dir.join("certs");
    std::fs::create_dir_all(&certs).unwrap();

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
        .args([
            "-days",
            "30",
            "-subj",
            "/CN=ZeroDDS Test GUID-Wiring CA",
            "-out",
        ])
        .arg(&ca_cert);
    must_run("ca self-sign", c);

    let mint = |name: &str| -> (PathBuf, PathBuf) {
        let key = certs.join(format!("{name}_key.pem"));
        let csr = certs.join(format!("{name}.csr"));
        let cert = certs.join(format!("{name}_cert.pem"));

        let mut c = Command::new("openssl");
        c.args([
            "genpkey",
            "-algorithm",
            "EC",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-out",
        ])
        .arg(&key);
        must_run("genpkey leaf-key", c);

        let mut c = Command::new("openssl");
        c.args(["req", "-new", "-key"])
            .arg(&key)
            .args(["-subj", &format!("/CN=zerodds-guid-{name}"), "-out"])
            .arg(&csr);
        must_run("leaf csr", c);

        let mut c = Command::new("openssl");
        c.args(["x509", "-req", "-in"])
            .arg(&csr)
            .args(["-CA"])
            .arg(&ca_cert)
            .args(["-CAkey"])
            .arg(&ca_key)
            .args(["-CAcreateserial", "-days", "30", "-out"])
            .arg(&cert);
        must_run("leaf ca-sign", c);
        (cert, key)
    };
    mint("alice");
    mint("bob");

    // Governance: domain 0, wide open (no protection kinds, no access
    // control) — SPDP/SEDP discovery bootstraps in plaintext (same
    // reasoning as `security_live_e2e.rs::GOV`).
    std::fs::write(
        dir.join("governance.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-SECURITY/20170801/omg_shared_ca_governance">
  <domain_access_rules>
    <domain_rule>
      <domains><id>0</id></domains>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>*</topic_expression>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
"#,
    )
    .unwrap();

    // Permissions: one grant per identity, full allow (access control is
    // disabled in governance above — these grants only need to parse +
    // CMS-verify so `c.perm` is present in the handshake).
    std::fs::write(
        dir.join("permissions.xml"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-SECURITY/20170801/omg_shared_ca_permissions">
  <permissions>
    <grant>
      <subject_name>CN=zerodds-guid-alice</subject_name>
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
    <grant>
      <subject_name>CN=zerodds-guid-bob</subject_name>
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

    for name in ["governance", "permissions"] {
        let xml = dir.join(format!("{name}.xml"));
        let p7s = dir.join(format!("{name}.p7s"));
        let mut c = Command::new("openssl");
        c.args(["smime", "-sign", "-in"])
            .arg(&xml)
            .args(["-text", "-out"])
            .arg(&p7s)
            .args(["-signer"])
            .arg(&ca_cert)
            .args(["-inkey"])
            .arg(&ca_key);
        must_run(&format!("cms-sign {name}"), c);
    }
}

/// Builds the [`SecurityProfileConfig`] for `name` (`"alice"`/`"bob"`)
/// against fixtures in `dir`.
fn profile_cfg(dir: &Path, name: &str) -> SecurityProfileConfig {
    let certs = dir.join("certs");
    SecurityProfileConfig {
        domain_id: 0,
        identity_ca_pem: certs.join("identity_ca.pem"),
        identity_cert_pem: certs.join(format!("{name}_cert.pem")),
        identity_key_pem: certs.join(format!("{name}_key.pem")),
        permissions_ca_pem: certs.join("identity_ca.pem"),
        governance_p7s: dir.join("governance.p7s"),
        permissions_p7s: dir.join("permissions.p7s"),
    }
}

/// Independent §9.3.3 oracle: recomputes the adjusted prefix directly from
/// the cert PEM bytes, without going through `SecurityProfile` at all —
/// so a bug in `SecurityProfile::from_files` itself would not be masked by
/// comparing its own output against itself.
fn expected_adjusted_prefix(cert_pem_path: &Path, candidate_guid: &[u8; 16]) -> GuidPrefix {
    let cert_pem = std::fs::read(cert_pem_path).unwrap();
    let cert_der = first_cert_der(&cert_pem).unwrap();
    let bytes = adjust_participant_guid_prefix(candidate_guid, &cert_der).unwrap();
    GuidPrefix::from_bytes(bytes)
}

fn candidate_guid(seed: u8) -> [u8; 16] {
    let mut g = [seed; 16];
    // Participant EntityId suffix (0x000001C1), same convention the FFI
    // path and `SecurityProfile` docs use for the 16-byte candidate.
    g[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);
    g
}

/// Wiring-only check (no live participant): `with_security_bundle` must
/// copy `profile.adjusted_participant_guid` into
/// `RuntimeConfig.security_guid_prefix` — the field
/// `DomainParticipant::new_with_runtime` reads instead of falling back to
/// `random_guid_prefix()`.
#[test]
fn with_security_bundle_wires_the_identity_adjusted_prefix_not_random() {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — security GUID-wiring test cannot run");
        return;
    }
    let tmp = tempdir_unique("zerodds_sec_guid_wire_");
    gen_fixtures(&tmp);
    let cfg = profile_cfg(&tmp, "alice");
    let candidate = candidate_guid(0x11);

    let expected = expected_adjusted_prefix(&cfg.identity_cert_pem, &candidate);
    // Sanity: the adjustment must actually change the prefix (high bit of
    // byte 0 is always set per §9.3.3 — the raw candidate here does not
    // have it), otherwise this test would not distinguish "wired
    // correctly" from "wired the raw candidate by accident".
    assert_ne!(
        expected.to_bytes()[..],
        candidate[..12],
        "the derived prefix must differ from the raw (pre-adjustment) candidate"
    );

    let profile = SecurityProfile::from_files(&cfg, candidate).expect("profile build");
    assert_eq!(
        &profile.adjusted_participant_guid[..12],
        &expected.to_bytes()[..],
        "SecurityProfile::from_files must store the §9.3.3-adjusted prefix"
    );

    let bundle = SecurityBundle::builder().security_profile(profile).build();
    let rt_cfg = RuntimeConfig::default().with_security_bundle(&bundle);
    assert_eq!(
        rt_cfg.security_guid_prefix,
        Some(expected),
        "with_security_bundle must wire the adjusted prefix into \
         RuntimeConfig.security_guid_prefix — before the fix this field did \
         not exist and the prefix was silently dropped"
    );
}

/// Full fix verification over the **public** Rust API: two secured
/// participants, created via `DomainParticipantFactory::
/// create_participant_with_config` (not `DcpsRuntime::start` with a
/// hardcoded `GuidPrefix::from_bytes([..])` like the other security e2e
/// tests, not `zerodds-c-api` FFI), each carrying a `SecurityProfile`
/// built from real generated identities. Asserts:
/// 1. each participant's actual on-wire GUID (`participant_handle`,
///    backed by `DcpsRuntime.guid_prefix`) equals the independently
///    recomputed §9.3.3 value — this is the line that was silently
///    wrong before the fix (`random_guid_prefix()` instead).
/// 2. the two participants actually find each other via SPDP and
///    complete the PKI auth handshake over real UDP — proving the wiring
///    holds under live secured discovery, not just in a config struct.
#[serial_test::serial(security_live)]
#[test]
fn secured_participants_on_public_rust_api_use_identity_adjusted_guids_and_discover() {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — secured public-API discovery test cannot run");
        return;
    }
    let tmp = tempdir_unique("zerodds_sec_guid_disc_");
    gen_fixtures(&tmp);

    let cfg_a = profile_cfg(&tmp, "alice");
    let cfg_b = profile_cfg(&tmp, "bob");
    let cand_a = candidate_guid(0x2a);
    let cand_b = candidate_guid(0x4b);
    let expected_a = expected_adjusted_prefix(&cfg_a.identity_cert_pem, &cand_a);
    let expected_b = expected_adjusted_prefix(&cfg_b.identity_cert_pem, &cand_b);
    assert_ne!(
        expected_a, expected_b,
        "distinct identities → distinct prefixes"
    );

    let profile_a = SecurityProfile::from_files(&cfg_a, cand_a).expect("alice profile");
    let profile_b = SecurityProfile::from_files(&cfg_b, cand_b).expect("bob profile");

    let bundle_a = SecurityBundle::builder()
        .security_profile(profile_a)
        .build();
    let bundle_b = SecurityBundle::builder()
        .security_profile(profile_b)
        .build();
    let profile_a = bundle_a.security_profile().expect("bundle carries profile");
    let profile_b = bundle_b.security_profile().expect("bundle carries profile");

    let rt_cfg_a = RuntimeConfig::default().with_security_bundle(&bundle_a);
    let rt_cfg_b = RuntimeConfig::default().with_security_bundle(&bundle_b);

    // The PUBLIC API: DomainParticipantFactory, not DcpsRuntime::start.
    let factory = DomainParticipantFactory::instance();
    let participant_a = factory
        .create_participant_with_config(0, DomainParticipantQos::default(), rt_cfg_a)
        .expect("alice participant create");
    let participant_b = factory
        .create_participant_with_config(0, DomainParticipantQos::default(), rt_cfg_b)
        .expect("bob participant create");

    // (1) on-wire GUID == §9.3.3-derived value, on the public API path.
    let expected_handle_a = InstanceHandle::from_guid(Guid::new(expected_a, EntityId::PARTICIPANT));
    let expected_handle_b = InstanceHandle::from_guid(Guid::new(expected_b, EntityId::PARTICIPANT));
    assert_eq!(
        participant_a.participant_handle(),
        expected_handle_a,
        "alice's on-wire participant GUID must equal the §9.3.3-derived \
         value on the public zerodds-dcps Rust API path — before the fix \
         this was a random_guid_prefix() value instead, which a peer's \
         §9.3.3 check would reject"
    );
    assert_eq!(
        participant_b.participant_handle(),
        expected_handle_b,
        "bob's on-wire participant GUID must equal the §9.3.3-derived value"
    );

    // (2) live secured discovery + auth handshake over real UDP, driven
    // entirely from the public API's `DomainParticipant::runtime()` handle
    // (still no FFI).
    let rt_a = participant_a.runtime().expect("live runtime").clone();
    let rt_b = participant_b.runtime().expect("live runtime").clone();
    rt_a.enable_security_builtins_with_auth(
        VendorId::ZERODDS,
        profile_a.pki.clone(),
        profile_a.identity_handle,
    );
    rt_b.enable_security_builtins_with_auth(
        VendorId::ZERODDS,
        profile_b.pki.clone(),
        profile_b.identity_handle,
    );

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut ok = false;
    while Instant::now() < deadline {
        if profile_a
            .gate
            .slot_for(&expected_b.to_bytes())
            .ok()
            .flatten()
            .is_some()
            && profile_b
                .gate
                .slot_for(&expected_a.to_bytes())
                .ok()
                .flatten()
                .is_some()
        {
            ok = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        ok,
        "secured handshake over the public Rust API did not complete \
         (a_disc={}, b_disc={}) — the identity-adjusted GUIDs must match \
         what each side's PKI plugin validated for the handshake to \
         succeed at all",
        rt_a.discovered_participants().len(),
        rt_b.discovered_participants().len(),
    );
}
