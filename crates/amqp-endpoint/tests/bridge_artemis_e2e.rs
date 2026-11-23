// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The bridge daemon (`zerodds-amqp-bridge-daemon-1.0`) against Apache
//! ActiveMQ Artemis.
//!
//! The broker matrix of §10 (`support/broker_matrix.rs`): data in both
//! directions with qpid-proton as the client at the broker, multi-frame
//! transfers, the reconnect after a broker restart, a refused address
//! beside a running link; the addresses in the three forms of §5.1
//! (`<address>` and `queue://<name>` in the matrix, `topic://<name>` in
//! `amqpbridge_5_1_artemis_a_topic_address_reaches_every_subscriber`).
//!
//! The SASL layer of §7.2: an Artemis broker of each test
//! alone whose acceptor offers only SCRAM-SHA-256 and SCRAM-SHA-512
//! (`saslMechanisms`) with Artemis's `SCRAMPropertiesLoginModule`, the user
//! entries made by that module's own encoder. Artemis checks the client's
//! proof and the bridge checks Artemis's server signature: a connection
//! that stands proves both sides computed the same key schedule.
//!
//! The tests need the Artemis release archive and a JDK; they run in the
//! integration pillar's job `integration-bridges-vendor-amqp`, where
//! `tests/integration/provision.sh artemis` installs them and a missing
//! program fails them:
//!
//! ```bash
//! cargo nextest run -p zerodds-amqp-endpoint --test bridge_artemis_e2e --run-ignored all
//! ```

#![cfg(all(feature = "std", unix))]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

#[path = "support/artemis.rs"]
mod artemis;
#[path = "support/broker_matrix.rs"]
mod broker_matrix;

use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use artemis::ArtemisBroker;
use zerodds_amqp_endpoint::bridge_daemon::{self, DaemonConfig};
use zerodds_amqp_endpoint::broker::{self, ConnectParams, ConnectStage, SaslCredentials};
use zerodds_amqp_endpoint::daemon_runtime::BridgeMetrics;
use zerodds_dcps::runtime::RuntimeConfig;
use zerodds_monitor::Registry;

/// The user of the SCRAM login module, and its password.
const USER: &str = "scram-user";
const PASSWORD: &str = "scram-pencil";

