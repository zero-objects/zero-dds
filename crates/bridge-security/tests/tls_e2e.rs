// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! End-to-end TLS handshake smoke: spawns a mini server with
//! `zerodds-bridge-security::load_server_config`, connects a
//! `rustls::ClientConnection` over loopback, and verifies that the
//! handshake completes through the `with_single_cert` path.
//!
//! Acceptance path for Bridge spec §7.1 TLS: spawn a daemon with a
//! self-signed cert (rcgen), connect with `rustls::ClientConfig`,
//! assert TLS handshake OK.

#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::field_reassign_with_default,
    clippy::manual_flatten,
    clippy::collapsible_if,
    clippy::empty_line_after_doc_comments,
    clippy::uninlined_format_args,
    clippy::drop_non_drop,
    missing_docs
)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rustls::ClientConfig;
use rustls_pki_types::{CertificateDer, ServerName};
use zerodds_bridge_security::tls::load_server_config;

fn write_temp(name: &str, body: &[u8]) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("zd-bridge-tls-e2e-{}-{}", name, std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join(name);
    let mut f = std::fs::File::create(&p).unwrap();
    f.write_all(body).unwrap();
    p
}

fn gen_self_signed_pem() -> (String, String, Vec<u8>) {
    let ck = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_der = ck.cert.der().to_vec();
    (ck.cert.pem(), ck.key_pair.serialize_pem(), cert_der)
}

#[test]
fn rustls_handshake_succeeds_through_loopback() {
    let (cert_pem, key_pem, cert_der) = gen_self_signed_pem();
    let c = write_temp("e2e_cert.pem", cert_pem.as_bytes());
    let k = write_temp("e2e_key.pem", key_pem.as_bytes());

    let server_cfg = load_server_config(&c, &k).expect("server cfg");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let local = listener.local_addr().expect("local-addr");

    let server_thread = thread::spawn(move || {
        let (mut stream, _peer) = listener.accept().expect("accept");
        let mut conn = rustls::ServerConnection::new(server_cfg).expect("server-conn");
        // complete_io drives handshake + reads any pending app-bytes.
        conn.complete_io(&mut stream).expect("server handshake");
        let mut buf = [0u8; 16];
        let n = conn.reader().read(&mut buf).unwrap_or(0);
        let _ = n; // not strictly required for handshake assert
        conn.send_close_notify();
        let _ = conn.complete_io(&mut stream);
    });

    // Client side.
    let mut roots = rustls::RootCertStore::empty();
    roots.add(CertificateDer::from(cert_der)).expect("add root");
    let client_cfg = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server_name = ServerName::try_from("localhost").unwrap().to_owned();
    let mut client_conn =
        rustls::ClientConnection::new(Arc::new(client_cfg), server_name).expect("client-conn");
    let mut sock = TcpStream::connect_timeout(&local, Duration::from_secs(5)).expect("connect");
    client_conn
        .complete_io(&mut sock)
        .expect("client handshake");
    let _ = client_conn.writer().write_all(b"hi");
    let _ = client_conn.complete_io(&mut sock);
    client_conn.send_close_notify();
    let _ = client_conn.complete_io(&mut sock);

    server_thread.join().expect("server thread panicked");
}
