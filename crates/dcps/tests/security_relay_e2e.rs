// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
//! DDS-Security 1.2 relay-only readers on the runtime path, live over UDP.
//!
//! §9.4.2.9.11 and Table 70: a remote reader whose grant allows only
//! `<relay>` of the topic is matched, with `allow_relay_only` TRUE.
//! §9.5.1.9.5: its datawriter token carries "only the cryptographic
//! material needed to verify and decode the RTPS SubMessage but not the
//! CryptoContent". A writer whose submessages and payload are both
//! protected therefore has two keys; the relay gets the first.
//!
//! Three participants with identities from one CA. Alice publishes the
//! topic, its submessages and payload encrypted. Carol may subscribe and
//! reads the samples. Bob may only relay: alice matches his reader, and he
//! receives the DATA submessages, but not one sample, because the payload
//! key never reached him. Bob's governance does not access-control reading
//! the topic, so his own runtime lets him create the reader (Table 70
//! `check_create_datareader` asks for `<subscribe>`); alice's does, and she
//! decides with his grant.
//!
//! Before, alice refused bob's reader, and with one key per writer a relay
//! token would have opened the payload.
//!
//! Skips (with a loud `[skip]` marker, not a silent pass) if `openssl` is
//! not on `PATH`.

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

use zerodds_dcps::runtime::{
    DcpsRuntime, RuntimeConfig, UserReaderConfig, UserSample, UserWriterConfig,
};
use zerodds_rtps::wire_types::{Guid, GuidPrefix, SequenceNumber, VendorId};
use zerodds_security_runtime::{SecurityBundle, SecurityProfile, SecurityProfileConfig};

#[path = "common/pki.rs"]
mod pki;

#[path = "common/mod.rs"]
mod common;

use pki::{must_run, openssl_available, tempdir_unique};

const TOPIC: &str = "Relayed";

/// The governance of a domain whose topic `Relayed` has its submessages
/// and payload encrypted; `read_access` decides whether reading it is
/// access-controlled.
fn governance(domain: i32, read_access: bool) -> String {
    let read = if read_access { "TRUE" } else { "FALSE" };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<dds>
  <domain_access_rules>
    <domain_rule>
      <domains><id>{domain}</id></domains>
      <allow_unauthenticated_participants>FALSE</allow_unauthenticated_participants>
      <enable_join_access_control>FALSE</enable_join_access_control>
      <discovery_protection_kind>NONE</discovery_protection_kind>
      <liveliness_protection_kind>NONE</liveliness_protection_kind>
      <rtps_protection_kind>NONE</rtps_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>{TOPIC}</topic_expression>
          <enable_discovery_protection>FALSE</enable_discovery_protection>
          <enable_liveliness_protection>FALSE</enable_liveliness_protection>
          <enable_read_access_control>{read}</enable_read_access_control>
          <enable_write_access_control>TRUE</enable_write_access_control>
          <metadata_protection_kind>ENCRYPT</metadata_protection_kind>
          <data_protection_kind>ENCRYPT</data_protection_kind>
        </topic_rule>
      </topic_access_rules>
    </domain_rule>
  </domain_access_rules>
</dds>
"#
    )
}

/// Alice may publish, bob only relay, carol subscribe, trent publish and
/// relay, mallory publish.
fn permissions(domain: i32) -> String {
    let grant = |name: &str, actions: &[&str]| {
        let actions: String = actions
            .iter()
            .map(|a| format!("        <{a}><topics><topic>{TOPIC}</topic></topics></{a}>\n"))
            .collect();
        format!(
            r#"    <grant name="{name}">
      <subject_name>CN=zerodds-relay-{name}</subject_name>
      <validity>
        <not_before>2025-01-01T00:00:00</not_before>
        <not_after>2099-01-01T00:00:00</not_after>
      </validity>
      <allow_rule>
        <domains><id>{domain}</id></domains>
{actions}      </allow_rule>
      <default>DENY</default>
    </grant>
"#
        )
    };
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<dds>\n  <permissions>\n{}{}{}{}{}  </permissions>\n</dds>\n",
        grant("alice", &["publish"]),
        grant("bob", &["relay"]),
        grant("carol", &["subscribe"]),
        grant("trent", &["publish", "relay"]),
        grant("mallory", &["publish"]),
    )
}

