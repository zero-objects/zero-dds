// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Zig endpoint ping-pong: raw generated codegen over UDP, plus the full stack
//! (generated types + endpoint SDK — sync `Client` pull-poll and async
//! `AsyncReader` callback reactor) with XRCE framing. Gated on `zig`.
//!
//! Zig has no module namespace clash to fear here: the generated module
//! (`gen.zig`) and the endpoint SDK (`zerodds.zig`) each define their own
//! `Endian`/`Writer`/`Reader`, but `@import` keeps them namespaced (`gen.Ping`
//! vs `zerodds.Client`) and only raw `[]const u8` frames cross the transport.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::Path;
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{IDL, Peer, bind_peer, ping_pong};
use zerodds_idl::config::ParserConfig;
use zerodds_idl_zig::{ZigGenOptions, generate_zig_module};

fn zig_available() -> bool {
    Command::new("zig").arg("version").output().is_ok()
}

/// The generated Ping/Pong module (self-contained: its own wire core + the
/// `marshalXCDR`/`unmarshalXCDR` methods).
fn gen_module() -> String {
    let ast = zerodds_idl::parse(IDL, &ParserConfig::default()).expect("parse");
    generate_zig_module(&ast, &ZigGenOptions::default()).expect("gen")
}

/// Lays out a build dir: `gen.zig` (generated types) and, when the app uses the
/// endpoint SDK, a copy of `endpoints/zig/src/zerodds.zig` next to it so the
/// app's relative `@import` resolves under a plain `zig run`.
fn scaffold(dir: &Path, main_src: &str, need_sdk: bool) {
    std::fs::create_dir_all(dir).expect("mkdir");
    std::fs::write(dir.join("gen.zig"), gen_module()).expect("gen.zig");
    std::fs::write(dir.join("main.zig"), main_src).expect("main.zig");
    if need_sdk {
        let sdk = format!(
            "{}/../../endpoints/zig/src/zerodds.zig",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::copy(&sdk, dir.join("zerodds.zig")).expect("copy sdk");
    }
}

// ---- 1) raw codegen over a plain UDP socket (no SDK, no XRCE frame) ----

const ZIG_RAW_MAIN: &str = r#"const std = @import("std");
const gen = @import("gen.zig");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const args = try std.process.argsAlloc(alloc);
    defer std.process.argsFree(alloc, args);
    const port = try std.fmt.parseInt(u16, args[1], 10);

    const fd = try std.posix.socket(std.posix.AF.INET, std.posix.SOCK.DGRAM, 0);
    defer std.posix.close(fd);
    const bind_addr = try std.net.Address.parseIp4("127.0.0.1", 0);
    try std.posix.bind(fd, &bind_addr.any, bind_addr.getOsSockLen());
    const tv = std.posix.timeval{ .tv_sec = 10, .tv_usec = 0 };
    try std.posix.setsockopt(fd, std.posix.SOL.SOCKET, std.posix.SO.RCVTIMEO, std.mem.asBytes(&tv));
    const peer = try std.net.Address.parseIp4("127.0.0.1", port);

    const ping = gen.Ping{ .seq = 1, .msg = "hello from app" };
    const sample = try ping.marshalXCDR(.little, alloc);
    defer alloc.free(sample);
    _ = try std.posix.sendto(fd, sample, 0, &peer.any, peer.getOsSockLen());

    var buf: [4096]u8 = undefined;
    const n = try std.posix.recvfrom(fd, &buf, 0, null, null);
    const pong = try gen.Pong.unmarshalXCDR(buf[0..n], .little, alloc);
    const stdout = std.io.getStdOut().writer();
    try stdout.print("PONG seq={d} reply={s}\n", .{ pong.seq, pong.reply });
}
"#;

fn build_zig_raw(port: u16) -> Child {
    let dir = std::env::temp_dir().join(format!("pp_zig_raw_{}", std::process::id()));
    scaffold(&dir, ZIG_RAW_MAIN, false);
    Command::new("zig")
        .arg("run")
        .arg(dir.join("main.zig"))
        .arg("--")
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn zig")
}

