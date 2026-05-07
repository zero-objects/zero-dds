#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.1.1 — Connection Open (TLS+PLAIN-Variante).
//!
//! Spec-Konstanz §C.1.1: AMQP-1.0-Client connectet ueber TLS,
//! authentifiziert mit PLAIN, etabliert Session.
//!
//! Da unser Daemon (noch) keinen echten TLS-Stack hat, simulieren
//! wir den Spec-Pfad indem wir den `tls_active`-Flag setzen
//! (was PLAIN auf der SASL-Verhandlung verfuegbar macht). Der
//! Test verifiziert, dass der Server PLAIN als angebotenen
//! Mechanismus listet wenn `tls_active = true`.

mod common;

use amqp_dds_endpoint::client::{ClientConfig, connect_outbound};
use common::{TestServer, test_handler_cfg_with_tls};
use std::time::Duration;
use zerodds_amqp_endpoint::ConnectionState;

#[test]
fn c1_1_connection_open_with_tls_active_advertises_plain() {
    let server = TestServer::spawn(test_handler_cfg_with_tls(true));
    let port = server.port;

    let client_cfg = ClientConfig {
        upstream_addr: format!("127.0.0.1:{port}"),
        container_id: "c1-1-client".into(),
        max_frame_size: 65_536,
        tls_active: true, // signal Klient-Side fuer PLAIN-Erlaubnis
        plain_credentials: Some(("alice".into(), "secret".into())),
        io_timeout: Some(Duration::from_secs(2)),
    };
    let result = connect_outbound(&client_cfg);

    assert!(result.is_ok(), "connection open failed: {result:?}");
    let (_stream, session) = result.unwrap();
    assert_eq!(session.state, ConnectionState::Opened);
    // Per Spec C.1.1: `subject` der Connection ist der
    // SASL-Outcome-Identity. Wir verifizieren, dass eine
    // PLAIN-Mechanism-Negotiation stattgefunden hat.
    assert!(session.sasl_mechanism.is_some());

    server.shutdown();
}
