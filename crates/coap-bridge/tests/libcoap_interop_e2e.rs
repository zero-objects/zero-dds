// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//! Live CoAP interop: the `libcoap` reference client against a responder
//! built from this crate's own codec.
//!
//! `tests/cross_vendor.rs` drives the same client against the daemon, and the
//! daemon sits behind the `daemon` feature and therefore behind
//! `zerodds-dcps`. This test drives the **library half** — `codec`, `message`,
//! `option` and `core_link` — which is the part that owns the octets: libcoap
//! builds every request byte and parses every response byte.
//!
//! Two properties of the reference side are asserted rather than assumed:
//!
//! * **The client exists.** Debian's `libcoap3-bin` installs it as
//!   `coap-client-openssl` / `-gnutls` / `-notls` and **not** as
//!   `coap-client`. A lookup for `coap-client` alone finds nothing on a host
//!   that has libcoap installed, and a test that then returns early reports
//!   success without a single foreign octet having been exchanged.
//! * **The client succeeded.** Its exit status and its stdout are both
//!   checked; a reference client that cannot run must fail this test, not
//!   the code under test.
//!
//! Gated on `COAP_LIBCOAP=1`; `#[ignore]` by default. With the gate set and
//! no client installed the test **fails** rather than passing.

#![cfg(feature = "std")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr,
    clippy::uninlined_format_args
)]

use std::net::UdpSocket;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use zerodds_coap_bridge::codec;
use zerodds_coap_bridge::core_link::{CoreLink, encode_links};
use zerodds_coap_bridge::message::{CoapCode, CoapMessage, MessageType};
use zerodds_coap_bridge::option::{CoapOption, OptionValue, numbers};

/// RFC 7252 §12.3 — `application/link-format`.
const CT_LINK_FORMAT: u16 = 40;
/// RFC 7252 §12.3 — `text/plain;charset=utf-8`.
const CT_TEXT_PLAIN: u16 = 0;

fn enabled() -> bool {
    std::env::var("COAP_LIBCOAP").is_ok()
}

/// The libcoap client under every name Debian and upstream give it.
///
/// Upstream builds `coap-client`; Debian's `libcoap3-bin` ships one binary
/// per TLS backend and no unversioned alias, so a single-name lookup misses
/// an installed libcoap entirely.
fn libcoap_client() -> Option<String> {
    for name in [
        "coap-client",
        "coap-client-openssl",
        "coap-client-gnutls",
        "coap-client-notls",
    ] {
        if Command::new(name).arg("-h").output().is_ok() {
            return Some(name.to_string());
        }
    }
    None
}

/// Resolves the reference client or fails loudly.
fn require_libcoap() -> String {
    libcoap_client().unwrap_or_else(|| {
        panic!(
            "COAP_LIBCOAP is set but no libcoap client was found under any of \
             coap-client / coap-client-openssl / coap-client-gnutls / \
             coap-client-notls — install `libcoap3-bin`. Skipping here would \
             report interop that never happened."
        )
    })
}

