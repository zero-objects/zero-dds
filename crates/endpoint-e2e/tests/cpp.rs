// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! C++ endpoint ping-pong: the generated DDS-PSM-Cxx `topic_type_support`
//! codec (`idl-cpp`) sends a typed `Ping` over a real POSIX UDP socket and
//! decodes the `Pong`. The raw case uses the generated `encode`/`decode`
//! directly; the sync/async cases layer the C++ endpoint SDK
//! (`zerodds_endpoint.h` XRCE framing, `zerodds::AsyncReader`/`AsyncWriter`)
//! over the same wire. Gated on `g++`/`gcc`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{IDL, Peer, bind_peer, ping_pong};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_cpp::{CppGenOptions, generate_cpp_header};

/// `(c++ compiler, c compiler)` if both are on the `PATH`.
fn compilers() -> Option<(&'static str, &'static str)> {
    let ok = |cc: &str| {
        Command::new(cc)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .ok()
            .is_some_and(|s| s.success())
    };
    if ok("g++") && ok("gcc") {
        Some(("g++", "gcc"))
    } else {
        None
    }
}

/// The generated DDS-PSM-Cxx header for the shared `Ping`/`Pong` IDL.
fn gen_header() -> String {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse");
    generate_cpp_header(&ast, &CppGenOptions::default()).expect("gen")
}

fn endpoints() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../endpoints")
}

// The app source. One `#include "<gen header>"` token is patched in per build.
// Modes: `raw` (bare XCDR, no framing), `sync` (poll the SDK transport), and
// `async` (event-driven `AsyncReader`/`AsyncWriter`).
const APP_CPP: &str = r####"
#include <cstdio>
#include <cstring>
#include <cstdint>
#include <cstdlib>
#include <string>
#include <vector>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <sys/time.h>
#include <unistd.h>

#include "__GENHDR__"
#include "zerodds_endpoint.h"
#include "zerodds_async.hpp"

namespace {

// Integrator-supplied transport (ADR 0013 frame-hook): a POSIX UDP socket.
struct UdpCtx {
    int fd;
    sockaddr_in peer;
};

extern "C" int udp_deliver(void* c, const unsigned char* frame, std::size_t len) {
    UdpCtx* u = static_cast<UdpCtx*>(c);
    if (sendto(u->fd, frame, len, 0,
               reinterpret_cast<sockaddr*>(&u->peer), sizeof(u->peer)) < 0) {
        return ZDW_T_ERROR;
    }
    return ZDW_T_OK;
}

extern "C" int udp_receive(void* c, unsigned char* buf, std::size_t cap, std::size_t* len) {
    UdpCtx* u = static_cast<UdpCtx*>(c);
    ssize_t n = recvfrom(u->fd, buf, cap, 0, nullptr, nullptr);
    if (n < 0) {
        return ZDW_T_AGAIN;  // SO_RCVTIMEO expiry / would-block
    }
    *len = static_cast<std::size_t>(n);
    return ZDW_T_OK;
}

int open_udp() {
    int fd = socket(AF_INET, SOCK_DGRAM, 0);
    if (fd < 0) {
        return -1;
    }
    timeval tv;
    tv.tv_sec = 0;
    tv.tv_usec = 100000;  // 100 ms poll granularity
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof(tv));
    return fd;
}

sockaddr_in peer_addr(const char* port) {
    sockaddr_in a;
    std::memset(&a, 0, sizeof(a));
    a.sin_family = AF_INET;
    a.sin_port = htons(static_cast<unsigned short>(std::atoi(port)));
    inet_pton(AF_INET, "127.0.0.1", &a.sin_addr);
    return a;
}

std::vector<uint8_t> encode_ping() {
    ::Ping p;
    p.seq(1);
    p.msg("hello from app");
    return ::dds::topic::topic_type_support<::Ping>::encode(p);
}

// Decode a Pong body and print the exact line the Rust harness asserts.
void print_pong(const unsigned char* body, std::size_t len) {
    ::Pong pong = ::dds::topic::topic_type_support<::Pong>::decode(
        body, len, ::dds::topic::xcdr2::XcdrVersion::Xcdr2, false);
    std::printf("PONG seq=%d reply=%s\n",
                static_cast<int>(pong.seq()), pong.reply().c_str());
}

