// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! `--listen` reaches the configuration. The flag becomes
//! `--set +listeners=<kind>:<address>`, a scalar; the daemon refused it with
//! "invalid listeners[0]: expected a map of settings" and never started, so
//! the documented flag could not be used at all.

#![cfg(feature = "daemon")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::process::Command;

#[test]
fn listen_flags_pass_the_configuration_check() {
    let out = Command::new(env!("CARGO_BIN_EXE_zerodds-xrce-agentd"))
        .args([
            "--listen",
            "udp:127.0.0.1:2019",
            "--listen",
            "tcp:127.0.0.1:2020",
            "--check-config",
        ])
        .output()
        .expect("run zerodds-xrce-agentd");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{text}");
    assert!(text.contains("127.0.0.1:2019"), "{text}");
    assert!(text.contains("127.0.0.1:2020"), "{text}");
}

#[test]
fn a_listener_that_is_neither_shorthand_nor_map_is_refused() {
    let out = Command::new(env!("CARGO_BIN_EXE_zerodds-xrce-agentd"))
        .args(["--listen", "quic:127.0.0.1:2019", "--check-config"])
        .output()
        .expect("run zerodds-xrce-agentd");
    assert!(!out.status.success());
}
