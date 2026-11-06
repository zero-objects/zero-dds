// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! DDS-Security 1.2 end to end: four participants in processes of their
//! own, configured through the standard `dds.sec.*` properties of their
//! `PropertyQosPolicy` alone (DDS-Security 1.2 §10.3.1 Table 50, §10.4.1
//! Table 63), with key material, governance and permissions made and
//! signed by the `openssl` CLI.
//!
//! * `alice` writes two topics: `SecretReadings`, which the governance
//!   protects with `data_protection_kind` ENCRYPT and read/write access
//!   control (§10.4.1.2.6.4–7), and `OpenReadings`, which it leaves in plain text.
//! * `bob` may subscribe to both and receives every sample of both, in
//!   order (DDS 1.4 §2.2.3.14 RELIABLE and §2.2.3.18
//!   KEEP_ALL).
//! * `carol` has an identity from the same CA, but her permissions grant
//!   another topic only: creating her reader of `SecretReadings` is
//!   refused (§9.4.2.9.6 `check_create_topic`, §9.4.2.9.5
//!   `check_create_datareader`).
//! * `mallory` has an identity from a CA the others do not trust: no
//!   participant authenticates her (§9.3.2.11.3 `validate_remote_identity`,
//!   with `allow_unauthenticated_participants` false, §10.4.1.2.5.2), so her
//!   reader matches no writer and receives nothing. Her identity fails at
//!   the first step of authentication, before any handshake message; bob
//!   needs the whole handshake and every sample acknowledged first, and
//!   the check runs after that.
//!
//! The wire half of the oracle is the kernel's record of what alice's
//! process sent: `strace` logs the bytes of every `sendto`/`sendmsg`/
//! `sendmmsg` of her process and its threads. The plain-text marker of
//! `OpenReadings` appears in it (the record sees payloads), the marker of
//! `SecretReadings` does not (§10.4.1.2.6.7, §9.5.1.10.1
//! `encode_serialized_payload`: the payload is AES-GCM
//! ciphertext).

#![cfg(target_os = "linux")]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout,
    clippy::print_stderr,
    missing_docs
)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use zerodds_dcps::qos::PropertyQosPolicy;
use zerodds_dcps::qos::{HistoryKind, HistoryQosPolicy, ReliabilityKind, ReliabilityQosPolicy};
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_dcps::{
    DataReaderQos, DataWriterQos, DomainParticipantFactory, DomainParticipantQos, PublisherQos,
    RawBytes, SubscriberQos, TopicQos,
};
use zerodds_integration::{Peer, Scratch, event, fields, lease_domain, role, role_command, tools};

const SAMPLES: u32 = 50;
const STEP: Duration = Duration::from_secs(15);
const SECRET: &str = "SecretReadings";
const OPEN: &str = "OpenReadings";
const SECRET_MARKER: &str = "SECRET-MARKER-";
const OPEN_MARKER: &str = "OPEN-MARKER-";

