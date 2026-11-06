// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! C reliable-stream endpoint, live: an app over the C SDK's async-decoupled
//! writer (`zdw_async_ring`: the producer enqueues, a drain thread submits,
//! sends WRITE_DATA and HEARTBEAT and retransmits on ACKNACK) plays the
//! reliable **sender** against the shared Rust
//! [`ReliablePeer`](zerodds_endpoint_e2e), which injects loss. Its samples
//! are the `Ping` of the codec the C generator writes from the IDL; every
//! one must arrive gap-free in order despite drops.
//! Plus the producer-latency comparison (decoupled enqueue vs. inline send,
//! the bench's exit status). Gated on `gcc`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{Language, runtime_dir, scratch_dir, source};
use zerodds_endpoint_e2e::{
    RELIABLE_SAMPLES, assert_reliable_pings, bind_reliable_peer, reliable_receive,
};

fn gcc_available() -> bool {
    Command::new("gcc").arg("--version").output().is_ok()
}

fn c_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../endpoints/c")
}

/// The reliable app: the `Ping` codec `codegen::source(Language::C)` writes
/// from the IDL (calling `runtime/c`), and the SDK's async-decoupled writer
/// `zdw_async_ring` (`endpoints/c/include/zerodds_reliable_async.h`): the
/// producer enqueues the encoded `Ping { seq = i, msg = "reliable" }`, the
/// drain thread frames, sends, and retransmits on ACKNACK.
const C_RELIABLE_MAIN: &str = r#"
static int fd;
static zdw_async_ring ring;

static int deliver(void *ctx, const unsigned char *frame, size_t len)
{
    (void)ctx;
    return send(fd, frame, len, 0) == (ssize_t)len ? 0 : -1;
}

static int poll_in(void *ctx, unsigned char *buf, size_t cap)
{
    ssize_t n;
    (void)ctx;
    n = recv(fd, buf, cap, 0);
    return n > 0 ? (int)n : 0;
}

int main(int argc, char **argv)
{
    struct sockaddr_in peer;
    struct timeval tv;
    int n, i;
    if (argc < 3) {
        fprintf(stderr, "usage: app <port> <count>\n");
        return 2;
    }
    n = atoi(argv[2]);
    fd = socket(AF_INET, SOCK_DGRAM, 0);
    memset(&peer, 0, sizeof peer);
    peer.sin_family = AF_INET;
    peer.sin_port = htons((unsigned short)atoi(argv[1]));
    peer.sin_addr.s_addr = htonl(0x7f000001); /* 127.0.0.1 */
    if (fd < 0 || connect(fd, (struct sockaddr *)&peer, sizeof peer) < 0) {
        perror("socket");
        return 1;
    }
    tv.tv_sec = 0;
    tv.tv_usec = 10000;
    setsockopt(fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof tv);
    if (zdw_async_ring_start(&ring, 0x80, ZDW_XRCE_OBJECT_ID(0x001, ZDW_OBJK_DATAWRITER),
                             deliver, poll_in, NULL) != 0) {
        fprintf(stderr, "zdw_async_ring_start failed\n");
        return 1;
    }
    for (i = 0; i < n; i++) {
        unsigned char body[256];
        zdw_writer w;
        Ping ping;
        ping.seq = i;
        ping.msg = "reliable";
        zdw_writer_init(&w, body, sizeof body, ZDW_LITTLE, 4);
        zd_enc_Ping(&w, &ping);
        if (zdw_writer_status(&w) != ZDW_OK) {
            fprintf(stderr, "encode: %s\n", zdw_status_name(zdw_writer_status(&w)));
            return 1;
        }
        while (zdw_async_ring_enqueue(&ring, body, zdw_writer_length(&w)) != 0) {
            sched_yield(); /* ring full: local backpressure */
        }
    }
    zdw_async_ring_stop(&ring);
    printf("SENT count=%d delivered=%lu\n", n, zdw_async_ring_sent(&ring));
    return 0;
}
"#;

