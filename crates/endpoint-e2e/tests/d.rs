// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! D endpoint ping-pong: raw generated codegen over UDP, plus the full stack
//! (generated types + endpoint SDK, sync `Client` and async `AsyncReader`) with
//! XRCE framing. Gated on `gdc`.
//!
//! The generated D module embeds its own wire-core (`Endian`/`Writer`/`Reader`),
//! and so does `endpoints/d/zerodds.d`. To use both in one app we compile the
//! generated code as `module gen;` and pull the SDK in with a *selective*
//! `import zerodds : ...` so the duplicate wire-core symbols never clash.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::codegen::{
    Language, endpoints_dir, runtime_dir, scratch_dir, source, tool_available,
};
use zerodds_endpoint_e2e::{Peer, bind_peer, ping_pong, xcdr2_reader, xcdr2_writer};

/// A D compiler, by the three names the toolchains install. `gdc` is GCC's,
/// `dmd` the reference one, `ldc2` the LLVM one.
fn d_compiler() -> Option<&'static str> {
    ["gdc", "ldc2", "dmd"]
        .into_iter()
        .find(|c| tool_available(c, "--version"))
}

/// Generated Ping/Pong D code, wrapped as `module gen;` so it can coexist with
/// the endpoint SDK's own wire-core, and importing the runtime it calls.
fn gen_module() -> String {
    format!("module gen;\n\nimport zdwire;\n\n{}", source(Language::D))
}

/// The endpoint SDK source (`module zerodds;`), copied next to the app.
fn sdk_source() -> String {
    std::fs::read_to_string(endpoints_dir("d").join("zerodds.d"))
        .expect("read endpoints/d/zerodds.d")
}

/// The XCDR2 little-endian framing both directions use.
const D_FRAMING: &str = "auto zdFraming = zdwire.PlCdr1Framing(\"sentinel-must-understand\", 16128, true, true, true, true);";

/// Compiles `files` (already written into `dir`) into `main_bin`, panicking with
/// the compiler diagnostics on failure.
/// Compiles `files` (already written into `dir`) into `main_bin`, panicking
/// with the compiler diagnostics on failure. `ldc2`/`dmd` name the output with
/// `-of`, `gdc` with `-o`.
fn d_build(compiler: &str, dir: &std::path::Path, files: &[&str]) {
    let mut cmd = Command::new(compiler);
    cmd.args(files);
    if compiler == "gdc" {
        cmd.args(["-o", "main_bin"]);
    } else {
        cmd.arg("-ofmain_bin");
    }
    let build = cmd.current_dir(dir).output().expect("spawn D compiler");
    assert!(
        build.status.success(),
        "{compiler} failed:\n{}\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
}

/// Copies `runtime/d/zdwire.d` next to the app, so `import zdwire` resolves.
fn write_runtime(dir: &std::path::Path) {
    std::fs::copy(runtime_dir("d").join("zdwire.d"), dir.join("zdwire.d")).expect("copy zdwire.d");
}

// ---- 1) raw codegen over a plain UDP socket (bare XCDR2, no XRCE frame) ----

const D_RAW_MAIN: &str = r#"
import gen;
import zdwire;
import std.socket;
import std.stdio : writefln;
import std.conv : to;
import core.time : dur;

void main(string[] args) {
    ushort port = to!ushort(args[1]);
    auto sock = new UdpSocket();
    sock.bind(new InternetAddress("127.0.0.1", cast(ushort) 0));
    auto peer = new InternetAddress("127.0.0.1", port);
    sock.setOption(SocketOptionLevel.SOCKET, SocketOption.RCVTIMEO, dur!"msecs"(10000));

    Ping ping;
    ping.seq = 1;
    ping.msg = "hello from app";
    __FRAMING__
    auto zdW = new zdwire.Writer(zdwire.Endianness.little, 4, zdFraming, false);
    zd_enc_Ping(zdW, ping);
    sock.sendTo(zdW.Finish(), peer);

    ubyte[4096] buf;
    auto n = sock.receiveFrom(buf[]);
    auto zdR = new zdwire.Reader(buf[0 .. n].dup, zdwire.Endianness.little, 4,
                                 zdFraming.pidFlags, zdFraming.highestShortID);
    Pong pong = zd_dec_Pong(zdR, zdwire.TryConstruct.discard);
    writefln("PONG seq=%d reply=%s", pong.seq, pong.reply);
}
"#;

fn build_d_raw(compiler: &str, port: u16) -> Child {
    let dir = scratch_dir("d_raw");
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("gen.d"), gen_module()).expect("write gen");
    std::fs::write(
        dir.join("main.d"),
        D_RAW_MAIN.replace("__FRAMING__", D_FRAMING),
    )
    .expect("write main");
    write_runtime(&dir);
    d_build(compiler, &dir, &["main.d", "gen.d", "zdwire.d"]);
    Command::new("./main_bin")
        .arg(port.to_string())
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn d")
}

