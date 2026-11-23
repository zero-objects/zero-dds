// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! SASL XOAUTH2 of the bridge daemon (`zerodds-amqp-bridge-daemon-1.0`
//! §7.2) against Apache Qpid Broker-J: an AMQP 1.0 port whose
//! authentication provider is Broker-J's OAuth2 provider, which offers
//! XOAUTH2 and asks an identity resolver who the bearer token belongs to.
//! The resolver is Broker-J's `GitHubUser`, pointed at an HTTPS endpoint of
//! this test (Broker-J accepts only secure endpoints; its trust store is the
//! test's CA as PEM) that answers for one token; Broker-J reads the token
//! out of the bridge's initial response and decides.
//!
//! The tests need the Broker-J release archive and a JDK; they run in the
//! integration pillar's job `integration-bridges-vendor-amqp`, where
//! `tests/integration/provision.sh brokerj` installs them and a missing
//! program fails them:
//!
//! ```bash
//! cargo nextest run -p zerodds-amqp-endpoint --test bridge_brokerj_e2e --run-ignored all
//! ```

#![cfg(all(feature = "std", unix))]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

#[path = "support/brokerj.rs"]
mod brokerj;

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use brokerj::BrokerJ;
use zerodds_amqp_endpoint::broker::{self, ConnectParams, ConnectStage, SaslCredentials};

/// The one token the identity endpoint knows, and its user.
const TOKEN: &str = "zerodds-test-token-7f3a";
const USER: &str = "zd-oauth-user";

/// A CA and a server certificate for `localhost` and `127.0.0.1`, as PEM
/// files in `dir`: `ca.pem`, `server.pem`, `server.key`.
fn write_pki(dir: &std::path::Path) {
    let ca_key = rcgen::KeyPair::generate().unwrap();
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca = ca_params.self_signed(&ca_key).unwrap();
    let key = rcgen::KeyPair::generate().unwrap();
    let cert =
        rcgen::CertificateParams::new(vec!["localhost".to_string(), "127.0.0.1".to_string()])
            .unwrap()
            .signed_by(&key, &ca, &ca_key)
            .unwrap();
    std::fs::write(dir.join("ca.pem"), ca.pem()).unwrap();
    std::fs::write(dir.join("server.pem"), cert.pem()).unwrap();
    std::fs::write(dir.join("server.key"), key.serialize_pem()).unwrap();
}

/// An identity endpoint over HTTPS in the manner of GitHub's `/user`: a
/// request with `Authorization: token <TOKEN>` gets `{"login": USER}`, any
/// other 401. Returns its port and the authorization headers it saw.
fn identity_endpoint(dir: &std::path::Path) -> (u16, Arc<Mutex<Vec<String>>>) {
    let config = zerodds_bridge_security::load_server_config(
        &dir.join("server.pem"),
        &dir.join("server.key"),
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
            let Ok(conn) = rustls::ServerConnection::new(Arc::clone(&config)) else {
                continue;
            };
            let mut tls = rustls::StreamOwned::new(conn, stream);
            let mut authorization = String::new();
            {
                let mut reader = BufReader::new(&mut tls);
                loop {
                    let mut line = String::new();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                        break;
                    }
                    let lower = line.to_ascii_lowercase();
                    if lower.starts_with("authorization:") {
                        authorization = line["authorization:".len()..].trim().to_string();
                    }
                }
            }
            s.lock().unwrap().push(authorization.clone());
            let (status, body) = if authorization == format!("token {TOKEN}") {
                ("200 OK", format!("{{\"login\":\"{USER}\",\"id\":1}}"))
            } else {
                (
                    "401 Unauthorized",
                    "{\"message\":\"Bad credentials\"}".to_string(),
                )
            };
            let _ = write!(
                tls,
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = tls.flush();
            tls.conn.send_close_notify();
            let _ = tls.flush();
        }
    });
    (port, seen)
}

