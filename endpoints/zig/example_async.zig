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
        var w = zerodds.Writer.init(alloc, endian, .xcdr2);
        errdefer w.deinit();
        try w.putU32(self.id);
        try w.putF32(self.value);
        try w.putString(self.label);
        // Zig 0.16's `std.ArrayList` is the unmanaged one: `toOwnedSlice`
        // takes the allocator, which the writer keeps in `alloc`.
        return w.buf.toOwnedSlice(w.alloc);
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
    /// Where the callback prints. Zig 0.16 replaced `std.io.getStdOut` with
    /// `std.Io.File.stdout().writer(io, buf)`, which needs the `Io` the
    /// entry point receives — and `on_sample` takes only `ctx` and `body`.
    /// The reactor's context is where the writer travels.
    out: *std.Io.Writer,
    fn onSample(ctx: *anyopaque, body: []const u8) void {
        const self: *Collector = @ptrCast(@alignCast(ctx));
        var r = zerodds.Reader.init(body, .little, .xcdr2);
        const id = r.getU32();
        const value = r.getF32();
        const label = r.getString();
        self.out.print("async reading {d}: id=0x{x} value={d:.1} label=\"{s}\"\n", .{ self.count, id, value, label }) catch {};
        self.count += 1;
    }
};

pub fn main(zd_init: std.process.Init) !void {
    const alloc = zd_init.arena.allocator();
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
        _ = try client.write(body);
    }

    // Subscriber: the callback reactor drains up to `total` frames.
    var stdout_buf: [4096]u8 = undefined;
    var stdout_file = std.Io.File.stdout().writer(zd_init.io, &stdout_buf);
    const stdout = &stdout_file.interface;
    defer stdout.flush() catch {};
    var col = Collector{ .out = stdout };
    var reader = zerodds.AsyncReader{ .transport = &t, .on_sample = Collector.onSample, .ctx = &col };
    _ = reader.run(total);

    if (col.count != total) return error.Incomplete;
    try stdout.print("ALL OK\n", .{});
}
