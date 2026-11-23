// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! zerodds-mqtt-bridge-1.0 §7.2 against EMQX 5: `auth.mode: enhanced` with
//! SCRAM-SHA-256 towards EMQX's own SCRAM authenticator (built-in database,
//! SHA-256, 4096 iterations), which reads the daemon's client-first and
//! client-final messages and signs the server-final message the daemon
//! checks; and `password` with a JWT in `auth.bearer_token` towards EMQX's
//! JWT authenticator (HMAC SHA-256, the token taken from the CONNECT
//! password). The AUTH packets, the properties and the SCRAM messages are
//! encoded and decoded on the broker's side by EMQX, not by this crate.
//!
//! Needs a running EMQX 5: its MQTT listener at `ZERODDS_EMQX`
//! (`host:port`) and its REST API at `ZERODDS_EMQX_API` (`host:port`, the
//! default dashboard user `admin`/`public`). The test sets up the two
//! authenticators and the SCRAM user through the API. A missing variable
//! fails the test instead of passing it.
//!
//! Not here: OAUTHBEARER, for which EMQX 5.8.6 CE has no authenticator (its
//! API answers `unsupported_mechanism`), and the re-authentication of MQTT
//! 5.0 §4.12.1, which EMQX 5.8.6 runs through its SCRAM authenticator up to
//! the success and then answers without AUTH 0x00, ending the connection
//! five seconds later (`session_stepdown_request_timeout` in its log).
//!
//! ```bash
//! docker run -d --rm --name emqx -p 1883:1883 -p 18083:18083 emqx/emqx:5.8.6
//! ZERODDS_EMQX=127.0.0.1:1883 ZERODDS_EMQX_API=127.0.0.1:18083 \
//!     cargo test -p zerodds-mqtt-bridge --features cross-vendor-tests \
//!     --test emqx_auth -- --ignored --test-threads=1
//! ```

#![cfg(all(feature = "daemon", feature = "cross-vendor-tests"))]
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    missing_docs
)]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use ring::hmac;

use zerodds_dcps::runtime::{UserReaderConfig, UserSample};
use zerodds_mqtt_bridge::daemon::auth::AuthKind;
use zerodds_mqtt_bridge::daemon::client::MqttClient;
use zerodds_mqtt_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_mqtt_bridge::daemon::server;

/// The secret of EMQX's JWT authenticator.
const JWT_SECRET: &str = "zerodds-emqx-jwt-secret";
/// The SCRAM user in EMQX's built-in database.
const SCRAM_USER: &str = "zerodds-bridge";
const SCRAM_PASSWORD: &str = "pencil-7f3a";
/// The SCRAM authenticator's id in the REST API, `scram:built_in_database`.
const SCRAM_ID: &str = "scram%3Abuilt_in_database";

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!("{name} is not set; this test measures against a running EMQX 5")
    })
}

/// `host` and `port` of `ZERODDS_EMQX`.
fn broker() -> (String, u16) {
    let addr = env("ZERODDS_EMQX");
    let (host, port) = addr.rsplit_once(':').expect("ZERODDS_EMQX is host:port");
    (host.to_string(), port.parse().expect("a port"))
}