fn run(label: &str, mut command: Command) {
    let out = command
        .output()
        .unwrap_or_else(|e| panic!("openssl {label}: {e}"));
    assert!(
        out.status.success(),
        "openssl {label}: {}\n{}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
}

fn openssl() -> Command {
    Command::new(tools::require("openssl", &[]))
}

/// A CA (ECDSA P-256, self-signed) in `dir/<ca>_ca.pem` and one leaf per
/// name (`dir/<name>_cert.pem`, PKCS#8 key `dir/<name>_key.pem`).
fn mint(dir: &Path, ca: &str, names: &[&str]) {
    let ca_key = dir.join(format!("{ca}_ca_key.pem"));
    let ca_cert = dir.join(format!("{ca}_ca.pem"));
    let mut c = openssl();
    c.args([
        "ecparam",
        "-name",
        "prime256v1",
        "-genkey",
        "-noout",
        "-out",
    ])
    .arg(&ca_key);
    run("ca key", c);
    let mut c = openssl();
    c.args(["req", "-x509", "-new", "-nodes", "-key"])
        .arg(&ca_key)
        .args([
            "-addext",
            "basicConstraints=critical,CA:TRUE",
            "-addext",
            "subjectKeyIdentifier=hash",
            "-days",
            "2",
            "-subj",
            &format!("/CN=integration {ca} CA"),
            "-out",
        ])
        .arg(&ca_cert);
    run("ca cert", c);
    let ext = dir.join("leaf.cnf");
    std::fs::write(
        &ext,
        "subjectKeyIdentifier=hash\nauthorityKeyIdentifier=keyid\n",
    )
    .unwrap();
    for name in names {
        let key = dir.join(format!("{name}_key.pem"));
        let csr = dir.join(format!("{name}.csr"));
        let cert = dir.join(format!("{name}_cert.pem"));
        let mut c = openssl();
        c.args([
            "genpkey",
            "-algorithm",
            "EC",
            "-pkeyopt",
            "ec_paramgen_curve:P-256",
            "-out",
        ])
        .arg(&key);
        run("leaf key", c);
        let mut c = openssl();
        c.args(["req", "-new", "-key"])
            .arg(&key)
            .args(["-subj", &format!("/CN=integration-{name}"), "-out"])
            .arg(&csr);
        run("leaf csr", c);
        let mut c = openssl();
        c.args(["x509", "-req", "-in"])
            .arg(&csr)
            .arg("-CA")
            .arg(&ca_cert)
            .arg("-CAkey")
            .arg(&ca_key)
            .args(["-CAcreateserial", "-days", "2", "-extfile"])
            .arg(&ext)
            .arg("-out")
            .arg(&cert);
        run("leaf cert", c);
    }
}

/// S/MIME-signs `xml` with the CA `ca` into `<xml>.p7s` (§10.4.1.2.3,
/// §10.4.1.5.1).
fn sign(dir: &Path, ca: &str, name: &str, xml: &str) -> PathBuf {
    let plain = dir.join(format!("{name}.xml"));
    std::fs::write(&plain, xml).unwrap();
    let signed = dir.join(format!("{name}.p7s"));
    let mut c = openssl();
    c.args(["smime", "-sign", "-text", "-in"])
        .arg(&plain)
        .arg("-out")
        .arg(&signed)
        .arg("-signer")
        .arg(dir.join(format!("{ca}_ca.pem")))
        .arg("-inkey")
        .arg(dir.join(format!("{ca}_ca_key.pem")));
    run(name, c);
    signed
}

/// The governance document, schema of DDS-Security 1.2 §10.4.1.2.3: no
/// unauthenticated participants, `Secret*` encrypted and access-controlled,
/// everything else open.
fn governance(domain: i32) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<dds xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <domain_access_rules>
    <domain_rule>
      <domains><id>{domain}</id></domains>
      <allow_unauthenticated_participants>false</allow_unauthenticated_participants>
      <enable_join_access_control>true</enable_join_access_control>
      <discovery_protection_kind>NONE</discovery_protection_kind>
      <liveliness_protection_kind>NONE</liveliness_protection_kind>
      <rtps_protection_kind>NONE</rtps_protection_kind>
      <topic_access_rules>
        <topic_rule>
          <topic_expression>Secret*</topic_expression>
          <enable_discovery_protection>false</enable_discovery_protection>
          <enable_liveliness_protection>false</enable_liveliness_protection>
          <enable_read_access_control>true</enable_read_access_control>
          <enable_write_access_control>true</enable_write_access_control>
          <metadata_protection_kind>NONE</metadata_protection_kind>
          <data_protection_kind>ENCRYPT</data_protection_kind>
        </topic_rule>
        <topic_rule>
          <topic_expression>*</topic_expression>
          <enable_discovery_protection>false</enable_discovery_protection>
          <enable_liveliness_protection>false</enable_liveliness_protection>
          <enable_read_access_control>false</enable_read_access_control>
          <enable_write_access_control>false</enable_write_access_control>
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

/// One permissions grant (DDS-Security 1.2 §10.4.1.5.3).
fn grant(name: &str, domain: i32, publish: &str, subscribe: &str) -> String {
    let topics = |list: &str| -> String {
        list.split_whitespace()
            .map(|t| format!("<topic>{t}</topic>"))
            .collect()
    };
    let mut rule = String::new();
    if !publish.is_empty() {
        rule.push_str(&format!(
            "<publish><topics>{}</topics></publish>",
            topics(publish)
        ));
    }
    if !subscribe.is_empty() {
        rule.push_str(&format!(
            "<subscribe><topics>{}</topics></subscribe>",
            topics(subscribe)
        ));
    }
    format!(
        r#"<grant name="{name}">
      <subject_name>CN=integration-{name}</subject_name>
      <validity><not_before>2025-01-01T00:00:00</not_before><not_after>2099-01-01T00:00:00</not_after></validity>
      <allow_rule><domains><id>{domain}</id></domains>{rule}</allow_rule>
      <default>DENY</default>
    </grant>"#
    )
}

fn permissions(grants: &[String]) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<dds><permissions>{}</permissions></dds>\n",
        grants.concat()
    )
}

