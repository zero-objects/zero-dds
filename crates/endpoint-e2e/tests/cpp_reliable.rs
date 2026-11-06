// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! C++ reliable endpoint: the `endpoints/cpp` reliable-stream layer
//! (`zerodds_reliable.hpp`) as a live reliable **sender** against the shared
//! Rust reliable peer, its unit and byte-golden binary
//! (`test/test_reliable_cpp.cpp`, which the endpoints-gate column of C++ does
//! not build) and the in-process example. Gated on `g++`. Loss recovery: the
//! peer drops every 3rd datagram, the app retransmits on ACKNACK, all samples
//! arrive gap-free in order. The samples are the `Ping` of the codec the C++
//! generator writes from the IDL, so the run checks IDL, generated codec and
//! runtime together.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, runtime_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

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
#include "__GENHDR__"
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

// XCDR2 little-endian, MAXALIGN four. The PL_CDR1 fields never fire under
// plain XCDR2; they are spelled out because the aggregate is positional.
static const zerodds::wire::PlCdr1Framing kFraming{
    zerodds::wire::PidFlagPolicy::SentinelMustUnderstand, 16128,
    true, true, true, true};

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
#if defined(__linux__)
        // One syscall for the whole batch, which is the point of the
        // decoupled writer's drain thread.
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
#else
        // `sendmmsg` is a Linux syscall; macOS and the BSDs have no
        // equivalent. One `send` per frame delivers the same octets in the
        // same order, so what this test asserts — gap-free reliable delivery
        // under injected loss — is measured identically. What is NOT measured
        // off Linux is the single-syscall batching itself.
        for (std::size_t i = 0; i < fs.size(); ++i) {
            ::send(fd, fs[i].data(), fs[i].size(), 0);
        }
#endif
    };
    auto poll_ack = [&](std::uint8_t* buf, std::size_t cap) -> int {
        ssize_t n = ::recv(fd, buf, cap, MSG_DONTWAIT);
        return n > 0 ? static_cast<int>(n) : -1;
    };

    {
        AsyncWriter w(send_batch, send_one, poll_ack, 1024);
        for (std::uint32_t i = 0; i < count; ++i) {
            // The generated codec writes Ping { seq = i, msg = "reliable" }.
            ::Ping p;
            p.seq = static_cast<std::int32_t>(i);
            p.msg = "reliable";
            zerodds::wire::Writer pw(zerodds::wire::Endianness::Little, 4, kFraming, false);
            zd_enc_Ping(pw, p);
            std::vector<std::uint8_t> b = pw.finish();
            while (!w.enqueue(b.data(), b.size())) { /* ring full: producer spins briefly */ }
        }
        w.finish();
    }  // AsyncWriter dtor joins the drain thread once the window has drained
    std::printf("SENT %u\n", count);
    return 0;
}
"####;

fn build_app(cc: &str, port: u16, count: u32) -> Child {
    // Per-test-unique dir (keyed by the test's unique UDP port): the parallel
    // test runner runs this file's tests as threads in ONE process, so a shared
    // `<name>_<pid>/app` path let one test compile the binary while another
    // exec'd it — ETXTBSY ("Text file busy"). A per-port dir removes the sharing.
    let dir = std::env::temp_dir().join(format!("rel_cpp_{}_{}", std::process::id(), port));
    std::fs::create_dir_all(&dir).expect("mkdir");
    let hdr = dir.join("pingpong.hpp");
    std::fs::write(
        &hdr,
        format!(
            "#pragma once\n#include \"zerodds/wire.hpp\"\n#include <cstdint>\n\
             #include <string>\n#include <vector>\n\n{}",
            source(Language::Cpp)
        ),
    )
    .expect("write header");
    let src = dir.join("app.cpp");
    std::fs::write(&src, APP_CPP.replace("__GENHDR__", &hdr.to_string_lossy())).expect("write app");
    let bin = dir.join("app");
    // The generated codec calls `zerodds::wire`, which is `runtime/cpp`.
    let rt = runtime_dir("cpp");
    let o = Command::new(cc)
        .args([
            "-std=c++17",
            "-pthread",
            "-D_GNU_SOURCE",
            "-Wall",
            "-Wno-unused",
        ])
        .arg(format!("-I{}", cpp_include().display()))
        .arg(format!("-I{}", rt.join("include").display()))
        .arg(&src)
        .arg(rt.join("src/writer.cpp"))
        .arg(rt.join("src/reader.cpp"))
        .arg(rt.join("src/md5.cpp"))
        .arg(rt.join("src/float128.cpp"))
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("spawn g++");
    assert!(
        o.status.success(),
        "compile the reliable app:\n{}",
        String::from_utf8_lossy(&o.stderr)
    );
    Command::new(&bin)
        .arg(port.to_string())
        .arg(count.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn app")
}

/// reliable-endpoint 1.0 §5 point 3: the C++ app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the C++ toolchain; runs in integration-bindings"]
fn reliable_5_3_cpp_samples_arrive_in_order_despite_dropped_datagrams() {
    let Some(cc) = gpp() else {
        eprintln!("SKIP cpp reliable loss recovery: `g++` not on PATH");
        return;
    };
    let count = RELIABLE_SAMPLES;
    let peer = bind_reliable_peer(Some(3)).expect("bind reliable peer");
    let child = build_app(cc, peer.port, u32::try_from(count).expect("count"));
    let delivered = reliable_receive(&peer, child, "cpp/loss", count);
    assert_reliable_pings("cpp/loss", &delivered, count);
}

/// reliable-endpoint 1.0 §5 points 1, 2 and 4: the C++ state machine
/// (`zerodds_reliable.hpp`) passes the reference cases of §3, writes
/// HEARTBEAT and ACKNACK as the goldens of §4, and its `AsyncWriter` enqueue
/// costs less than the inline send; the endpoints-gate column of C++ does
/// not build `test/test_reliable_cpp.cpp`.
#[test]
#[ignore = "needs the C++ toolchain; runs in integration-bindings"]
fn reliable_5_1_cpp_state_machine_passes_the_reference_cases_and_the_goldens() {
    let Some(cc) = gpp() else {
        eprintln!("SKIP cpp reliable unit and golden: `g++` not on PATH");
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
    zerodds_endpoint_e2e::assert_enqueue_below_inline("cpp", &stdout);
}

/// reliable-endpoint 1.0 §5 point 5: the C++ reliable example runs and
/// reports its sequence without gaps; the endpoints-gate column of C++
/// builds it without running it.
#[test]
#[ignore = "needs the C++ toolchain; runs in integration-bindings"]
fn reliable_5_5_cpp_example_recovers_its_sequence_without_gaps() {
    let Some(cc) = gpp() else {
        eprintln!("SKIP cpp reliable example: `g++` not on PATH");
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