/// An Artemis whose acceptor offers SCRAM-SHA-256 and SCRAM-SHA-512 alone,
/// with `USER` in the users file of a `SCRAMPropertiesLoginModule` scope.
fn scram_artemis() -> ArtemisBroker {
    ArtemisBroker::start_configured(&["zd_scram"], |dir: &Path| {
        let home = std::env::var_os("ARTEMIS_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("/opt/artemis"));
        // The module's own encoder: one line per mechanism, `user|SHA256=…`
        // and `user|SHA512=…`.
        let out = Command::new("java")
            .arg("-cp")
            .arg(home.join("lib").join("*"))
            .arg("org.apache.activemq.artemis.spi.core.security.jaas.SCRAMPropertiesLoginModule")
            .args([USER, PASSWORD, "4096"])
            .output()
            .expect("java");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let lines: Vec<&str> = text
            .lines()
            .filter(|l| l.starts_with(&format!("{USER}|")))
            .collect();
        assert_eq!(lines.len(), 2, "the encoder's output:\n{text}");
        let etc = dir.join("etc");
        std::fs::write(
            etc.join("artemis-users-scram.properties"),
            lines.join("\n") + "\n",
        )
        .unwrap();
        std::fs::write(
            etc.join("artemis-roles-scram.properties"),
            format!("amq = {USER}\n"),
        )
        .unwrap();
        let login = etc.join("login.config");
        let mut config = std::fs::read_to_string(&login).unwrap();
        config.push_str(
            "\namqp-sasl-scram {\n   org.apache.activemq.artemis.spi.core.security.jaas.SCRAMPropertiesLoginModule required\n       org.apache.activemq.jaas.properties.user=\"artemis-users-scram.properties\"\n       org.apache.activemq.jaas.properties.role=\"artemis-roles-scram.properties\";\n};\n",
        );
        std::fs::write(&login, config).unwrap();
        let xml_path = etc.join("broker.xml");
        let xml = std::fs::read_to_string(&xml_path).unwrap();
        let start = xml
            .find("<acceptor name=\"artemis\">")
            .expect("the acceptor of artemis create");
        let end = start + xml[start..].find("</acceptor>").unwrap();
        let mut patched = xml[..end].to_string();
        patched.push_str(
            ";saslMechanisms=SCRAM-SHA-256,SCRAM-SHA-512;saslLoginConfigScope=amqp-sasl-scram",
        );
        patched.push_str(&xml[end..]);
        std::fs::write(&xml_path, patched).unwrap();
    })
}

fn params(broker: &ArtemisBroker, sasl: SaslCredentials) -> ConnectParams {
    ConnectParams {
        addr: broker.addr(),
        host: "127.0.0.1".into(),
        tls: None,
        sasl,
        container_id: "zerodds-scram".into(),
        hostname: None,
        max_frame_size: 65_536,
        channel_max: 7,
        idle_time_out_ms: 0,
        properties: Vec::new(),
        capabilities: Vec::new(),
        timeout: Duration::from_secs(10),
    }
}

/// §7.2: Artemis accepts SCRAM-SHA-256 and SCRAM-SHA-512 of the bridge
/// (its proof verified by Artemis, Artemis's server signature by the
/// bridge); a wrong password is refused in the SASL stage; PLAIN, which
/// the acceptor does not offer, is refused by name before any credential
/// is sent.
#[test]
#[ignore = "needs Apache ActiveMQ Artemis and a JDK; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_7_2_artemis_accepts_scram_sha_256_and_512() {
    let broker = scram_artemis();
    for (name, good, bad) in [
        (
            "SCRAM-SHA-256",
            SaslCredentials::ScramSha256 {
                username: USER.into(),
                password: PASSWORD.into(),
            },
            SaslCredentials::ScramSha256 {
                username: USER.into(),
                password: "wrong".into(),
            },
        ),
        (
            "SCRAM-SHA-512",
            SaslCredentials::ScramSha512 {
                username: USER.into(),
                password: PASSWORD.into(),
            },
            SaslCredentials::ScramSha512 {
                username: USER.into(),
                password: "wrong".into(),
            },
        ),
    ] {
        let (w, _r, remote) =
            broker::connect(&params(&broker, good)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(!remote.container_id.is_empty(), "{name}");
        w.close(None);
        let e = broker::connect(&params(&broker, bad))
            .err()
            .unwrap_or_else(|| panic!("{name}: a wrong password was accepted"));
        assert_eq!(e.stage, ConnectStage::Sasl, "{name}: {e}");
    }
    let e = broker::connect(&params(
        &broker,
        SaslCredentials::Plain {
            username: USER.into(),
            password: PASSWORD.into(),
        },
    ))
    .err()
    .expect("PLAIN is not offered");
    assert!(e.to_string().contains("not the configured PLAIN"), "{e}");
}

/// §2, §7.2: the daemon with SCRAM-SHA-512 and a wrong password stops with
/// exit code 5 at its first connect.
#[test]
#[ignore = "needs Apache ActiveMQ Artemis and a JDK; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_7_2_a_wrong_scram_sha_512_password_is_exit_5() {
    let broker = scram_artemis();
    let domain = zerodds_test_vendor_reader::lease_domain();
    let yaml = format!(
        "domain: {}\namqp:\n  broker_url: \"amqp://127.0.0.1:{}\"\n  container_id: zd-artemis-scram\n  sasl:\n    mechanism: SCRAM-SHA-512\n    username: {USER}\n    password: wrong\ncatalog:\n  enabled: false\n",
        domain.id(),
        broker.port
    );
    let mut c = zerodds_config::Config::default();
    c.push(
        zerodds_config::parse_str(&yaml, "<test>").unwrap(),
        "<test>".into(),
    )
    .unwrap();
    let cfg = DaemonConfig::from_config(Some(&c)).unwrap();
    let e = bridge_daemon::run(
        &cfg,
        RuntimeConfig::default(),
        &Arc::new(AtomicBool::new(false)),
        &BridgeMetrics::register(&Registry::new()),
        false,
        Duration::from_secs(10),
    )
    .expect_err("a refused SASL exchange ends the daemon");
    assert_eq!(e.code, 5, "{e}");
    assert!(e.msg.contains("SCRAM-SHA-512"), "{e}");
}

/// The queues of the matrix's cases.
const MATRIX_QUEUES: [&str; 6] = [
    "zd_out",
    "zd_in",
    "zd_big_out",
    "zd_big_in",
    "zd_reconnect",
    "zd_beside",
];

/// An Artemis for the matrix: the acceptor maps `queue://` and `topic://`
/// to anycast and multicast addresses (§5.1), and the address `zd_refused`
/// is neither there nor created on demand, so its attach is refused.
struct MatrixArtemis(ArtemisBroker);

impl MatrixArtemis {
    fn start() -> Self {
        Self(ArtemisBroker::start_configured(
            &MATRIX_QUEUES,
            |dir: &Path| {
                let xml_path = dir.join("etc").join("broker.xml");
                let mut xml = std::fs::read_to_string(&xml_path).unwrap();
                let start = xml
                    .find("<acceptor name=\"artemis\">")
                    .expect("the acceptor of artemis create");
                let end = start + xml[start..].find("</acceptor>").unwrap();
                xml.insert_str(end, ";anycastPrefix=queue://;multicastPrefix=topic://");
                let settings = xml
                    .find("</address-settings>")
                    .expect("the address settings of artemis create");
                xml.insert_str(
                settings,
                "<address-setting match=\"zd_refused\"><auto-create-queues>false</auto-create-queues><auto-create-addresses>false</auto-create-addresses></address-setting>\n",
            );
                std::fs::write(&xml_path, xml).unwrap();
            },
        ))
    }
}

