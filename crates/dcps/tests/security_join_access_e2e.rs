// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 `is_access_protected` on the runtime path, live over UDP.
//!
//! §9.4.2.4 Table 33: "If is_access_protected is TRUE, then the DDS
//! middleware shall call [...] validate_remote_permissions, and
//! check_remote_participant operations on the matched and authenticated
//! remote DomainParticipant. Any failure in these calls will result in
//! failing to authorize the remote participant, which shall be removed by
//! the local participant. If is_access_protected is FALSE [...] a HandleNIL
//! return from these operations will not prevent authorization." §9.8.7:
//! such a participant "shall be considered ignored".
//!
//! Alice reads the topic. Carol and mallory write it; both authenticate
//! with identities of one CA, and the topic is open. Carol's grant allows
//! joining the domain, mallory's names another domain. With
//! `enable_join_access_control` TRUE in alice's governance mallory is
//! ignored (it stays out of the matching; the discovery keeps its lease)
//! and its writer never matches; with FALSE the permissions of the
//! remote participant are not asked and the writer matches.
//!
//! Skips (with a loud `[skip]` marker) if `openssl` is not on `PATH`.

#![cfg(all(target_os = "linux", feature = "security"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout
)]

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use zerodds_dcps::runtime::{DcpsRuntime, RuntimeConfig, UserReaderConfig, UserWriterConfig};
use zerodds_rtps::wire_types::{Guid, GuidPrefix, VendorId};
use zerodds_security_runtime::{SecurityBundle, SecurityProfile, SecurityProfileConfig};

#[path = "common/pki.rs"]
mod pki;

#[path = "common/mod.rs"]
mod common;

use pki::{must_run, openssl_available, tempdir_unique};

const TOPIC: &str = "JoinChecked";

/// The governance of a domain with an open topic; `join` is
/// `enable_join_access_control`.
fn governance(domain: i32, join: bool) -> String {
    let join = if join { "TRUE" } else { "FALSE" };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<dds>
  <domain_access_rules>
    <domain_rule>
      <domains><id>{domain}</id></domains>
      <allow_unauthenticated_participants>FALSE</allow_unauthenticated_participants>
      <enable_join_access_control>{join}</enable_join_access_control>
      <discovery_protection_kind>NONE</discovery_protection_kind>
      <liveliness_protection_kind>NONE</liveliness_protection_kind>
      <rtps_protection_kind>NONE</rtps_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>{TOPIC}</topic_expression>
          <enable_discovery_protection>FALSE</enable_discovery_protection>
          <enable_liveliness_protection>FALSE</enable_liveliness_protection>
          <enable_read_access_control>FALSE</enable_read_access_control>
          <enable_write_access_control>FALSE</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>NONE</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
"#
    )
}

/// Alice and carol may join `domain`; mallory's grant names the next one.
fn permissions(domain: i32) -> String {
    let grant = |name: &str, id: i32| {
        format!(
            r#"    <grant name="{name}">
      <subject_name>CN=zerodds-join-{name}</subject_name>
      <validity>
        <not_before>2025-01-01T00:00:00</not_before>
        <not_after>2099-01-01T00:00:00</not_after>
      </validity>
      <allow_rule>
        <domains><id>{id}</id></domains>
        <publish><topics><topic>{TOPIC}</topic></topics></publish>
        <subscribe><topics><topic>{TOPIC}</topic></topics></subscribe>
      </allow_rule>
      <default>DENY</default>
    </grant>
"#
        )
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<dds>\n  <permissions>\n{}{}{}  </permissions>\n</dds>\n",
        grant("alice", domain),
        grant("carol", domain),
        grant("mallory", domain + 1),
    )
}

fn gen_fixtures(dir: &Path, domain: i32) {
    let _g = pki::OPENSSL_LOCK.lock().unwrap();
    pki::mint_ca_and_leaves(
        dir,
        "/CN=ZeroDDS Test Join CA",
        "/CN=zerodds-join-{name}",
        &["alice", "carol", "mallory"],
    );
    let certs = dir.join("certs");
    let ca_cert = certs.join("identity_ca.pem");
    let ca_key = certs.join("identity_ca_key.pem");
    std::fs::write(dir.join("governance_join.xml"), governance(domain, true)).unwrap();
    std::fs::write(dir.join("governance_open.xml"), governance(domain, false)).unwrap();
    std::fs::write(dir.join("permissions.xml"), permissions(domain)).unwrap();
    for name in ["governance_join", "governance_open", "permissions"] {
        let mut c = Command::new("openssl");
        c.args(["smime", "-sign", "-in"])
            .arg(dir.join(format!("{name}.xml")))
            .args(["-text", "-out"])
            .arg(dir.join(format!("{name}.p7s")))
            .args(["-signer"])
            .arg(&ca_cert)
            .args(["-inkey"])
            .arg(&ca_key);
        must_run(&format!("cms-sign {name}"), c);
    }
}

struct Peer {
    rt: Arc<DcpsRuntime>,
    prefix: GuidPrefix,
    profile: Arc<SecurityProfile>,
}

