// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Zig endpoint ping-pong: raw generated codegen over UDP, plus the full stack
//! (generated types + endpoint SDK — sync `Client` pull-poll and async
//! `AsyncReader` callback reactor) with XRCE framing. Gated on `zig`.
//!
//! Zig has no module namespace clash to fear here: the generated module
//! (`gen.zig`), the wire runtime (`zdwire.zig`) and the endpoint SDK
//! (`zerodds.zig`) each define their own names, but `@import` keeps them
//! namespaced (`gen.Ping` vs `zerodds.Client`) and only raw `[]const u8`
//! frames cross the transport.
//!
//! ## Toolchain floor
//!
//! `runtime/zig/zdwire.zig` uses the unmanaged `std.ArrayList` — `.empty` and
//! an allocator argument on `append` — which is Zig 0.15 and newer. On an
//! older compiler the runtime does not build, and every case here skips with
//! the compiler's own diagnostic rather than reporting a pass. [`zdwire_builds`]
//! is that probe.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;

use zerodds_endpoint_e2e::codegen::{
    Language, endpoints_dir, runtime_dir, scratch_dir, source, tool_available,
};
use zerodds_endpoint_e2e::{Peer, bind_peer, ping_pong, xcdr2_reader, xcdr2_writer};

fn zig_available() -> bool {
    tool_available("zig", "version")
}

/// Does this compiler build `runtime/zig/zdwire.zig`'s writer at all?
///
/// The probe instantiates `Writer.init` and one write, rather than just
/// compiling the file: Zig is lazy, and a `build-obj` over a runtime nothing
/// references type-checks almost none of it — `zig build-obj zdwire.zig`
/// succeeds on a 0.14 compiler that cannot build the writer.
///
/// Compiled once. `Err` carries the compiler's own message, which the skip
/// prints — so a version floor shows up as the diagnostic that states it, not
/// as a guess about a version number.
fn zdwire_builds() -> Result<(), &'static String> {
    static CELL: OnceLock<Result<(), String>> = OnceLock::new();
    match CELL.get_or_init(|| {
        let dir = scratch_dir("zig_probe");
        std::fs::copy(runtime_dir("zig").join("zdwire.zig"), dir.join("zdwire.zig"))
            .map_err(|e| e.to_string())?;
        std::fs::write(
            dir.join("probe.zig"),
            "const std = @import(\"std\");\n\
             const zdwire = @import(\"zdwire.zig\");\n\
             pub fn main() !void {\n\
             \x20   var gpa = std.heap.DebugAllocator(.{}){};\n\
             \x20   const f = zdwire.PlCdr1Framing{ .pid_flags = \"sentinel-must-understand\", \
             .highest_short_id = 16128, .pad_declared_length = true, \
             .pad_declared_optional_length = true, .member_must_understand = true, \
             .pad_after_value = true };\n\
             \x20   var w = zdwire.Writer.init(gpa.allocator(), zdwire.Endianness.little, 4, f, false);\n\
             \x20   try w.writeI32(1);\n\
             \x20   std.mem.doNotOptimizeAway(w.finish().len);\n\
             }\n",
        )
        .map_err(|e| e.to_string())?;
        let out = Command::new("zig")
            .arg("build-exe")
            .arg("probe.zig")
            .current_dir(&dir)
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr)
                .lines()
                .take(3)
                .collect::<Vec<_>>()
                .join("\n"))
        }
    }) {
        Ok(()) => Ok(()),
        Err(why) => Err(why),
    }
}

/// The generated Ping/Pong module, importing the wire runtime it calls.
fn gen_module() -> String {
    // The two imports the emission expects beside it. `zdwire` is the
    // runtime it calls; `std` is what its `zd_def_*` constructors name
    // (`std.mem.Allocator`), and neither is written by the emitter — the
    // module is composed here, as it is in a real build.
    format!(
        "const std = @import(\"std\");\nconst zdwire = @import(\"zdwire.zig\");\n\n{}",
        source(Language::Zig)
    )
}

/// Lays out a build dir: `gen.zig` (generated types) and, when the app uses the
/// endpoint SDK, copies of `endpoints/zig/src/zerodds.zig` AND `reliable.zig`
/// next to it so the app's relative `@import`s resolve under a plain `zig run`.
/// `zerodds.zig` references `reliable.zig` from a `test {}` block, and zig
/// resolves that file path even for `zig run`, so both must be present.
fn scaffold(dir: &Path, main_src: &str, need_sdk: bool) {
    std::fs::create_dir_all(dir).expect("mkdir");
    std::fs::write(dir.join("gen.zig"), gen_module()).expect("gen.zig");
    std::fs::write(dir.join("main.zig"), main_src).expect("main.zig");
    // The runtime beside the generated module, so `@import("zdwire.zig")`
    // resolves under a plain `zig run`.
    std::fs::copy(
        runtime_dir("zig").join("zdwire.zig"),
        dir.join("zdwire.zig"),
    )
    .expect("copy zdwire.zig");
    if need_sdk {
        let src = endpoints_dir("zig").join("src");
        // `zerodds.zig` names four files from its `test {}` block, and zig
        // resolves those paths even under `zig run` — a missing one is
        // `unable to load '<f>': FileNotFound`, not a skipped test.
        for f in [
            "zerodds.zig",
            "reliable.zig",
            "surface_test.zig",
            "xrce_client.zig",
        ] {
            std::fs::copy(src.join(f), dir.join(f)).unwrap_or_else(|e| panic!("copy sdk {f}: {e}"));
        }
    }
}

