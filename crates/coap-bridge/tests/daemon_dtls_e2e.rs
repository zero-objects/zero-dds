// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! E2E for the `zerodds-coap-bridged` daemon serving `coaps://` over DTLS
//! (features `daemon` + `dtls`, RFC 7252 §9, ADR 0014).
//!
//! A real DTLS client handshakes with the daemon and drives the full request
//! path through the shared `dispatch` core — POST → 2.04 Changed, GET-Observe →
//! initial 2.05, and a subsequent DDS sample is pushed back as a CoAP Observe
//! notification *over the encrypted DTLS session* (the pump queues it for the
//! session's thread and wakes it).
//!
//! The crate doc sits before `#![cfg]` so the (empty) crate still carries docs
//! when the features are off — otherwise `missing_docs` + `-D warnings` fails.
#![cfg(all(feature = "daemon", feature = "dtls"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::field_reassign_with_default,
    missing_docs
)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use zerodds_coap_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_coap_bridge::daemon::server;
use zerodds_coap_bridge::dtls_transport::{DtlsCoapClient, DtlsCoapSession, KeyName};
use zerodds_coap_bridge::message::{CoapCode, CoapMessage, MessageType};
use zerodds_coap_bridge::option::CoapOption;

const WAIT: Duration = Duration::from_secs(10);

/// The configuration on a leased DDS domain (the host's share of the band,
/// `zerodds-test-domains`); the lease is held while the test runs.
fn dtls_test_config() -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.bind = "127.0.0.1:0".to_string(); // plaintext coap:// (required)
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.dtls_enabled = true;
    cfg.bind_dtls = Some("127.0.0.1:0".to_string()); // ephemeral coaps://
    cfg.topics.push(TopicConfig {
        dds_name: "Trade".to_string(),
        dds_type: "Trade".to_string(),
        coap_uri_path: "trade".to_string(),
        direction: "bidir".to_string(),
        reliability: "best_effort".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
        ..Default::default()
    });
    (cfg, lease)
}

fn scratch(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "zd-coap-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A test CA, written to `dir/ca.pem`, and a function that issues a
/// certificate for `cn` (with the SAN `localhost` when `server`).
struct Pki {
    dir: PathBuf,
    ca: rcgen::Certificate,
    ca_key: rcgen::KeyPair,
}

impl Pki {
    fn new(dir: &Path) -> Self {
        let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        ca_params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "coap test CA");
        let ca_key = rcgen::KeyPair::generate().unwrap();
        let ca = ca_params.self_signed(&ca_key).unwrap();
        std::fs::write(dir.join("ca.pem"), ca.pem()).unwrap();
        Self {
            dir: dir.to_path_buf(),
            ca,
            ca_key,
        }
    }

    fn ca_file(&self) -> String {
        self.dir.join("ca.pem").to_string_lossy().into_owned()
    }

    fn issue(&self, cn: &str, server: bool) -> (String, String) {
        let names = if server {
            vec!["localhost".to_string()]
        } else {
            Vec::new()
        };
        let mut p = rcgen::CertificateParams::new(names).unwrap();
        p.distinguished_name.push(rcgen::DnType::CommonName, cn);
        let key = rcgen::KeyPair::generate().unwrap();
        let cert = p.signed_by(&key, &self.ca, &self.ca_key).unwrap();
        let (c, k) = (
            self.dir.join(format!("{cn}.pem")),
            self.dir.join(format!("{cn}.key")),
        );
        std::fs::write(&c, cert.pem()).unwrap();
        std::fs::write(&k, key.serialize_pem()).unwrap();
        (
            c.to_string_lossy().into_owned(),
            k.to_string_lossy().into_owned(),
        )
    }
}

fn dtls_addr(handle: &server::DaemonHandle) -> std::net::SocketAddr {
    handle.dtls_local_addr().expect("dtls listener")
}

/// Receive one CoAP message over DTLS with a generous timeout.
fn recv(client: &mut DtlsCoapSession) -> Option<CoapMessage> {
    client.recv_message(Duration::from_secs(3)).ok().flatten()
}

fn post(mid: u16, payload: &[u8]) -> CoapMessage {
    let mut m = CoapMessage::new(MessageType::Confirmable, CoapCode::POST, mid);
    m.token = mid.to_be_bytes().to_vec();
    m.options.push(CoapOption::uri_path("trade"));
    m.options.push(CoapOption::content_format(65000));
    m.payload = payload.to_vec();
    m
}