/// Everything the four participants need, under `dir`.
fn fixtures(dir: &Path, domain: i32) {
    mint(dir, "trusted", &["alice", "bob", "carol"]);
    mint(dir, "foreign", &["mallory"]);
    let gov = governance(domain);
    sign(dir, "trusted", "governance", &gov);
    sign(dir, "foreign", "governance_foreign", &gov);
    // Forty grants of participants that never join make the signed document
    // larger than 8 KiB. Each participant sends it whole in its handshake
    // (`c.perm`, §10.3.2.5.1), in a binary property whose length the IDL
    // does not bound (§7.3.3.1); a decoder that capped it at 8 KiB refused
    // the handshake.
    let mut grants = vec![
        grant("alice", domain, "Secret* Open*", ""),
        grant("bob", domain, "", "Secret* Open*"),
        grant("carol", domain, "", "Other*"),
    ];
    grants.extend((0..40).map(|i| grant(&format!("absent-{i:02}"), domain, "Other*", "Other*")));
    let signed = sign(dir, "trusted", "permissions", &permissions(&grants));
    let size = std::fs::metadata(&signed).unwrap().len();
    assert!(size > 8 * 1024, "the signed permissions have {size} octets");
    sign(
        dir,
        "foreign",
        "permissions_foreign",
        &permissions(&[grant("mallory", domain, "*", "*")]),
    );
}

/// The six properties of §10.3.1 Table 50 and §10.4.1 Table 63 for `name`,
/// as `file:` URIs.
fn properties(dir: &Path, name: &str) -> PropertyQosPolicy {
    let (ca, suffix) = if name == "mallory" {
        ("foreign", "_foreign")
    } else {
        ("trusted", "")
    };
    let uri = |p: PathBuf| format!("file://{}", p.display());
    PropertyQosPolicy::new()
        .with(
            "dds.sec.auth.identity_ca",
            uri(dir.join(format!("{ca}_ca.pem"))),
        )
        .with(
            "dds.sec.auth.identity_certificate",
            uri(dir.join(format!("{name}_cert.pem"))),
        )
        .with(
            "dds.sec.auth.private_key",
            uri(dir.join(format!("{name}_key.pem"))),
        )
        .with(
            "dds.sec.access.permissions_ca",
            uri(dir.join(format!("{ca}_ca.pem"))),
        )
        .with(
            "dds.sec.access.governance",
            uri(dir.join(format!("governance{suffix}.p7s"))),
        )
        .with(
            "dds.sec.access.permissions",
            uri(dir.join(format!("permissions{suffix}.p7s"))),
        )
}

fn reliable_keep_all() -> (DataWriterQos, DataReaderQos) {
    let reliability = ReliabilityQosPolicy {
        kind: ReliabilityKind::Reliable,
        ..ReliabilityQosPolicy::default()
    };
    let history = HistoryQosPolicy {
        kind: HistoryKind::KeepAll,
        depth: 1,
    };
    let w = DataWriterQos {
        reliability,
        history,
        ..DataWriterQos::default()
    };
    let r = DataReaderQos {
        reliability,
        history,
        ..DataReaderQos::default()
    };
    (w, r)
}

fn participant(name: &str) -> zerodds_dcps::DomainParticipant {
    let dir = PathBuf::from(std::env::var("IT_DIR").unwrap());
    let domain: i32 = std::env::var("IT_DOMAIN").unwrap().parse().unwrap();
    let cfg = RuntimeConfig {
        tick_period: Duration::from_millis(20),
        spdp_period: Duration::from_millis(100),
        ..RuntimeConfig::default()
    };
    DomainParticipantFactory::instance()
        .create_participant_with_config(
            domain,
            DomainParticipantQos {
                property: properties(&dir, name),
                ..DomainParticipantQos::default()
            },
            cfg,
        )
        .unwrap_or_else(|e| panic!("{name}: participant: {e:?}"))
}