/// The CA, the three identities, the two governance documents and the
/// permissions document, CMS-signed by the CA.
fn gen_fixtures(dir: &Path, domain: i32) {
    let _g = pki::OPENSSL_LOCK.lock().unwrap();
    pki::mint_ca_and_leaves(
        dir,
        "/CN=ZeroDDS Test Relay CA",
        "/CN=zerodds-relay-{name}",
        &["alice", "bob", "carol", "trent", "mallory"],
    );
    let certs = dir.join("certs");
    let ca_cert = certs.join("identity_ca.pem");
    let ca_key = certs.join("identity_ca_key.pem");
    std::fs::write(dir.join("governance.xml"), governance(domain, true)).unwrap();
    std::fs::write(dir.join("governance_relay.xml"), governance(domain, false)).unwrap();
    std::fs::write(dir.join("permissions.xml"), permissions(domain)).unwrap();
    for name in ["governance", "governance_relay", "permissions"] {
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

fn authenticated(a: &Peer, b: &Peer) -> bool {
    a.profile
        .gate
        .slot_for(&b.prefix.to_bytes())
        .ok()
        .flatten()
        .is_some()
        && b.profile
            .gate
            .slot_for(&a.prefix.to_bytes())
            .ok()
            .flatten()
            .is_some()
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

/// §9.4.2.9.11 / §9.5.1.9.5: the relay is matched and reads no sample, the
/// subscriber reads them.
#[serial_test::file_serial(security_live)]
#[test]
fn a_relay_only_reader_is_matched_and_cannot_read_the_payload() {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — the relay test cannot run");
        return;
    }
    let lease = common::lease_domain();
    let domain = lease.id();
    let tmp = tempdir_unique("zerodds_sec_relay_");
    gen_fixtures(&tmp, domain);
    let bus = common::isolated_cfg();
    let alice = start(&tmp, domain, "alice", "governance", 0x2a, &bus);
    let bob = start(&tmp, domain, "bob", "governance_relay", 0x4b, &bus);
    let carol = start(&tmp, domain, "carol", "governance", 0x6c, &bus);

    let deadline = Instant::now() + Duration::from_secs(20);
    while !(authenticated(&alice, &bob) && authenticated(&alice, &carol)) {
        assert!(
            Instant::now() < deadline,
            "the handshakes did not complete (alice-bob {}, alice-carol {})",
            authenticated(&alice, &bob),
            authenticated(&alice, &carol)
        );
        std::thread::sleep(Duration::from_millis(100));
    }

    let writer = alice
        .rt
        .register_user_writer(writer_cfg())
        .expect("alice writes");
    let (bob_reader, bob_rx) = bob
        .rt
        .register_user_reader(reader_cfg())
        .expect("bob's governance does not access-control reading");
    let (carol_reader, carol_rx) = carol
        .rt
        .register_user_reader(reader_cfg())
        .expect("carol reads");
    let bob_guid = Guid::new(bob.prefix, bob_reader).to_bytes();
    let carol_guid = Guid::new(carol.prefix, carol_reader).to_bytes();
    let writer_guid = Guid::new(alice.prefix, writer).to_bytes();

    let deadline = Instant::now() + Duration::from_secs(20);
    let mut carol_read = false;
    let mut n = 0u32;
    while Instant::now() < deadline && !carol_read {
        n += 1;
        alice
            .rt
            .write_user_sample(writer, format!("sample {n}").into_bytes())
            .expect("write");
        if let Ok(UserSample::Alive { payload, .. }) =
            carol_rx.recv_timeout(Duration::from_millis(300))
        {
            assert!(payload.as_ref().starts_with(b"sample "));
            carol_read = true;
        }
    }
    let matched = alice.rt.user_writer_matched_reader_guids(writer);
    assert!(carol_read, "carol may subscribe and read no sample");
    assert!(
        matched.contains(&carol_guid),
        "alice did not match carol's reader"
    );
    assert!(
        matched.contains(&bob_guid),
        "alice did not match the relay-only reader (§9.4.2.9.11): {matched:02x?}"
    );
    assert!(
        bob.rt
            .user_reader_matched_writer_guids(bob_reader)
            .contains(&writer_guid),
        "bob's reader did not match alice's writer"
    );
    assert_eq!(
        alice.rt.relay_only_readers(),
        vec![bob_guid],
        "alice must hold bob's reader, and only it, as relay-only"
    );

    // Alice keeps writing; carol reads on, bob never.
    let until = Instant::now() + Duration::from_secs(3);
    let mut carol_more = 0u32;
    while Instant::now() < until {
        n += 1;
        alice
            .rt
            .write_user_sample(writer, format!("sample {n}").into_bytes())
            .expect("write");
        if carol_rx.recv_timeout(Duration::from_millis(200)).is_ok() {
            carol_more += 1;
        }
        if let Ok(sample) = bob_rx.try_recv() {
            panic!("the relay-only reader read a sample (§9.5.1.9.5): {sample:?}");
        }
    }
    assert!(carol_more > 0, "carol read nothing after the first sample");
    assert!(
        bob_rx.recv_timeout(Duration::from_millis(500)).is_err(),
        "the relay-only reader read a sample (§9.5.1.9.5)"
    );
    alice.rt.shutdown();
    bob.rt.shutdown();
    carol.rt.shutdown();
}

/// DDS-Security 1.2 §7.1.1.4 end to end: Trent stores and forwards what he
/// cannot read, Carol reads it with Alice's key and takes it as Alice's, and
/// a writer without `<relay>` in its grant is not taken as forwarder.
///
/// Alice keeps one sample (KEEP_LAST 1, TRANSIENT_LOCAL) and writes five
/// before Carol has a reader. Trent's relay reader receives them with the
/// payload still encrypted; his TRANSIENT_LOCAL writer forwards each
/// unchanged on Alice's behalf (DDSI-RTPS 2.5 §8.7.9). Carol's late reader
/// then gets samples 1 to 4 from Trent only and sample 5 from both, and
/// takes each once, as Alice's, with Alice's sequence numbers. Mallory may
/// publish but not relay; a sample she sends on Alice's behalf reaches
/// Carol as Mallory's own.
#[serial_test::file_serial(security_live)]
#[test]
fn a_relay_forwards_what_it_cannot_read_and_the_reader_takes_it_as_the_originals() {
    if !openssl_available() {
        println!("[skip] openssl not on PATH — the relay test cannot run");
        return;
    }
    let lease = common::lease_domain();
    let domain = lease.id();
    let tmp = tempdir_unique("zerodds_sec_relay_fwd_");
    gen_fixtures(&tmp, domain);
    let bus = common::isolated_cfg();
    let alice = start(&tmp, domain, "alice", "governance", 0x2a, &bus);
    let trent = start(&tmp, domain, "trent", "governance", 0x7e, &bus);
    let carol = start(&tmp, domain, "carol", "governance", 0x6c, &bus);
    let mallory = start(&tmp, domain, "mallory", "governance", 0x3d, &bus);
    let deadline = Instant::now() + Duration::from_secs(30);
    let pairs = [
        (&alice, &trent),
        (&alice, &carol),
        (&trent, &carol),
        (&mallory, &carol),
    ];
    while !pairs.iter().all(|(a, b)| authenticated(a, b)) {
        assert!(Instant::now() < deadline, "the handshakes did not complete");
        std::thread::sleep(Duration::from_millis(100));
    }

    let alice_writer = alice
        .rt
        .register_user_writer(UserWriterConfig {
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            ..writer_cfg()
        })
        .expect("alice writes");
    alice
        .rt
        .set_user_writer_history_depth(alice_writer, 1)
        .expect("KEEP_LAST 1");
    let (relay_reader, relay_rx) = trent
        .rt
        .register_user_reader(UserReaderConfig {
            relay: true,
            ..reader_cfg()
        })
        .expect("trent may relay");
    let trent_writer = trent
        .rt
        .register_user_writer(UserWriterConfig {
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            ..writer_cfg()
        })
        .expect("trent may publish");
    trent
        .rt
        .set_user_writer_history_depth(trent_writer, 100)
        .expect("trent keeps the history");
    let alice_guid = Guid::new(alice.prefix, alice_writer);
    let relay_guid = Guid::new(trent.prefix, relay_reader).to_bytes();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !alice
        .rt
        .user_writer_matched_reader_guids(alice_writer)
        .contains(&relay_guid)
    {
        assert!(
            Instant::now() < deadline,
            "alice did not match trent's relay reader"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    assert_eq!(alice.rt.relay_only_readers(), vec![relay_guid]);

    // Trent forwards what his relay reader receives, unopened. Alice keeps
    // one sample, so she writes the next one only once Trent has the last:
    // a sample that reached Trent before his key would be repaired only
    // while it is still the one Alice keeps.
    let mut forwarded = Vec::new();
    for i in 1..=5u32 {
        alice
            .rt
            .write_user_sample(alice_writer, format!("early {i}").into_bytes())
            .expect("write");
        let deadline = Instant::now() + Duration::from_secs(20);
        let (payload, key_hash, source_timestamp, writer_guid, source_sequence_number) = loop {
            assert!(
                Instant::now() < deadline,
                "trent's relay reader did not receive sample {i}"
            );
            if let Ok(UserSample::Alive {
                payload,
                crypto_content,
                key_hash,
                source_timestamp,
                writer_guid,
                source_sequence_number,
                ..
            }) = relay_rx.recv_timeout(Duration::from_millis(300))
            {
                assert!(crypto_content, "the relay reader opened the payload");
                break (
                    payload,
                    key_hash,
                    source_timestamp,
                    writer_guid,
                    source_sequence_number,
                );
            }
        };
        assert_eq!(writer_guid, alice_guid.to_bytes());
        assert!(
            !payload.as_ref().windows(5).any(|w| w == b"early"),
            "the relay sees the plaintext"
        );
        trent
            .rt
            .write_user_sample_relayed(
                trent_writer,
                payload.as_ref(),
                true,
                key_hash,
                source_timestamp,
                alice_guid,
                SequenceNumber(source_sequence_number),
            )
            .expect("forward");
        forwarded.push(source_sequence_number);
    }

    // Mallory claims a change of Alice.
    let mallory_writer = mallory
        .rt
        .register_user_writer(UserWriterConfig {
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            ..writer_cfg()
        })
        .expect("mallory may publish");
    mallory
        .rt
        .write_user_sample_on_behalf(
            mallory_writer,
            b"forged",
            None,
            None,
            alice_guid,
            SequenceNumber(forwarded[0]),
        )
        .expect("write");

    let (_carol_reader, carol_rx) = carol
        .rt
        .register_user_reader(UserReaderConfig {
            durability: zerodds_qos::DurabilityKind::TransientLocal,
            ..reader_cfg()
        })
        .expect("carol reads");
    let mallory_guid = Guid::new(mallory.prefix, mallory_writer).to_bytes();
    let mut from_alice: Vec<(i64, Vec<u8>)> = Vec::new();
    let mut from_mallory: Vec<Vec<u8>> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(20);
    while (from_alice.len() < 5 || from_mallory.is_empty()) && Instant::now() < deadline {
        let Ok(UserSample::Alive {
            payload,
            writer_guid,
            source_sequence_number,
            crypto_content,
            ..
        }) = carol_rx.recv_timeout(Duration::from_millis(300))
        else {
            continue;
        };
        assert!(!crypto_content, "carol is not a relay reader");
        if writer_guid == alice_guid.to_bytes() {
            from_alice.push((source_sequence_number, payload.as_ref().to_vec()));
        } else if writer_guid == mallory_guid {
            from_mallory.push(payload.as_ref().to_vec());
        } else {
            panic!("a sample of an unexpected writer {writer_guid:02x?}");
        }
    }
    // Anything still arriving would be a duplicate.
    if let Ok(UserSample::Alive {
        writer_guid,
        source_sequence_number,
        ..
    }) = carol_rx.recv_timeout(Duration::from_millis(1500))
    {
        panic!("a second copy of {writer_guid:02x?} {source_sequence_number}");
    }
    from_alice.sort();
    let expected: Vec<(i64, Vec<u8>)> = forwarded
        .iter()
        .zip(1..=5u32)
        .map(|(sn, i)| (*sn, format!("early {i}").into_bytes()))
        .collect();
    assert_eq!(
        from_alice, expected,
        "carol takes every sample once, as alice's, opened with her key"
    );
    assert_eq!(
        from_mallory,
        vec![b"forged".to_vec()],
        "a writer without <relay> is not taken as forwarder"
    );
    alice.rt.shutdown();
    trent.rt.shutdown();
    carol.rt.shutdown();
    mallory.rt.shutdown();
}
