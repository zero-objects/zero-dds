// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

//! Zig reliable-stream endpoint E2E. The zig app is the reliable **sender**
//! (`endpoints/zig/src/reliable.zig`): it submits N samples on `stream_id 0x80`,
//! sends `WRITE_DATA`, and retransmits on `ACKNACK` from the shared Rust
//! [`ReliablePeer`], which injects loss. Also runs the zig unit + byte-golden
//! tests (`zig test`), the runnable example, and the async-writer latency bench.
//! Gated on `zig`.

#![allow(clippy::expect_used, clippy::panic, clippy::print_stderr)]

use std::path::Path;
use std::process::{Child, Command, Stdio};

use zerodds_endpoint_e2e::{bind_reliable_peer, reliable_receive, zig_ready_or_skip};

fn reliable_zig() -> String {
    format!(
        "{}/../../endpoints/zig/src/reliable.zig",
        env!("CARGO_MANIFEST_DIR")
    )
}

/// Lays out a build dir with `reliable.zig` (copied) + `main.zig` (the app).
fn scaffold(main_src: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rel_zig_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::copy(reliable_zig(), dir.join("reliable.zig")).expect("copy reliable.zig");
    std::fs::write(dir.join("main.zig"), main_src).expect("main.zig");
    dir
}

// The reliable sender app: `e2e <port> <N>` submits N and recovers loss;
// `bench <port>` reports the async-writer wait-free push latency vs inline sendto.
const ZIG_RELIABLE_MAIN: &str = r#"const std = @import("std");
const rel = @import("reliable.zig");

// Zig 0.16 moved sockets out of `std.posix` and into `std.Io.net`:
// `std.posix.socket`, `bind`, `sendto` and `recvfrom` are gone, and the
// replacements take the `Io` handle the entry point is now given. The
// receive timeout comes with them — `receiveTimeout` replaces the
// `SO_RCVTIMEO` this program used to set by hand.
const net = std.Io.net;

const SendCtx = struct {
    io: std.Io,
    sock: net.Socket,
    peer: net.IpAddress,
    fn send(ctx: *anyopaque, frame: []const u8) void {
        const self: *SendCtx = @ptrCast(@alignCast(ctx));
        self.sock.send(self.io, &self.peer, frame) catch {};
    }
    fn recv(ctx: *anyopaque, buf: []u8) ?usize {
        _ = ctx;
        _ = buf;
        return null;
    }
};

fn runBench(alloc: std.mem.Allocator, io: std.Io, sock: net.Socket, peer: net.IpAddress, stdout: anytype) !void {
    const M: usize = 4000;
    var pp = [_]u8{ 1, 2, 3, 4 };
    var frame: [16]u8 = undefined;
    const fl = rel.writeDataFrame(&frame, 0, &pp);

    const t1 = rel.monotonicNs();
    var i: usize = 0;
    while (i < M) : (i += 1) sock.send(io, &peer, frame[0..fl]) catch {};
    const inline_ns = (rel.monotonicNs() - t1) / M;

    var ctx = SendCtx{ .io = io, .sock = sock, .peer = peer };
    var w = rel.AsyncWriter.init(alloc, &ctx, SendCtx.send, &ctx, SendCtx.recv);
    try w.start();
    i = 0;
    while (i < 100) : (i += 1) _ = w.write(&pp); // warmup
    var pushed: usize = 0;
    const t2 = rel.monotonicNs();
    i = 0;
    while (i < M) : (i += 1) {
        if (w.write(&pp)) pushed += 1;
    }
    const dec_ns = (rel.monotonicNs() - t2) / M;
    w.close();
    try stdout.print("BENCH decoupled_ns={d} inline_ns={d} pushed={d}/{d}\n", .{ dec_ns, inline_ns, pushed, M });
}

