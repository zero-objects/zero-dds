// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! omniORB's `catior` against the IOR the daemon prints
//! (`zerodds-corba-bridge-1.0` §4.1, §12.3). Needs the omniORB tools; runs
//! in the integration job `integration-bridges-vendor-corba`.

#![cfg(feature = "std")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)]

use std::process::Command;

fn catior_available() -> bool {
    Command::new("catior").arg("--help").output().is_ok()
}

/// omniORB's `catior` reads the IOR the daemon prints: the type id, the
/// IIOP profile's host and port.
#[test]
#[ignore = "needs catior (omniORB tools, integration-bridges-vendor-corba)"]
fn corbabridge_4_1_catior_reads_the_ior_the_daemon_prints() {
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
