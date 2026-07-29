// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deeper ASYNC example for the native Zig endpoint: the same telemetry publisher,
// but the subscriber consumes via the callback-reactor AsyncReader and decodes
// every field. Run: `zig build run-example_async`

const std = @import("std");
const zerodds = @import("zerodds");

const Reading = struct {
    id: u32,
    value: f32,
    label: []const u8,
    fn marshal(self: Reading, alloc: std.mem.Allocator, endian: zerodds.Endian) ![]u8 {
        var w = zerodds.Writer.init(alloc, endian);
        errdefer w.deinit();
        try w.putU32(self.id);
        try w.putF32(self.value);
        try w.putString(self.label);
        return w.buf.toOwnedSlice();
    }
};

const Fifo = struct {
    frames: [16][256]u8 = undefined,
    lens: [16]usize = undefined,
    n: usize = 0,
    head: usize = 0,
    fn deliver(ctx: *anyopaque, frame: []const u8) bool {
        const self: *Fifo = @ptrCast(@alignCast(ctx));
        if (self.n >= 16) return false;
        std.mem.copyForwards(u8, self.frames[self.n][0..frame.len], frame);
        self.lens[self.n] = frame.len;
        self.n += 1;
        return true;
    }
    fn receive(ctx: *anyopaque, buf: []u8) ?usize {
        const self: *Fifo = @ptrCast(@alignCast(ctx));
        if (self.head >= self.n) return null;
        const len = self.lens[self.head];
        std.mem.copyForwards(u8, buf[0..len], self.frames[self.head][0..len]);
        self.head += 1;
        return len;
    }
};

// The async consumer: decode every field in the callback and count.
const Collector = struct {
    count: usize = 0,
    fn onSample(ctx: *anyopaque, body: []const u8) void {
        const self: *Collector = @ptrCast(@alignCast(ctx));
        var r = zerodds.Reader.init(body, .little);
        const id = r.getU32();
        const value = r.getF32();
        const label = r.getString();
        const stdout = std.io.getStdOut().writer();
        stdout.print("async reading {d}: id=0x{x} value={d:.1} label=\"{s}\"\n", .{ self.count, id, value, label }) catch {};
        self.count += 1;
    }
};

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    const alloc = gpa.allocator();
    const total: usize = 5;

    var fifo = Fifo{};
    const t = zerodds.Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var client = zerodds.Client{ .transport = &t };

    // Publisher.
    var i: usize = 0;
    while (i < total) : (i += 1) {
        var label_buf: [16]u8 = undefined;
        const label = try std.fmt.bufPrint(&label_buf, "sensor-{d:0>2}", .{i});
        const r = Reading{ .id = @intCast(0x2000 + i), .value = 100.0 - @as(f32, @floatFromInt(i)), .label = label };
        const body = try r.marshal(alloc, .little);
        defer alloc.free(body);
        _ = client.write(body);
    }

    // Subscriber: the callback reactor drains up to `total` frames.
    var col = Collector{};
    var reader = zerodds.AsyncReader{ .transport = &t, .on_sample = Collector.onSample, .ctx = &col };
    _ = reader.run(total);

    const stdout = std.io.getStdOut().writer();
    if (col.count != total) return error.Incomplete;
    try stdout.print("ALL OK\n", .{});
}
