// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! C endpoint ping-pong — `zerodds-endpoint-c-1.0` §3.4.
//!
//! The generated C codec (`codegen::source(Language::C)`, calling the
//! `runtime/c` wire runtime) sends a typed `Ping` over a real POSIX UDP socket
//! and decodes the `Pong`. The raw case uses the generated `zd_enc_Ping` /
//! `zd_dec_Pong` directly against a bare socket; the sync and async cases layer
//! the C endpoint SDK over the same wire — `zdw_transport` plus
//! `zdw_xrce_write_frame`/`zdw_xrce_read_frame` (`endpoints/c/include/zerodds_endpoint.h`)
//! for sync, and `zdw_async_writer`/`zdw_async_reader`
//! (`endpoints/c/include/zerodds_async.h`) for async. Gated on `cc`.
//!
//! Follows the `cpp.rs` pattern: same three cases, same shared Rust peer.
//!
//! ## Why this app declares no `_POSIX_C_SOURCE`
//!
//! `endpoints/c/test/reliable_udp_app.c` sets `_POSIX_C_SOURCE 200809L`, which
//! under Apple's libc hides `INADDR_LOOPBACK` — a BSD socket extension, not a
//! POSIX constant — and that file therefore does not compile on macOS. This app
//! sets no feature-test macro and spells the loopback address `htonl(0x7f000001)`,
//! so it builds on both libcs.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use zerodds_endpoint_e2e::codegen::{
    Language, endpoints_dir, runtime_dir, scratch_dir, source, tool_available,
};
use zerodds_endpoint_e2e::{Peer, bind_peer, ping_pong, xcdr2_reader, xcdr2_writer};

/// The C compiler, if there is one. `cc` is the portable spelling; the C
/// coverage document's own test runs used it too.
fn cc_available() -> bool {
    tool_available("cc", "--version")
}

/// The UDP transport plus the `Ping` the app sends — shared by all three
/// cases. `htonl(0x7f000001)` rather than `INADDR_LOOPBACK`: see the module
/// note. `recv` is non-blocking through a 10 ms `SO_RCVTIMEO`, which is what
/// lets the transport answer `ZDW_T_AGAIN` the way the SDK expects.
const C_PRELUDE: &str = r#"
typedef struct {
    int fd;
    struct sockaddr_in peer;
} udp_ctx;

static int udp_deliver(void *ctx, const unsigned char *frame, size_t len)
{
    udp_ctx *u = (udp_ctx *)ctx;
    ssize_t n = sendto(u->fd, frame, len, 0,
                       (struct sockaddr *)&u->peer, sizeof u->peer);
    return (n == (ssize_t)len) ? ZDW_T_OK : ZDW_T_ERROR;
}

static int udp_receive(void *ctx, unsigned char *buf, size_t cap, size_t *len)
{
    udp_ctx *u = (udp_ctx *)ctx;
    ssize_t n = recv(u->fd, buf, cap, 0);
    if (n < 0) {
        return ZDW_T_AGAIN;
    }
    *len = (size_t)n;
    return ZDW_T_OK;
}

/* Binds a loopback UDP socket with a 10 ms receive timeout and points it at
   the peer port in argv[1]. */
static void udp_open(udp_ctx *u, const char *port)
{
    struct timeval tv;
    u->fd = socket(AF_INET, SOCK_DGRAM, 0);
    if (u->fd < 0) {
        fprintf(stderr, "socket failed\n");
        exit(1);
    }
    tv.tv_sec = 0;
    tv.tv_usec = 10000;
    setsockopt(u->fd, SOL_SOCKET, SO_RCVTIMEO, &tv, sizeof tv);
    memset(&u->peer, 0, sizeof u->peer);
    u->peer.sin_family = AF_INET;
    u->peer.sin_port = htons((unsigned short)atoi(port));
    u->peer.sin_addr.s_addr = htonl(0x7f000001); /* 127.0.0.1 */
}

