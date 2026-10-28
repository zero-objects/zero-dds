// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deeper SYNC example for the native Zig endpoint: a sensor-telemetry publisher
// writes typed Reading samples; a subscriber polls (pull) and decodes every
// field. Run: `zig build run-example_sync`

const std = @import("std");
const zerodds = @import("zerodds");

// Reading mirrors an IDL `@final struct Reading { uint32 id; float value; string label; }`.
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

fn decodeReading(body: []const u8) Reading {
    var r = zerodds.Reader.init(body, .little, .xcdr2);
    return .{ .id = r.getU32(), .value = r.getF32(), .label = r.getString() };
}

// A small in-memory FIFO transport (an integrator supplies a real UDP/SHM one).
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

pub fn main(zd_init: std.process.Init) !void {
    const alloc = zd_init.arena.allocator();
    const total: usize = 5;

    var fifo = Fifo{};
    const t = zerodds.Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var client = zerodds.Client{ .transport = &t };

    // Publisher: frame + deliver 5 typed readings with varying values.
    var i: usize = 0;
    while (i < total) : (i += 1) {
        var label_buf: [16]u8 = undefined;
        const label = try std.fmt.bufPrint(&label_buf, "bay-{d:0>2}", .{i});
        const r = Reading{ .id = @intCast(0x1000 + i), .value = 20.0 + @as(f32, @floatFromInt(i)) * 0.5, .label = label };
        const body = try r.marshal(alloc, .little);
        defer alloc.free(body);
        _ = try client.write(body);
    }

    // Subscriber: poll (pull); decode every field; stop at `total`.
    var stdout_buf: [4096]u8 = undefined;
    var stdout_file = std.Io.File.stdout().writer(zd_init.io, &stdout_buf);
    const stdout = &stdout_file.interface;
    defer stdout.flush() catch {};
    var got: usize = 0;
    while (got < total) {
        if (client.poll()) |body| {
            const r = decodeReading(body);
            try stdout.print("sync reading {d}: id=0x{x} value={d:.1} label=\"{s}\"\n", .{ got, r.id, r.value, r.label });
            got += 1;
        } else break;
    }
    if (got != total) return error.Incomplete;
    try stdout.print("ALL OK\n", .{});
}
