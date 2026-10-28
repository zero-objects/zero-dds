// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Swift endpoint ping-pong: the generated `idlc` types (own XCDR2 wire core)
//! plus the Swift endpoint SDK (`Client` poll loop and `AsyncReader` stream),
//! XRCE-framed over a real UDP socket. Sync and async variants. Gated on
//! `swiftc` (local macOS only — the Linux test host has no Swift toolchain).
//!
//! The generated module and the SDK both define `Endianness`/`Writer`/`Reader`
//! (the shared wire core), which would collide in one module — so the SDK is
//! compiled as its own `Zerodds` module (object + `.swiftmodule`) and the app
//! links against it, while the generated types stay in the app module.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{
    Language, endpoints_dir, runtime_dir, scratch_dir, source, tool_available,
};
use zerodds_endpoint_e2e::{Peer, bind_peer, ping_pong};

fn swiftc_available() -> bool {
    tool_available("swiftc", "--version")
}

/// The generated Ping/Pong types + codec, as an app-module source file.
///
/// The runtime's own sources compile beside it as one module rather than as a
/// separate one — that is how `wireprobe-zerodds-swift-gen` builds its probe,
/// and it is what keeps `ZdWriter` reachable without an `import ZdWire` the
/// generator does not emit.
fn gen_swift() -> String {
    format!("import Foundation\n{}", source(Language::Swift))
}

/// Every `.swift` in `runtime/swift/Sources/ZdWire`, which compiles into the
/// app module beside the generated codec.
fn runtime_sources() -> Vec<std::path::PathBuf> {
    let dir = runtime_dir("swift").join("Sources/ZdWire");
    let mut out: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "swift"))
        .collect();
    out.sort();
    assert!(!out.is_empty(), "no runtime sources in {}", dir.display());
    out
}

// The app: a real UDP `Transport`, plus `Client` (sync poll) or `AsyncReader`
// (async stream). Sends a typed `Ping` via the generated `marshalXCDR`, decodes
// the `Pong` the Rust peer sends back, prints the exact line the harness checks.
const APP_MAIN: &str = r#"import Foundation
#if canImport(Glibc)
import Glibc
#endif
import Zerodds

// A real loopback UDP transport implementing the SDK's `Transport` protocol:
// binds an ephemeral local port, targets the peer, non-blocking `receive()`.
final class UDPTransport: Transport, @unchecked Sendable {
    let fd: Int32
    var peer: sockaddr_in
    init?(port: UInt16) {
        // Work on the local `s` so no closure captures `self` before both
        // stored properties are assigned (Swift init rule).
        // SOCK_DGRAM is Int32 on Darwin but a `__socket_type` enum on Glibc.
        #if canImport(Glibc)
        let s = socket(AF_INET, Int32(SOCK_DGRAM.rawValue), 0)
        #else
        let s = socket(AF_INET, SOCK_DGRAM, 0)
        #endif
        if s < 0 { return nil }
        var local = sockaddr_in()
        local.sin_family = sa_family_t(AF_INET)
        local.sin_port = 0
        local.sin_addr.s_addr = inet_addr("127.0.0.1")
        let br = withUnsafePointer(to: &local) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                bind(s, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        if br < 0 { close(s); return nil }
        let flags = fcntl(s, F_GETFL, 0)
        _ = fcntl(s, F_SETFL, flags | O_NONBLOCK)
        var p = sockaddr_in()
        p.sin_family = sa_family_t(AF_INET)
        p.sin_port = in_port_t(port).bigEndian
        p.sin_addr.s_addr = inet_addr("127.0.0.1")
        fd = s
        peer = p
    }
    func deliver(_ frame: [UInt8]) {
        var p = peer
        _ = frame.withUnsafeBytes { (raw: UnsafeRawBufferPointer) in
            withUnsafePointer(to: &p) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { sa in
                    sendto(fd, raw.baseAddress, raw.count, 0, sa, socklen_t(MemoryLayout<sockaddr_in>.size))
                }
            }
        }
    }
    func receive() -> [UInt8]? {
        var buf = [UInt8](repeating: 0, count: 4096)
        let n = buf.withUnsafeMutableBytes { (raw: UnsafeMutableRawBufferPointer) -> Int in
            recvfrom(fd, raw.baseAddress, raw.count, 0, nil, nil)
        }
        if n <= 0 { return nil }
        return Array(buf[0..<n])
    }
}

func fail(_ msg: String) -> Never {
    FileHandle.standardError.write((msg + "\n").data(using: .utf8)!)
    exit(1)
}

