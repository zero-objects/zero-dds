// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! The contract of `zerodds-coap-bridge` with `zerodds-dtls-openssl`
//! (internal/release/rc8-test-suite.md, "Crate tier"): what the bridge uses
//! of it — the server and client configurations, `Server::bind`/`accept`/
//! `rotate`, `connect`, `Session::send`/`recv`/`waker`/`peer_identity`/
//! `cipher`/`close`, `KeyName` and its three forms, and the errors.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::time::Duration;

use zerodds_dtls_openssl::{
    ClientConfig, ConfigError, KeyName, PeerIdentity, Server, ServerConfig, connect,
};

#[test]
fn the_bridge_runs_a_psk_session_and_wakes_it() {
    let secret = vec![3u8; 16];
    let server = Server::bind(
        "127.0.0.1:0".parse().unwrap(),
        &ServerConfig {
            psk: vec![("bridge".into(), secret.clone())],
            psk_identity_hint: Some("bridge".into()),
            ..ServerConfig::default()
        },
    )
    .expect("bind");
    let addr = server.local_addr().unwrap();
    let mut client = connect(
        addr,
        &ClientConfig {
            psk: Some(("bridge".into(), secret)),
            ..ClientConfig::default()
        },
        Duration::from_secs(10),
    )
    .expect("connect");
    assert_eq!(client.cipher(), Some("PSK-AES128-CCM8"));
    let mut session = server.accept(Duration::from_secs(10)).expect("accept");
    assert_eq!(
        session.peer_identity(),
        Some(&PeerIdentity::Psk(b"bridge".to_vec()))
    );
    client.send(b"one").unwrap();
    assert_eq!(
        session.recv(Duration::from_secs(5)).unwrap().as_deref(),
        Some(&b"one"[..])
    );
    assert!(session.waker().wake());
    assert_eq!(session.recv(Duration::from_secs(5)).unwrap(), None);
    server
        .rotate(&ServerConfig {
            psk: vec![("other".into(), vec![4u8; 16])],
            ..ServerConfig::default()
        })
        .expect("rotate");
    client.close();
}

#[test]
fn the_bridge_reads_key_names_and_refuses_bad_configurations() {
    let name = KeyName::parse("nih:sha-256-120;5326-9057-e12f-e2b7-4ba0-7c89-2560-a2;f").unwrap();
    assert_eq!(KeyName::parse(&name.to_ni_uri()).unwrap(), name);
    assert!(KeyName::parse("ni:///sha-1;abc").is_err());
    assert!(matches!(
        ServerConfig::default().context(),
        Err(ConfigError::NoTrust(_))
    ));
    assert!(matches!(
        ClientConfig::default().context(),
        Err(ConfigError::NoTrust(_))
    ));
    assert!(matches!(
        ServerConfig {
            psk: vec![("x".into(), vec![1u8; 16])],
            cipher_suites: vec!["TLS_RSA_WITH_RC4_128_MD5".into()],
            ..ServerConfig::default()
        }
        .context(),
        Err(ConfigError::UnknownSuite(_))
    ));
    let generated = ServerConfig {
        self_signed: vec!["localhost".into()],
        ..ServerConfig::default()
    };
    assert!(generated.context().is_ok());
}
