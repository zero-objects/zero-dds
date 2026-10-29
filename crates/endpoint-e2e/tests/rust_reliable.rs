// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Rust endpoint reliable-stream E2E: the `endpoints/rust` `example_reliable`
//! sender streams N samples on a reliable stream (`stream_id 0x80`) to the shared
//! Rust reliable peer, which injects loss and replies ACKNACK. The app retransmits
//! until every sample is delivered gap-free and in order.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive, xcdr2_reader};

const N: usize = 12;

/// Builds the `endpoints/rust` reliable example and returns its binary path.
///
/// Every failure here is a failure. The other endpoint suites skip when
/// their toolchain is absent, which is a real condition; this one runs
/// under `cargo`, so the compiler it needs is the one already running it.
/// A build error used to return `None` and print `SKIP`, which turned a
/// broken example into two green tests.
fn build_example() -> PathBuf {
    // crates/endpoint-e2e -> crates -> workspace root.
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/endpoint-e2e sits two levels below the workspace root");
    let status = Command::new(env!("CARGO"))
        .args([
            "build",
            "-p",
            "zerodds-endpoint-rust",
            "--example",
            "example_reliable",
        ])
        .current_dir(workspace)
        .status()
        .expect("cargo runs this test, so cargo starts");
    assert!(
        status.success(),
        "example_reliable does not build; see the cargo output above"
    );
    // Where cargo put it: under `CARGO_TARGET_DIR` when that is set,
    // resolved against the directory the build ran in.
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map_or_else(|| workspace.join("target"), |dir| workspace.join(dir));
    let bin = target.join("debug/examples/example_reliable");
    assert!(
        bin.exists(),
        "cargo reported success and {} is not there",
        bin.display()
    );
    bin
}

fn run_case(drop_every: Option<usize>, label: &str) {
    let bin = build_example();
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = Command::new(&bin)
        .arg(peer.port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn example_reliable");

    let delivered = reliable_receive(&peer, child, label, N);
    assert_eq!(delivered.len(), N, "{label}: delivered count");
    // Each sample is a `Reading { id, value, label }`; the id must equal its index.
    for (i, payload) in delivered.iter().enumerate() {
        let mut r = xcdr2_reader(payload);
        let id = r.read_u32().expect("reading id");
        assert_eq!(id as usize, i, "{label}: sample {i} id (gap-free order)");
    }
}

#[test]
fn rust_reliable_loss_recovery() {
    run_case(Some(3), "rust/loss");
}

#[test]
fn rust_reliable_no_loss_baseline() {
    run_case(None, "rust/baseline");
}
