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

use zerodds_endpoint_e2e::{IDL, Peer, bind_peer, ping_pong};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_d::{DGenOptions, generate_d_module};

fn gdc_available() -> bool {
    Command::new("gdc").arg("--version").output().is_ok()
}

/// Generated Ping/Pong D code, wrapped as `module gen;` so it can coexist with
/// the endpoint SDK's own wire-core.
fn gen_module() -> String {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse");
    let src = generate_d_module(&ast, &DGenOptions::default()).expect("gen");
    format!("module gen;\n{src}")
}

/// The endpoint SDK source (`module zerodds;`), copied next to the app.
fn sdk_source() -> String {
    let path = format!("{}/../../endpoints/d/zerodds.d", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).expect("read endpoints/d/zerodds.d")
}

/// Compiles `files` (already written into `dir`) into `main_bin`, panicking with
/// the compiler diagnostics on failure.
fn gdc_build(dir: &std::path::Path, files: &[&str]) {
    let mut cmd = Command::new("gdc");
    cmd.args(files).args(["-o", "main_bin"]).current_dir(dir);
    let build = cmd.output().expect("gdc");
    assert!(
        build.status.success(),
        "gdc failed:\n{}",
        String::from_utf8_lossy(&build.stderr)
    );
}

// ---- 1) raw codegen over a plain UDP socket (bare XCDR2, no XRCE frame) ----

const D_RAW_MAIN: &str = r#"
import gen;
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
    sock.sendTo(ping.marshalXCDR(Endian.LE), peer);

    ubyte[4096] buf;
    auto n = sock.receiveFrom(buf[]);
    Pong pong = UnmarshalXCDRPong(buf[0 .. n].dup, Endian.LE);
    writefln("PONG seq=%d reply=%s", pong.seq, pong.reply);
}
"#;

fn build_d_raw(port: u16) -> Child {
    let dir = std::env::temp_dir().join(format!("pp_d_raw_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("gen.d"), gen_module()).expect("write gen");
    std::fs::write(dir.join("main.d"), D_RAW_MAIN).expect("write main");
    gdc_build(&dir, &["main.d", "gen.d"]);
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
    if !gdc_available() {
        eprintln!("SKIP d_raw_udp: `gdc` not on PATH");
        return;
    }
    // The raw app sends a bare XCDR2 Ping (no XRCE frame); run a minimal peer.
    use std::net::UdpSocket;
    use std::time::Duration;
    use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let port = sock.local_addr().expect("addr").port();
    let child = build_d_raw(port);
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

const D_ENDPOINT_MAIN: &str = r#"
import gen;
import zerodds : Client, AsyncReader, Transport, writeFrame, SessionNoKey, StreamBestEffort;
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
    ubyte[] sample = ping.marshalXCDR(Endian.LE);

    ubyte[] body;
    if (mode == "async") {
        auto reader = new AsyncReader();
        sock.sendTo(writeFrame(SessionNoKey, StreamBestEffort, 1, sample), peer);
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
        auto c = new Client(t);
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

    Pong pong = UnmarshalXCDRPong(body, Endian.LE);
    writefln("PONG seq=%d reply=%s", pong.seq, pong.reply);
}
"#;

fn build_d_endpoint(mode: &str, port: u16) -> Child {
    let dir = std::env::temp_dir().join(format!("pp_d_ep_{mode}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(dir.join("gen.d"), gen_module()).expect("write gen");
    std::fs::write(dir.join("zerodds.d"), sdk_source()).expect("write sdk");
    std::fs::write(dir.join("main.d"), D_ENDPOINT_MAIN).expect("write main");
    gdc_build(&dir, &["main.d", "gen.d", "zerodds.d"]);
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
    if !gdc_available() {
        eprintln!("SKIP d endpoint {mode}: `gdc` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let child = build_d_endpoint(mode, peer.port);
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
