// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-Zig endpoint SDK (ADR 0013): a from-scratch XCDR wire-core, no C,
// byte-identical to the Rust core and the other SDKs. Zig has no async/await, so
// the async model is a callback reactor (push) alongside the sync poll (pull).

const std = @import("std");

pub const Endian = enum { little, big };

pub const Writer = struct {
    buf: std.ArrayList(u8),
    endian: Endian,

    pub fn init(alloc: std.mem.Allocator, endian: Endian) Writer {
        return .{ .buf = std.ArrayList(u8).init(alloc), .endian = endian };
    }
    pub fn deinit(self: *Writer) void {
        self.buf.deinit();
    }

    fn alignTo(self: *Writer, a: usize) !void {
        const cap: usize = if (a > 4) 4 else a;
        while (self.buf.items.len % cap != 0) try self.buf.append(0);
    }
    fn putLE(self: *Writer, a: usize, le: []const u8) !void {
        try self.alignTo(a);
        if (self.endian == .big) {
            var i: usize = le.len;
            while (i > 0) {
                i -= 1;
                try self.buf.append(le[i]);
            }
        } else {
            try self.buf.appendSlice(le);
        }
    }
    pub fn putU8(self: *Writer, v: u8) !void {
        try self.buf.append(v);
    }
    pub fn putU16(self: *Writer, v: u16) !void {
        try self.putLE(2, &[_]u8{ @truncate(v), @truncate(v >> 8) });
    }
    pub fn putU32(self: *Writer, v: u32) !void {
        try self.putLE(4, &[_]u8{ @truncate(v), @truncate(v >> 8), @truncate(v >> 16), @truncate(v >> 24) });
    }
    pub fn putU64(self: *Writer, v: u64) !void {
        const le = [_]u8{
            @truncate(v),       @truncate(v >> 8),  @truncate(v >> 16), @truncate(v >> 24),
            @truncate(v >> 32), @truncate(v >> 40), @truncate(v >> 48), @truncate(v >> 56),
        };
        try self.putLE(4, &le);
    }
    pub fn putF32(self: *Writer, v: f32) !void {
        const bits: u32 = @bitCast(v);
        try self.putU32(bits);
    }
    pub fn putBytes(self: *Writer, b: []const u8) !void {
        try self.buf.appendSlice(b);
    }
    pub fn putString(self: *Writer, s: []const u8) !void {
        try self.putU32(@intCast(s.len + 1));
        try self.putBytes(s);
        try self.putU8(0);
    }
    pub fn putSeqU8(self: *Writer, b: []const u8) !void {
        try self.putU32(@intCast(b.len));
        try self.putBytes(b);
    }
    pub fn bytes(self: *Writer) []const u8 {
        return self.buf.items;
    }
};

pub const Reader = struct {
    buf: []const u8,
    pos: usize,
    endian: Endian,

    pub fn init(b: []const u8, endian: Endian) Reader {
        return .{ .buf = b, .pos = 0, .endian = endian };
    }
    fn getLE(self: *Reader, a: usize, out: []u8) void {
        const cap: usize = if (a > 4) 4 else a;
        while (self.pos % cap != 0) self.pos += 1;
        var k: usize = 0;
        while (k < out.len) : (k += 1) out[k] = self.buf[self.pos + k];
        self.pos += out.len;
        if (self.endian == .big) std.mem.reverse(u8, out);
    }
    pub fn getU32(self: *Reader) u32 {
        var le: [4]u8 = undefined;
        self.getLE(4, &le);
        return @as(u32, le[0]) | @as(u32, le[1]) << 8 | @as(u32, le[2]) << 16 | @as(u32, le[3]) << 24;
    }
    // --- primitives beyond getU32 — the byte-exact inverse of the Writer ---
    pub fn getU8(self: *Reader) u8 {
        const b = self.buf[self.pos];
        self.pos += 1;
        return b;
    }
    pub fn getU16(self: *Reader) u16 {
        var le: [2]u8 = undefined;
        self.getLE(2, &le);
        return @as(u16, le[0]) | @as(u16, le[1]) << 8;
    }
    pub fn getU64(self: *Reader) u64 {
        var le: [8]u8 = undefined;
        self.getLE(4, &le);
        return @as(u64, le[0]) | @as(u64, le[1]) << 8 | @as(u64, le[2]) << 16 | @as(u64, le[3]) << 24 |
            @as(u64, le[4]) << 32 | @as(u64, le[5]) << 40 | @as(u64, le[6]) << 48 | @as(u64, le[7]) << 56;
    }
    pub fn getF32(self: *Reader) f32 {
        return @bitCast(self.getU32());
    }
    pub fn getString(self: *Reader) []const u8 {
        const n: usize = self.getU32();
        const s = self.buf[self.pos .. self.pos + n - 1];
        self.pos += n;
        return s;
    }
    pub fn getSeqU8(self: *Reader) []const u8 {
        const n: usize = self.getU32();
        const b = self.buf[self.pos .. self.pos + n];
        self.pos += n;
        return b;
    }
};

