// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Productive DTLS 1.2 e2e (feature `dtls`): a real webrtc-dtls handshake
//! between [`WebrtcDtlsServer`] and [`WebrtcDtls`] over loopback UDP, then an
//! application round-trip through the sync [`DtlsLayer`] trait. Proves the
//! §3.1.9 DTLS transport profile carries XRCE plaintext over an encrypted
//! record layer (not the `DummyDtls` pass-through).

#![cfg(feature = "dtls")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::thread;

use zerodds_xrce::transport_dtls::{DtlsLayer, WebrtcDtls, WebrtcDtlsServer};

#[test]
fn dtls_handshake_and_roundtrip() {
    // Server binds with a fresh self-signed cert on an ephemeral port.
    let server = WebrtcDtlsServer::bind("127.0.0.1:0".parse().unwrap()).expect("bind");
    let addr = server.local_addr();

    // Server thread: accept one DTLS connection, echo one message back.
    let srv = thread::spawn(move || {
        let mut sess = server.accept().expect("accept");
        let got = sess.recv().expect("server recv");
        // Echo with a marker so the client can verify decryption end-to-end.
        let mut reply = b"echo:".to_vec();
        reply.extend_from_slice(&got);
        sess.send(&reply).expect("server send");
        got
    });

    // Client: dial, complete the handshake (verify=false accepts the self-signed
    // cert), send a plaintext payload, receive the echoed reply.
    let mut client = WebrtcDtls::connect(addr, "localhost", false).expect("connect");
    assert!(client.is_handshake_complete());
    let payload = b"hello-xrce-over-dtls";
    client.send(payload).expect("client send");
    let reply = client.recv().expect("client recv");
    client.close().ok();

    let server_saw = srv.join().expect("server thread");
    assert_eq!(
        server_saw, payload,
        "server decrypted the client's plaintext"
    );
    assert_eq!(
        reply, b"echo:hello-xrce-over-dtls",
        "client decrypted the echo"
    );
}