/// The gate every case takes: a compiler, and one that builds the runtime.
/// Prints the reason and returns `false` when either is missing.
fn zig_ready(case: &str) -> bool {
    if !zig_available() {
        eprintln!("SKIP {case}: `zig` not on PATH");
        return false;
    }
    if let Err(why) = zdwire_builds() {
        eprintln!(
            "SKIP {case}: this `zig` does not build runtime/zig/zdwire.zig \
             (it needs the unmanaged std.ArrayList, Zig 0.15+):\n{why}"
        );
        return false;
    }
    true
}

// ---- 1) raw codegen over a plain UDP socket (no SDK, no XRCE frame) ----

const ZIG_RAW_MAIN: &str = r#"const std = @import("std");
const gen = @import("gen.zig");
const zdwire = @import("zdwire.zig");

// XCDR2 little-endian. File scope: the async callback below needs it too.
const zd_pid_flags = "sentinel-must-understand";
const zd_highest_short_id = 16128;
// The writer takes the whole framing; the reader takes the two fields of
// it that a decode consults, as its own arguments.
const zd_framing = zdwire.PlCdr1Framing{ .pid_flags = zd_pid_flags, .highest_short_id = zd_highest_short_id, .pad_declared_length = true, .pad_declared_optional_length = true, .member_must_understand = true, .pad_after_value = true };

// Zig 0.16: `main` takes `std.process.Init`, sockets live in
// `std.Io.net`, and the ten seconds this program used to set as
// SO_RCVTIMEO are the timeout argument of the receive.
const net = std.Io.net;
const recv_timeout: std.Io.Timeout = .{ .duration = .{
    .raw = .fromSeconds(10),
    .clock = .awake,
} };

pub fn main(zd_init: std.process.Init) !void {
    const alloc = zd_init.arena.allocator();
    const args = try zd_init.minimal.args.toSlice(alloc);
    const port = try std.fmt.parseInt(u16, args[1], 10);

    const bind_addr = try net.IpAddress.parseIp4("127.0.0.1", 0);
    const sock = try net.IpAddress.bind(&bind_addr, zd_init.io, .{ .mode = .dgram });
    defer sock.close(zd_init.io);
    const peer = try net.IpAddress.parseIp4("127.0.0.1", port);

    var zd_w = zdwire.Writer.init(alloc, zdwire.Endianness.little, 4, zd_framing, false);
    try gen.zd_enc_Ping(&zd_w, .{ .seq = 1, .msg = "hello from app" });
    const sample = zd_w.finish();
    defer alloc.free(sample);
    try sock.send(zd_init.io, &peer, sample);

    var buf: [4096]u8 = undefined;
    const msg = try sock.receiveTimeout(zd_init.io, &buf, recv_timeout);
    var zd_r = zdwire.Reader.init(alloc, msg.data, zdwire.Endianness.little, 4, zd_pid_flags, zd_highest_short_id);
    const pong = try gen.zd_dec_Pong(&zd_r, zdwire.TryConstruct.discard);
    var stdout_buf: [4096]u8 = undefined;
    var stdout_file = std.Io.File.stdout().writer(zd_init.io, &stdout_buf);
    const stdout = &stdout_file.interface;
    defer stdout.flush() catch {};
    try stdout.print("PONG seq={d} reply={s}\n", .{ pong.seq, pong.reply });
}
"#;