// --- XRCE framing ---

pub const XRCE_SESSION_NOKEY: u8 = 0x80;
pub const XRCE_STREAM_BEST_EFFORT: u8 = 0x01;

pub fn xrceWriteFrame(out: []u8, session: u8, stream: u8, seq: u16, sample: []const u8) usize {
    const n: u16 = @intCast(sample.len);
    out[0] = session;
    out[1] = stream;
    out[2] = @truncate(seq);
    out[3] = @truncate(seq >> 8);
    out[4] = 0x07; // WRITE_DATA
    out[5] = 0x03; // flags
    out[6] = @truncate(n);
    out[7] = @truncate(n >> 8);
    std.mem.copyForwards(u8, out[8 .. 8 + sample.len], sample);
    return 8 + sample.len;
}

pub fn xrceReadFrame(frame: []const u8) ?[]const u8 {
    if (frame.len < 8 or frame[4] != 0x07) return null;
    return frame[8..];
}

// --- transport (function-pointer vtable, no heap) ---

pub const Transport = struct {
    ctx: *anyopaque,
    deliver: *const fn (ctx: *anyopaque, frame: []const u8) bool,
    // returns the frame length, or null when nothing is ready (again).
    receive: *const fn (ctx: *anyopaque, buf: []u8) ?usize,
};

// --- sync client: pull ---

pub const Client = struct {
    transport: *const Transport,
    session: u8 = XRCE_SESSION_NOKEY,
    stream: u8 = XRCE_STREAM_BEST_EFFORT,
    seq: u16 = 1,
    txbuf: [1024]u8 = undefined,
    rxbuf: [2048]u8 = undefined,

    pub fn write(self: *Client, sample: []const u8) bool {
        const n = xrceWriteFrame(&self.txbuf, self.session, self.stream, self.seq, sample);
        self.seq +%= 1;
        return self.transport.deliver(self.transport.ctx, self.txbuf[0..n]);
    }
    // One synchronous receive: returns the sample body, or null when nothing.
    pub fn poll(self: *Client) ?[]const u8 {
        const n = self.transport.receive(self.transport.ctx, &self.rxbuf) orelse return null;
        return xrceReadFrame(self.rxbuf[0..n]);
    }
};

// --- async reader: push (callback reactor) ---

pub const OnSample = *const fn (ctx: *anyopaque, body: []const u8) void;

pub const AsyncReader = struct {
    transport: *const Transport,
    on_sample: OnSample,
    ctx: *anyopaque,
    rxbuf: [2048]u8 = undefined,

    // Dispatch one ready frame to the callback. Returns true if dispatched.
    pub fn poll(self: *AsyncReader) bool {
        const n = self.transport.receive(self.transport.ctx, &self.rxbuf) orelse return false;
        if (xrceReadFrame(self.rxbuf[0..n])) |body| {
            self.on_sample(self.ctx, body);
            return true;
        }
        return false;
    }
    // Drain the transport (until nothing ready, or `max` frames). Returns count.
    pub fn run(self: *AsyncReader, max: usize) usize {
        var count: usize = 0;
        while (max == 0 or count < max) {
            if (self.poll()) {
                count += 1;
            } else break;
        }
        return count;
    }
};

// ============================ tests ============================

const testing = std.testing;

