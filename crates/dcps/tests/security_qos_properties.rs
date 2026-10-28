// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 §7.3.21 / Tables 50, 63, 71 — the participant's own
//! `PropertyQosPolicy` selects and configures its security plugins.
//!
//! Before this path existed, `crates/dcps` read exactly one subset of
//! the `dds.sec.*` namespace: `dds.sec.log.*`, through
//! `RuntimeConfig::with_security_log_properties`. §10.3.1 Table 50
//! (authentication) and §10.4.1 Table 63 (access control) had no reader
//! at all. A participant created with a complete and correct property
//! list therefore came up **without** a `SecurityBundle` — plaintext
//! RTPS, with nothing on either side saying so. `tools/omg-security-main`
//! worked around it by reading the list itself and building the bundle
//! by hand; every other caller got plaintext.
//!
//! Four clauses, four tests:
//!
//! 1. §9.3.1 + §9.4.1.2 — a complete property list produces a secured
//!    participant: the identity-derived §9.3.3 GUID on the wire, the
//!    handshake driver armed, and two such participants completing the
//!    authentication handshake with each other. Configuration passes
//!    through the QoS and nothing else.
//! 2. §8.4 — no `dds.sec.auth.*` / `dds.sec.access.*` property means no
//!    security. The participant stays plaintext, which is what an empty
//!    list asks for.
//! 3. §9.3.1 / §9.4.1.2 — an **incomplete** list fails the create and
//!    names the absent property. It does not fall back to plaintext:
//!    that is the silent-drop class `docs/silent-drop-sweep.md` names.
//! 4. §9.3.1 — the property values are URIs. All three `file:` spellings
//!    in the wild resolve to the path they name.
//!
//! Tests 1 and 4's positive half need real key material, so they mint it
//! with the `openssl` CLI (the recipe `security_guid_wiring_e2e.rs` and
//! `zerodds-c-api/tests/security_ffi_e2e.rs` already use) and skip with a
//! loud `[skip]` marker — never a silent pass — when `openssl` is not on
//! `PATH`. Tests 2 and 3 need no material at all.

#![cfg(feature = "security")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use zerodds_dcps::error::DdsError;
use zerodds_dcps::qos::PropertyQosPolicy;
use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_security_pki::{adjust_participant_guid_prefix, first_cert_der};
use zerodds_security_runtime::{
    PROP_GOVERNANCE, PROP_IDENTITY_CA, PROP_IDENTITY_CERT, PROP_IDENTITY_KEY, PROP_PERMISSIONS,
    PROP_PERMISSIONS_CA,
};

#[path = "common/pki.rs"]
mod pki;

use pki::{must_run, openssl_available, tempdir_unique};

