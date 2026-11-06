// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Kotlin endpoint ping-pong: the native pure-Kotlin wire-core (`zerodds.Writer`
//! /`Reader`, byte-identical XCDR2) marshals the `@final Ping { long seq; string
//! msg; }` and decodes the `Pong`, the endpoint SDK (`Client` / `AsyncReader`
//! over `UdpTransport`) does the XRCE `WRITE_DATA` framing, and a real
//! `java.net.DatagramSocket` carries the bytes to the Rust peer. `sync` = the
//! caller owns the receive loop (`Client.poll()`); `async` = a daemon reader
//! thread drains the socket into a `LinkedBlockingQueue` the consumer blocks on.
//! Gated on `kotlinc`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{Peer, bind_peer, ping_pong};

fn kotlinc_available() -> bool {
    Command::new("kotlinc")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn endpoints_kotlin() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../endpoints/kotlin")
}

/// Compiles the wire-core + endpoint SDK (`src/Zerodds.kt`) plus the ping-pong
/// driver (`PingPong.kt`) into a self-contained `app.jar` (`-include-runtime`
/// bundles the Kotlin stdlib so `java -cp` runs it), then spawns it against
/// `port` for `mode`.
fn build_kotlin(mode: &str, port: u16) -> Child {
    let kt = endpoints_kotlin();
    let dir = std::env::temp_dir().join(format!("pp_kt_{mode}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    let jar = dir.join("app.jar");

    let o = Command::new("kotlinc")
        .arg(kt.join("src/Zerodds.kt"))
        .arg(kt.join("PingPong.kt"))
        .arg("-include-runtime")
        .arg("-d")
        .arg(&jar)
        .output()
        .expect("run kotlinc");
    assert!(
        o.status.success(),
        "kotlinc failed:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );

    Command::new("java")
        .arg("-cp")
        .arg(&jar)
        .arg("PingPongKt")
        .arg(mode)
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn kotlin PingPong")
}

fn endpoint_case(mode: &str) {
    if !kotlinc_available() {
        eprintln!("SKIP kotlin endpoint {mode}: `kotlinc` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let child = build_kotlin(mode, peer.port);
    ping_pong(&peer, child, &format!("kotlin/{mode}"));
}

/// DDS-XRCE 1.0 §8.3.5.8 and §8.3.5.10: the Kotlin app sends its `Ping`
/// through the endpoint SDK's sync API as a `WRITE_DATA`; the Rust peer
/// decodes the frame with `zerodds-xrce` and the body with the XCDR2 reader
/// (DDS-XTypes 1.3 §7.4), and the app reads the `Pong` the peer sends as `DATA`.
#[test]
#[ignore = "needs the Kotlin toolchain; runs in integration-bindings-jvm-julia"]
fn xrce_8_3_5_8_kotlin_sync_app_writes_the_ping_and_reads_the_pong() {
    endpoint_case("sync");
}

/// DDS-XRCE 1.0 §8.3.5.8 and §8.3.5.10: the Kotlin app sends its `Ping`
/// through the endpoint SDK's async API as a `WRITE_DATA`; the Rust peer
/// decodes the frame with `zerodds-xrce` and the body with the XCDR2 reader
/// (DDS-XTypes 1.3 §7.4), and the app reads the `Pong` the peer sends as `DATA`.
#[test]
#[ignore = "needs the Kotlin toolchain; runs in integration-bindings-jvm-julia"]
fn xrce_8_3_5_8_kotlin_async_app_writes_the_ping_and_reads_the_pong() {
    endpoint_case("async");
}
