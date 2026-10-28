// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 §9.8.1 (Figure 22) — what has to hold before a
//! DomainParticipant exists, and what its first announcement carries.
//!
//! Steps 1 to 3: the middleware validates the identity
//! (`validate_local_identity`), validates the permissions
//! (`validate_local_permissions`) and checks that the application may join
//! the domain (`check_create_participant`). "If the identity is not
//! successfully validated, the DDS middleware shall not create the
//! DomainParticipant and the create_participant operation shall return NIL
//! and set the return code to NOT_ALLOWED_BY_SECURITY." The same sentence
//! stands under steps 2 and 3.
//!
//! The positive control creates a participant from the same fixtures with
//! nothing wrong, so that the three failures are the faults and not the
//! fixtures. The fixtures need the `openssl` command; a host without it
//! prints a `[skip]` line and the test does not count as run.

#![cfg(feature = "security")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout
)]

use std::path::{Path, PathBuf};
use std::process::Command;

use zerodds_dcps::error::{DdsError, return_code};
use zerodds_dcps::qos::PropertyQosPolicy;
use zerodds_dcps::{DomainParticipantFactory, DomainParticipantQos};
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
      <subject_name>CN=zerodds-sequence-alice</subject_name>
      <validity>
        <not_before>2025-01-01T00:00:00</not_before>
        <not_after>2099-01-01T00:00:00</not_after>
      </validity>
      <allow_rule>
        <domains><id>@GRANTED@</id></domains>
        <publish><topics><topic>*</topic></topics></publish>
        <subscribe><topics><topic>*</topic></topics></subscribe>
      </allow_rule>
      <default>DENY</default>
    </grant>
  </permissions>
</dds>
"#;

/// A CA with the identity `alice`, in `dir/certs`.
fn mint(dir: &Path, ca_subject: &str) {
    let _g = pki::OPENSSL_LOCK.lock().unwrap();
    pki::mint_ca_and_leaves(dir, ca_subject, "/CN=zerodds-sequence-{name}", &["alice"]);
}

/// Writes governance and permissions for `domain` and signs both with the CA
/// of `signer`: `join` is `enable_join_access_control`, `granted` the domain
/// the permissions grant.
fn sign_documents(dir: &Path, signer: &Path, domain: i32, join: bool, granted: i32) {
    let _g = pki::OPENSSL_LOCK.lock().unwrap();
    for (name, template) in [("governance", GOVERNANCE), ("permissions", PERMISSIONS)] {
        let xml = dir.join(format!("{name}.xml"));
        std::fs::write(
            &xml,
            template
                .replace("@DOMAIN@", &domain.to_string())
                .replace("@JOIN@", if join { "TRUE" } else { "FALSE" })
                .replace("@GRANTED@", &granted.to_string()),
        )
        .unwrap();
        let mut c = Command::new("openssl");
        c.args(["smime", "-sign", "-in"])
            .arg(&xml)
            .args(["-text", "-out"])
            .arg(dir.join(format!("{name}.p7s")))
            .args(["-signer"])
            .arg(signer.join("certs/identity_ca.pem"))
            .args(["-inkey"])
            .arg(signer.join("certs/identity_ca_key.pem"));
        must_run(&format!("cms-sign {name}"), c);
    }
}

fn uri(p: PathBuf) -> String {
    format!("file://{}", p.display())
}

/// The six properties of §9.3.1 / §9.4.1.2 for `alice`; `identity` is the
/// directory whose CA minted her certificate, `dir` holds the CA the
/// permissions are checked against and the signed documents.
fn properties(dir: &Path, identity: &Path) -> PropertyQosPolicy {
    PropertyQosPolicy::new()
        .with(PROP_IDENTITY_CA, uri(dir.join("certs/identity_ca.pem")))
        .with(
            PROP_IDENTITY_CERT,
            uri(identity.join("certs/alice_cert.pem")),
        )
        .with(PROP_IDENTITY_KEY, uri(identity.join("certs/alice_key.pem")))
        .with(PROP_PERMISSIONS_CA, uri(dir.join("certs/identity_ca.pem")))
        .with(PROP_GOVERNANCE, uri(dir.join("governance.p7s")))
        .with(PROP_PERMISSIONS, uri(dir.join("permissions.p7s")))
}

fn create(
    domain: i32,
    property: PropertyQosPolicy,
) -> Result<zerodds_dcps::DomainParticipant, DdsError> {
    DomainParticipantFactory::instance().create_participant_with_config(
        domain,
        DomainParticipantQos {
            property,
            ..DomainParticipantQos::default()
        },
        common::isolated_cfg(),
    )
}

