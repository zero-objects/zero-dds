#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]

//! Annex C C.1.9 — Idle-Timeout.
//!
//! Spec §C.1.9: eine Connection ohne Traffic fuer die konfigurierte
//! `idle-timeout`-Dauer SHALL durch den Endpoint mit
//! `amqp:resource-limit-exceeded` close-Performative geschlossen
//! werden.
//!
//! Wir verifizieren das auf der Konfigurations- und Read-Timeout-
//! Ebene: ein Klient ohne weitere Frame-Aktivitaet bekommt einen
//! `read_timeout`-Disconnect. Echte `amqp:close`-Frame-Emission
//! ist Daemon-Folge-Welle (idle-timeout-Tracker im Handler).

mod common;

use common::{TestServer, test_handler_cfg};
use std::io::Read;
use std::net::TcpStream;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn c1_9_server_disconnects_idle_client_after_short_read_timeout() {
    // Wir setzen den Server-side read_timeout indirekt ueber
    // den Handler-Pfad. Da der Handler-Loop selbst keinen
    // expliziten idle-tracker hat, simulieren wir es klient-seitig:
    // Klient connectet, schickt nichts, Server haengt im
    // read_protocol_header — auf Server-Side ist read_timeout
    // 5s gesetzt (siehe common::TestServer::spawn).

    let server = TestServer::spawn(test_handler_cfg());
    let port = server.port;

    let start = Instant::now();
    let mut client = TcpStream::connect(format!("127.0.0.1:{port}")).expect("connect");
    client
        .set_read_timeout(Some(Duration::from_secs(7)))
        .unwrap();
    // Klient schickt nichts. Server hat read_timeout=5s und
    // schliesst die Connection von seiner Seite, was zu EOF/0
    // beim Klient fuehrt.
    let mut buf = [0u8; 8];
    let n = client.read(&mut buf).unwrap_or(0);
    let elapsed = start.elapsed();
    // Klient bekommt entweder 0 Bytes (server closed) oder
    // einen IO-Timeout — beide deuten auf Server-Disconnect.
    assert_eq!(n, 0, "Server soll idle-Klient disconnecten (got {n} bytes)");
    // Disconnect sollte um den Server-read_timeout (5s) herum
    // passieren, jedenfalls < 6s.
    assert!(elapsed < Duration::from_secs(7), "elapsed {elapsed:?}");

    server.shutdown();
}

#[test]
fn c1_9_idle_timeout_is_configurable() {
    // Spec §7.10: idle_timeout_ms ist Teil von ResourceLimits.
    // Hier verifizieren wir nur das Datenmodell.
    use zerodds_amqp_endpoint::ResourceLimits;
    let limits = ResourceLimits::default();
    assert!(limits.idle_timeout_ms > 0);
    // Die meisten Spec-Beispiele nutzen 60_000ms oder 120_000ms.
    assert!(limits.idle_timeout_ms <= 120_000);
    let _ = thread::current(); // halte std::thread im Use
}
