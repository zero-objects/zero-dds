// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 §9.8.7 (Figure 25) — AccessControl behaviour with a
//! discovered DomainParticipant.
//!
//! Two participants share one CA and authenticate. `alice`'s governance has
//! `enable_join_access_control` TRUE (`is_access_protected`), `bob`'s FALSE.
//! The permissions document grants `alice` the domain and `bob` another
//! domain only:
//!
//! - `alice` validates `bob`'s permissions (`validate_remote_permissions`) and
//!   asks `check_remote_participant`; the grant of `bob` does not allow the
//!   domain, so "the discovered DomainParticipant shall be considered ignored
//!   and all the builtin Topics should not be matched" (§9.8.7): `bob` is not
//!   among `alice`'s discovered participants.
//! - `bob`'s domain is not access protected: "validation will succeed with no
//!   access control checking. In this case, only
//!   get_authenticated_peer_credential_token and validate_remote_permissions
//!   are called, and a HandleNIL return will not impact the validation
//!   result." `alice` stays among `bob`'s discovered participants.
//!
//! The fixtures need the `openssl` command; a host without it prints a
//! `[skip]` line and the test does not count as run. Linux only: the
//! discovery runs over UDP multicast.

#![cfg(all(target_os = "linux", feature = "security"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use zerodds_dcps::qos::PropertyQosPolicy;
use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};
use zerodds_rtps::wire_types::GuidPrefix;
use zerodds_security_pki::{adjust_participant_guid_prefix, first_cert_der};
use zerodds_security_runtime::{
    PROP_GOVERNANCE, PROP_IDENTITY_CA, PROP_IDENTITY_CERT, PROP_IDENTITY_KEY, PROP_PERMISSIONS,
    PROP_PERMISSIONS_CA,
};

#[path = "common/mod.rs"]
mod common;
#[path = "common/pki.rs"]
mod pki;

use pki::{must_run, openssl_available, tempdir_unique};

const GOVERNANCE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-SECURITY/20170801/omg_shared_ca_governance">
  <domain_access_rules>
    <domain_rule>
      <domains><id>@DOMAIN@</id></domains><allow_unauthenticated_participants>false</allow_unauthenticated_participants><enable_join_access_control>@JOIN@</enable_join_access_control>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>*</topic_expression>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
"#;

const PERMISSIONS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns="http://www.omg.org/spec/DDS-SECURITY/20170801/omg_shared_ca_permissions">
  <permissions>
    <grant>
      <subject_name>CN=zerodds-remote-alice</subject_name>
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
      <subject_name>CN=zerodds-remote-bob</subject_name>
      <validity>
        <not_before>2025-01-01T00:00:00</not_before>
        <not_after>2099-01-01T00:00:00</not_after>
      </validity>
      <allow_rule>
        <domains><id>@OTHER@</id></domains>
        <publish><topics><topic>*</topic></topics></publish>
        <subscribe><topics><topic>*</topic></topics></subscribe>
      </allow_rule>
      <default>DENY</default>
    </grant>
  </permissions>
</dds>
"#;

fn sign(dir: &Path, name: &str, xml_text: &str) {
    let _g = pki::OPENSSL_LOCK.lock().unwrap();
    let xml = dir.join(format!("{name}.xml"));
    std::fs::write(&xml, xml_text).unwrap();
    let certs = dir.join("certs");
    let mut c = Command::new("openssl");
    c.args(["smime", "-sign", "-in"])
        .arg(&xml)
        .args(["-text", "-out"])
        .arg(dir.join(format!("{name}.p7s")))
        .args(["-signer"])
        .arg(certs.join("identity_ca.pem"))
        .args(["-inkey"])
        .arg(certs.join("identity_ca_key.pem"));
    must_run(&format!("cms-sign {name}"), c);
}

fn uri(p: PathBuf) -> String {
    format!("file://{}", p.display())
}

fn properties(dir: &Path, name: &str, governance: &str) -> PropertyQosPolicy {
    let certs = dir.join("certs");
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
        .with(PROP_GOVERNANCE, uri(dir.join(governance)))
        .with(PROP_PERMISSIONS, uri(dir.join("permissions.p7s")))
}

fn prefix_of(cert_pem: &Path) -> GuidPrefix {
    let mut candidate = [0u8; 16];
    candidate[..12].copy_from_slice(
        &GuidPrefix::zerodds(zerodds_dcps::participant::host_id_bytes(), [0; 6]).to_bytes(),
    );
    candidate[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);
    let der = first_cert_der(&std::fs::read(cert_pem).unwrap()).unwrap();
    GuidPrefix::from_bytes(adjust_participant_guid_prefix(&candidate, &der).unwrap())
}