/// A Broker-J whose AMQP 1.0 port authenticates with the OAuth2 provider
/// and the `GitHubUser` resolver at `identity_port`, trusting the CA in
/// `ca_pem`.
fn oauth_brokerj(identity_port: u16, ca_pem: &std::path::Path) -> BrokerJ {
    let base = format!("https://localhost:{identity_port}");
    let ca = ca_pem.display().to_string();
    BrokerJ::start(|port| {
        format!(
            r#"{{
  "name": "zerodds-test",
  "modelVersion": "8.0",
  "truststores": [ {{
    "name": "testca",
    "type": "NonJavaTrustStore",
    "certificatesUrl": "file://{ca}"
  }} ],
  "authenticationproviders": [ {{
    "name": "oauth",
    "type": "OAuth2",
    "authorizationEndpointURI": "{base}/authorize",
    "tokenEndpointURI": "{base}/token",
    "identityResolverType": "GitHubUser",
    "identityResolverEndpointURI": "{base}/user",
    "trustStore": "testca",
    "clientId": "zerodds",
    "clientSecret": "unused",
    "scope": "user",
    "secureOnlyMechanisms": []
  }} ],
  "brokerloggers": [ {{
    "name": "console",
    "type": "Console",
    "brokerloginclusionrules": [ {{
      "name": "Operational",
      "type": "NameAndLevel",
      "level": "INFO",
      "loggerName": "qpid.message.*"
    }}, {{
      "name": "Qpid",
      "type": "NameAndLevel",
      "level": "WARN",
      "loggerName": "org.apache.qpid.*"
    }} ]
  }} ],
  "ports": [ {{
    "name": "AMQP",
    "port": "{port}",
    "bindingAddress": "127.0.0.1",
    "protocols": [ "AMQP_1_0" ],
    "authenticationProvider": "oauth",
    "virtualhostaliases": [
      {{ "name": "nameAlias", "type": "nameAlias" }},
      {{ "name": "defaultAlias", "type": "defaultAlias" }},
      {{ "name": "hostnameAlias", "type": "hostnameAlias" }}
    ]
  }} ],
  "virtualhostnodes": [ {{
    "name": "default",
    "type": "Memory",
    "defaultVirtualHostNode": "true",
    "virtualHostInitialConfiguration": "{{\"type\": \"Memory\"}}"
  }} ]
}}"#
        )
    })
}

fn params(broker: &BrokerJ, token: &str) -> ConnectParams {
    ConnectParams {
        addr: broker.addr(),
        host: "127.0.0.1".into(),
        tls: None,
        sasl: SaslCredentials::XOAuth2 {
            username: USER.into(),
            token: token.into(),
        },
        container_id: "zerodds-xoauth2".into(),
        hostname: None,
        max_frame_size: 65_536,
        channel_max: 7,
        idle_time_out_ms: 0,
        properties: Vec::new(),
        capabilities: Vec::new(),
        timeout: Duration::from_secs(15),
    }
}

/// §7.2: Broker-J takes the bearer token of the bridge's XOAUTH2 initial
/// response to its identity resolver and admits the connection for the
/// token the resolver knows; a token it does not know is refused in the
/// SASL stage.
#[test]
#[ignore = "needs Apache Qpid Broker-J and a JDK; runs in integration-bridges-vendor-amqp"]
fn amqpbridge_7_2_brokerj_accepts_the_xoauth2_token() {
    let dir = std::env::temp_dir().join(format!("zd-brokerj-pki-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    write_pki(&dir);
    let (identity_port, seen) = identity_endpoint(&dir);
    let broker = oauth_brokerj(identity_port, &dir.join("ca.pem"));
    let (w, _r, remote) = broker::connect(&params(&broker, TOKEN)).unwrap_or_else(|e| {
        panic!(
            "XOAUTH2 with the known token: {e}\nresolver saw {:?}\n{}",
            seen.lock().unwrap(),
            broker.output()
        )
    });
    assert!(!remote.container_id.is_empty());
    w.close(None);
    assert!(
        seen.lock()
            .unwrap()
            .iter()
            .any(|a| a == &format!("token {TOKEN}")),
        "Broker-J asked the resolver with the bridge's token: {:?}",
        seen.lock().unwrap()
    );
    let e = broker::connect(&params(&broker, "a-token-nobody-issued"))
        .err()
        .expect("an unknown token is refused");
    assert_eq!(e.stage, ConnectStage::Sasl, "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}