pub fn main(zd_init: std.process.Init) !void {
    // Zig 0.16: the entry point takes `std.process.Init`, the arguments
    // come off it, and stdout is a buffered writer over its `Io`.
    // `std.process.argsAlloc` and `std.io.getStdOut()` are gone.
    const alloc = zd_init.arena.allocator();
    const args = try zd_init.minimal.args.toSlice(alloc);
    const mode = args[1];
    const port = try std.fmt.parseInt(u16, args[2], 10);
    var stdout_buf: [4096]u8 = undefined;
    var stdout_file = std.Io.File.stdout().writer(zd_init.io, &stdout_buf);
    const stdout = &stdout_file.interface;
    defer stdout.flush() catch {};

    const bind_addr = try net.IpAddress.parseIp4("127.0.0.1", 0);
    const sock = try net.IpAddress.bind(&bind_addr, zd_init.io, .{ .mode = .dgram });
    defer sock.close(zd_init.io);
    const peer = try net.IpAddress.parseIp4("127.0.0.1", port);

    if (std.mem.eql(u8, mode, "bench")) {
        try runBench(alloc, zd_init.io, sock, peer, stdout);
        return;
    }

    // e2e reliable sender. The 100 ms the socket used to carry as
    // SO_RCVTIMEO is now the timeout argument of every receive.
    const recv_timeout: std.Io.Timeout = .{ .duration = .{
        .raw = .fromMilliseconds(100),
        .clock = .awake,
    } };
    const n_count = try std.fmt.parseInt(u32, args[3], 10);

    var snd = rel.Sender.init(alloc);
    defer snd.deinit();
    var frame: [1024]u8 = undefined;

    var i: u32 = 0;
    while (i < n_count) : (i += 1) {
        var p: [4]u8 = undefined;
        std.mem.writeInt(u32, &p, i, .little);
        const seq = try snd.submit(&p);
        const m = rel.writeDataFrame(&frame, seq, &p);
        try sock.send(zd_init.io, &peer, frame[0..m]);
    }

    const started_ns = rel.monotonicNs();
    var rbuf: [2048]u8 = undefined;
    var round: usize = 0;
    while (snd.inFlightCount() > 0 and round < 2000) : (round += 1) {
        while (true) {
            const msg = sock.receiveTimeout(zd_init.io, &rbuf, recv_timeout) catch |e| switch (e) {
                error.Timeout => break,
                else => return e,
            };
            if (rel.parseAckNack(msg.data)) |an| snd.recvAckNack(an);
        }
        if (snd.inFlightCount() == 0) break;
        var seqs: [rel.SENDER_WINDOW]u16 = undefined;
        const k = snd.inFlightSeqs(&seqs);
        var j: usize = 0;
        while (j < k) : (j += 1) {
            if (snd.getInFlight(seqs[j])) |data| {
                const m = rel.writeDataFrame(&frame, seqs[j], data);
                try sock.send(zd_init.io, &peer, frame[0..m]);
            }
        }
        const now_ms: i64 = @intCast((rel.monotonicNs() - started_ns) / std.time.ns_per_ms);
        if (snd.pendingHeartbeat(now_ms)) |hb| {
            const m = rel.heartbeatFrame(&frame, hb);
            try sock.send(zd_init.io, &peer, frame[0..m]);
        }
        rel.sleepNs(2 * std.time.ns_per_ms);
    }
    try stdout.print("SENT count={d} rounds={d} inflight={d}\n", .{ n_count, round, snd.inFlightCount() });
}
"#;

fn spawn_app(mode: &str, port: u16, n: u32) -> Child {
    let dir = scaffold(ZIG_RELIABLE_MAIN);
    Command::new("zig")
        .arg("run")
        .arg(dir.join("main.zig"))
        .arg("--")
        .arg(mode)
        .arg(port.to_string())
        .arg(n.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn zig")
}

fn decode_u32(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn loss_case(drop_every: Option<usize>, label: &str) {
    if !zig_ready_or_skip(label) {
        return;
    }
    const N: u32 = 12;
    let peer = bind_reliable_peer(drop_every).expect("bind reliable peer");
    let child = spawn_app("e2e", peer.port, N);
    let delivered = reliable_receive(&peer, child, label, N as usize);
    assert_eq!(delivered.len(), N as usize, "{label}: sample count");
    for (i, payload) in delivered.iter().enumerate() {
        assert_eq!(
            decode_u32(payload),
            i as u32,
            "{label}: gap/misorder at {i}"
        );
    }
}

#[test]
fn zig_reliable_loss_recovery() {
    // Peer drops every 3rd distinct sample once → the app must retransmit.
    loss_case(Some(3), "zig/loss");
}

#[test]
fn zig_reliable_no_loss() {
    loss_case(None, "zig/baseline");
}

#[test]
fn zig_reliable_unit() {
    // Mirror-of-reference unit tests + byte-golden HEARTBEAT/ACKNACK (`zig test`).
    if !zig_ready_or_skip("zig_reliable_unit") {
        return;
    }
    let out = Command::new("zig")
        .arg("test")
        .arg(reliable_zig())
        .output()
        .expect("zig test");
    assert!(
        out.status.success(),
        "zig test failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn zig_reliable_example() {
    // The runnable in-process loss-recovery demo prints the recovered sequence.
    if !zig_ready_or_skip("zig_reliable_example") {
        return;
    }
    let example = format!(
        "{}/../../endpoints/zig/example_reliable.zig",
        env!("CARGO_MANIFEST_DIR")
    );
    // Run from endpoints/zig so the relative `@import("src/reliable.zig")` resolves.
    let cwd = format!("{}/../../endpoints/zig", env!("CARGO_MANIFEST_DIR"));
    let out = Command::new("zig")
        .arg("run")
        .arg(&example)
        .current_dir(Path::new(&cwd))
        .output()
        .expect("zig run example");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("sequence 0..11 verified in order"),
        "example did not verify:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn zig_reliable_latency_bench() {
    // Async-decoupled writer: wait-free ring push vs inline sendto.
    if !zig_ready_or_skip("zig_reliable_latency_bench") {
        return;
    }
    let child = spawn_app("bench", 9, 0);
    let out = child.wait_with_output().expect("bench wait");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .find(|l| l.starts_with("BENCH "))
        .unwrap_or_else(|| {
            panic!(
                "no BENCH line\nstderr: {}",
                String::from_utf8_lossy(&out.stderr)
            )
        });
    let dec = parse_kv(line, "decoupled_ns=");
    let inl = parse_kv(line, "inline_ns=");
    eprintln!("zig async-writer latency: decoupled={dec} ns, inline(sendto)={inl} ns");
    assert!(
        dec < inl,
        "decoupled push ({dec} ns) must beat inline sendto ({inl} ns)"
    );
}

fn parse_kv(line: &str, key: &str) -> u64 {
    line.split_whitespace()
        .find_map(|tok| tok.strip_prefix(key))
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("missing {key} in `{line}`"))
}