#[test]
fn zig_raw_udp() {
    if !zig_available() {
        eprintln!("SKIP zig_raw_udp: `zig` not on PATH");
        return;
    }
    // The raw app sends a bare XCDR2 Ping (no XRCE frame); run a minimal peer.
    use std::net::UdpSocket;
    use std::time::Duration;
    use zerodds_cdr::{BufferReader, BufferWriter, Endianness};
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let port = sock.local_addr().expect("addr").port();
    let child = build_zig_raw(port);
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

const ZIG_ENDPOINT_MAIN: &str = r#"const std = @import("std");
const gen = @import("gen.zig");
const zerodds = @import("zerodds.zig");

// A real UDP datagram transport implementing the SDK's function-pointer vtable.
const UdpTransport = struct {
    fd: std.posix.socket_t,
    peer: std.net.Address,
    fn deliver(ctx: *anyopaque, frame: []const u8) bool {
        const self: *UdpTransport = @ptrCast(@alignCast(ctx));
        _ = std.posix.sendto(self.fd, frame, 0, &self.peer.any, self.peer.getOsSockLen()) catch return false;
        return true;
    }
    fn receive(ctx: *anyopaque, buf: []u8) ?usize {
        const self: *UdpTransport = @ptrCast(@alignCast(ctx));
        const n = std.posix.recvfrom(self.fd, buf, 0, null, null) catch return null;
        if (n == 0) return null;
        return n;
    }
};

// The async consumer: decode the Pong in the callback and print the line.
const Collector = struct {
    alloc: std.mem.Allocator,
    printed: bool = false,
    fn onSample(ctx: *anyopaque, body: []const u8) void {
        const self: *Collector = @ptrCast(@alignCast(ctx));
        const pong = gen.Pong.unmarshalXCDR(body, .little, self.alloc) catch return;
        const stdout = std.io.getStdOut().writer();
        stdout.print("PONG seq={d} reply={s}\n", .{ pong.seq, pong.reply }) catch {};
        self.printed = true;
    }
};

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const args = try std.process.argsAlloc(alloc);
    defer std.process.argsFree(alloc, args);
    const mode = args[1];
    const port = try std.fmt.parseInt(u16, args[2], 10);

    const fd = try std.posix.socket(std.posix.AF.INET, std.posix.SOCK.DGRAM, 0);
    defer std.posix.close(fd);
    const bind_addr = try std.net.Address.parseIp4("127.0.0.1", 0);
    try std.posix.bind(fd, &bind_addr.any, bind_addr.getOsSockLen());
    const tv = std.posix.timeval{ .tv_sec = 10, .tv_usec = 0 };
    try std.posix.setsockopt(fd, std.posix.SOL.SOCKET, std.posix.SO.RCVTIMEO, std.mem.asBytes(&tv));

    var tr = UdpTransport{ .fd = fd, .peer = try std.net.Address.parseIp4("127.0.0.1", port) };
    const t = zerodds.Transport{ .ctx = &tr, .deliver = UdpTransport.deliver, .receive = UdpTransport.receive };

    // The app marshals a typed Ping via the GENERATED codegen, frames + sends it
    // through the endpoint SDK, then decodes the Pong the DDS peer replies.
    const ping = gen.Ping{ .seq = 1, .msg = "hello from app" };
    const sample = try ping.marshalXCDR(.little, alloc);
    defer alloc.free(sample);

    var client = zerodds.Client{ .transport = &t };
    if (!client.write(sample)) return error.WriteFailed;

    if (std.mem.eql(u8, mode, "async")) {
        var col = Collector{ .alloc = alloc };
        var reader = zerodds.AsyncReader{ .transport = &t, .on_sample = Collector.onSample, .ctx = &col };
        _ = reader.run(1);
        if (!col.printed) return error.NoPong;
    } else {
        const body = client.poll() orelse return error.NoPong;
        const pong = try gen.Pong.unmarshalXCDR(body, .little, alloc);
        const stdout = std.io.getStdOut().writer();
        try stdout.print("PONG seq={d} reply={s}\n", .{ pong.seq, pong.reply });
    }
}
"#;

fn build_zig_endpoint(mode: &str, port: u16) -> Child {
    let dir = std::env::temp_dir().join(format!("pp_zig_ep_{mode}_{}", std::process::id()));
    scaffold(&dir, ZIG_ENDPOINT_MAIN, true);
    Command::new("zig")
        .arg("run")
        .arg(dir.join("main.zig"))
        .arg("--")
        .arg(mode)
        .arg(port.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn zig")
}

fn endpoint_case(mode: &str) {
    if !zig_available() {
        eprintln!("SKIP zig endpoint {mode}: `zig` not on PATH");
        return;
    }
    let peer: Peer = bind_peer().expect("bind peer");
    let child = build_zig_endpoint(mode, peer.port);
    ping_pong(&peer, child, &format!("zig/{mode}"));
}

#[test]
fn zig_endpoint_sync() {
    endpoint_case("sync");
}

#[test]
fn zig_endpoint_async() {
    endpoint_case("async");
}
