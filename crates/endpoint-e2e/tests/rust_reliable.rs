// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Rust endpoint reliable-stream E2E: the `endpoints/rust` `example_reliable`
//! sender streams N samples on a reliable stream (`stream_id 0x80`) to the shared
//! Rust reliable peer, which injects loss and replies ACKNACK. The app retransmits
//! until every sample is delivered gap-free and in order.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zerodds_cdr::{BufferReader, Endianness};
use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

const N: usize = 12;

/// Builds the `endpoints/rust` reliable example and returns its binary path.
fn build_example() -> Option<PathBuf> {
    // crates/endpoint-e2e -> crates -> workspace root.
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2)?;
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
        .ok()?;
    if !status.success() {
        return None;
    }
    let bin = workspace.join("target/debug/examples/example_reliable");
    bin.exists().then_some(bin)
}

fn run_case(drop_every: Option<usize>, label: &str) {
    let Some(bin) = build_example() else {
        eprintln!("SKIP {label}: could not build example_reliable");
        return;
    };
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
        let mut r = BufferReader::new(payload, Endianness::Little).xcdr2();
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
