// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Swift reliable-stream endpoint E2E. The swift app is the reliable
//! **sender** (`endpoints/swift/Reliable.swift`): it submits N samples on
//! `stream_id 0x80`, sends `WRITE_DATA`, and retransmits on `ACKNACK` from
//! the shared Rust [`ReliablePeer`], which injects loss. Also runs the swift
//! unit + byte-golden tests, the runnable example, and the async-writer
//! latency bench. Gated on `swiftc` (local macOS only — codepit has no Swift
//! toolchain).
//!
//! `Reliable.swift` is compiled as its own `ZeroddsReliable` module (object +
//! `.swiftmodule`) — the same recipe `crates/endpoint-e2e/tests/swift.rs`
//! uses for the ping-pong `Zerodds` module — so it never collides with a
//! generated IDL module's own wire-core types.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

fn swiftc_available() -> bool {
    Command::new("swiftc").arg("--version").output().is_ok()
}

fn reliable_swift() -> String {
    format!(
        "{}/../../endpoints/swift/Reliable.swift",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// Compiles `Reliable.swift` into its own `ZeroddsReliable` module (object +
/// `.swiftmodule`) inside `dir`, so any Swift file compiled with `-I dir`
/// linking `dir/ZeroddsReliable.o` can `import ZeroddsReliable`.
fn build_module(dir: &Path) {
    let build = Command::new("swiftc")
        .args([
            "-swift-version",
            "5",
            "-parse-as-library",
            "-module-name",
            "ZeroddsReliable",
        ])
        .arg("-emit-module")
        .arg("-emit-module-path")
        .arg(dir.join("ZeroddsReliable.swiftmodule"))
        .arg("-emit-object")
        .arg("-o")
        .arg(dir.join("ZeroddsReliable.o"))
        .arg(reliable_swift())
        .output()
        .expect("swiftc ZeroddsReliable module");
    assert!(
        build.status.success(),
        "swiftc (ZeroddsReliable module) failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
}

/// Compiles `src` (a standalone Swift source file) against the
/// `ZeroddsReliable` module already built in `dir`, producing `dir/<name>`.
fn build_against_module(dir: &Path, src: &Path, name: &str) -> PathBuf {
    let out = dir.join(name);
    let build = Command::new("swiftc")
        .args(["-swift-version", "5", "-module-name", name])
        .arg("-I")
        .arg(dir)
        .arg(src)
        .arg(dir.join("ZeroddsReliable.o"))
        .arg("-o")
        .arg(&out)
        .output()
        .unwrap_or_else(|e| panic!("swiftc {name}: {e}"));
    assert!(
        build.status.success(),
        "swiftc ({name}) failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
    out
}

/// Compiles inline Swift source `main_src` (written to `dir/<name>.swift`)
/// against the `ZeroddsReliable` module, producing `dir/<name>`.
fn build_inline_against_module(dir: &Path, main_src: &str, name: &str) -> PathBuf {
    let main_path = dir.join(format!("{name}.swift"));
    std::fs::write(&main_path, main_src).expect("write main.swift");
    build_against_module(dir, &main_path, name)
}

/// A fresh build dir with the `ZeroddsReliable` module already compiled.
fn scaffold(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rel_swift_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    build_module(&dir);
    dir
}

// The reliable sender app: `e2e <port> <N>` submits N and recovers loss via
// ACKNACK-driven retransmit (direct `ReliableSender` usage, matching the
// spec's sender protocol); `bench <port>` reports the async-writer wait-free
// enqueue latency vs inline `sendto`.
const SWIFT_RELIABLE_MAIN: &str = r#"import Foundation
import ZeroddsReliable

final class UDP {
    let fd: Int32
    var peerAddr: sockaddr_in
    init(port: UInt16) {
        // Work on the local `s` so no closure captures `self` before both
        // stored properties are assigned (Swift init rule).
        let s = socket(AF_INET, SOCK_DGRAM, 0)
        var local = sockaddr_in()
        local.sin_family = sa_family_t(AF_INET)
        local.sin_port = 0
        local.sin_addr.s_addr = inet_addr("127.0.0.1")
        _ = withUnsafePointer(to: &local) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(s, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        let flags = fcntl(s, F_GETFL, 0)
        _ = fcntl(s, F_SETFL, flags | O_NONBLOCK)
        var p = sockaddr_in()
        p.sin_family = sa_family_t(AF_INET)
        p.sin_port = in_port_t(port).bigEndian
        p.sin_addr.s_addr = inet_addr("127.0.0.1")
        fd = s
        peerAddr = p
    }
    func send(_ frame: [UInt8]) {
        var p = peerAddr
        _ = frame.withUnsafeBytes { (raw: UnsafeRawBufferPointer) in
            withUnsafePointer(to: &p) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
                    sendto(fd, raw.baseAddress, raw.count, 0, sa, socklen_t(MemoryLayout<sockaddr_in>.size))
                }
            }
        }
    }
    func recvNonBlocking() -> [UInt8]? {
        var buf = [UInt8](repeating: 0, count: 8192)
        let n = buf.withUnsafeMutableBytes { (raw: UnsafeMutableRawBufferPointer) -> Int in
            recvfrom(fd, raw.baseAddress, raw.count, 0, nil, nil)
        }
        if n <= 0 { return nil }
        return Array(buf[0..<n])
    }
}

func u32le(_ v: UInt32) -> [UInt8] {
    [UInt8(v & 0xff), UInt8((v >> 8) & 0xff), UInt8((v >> 16) & 0xff), UInt8((v >> 24) & 0xff)]
}

func modeE2E(port: UInt16, n: Int) {
    let udp = UDP(port: port)
    let sender = ReliableSender()
    for i in 0..<n {
        let payload = u32le(UInt32(i))
        guard let seq = try? sender.submit(payload) else {
            FileHandle.standardError.write("submit failed\n".data(using: .utf8)!)
            exit(1)
        }
        udp.send(writeDataFrame(seq: seq, sample: payload))
    }
    let start = Date()
    var round = 0
    while sender.inFlightCount > 0 && round < 2000 {
        while let frame = udp.recvNonBlocking() {
            if let an = parseAckNack(frame) { sender.recvAckNack(an) }
        }
        if sender.inFlightCount == 0 { break }
        for seq in sender.inFlightSeqs() {
            if let data = sender.getInFlight(seq) {
                udp.send(writeDataFrame(seq: seq, sample: data))
            }
        }
        let nowMs = Int64(Date().timeIntervalSince(start) * 1000)
        if let hb = sender.pendingHeartbeat(nowMs: nowMs) {
            udp.send(heartbeatFrame(hb))
        }
        usleep(2000)
        round += 1
    }
    print("SENT count=\(n) rounds=\(round) inflight=\(sender.inFlightCount)")
}

func modeBench(port: UInt16) {
    let udp = UDP(port: port)
    let payload: [UInt8] = [1, 2, 3, 4]
    let m = 4000
    let frame = writeDataFrame(seq: 0, sample: payload)

    let t0 = Date()
    for _ in 0..<m { udp.send(frame) }
    let inlineNs = Int(Date().timeIntervalSince(t0) / Double(m) * 1_000_000_000)

    let writer = AsyncWriter(sendFn: { f in udp.send(f) }, recvFn: { udp.recvNonBlocking() })
    writer.start()
    for _ in 0..<100 { _ = writer.write(payload) } // warmup
    var pushed = 0
    let t1 = Date()
    for _ in 0..<m {
        if writer.write(payload) { pushed += 1 }
    }
    let decNs = Int(Date().timeIntervalSince(t1) / Double(m) * 1_000_000_000)
    writer.close()
    print("BENCH decoupled_ns=\(decNs) inline_ns=\(inlineNs) pushed=\(pushed)/\(m)")
}

let args = CommandLine.arguments
let mode = args[1]
let port = UInt16(args[2]) ?? 0
if mode == "bench" {
    modeBench(port: port)
} else {
    let n = Int(args[3]) ?? 0
    modeE2E(port: port, n: n)
}
"#;

fn spawn_app(dir: &Path, mode: &str, port: u16, n: u32) -> Child {
    let exe = build_inline_against_module(dir, SWIFT_RELIABLE_MAIN, "swiftrelapp");
    Command::new(&exe)
        .arg(mode)
        .arg(port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn swift app")
}

fn decode_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn loss_case(drop_every: Option<usize>, tag: &str, label: &str) {
    if !swiftc_available() {
        eprintln!("SKIP {label}: `swiftc` not on PATH");
        return;
    }
    const N: u32 = 12;
    let dir = scaffold(tag);
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = spawn_app(&dir, "e2e", peer.port, N);
    let delivered = reliable_receive(&peer, child, label, N as usize);
    assert_eq!(delivered.len(), N as usize, "{label}: sample count");
    for (i, payload) in delivered.iter().enumerate() {
        assert_eq!(
            decode_u32(payload),
            i as u32,
            "{label}: gap/misorder at {i}"
        );
    }
}

#[test]
fn swift_reliable_loss_recovery() {
    // Peer drops every 3rd distinct sample once -> the app must retransmit.
    loss_case(Some(3), "loss", "swift/loss");
}

#[test]
fn swift_reliable_no_loss() {
    loss_case(None, "baseline", "swift/baseline");
}

#[test]
fn swift_reliable_unit() {
    // Mirror-of-reference unit tests + byte-golden HEARTBEAT/ACKNACK.
    if !swiftc_available() {
        eprintln!("SKIP swift_reliable_unit: `swiftc` not on PATH");
        return;
    }
    let dir = scaffold("unit");
    let src = format!(
        "{}/../../endpoints/swift/reliable_tests.swift",
        env!("CARGO_MANIFEST_DIR")
    );
    let exe = build_against_module(&dir, Path::new(&src), "swiftreltests");
    let out = Command::new(&exe).output().expect("run unit tests");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("UNIT OK"),
        "swift unit tests failed:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn swift_reliable_example() {
    // The runnable in-process loss-recovery demo prints the recovered sequence.
    if !swiftc_available() {
        eprintln!("SKIP swift_reliable_example: `swiftc` not on PATH");
        return;
    }
    let dir = scaffold("example");
    let src = format!(
        "{}/../../endpoints/swift/example_reliable.swift",
        env!("CARGO_MANIFEST_DIR")
    );
    let exe = build_against_module(&dir, Path::new(&src), "swiftrelexample");
    let out = Command::new(&exe).output().expect("run example");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("sequence 0..11 verified in order"),
        "example did not verify:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn swift_reliable_latency_bench() {
    // Async-decoupled writer: wait-free queue push vs inline sendto.
    if !swiftc_available() {
        eprintln!("SKIP swift_reliable_latency_bench: `swiftc` not on PATH");
        return;
    }
    let dir = scaffold("bench");
    let child = spawn_app(&dir, "bench", 9, 0);
    let out = child.wait_with_output().expect("bench wait");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .find(|l| l.starts_with("BENCH "))
        .unwrap_or_else(|| {
            panic!(
                "no BENCH line\nstderr: {}",
                String::from_utf8_lossy(&out.stderr)
            )
        });
    let dec = parse_kv(line, "decoupled_ns=");
    let inl = parse_kv(line, "inline_ns=");
    eprintln!("swift async-writer latency: decoupled={dec} ns, inline(sendto)={inl} ns");
    assert!(
        dec < inl,
        "decoupled push ({dec} ns) must beat inline sendto ({inl} ns)"
    );
}

fn parse_kv(line: &str, key: &str) -> u64 {
    line.split_whitespace()
        .find_map(|tok| tok.strip_prefix(key))
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("missing {key} in `{line}`"))
}
