// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-vendor interop tests against libcoap.
//!
//! Spec: `zerodds-coap-bridge-1.0.md` §10 + §12.3.
//!
//! These tests bring up a `libcoap` container (or local
//! `coap-client` binary) and verify that:
//! * `coap-client -m post coap://127.0.0.1:<port>/trade` receives a
//!   2.04 Changed (wire compliance against RFC 7252).
//! * `coap-client -s 5 -o coap://127.0.0.1:<port>/trade` (Observe)
//!   receives a notify on sample publish.
//!
//! Since Docker is not guaranteed in CI, the tests are marked
//! `#[ignore]`. Run via:
//! ```bash
//! cargo test -p zerodds-coap-bridge --features cross-vendor-tests \
//!     --test cross_vendor -- --ignored
//! ```

#![cfg(all(feature = "daemon", feature = "cross-vendor-tests"))]
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

use std::process::Command;

use zerodds_coap_bridge::daemon::config::{DaemonConfig, TopicConfig};
use zerodds_coap_bridge::daemon::server;

fn make_test_config(bind: &str) -> DaemonConfig {
    let mut cfg = DaemonConfig::default_for_dev();
    cfg.bind = bind.to_string();
    cfg.domain = 99;
    cfg.topics.push(TopicConfig {
        dds_name: "Trade".to_string(),
        dds_type: "Trade".to_string(),
        coap_uri_path: "trade".to_string(),
        direction: "bidir".to_string(),
        reliability: "best_effort".to_string(),
        durability: "volatile".to_string(),
        history_depth: 10,
    });
    cfg
}

/// The libcoap client under every name Debian and upstream give it.
///
/// This looked for `coap-client` alone. Debian's `libcoap3-bin` installs one
/// binary per TLS backend — `coap-client-openssl`, `-gnutls`, `-notls` — and
/// registers no unversioned alias, so on a host with libcoap installed the
/// lookup found nothing, every test below returned early, and the suite
/// reported success with no foreign octet exchanged. Measured on the Linux test host
/// against libcoap 4.3.4.
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

#[test]
#[ignore = "requires libcoap (apt install libcoap3-bin) — run via cargo test --features cross-vendor-tests -- --ignored"]
fn libcoap_post_to_zerodds_daemon_returns_2_04_changed() {
    // Run with `--ignored` means measure: a missing client fails instead
    // of reporting interop that never happened.
    let bin = libcoap_client().expect("no libcoap client found; install libcoap3-bin");
    let cfg = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let port = handle.local_addr.rsplit(':').next().expect("port");

    let url = format!("coap://127.0.0.1:{port}/trade");
    // `-v 6`, not `-v 3`. Measured on the Linux test host against libcoap 4.3.4: at
    // `-v 3` the client prints the response payload and nothing else — no
    // response code on either stream — so an assertion on "2.04" at that
    // level can never hold. At `-v 6` it prints the compact message trace,
    // `v:1 t:ACK c:2.04 i:… `, on **stdout**; libcoap sends its whole log to
    // stdout, so stderr stays empty.
    let out = Command::new(&bin)
        .args(["-v", "6", "-m", "post", "-e", "AAPL@200", &url])
        .output()
        .expect("coap-client");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    eprintln!("{bin} stdout: {stdout}");
    eprintln!("{bin} stderr: {stderr}");
    assert!(
        out.status.success(),
        "{bin} should exit 0; stderr: {stderr}"
    );
    // The test is named for 2.04 Changed and nothing checked it: libcoap
    // exits 0 on any response it can parse, including 4.04. `c:2.04` is the
    // code field of the trace line for the response message.
    assert!(
        stdout.contains("c:2.04") || stderr.contains("c:2.04"),
        "expected a 2.04 Changed from the daemon; stdout={stdout} stderr={stderr}"
    );

    handle.shutdown();
}

#[test]
#[ignore = "requires libcoap-bin"]
fn libcoap_get_well_known_core_returns_catalog() {
    // Run with `--ignored` means measure: a missing client fails instead
    // of reporting interop that never happened.
    let bin = libcoap_client().expect("no libcoap client found; install libcoap3-bin");
    let cfg = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let port = handle.local_addr.rsplit(':').next().expect("port");
    let url = format!("coap://127.0.0.1:{port}/.well-known/core");
    let out = Command::new(&bin)
        .args(["-m", "get", &url])
        .output()
        .expect("coap-client");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{bin} should exit 0; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        stdout.contains("</trade>"),
        "expected </trade> in catalog: {stdout}"
    );
    handle.shutdown();
}

#[test]
#[ignore = "requires libcoap-bin + a publish source — composite via docker-compose"]
fn libcoap_observe_receives_notification() {
    // Run with `--ignored` means measure: a missing client fails instead
    // of reporting interop that never happened.
    let bin = libcoap_client().expect("no libcoap client found; install libcoap3-bin");
    let cfg = make_test_config("127.0.0.1:0");
    let mut handle = server::start(cfg).expect("daemon start");
    let port = handle.local_addr.rsplit(':').next().expect("port");
    let url = format!("coap://127.0.0.1:{port}/trade");
    // Observe for 2 seconds. `-s <duration>` is libcoap's Observe flag and
    // is the whole of it; `-o` is `[-o file]`, an output-file argument.
    // `-s 2 -o <url>` therefore consumed the URL as a filename, left the
    // client with no address, and it exited on its usage message —
    // measured against coap-client-openssl 4.3.4.
    let out = Command::new(&bin)
        .args(["-v", "6", "-s", "2", &url])
        .output()
        .expect("coap-client");
    // This was `let _ = out;` under a comment reading "Functional assert:
    // process exited cleanly" — there was no assert, so the test passed
    // whatever libcoap did, including failing to reach the daemon at all.
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "{bin} -s should exit 0; stdout={stdout} stderr={stderr}"
    );
    // A server that does not implement Observe answers the same request with
    // a plain 2.05 and no Observe option, and libcoap exits 0 on it — so a
    // non-empty stdout proves nothing. RFC 7641 §4.2: the registration is
    // accepted only if the **response** carries the Observe option. At
    // `-v 6` libcoap prints one trace line per message; the request line
    // carries `Observe:` too, so the response line has to be picked out by
    // its code.
    let accepted = stdout
        .lines()
        .any(|l| l.contains("c:2.05") && l.contains("Observe"));
    assert!(
        accepted,
        "the daemon must accept the Observe registration and answer with the \
         Observe option (RFC 7641 §4.2); stdout={stdout} stderr={stderr}"
    );
    handle.shutdown();
}