/// Writes the generated codec and the reliable app into a scratch directory
/// and compiles them with the C SDK and `runtime/c`.
fn build_app(tag: &str) -> PathBuf {
    let dir = scratch_dir(&format!("c_reliable_{tag}"));
    let src = dir.join("app.c");
    std::fs::write(
        &src,
        format!(
            "#include <stdio.h>\n#include <stdlib.h>\n#include <string.h>\n#include <stdint.h>\n\
             #include <sched.h>\n#include <unistd.h>\n#include <sys/types.h>\n#include <sys/time.h>\n\
             #include <sys/socket.h>\n#include <netinet/in.h>\n#include <arpa/inet.h>\n\
             #include \"zerodds_wire.h\"\n#include \"zerodds_endpoint.h\"\n\
             #include \"zerodds_reliable_async.h\"\n\n{}\n{C_RELIABLE_MAIN}",
            source(Language::C)
        ),
    )
    .expect("write app.c");
    let rt = runtime_dir("c");
    let build = dir.join("rt");
    let make = Command::new("make")
        .arg("-C")
        .arg(&rt)
        .arg(format!("BUILD={}", build.display()))
        .output()
        .expect("spawn make");
    assert!(
        make.status.success(),
        "make -C runtime/c failed:\n{}",
        String::from_utf8_lossy(&make.stderr)
    );
    let c = c_dir();
    let bin = dir.join("app");
    let out = Command::new("gcc")
        .arg("-O1")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Wno-unused-function")
        .arg("-I")
        .arg(rt.join("include"))
        .arg("-I")
        .arg(c.join("include"))
        .arg(&src)
        .arg(c.join("src/zerodds_endpoint.c"))
        .arg(c.join("src/zerodds_reliable_async.c"))
        .arg(build.join("libzerodds_wire.a"))
        .arg("-lm")
        .arg("-lpthread")
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("spawn gcc");
    assert!(
        out.status.success(),
        "gcc failed for the reliable app:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    bin
}

fn spawn_app(bin: &Path, port: u16, count: usize) -> Child {
    Command::new(bin)
        .arg(port.to_string())
        .arg(count.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn app")
}

/// reliable-endpoint 1.0 §5 point 3: the C app sends its samples on the
/// reliable stream (DDS-XRCE 1.0 §8.4.10, stream id 0x80); the Rust peer
/// drops datagrams and answers with ACKNACK (§8.3.5.11), and every sample
/// arrives once and in order.
#[test]
#[ignore = "needs the C toolchain; runs in integration-bindings"]
fn reliable_5_3_c_samples_arrive_in_order_despite_dropped_datagrams() {
    if !gcc_available() {
        eprintln!("SKIP c reliable loss recovery: `gcc` not on PATH");
        return;
    }
    let bin = build_app("loss");
    let n = RELIABLE_SAMPLES;
    let peer = bind_reliable_peer(Some(3)).expect("bind reliable peer");
    let child = spawn_app(&bin, peer.port, n);
    let got = reliable_receive(&peer, child, "c/loss", n);
    assert_reliable_pings("c/loss", &got, n);
}

/// reliable-endpoint 1.0 §5 point 4 and §2: the C producer's enqueue into
/// the async-decoupled writer costs less than the inline send it replaces,
/// which enters the kernel with `sendto`; both values are printed.
#[test]
#[ignore = "needs the C toolchain; runs in integration-bindings"]
fn reliable_5_4_c_the_producer_enqueue_costs_less_than_the_inline_send() {
    if !gcc_available() {
        eprintln!("SKIP c reliable latency bench: `gcc` not on PATH");
        return;
    }
    let c = c_dir();
    let out = std::env::temp_dir().join(format!("zd_c_reliable_bench_{}", std::process::id()));
    let status = Command::new("gcc")
        .arg("-O2")
        .arg("-std=c11")
        .arg("-I")
        .arg(c.join("include"))
        .arg(c.join("test/bench_reliable_async.c"))
        .arg(c.join("src/zerodds_reliable_async.c"))
        .arg(c.join("src/zerodds_endpoint.c"))
        .arg("-o")
        .arg(&out)
        .arg("-lpthread")
        .status()
        .expect("spawn gcc");
    assert!(
        status.success(),
        "gcc failed to build bench_reliable_async.c"
    );
    let res = Command::new(&out).output().expect("run bench");
    let stdout = String::from_utf8_lossy(&res.stdout);
    eprintln!("c reliable producer latency: {}", stdout.trim());
    assert!(
        stdout.contains("decoupled_enqueue_ns_per_op="),
        "bench output missing latency figure: {stdout}"
    );
    assert!(
        res.status.success(),
        "bench: decoupled enqueue was not <= inline send\n{stdout}\nstderr: {}",
        String::from_utf8_lossy(&res.stderr)
    );
}