fn build_zig_raw(port: u16) -> Child {
    let dir = scratch_dir("zig_raw");
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
    if !zig_ready("zig_raw_udp") {
        return;
    }
    // The raw app sends a bare XCDR2 Ping (no XRCE frame); run a minimal peer.
    use std::net::UdpSocket;
    use std::time::Duration;
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind");
    let port = sock.local_addr().expect("addr").port();
    let child = build_zig_raw(port);
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

const ZIG_ENDPOINT_MAIN: &str = r#"const std = @import("std");
const gen = @import("gen.zig");
const zdwire = @import("zdwire.zig");

// XCDR2 little-endian. File scope: the async callback below needs it too.
const zd_pid_flags = "sentinel-must-understand";
const zd_highest_short_id = 16128;
// The writer takes the whole framing; the reader takes the two fields of
// it that a decode consults, as its own arguments.
const zd_framing = zdwire.PlCdr1Framing{ .pid_flags = zd_pid_flags, .highest_short_id = zd_highest_short_id, .pad_declared_length = true, .pad_declared_optional_length = true, .member_must_understand = true, .pad_after_value = true };
const zerodds = @import("zerodds.zig");

// Zig 0.16: sockets live in `std.Io.net` and every call takes the `Io`
// the entry point is given. The ten seconds this program used to set as
// SO_RCVTIMEO are the timeout argument of the receive.
const net = std.Io.net;
const recv_timeout: std.Io.Timeout = .{ .duration = .{
    .raw = .fromSeconds(10),
    .clock = .awake,
} };

// A real UDP datagram transport implementing the SDK's function-pointer vtable.
const UdpTransport = struct {
    io: std.Io,
    sock: net.Socket,
    peer: net.IpAddress,
    fn deliver(ctx: *anyopaque, frame: []const u8) bool {
        const self: *UdpTransport = @ptrCast(@alignCast(ctx));
        self.sock.send(self.io, &self.peer, frame) catch return false;
        return true;
    }
    fn receive(ctx: *anyopaque, buf: []u8) ?usize {
        const self: *UdpTransport = @ptrCast(@alignCast(ctx));
        const msg = self.sock.receiveTimeout(self.io, buf, recv_timeout) catch return null;
        if (msg.data.len == 0) return null;
        return msg.data.len;
    }
};

// The async consumer: decode the Pong in the callback and print the line.
const Collector = struct {
    alloc: std.mem.Allocator,
    printed: bool = false,
    io: std.Io,
    fn onSample(ctx: *anyopaque, body: []const u8) void {
        const self: *Collector = @ptrCast(@alignCast(ctx));
        var zd_r = zdwire.Reader.init(self.alloc, body, zdwire.Endianness.little, 4, zd_pid_flags, zd_highest_short_id);
        const pong = gen.zd_dec_Pong(&zd_r, zdwire.TryConstruct.discard) catch return;
        var stdout_buf: [4096]u8 = undefined;
        var stdout_file = std.Io.File.stdout().writer(self.io, &stdout_buf);
        const stdout = &stdout_file.interface;
        stdout.print("PONG seq={d} reply={s}\n", .{ pong.seq, pong.reply }) catch {};
        stdout.flush() catch {};
        self.printed = true;
    }
};

pub fn main(zd_init: std.process.Init) !void {
    const alloc = zd_init.arena.allocator();
    const args = try zd_init.minimal.args.toSlice(alloc);
    const mode = args[1];
    const port = try std.fmt.parseInt(u16, args[2], 10);

    const bind_addr = try net.IpAddress.parseIp4("127.0.0.1", 0);
    const sock = try net.IpAddress.bind(&bind_addr, zd_init.io, .{ .mode = .dgram });
    defer sock.close(zd_init.io);

    var tr = UdpTransport{
        .io = zd_init.io,
        .sock = sock,
        .peer = try net.IpAddress.parseIp4("127.0.0.1", port),
    };
    const t = zerodds.Transport{ .ctx = &tr, .deliver = UdpTransport.deliver, .receive = UdpTransport.receive };

    // The app marshals a typed Ping via the GENERATED codegen, frames + sends it
    // through the endpoint SDK, then decodes the Pong the DDS peer replies.
    var zd_w = zdwire.Writer.init(alloc, zdwire.Endianness.little, 4, zd_framing, false);
    try gen.zd_enc_Ping(&zd_w, .{ .seq = 1, .msg = "hello from app" });
    const sample = zd_w.finish();
    defer alloc.free(sample);

    var client = zerodds.Client{ .transport = &t };
    // `Client.write` reports a frame it cannot build as an error and a
    // transport that refused as `false`; the two are different answers.
    if (!try client.write(sample)) return error.WriteFailed;

    if (std.mem.eql(u8, mode, "async")) {
        var col = Collector{ .alloc = alloc, .io = zd_init.io };
        var reader = zerodds.AsyncReader{ .transport = &t, .on_sample = Collector.onSample, .ctx = &col };
        _ = reader.run(1);
        if (!col.printed) return error.NoPong;
    } else {
        const body = client.poll() orelse return error.NoPong;
        var zd_r = zdwire.Reader.init(alloc, body, zdwire.Endianness.little, 4, zd_pid_flags, zd_highest_short_id);
        const pong = try gen.zd_dec_Pong(&zd_r, zdwire.TryConstruct.discard);
        var stdout_buf: [4096]u8 = undefined;
        var stdout_file = std.Io.File.stdout().writer(zd_init.io, &stdout_buf);
        const stdout = &stdout_file.interface;
        defer stdout.flush() catch {};
        try stdout.print("PONG seq={d} reply={s}\n", .{ pong.seq, pong.reply });
    }
}
"#;

fn build_zig_endpoint(mode: &str, port: u16) -> Child {
    let dir = scratch_dir(&format!("zig_ep_{mode}"));
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
    if !zig_ready(&format!("zig endpoint {mode}")) {
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
