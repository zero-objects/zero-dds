// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! The three DTLS modes of RFC 7252 §9 between this crate's client and
//! server over loopback.

#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use openssl::asn1::Asn1Time;
use openssl::bn::BigNum;
use openssl::ec::{EcGroup, EcKey};
use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::x509::extension::{BasicConstraints, SubjectAlternativeName};
use openssl::x509::{X509, X509NameBuilder};

use zerodds_dtls_openssl::{
    CertificateFiles, ClientConfig, KeyName, PeerIdentity, Server, ServerConfig, connect,
};

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "zerodds-dtls-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn p256() -> PKey<Private> {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).unwrap();
    PKey::from_ec_key(EcKey::generate(&group).unwrap()).unwrap()
}

fn write_key(d: &Path, name: &str, key: &PKey<Private>) -> PathBuf {
    let p = d.join(name);
    std::fs::write(&p, key.private_key_to_pem_pkcs8().unwrap()).unwrap();
    p
}

fn cert(
    subject: &str,
    key: &PKey<Private>,
    issuer: Option<(&X509, &PKey<Private>)>,
    ca: bool,
) -> X509 {
    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_text("CN", subject).unwrap();
    let name = name.build();
    let mut b = X509::builder().unwrap();
    b.set_version(2).unwrap();
    b.set_serial_number(&BigNum::from_u32(7).unwrap().to_asn1_integer().unwrap())
        .unwrap();
    b.set_subject_name(&name).unwrap();
    b.set_issuer_name(issuer.map_or(&name, |(c, _)| c.subject_name()))
        .unwrap();
    b.set_pubkey(key).unwrap();
    b.set_not_before(&Asn1Time::days_from_now(0).unwrap())
        .unwrap();
    b.set_not_after(&Asn1Time::days_from_now(2).unwrap())
        .unwrap();
    if ca {
        b.append_extension(BasicConstraints::new().critical().ca().build().unwrap())
            .unwrap();
    } else {
        let san = SubjectAlternativeName::new()
            .ip("127.0.0.1")
            .dns("localhost")
            .build(&b.x509v3_context(issuer.map(|(c, _)| &**c), None))
            .unwrap();
        b.append_extension(san).unwrap();
    }
    b.sign(issuer.map_or(key, |(_, k)| k), MessageDigest::sha256())
        .unwrap();
    b.build()
}

fn loopback() -> SocketAddr {
    "127.0.0.1:0".parse().unwrap()
}

/// Accepts one session on `server` and echoes one record.
fn echo(server: Server) -> std::thread::JoinHandle<Option<PeerIdentity>> {
    std::thread::spawn(move || {
        let mut s = server.accept(Duration::from_secs(10))?;
        let data = s.recv(Duration::from_secs(5)).ok()??;
        s.send(&data).ok()?;
        std::thread::sleep(Duration::from_millis(200));
        s.peer_identity().cloned()
    })
}

#[test]
fn coap_9_1_3_1_a_pre_shared_key_with_tls_psk_with_aes_128_ccm_8() {
    let secret = b"0123456789abcdef".to_vec();
    let server = Server::bind(
        loopback(),
        &ServerConfig {
            psk: vec![("device-1".into(), secret.clone())],
            psk_identity_hint: Some("bridge".into()),
            ..ServerConfig::default()
        },
    )
    .expect("server");
    let addr = server.local_addr().unwrap();
    let peer = echo(server);
    let mut client = connect(
        addr,
        &ClientConfig {
            psk: Some(("device-1".into(), secret)),
            cipher_suites: vec!["TLS_PSK_WITH_AES_128_CCM_8".into()],
            ..ClientConfig::default()
        },
        Duration::from_secs(10),
    )
    .expect("handshake");
    assert_eq!(client.cipher(), Some("PSK-AES128-CCM8"));
    client.send(b"ping").unwrap();
    assert_eq!(
        client.recv(Duration::from_secs(5)).unwrap().as_deref(),
        Some(&b"ping"[..])
    );
    assert_eq!(
        peer.join().unwrap(),
        Some(PeerIdentity::Psk(b"device-1".to_vec()))
    );
}