/* Encodes the shared Ping into `buf`, returning its length. */
static size_t encode_ping(unsigned char *buf, size_t cap)
{
    zdw_writer w;
    Ping ping;
    ping.seq = 1;
    ping.msg = "hello from app";
    zdw_writer_init(&w, buf, cap, ZDW_LITTLE, 4);
    zd_enc_Ping(&w, &ping);
    if (zdw_writer_status(&w) != ZDW_OK) {
        fprintf(stderr, "encode: %s\n", zdw_status_name(zdw_writer_status(&w)));
        exit(1);
    }
    return zdw_writer_length(&w);
}

/* Decodes a Pong body and prints the line the peer asserts on. */
static void print_pong(const unsigned char *body, size_t len)
{
    static unsigned char heap[4096];
    zdw_reader r;
    Pong pong;
    zdw_reader_init(&r, body, len, ZDW_LITTLE, 4);
    zdw_reader_set_heap(&r, heap, sizeof heap);
    zd_dec_Pong(&r, &pong, ZDW_TC_DISCARD);
    if (zdw_reader_status(&r) != ZDW_OK) {
        fprintf(stderr, "decode: %s\n", zdw_status_name(zdw_reader_status(&r)));
        exit(1);
    }
    printf("PONG seq=%d reply=%s\n", (int)pong.seq, pong.reply);
    fflush(stdout);
}
"#;

/// Case 1: the generated codec straight onto a socket, no XRCE envelope.
const C_RAW_MAIN: &str = r#"
int main(int argc, char **argv)
{
    static unsigned char sample[4096];
    static unsigned char rx[4096];
    udp_ctx u;
    size_t n;
    ssize_t got;
    int waited;
    if (argc < 2) {
        fprintf(stderr, "usage: app <port>\n");
        return 2;
    }
    udp_open(&u, argv[1]);
    n = encode_ping(sample, sizeof sample);
    if (udp_deliver(&u, sample, n) != ZDW_T_OK) {
        fprintf(stderr, "send failed\n");
        return 1;
    }
    for (waited = 0; waited < 9000; waited++) {
        got = recv(u.fd, rx, sizeof rx, 0);
        if (got >= 0) {
            print_pong(rx, (size_t)got);
            return 0;
        }
    }
    fprintf(stderr, "raw: no pong\n");
    return 1;
}
"#;

/// Case 2: the same sample inside an XRCE `WRITE_DATA`, built and read back
/// through the endpoint SDK's own framing calls.
const C_SYNC_MAIN: &str = r#"
int main(int argc, char **argv)
{
    static unsigned char sample[4096];
    static unsigned char frame[4096];
    static unsigned char rx[4096];
    udp_ctx u;
    zdw_transport t;
    const unsigned char *body;
    size_t n, flen, rlen, blen;
    int waited;
    if (argc < 2) {
        fprintf(stderr, "usage: app <port>\n");
        return 2;
    }
    udp_open(&u, argv[1]);
    t.ctx = &u;
    t.deliver = udp_deliver;
    t.receive = udp_receive;

    n = encode_ping(sample, sizeof sample);
    flen = zdw_xrce_write_frame(frame, sizeof frame,
                                ZDW_XRCE_SESSION_NOKEY,
                                ZDW_XRCE_STREAM_BEST_EFFORT,
                                1u, ZDW_XRCE_OBJECT_ID(0x001, ZDW_OBJK_DATAWRITER),
                                sample, n);
    if (flen == 0u) {
        fprintf(stderr, "frame did not fit\n");
        return 1;
    }
    if (zdw_endpoint_send(&t, frame, flen) != ZDW_T_OK) {
        fprintf(stderr, "send failed\n");
        return 1;
    }
    for (waited = 0; waited < 9000; waited++) {
        if (zdw_endpoint_recv(&t, rx, sizeof rx, &rlen) != ZDW_T_OK) {
            continue;
        }
        if (zdw_xrce_read_frame(rx, rlen, &body, &blen) != ZDW_T_OK) {
            fprintf(stderr, "not an XRCE frame\n");
            return 1;
        }
        print_pong(body, blen);
        return 0;
    }
    fprintf(stderr, "sync: no pong\n");
    return 1;
}
"#;

