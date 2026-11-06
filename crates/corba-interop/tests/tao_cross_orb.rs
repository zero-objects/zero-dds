// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stderr
)]
//! ZeroDDS against TAO, both directions, each side generated from the same
//! IDL by its own compiler (`competitors/interop.idl`, `cosnaming.idl`;
//! tao_idl for TAO, `build.rs` for ZeroDDS): TAO's servants and clients are
//! the oracle of the GIOP/IIOP requests and replies ZeroDDS writes and
//! reads. The TAO cells of the former `competitors/run_interop.sh`; a TAO
//! naming daemon is not in the TAO of the OpenDDS build, so ZeroDDS's
//! naming client meets omniNames and JacORB's NameServer instead
//! (`omniorb_cross_orb.rs`, `jacorb_cross_orb.rs`).
//!
//! The TAO programs come from `competitors/build_peers.sh tao`
//! (`ZERODDS_CORBA_PEERS`), against the TAO of the interop image's OpenDDS
//! build (`TAO_PREFIX`); the job `vendor-live-corba-tao` runs the tests.

use std::process::Command;

#[path = "support/peers.rs"]
mod peers;

/// A TAO program: its libraries under `TAO_PREFIX/lib` (`build_peers.sh`
/// links them with that run path), at most 1024 open files, under which TAO
/// 2.5 allocates its transport cache (crates/corba-dds-bridge/tests/tao_e2e.rs).
fn tao(name: &str, args: &[&str]) -> Command {
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg("ulimit -n 1024 2>/dev/null; exec \"$0\" \"$@\"")
        .arg(peers::peer(name))
        .args(args);
    command
}

fn tao_client(name: &str, args: &[&str]) {
    let output = peers::run_client(tao(name, args));
    assert!(output.contains("ALL OK"), "{name}:\n{output}");
}

#[test]
#[ignore = "needs TAO (vendor-live-corba-tao)"]
fn corbarust_13_zerodds_calls_echo_and_bench_on_a_tao_server() {
    let mut server = peers::Server::start(tao("tao_interop_server", &[]));
    let echo = server.ior("ECHO_IOR=");
    let bench = server.ior("BENCH_IOR=");
    peers::zerodds_client("echo", &echo);
    peers::zerodds_client("bench", &bench);
}

#[test]
#[ignore = "needs TAO (vendor-live-corba-tao)"]
fn corbarust_13_a_tao_client_calls_echo_and_bench_on_a_zerodds_server() {
    let mut server = peers::zerodds_server();
    let echo = server.ior("ECHO_IOR=");
    let bench = server.ior("BENCH_IOR=");
    tao_client("tao_interop_client", &["echo", &echo]);
    tao_client("tao_interop_client", &["bench", &bench]);
}

#[test]
#[ignore = "needs TAO (vendor-live-corba-tao)"]
fn corbarust_13_a_tao_client_drives_a_zerodds_naming_context() {
    let mut server = peers::zerodds_server();
    let naming = server.ior("NAMING_IOR=");
    tao_client("tao_naming_client", &[&naming]);
}