impl broker_matrix::MatrixBroker for MatrixArtemis {
    fn amqp_yaml(&self) -> String {
        format!("  broker_url: \"amqp://{}\"\n", self.0.addr())
    }

    fn peer_url(&self) -> String {
        format!("amqp://{}", self.0.addr())
    }

    /// `zd_out` in the plain form, the other queues as `queue://<name>`.
    fn address(&self, name: &str) -> String {
        if name == "zd_out" {
            name.to_string()
        } else {
            format!("queue://{name}")
        }
    }

    fn stores(&self) -> bool {
        true
    }

    fn drop_connections(&mut self) {
        self.0.restart();
    }

    fn refused_address(&self) -> String {
        "zd_refused".into()
    }
}

/// §10, §4.3 against Artemis: DDS samples arrive at qpid-proton as
/// pass-through messages.
#[test]
#[ignore = "needs Apache ActiveMQ Artemis, a JDK and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_10_artemis_dds_samples_arrive_as_pass_through_messages() {
    broker_matrix::samples_arrive_as_pass_through_messages(&mut MatrixArtemis::start());
}

/// §10, §4.4 against Artemis: a message of qpid-proton reaches a DDS
/// reader; a bad body is rejected and reaches none.
#[test]
#[ignore = "needs Apache ActiveMQ Artemis, a JDK and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_10_artemis_a_message_reaches_a_reader_and_a_bad_body_is_rejected() {
    broker_matrix::a_message_reaches_a_reader_and_a_bad_body_is_rejected(
        &mut MatrixArtemis::start(),
    );
}

/// §10, Part 2 §2.6.14 against Artemis: a 20 000-byte sample at a frame
/// size of 4096 in both directions.
#[test]
#[ignore = "needs Apache ActiveMQ Artemis, a JDK and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_10_artemis_a_multi_frame_sample_crosses_in_both_directions() {
    broker_matrix::a_multi_frame_sample_crosses_in_both_directions(&mut MatrixArtemis::start());
}

/// §10, §9.3 against Artemis: after a restart of the broker the bridge
/// connects again and delivers the sample written meanwhile.
#[test]
#[ignore = "needs Apache ActiveMQ Artemis, a JDK and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_10_artemis_the_bridge_reconnects_after_the_broker_drops_the_connection() {
    broker_matrix::the_bridge_reconnects_after_the_broker_drops_the_connection(
        &mut MatrixArtemis::start(),
    );
}

/// §10, §4.2 against Artemis: the attach to an address Artemis does not
/// have is refused; the other link keeps running.
#[test]
#[ignore = "needs Apache ActiveMQ Artemis, a JDK and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_10_artemis_a_refused_address_leaves_the_other_links_running() {
    broker_matrix::a_refused_address_leaves_the_other_links_running(&mut MatrixArtemis::start());
}

/// §5.1 against Artemis, the third address form: `topic://<name>`, a
/// multicast address under the acceptor's `multicastPrefix`. Two receivers
/// of qpid-proton subscribe first; each gets every sample the bridge sends
/// there, and each round trip ends `accepted` at the bridge.
#[test]
#[ignore = "needs Apache ActiveMQ Artemis, a JDK and qpid-proton; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_5_1_artemis_a_topic_address_reaches_every_subscriber() {
    use broker_matrix::{
        Bridge, MatrixBroker, ProtonPeer, STEP, bridge_yaml, config, matched_writer, participant,
        topic,
    };
    let artemis = MatrixArtemis::start();
    let address = "topic://zd_multi";
    let mut first = ProtonPeer::connect(&artemis.peer_url());
    first.receiver(address, 10);
    let mut second = ProtonPeer::connect(&artemis.peer_url());
    second.receiver(address, 10);
    let bridge = Bridge::start(config(&bridge_yaml(
        &artemis,
        "multi",
        "",
        &topic("MatrixMulti", address, "out"),
    )));
    let dds = participant();
    let w = matched_writer(&dds, "MatrixMulti");
    for n in 1u8..=2 {
        dds.write_user_sample(w, vec![n, n, n, n]).unwrap();
    }
    let got_first = first.take(address, 2, STEP);
    let got_second = second.take(address, 2, STEP);
    bridge.wait_metric("zerodds_amqp_bridge_dispositions_accepted_total", 2);
    bridge.stop();
    dds.shutdown();
    for got in [&got_first, &got_second] {
        assert_eq!(got.len(), 2, "{got:?}");
        for (i, m) in got.iter().enumerate() {
            let n = (i + 1) as u8;
            assert_eq!(&m.body[4..], &[n, n, n, n], "{m:?}");
            assert!(m.properties.contains("dds:bridge-id="), "{m:?}");
        }
    }
}