fn alice() {
    let p = participant("alice");
    let (wq, _) = reliable_keep_all();
    let publisher = p.create_publisher(PublisherQos::default());
    let writer = |topic: &str| {
        let t = p
            .create_topic::<RawBytes>(topic, TopicQos::default())
            .unwrap();
        publisher
            .create_datawriter::<RawBytes>(&t, wq.clone())
            .unwrap_or_else(|e| panic!("alice: writer {topic}: {e:?}"))
    };
    let (secret, open) = (writer(SECRET), writer(OPEN));
    secret
        .wait_for_matched_subscription(1, STEP)
        .expect("secret matched");
    open.wait_for_matched_subscription(1, STEP)
        .expect("open matched");
    event("matched");
    for seq in 0..SAMPLES {
        secret
            .write(&RawBytes::new(
                format!("{SECRET_MARKER}{seq:04}").into_bytes(),
            ))
            .unwrap();
        open.write(&RawBytes::new(
            format!("{OPEN_MARKER}{seq:04}").into_bytes(),
        ))
        .unwrap();
    }
    secret.wait_for_acknowledgments(STEP).expect("secret acked");
    open.wait_for_acknowledgments(STEP).expect("open acked");
    event(format!(
        "acknowledged secret_matched={} open_matched={}",
        secret
            .get_publication_matched_status()
            .unwrap()
            .current_count,
        open.get_publication_matched_status().unwrap().current_count
    ));
    zerodds_integration::wait_for_release();
}

fn bob() {
    let p = participant("bob");
    let (_, rq) = reliable_keep_all();
    let subscriber = p.create_subscriber(SubscriberQos::default());
    let reader = |topic: &str| {
        let t = p
            .create_topic::<RawBytes>(topic, TopicQos::default())
            .unwrap();
        subscriber
            .create_datareader::<RawBytes>(&t, rq.clone())
            .unwrap_or_else(|e| panic!("bob: reader {topic}: {e:?}"))
    };
    let readers = [(SECRET, reader(SECRET)), (OPEN, reader(OPEN))];
    for (topic, r) in &readers {
        r.wait_for_matched_publication(1, STEP)
            .unwrap_or_else(|e| panic!("bob: {topic} matched: {e:?}"));
    }
    event("matched");
    let deadline = std::time::Instant::now() + STEP;
    for (topic, r) in &readers {
        let mut got = 0;
        while got < SAMPLES {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if r.wait_for_data(left).is_err() {
                event(format!("incomplete topic={topic} got={got}"));
                break;
            }
            for s in r.take_with_info().unwrap() {
                if s.info.valid_data {
                    got += 1;
                    event(format!(
                        "sample topic={topic} text={}",
                        String::from_utf8_lossy(&s.data.data)
                    ));
                }
            }
        }
    }
    event("done");
    zerodds_integration::wait_for_release();
}

fn carol() {
    let p = participant("carol");
    let (_, rq) = reliable_keep_all();
    // §9.4.2.9.6 `check_create_topic` and §9.4.2.9.5 `check_create_datareader`:
    // either refusal keeps carol from reading the topic.
    match p.create_topic::<RawBytes>(SECRET, TopicQos::default()) {
        Err(e) => event(format!(
            "refused at=topic error={}",
            format!("{e:?}").replace(' ', "_")
        )),
        Ok(t) => match p
            .create_subscriber(SubscriberQos::default())
            .create_datareader::<RawBytes>(&t, rq)
        {
            Ok(_) => event("created"),
            Err(e) => event(format!(
                "refused at=reader error={}",
                format!("{e:?}").replace(' ', "_")
            )),
        },
    }
    zerodds_integration::wait_for_release();
}

fn mallory() {
    let p = participant("mallory");
    let (_, rq) = reliable_keep_all();
    let t = p
        .create_topic::<RawBytes>(SECRET, TopicQos::default())
        .unwrap();
    let reader = p
        .create_subscriber(SubscriberQos::default())
        .create_datareader::<RawBytes>(&t, rq)
        .expect("mallory's own reader");
    event("ready");
    zerodds_integration::wait_for_release();
    let received = reader.take().map(|v| v.len()).unwrap_or(0);
    event(format!(
        "final matched={} received={received}",
        reader
            .get_subscription_matched_status()
            .unwrap()
            .total_count
    ));
}

fn bytes_escaped(text: &str) -> String {
    text.bytes().map(|b| format!("\\x{b:02x}")).collect()
}