// --- raw: generated encode/decode over a bare UDP datagram (no XRCE frame) ---
int run_raw(const char* port) {
    int fd = open_udp();
    if (fd < 0) {
        std::fprintf(stderr, "raw: socket\n");
        return 1;
    }
    sockaddr_in peer = peer_addr(port);
    std::vector<uint8_t> body = encode_ping();
    if (sendto(fd, body.data(), body.size(), 0,
               reinterpret_cast<sockaddr*>(&peer), sizeof(peer)) < 0) {
        std::fprintf(stderr, "raw: send\n");
        return 1;
    }
    unsigned char buf[4096];
    for (int i = 0; i < 100; ++i) {  // up to ~10 s
        ssize_t n = recvfrom(fd, buf, sizeof(buf), 0, nullptr, nullptr);
        if (n > 0) {
            print_pong(buf, static_cast<std::size_t>(n));
            return 0;
        }
    }
    std::fprintf(stderr, "raw: no pong\n");
    return 1;
}

// --- sync: endpoint SDK, poll the transport for the reply ---
int run_sync(const char* port) {
    UdpCtx u;
    u.fd = open_udp();
    if (u.fd < 0) {
        std::fprintf(stderr, "sync: socket\n");
        return 1;
    }
    u.peer = peer_addr(port);
    zdw_transport t;
    t.ctx = &u;
    t.deliver = udp_deliver;
    t.receive = udp_receive;

    std::vector<uint8_t> body = encode_ping();
    unsigned char frame[4096];
    std::size_t flen = zdw_xrce_write_frame(frame, sizeof(frame),
        ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT, 1,
        body.data(), body.size());
    if (flen == 0 || zdw_endpoint_send(&t, frame, flen) != ZDW_T_OK) {
        std::fprintf(stderr, "sync: send\n");
        return 1;
    }

    unsigned char rx[4096];
    std::size_t rxlen = 0;
    for (int i = 0; i < 100; ++i) {
        if (zdw_endpoint_recv(&t, rx, sizeof(rx), &rxlen) == ZDW_T_OK) {
            const unsigned char* pbody = nullptr;
            std::size_t pblen = 0;
            if (zdw_xrce_read_frame(rx, rxlen, &pbody, &pblen) == ZDW_T_OK) {
                print_pong(pbody, pblen);
                return 0;
            }
        }
    }
    std::fprintf(stderr, "sync: no pong\n");
    return 1;
}

// --- async: event-driven AsyncReader/AsyncWriter over the same transport ---
int run_async(const char* port) {
    UdpCtx u;
    u.fd = open_udp();
    if (u.fd < 0) {
        std::fprintf(stderr, "async: socket\n");
        return 1;
    }
    u.peer = peer_addr(port);
    zdw_transport t;
    t.ctx = &u;
    t.deliver = udp_deliver;
    t.receive = udp_receive;

    bool got = false;
    int rseq = 0;
    std::string rreply;
    unsigned char rxbuf[4096];
    zerodds::AsyncReader reader(&t, rxbuf, sizeof(rxbuf),
        [&](const unsigned char* body, std::size_t len) {
            ::Pong pong = ::dds::topic::topic_type_support<::Pong>::decode(
                body, len, ::dds::topic::xcdr2::XcdrVersion::Xcdr2, false);
            rseq = static_cast<int>(pong.seq());
            rreply = pong.reply();
            got = true;
        });

    unsigned char txbuf[4096];
    zerodds::AsyncWriter writer(&t, txbuf, sizeof(txbuf),
        ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT);
    std::vector<uint8_t> body = encode_ping();
    if (!writer.write(body.data(), body.size())) {
        std::fprintf(stderr, "async: write\n");
        return 1;
    }

    for (int i = 0; i < 100 && !got; ++i) {
        reader.run(1);
    }
    if (!got) {
        std::fprintf(stderr, "async: no pong\n");
        return 1;
    }
    std::printf("PONG seq=%d reply=%s\n", rseq, rreply.c_str());
    return 0;
}

}  // namespace

int main(int argc, char** argv) {
    if (argc < 3) {
        std::fprintf(stderr, "usage: %s <mode> <port>\n", argv[0]);
        return 2;
    }
    std::string mode = argv[1];
    if (mode == "raw") {
        return run_raw(argv[2]);
    }
    if (mode == "sync") {
        return run_sync(argv[2]);
    }
    if (mode == "async") {
        return run_async(argv[2]);
    }
    std::fprintf(stderr, "unknown mode %s\n", argv[1]);
    return 2;
}
"####;