#[test]
fn coap_9_daemon_serves_coaps_over_dtls_request_response_and_observe() {
    let dir = scratch("dtls");
    let pki = Pki::new(&dir);
    let (cert, key) = pki.issue("daemon", true);
    let (mut cfg, _lease) = dtls_test_config();
    cfg.dtls.cert_file = Some(cert);
    cfg.dtls.key_file = Some(key);
    let mut handle = server::start(cfg).expect("daemon start");

    // Real DTLS handshake, the daemon's certificate checked against the CA.
    let mut client = DtlsCoapClient::connect_verified(
        dtls_addr(&handle),
        "localhost",
        &pki.ca_file(),
        None,
        WAIT,
    )
    .expect("dtls handshake with daemon");

    // 1) POST → DDS write → 2.04 Changed, over DTLS.
    let req = post(0x4001, b"AAPL@200");
    client.send_message(&req).expect("send POST");
    let resp = recv(&mut client).expect("POST response");
    assert_eq!(
        resp.code,
        CoapCode::CHANGED,
        "POST → 2.04 Changed over DTLS"
    );
    assert_eq!(resp.token, req.token, "token echoed");

    // 2) GET with Observe:0 → initial 2.05 Content with the Observe option.
    let mut obs = CoapMessage::new(MessageType::Confirmable, CoapCode::GET, 0x4002);
    obs.token = b"obs01".to_vec();
    obs.options.push(CoapOption::uri_path("trade"));
    obs.options.push(CoapOption::observe(0));
    client.send_message(&obs).expect("send observe GET");
    let initial = recv(&mut client).expect("observe initial");
    assert_eq!(initial.code, CoapCode::CONTENT, "observe initial → 2.05");
    assert!(
        initial.options.iter().any(|o| o.number == 6), // Observe option number
        "initial observe response must carry the Observe option"
    );

    // 3) A new sample → pushed back as an Observe notification over DTLS.
    client
        .send_message(&post(0x4003, b"MSFT@300"))
        .expect("send POST 2");
    // Collect a few messages; one of them must be the NON-confirmable Observe
    // notification carrying the new sample's payload (the DDS sample loops back
    // through the daemon's own reader → pump → DTLS session).
    let mut got_notify = false;
    for _ in 0..4 {
        match recv(&mut client) {
            Some(m)
                if matches!(m.message_type, MessageType::NonConfirmable)
                    && m.payload == b"MSFT@300" =>
            {
                got_notify = true;
                break;
            }
            Some(_) => continue, // the 2.04 ack for post2, etc.
            None => break,
        }
    }
    assert!(
        got_notify,
        "expected an Observe notification with the new sample over DTLS"
    );

    // 4) The shutdown ends the observation with 5.03 over the session.
    handle.shutdown();
    let mut ended = false;
    for _ in 0..4 {
        match recv(&mut client) {
            Some(m) if m.code == CoapCode::new(5, 3) && m.token == b"obs01" => {
                ended = true;
                break;
            }
            Some(_) => continue,
            None => break,
        }
    }
    assert!(ended, "the observation ends with 5.03 over DTLS");
    client.close();
    let _ = std::fs::remove_dir_all(&dir);
}

/// §7.2 `auth.mode: mtls` over DTLS-PSK: the PSK identity of the handshake
/// is the subject (`psk:<identity>`) the ACL names. The identity did not
/// reach the request path before, so `mtls` answered every request 4.01.
#[test]
fn coapbridge_7_3_mtls_takes_the_psk_identity_as_the_acl_subject() {
    let (mut cfg, _lease) = dtls_test_config();
    cfg.dtls.psk.add("sensor-1", vec![7u8; 16]).unwrap();
    cfg.dtls.psk.add("sensor-2", vec![8u8; 16]).unwrap();
    cfg.auth_mode = "mtls".into();
    cfg.topic_acl.insert(
        "Trade".into(),
        (vec!["psk:sensor-1".into()], vec!["psk:sensor-1".into()]),
    );
    let mut handle = server::start(cfg).expect("daemon start");
    for (identity, secret, expect) in [
        ("sensor-1", 7u8, CoapCode::CHANGED),
        ("sensor-2", 8u8, CoapCode::new(4, 3)),
    ] {
        let mut client =
            DtlsCoapClient::connect_psk(dtls_addr(&handle), identity, vec![secret; 16], WAIT)
                .expect("psk handshake");
        client
            .send_message(&post(0x5000 + u16::from(secret), b"X@1"))
            .expect("send POST");
        let resp = recv(&mut client).expect("POST response");
        assert_eq!(resp.code, expect, "{identity}");
        client.close();
    }
    handle.shutdown();
}