/// One HTTP/1.1 request to the REST API: its status and body, or why
/// there was none.
fn try_http(
    method: &str,
    path: &str,
    token: Option<&str>,
    body: &str,
) -> Result<(u16, String), String> {
    let api = env("ZERODDS_EMQX_API");
    let mut s = TcpStream::connect(&api).map_err(|e| format!("EMQX API {api}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|e| e.to_string())?;
    let auth = token.map_or(String::new(), |t| format!("Authorization: Bearer {t}\r\n"));
    write!(
        s,
        "{method} /api/v5{path} HTTP/1.1\r\nHost: {api}\r\n{auth}Content-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .map_err(|e| e.to_string())?;
    let mut raw = String::new();
    s.read_to_string(&mut raw).map_err(|e| e.to_string())?;
    let status = raw
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| format!("no HTTP status in {raw:?}"))?;
    let body = raw
        .split_once("\r\n\r\n")
        .map_or("", |(_, b)| b)
        .to_string();
    Ok((status, body))
}

/// [`try_http`], which must answer.
fn http(method: &str, path: &str, token: Option<&str>, body: &str) -> (u16, String) {
    try_http(method, path, token, body).unwrap_or_else(|e| panic!("{e}"))
}

/// Logs in, then sets up the SCRAM authenticator with its user and the JWT
/// authenticator (each kept when it exists from an earlier run).
fn set_up_emqx() {
    // A broker that has just started answers the API last.
    let deadline = Instant::now() + Duration::from_secs(20);
    let token = loop {
        let answer = try_http(
            "POST",
            "/login",
            None,
            r#"{"username":"admin","password":"public"}"#,
        );
        if let Ok((200, body)) = &answer {
            let rest = body.split_once(r#""token":""#).expect("a token").1;
            break rest.split('"').next().unwrap().to_string();
        }
        assert!(Instant::now() < deadline, "EMQX login: {answer:?}");
        std::thread::sleep(Duration::from_millis(500));
    };
    // An authenticator that exists from an earlier run is kept; a broker
    // that has just started may refuse the first configuration change.
    let create = |body: &str| loop {
        let answer = try_http("POST", "/authentication", Some(&token), body);
        if let Ok((status, text)) = &answer
            && (matches!(status, 200 | 201) || text.contains("ALREADY_EXISTS"))
        {
            return;
        }
        assert!(Instant::now() < deadline, "EMQX authenticator: {answer:?}");
        std::thread::sleep(Duration::from_millis(500));
    };
    create(
        r#"{"mechanism":"scram","backend":"built_in_database","algorithm":"sha256","iteration_count":4096}"#,
    );
    scram_user(&token, SCRAM_USER, SCRAM_PASSWORD);
    create(&format!(
        r#"{{"mechanism":"jwt","from":"password","use_jwks":false,"algorithm":"hmac-based","secret":"{JWT_SECRET}","secret_base64_encoded":false,"verify_claims":{{}}}}"#
    ));
}

/// The SCRAM user `user` with `password` in EMQX's built-in database,
/// created or given the password.
fn scram_user(token: &str, user: &str, password: &str) {
    let t = Some(token);
    let (status, body) = http(
        "POST",
        &format!("/authentication/{SCRAM_ID}/users"),
        t,
        &format!(r#"{{"user_id":"{user}","password":"{password}"}}"#),
    );
    if body.contains("ALREADY_EXISTS") {
        let (status, body) = http(
            "PUT",
            &format!("/authentication/{SCRAM_ID}/users/{user}"),
            t,
            &format!(r#"{{"password":"{password}"}}"#),
        );
        assert_eq!(status, 200, "{body}");
    } else {
        assert!(matches!(status, 200 | 201), "{status} {body}");
    }
}

/// A JWT with `sub` and an expiry an hour ahead, signed with HMAC SHA-256
/// under `secret`.
fn jwt(sub: &str, secret: &str) -> String {
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    let header = B64URL.encode(br#"{"alg":"HS256","typ":"JWT"}"#);
    let claims = B64URL.encode(format!(r#"{{"sub":"{sub}","exp":{exp}}}"#));
    let input = format!("{header}.{claims}");
    let sig = hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes()),
        input.as_bytes(),
    );
    format!("{input}.{}", B64URL.encode(sig.as_ref()))
}

fn scram_cfg(password: &str) -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.client_id = "zerodds-emqx-scram".into();
    cfg.auth_mode = Some(AuthKind::Enhanced);
    cfg.auth_enhanced_method = Some("SCRAM-SHA-256".into());
    cfg.outbound_username = Some(SCRAM_USER.into());
    cfg.outbound_password = Some(password.into());
    cfg
}

fn jwt_cfg(token: String) -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.client_id = "zerodds-emqx-jwt".into();
    cfg.auth_mode = AuthKind::parse("bearer");
    cfg.auth_bearer_token = Some(token);
    cfg
}

/// `cfg` as a daemon towards EMQX on a leased domain with `topic`.
fn daemon(
    mut cfg: DaemonConfig,
    topic: TopicConfig,
) -> (DaemonConfig, zerodds_test_domains::DomainLease) {
    let (host, port) = broker();
    let lease = zerodds_test_domains::lease_domain();
    cfg.domain = lease.id();
    cfg.broker_url = format!("mqtt://{host}:{port}");
    cfg.session_dir = std::env::temp_dir()
        .join(format!(
            "zerodds-mqtt-emqx-{}-{}",
            cfg.client_id,
            std::process::id()
        ))
        .display()
        .to_string();
    cfg.drain_timeout_secs = 2;
    cfg.topics.push(topic);
    (cfg, lease)
}

fn topic(dds: &str, mqtt: &str, direction: &str) -> TopicConfig {
    TopicConfig {
        dds_name: dds.to_string(),
        dds_type: dds.to_string(),
        mqtt_topic: mqtt.to_string(),
        direction: direction.to_string(),
        reliability: "reliable".to_string(),
        durability: "volatile".to_string(),
        ..TopicConfig::default()
    }
}

/// Spec §7.2 against EMQX 5: a daemon under `enhanced` with SCRAM-SHA-256
/// connects with EMQX's SCRAM authenticator (the start succeeds only after
/// the server signature of EMQX's CONNACK verified), a daemon under
/// `password` with a JWT as `auth.bearer_token` with EMQX's JWT
/// authenticator; a sample published by the first reaches DDS through the
/// second.
#[test]
#[ignore = "needs a running EMQX 5 (ZERODDS_EMQX, ZERODDS_EMQX_API); job integration-bridges-vendor"]
fn mqttbridge_7_2_scram_sha_256_and_a_jwt_connect_to_emqx() {
    set_up_emqx();
    let (cfg_in, _lease_in) = daemon(
        jwt_cfg(jwt("zerodds-jwt", JWT_SECRET)),
        topic("EmqxAuth", "zerodds/emqx/auth", "in"),
    );
    let mut sub = server::start(cfg_in).expect("the JWT daemon connects to EMQX");
    let rx = sub
        .runtime
        .register_user_reader(UserReaderConfig {
            topic_name: "EmqxAuth".into(),
            type_name: "EmqxAuth".into(),
            reliable: true,
            ..Default::default()
        })
        .unwrap()
        .1;
    let (cfg_out, _lease_out) = daemon(
        scram_cfg(SCRAM_PASSWORD),
        topic("EmqxAuth", "zerodds/emqx/auth", "out"),
    );
    let mut publisher = server::start(cfg_out).expect("the SCRAM daemon connects to EMQX");
    let deadline = Instant::now() + Duration::from_secs(10);
    let got = loop {
        assert!(
            publisher
                .runtime
                .test_inject_user_alive(publisher.user_readers["EmqxAuth"], vec![0x5A])
        );
        if let Ok(UserSample::Alive { payload, .. }) = rx.recv_timeout(Duration::from_millis(500)) {
            break payload.to_vec();
        }
        assert!(Instant::now() < deadline, "no sample through EMQX");
    };
    assert_eq!(got, [0x5A]);
    publisher.shutdown();
    sub.shutdown();
}

/// Spec §7.2 against EMQX 5: EMQX refuses a SCRAM client-final message
/// whose proof comes from another password, and a JWT signed with another
/// secret; both are refusals of the credentials (exit 5).
#[test]
#[ignore = "needs a running EMQX 5 (ZERODDS_EMQX, ZERODDS_EMQX_API); job integration-bridges-vendor"]
fn mqttbridge_7_2_emqx_refuses_a_wrong_scram_password_and_a_forged_jwt() {
    set_up_emqx();
    let (host, port) = broker();
    let mut good = scram_cfg(SCRAM_PASSWORD);
    good.client_id = "zerodds-emqx-scram-check".into();
    let c = MqttClient::connect(&host, port, &good).expect("the right password");
    assert_eq!(c.auth_method(), Some("SCRAM-SHA-256"));
    drop(c);
    let mut wrong = scram_cfg("rubber");
    wrong.client_id = "zerodds-emqx-scram-wrong".into();
    match MqttClient::connect(&host, port, &wrong) {
        Ok(_) => panic!("EMQX took a wrong SCRAM password"),
        Err(e) => assert!(e.is_auth_refusal(), "{e}"),
    }
    let mut forged = jwt_cfg(jwt("zerodds-jwt", "another-secret"));
    forged.client_id = "zerodds-emqx-jwt-forged".into();
    match MqttClient::connect(&host, port, &forged) {
        Ok(_) => panic!("EMQX took a JWT of another secret"),
        Err(e) => assert!(e.is_auth_refusal(), "{e}"),
    }
}