fn wait_for(timeout: Duration, mut ready: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if ready() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// §9.8.7: an authenticated peer whose permissions do not allow the domain is
/// ignored where the domain is access protected (`is_access_protected`
/// TRUE); where it is not, the same check has no consequence.
#[serial_test::file_serial(security_live)]
#[test]
fn an_authenticated_peer_without_a_domain_grant_is_ignored_only_where_the_domain_is_protected() {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — the remote participant access test cannot run");
        return;
    }
    let dir = tempdir_unique("zerodds_sec_remote_access_");
    {
        let _g = pki::OPENSSL_LOCK.lock().unwrap();
        pki::mint_ca_and_leaves(
            &dir,
            "/CN=ZeroDDS Remote Access CA",
            "/CN=zerodds-remote-{name}",
            &["alice", "bob"],
        );
    }
    let lease = common::lease_domain();
    let domain = lease.id();
    let gov = |join: &str| {
        GOVERNANCE
            .replace("@DOMAIN@", &domain.to_string())
            .replace("@JOIN@", join)
    };
    sign(&dir, "governance_protected", &gov("TRUE"));
    sign(&dir, "governance_open", &gov("FALSE"));
    sign(
        &dir,
        "permissions",
        &PERMISSIONS
            .replace("@DOMAIN@", &domain.to_string())
            .replace("@OTHER@", &(domain + 1).to_string()),
    );

    let factory = DomainParticipantFactory::instance();
    let sink = std::sync::Arc::new(zerodds_foundation::observability::VecSink::new());
    let mut cfg = common::isolated_cfg();
    cfg.observability = sink.clone();
    let create = |name: &str, governance: &str| {
        factory
            .create_participant_with_config(
                domain,
                DomainParticipantQos {
                    property: properties(&dir, name, governance),
                    ..DomainParticipantQos::default()
                },
                cfg.clone(),
            )
            .unwrap_or_else(|e| panic!("{name}: {e}"))
    };
    let alice = common::ParticipantGuard::new(create("alice", "governance_protected.p7s"));
    let bob = common::ParticipantGuard::new(create("bob", "governance_open.p7s"));
    let alice_prefix = prefix_of(&dir.join("certs/alice_cert.pem"));
    let bob_prefix = prefix_of(&dir.join("certs/bob_cert.pem"));
    let rt_a = alice.runtime().expect("runtime").clone();
    let rt_b = bob.runtime().expect("runtime").clone();

    // The handshake completes on both sides: `bob` is authenticated for
    // `alice` as `alice` is for `bob`.
    let stack_a = rt_a.security_builtin_snapshot().expect("stack of alice");
    let stack_b = rt_b.security_builtin_snapshot().expect("stack of bob");
    assert!(
        wait_for(Duration::from_secs(30), || {
            stack_a
                .lock()
                .map(|s| s.completed_peer_prefixes().contains(&bob_prefix))
                .unwrap_or(false)
                && stack_b
                    .lock()
                    .map(|s| s.completed_peer_prefixes().contains(&alice_prefix))
                    .unwrap_or(false)
        }),
        "both sides complete the authentication handshake"
    );

    // is_access_protected TRUE: bob's grant does not allow the domain, so
    // alice considers him ignored, as by `ignore_participant` (§9.8.7); his
    // endpoints are matched with nothing.
    let events = || {
        sink.snapshot()
            .iter()
            .map(|e| {
                format!(
                    "{} {:?}",
                    e.name,
                    e.attrs.iter().map(|a| a.value.clone()).collect::<Vec<_>>()
                )
            })
            .collect::<Vec<_>>()
    };
    let bob_handle = zerodds_dcps::instance_handle::InstanceHandle::from_guid(
        zerodds_rtps::wire_types::Guid::new(
            bob_prefix,
            zerodds_rtps::wire_types::EntityId::PARTICIPANT,
        ),
    );
    assert!(
        wait_for(Duration::from_secs(30), || alice
            .is_participant_ignored(bob_handle)),
        "alice ignores the participant whose grant does not allow the domain; events: {:?}",
        events()
    );
    assert!(
        !bob.is_participant_ignored(zerodds_dcps::instance_handle::InstanceHandle::from_guid(
            zerodds_rtps::wire_types::Guid::new(
                alice_prefix,
                zerodds_rtps::wire_types::EntityId::PARTICIPANT,
            ),
        )),
        "bob ignores nobody: his domain checks nothing"
    );

    // is_access_protected FALSE: the check has no consequence for bob.
    std::thread::sleep(Duration::from_secs(2));
    assert!(
        rt_b.discovered_participants()
            .iter()
            .any(|p| p.sender_prefix == alice_prefix),
        "bob keeps alice: a domain without join access control checks nothing"
    );

    // Nothing is matched with the ignored participant on alice's side.
    let topic = "RemoteAccessTopic";
    let _writer = rt_b
        .register_user_writer(zerodds_dcps::runtime::UserWriterConfig {
            topic_name: topic.into(),
            type_name: "RawBytes".into(),
            reliable: true,
            ..Default::default()
        })
        .expect("writer");
    let (reader, _rx) = rt_a
        .register_user_reader(zerodds_dcps::runtime::UserReaderConfig {
            topic_name: topic.into(),
            type_name: "RawBytes".into(),
            ..Default::default()
        })
        .expect("reader");
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(
        rt_a.user_reader_matched_count(reader),
        0,
        "alice matches nothing of the ignored participant"
    );
}
