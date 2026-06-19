// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-vendor interop tests against `grpcurl`.
//!
//! Spec: `zerodds-grpc-bridge-1.0.md` §10 + §12.3.
//!
//! Tests are marked `#[ignore]`. Run via:
//! ```bash
//! cargo test -p zerodds-grpc-bridge --features cross-vendor-tests \
//!     --test cross_vendor -- --ignored
//! ```

#![cfg(feature = "cross-vendor-tests")]
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

use zerodds_grpc_bridge::reflection::{REFLECTION_PATH, encode_list_services};
use zerodds_grpc_bridge::service_gen::{ServiceCatalog, TopicService};

fn grpcurl_available() -> bool {
    Command::new("grpcurl").arg("-version").output().is_ok()
}

#[test]
#[ignore = "requires grpcurl + a running zerodds-grpc-bridged daemon — run via cargo test --features cross-vendor-tests -- --ignored"]
fn grpcurl_list_services_against_zerodds_daemon() {
    if !grpcurl_available() {
        eprintln!("grpcurl not available; skipping");
        return;
    }
    // Caller setup: start `zerodds-grpc-bridged` on 127.0.0.1:50051
    // with `topics: [{ dds_name: Trade, dds_type: Trade, ... }]` via
    // CLI/YAML; this test expects the reflection response
    // `zerodds.bridge.v1.TradeStream`.
    let out = Command::new("grpcurl")
        .args(["-plaintext", "127.0.0.1:50051", "list"])
        .output()
        .expect("grpcurl");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    eprintln!("grpcurl stdout: {stdout}");
    eprintln!("grpcurl stderr: {stderr}");
    assert!(
        stdout.contains("zerodds.bridge.v1.TradeStream") || stdout.contains("TradeStream"),
        "grpcurl list should reveal TradeStream"
    );
}

#[test]
#[ignore = "requires grpcurl + daemon"]
fn grpcurl_describe_service_against_zerodds_daemon() {
    if !grpcurl_available() {
        return;
    }
    let out = Command::new("grpcurl")
        .args([
            "-plaintext",
            "127.0.0.1:50051",
            "describe",
            "zerodds.bridge.v1.TradeStream",
        ])
        .output()
        .expect("grpcurl");
    let _ = out;
    // We don't assert on describe — it requires FileContainingSymbol
    // which is out of RC1-scope (we only ship ListServices).
}

#[test]
fn list_services_reflection_payload_is_well_formed() {
    // Pure-Rust verification of the reflection encoding without
    // requiring grpcurl. Spec §4.6 wire-shape: outer tag 6.
    let mut cat = ServiceCatalog::new();
    cat.register(TopicService::from_topic("Trade", "Trade"));
    cat.register(TopicService::from_topic("Quote", "Quote"));
    let bytes = encode_list_services(&cat);
    assert_eq!(bytes[0], 0x32, "outer tag should be (6<<3|2) = 0x32");
    // Reflection path constant must be the v1alpha-FQN that grpcurl uses.
    assert!(REFLECTION_PATH.contains("ServerReflection/ServerReflectionInfo"));
}