fn fixture(w: *Writer) !void {
    try w.putU32(0xA1B2C3D4);
    try w.putU16(0x1234);
    try w.putU8(0x5A);
    try w.putF32(3.5);
    try w.putU64(0x0102030405060708);
    try w.putString("bay-12");
    try w.putSeqU8(&[_]u8{ 0xDE, 0xAD, 0xBE, 0xEF });
}

test "byte identity vs Rust goldens" {
    const cases = [_]struct { endian: Endian, file: []const u8 }{
        .{ .endian = .little, .file = "build/golden_le.bin" },
        .{ .endian = .big, .file = "build/golden_be.bin" },
    };
    for (cases) |tc| {
        var w = Writer.init(testing.allocator, tc.endian);
        defer w.deinit();
        try fixture(&w);
        const golden = try std.fs.cwd().readFileAlloc(testing.allocator, tc.file, 4096);
        defer testing.allocator.free(golden);
        try testing.expectEqualSlices(u8, golden, w.bytes());
    }
}

// In-memory FIFO transport for the loopback tests.
const Fifo = struct {
    frames: [16][256]u8 = undefined,
    lens: [16]usize = undefined,
    head: usize = 0,
    tail: usize = 0,
    count: usize = 0,

    fn deliver(ctx: *anyopaque, frame: []const u8) bool {
        const f: *Fifo = @ptrCast(@alignCast(ctx));
        if (f.count == 16) return false;
        std.mem.copyForwards(u8, f.frames[f.tail][0..frame.len], frame);
        f.lens[f.tail] = frame.len;
        f.tail = (f.tail + 1) % 16;
        f.count += 1;
        return true;
    }
    fn receive(ctx: *anyopaque, buf: []u8) ?usize {
        const f: *Fifo = @ptrCast(@alignCast(ctx));
        if (f.count == 0) return null;
        const l = f.lens[f.head];
        std.mem.copyForwards(u8, buf[0..l], f.frames[f.head][0..l]);
        f.head = (f.head + 1) % 16;
        f.count -= 1;
        return l;
    }
};

fn sampleBody(w: *Writer, id: u32) ![]const u8 {
    try w.putU32(id);
    try w.putU16(0);
    try w.putU8(0);
    return w.bytes();
}

test "sync loopback (pull)" {
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var c = Client{ .transport = &t };
    var i: u32 = 0;
    while (i < 5) : (i += 1) {
        var bw = Writer.init(testing.allocator, .little);
        defer bw.deinit();
        const body = try sampleBody(&bw, 0x3000 + i);
        try testing.expect(c.write(body));
    }
    i = 0;
    while (i < 5) : (i += 1) {
        const body = c.poll() orelse return error.NoSample;
        var r = Reader.init(body, .little);
        try testing.expectEqual(@as(u32, 0x3000 + i), r.getU32());
    }
}

const Collector = struct {
    ids: [5]u32 = undefined,
    got: usize = 0,
    fn on(ctx: *anyopaque, body: []const u8) void {
        const c: *Collector = @ptrCast(@alignCast(ctx));
        var r = Reader.init(body, .little);
        if (c.got < 5) {
            c.ids[c.got] = r.getU32();
            c.got += 1;
        }
    }
};

test "async loopback (push / callback reactor)" {
    var fifo = Fifo{};
    const t = Transport{ .ctx = &fifo, .deliver = Fifo.deliver, .receive = Fifo.receive };
    var w = Client{ .transport = &t };
    var i: u32 = 0;
    while (i < 5) : (i += 1) {
        var bw = Writer.init(testing.allocator, .little);
        defer bw.deinit();
        const body = try sampleBody(&bw, 0x1000 + i);
        try testing.expect(w.write(body));
    }
    var col = Collector{};
    var reader = AsyncReader{ .transport = &t, .on_sample = Collector.on, .ctx = &col };
    const n = reader.run(0);
    try testing.expectEqual(@as(usize, 5), n);
    try testing.expectEqual(@as(usize, 5), col.got);
    i = 0;
    while (i < 5) : (i += 1) try testing.expectEqual(@as(u32, 0x1000 + i), col.ids[i]);
}
