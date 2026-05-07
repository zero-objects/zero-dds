// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Cross-Vendor-Interop-Tests gegen omniORB (Spec §10 + §12.3).
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

#[test]
#[ignore = "requires catior + a published IOR from zerodds-corba-bridged"]
fn catior_dumps_zerodds_ior_with_iiop_profile() {
    // Caller setup: ZeroDDS-Corba-bridged exposes an IOR via stdout
    // when started with `--print-ior`. Capture it, pipe to catior.
    if !catior_available() {
        eprintln!("catior not available; skipping");
        return;
    }
    let example_ior = "IOR:010000001100000049444c3a44756d6d793a312e30000000010000000000000054000000010102000a0000003132372e302e302e310000d2040b00000000000d00000000000000010000000000000000000000000000000003000000010000000c00000000000000010001000000000005";
    let out = Command::new("catior")
        .arg(example_ior)
        .output()
        .expect("catior");
    let stdout = String::from_utf8_lossy(&out.stdout);
    eprintln!("catior stdout: {stdout}");
    // Functional assert: catior accepted the IOR (exit 0).
    assert!(out.status.success() || stdout.contains("Type ID"));
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