/// A test CA plus two leaf identities (`alice`, `bob`) and a
/// CA-self-signed CMS governance/permissions pair — the same recipe
/// `security_guid_wiring_e2e.rs::gen_fixtures` uses, with the subject
/// names this file's permissions grant.
fn gen_fixtures(dir: &Path) {
    let _g = pki::OPENSSL_LOCK.lock().unwrap();
    pki::mint_ca_and_leaves(
        dir,
        "/CN=ZeroDDS QoS-Property Test CA",
        "/CN=zerodds-qosprop-{name}",
        &["alice", "bob"],
    );
    // Governance: one domain rule with no protection kinds — SPDP/SEDP
    // bootstrap in plaintext. The point here is the property path, not
    // the crypto transform (covered elsewhere). The `<id>` is filled in
    // per test run, because the domain is leased.
    std::fs::write(
        dir.join("governance.xml.in"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-SECURITY/20170801/omg_shared_ca_governance">
  <domain_access_rules>
    <domain_rule>
      <domains><id>@DOMAIN@</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>false</enable_join_access_control>
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

    std::fs::write(
        dir.join("permissions.xml.in"),
        r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-SECURITY/20170801/omg_shared_ca_permissions">
  <permissions>
    <grant>
      <subject_name>CN=zerodds-qosprop-alice</subject_name>
      <validity>
        <not_before>2025-01-01T00:00:00</not_before>
        <not_after>2099-01-01T00:00:00</not_after>
      </validity>
      <allow_rule>
        <domains><id>@DOMAIN@</id></domains>
        <publish><topics><topic>*</topic></topics></publish>
        <subscribe><topics><topic>*</topic></topics></subscribe>
      </allow_rule>
      <default>DENY</default>
    </grant>
    <grant>
      <subject_name>CN=zerodds-qosprop-bob</subject_name>
      <validity>
        <not_before>2025-01-01T00:00:00</not_before>
        <not_after>2099-01-01T00:00:00</not_after>
      </validity>
      <allow_rule>
        <domains><id>@DOMAIN@</id></domains>
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
}

/// Fills the leased domain id into the two XML templates and CMS-signs
/// them with the test CA.
fn sign_documents(dir: &Path, domain_id: i32) {
    let _g = pki::OPENSSL_LOCK.lock().unwrap();
    let certs = dir.join("certs");
    let ca_key = certs.join("identity_ca_key.pem");
    let ca_cert = certs.join("identity_ca.pem");
    for name in ["governance", "permissions"] {
        let template = std::fs::read_to_string(dir.join(format!("{name}.xml.in"))).unwrap();
        let xml = dir.join(format!("{name}.xml"));
        std::fs::write(&xml, template.replace("@DOMAIN@", &domain_id.to_string())).unwrap();
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

/// The six §9.3.1 / §9.4.1.2 properties for `name`, every value written
/// as a `file:///abs/path` URI — the spelling DDS-Security 1.2 §10.3.1
/// Table 44 asks for.
fn complete_properties(dir: &Path, name: &str) -> PropertyQosPolicy {
    let certs = dir.join("certs");
    let uri = |p: PathBuf| format!("file://{}", p.display());
    PropertyQosPolicy::new()
        .with(PROP_IDENTITY_CA, uri(certs.join("identity_ca.pem")))
        .with(
            PROP_IDENTITY_CERT,
            uri(certs.join(format!("{name}_cert.pem"))),
        )
        .with(
            PROP_IDENTITY_KEY,
            uri(certs.join(format!("{name}_key.pem"))),
        )
        .with(PROP_PERMISSIONS_CA, uri(certs.join("identity_ca.pem")))
        .with(PROP_GOVERNANCE, uri(dir.join("governance.p7s")))
        .with(PROP_PERMISSIONS, uri(dir.join("permissions.p7s")))
}

/// Independent §9.3.3 oracle: the adjusted prefix recomputed straight
/// from the certificate bytes, so the assertion does not compare the
/// property path against itself.
fn expected_prefix(cert_pem_path: &Path) -> GuidPrefix {
    // The candidate the participant builds: the vendor id, the host id,
    // six zero octets (DDSI-RTPS 2.5 §9.3.1.5 layout), and the participant
    // entity id. §9.3.3 hashes it into the second half of the prefix.
    let mut candidate = [0u8; 16];
    candidate[..12].copy_from_slice(
        &GuidPrefix::zerodds(zerodds_dcps::participant::host_id_bytes(), [0; 6]).to_bytes(),
    );
    candidate[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);
    let cert_pem = std::fs::read(cert_pem_path).unwrap();
    let cert_der = first_cert_der(&cert_pem).unwrap();
    GuidPrefix::from_bytes(adjust_participant_guid_prefix(&candidate, &cert_der).unwrap())
}

// ---------------------------------------------------------------------
// 1. §9.3.1 + §9.4.1.2 — a complete property list produces security
// ---------------------------------------------------------------------

/// A participant whose QoS carries the six properties gets a
/// `SecurityBundle` built from them, runs on the §9.3.3 identity-derived
/// GUID, has the handshake driver armed, and completes the
/// authentication handshake with a second such participant.
///
/// Nothing but the `PropertyQosPolicy` is handed in — no `with_security`,
/// no `SecurityProfile`, no `enable_security_builtins_with_auth` from the
/// test. Against the parent commit this fails on the first assertion:
/// the participant came up on a random GUID prefix with no security at
/// all.
#[serial_test::file_serial(security_live)]
#[test]
fn a_complete_dds_sec_property_list_creates_a_secured_participant() {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — dds.sec.* property security test cannot run");
        return;
    }
    let tmp = tempdir_unique("zerodds_sec_qosprop_live_");
    gen_fixtures(&tmp);

    let lease = common::lease_domain();
    let domain = lease.id();
    sign_documents(&tmp, domain);

    let factory = DomainParticipantFactory::instance();
    // One shared SPDP group for the two — `isolated_cfg` hands out a
    // fresh group per call, so the second participant gets a clone
    // rather than a second call.
    let cfg = common::isolated_cfg();
    let participant_a = factory
        .create_participant_with_config(
            domain,
            DomainParticipantQos {
                property: complete_properties(&tmp, "alice"),
                ..DomainParticipantQos::default()
            },
            cfg.clone(),
        )
        .expect("alice must be created from her property list alone");
    let participant_a = common::ParticipantGuard::new(participant_a);
    let participant_b = factory
        .create_participant_with_config(
            domain,
            DomainParticipantQos {
                property: complete_properties(&tmp, "bob"),
                ..DomainParticipantQos::default()
            },
            cfg,
        )
        .expect("bob must be created from his property list alone");
    let participant_b = common::ParticipantGuard::new(participant_b);

    // (1) §9.3.3 — the on-wire GUID prefix is the identity-derived one,
    // which can only be true if the profile was built from the property
    // list and wired into the runtime.
    let certs = tmp.join("certs");
    let want_a = expected_prefix(&certs.join("alice_cert.pem"));
    let want_b = expected_prefix(&certs.join("bob_cert.pem"));
    assert_ne!(want_a, want_b, "distinct identities → distinct prefixes");
    let rt_a = participant_a.runtime().expect("live runtime").clone();
    let rt_b = participant_b.runtime().expect("live runtime").clone();
    assert_eq!(
        rt_a.guid_prefix, want_a,
        "the participant's wire GUID must be derived from the identity certificate \
         named by dds.sec.auth.identity_certificate — a random prefix means the \
         property list was never read"
    );
    assert_eq!(rt_b.guid_prefix, want_b);

    // (2) the authentication handshake driver is armed. A crypto gate
    // without it never runs a handshake, so every secured peer stays
    // unauthenticated and no endpoint matches, with nothing saying why.
    assert!(
        rt_a.security_builtin_snapshot().is_some(),
        "creating a participant from a dds.sec.* property list must arm the \
         handshake driver, not just install the gate"
    );
    assert!(rt_b.security_builtin_snapshot().is_some());

    // (3) the two speak secured: a real SPDP discovery plus the PKI
    // authentication handshake to a shared secret, over UDP.
    let stack_a = rt_a.security_builtin_snapshot().unwrap();
    let stack_b = rt_b.security_builtin_snapshot().unwrap();
    let deadline = Instant::now() + common::match_timeout().max(Duration::from_secs(30));
    let mut done = false;
    while Instant::now() < deadline {
        let a_ok = stack_a
            .lock()
            .map(|s| s.completed_peer_prefixes().contains(&want_b))
            .unwrap_or(false);
        let b_ok = stack_b
            .lock()
            .map(|s| s.completed_peer_prefixes().contains(&want_a))
            .unwrap_or(false);
        if a_ok && b_ok {
            done = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let seen = |rt: &std::sync::Arc<zerodds_dcps::runtime::DcpsRuntime>| {
        rt.discovered_participants()
            .iter()
            .map(|p| format!("{:?}/{:?}", p.sender_prefix, p.data.domain_id))
            .collect::<Vec<_>>()
    };
    let completed = |stack: &std::sync::Arc<std::sync::Mutex<_>>| {
        stack
            .lock()
            .map(
                |s: std::sync::MutexGuard<
                    '_,
                    zerodds_discovery::security::SecurityBuiltinStack,
                >| { s.completed_peer_prefixes() },
            )
            .unwrap_or_default()
    };
    assert!(
        done,
        "two participants configured only through their PropertyQosPolicy must \
         complete the DDS-Security authentication handshake (domain {domain}; \
         a = {want_a:?} saw {:?}, completed {:?}; b = {want_b:?} saw {:?}, completed {:?})",
        seen(&rt_a),
        completed(&stack_a),
        seen(&rt_b),
        completed(&stack_b),
    );
}

// ---------------------------------------------------------------------
// 2. §8.4 — no security property, no security
// ---------------------------------------------------------------------

/// A participant with no `dds.sec.auth.*` / `dds.sec.access.*` property
/// stays plaintext. `dds.sec.log.*` alone does not change that: the §8.6
/// logging plugin is observability, not a security configuration, and
/// treating it as one would refuse every plaintext participant that
/// wants security-event logging.
#[test]
fn a_participant_without_security_properties_stays_plaintext() {
    let lease = common::lease_domain();
    let factory = DomainParticipantFactory::instance();

    for property in [
        PropertyQosPolicy::new(),
        PropertyQosPolicy::new().with("dds.sec.log.plugin", "stderr"),
    ] {
        let participant = factory
            .create_participant_with_config(
                lease.id(),
                DomainParticipantQos {
                    property,
                    ..DomainParticipantQos::default()
                },
                common::isolated_cfg(),
            )
            .expect("a participant without security properties must be created");
        let participant = common::ParticipantGuard::new(participant);
        let rt = participant.runtime().expect("live runtime");
        assert!(
            rt.security_builtin_snapshot().is_none(),
            "no dds.sec.auth./dds.sec.access. property means no security — the \
             handshake driver must not be armed"
        );
    }
}

// ---------------------------------------------------------------------
// 3. §9.3.1 / §9.4.1.2 — an incomplete list fails, by name
// ---------------------------------------------------------------------

/// Every one of the six is required once the list asks for security.
/// Dropping any one of them fails `create_participant` and names it —
/// it does not produce a participant that quietly runs unencrypted
/// because one path was missing.
#[test]
fn an_incomplete_property_list_fails_the_create_and_names_the_property() {
    let lease = common::lease_domain();
    let factory = DomainParticipantFactory::instance();
    let full = [
        (PROP_IDENTITY_CA, "file:/zerodds/nonexistent/ca.pem"),
        (PROP_IDENTITY_CERT, "file:/zerodds/nonexistent/cert.pem"),
        (PROP_IDENTITY_KEY, "file:/zerodds/nonexistent/key.pem"),
        (PROP_PERMISSIONS_CA, "file:/zerodds/nonexistent/pca.pem"),
        (PROP_GOVERNANCE, "file:/zerodds/nonexistent/gov.p7s"),
        (PROP_PERMISSIONS, "file:/zerodds/nonexistent/perm.p7s"),
    ];

    for (dropped, _) in full {
        let mut property = PropertyQosPolicy::new();
        for (name, value) in full {
            if name != dropped {
                property.set(name, value);
            }
        }
        let result = factory.create_participant_with_config(
            lease.id(),
            DomainParticipantQos {
                property,
                ..DomainParticipantQos::default()
            },
            common::isolated_cfg(),
        );
        let err = match result {
            Err(e) => e,
            Ok(p) => {
                let _ = factory.delete_participant(&p);
                panic!(
                    "dropping {dropped} produced a participant — an incomplete \
                     dds.sec.* list must fail the create, never come up in plaintext"
                );
            }
        };
        assert!(
            matches!(err, DdsError::SecurityConfiguration { .. }),
            "dropping {dropped} must fail as a security-configuration error, got {err:?}"
        );
        let text = err.to_string();
        assert!(
            text.contains(dropped),
            "the failure must name the absent property '{dropped}'; got: {text}"
        );
    }
}

// ---------------------------------------------------------------------
// 4. §9.3.1 — the property values are URIs
// ---------------------------------------------------------------------

/// All three `file:` spellings that occur in the wild resolve to the
/// path they name, on the participant-create path.
///
/// - `file:///abs/path` — the spec's own examples.
/// - `file://host/path` — the two-slash form `strip_file_url` was
///   written for.
/// - `file:relative/path` — the scheme with no authority at all, which
///   the OMG `dds-security` suite and DDS-Security 1.2 §10.4.1 Table 63's
///   examples write. `strip_file_url` returns that string unchanged, so
///   before `resolve_file_uri` an open failed on a path literally
///   beginning `file:`.
///
/// The negative half points every property at a directory that does not
/// exist and asserts the reported path carries no `file:` prefix any
/// more — the scheme was stripped, whichever spelling it had. The
/// positive half is the live test above, which creates a working
/// participant from `file://`-prefixed absolute paths.
#[test]
fn every_file_uri_spelling_is_resolved_on_the_participant_path() {
    let lease = common::lease_domain();
    let factory = DomainParticipantFactory::instance();
    // (the value as written, the path it must resolve to). The
    // authority-consuming `file://` form is why the third case reports a
    // relative path: `file:///a/b` is `/a/b`, `file://a/b` is `a/b`.
    let forms = [
        (
            "file:///zerodds/nonexistent/uri/ca.pem",
            "/zerodds/nonexistent/uri/ca.pem",
        ),
        (
            "file:/zerodds/nonexistent/uri/ca.pem",
            "/zerodds/nonexistent/uri/ca.pem",
        ),
        (
            "file://zerodds/nonexistent/uri/ca.pem",
            "zerodds/nonexistent/uri/ca.pem",
        ),
    ];

    for (value, expected) in forms {
        let value = value.to_string();
        let property = PropertyQosPolicy::new()
            .with(PROP_IDENTITY_CA, value.clone())
            .with(PROP_IDENTITY_CERT, value.clone())
            .with(PROP_IDENTITY_KEY, value.clone())
            .with(PROP_PERMISSIONS_CA, value.clone())
            .with(PROP_GOVERNANCE, value.clone())
            .with(PROP_PERMISSIONS, value.clone());
        let result = factory.create_participant_with_config(
            lease.id(),
            DomainParticipantQos {
                property,
                ..DomainParticipantQos::default()
            },
            common::isolated_cfg(),
        );
        let err = match result {
            Err(e) => e,
            Ok(p) => {
                let _ = factory.delete_participant(&p);
                panic!("'{value}' names no file — the create must fail");
            }
        };
        let text = err.to_string();
        assert!(
            text.contains(expected),
            "'{value}' must resolve to '{expected}'; got: {text}"
        );
        assert!(
            !text.contains("file:"),
            "the scheme in '{value}' must be stripped before the open; got: {text}"
        );
    }
}

/// Leases a DDS domain no other process on this host holds — the domain
/// number is the UDP port number, so it is a host-wide resource.
#[path = "common/mod.rs"]
mod common;

// ---------------------------------------------------------------------
// 5. `security.enclave_dir` of the runtime configuration
// ---------------------------------------------------------------------

/// The SROS2 layout of `SecurityProfile::from_enclave_dir`, out of the
/// fixtures of this file.
fn enclave_for(dir: &Path, name: &str) -> PathBuf {
    let certs = dir.join("certs");
    let enclave = dir.join(format!("enclave-{name}"));
    std::fs::create_dir_all(&enclave).unwrap();
    for (from, to) in [
        (certs.join("identity_ca.pem"), "identity_ca.cert.pem"),
        (certs.join(format!("{name}_cert.pem")), "cert.pem"),
        (certs.join(format!("{name}_key.pem")), "key.pem"),
        (certs.join("identity_ca.pem"), "permissions_ca.cert.pem"),
        (dir.join("governance.p7s"), "governance.p7s"),
        (dir.join("permissions.p7s"), "permissions.p7s"),
    ] {
        std::fs::copy(from, enclave.join(to)).unwrap();
    }
    enclave
}

/// A configuration naming an enclave (`security.enclave_dir`, env
/// `ZERODDS_SECURITY_DIR`) gives every Rust participant its security: the
/// identity-derived GUID and the handshake driver. Before, only the ROS 2
/// RMW and the C-API's secured constructor read the setting; every other
/// program accepted it and started plaintext.
#[serial_test::file_serial(security_live)]
#[test]
fn a_configured_enclave_secures_the_participant() {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — enclave participant test cannot run");
        return;
    }
    let tmp = tempdir_unique("zerodds_sec_enclave_cfg_");
    gen_fixtures(&tmp);
    let lease = common::lease_domain();
    let domain = lease.id();
    sign_documents(&tmp, domain);
    let enclave = enclave_for(&tmp, "alice");

    let factory = DomainParticipantFactory::instance();
    let mut cfg = common::isolated_cfg();
    cfg.security_enclave_dir = Some(enclave.clone());
    let p = factory
        .create_participant_with_config(domain, DomainParticipantQos::default(), cfg.clone())
        .expect("a participant from a configured enclave");
    let p = common::ParticipantGuard::new(p);
    let rt = p.runtime().expect("live runtime").clone();
    assert_eq!(
        rt.guid_prefix,
        expected_prefix(&tmp.join("certs").join("alice_cert.pem")),
        "the GUID must come from the enclave's identity"
    );
    assert!(
        rt.security_builtin_snapshot().is_some(),
        "handshake driver armed"
    );

    // Enclave and properties at once is a contradiction, refused.
    let both = factory.create_participant_with_config(
        domain,
        DomainParticipantQos {
            property: complete_properties(&tmp, "alice"),
            ..DomainParticipantQos::default()
        },
        cfg.clone(),
    );
    let err = format!("{}", both.expect_err("enclave plus properties is refused"));
    assert!(err.contains("name one"), "{err}");

    // An enclave that does not load is an error, never a plain participant.
    cfg.security_enclave_dir = Some(tmp.join("no-such-enclave"));
    let missing =
        factory.create_participant_with_config(domain, DomainParticipantQos::default(), cfg);
    let err = format!("{}", missing.expect_err("a missing enclave is refused"));
    assert!(err.contains("security.enclave_dir"), "{err}");
}