#[test]
fn coap_9_1_3_1_an_unknown_identity_or_another_key_fails() {
    let server = Server::bind(
        loopback(),
        &ServerConfig {
            psk: vec![("device-1".into(), b"0123456789abcdef".to_vec())],
            ..ServerConfig::default()
        },
    )
    .expect("server");
    let addr = server.local_addr().unwrap();
    for (id, key) in [
        ("device-2", b"0123456789abcdef"),
        ("device-1", b"fedcba9876543210"),
    ] {
        let r = connect(
            addr,
            &ClientConfig {
                psk: Some((id.into(), key.to_vec())),
                ..ClientConfig::default()
            },
            Duration::from_secs(5),
        );
        assert!(r.is_err(), "{id}");
    }
}

#[test]
fn coap_9_1_3_3_a_certificate_is_checked_against_the_anchors_and_the_name() {
    let d = dir();
    let ca_key = p256();
    let ca = cert("ca", &ca_key, None, true);
    let srv_key = p256();
    let srv = cert("server", &srv_key, Some((&ca, &ca_key)), false);
    let cli_key = p256();
    let cli = cert("client", &cli_key, Some((&ca, &ca_key)), false);
    let other_key = p256();
    let other = cert("other", &other_key, None, true);
    std::fs::write(d.join("ca.pem"), ca.to_pem().unwrap()).unwrap();
    std::fs::write(d.join("other.pem"), other.to_pem().unwrap()).unwrap();
    std::fs::write(d.join("srv.pem"), srv.to_pem().unwrap()).unwrap();
    std::fs::write(d.join("cli.pem"), cli.to_pem().unwrap()).unwrap();
    let files = |c: &str, k: &PKey<Private>, kn: &str| CertificateFiles {
        cert_file: d.join(c),
        key_file: write_key(&d, kn, k),
    };
    let server = Server::bind(
        loopback(),
        &ServerConfig {
            certificate: Some(files("srv.pem", &srv_key, "srv.key")),
            client_ca_file: Some(d.join("ca.pem")),
            ..ServerConfig::default()
        },
    )
    .expect("server");
    let addr = server.local_addr().unwrap();
    let peer = echo(server);
    let good = ClientConfig {
        certificate: Some(files("cli.pem", &cli_key, "cli.key")),
        ca_file: Some(d.join("ca.pem")),
        server_name: Some("127.0.0.1".into()),
        cipher_suites: vec!["TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8".into()],
        ..ClientConfig::default()
    };
    let mut client = connect(addr, &good, Duration::from_secs(10)).expect("handshake");
    assert_eq!(client.cipher(), Some("ECDHE-ECDSA-AES128-CCM8"));
    match client.peer_identity() {
        Some(PeerIdentity::Certificate(der)) => assert_eq!(*der, srv.to_der().unwrap()),
        other => panic!("{other:?}"),
    }
    client.send(b"x").unwrap();
    assert!(client.recv(Duration::from_secs(5)).unwrap().is_some());
    assert_eq!(
        peer.join().unwrap(),
        Some(PeerIdentity::Certificate(cli.to_der().unwrap()))
    );

    // Another trust anchor, another name, no client certificate: refused.
    let server = Server::bind(
        loopback(),
        &ServerConfig {
            certificate: Some(files("srv.pem", &srv_key, "srv.key")),
            client_ca_file: Some(d.join("ca.pem")),
            ..ServerConfig::default()
        },
    )
    .expect("server");
    let addr = server.local_addr().unwrap();
    let wrong_anchor = ClientConfig {
        ca_file: Some(d.join("other.pem")),
        ..good.clone()
    };
    let wrong_name = ClientConfig {
        server_name: Some("bridge.example".into()),
        ..good.clone()
    };
    let no_cert = ClientConfig {
        certificate: None,
        ..good.clone()
    };
    for c in [wrong_anchor, wrong_name, no_cert] {
        assert!(connect(addr, &c, Duration::from_secs(5)).is_err());
    }
    drop(server);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn coap_9_1_3_2_raw_public_keys_named_after_rfc_6920() {
    let d = dir();
    let srv_key = p256();
    let cli_key = p256();
    let stranger = p256();
    let srv_spki = srv_key.public_key_to_der().unwrap();
    let cli_spki = cli_key.public_key_to_der().unwrap();
    // §9.1.3.2.1: at least sha-256-120.
    let srv_name = KeyName::of_key(&srv_spki, 3).unwrap();
    let cli_name = KeyName::parse(&KeyName::of_key(&cli_spki, 3).unwrap().to_nih_uri()).unwrap();
    let server_config = ServerConfig {
        raw_public_key: Some(write_key(&d, "srv.key", &srv_key)),
        trusted_client_keys: vec![cli_name],
        ..ServerConfig::default()
    };
    let server = Server::bind(loopback(), &server_config).expect("server");
    let addr = server.local_addr().unwrap();
    let peer = echo(server);
    let client_config = ClientConfig {
        raw_public_key: Some(write_key(&d, "cli.key", &cli_key)),
        trusted_server_keys: vec![srv_name.clone()],
        cipher_suites: vec!["TLS_ECDHE_ECDSA_WITH_AES_128_CCM_8".into()],
        ..ClientConfig::default()
    };
    let mut client = connect(addr, &client_config, Duration::from_secs(10)).expect("handshake");
    assert_eq!(client.cipher(), Some("ECDHE-ECDSA-AES128-CCM8"));
    assert_eq!(
        client.peer_identity(),
        Some(&PeerIdentity::RawPublicKey(srv_spki.clone()))
    );
    client.send(b"rpk").unwrap();
    assert!(client.recv(Duration::from_secs(5)).unwrap().is_some());
    assert_eq!(
        peer.join().unwrap(),
        Some(PeerIdentity::RawPublicKey(cli_spki))
    );

    // A key the other side does not trust: no session.
    let server = Server::bind(loopback(), &server_config).expect("server");
    let addr = server.local_addr().unwrap();
    let unknown_client = ClientConfig {
        raw_public_key: Some(write_key(&d, "stranger.key", &stranger)),
        ..client_config.clone()
    };
    assert!(connect(addr, &unknown_client, Duration::from_secs(5)).is_err());
    let wrong_server = ClientConfig {
        trusted_server_keys: vec![
            KeyName::of_key(&stranger.public_key_to_der().unwrap(), 3).unwrap(),
        ],
        ..client_config
    };
    assert!(connect(addr, &wrong_server, Duration::from_secs(5)).is_err());
    drop(server);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_waiting_receive_returns_when_woken_and_a_close_ends_the_peer() {
    let secret = b"0123456789abcdef".to_vec();
    let server = Server::bind(
        loopback(),
        &ServerConfig {
            psk: vec![("w".into(), secret.clone())],
            ..ServerConfig::default()
        },
    )
    .expect("server");
    let addr = server.local_addr().unwrap();
    let mut client = connect(
        addr,
        &ClientConfig {
            psk: Some(("w".into(), secret)),
            ..ClientConfig::default()
        },
        Duration::from_secs(10),
    )
    .expect("handshake");
    let mut session = server.accept(Duration::from_secs(10)).expect("accepted");
    let waker = session.waker();
    let started = std::time::Instant::now();
    let t = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        waker.wake()
    });
    assert_eq!(session.recv(Duration::from_secs(10)).unwrap(), None);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(t.join().unwrap());
    client.close();
    let err = session.recv(Duration::from_secs(5)).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::ConnectionAborted);
}