/// Builds the C++ app for `mode` (compiling the C reactor + the app) and spawns
/// it against `port`. Panics with the compiler output on any build failure.
fn build_app(cc_cpp: &str, cc_c: &str, mode: &str, port: u16) -> Child {
    let ep = endpoints();
    let c_include = ep.join("c/include");
    let cpp_include = ep.join("cpp/include");
    // The generated DDS-PSM-Cxx header pulls in the support headers
    // (`dds/topic/TopicTraits.hpp`, `xcdr2.hpp`, ...) shipped in `crates/cpp`.
    let psm_include = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../cpp/include");
    let endpoint_c = ep.join("c/src/zerodds_endpoint.c");
    let async_c = ep.join("c/src/zerodds_async.c");

    let dir = std::env::temp_dir().join(format!("pp_cpp_{mode}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");

    let hdr = dir.join("pingpong.hpp");
    std::fs::write(&hdr, gen_header()).expect("write header");
    let app = dir.join("app.cpp");
    std::fs::write(&app, APP_CPP.replace("__GENHDR__", &hdr.to_string_lossy())).expect("write app");

    // Compile the audited C reactor as C, then link the C++ app against it.
    let endpoint_o = dir.join("endpoint.o");
    let async_o = dir.join("async.o");
    for (src, obj) in [(&endpoint_c, &endpoint_o), (&async_c, &async_o)] {
        let out = Command::new(cc_c)
            .args(["-std=c99", "-Wall", "-c"])
            .arg(format!("-I{}", c_include.display()))
            .arg(src)
            .arg("-o")
            .arg(obj)
            .output()
            .expect("spawn cc");
        assert!(
            out.status.success(),
            "compile {}:\n{}",
            src.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let bin = dir.join("app");
    let out = Command::new(cc_cpp)
        .args(["-std=c++17", "-Wall", "-Wno-unused"])
        .arg(format!("-I{}", c_include.display()))
        .arg(format!("-I{}", cpp_include.display()))
        .arg(format!("-I{}", psm_include.display()))
        .arg(&app)
        .arg(&endpoint_o)
        .arg(&async_o)
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("spawn g++");
    assert!(
        out.status.success(),
        "link app.cpp:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    Command::new(&bin)
        .arg(mode)
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn app")
}

// ---- 1) raw generated encode/decode over a plain UDP socket ----

#[test]
fn cpp_raw_udp() {
    let Some((cc_cpp, cc_c)) = compilers() else {
        eprintln!("SKIP cpp_raw_udp: `g++`/`gcc` not on PATH");
        return;
    };
    // The raw app sends a bare XCDR2 Ping (no XRCE frame); run a minimal peer.
    use std::net::UdpSocket;
    use std::time::Duration;
    use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let port = sock.local_addr().expect("addr").port();
    let child = build_app(cc_cpp, cc_c, "raw", port);
    sock.set_read_timeout(Some(Duration::from_secs(30)))
        .expect("timeout");
    let mut buf = [0u8; 4096];
    let (n, app) = sock.recv_from(&mut buf).expect("recv");
    let mut r = BufferReader::new(&buf[..n], Endianness::Little).xcdr2();
    let seq = r.read_u32().expect("seq");
    let msg = r.read_string().expect("msg");
    assert_eq!((seq, msg.as_str()), (1, "hello from app"));
    let mut w = BufferWriter::new(Endianness::Little).xcdr2();
    w.write_u32(seq).expect("s");
    w.write_string(&format!("pong:{msg}")).expect("r");
    sock.send_to(&w.into_bytes(), app).expect("send");
    let out = child.wait_with_output().expect("wait");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("PONG seq=1 reply=pong:hello from app"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---- 2/3) full stack: generated types + endpoint SDK (sync/async) over XRCE ----

fn endpoint_case(mode: &str) {
    let Some((cc_cpp, cc_c)) = compilers() else {
        eprintln!("SKIP cpp endpoint {mode}: `g++`/`gcc` not on PATH");
        return;
    };
    let peer: Peer = bind_peer().expect("bind peer");
    let child = build_app(cc_cpp, cc_c, mode, peer.port);
    ping_pong(&peer, child, &format!("cpp/{mode}"));
}

#[test]
fn cpp_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn cpp_endpoint_async() {
    endpoint_case("async");
}