let args = CommandLine.arguments
let mode = args.count > 1 ? args[1] : "sync"
let port = UInt16(args.count > 2 ? args[2] : "0") ?? 0
guard let transport = UDPTransport(port: port) else { fail("udp init failed") }

// Marshal the Ping with the generated codec, hand the bytes to the SDK.
// The generated codec throws a ZdWireError on a refusal; this app owns
// the exit code for one, the same as the wireprobe harness does.
let zdFraming = ZdPlCdr1Framing("sentinel-must-understand", 16128, true, true, true, true)
let zdWriter = ZdWriter(ZdEndianness.little, 4, zdFraming, false)
do {
    try zd_enc_Ping(zdWriter, Ping(seq: 1, msg: "hello from app"))
} catch {
    fail("encode failed: \(error)")
}
let sample = zdWriter.finish()

var body: [UInt8]? = nil
if mode == "async" {
    let client = Client(transport, objectId: xrceObjectId(0x001, objkDataWriter))
    client.write(sample)
    let reader = AsyncReader(transport)
    body = await withTaskGroup(of: [UInt8]?.self) { group in
        group.addTask { for await b in reader.stream() { return b }; return nil }
        group.addTask { try? await Task.sleep(nanoseconds: 10_000_000_000); return nil }
        let first = await group.next() ?? nil
        group.cancelAll()
        return first
    }
} else {
    let client = Client(transport, objectId: xrceObjectId(0x001, objkDataWriter))
    client.write(sample)
    let deadline = Date().addingTimeInterval(10)
    while Date() < deadline {
        if let b = client.poll() { body = b; break }
        usleep(1000)
    }
}

guard let got = body else { fail("\(mode): no pong") }
let zdReader = ZdReader([UInt8](got), ZdEndianness.little, 4, zdFraming)
let pong: Pong
do {
    pong = try zd_dec_Pong(zdReader, ZdTryConstruct.discard)
} catch {
    fail("decode failed: \(error)")
}
print("PONG seq=\(pong.seq) reply=\(pong.reply)")
"#;

/// Builds the Swift app for `mode` targeting the peer `port`, returns the spawned
/// child (stdout piped). Compiles the SDK as its own `Zerodds` module first so
/// the shared wire-core types don't collide with the generated module.
fn build_swift_app(mode: &str, port: u16) -> Child {
    let sdk = endpoints_dir("swift").join("Sources/Zerodds/Zerodds.swift");
    let dir = scratch_dir(&format!("swift_{mode}"));
    std::fs::write(dir.join("gen.swift"), gen_swift()).expect("write gen");
    std::fs::write(dir.join("main.swift"), APP_MAIN).expect("write main");

    // 1) SDK -> Zerodds.o + Zerodds.swiftmodule.
    let sdk_build = Command::new("swiftc")
        .args([
            "-swift-version",
            "5",
            "-parse-as-library",
            "-module-name",
            "Zerodds",
        ])
        .arg("-emit-module")
        .arg("-emit-module-path")
        .arg(dir.join("Zerodds.swiftmodule"))
        .arg("-emit-object")
        .arg("-o")
        .arg(dir.join("Zerodds.o"))
        .arg(&sdk)
        .output()
        .expect("swiftc sdk");
    assert!(
        sdk_build.status.success(),
        "swiftc (Zerodds module) failed:\n{}",
        String::from_utf8_lossy(&sdk_build.stderr)
    );

    // 2) app: generated types + main, linking the Zerodds object.
    let app_build = Command::new("swiftc")
        .args(["-swift-version", "5", "-module-name", "pingpongapp"])
        .arg("-I")
        .arg(&dir)
        .args(runtime_sources())
        .arg(dir.join("gen.swift"))
        .arg(dir.join("main.swift"))
        .arg(dir.join("Zerodds.o"))
        .arg("-o")
        .arg(dir.join("app"))
        .output()
        .expect("swiftc app");
    assert!(
        app_build.status.success(),
        "swiftc (app) failed:\n{}\n--- gen.swift + main.swift in {} ---",
        String::from_utf8_lossy(&app_build.stderr),
        dir.display()
    );

    Command::new(dir.join("app"))
        .arg(mode)
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn swift app")
}

fn endpoint_case(mode: &str) {
    if !swiftc_available() {
        eprintln!("SKIP swift endpoint {mode}: `swiftc` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let child = build_swift_app(mode, peer.port);
    ping_pong(&peer, child, &format!("swift/{mode}"));
}

#[test]
fn swift_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn swift_endpoint_async() {
    endpoint_case("async");
}