/// A minimal RFC 7252 responder over this crate's codec.
struct Responder {
    port: u16,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl Responder {
    fn start() -> Self {
        let sock = UdpSocket::bind("127.0.0.1:0").expect("bind udp");
        let port = sock.local_addr().expect("local_addr").port();
        sock.set_read_timeout(Some(Duration::from_millis(200)))
            .expect("read timeout");
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let join = thread::spawn(move || {
            let mut buf = [0u8; 4096];
            while !stop_thread.load(Ordering::Relaxed) {
                let Ok((n, peer)) = sock.recv_from(&mut buf) else {
                    continue;
                };
                let Ok(req) = codec::decode(&buf[..n]) else {
                    continue;
                };
                let resp = respond(&req);
                let bytes = codec::encode(&resp).expect("encode response");
                let _ = sock.send_to(&bytes, peer);
            }
        });
        Self {
            port,
            stop,
            join: Some(join),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("coap://127.0.0.1:{}/{}", self.port, path)
    }
}

impl Drop for Responder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// Builds the response for one decoded request.
fn respond(req: &CoapMessage) -> CoapMessage {
    let path = req
        .options
        .iter()
        .filter(|o| o.number == numbers::URI_PATH)
        .map(|o| match &o.value {
            OptionValue::String(s) => s.clone(),
            other => String::from_utf8_lossy(&other.to_wire_bytes()).into_owned(),
        })
        .collect::<Vec<_>>()
        .join("/");

    // RFC 7252 §5.2.1 — a Confirmable request is answered piggybacked in the
    // Acknowledgement; a Non-Confirmable one with a Non-Confirmable response.
    let message_type = match req.message_type {
        MessageType::Confirmable => MessageType::Acknowledgement,
        _ => MessageType::NonConfirmable,
    };

    let (code, ct, payload) = match (req.code, path.as_str()) {
        (CoapCode::GET, ".well-known/core") => (
            CoapCode::CONTENT,
            CT_LINK_FORMAT,
            encode_links(&[CoreLink::new("/trade")
                .attr("rt", "dds.topic")
                .attr("ct", "0")])
            .into_bytes(),
        ),
        (CoapCode::GET, "trade") => (CoapCode::CONTENT, CT_TEXT_PLAIN, b"AAPL@200".to_vec()),
        (CoapCode::POST, "trade") => (CoapCode::CHANGED, CT_TEXT_PLAIN, req.payload.clone()),
        _ => (CoapCode::new(4, 4), CT_TEXT_PLAIN, Vec::new()),
    };

    CoapMessage {
        version: 1,
        message_type,
        code,
        message_id: req.message_id,
        token: req.token.clone(),
        options: vec![CoapOption::content_format(ct)],
        payload,
    }
}

/// Runs the reference client and returns its stdout, failing on a
/// non-zero exit.
fn run_client(bin: &str, args: &[&str]) -> String {
    let out = Command::new(bin)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn {bin}: {e}"));
    assert!(
        out.status.success(),
        "the libcoap reference client failed ({bin} {args:?}): status={:?} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
#[ignore = "needs libcoap (apt install libcoap3-bin); set COAP_LIBCOAP=1"]
fn libcoap_get_reads_a_zerodds_encoded_response() {
    if !enabled() {
        eprintln!("[skip] COAP_LIBCOAP not set");
        return;
    }
    let bin = require_libcoap();
    let srv = Responder::start();
    let got = run_client(&bin, &["-m", "get", &srv.url("trade")]);
    assert!(
        got.contains("AAPL@200"),
        "libcoap must read the payload this crate's codec wrote; got: {got:?}"
    );
    eprintln!("OK: {bin} -m get read a ZeroDDS-encoded 2.05 Content");
}

#[test]
#[ignore = "needs libcoap; set COAP_LIBCOAP=1"]
fn libcoap_post_is_decoded_and_echoed() {
    if !enabled() {
        eprintln!("[skip] COAP_LIBCOAP not set");
        return;
    }
    let bin = require_libcoap();
    let srv = Responder::start();
    // libcoap builds the request octets; the payload comes back only if this
    // crate's decoder read them.
    let got = run_client(&bin, &["-m", "post", "-e", "MSFT@42", &srv.url("trade")]);
    assert!(
        got.contains("MSFT@42"),
        "the POST payload libcoap wrote must survive decode+encode; got: {got:?}"
    );
    eprintln!("OK: {bin} -m post round-tripped a payload through the codec");
}

#[test]
#[ignore = "needs libcoap; set COAP_LIBCOAP=1"]
fn libcoap_reads_the_well_known_core_catalog() {
    if !enabled() {
        eprintln!("[skip] COAP_LIBCOAP not set");
        return;
    }
    let bin = require_libcoap();
    let srv = Responder::start();
    let got = run_client(&bin, &["-m", "get", &srv.url(".well-known/core")]);
    assert!(
        got.contains("</trade>"),
        "RFC 6690 catalog must reach libcoap; got: {got:?}"
    );
    assert!(
        got.contains("rt=\"dds.topic\""),
        "link-format attributes must survive; got: {got:?}"
    );
    eprintln!("OK: {bin} parsed an RFC 6690 link-format catalog from `core_link`");
}
