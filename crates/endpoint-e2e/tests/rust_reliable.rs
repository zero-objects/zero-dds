// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Rust endpoint reliable-stream E2E: an app over the `endpoints/rust`
//! async-decoupled writer streams N samples on a reliable stream
//! (`stream_id 0x80`) to the shared Rust reliable peer, which injects loss and
//! replies ACKNACK. The app retransmits until every sample is delivered
//! gap-free and in order. The samples are the `Ping` of the codec the Rust
//! generator writes from the IDL. And the producer-latency comparison of the
//! `example_reliable` bench.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, source};
use zerodds_endpoint_e2e::{
    RELIABLE_MSG, RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
    xcdr2_writer,
};
use zerodds_endpoint_rust::reliable::AsyncReliableWriter;

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

/// The `Ping`/`Pong` declarations and codec `codegen::source(Language::Rust)`
/// writes from [`zerodds_endpoint_e2e::IDL`], behind the `use` line `idlc
/// generate --lang rust` writes. Committed, because a Rust test cannot
/// compile source it generates at run time inside its 25 s;
/// [`reliable_the_committed_rust_ping_is_what_the_generator_writes`] holds
/// the file to the generator.
#[path = "generated/pingpong_rust.rs"]
#[rustfmt::skip]
#[allow(dead_code, unreachable_pub, clippy::all, clippy::pedantic)]
mod pingpong;

/// The file [`pingpong`] is, as the generator writes it now.
fn generated_rust() -> String {
    format!(
        "// Generated from zerodds_endpoint_e2e::IDL by codegen::source(Language::Rust);\n\
         // tests/rust_reliable.rs holds this file to the generator.\n\
         use zerodds_wire::{{Reader, WireError, Writer}};\n\n{}",
        source(Language::Rust)
    )
}

/// The environment variable that makes this test binary the reliable app.
const APP_ROLE: &str = "ZERODDS_E2E_RUST_RELIABLE_APP";

/// The DataWriter of every frame's BaseObjectRequest (DDS-XRCE 1.0 §7.7.8).
const WRITER_ID: u16 = 0x0015;

/// The reliable app, in this test binary started again: the generated codec
/// encodes `Ping { seq: i, msg: "reliable" }` and the SDK's async-decoupled
/// `AsyncReliableWriter` (`endpoints/rust/src/reliable.rs`) sends it to the
/// peer on the port `APP_ROLE` names.
fn app(port: u16) {
    let sock = std::net::UdpSocket::bind("127.0.0.1:0").expect("bind");
    sock.connect(("127.0.0.1", port)).expect("connect");
    let mut writer = AsyncReliableWriter::start(sock, 64, WRITER_ID);
    let handle = writer.handle();
    for i in 0..RELIABLE_SAMPLES {
        let ping = pingpong::Ping {
            seq: i32::try_from(i).expect("index fits a long"),
            msg: RELIABLE_MSG.to_owned(),
        };
        let mut w = xcdr2_writer();
        ping.encode(&mut w)
            .expect("the generated encoder writes the Ping");
        let mut item = w.finish();
        while let Err(back) = handle.enqueue(item) {
            item = back;
            std::thread::yield_now();
        }
    }
    writer.shutdown();
}

fn run_case(test: &str, drop_every: Option<usize>, label: &str) {
    if let Ok(port) = std::env::var(APP_ROLE) {
        app(port.parse().expect("port"));
        return;
    }
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = Command::new(std::env::current_exe().expect("this test binary"))
        .args(["--exact", test, "--nocapture", "--include-ignored"])
        .env(APP_ROLE, peer.port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the reliable app");
    let delivered = reliable_receive(&peer, child, label, RELIABLE_SAMPLES);
    assert_reliable_pings(label, &delivered, RELIABLE_SAMPLES);
}

/// reliable-endpoint 1.0 §5 point 3: the Rust app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "a live loss run, with the reliable runs of the other languages in integration-bindings"]
fn reliable_5_3_rust_samples_arrive_in_order_despite_dropped_datagrams() {
    run_case(
        "reliable_5_3_rust_samples_arrive_in_order_despite_dropped_datagrams",
        Some(3),
        "rust/loss",
    );
}

/// The committed generated module is what the generator writes from the IDL
/// now, so the reliable run above exercises the generator's output and not a
/// copy that has drifted from it.
#[test]
fn reliable_the_committed_rust_ping_is_what_the_generator_writes() {
    let committed = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/generated/pingpong_rust.rs"),
    )
    .expect("tests/generated/pingpong_rust.rs");
    assert!(
        committed == generated_rust(),
        "tests/generated/pingpong_rust.rs differs from what codegen::source(Language::Rust) \
         writes; regenerate it with `cargo run -p zerodds-endpoint-e2e --example gen -- rust` \
         behind the two comment lines and the `use` line of `generated_rust`"
    );
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the Rust producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `send`; both values are printed.
#[test]
#[ignore = "builds the endpoints/rust example with cargo; runs in integration-bindings"]
fn reliable_5_4_rust_the_producer_enqueue_costs_less_than_the_inline_send() {
    let bin = build_example();
    let out = Command::new(&bin)
        .arg("bench")
        .output()
        .expect("run example_reliable bench");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("LATENCY"),
        "bench output missing:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    eprintln!("rust reliable {}", stdout.trim());
    zerodds_endpoint_e2e::assert_enqueue_below_inline("rust", &stdout);
}
