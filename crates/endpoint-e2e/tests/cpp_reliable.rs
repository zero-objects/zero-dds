// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! C++ reliable endpoint: the `endpoints/cpp` reliable-stream layer
//! (`zerodds_reliable.hpp`) as a live reliable **sender** against the shared
//! Rust reliable peer, plus the unit + byte-golden + latency-bench binary and
//! the in-process example. Gated on `g++`. Proves loss recovery: the peer drops
//! every 3rd datagram, the app retransmits on ACKNACK, all samples arrive
//! gap-free in order.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive};

fn gpp() -> Option<&'static str> {
    Command::new("g++")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .filter(|s| s.success())
        .map(|_| "g++")
}

fn cpp_include() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../endpoints/cpp/include")
}
fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Compiles a single self-contained C++17 source (header-only `zerodds_reliable`
/// is on the include path) to `out`. Panics with the compiler diagnostics.
fn compile(cc: &str, src: &std::path::Path, out: &std::path::Path) {
    let o = Command::new(cc)
        .args(["-std=c++17", "-O2", "-pthread", "-D_GNU_SOURCE", "-Wall"])
        .arg(format!("-I{}", cpp_include().display()))
        .arg(src)
        .arg("-o")
        .arg(out)
        .output()
        .expect("spawn g++");
    assert!(
        o.status.success(),
        "compile {}:\n{}",
        src.display(),
        String::from_utf8_lossy(&o.stderr)
    );
}

// The live reliable-sender app: submits `count` samples through the
// async-decoupled `AsyncWriter` (SPSC ring + drain thread doing sendmmsg +
// ACKNACK-driven retransmit) over a real UDP socket to the peer.
const APP_CPP: &str = r####"
#define _GNU_SOURCE
#include "zerodds_reliable.hpp"
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <cstdint>
#include <vector>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <unistd.h>

using namespace zerodds::reliable;

int main(int argc, char** argv) {
    if (argc < 3) { std::fprintf(stderr, "usage: %s <port> <count>\n", argv[0]); return 2; }
    int port = std::atoi(argv[1]);
    std::uint32_t count = static_cast<std::uint32_t>(std::atoi(argv[2]));

    int fd = ::socket(AF_INET, SOCK_DGRAM, 0);
    if (fd < 0) { std::perror("socket"); return 1; }
    sockaddr_in peer;
    std::memset(&peer, 0, sizeof(peer));
    peer.sin_family = AF_INET;
    peer.sin_port = htons(static_cast<unsigned short>(port));
    inet_pton(AF_INET, "127.0.0.1", &peer.sin_addr);
    if (::connect(fd, reinterpret_cast<sockaddr*>(&peer), sizeof(peer)) < 0) { std::perror("connect"); return 1; }

    auto send_one = [&](const Bytes& f) { ::send(fd, f.data(), f.size(), 0); };
    auto send_batch = [&](const std::vector<Bytes>& fs) {
        std::vector<mmsghdr> m(fs.size());
        std::vector<iovec> iov(fs.size());
        for (std::size_t i = 0; i < fs.size(); ++i) {
            iov[i].iov_base = const_cast<std::uint8_t*>(fs[i].data());
            iov[i].iov_len = fs[i].size();
            std::memset(&m[i], 0, sizeof(mmsghdr));
            m[i].msg_hdr.msg_iov = &iov[i];
            m[i].msg_hdr.msg_iovlen = 1;
        }
        ::sendmmsg(fd, m.data(), static_cast<unsigned>(fs.size()), 0);
    };
    auto poll_ack = [&](std::uint8_t* buf, std::size_t cap) -> int {
        ssize_t n = ::recv(fd, buf, cap, MSG_DONTWAIT);
        return n > 0 ? static_cast<int>(n) : -1;
    };

    {
        AsyncWriter w(send_batch, send_one, poll_ack, 1024);
        for (std::uint32_t i = 0; i < count; ++i) {
            std::uint8_t b[4] = {static_cast<std::uint8_t>(i), static_cast<std::uint8_t>(i >> 8),
                                 static_cast<std::uint8_t>(i >> 16), static_cast<std::uint8_t>(i >> 24)};
            while (!w.enqueue(b, 4)) { /* ring full: producer spins briefly */ }
        }
        w.finish();
    }  // AsyncWriter dtor joins the drain thread once the window has drained
    std::printf("SENT %u\n", count);
    return 0;
}
"####;

fn build_app(cc: &str, port: u16, count: u32) -> Child {
    let dir = std::env::temp_dir().join(format!("rel_cpp_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let src = dir.join("app.cpp");
    std::fs::write(&src, APP_CPP).expect("write app");
    let bin = dir.join("app");
    compile(cc, &src, &bin);
    Command::new(&bin)
        .arg(port.to_string())
        .arg(count.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn app")
}

/// Decodes the delivered 4-byte LE payloads and asserts they are exactly the
/// contiguous sequence `0..count`.
fn assert_contiguous(delivered: &[Vec<u8>], count: u32, label: &str) {
    assert_eq!(delivered.len(), count as usize, "{label}: delivered count");
    for (i, p) in delivered.iter().enumerate() {
        assert_eq!(p.len(), 4, "{label}: payload {i} width");
        let v = u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
        assert_eq!(v, i as u32, "{label}: payload {i} value");
    }
}

#[test]
fn cpp_reliable_loss_recovery() {
    let Some(cc) = gpp() else {
        eprintln!("SKIP cpp_reliable_loss_recovery: `g++` not on PATH");
        return;
    };
    let count = 12u32;
    let peer = bind_reliable_peer(Some(3)).expect("bind reliable peer");
    let child = build_app(cc, peer.port, count);
    let delivered = reliable_receive(&peer, child, "cpp/loss", count as usize);
    assert_contiguous(&delivered, count, "cpp/loss");
}

#[test]
fn cpp_reliable_lossless_baseline() {
    let Some(cc) = gpp() else {
        eprintln!("SKIP cpp_reliable_lossless_baseline: `g++` not on PATH");
        return;
    };
    let count = 12u32;
    let peer = bind_reliable_peer(None).expect("bind reliable peer");
    let child = build_app(cc, peer.port, count);
    let delivered = reliable_receive(&peer, child, "cpp/baseline", count as usize);
    assert_contiguous(&delivered, count, "cpp/baseline");
}

#[test]
fn cpp_reliable_unit_and_golden() {
    let Some(cc) = gpp() else {
        eprintln!("SKIP cpp_reliable_unit_and_golden: `g++` not on PATH");
        return;
    };
    // Generate the real Rust HEARTBEAT/ACKNACK goldens to assert byte-identity.
    let gold = std::env::temp_dir().join(format!("rel_cpp_gold_{}", std::process::id()));
    std::fs::create_dir_all(&gold).expect("mkdir gold");
    let g = Command::new("cargo")
        .args(["run", "-q", "-p", "zerodds-endpoint-golden", "--"])
        .arg(&gold)
        .current_dir(workspace_root())
        .output()
        .expect("spawn golden gen");
    assert!(
        g.status.success(),
        "golden gen:\n{}",
        String::from_utf8_lossy(&g.stderr)
    );
    let hb = gold.join("golden_heartbeat_le.bin");
    let ack = gold.join("golden_acknack_le.bin");
    assert!(hb.exists() && ack.exists(), "goldens not written");

    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../endpoints/cpp/test/test_reliable_cpp.cpp");
    let bin = std::env::temp_dir().join(format!("rel_cpp_test_{}", std::process::id()));
    compile(cc, &src, &bin);
    let out = Command::new(&bin)
        .arg(&hb)
        .arg(&ack)
        .output()
        .expect("run test");
    let stdout = String::from_utf8_lossy(&out.stdout);
    eprintln!("cpp reliable: {}", stdout.trim());
    assert!(
        out.status.success() && stdout.contains("ALL OK"),
        "unit/golden failed:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn cpp_reliable_example() {
    let Some(cc) = gpp() else {
        eprintln!("SKIP cpp_reliable_example: `g++` not on PATH");
        return;
    };
    let src =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../endpoints/cpp/example_reliable.cpp");
    let bin = std::env::temp_dir().join(format!("rel_cpp_ex_{}", std::process::id()));
    compile(cc, &src, &bin);
    let out = Command::new(&bin).output().expect("run example");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success() && stdout.contains("RELIABLE OK"),
        "example failed:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}