fn scenario() {
    let lease = lease_domain();
    let domain = lease.id();
    let scratch = Scratch::new("security");
    let dir = scratch.path().to_owned();
    fixtures(&dir, domain);
    let start = |name: &str, traced: Option<&Path>| {
        let mut command = match traced {
            None => role_command("security_end_to_end", name),
            Some(trace) => {
                let inner = role_command("security_end_to_end", name);
                let mut c = Command::new(tools::require("strace", &[]));
                c.args(["-f", "-qq", "-xx", "-s", "65536", "-e"])
                    .arg("trace=sendto,sendmsg,sendmmsg")
                    .arg("-o")
                    .arg(trace)
                    .arg(inner.get_program())
                    .args(inner.get_args());
                for (k, v) in inner.get_envs() {
                    if let Some(v) = v {
                        c.env(k, v);
                    }
                }
                c
            }
        };
        command
            .env("IT_DIR", &dir)
            .env("IT_DOMAIN", domain.to_string());
        Peer::spawn(name, command)
    };
    let mut mallory = start("mallory", None);
    mallory.wait_for_event("ready", STEP);
    let mut carol = start("carol", None);
    let mut bob = start("bob", None);
    let trace = dir.join("alice.strace");
    let mut alice = start("alice", Some(&trace));

    let refusal = carol.wait_for("carol's reader result", STEP, |l| {
        l.text.starts_with("EVENT refused") || l.text.starts_with("EVENT created")
    });
    assert!(
        refusal.text.starts_with("EVENT refused"),
        "carol's permissions grant `Other*` only; her topic or her reader of {SECRET} \
         must be refused (DDS-Security 1.2 §9.4.2.9.6, §9.4.2.9.5), got: {}",
        refusal.text
    );
    alice.wait_for_event("matched", STEP);
    bob.wait_for_event("matched", STEP);
    let acked = fields(&alice.wait_for_event("acknowledged", STEP));
    bob.wait_for_event("done", STEP);
    assert_eq!(
        (
            acked["secret_matched"].as_str(),
            acked["open_matched"].as_str()
        ),
        ("1", "1"),
        "alice's writers match bob alone: carol has no reader, mallory is not authenticated"
    );

    let bob_lines: Vec<String> = bob.lines().iter().map(|l| l.text.clone()).collect();
    assert!(alice.release(STEP).success(), "alice failed");
    assert!(bob.release(STEP).success(), "bob failed");
    assert!(carol.release(STEP).success(), "carol failed");
    mallory.close_stdin();
    let last = fields(&mallory.wait_for_event("final", STEP));
    assert_eq!(
        (last["matched"].as_str(), last["received"].as_str()),
        ("0", "0"),
        "mallory's identity is not trusted (DDS-Security 1.2 §9.3.2.11.3, §10.4.1.2.5.2, \
         allow_unauthenticated_participants false): no writer may match her reader"
    );
    assert!(mallory.wait_exit(STEP).success(), "mallory failed");

    for (topic, marker) in [(SECRET, SECRET_MARKER), (OPEN, OPEN_MARKER)] {
        let texts: Vec<String> = bob_lines
            .iter()
            .filter_map(|l| l.strip_prefix("EVENT sample "))
            .map(fields)
            .filter(|f| f["topic"] == topic)
            .map(|f| f["text"].clone())
            .collect();
        let expected: Vec<String> = (0..SAMPLES).map(|s| format!("{marker}{s:04}")).collect();
        assert_eq!(
            texts, expected,
            "bob receives every sample of {topic} once, in order (DDS 1.4 §2.2.3.14, §2.2.3.18)"
        );
    }

    let record = std::fs::read_to_string(&trace).expect("strace record of alice");
    assert!(
        record.contains(&bytes_escaped(OPEN_MARKER)),
        "the record of alice's sends shows no plain-text sample of {OPEN}: it does not see \
         payloads, so it cannot show their absence either"
    );
    assert!(
        !record.contains(&bytes_escaped(SECRET_MARKER)),
        "{SECRET} is ENCRYPT in the governance; its plain text left alice's process \
         (DDS-Security 1.2 §10.4.1.2.6.7, §9.5.1.10.1)"
    );
    drop(lease);
}

#[test]
fn security_end_to_end() {
    match role().as_deref() {
        Some("alice") => alice(),
        Some("bob") => bob(),
        Some("carol") => carol(),
        Some("mallory") => mallory(),
        Some(other) => panic!("unknown role {other}"),
        None => scenario(),
    }
}