/// Case 3: the event-driven surface — `zdw_async_write` frames and delivers,
/// `zdw_async_poll` unwraps and dispatches to the callback.
const C_ASYNC_MAIN: &str = r#"
static int got_pong = 0;

static void on_sample(void *ctx, const unsigned char *body, size_t len)
{
    (void)ctx;
    print_pong(body, len);
    got_pong = 1;
}

int main(int argc, char **argv)
{
    static unsigned char sample[4096];
    static unsigned char txbuf[4096];
    static unsigned char rxbuf[4096];
    udp_ctx u;
    zdw_transport t;
    zdw_async_writer w;
    zdw_async_reader r;
    size_t n;
    int waited;
    if (argc < 2) {
        fprintf(stderr, "usage: app <port>\n");
        return 2;
    }
    udp_open(&u, argv[1]);
    t.ctx = &u;
    t.deliver = udp_deliver;
    t.receive = udp_receive;

    zdw_async_writer_init(&w, &t, txbuf, sizeof txbuf,
                          ZDW_XRCE_SESSION_NOKEY, ZDW_XRCE_STREAM_BEST_EFFORT,
                          ZDW_XRCE_OBJECT_ID(0x001, ZDW_OBJK_DATAWRITER));
    zdw_async_reader_init(&r, &t, rxbuf, sizeof rxbuf, on_sample, NULL);

    n = encode_ping(sample, sizeof sample);
    if (zdw_async_write(&w, sample, n) != ZDW_T_OK) {
        fprintf(stderr, "async write failed\n");
        return 1;
    }
    for (waited = 0; waited < 9000 && !got_pong; waited++) {
        zdw_async_poll(&r);
    }
    if (!got_pong) {
        fprintf(stderr, "async: no pong\n");
        return 1;
    }
    return 0;
}
"#;

/// The complete translation unit for one case: system headers, the wire
/// runtime's and the endpoint SDK's headers, the generated types + codec, the
/// shared transport, then the case's `main`.
fn app_source(main: &str, async_case: bool) -> String {
    let async_include = if async_case {
        "#include \"zerodds_async.h\"\n"
    } else {
        ""
    };
    format!(
        "#include <stdio.h>\n\
         #include <stdlib.h>\n\
         #include <string.h>\n\
         #include <stdint.h>\n\
         #include <unistd.h>\n\
         #include <sys/types.h>\n\
         #include <sys/time.h>\n\
         #include <sys/socket.h>\n\
         #include <netinet/in.h>\n\
         #include <arpa/inet.h>\n\
         #include \"zerodds_wire.h\"\n\
         #include \"zerodds_endpoint.h\"\n\
         {async_include}\n\
         {}\n\
         {C_PRELUDE}\n\
         {main}\n",
        source(Language::C)
    )
}

