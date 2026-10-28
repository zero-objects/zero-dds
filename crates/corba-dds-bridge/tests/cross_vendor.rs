// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-vendor interop tests against omniORB (Spec §10 + §12.3).
//!
//! Run via:
//! ```bash
//! cargo test -p zerodds-corba-dds-bridge --features cross-vendor-tests \
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

fn omniidl_available() -> bool {
    Command::new("omniidl").arg("-V").output().is_ok()
}

fn catior_available() -> bool {
    Command::new("catior").arg("--help").output().is_ok()
}

#[test]
#[ignore = "requires omniORB tools (apt install omniorb-tools)"]
fn omniidl_smoke_check_present() {
    if !omniidl_available() {
        eprintln!("omniidl not available; skipping");
        return;
    }
    let out = Command::new("omniidl").arg("-V").output().expect("omniidl");
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stderr.contains("omniidl") || stdout.contains("omniidl"),
        "omniidl -V should mention omniidl"
    );
}

/// omniORB's `catior` reads the IOR the daemon prints: the type id, the
/// IIOP profile's host and port.
#[test]
#[ignore = "requires catior (omniORB tools)"]
fn catior_reads_the_ior_the_daemon_prints() {
    assert!(
        catior_available(),
        "catior is not installed; this test measures against it"
    );
    let out = Command::new(env!("CARGO_BIN_EXE_zerodds-corba-bridged"))
        .args([
            "--dump-iors",
            "--iiop-bind",
            "127.0.0.1:6833",
            "--topic",
            "IDL:Foo/Bar:1.0=Foo::Bar::Req",
        ])
        .output()
        .expect("zerodds-corba-bridged --dump-iors");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let ior = stdout
        .split_whitespace()
        .find(|w| w.starts_with("IOR:"))
        .expect("an IOR on stdout");
    let cat = Command::new("catior").arg(ior).output().expect("catior");
    let text = String::from_utf8_lossy(&cat.stdout);
    assert!(cat.status.success(), "catior refused the IOR: {text}");
    assert!(text.contains("IDL:Foo/Bar:1.0"), "{text}");
    assert!(
        text.contains("127.0.0.1") && text.contains("6833"),
        "{text}"
    );
}

#[test]
fn locate_request_handler_classifies_known_and_unknown() {
    // Pure-Rust verification — no omniORB needed.
    use zerodds_corba_dds_bridge::locate::{LocateOutcome, locate_lookup};
    use zerodds_corba_dds_bridge::mapping::{
        BridgeMapping, BridgeRoute, Direction, OperationMapping, TopicQosRef,
    };
    let mut m = BridgeMapping::new();
    m.add_route(BridgeRoute {
        repository_id: "IDL:demo/Trader:1.0".into(),
        object_key: b"trader-key".to_vec(),
        direction: Direction::CorbaToDds,
        operations: vec![OperationMapping {
            operation: "place".into(),
            request_topic: "Trade/Req".into(),
            reply_topic: "Trade/Rep".into(),
            qos: TopicQosRef::default(),
        }],
    });
    assert_eq!(locate_lookup(&m, b"trader-key"), LocateOutcome::Here);
    assert_eq!(locate_lookup(&m, b"unknown"), LocateOutcome::Unknown);
}