#[test]
fn d_raw_udp() {
    let Some(compiler) = d_compiler() else {
        eprintln!("SKIP d_raw_udp: no D compiler (gdc/ldc2/dmd) on PATH");
        return;
    };
    // The raw app sends a bare XCDR2 Ping (no XRCE frame); run a minimal peer.
    use std::net::UdpSocket;
    use std::time::Duration;
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let port = sock.local_addr().expect("addr").port();
    let child = build_d_raw(compiler, port);
    sock.set_read_timeout(Some(Duration::from_secs(30)))
        .expect("timeout");
    let mut buf = [0u8; 4096];
    let (n, app) = sock.recv_from(&mut buf).expect("recv");
    let mut r = xcdr2_reader(&buf[..n]);
    let seq = r.read_u32().expect("seq");
    let msg = r.read_string().expect("msg");
    assert_eq!((seq, msg.as_str()), (1, "hello from app"));
    let mut w = xcdr2_writer();
    w.write_u32(seq);
    w.write_string(&format!("pong:{msg}"));
    sock.send_to(&w.finish(), app).expect("send");
    let out = child.wait_with_output().expect("wait");
    assert!(
        String::from_utf8_lossy(&out.stdout).contains("PONG seq=1 reply=pong:hello from app"),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// ---- 2/3) full stack: generated types + endpoint SDK (sync/async) over XRCE ----

const D_ENDPOINT_MAIN: &str = r#"
import gen;
import zdwire;
import zerodds : Client, AsyncReader, Transport, writeFrame, SessionNoKey, StreamBestEffort,
                 xrceObjectId, ObjkDataWriter;
import std.socket;
import std.stdio : writefln, stderr;
import std.conv : to;
import core.time : dur;

void main(string[] args) {
    string mode = args[1];
    ushort port = to!ushort(args[2]);

    auto sock = new UdpSocket();
    sock.bind(new InternetAddress("127.0.0.1", cast(ushort) 0));
    auto peer = new InternetAddress("127.0.0.1", port);
    // 10ms receive timeout so the poll / actor-feed loops never block forever.
    sock.setOption(SocketOptionLevel.SOCKET, SocketOption.RCVTIMEO, dur!"msecs"(10));

    Ping ping;
    ping.seq = 1;
    ping.msg = "hello from app";
    __FRAMING__
    auto zdW = new zdwire.Writer(zdwire.Endianness.little, 4, zdFraming, false);
    zd_enc_Ping(zdW, ping);
    ubyte[] sample = zdW.Finish();

    ubyte[] body;
    if (mode == "async") {
        auto reader = new AsyncReader();
        sock.sendTo(writeFrame(SessionNoKey, StreamBestEffort, 1,
            xrceObjectId(0x001, ObjkDataWriter), sample), peer);
        ubyte[4096] buf;
        bool got = false;
        foreach (_; 0 .. 1000) {
            auto n = sock.receiveFrom(buf[]);
            if (n > 0) {
                reader.feed(buf[0 .. n].dup);
                body = reader.recv().dup;
                got = true;
                break;
            }
        }
        reader.stop();
        if (!got) {
            stderr.writeln("async: no pong");
            return;
        }
    } else {
        Transport t;
        t.deliver = (ubyte[] frame) { sock.sendTo(frame, peer); };
        t.receive = () {
            ubyte[4096] buf;
            auto n = sock.receiveFrom(buf[]);
            if (n > 0)
                return buf[0 .. n].dup;
            return cast(ubyte[]) null;
        };
        auto c = new Client(t, xrceObjectId(0x001, ObjkDataWriter));
        c.write(sample);
        foreach (_; 0 .. 1000) {
            body = c.poll();
            if (body !is null)
                break;
        }
        if (body is null) {
            stderr.writeln("sync: no pong");
            return;
        }
    }

    auto zdR = new zdwire.Reader(body, zdwire.Endianness.little, 4,
                                 zdFraming.pidFlags, zdFraming.highestShortID);
    Pong pong = zd_dec_Pong(zdR, zdwire.TryConstruct.discard);
    writefln("PONG seq=%d reply=%s", pong.seq, pong.reply);
}
"#;

fn build_d_endpoint(compiler: &str, mode: &str, port: u16) -> Child {
    let dir = scratch_dir(&format!("d_ep_{mode}"));
    std::fs::write(dir.join("gen.d"), gen_module()).expect("write gen");
    std::fs::write(dir.join("zerodds.d"), sdk_source()).expect("write sdk");
    std::fs::write(
        dir.join("main.d"),
        D_ENDPOINT_MAIN.replace("__FRAMING__", D_FRAMING),
    )
    .expect("write main");
    write_runtime(&dir);
    d_build(
        compiler,
        &dir,
        &["main.d", "gen.d", "zerodds.d", "zdwire.d"],
    );
    Command::new("./main_bin")
        .arg(mode)
        .arg(port.to_string())
        .current_dir(&dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn d")
}

fn endpoint_case(mode: &str) {
    let Some(compiler) = d_compiler() else {
        eprintln!("SKIP d endpoint {mode}: no D compiler (gdc/ldc2/dmd) on PATH");
        return;
    };
    let peer: Peer = bind_peer().expect("bind peer");
    let child = build_d_endpoint(compiler, mode, peer.port);
    ping_pong(&peer, child, &format!("d/{mode}"));
}

#[test]
fn d_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn d_endpoint_async() {
    endpoint_case("async");
}