/// Starts the participant `name` with the governance `governance` on the
/// shared SPDP bus `bus`.
fn start(
    dir: &Path,
    domain: i32,
    name: &str,
    governance: &str,
    seed: u8,
    bus: &RuntimeConfig,
) -> Peer {
    let certs = dir.join("certs");
    let cfg = SecurityProfileConfig {
        domain_id: domain as u32,
        identity_ca_pem: certs.join("identity_ca.pem").into(),
        identity_cert_pem: certs.join(format!("{name}_cert.pem")).into(),
        identity_key_pem: certs.join(format!("{name}_key.pem")).into(),
        permissions_ca_pem: certs.join("identity_ca.pem").into(),
        governance_p7s: dir.join(format!("{governance}.p7s")).into(),
        permissions_p7s: dir.join("permissions.p7s").into(),
        identity_key_password: None,
        identity_key_password_reading: Default::default(),
        symmetric_cipher_bits: 256,
        max_blocks_per_session: None,
        partition_matching: zerodds_security_permissions::PartitionMatching::Subset,
        plaintext_loopback: false,
        // The relay participant runs a governance of its own (reading not
        // access-controlled), so the two masks of the topic differ on purpose;
        // DDS-Security 1.2 §7.3.24 would keep the pair apart.
        compatibility_checks: false,
        kx_key_fallback: false,
        algo_name_nul: Default::default(),
        algorithm_names: Default::default(),
        subject_name_form: Default::default(),
        origin_authentication_fallback: false,
        identity_crl: None,
    };
    let mut candidate = [seed; 16];
    candidate[12..].copy_from_slice(&[0x00, 0x00, 0x01, 0xC1]);
    let profile = SecurityProfile::from_files(&cfg, candidate).expect("profile");
    let bundle = SecurityBundle::builder().security_profile(profile).build();
    let profile = bundle.security_profile().expect("profile");
    let rt_cfg = bus.clone().with_security_bundle(&bundle);
    let prefix = rt_cfg
        .security_guid_prefix
        .expect("identity-adjusted prefix");
    let rt = DcpsRuntime::start(domain, prefix, rt_cfg).expect("runtime");
    rt.enable_security_builtins_with_auth(
        VendorId::ZERODDS,
        profile.pki.clone(),
        profile.identity_handle,
    );
    Peer {
        rt,
        prefix,
        profile,
    }
}

fn writer_cfg() -> UserWriterConfig {
    UserWriterConfig {
        topic_name: TOPIC.into(),
        type_name: "RawBytes".into(),
        ..Default::default()
    }
}

fn reader_cfg() -> UserReaderConfig {
    UserReaderConfig {
        topic_name: TOPIC.into(),
        type_name: "RawBytes".into(),
        reliable: true,
        ..Default::default()
    }
}

/// Runs the scenario with alice's governance `governance`; returns whether
/// carol's and mallory's writers matched alice's reader and whether alice
/// holds mallory as a discovered participant.
fn scenario(governance: &str) -> (bool, bool, bool) {
    let lease = common::lease_domain();
    let domain = lease.id();
    let tmp = tempdir_unique("zerodds_sec_join_");
    gen_fixtures(&tmp, domain);
    let bus = common::isolated_cfg();
    // The peers run the open governance, so that they create their own
    // participants whatever their grants say.
    let alice = start(&tmp, domain, "alice", governance, 0x2a, &bus);
    let carol = start(&tmp, domain, "carol", "governance_open", 0x6c, &bus);
    let mallory = start(&tmp, domain, "mallory", "governance_open", 0x8d, &bus);
    let reader = alice
        .rt
        .register_user_reader(reader_cfg())
        .expect("alice reads")
        .0;
    let (carol_writer, mallory_writer) = (
        carol.rt.register_user_writer(writer_cfg()).expect("carol"),
        mallory
            .rt
            .register_user_writer(writer_cfg())
            .expect("mallory"),
    );
    let carol_guid = Guid::new(carol.prefix, carol_writer).to_bytes();
    let mallory_guid = Guid::new(mallory.prefix, mallory_writer).to_bytes();
    // Carol is the control: once her writer matched, the handshakes and the
    // discovery had the time mallory's needs.
    let deadline = Instant::now() + Duration::from_secs(20);
    while !alice
        .rt
        .user_reader_matched_writer_guids(reader)
        .contains(&carol_guid)
    {
        assert!(
            Instant::now() < deadline,
            "carol's writer did not match alice's reader"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    // Mallory authenticated alice as well: the handshake is not what keeps
    // the two apart.
    let deadline = Instant::now() + Duration::from_secs(10);
    while mallory
        .profile
        .gate
        .slot_for(&alice.prefix.to_bytes())
        .ok()
        .flatten()
        .is_none()
    {
        assert!(
            Instant::now() < deadline,
            "mallory did not authenticate alice"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    std::thread::sleep(Duration::from_secs(4));
    let matched = alice.rt.user_reader_matched_writer_guids(reader);
    let known = alice
        .rt
        .discovered_participants()
        .iter()
        .any(|p| p.sender_prefix == mallory.prefix);
    let result = (
        matched.contains(&carol_guid),
        matched.contains(&mallory_guid),
        known,
    );
    for peer in [&alice, &carol, &mallory] {
        peer.rt.shutdown();
    }
    result
}

/// Table 33: with `enable_join_access_control` TRUE a peer the permissions
/// refuse (`check_remote_participant`) is ignored: its writer never matches.
#[serial_test::file_serial(security_live)]
#[test]
fn a_participant_the_permissions_refuse_is_ignored_when_the_domain_is_access_protected() {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — the join test cannot run");
        return;
    }
    let (carol, mallory, _) = scenario("governance_join");
    assert!(carol, "the permitted peer matches");
    assert!(!mallory, "the refused peer's writer must not match");
}

/// Table 33: with `enable_join_access_control` FALSE the permissions of the
/// remote participant are not asked: its writer matches.
#[serial_test::file_serial(security_live)]
#[test]
fn a_participant_is_not_checked_when_the_domain_is_not_access_protected() {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — the join test cannot run");
        return;
    }
    let (carol, mallory, mallory_known) = scenario("governance_open");
    assert!(carol);
    assert!(mallory, "an unchecked peer matches");
    assert!(mallory_known);
}