/// Builds `runtime/c` into `dir/rt` through the runtime's **own** Makefile and
/// returns the static library.
///
/// Its `CFLAGS` are `-std=c99 -Wpedantic -Wconversion -Wsign-conversion`; a
/// hand-rolled `cc` line over the same five sources compiles them under the
/// default standard instead and `float128_decimal.c` does not survive it. The
/// runtime states how it is built, so it is built that way.
///
/// `BUILD=` puts the objects in this test's scratch directory: the checkout's
/// `runtime/c/build` is not written to, so a concurrent build there cannot
/// collide with this one.
fn build_wire_runtime(dir: &std::path::Path) -> PathBuf {
    let rt = runtime_dir("c");
    let build = dir.join("rt");
    let out = Command::new("make")
        .arg("-C")
        .arg(&rt)
        .arg(format!("BUILD={}", build.display()))
        .output()
        .expect("spawn make");
    assert!(
        out.status.success(),
        "make -C runtime/c failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let lib = build.join("libzerodds_wire.a");
    assert!(lib.exists(), "no libzerodds_wire.a at {}", lib.display());
    lib
}

/// Compiles one case into a binary and returns its path.
///
/// `-Wno-unused-function`: the generator emits the codec for **every** type in
/// the model, so each app carries the two functions of the direction it does
/// not use (`zd_enc_Pong`, `zd_dec_Ping`). That is the generator being
/// complete, not this app leaving something behind.
fn build_app(label: &str, main: &str, async_case: bool) -> PathBuf {
    let dir = scratch_dir(label);
    let src = dir.join("app.c");
    std::fs::write(&src, app_source(main, async_case)).expect("write app.c");
    let c = endpoints_dir("c");
    let rt = runtime_dir("c");
    let bin = dir.join("app");
    let lib = build_wire_runtime(&dir);

    let mut cmd = Command::new("cc");
    cmd.arg("-O1")
        .arg("-Wall")
        .arg("-Wextra")
        .arg("-Wno-unused-function")
        // `runtime/c/include` FIRST, and the order is load-bearing: both
        // `runtime/c/include` and `endpoints/c/include` hold a
        // `zerodds_wire.h`, and both guard it with `ZERODDS_WIRE_H`, so
        // whichever is found first silently suppresses the other. The
        // generated codec calls the runtime's (`zdw_writer_init`,
        // `zdw_try_construct`); the endpoint SDK's is its own older C89
        // wire-core, which neither `zerodds_endpoint.c` nor `zerodds_async.c`
        // includes — so putting the runtime first costs the SDK nothing.
        .arg("-I")
        .arg(rt.join("include"))
        .arg("-I")
        .arg(c.join("include"))
        .arg(&src)
        .arg(c.join("src/zerodds_endpoint.c"));
    if async_case {
        cmd.arg(c.join("src/zerodds_async.c"));
    }
    let out = cmd
        .arg(&lib)
        .arg("-lm")
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("spawn cc");
    assert!(
        out.status.success(),
        "cc failed for {label}:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    bin
}

fn spawn(bin: &PathBuf, port: u16) -> Child {
    Command::new(bin)
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn app")
}

// ---- 1) the generated codec over a plain UDP socket ----

#[test]
fn c_raw_udp() {
    if !cc_available() {
        eprintln!("SKIP c_raw_udp: `cc` not on PATH");
        return;
    }
    // The raw app sends a bare XCDR2 Ping (no XRCE frame), so the shared
    // peer's `xrce_unframe` would reject it — this case runs its own peer.
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let port = sock.local_addr().expect("addr").port();
    let bin = build_app("c_raw", C_RAW_MAIN, false);
    let child = spawn(&bin, port);
    sock.set_read_timeout(Some(Duration::from_secs(90)))
        .expect("timeout");
    let mut buf = [0u8; 4096];
    let (n, app) = sock.recv_from(&mut buf).expect("recv ping");
    let mut r = xcdr2_reader(&buf[..n]);
    let seq = r.read_u32().expect("seq");
    let msg = r.read_string().expect("msg");
    assert_eq!((seq, msg.as_str()), (1, "hello from app"));
    let mut w = xcdr2_writer();
    w.write_u32(seq);
    w.write_string(&format!("pong:{msg}"));
    sock.send_to(&w.finish(), app).expect("send pong");
    let out = child.wait_with_output().expect("wait");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("PONG seq=1 reply=pong:hello from app"),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---- 2/3) the endpoint SDK: XRCE framing, sync and async ----

#[test]
fn c_endpoint_sync() {
    if !cc_available() {
        eprintln!("SKIP c_endpoint_sync: `cc` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let bin = build_app("c_ep_sync", C_SYNC_MAIN, false);
    let child = spawn(&bin, peer.port);
    ping_pong(&peer, child, "c/sync");
}

#[test]
fn c_endpoint_async() {
    if !cc_available() {
        eprintln!("SKIP c_endpoint_async: `cc` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let bin = build_app("c_ep_async", C_ASYNC_MAIN, true);
    let child = spawn(&bin, peer.port);
    ping_pong(&peer, child, "c/async");
}
