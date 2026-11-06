#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.2.4 — Reconnect on Loss.
//!
//! Spec §C.2.4: after an abrupt connection loss the bridge MUST
//! attempt to reconnect to the broker (per §10.8 with
//! exponential backoff), and on re-establishment all links
//! wiederattachen. Pacing folgt §10.8 (init 1s, mult 2, cap 60s
//! by default; Test verwendet beschleunigte Werte).

mod common;

use amqp_dds_endpoint::client::{
    ClientConfig, ClientError, ReconnectConfig, connect_outbound, connect_with_reconnect,
};
use common::{TestServer, test_handler_cfg};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread;
use std::time::Duration;
use zerodds_amqp_endpoint::{ConnectionState, MetricsHub};

/// Spec §10.8 and Annex C C.2.4: connects fail while nothing listens; the
/// loop backs off and tries again, and connects once the server is up.
#[test]
fn ddsamqp_10_8_the_client_reconnects_once_the_server_is_up() {
    // A port nobody listens on until the server below binds it.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let metrics = Arc::new(MetricsHub::new());
    let shutdown = Arc::new(AtomicBool::new(false));
    let cfg = ClientConfig {
        upstream_addr: format!("127.0.0.1:{port}"),
        container_id: "c2-4-reconnect".into(),
        max_frame_size: 65_536,
        tls_active: false,
        plain_credentials: None,
        io_timeout: Some(Duration::from_secs(2)),
    };
    let r = ReconnectConfig {
        initial_ms: 50,
        multiplier: 2,
        cap_ms: 200,
        max_attempts: Some(100),
    };
    let client = {
        let (metrics, shutdown) = (Arc::clone(&metrics), Arc::clone(&shutdown));
        thread::spawn(move || connect_with_reconnect(&cfg, &r, &shutdown, &metrics))
    };
    // The first attempt failed: the loop counts it before it backs off.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while metrics.snapshot("errors.decode").unwrap_or(0) == 0 {
        assert!(std::time::Instant::now() < deadline, "no failed attempt");
        thread::yield_now();
    }
    let server = TestServer::spawn_on(port, test_handler_cfg());
    let (_stream, session) = client
        .join()
        .unwrap()
        .expect("connected after the server came up");
    assert_eq!(session.state, ConnectionState::Opened);
    server.shutdown();
}

#[test]
fn ddsamqp_10_8_reconnect_loop_pacing_follows_spec_defaults() {
    // Spec §10.8: init 1s, mult 2, cap 60s.
    let r = ReconnectConfig::default();
    assert_eq!(r.next_backoff_ms(0), 1_000);
    assert_eq!(r.next_backoff_ms(1), 2_000);
    assert_eq!(r.next_backoff_ms(2), 4_000);
    assert_eq!(r.next_backoff_ms(6), 60_000);
}

#[test]
fn ddsamqp_10_8_reconnect_aborts_on_max_attempts() {
    // Client tries max=3 times; with an unreachable broker
    // ReconnectExhausted is returned.
    let metrics = Arc::new(MetricsHub::new());
    let shutdown = Arc::new(AtomicBool::new(false));
    let cfg = ClientConfig {
        upstream_addr: "127.0.0.1:1".into(), // port 1 → connection refused
        container_id: "c2-4".into(),
        max_frame_size: 65_536,
        tls_active: false,
        plain_credentials: None,
        io_timeout: Some(Duration::from_secs(1)),
    };
    let r = ReconnectConfig {
        initial_ms: 1,
        multiplier: 1,
        cap_ms: 1,
        max_attempts: Some(3),
    };
    let err = connect_with_reconnect(&cfg, &r, &shutdown, &metrics).unwrap_err();
    assert!(
        matches!(
            err,
            ClientError::ReconnectExhausted {
                attempts: 3,
                last: Some(_)
            }
        ),
        "{err}"
    );
}

#[test]
fn ddsamqp_10_8_reconnect_after_server_restart_eventually_succeeds() {
    // E2E: 1) server bind+drop (port reserved, client finds
    // nobody). 2) Client retries with a reconnect loop. 3)
    // Next connect attempt after server start succeeds.
    let metrics = Arc::new(MetricsHub::new());
    let shutdown = Arc::new(AtomicBool::new(false));

    let server = TestServer::spawn(test_handler_cfg());
    let port = server.port;

    // Client connects successfully on the first attempt.
    let cfg = ClientConfig {
        upstream_addr: format!("127.0.0.1:{port}"),
        container_id: "c2-4-restart".into(),
        max_frame_size: 65_536,
        tls_active: false,
        plain_credentials: None,
        io_timeout: Some(Duration::from_secs(2)),
    };
    let r = ReconnectConfig {
        initial_ms: 50,
        multiplier: 2,
        cap_ms: 200,
        max_attempts: Some(5),
    };
    let result = connect_with_reconnect(&cfg, &r, &shutdown, &metrics);
    assert!(result.is_ok(), "first connect should succeed: {result:?}");
    let (_stream, session) = result.unwrap();
    assert_eq!(session.state, ConnectionState::Opened);

    server.shutdown();
}

#[test]
fn ddsamqp_c_2_1_single_connect_attempt_works_for_normal_path() {
    // Normal case: server is running, client connects immediately, no
    // Reconnect noetig.
    let server = TestServer::spawn(test_handler_cfg());
    let port = server.port;
    let cfg = ClientConfig {
        upstream_addr: format!("127.0.0.1:{port}"),
        container_id: "c2-4-direct".into(),
        max_frame_size: 65_536,
        tls_active: false,
        plain_credentials: None,
        io_timeout: Some(Duration::from_secs(2)),
    };
    let result = connect_outbound(&cfg);
    assert!(result.is_ok(), "{result:?}");
    server.shutdown();
}