/// `auth.mode: mtls` without DTLS stops the start as a configuration error.
#[test]
fn coapbridge_7_3_mtls_without_dtls_is_a_config_error() {
    let (mut cfg, _lease) = dtls_test_config();
    cfg.dtls_enabled = false;
    cfg.auth_mode = "mtls".into();
    match server::start(cfg) {
        Err(server::ServerError::Config(_)) => {}
        Err(e) => panic!("{e}"),
        Ok(_) => panic!("started"),
    }
}

/// §7.3 over DTLS with a client certificate: the subject is the
/// certificate's DN (`CN=sensor-9`, RFC 4514); a second certificate from the
/// same CA is authenticated and refused by the ACL.
#[test]
fn coapbridge_7_3_mtls_takes_the_certificate_dn_as_the_acl_subject() {
    let dir = scratch("mtls");
    let pki = Pki::new(&dir);
    let (server_cert, server_key) = pki.issue("daemon", true);
    let sensor = pki.issue("sensor-9", false);
    let other = pki.issue("other", false);
    let (mut cfg, _lease) = dtls_test_config();
    cfg.dtls.cert_file = Some(server_cert);
    cfg.dtls.key_file = Some(server_key);
    cfg.dtls.ca_file = Some(pki.ca_file());
    cfg.auth_mode = "mtls".into();
    cfg.topic_acl.insert(
        "Trade".into(),
        (vec!["CN=sensor-9".into()], vec!["CN=sensor-9".into()]),
    );
    let mut handle = server::start(cfg).expect("daemon start");
    for ((cert, key), mid, expect) in [
        (&sensor, 0x6001u16, CoapCode::CHANGED),
        (&other, 0x6002, CoapCode::new(4, 3)),
    ] {
        let mut client = DtlsCoapClient::connect_verified(
            dtls_addr(&handle),
            "localhost",
            &pki.ca_file(),
            Some((cert, key)),
            WAIT,
        )
        .expect("certificate handshake");
        client.send_message(&post(mid, b"X@2")).expect("send POST");
        let resp = recv(&mut client).expect("POST response");
        assert_eq!(resp.code, expect, "{cert}");
        client.close();
    }
    handle.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}

/// RFC 7252 §9.1.3.2 at the daemon: `coap.dtls.rpk_key_file` and
/// `coap.dtls.rpk_trusted`; the subject of a raw public key is `rpk:` and its
/// `ni` URI (RFC 6920, sha-256), the name the ACL takes.
#[test]
fn coap_9_1_3_2_the_daemon_takes_raw_public_keys_and_names_them_in_the_acl() {
    let dir = scratch("rpk");
    let write = |name: &str, k: &rcgen::KeyPair| {
        let p = dir.join(name);
        std::fs::write(&p, k.serialize_pem()).unwrap();
        p.to_string_lossy().into_owned()
    };
    let daemon_key = rcgen::KeyPair::generate().unwrap();
    let allowed = rcgen::KeyPair::generate().unwrap();
    let refused = rcgen::KeyPair::generate().unwrap();
    let name = |k: &rcgen::KeyPair, suite: u8| KeyName::of_key(&k.public_key_der(), suite).unwrap();
    let (mut cfg, _lease) = dtls_test_config();
    cfg.dtls.rpk_key_file = Some(write("daemon.pem", &daemon_key));
    cfg.dtls.rpk_trusted = vec![
        name(&allowed, 3).to_ni_uri(),
        name(&refused, 3).to_nih_uri(),
    ];
    cfg.auth_mode = "mtls".into();
    let subject = format!("rpk:{}", name(&allowed, 1).to_ni_uri());
    cfg.topic_acl
        .insert("Trade".into(), (vec![subject.clone()], vec![subject]));
    let mut handle = server::start(cfg).expect("daemon start");
    for (key, mid, expect) in [
        (&allowed, 0x7001u16, CoapCode::CHANGED),
        (&refused, 0x7002, CoapCode::new(4, 3)),
    ] {
        let mut client = DtlsCoapClient::connect_rpk(
            dtls_addr(&handle),
            &write(&format!("client-{mid}.pem"), key),
            vec![name(&daemon_key, 3)],
            WAIT,
        )
        .expect("rpk handshake");
        client.send_message(&post(mid, b"X@3")).expect("send POST");
        let resp = recv(&mut client).expect("POST response");
        assert_eq!(resp.code, expect);
        client.close();
    }
    let stranger = rcgen::KeyPair::generate().unwrap();
    assert!(
        DtlsCoapClient::connect_rpk(
            dtls_addr(&handle),
            &write("stranger.pem", &stranger),
            vec![name(&daemon_key, 3)],
            Duration::from_secs(5),
        )
        .is_err(),
        "a key not in coap.dtls.rpk_trusted"
    );
    handle.shutdown();
    let _ = std::fs::remove_dir_all(&dir);
}