/// The create failed, with NOT_ALLOWED_BY_SECURITY, and left no
/// DomainParticipant behind.
fn assert_not_created(
    result: Result<zerodds_dcps::DomainParticipant, DdsError>,
    domain: i32,
    step: &str,
) -> DdsError {
    let err = match result {
        Err(e) => e,
        Ok(p) => {
            let _ = DomainParticipantFactory::instance().delete_participant(&p);
            panic!("{step}: the DomainParticipant was created");
        }
    };
    assert_eq!(
        err.as_return_code(),
        return_code::NOT_ALLOWED_BY_SECURITY,
        "{step}: {err}"
    );
    assert!(
        DomainParticipantFactory::instance()
            .lookup_participant(domain)
            .is_none(),
        "{step}: create_participant returns NIL, no participant exists"
    );
    err
}

/// Control: the fixtures, unharmed, give a participant. Without it the three
/// failures below could be the fixtures.
#[serial_test::file_serial(security_live)]
#[test]
fn the_fixtures_create_a_participant_when_nothing_is_wrong() {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — the participant creation sequence cannot run");
        return;
    }
    let dir = tempdir_unique("zerodds_sec_sequence_ok_");
    mint(&dir, "/CN=ZeroDDS Sequence CA");
    let lease = common::lease_domain();
    sign_documents(&dir, &dir, lease.id(), true, lease.id());
    let p = create(lease.id(), properties(&dir, &dir)).expect("a valid configuration");
    let _ = DomainParticipantFactory::instance().delete_participant(&p);
}

/// §9.8.1 step 1: an identity certificate the identity CA did not issue is
/// not validated by `validate_local_identity`; the create returns
/// NOT_ALLOWED_BY_SECURITY and no participant exists.
#[serial_test::file_serial(security_live)]
#[test]
fn step_1_an_identity_that_is_not_validated_refuses_the_participant() {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — the participant creation sequence cannot run");
        return;
    }
    let dir = tempdir_unique("zerodds_sec_sequence_id_");
    let foreign = tempdir_unique("zerodds_sec_sequence_id_foreign_");
    mint(&dir, "/CN=ZeroDDS Sequence CA");
    mint(&foreign, "/CN=Another CA");
    let lease = common::lease_domain();
    sign_documents(&dir, &dir, lease.id(), true, lease.id());
    // The certificate and the key of alice come from the other CA; the
    // identity CA of the property is the first one.
    let err = assert_not_created(
        create(lease.id(), properties(&dir, &foreign)),
        lease.id(),
        "step 1",
    );
    assert!(matches!(err, DdsError::SecurityConfiguration { .. }));
}

/// §9.8.1 step 2: a permissions document whose signature the permissions CA
/// does not verify is not validated by `validate_local_permissions`.
#[serial_test::file_serial(security_live)]
#[test]
fn step_2_permissions_that_are_not_validated_refuse_the_participant() {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — the participant creation sequence cannot run");
        return;
    }
    let dir = tempdir_unique("zerodds_sec_sequence_perm_");
    let foreign = tempdir_unique("zerodds_sec_sequence_perm_foreign_");
    mint(&dir, "/CN=ZeroDDS Sequence CA");
    mint(&foreign, "/CN=Another CA");
    let lease = common::lease_domain();
    sign_documents(&dir, &dir, lease.id(), true, lease.id());
    // Permissions signed by the other CA replace the valid ones.
    let rogue = tempdir_unique("zerodds_sec_sequence_perm_rogue_");
    sign_documents(&rogue, &foreign, lease.id(), true, lease.id());
    std::fs::copy(rogue.join("permissions.p7s"), dir.join("permissions.p7s")).unwrap();
    let err = assert_not_created(
        create(lease.id(), properties(&dir, &dir)),
        lease.id(),
        "step 2",
    );
    assert!(matches!(err, DdsError::SecurityConfiguration { .. }));
}

/// §9.8.1 step 3: with `enable_join_access_control` the permissions must
/// grant the domain; a grant for another domain fails
/// `check_create_participant`.
#[serial_test::file_serial(security_live)]
#[test]
fn step_3_a_domain_the_permissions_do_not_grant_refuses_the_participant() {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — the participant creation sequence cannot run");
        return;
    }
    let dir = tempdir_unique("zerodds_sec_sequence_join_");
    mint(&dir, "/CN=ZeroDDS Sequence CA");
    let lease = common::lease_domain();
    sign_documents(&dir, &dir, lease.id(), true, lease.id() + 1);
    let err = assert_not_created(
        create(lease.id(), properties(&dir, &dir)),
        lease.id(),
        "step 3",
    );
    // §9.8.1 step 3: "the create_participant operation shall return NIL and
    // set the return code to NOT_ALLOWED_BY_SECURITY".
    assert!(
        matches!(
            err,
            DdsError::NotAllowedBySecurity {
                what: "domain participant"
            }
        ),
        "{err}"
    );
}
