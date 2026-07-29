// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Runnable example for the native Zig endpoint SDK: sync (pull) and async
// (callback reactor) over an in-memory transport.  zig build run

const std = @import("std");
const zerodds = @import("zerodds");

const Loopback = struct {
    frames: [16][256]u8 = undefined,
    lens: [16]usize = undefined,
    head: usize = 0,
    tail: usize = 0,
    count: usize = 0,

    fn deliver(ctx: *anyopaque, frame: []const u8) bool {
        const l: *Loopback = @ptrCast(@alignCast(ctx));
        if (l.count == 16) return false;
        std.mem.copyForwards(u8, l.frames[l.tail][0..frame.len], frame);
        l.lens[l.tail] = frame.len;
        l.tail = (l.tail + 1) % 16;
        l.count += 1;
        return true;
    }
    fn receive(ctx: *anyopaque, buf: []u8) ?usize {
        const l: *Loopback = @ptrCast(@alignCast(ctx));
        if (l.count == 0) return null;
        const n = l.lens[l.head];
        std.mem.copyForwards(u8, buf[0..n], l.frames[l.head][0..n]);
        l.head = (l.head + 1) % 16;
        l.count -= 1;
        return n;
    }
};

fn onSample(ctx: *anyopaque, body: []const u8) void {
    _ = ctx;
    var r = zerodds.Reader.init(body, .little);
    std.debug.print("async: received id=0x{X}\n", .{r.getU32()});
}

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();

    // sample body = id + label
    var bw = zerodds.Writer.init(alloc, .little);
    defer bw.deinit();
    try bw.putU32(0x42);
    try bw.putString("hello");
    const body = bw.bytes();

    // --- sync (pull) ---
    var lb = Loopback{};
    const t = zerodds.Transport{ .ctx = &lb, .deliver = Loopback.deliver, .receive = Loopback.receive };
    var c = zerodds.Client{ .transport = &t };
    _ = c.write(body);
    if (c.poll()) |b| {
        var r = zerodds.Reader.init(b, .little);
        std.debug.print("sync: received id=0x{X}\n", .{r.getU32()});
    }

    // --- async (push / callback reactor) ---
    var lb2 = Loopback{};
    const t2 = zerodds.Transport{ .ctx = &lb2, .deliver = Loopback.deliver, .receive = Loopback.receive };
    var w = zerodds.Client{ .transport = &t2 };
    _ = w.write(body);
    _ = w.write(body);
    var reader = zerodds.AsyncReader{ .transport = &t2, .on_sample = onSample, .ctx = &lb2 };
    _ = reader.run(0);

    std.debug.print("ALL OK\n", .{});
}
