// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-Zig reliable XRCE stream layer (DDS-XRCE 1.0 §8.4.10/§8.4.11),
// byte-identical to `crates/xrce` + `endpoints/c`. Self-contained (only `std`) so
// the ping-pong scaffold — which copies just `zerodds.zig` — is never coupled to
// this file. `zerodds.zig` may re-export it, but does not `@import` it eagerly.
//
// Two roles:
//   Sender   — submit / pendingHeartbeat / recvAckNack / getInFlight (history +
//              retransmit), plus the async-decoupled `AsyncWriter` (wait-free SPSC
//              ring + a drain thread that holds the reliable state; the producer
//              never enters the kernel).
//   Receiver — recvData / drainInOrder / pendingAckNack (reorder + de-dup).
//
// RFC-1982 16-bit sequence numbers; sender window 16, receiver buffer 64,
// heartbeat 500 ms, payload <= 65535.

const std = @import("std");

pub const SENDER_WINDOW: usize = 16;
pub const RECEIVER_BUFFER: usize = 64;
pub const MAX_PAYLOAD: usize = 65535;
pub const HEARTBEAT_PERIOD_MS: i64 = 500;
pub const RELIABLE_STREAM_ID: u8 = 0x80;

const HALF: u16 = 0x8000;

/// RFC-1982: is `a` strictly before `b`?
pub fn seqLt(a: u16, b: u16) bool {
    const d = b -% a;
    return d != 0 and d < HALF;
}
/// RFC-1982: is `a` strictly after `b`?
pub fn seqGt(a: u16, b: u16) bool {
    return seqLt(b, a);
}

pub const Heartbeat = struct { first_unacked: u16, last_unacked: u16, stream_id: u8 };
pub const AckNack = struct { first_unacked: u16, nack_bitmap: u16, stream_id: u8 };

pub const Error = error{ PayloadTooLarge, WindowFull, BufferFull };

// ------------------------------------------------------------------
// Wire framing — golden layout (byte-identical to golden_*.bin):
//   [0]=0x80 session, [1]=0x00 stream(NONE), [2..4]=msg-seq LE,
//   [4]=submsg id, [5]=flags, [6..8]=body-len LE, [8..]=body.
// WRITE_DATA keeps the sample's RFC-1982 seq in the stream-header seq field
// ([2..4]) with stream byte 0x80, matching the reliable peer's `reliable_unframe`.
// ------------------------------------------------------------------

/// Reliable WRITE_DATA frame; the 2-byte stream-header seq carries the sample seq.
pub fn writeDataFrame(out: []u8, seq: u16, sample: []const u8) usize {
    const n: u16 = @intCast(sample.len);
    out[0] = 0x80;
    out[1] = RELIABLE_STREAM_ID;
    out[2] = @truncate(seq);
    out[3] = @truncate(seq >> 8);
    out[4] = 0x07; // WRITE_DATA
    out[5] = 0x03; // flags
    out[6] = @truncate(n);
    out[7] = @truncate(n >> 8);
    std.mem.copyForwards(u8, out[8 .. 8 + sample.len], sample);
    return 8 + sample.len;
}

/// Parses a reliable WRITE_DATA frame into `(seq, sample)`.
pub fn parseWriteData(frame: []const u8) ?struct { seq: u16, sample: []const u8 } {
    if (frame.len < 8 or frame[4] != 0x07) return null;
    return .{ .seq = @as(u16, frame[2]) | @as(u16, frame[3]) << 8, .sample = frame[8..] };
}

/// HEARTBEAT frame (golden layout, 13 bytes).
pub fn heartbeatFrame(out: []u8, hb: Heartbeat) usize {
    out[0] = 0x80;
    out[1] = 0x00;
    out[2] = 0x01;
    out[3] = 0x00;
    out[4] = 0x0B; // HEARTBEAT
    out[5] = 0x01; // flags (E-flag LE)
    out[6] = 0x05;
    out[7] = 0x00;
    out[8] = @truncate(hb.first_unacked);
    out[9] = @truncate(hb.first_unacked >> 8);
    out[10] = @truncate(hb.last_unacked);
    out[11] = @truncate(hb.last_unacked >> 8);
    out[12] = hb.stream_id;
    return 13;
}

/// Parses a HEARTBEAT frame.
pub fn parseHeartbeat(frame: []const u8) ?Heartbeat {
    if (frame.len < 13 or frame[4] != 0x0B) return null;
    return .{
        .first_unacked = @as(u16, frame[8]) | @as(u16, frame[9]) << 8,
        .last_unacked = @as(u16, frame[10]) | @as(u16, frame[11]) << 8,
        .stream_id = frame[12],
    };
}

/// ACKNACK frame (golden layout, 13 bytes).
pub fn acknackFrame(out: []u8, an: AckNack) usize {
    out[0] = 0x80;
    out[1] = 0x00;
    out[2] = 0x01;
    out[3] = 0x00;
    out[4] = 0x0A; // ACKNACK
    out[5] = 0x01; // flags (E-flag LE)
    out[6] = 0x05;
    out[7] = 0x00;
    out[8] = @truncate(an.first_unacked);
    out[9] = @truncate(an.first_unacked >> 8);
    out[10] = @truncate(an.nack_bitmap);
    out[11] = @truncate(an.nack_bitmap >> 8);
    out[12] = an.stream_id;
    return 13;
}

/// Parses an ACKNACK frame.
pub fn parseAckNack(frame: []const u8) ?AckNack {
    if (frame.len < 13 or frame[4] != 0x0A) return null;
    return .{
        .first_unacked = @as(u16, frame[8]) | @as(u16, frame[9]) << 8,
        .nack_bitmap = @as(u16, frame[10]) | @as(u16, frame[11]) << 8,
        .stream_id = frame[12],
    };
}

// ------------------------------------------------------------------
// Sender
// ------------------------------------------------------------------

const Slot = struct { used: bool = false, seq: u16 = 0, data: []u8 = &[_]u8{} };

pub const Sender = struct {
    alloc: std.mem.Allocator,
    next_seq: u16 = 0,
    slots: [SENDER_WINDOW]Slot = [_]Slot{.{}} ** SENDER_WINDOW,
    count: usize = 0,
    last_hb_ms: i64 = -1,

    pub fn init(alloc: std.mem.Allocator) Sender {
        return .{ .alloc = alloc };
    }
    pub fn deinit(self: *Sender) void {
        for (&self.slots) |*s| {
            if (s.used) self.alloc.free(s.data);
            s.* = .{};
        }
        self.count = 0;
    }

    /// Buffers a new sample, returns its assigned RFC-1982 seq.
    pub fn submit(self: *Sender, payload: []const u8) Error!u16 {
        if (payload.len > MAX_PAYLOAD) return error.PayloadTooLarge;
        if (self.count >= SENDER_WINDOW) return error.WindowFull;
        for (&self.slots) |*s| {
            if (!s.used) {
                const buf = self.alloc.alloc(u8, payload.len) catch return error.WindowFull;
                std.mem.copyForwards(u8, buf, payload);
                s.* = .{ .used = true, .seq = self.next_seq, .data = buf };
                self.count += 1;
                const seq = self.next_seq;
                self.next_seq +%= 1;
                return seq;
            }
        }
        return error.WindowFull;
    }

    pub fn inFlightCount(self: *const Sender) usize {
        return self.count;
    }

    pub fn getInFlight(self: *const Sender, seq: u16) ?[]const u8 {
        for (&self.slots) |*s| {
            if (s.used and s.seq == seq) return s.data;
        }
        return null;
    }

    /// Writes the in-flight seqs into `out`, returns how many.
    pub fn inFlightSeqs(self: *const Sender, out: []u16) usize {
        var k: usize = 0;
        for (&self.slots) |*s| {
            if (s.used) {
                out[k] = s.seq;
                k += 1;
            }
        }
        return k;
    }

    fn removeSeq(self: *Sender, seq: u16) void {
        for (&self.slots) |*s| {
            if (s.used and s.seq == seq) {
                self.alloc.free(s.data);
                s.* = .{};
                self.count -= 1;
                return;
            }
        }
    }

    /// Emits a HEARTBEAT if the window is non-empty and the period elapsed.
    /// `now_ms` is a monotonic millisecond clock supplied by the caller.
    pub fn pendingHeartbeat(self: *Sender, now_ms: i64) ?Heartbeat {
        if (self.count == 0) return null;
        const due = self.last_hb_ms < 0 or (now_ms - self.last_hb_ms) >= HEARTBEAT_PERIOD_MS;
        if (!due) return null;
        self.last_hb_ms = now_ms;
        // RFC-1982 window base (oldest unacked) + end (newest unacked); NOT the
        // numeric min/max, which is wrong across a 16-bit wrap: window
        // 0xFFFE,0xFFFF,0x0000,0x0001 → base 0xFFFE / end 0x0001, not 0x0000 /
        // 0xFFFF. Mirrors window_base / serial_max_in_flight in crates/xrce.
        var lo: u16 = 0;
        var hi: u16 = 0;
        var seen = false;
        for (&self.slots) |*s| {
            if (!s.used) continue;
            if (!seen) {
                lo = s.seq;
                hi = s.seq;
                seen = true;
            } else {
                if (seqLt(s.seq, lo)) lo = s.seq;
                if (seqGt(s.seq, hi)) hi = s.seq;
            }
        }
        return .{ .first_unacked = lo, .last_unacked = hi, .stream_id = RELIABLE_STREAM_ID };
    }

    /// Processes an ACKNACK: everything < base is acked; within `[base,base+16)`
    /// a clear bit is acked (dropped), a set bit stays in-flight (retransmit).
    pub fn recvAckNack(self: *Sender, an: AckNack) void {
        const base = an.first_unacked;
        // 1) ack everything strictly before base (RFC-1982).
        var i: usize = 0;
        while (i < SENDER_WINDOW) : (i += 1) {
            const s = &self.slots[i];
            if (s.used and seqLt(s.seq, base)) {
                self.alloc.free(s.data);
                s.* = .{};
                self.count -= 1;
            }
        }
        // 2) within the window: clear bit → acked.
        var b: u16 = 0;
        while (b < 16) : (b += 1) {
            const seq = base +% b;
            const bit = (an.nack_bitmap >> @intCast(b)) & 1;
            if (bit == 0) self.removeSeq(seq);
        }
    }
};

// ------------------------------------------------------------------
// Receiver
// ------------------------------------------------------------------

pub const Receiver = struct {
    alloc: std.mem.Allocator,
    expected_seq: u16 = 0,
    slots: [RECEIVER_BUFFER]Slot = [_]Slot{.{}} ** RECEIVER_BUFFER,
    count: usize = 0,

    pub fn init(alloc: std.mem.Allocator) Receiver {
        return .{ .alloc = alloc };
    }
    pub fn deinit(self: *Receiver) void {
        for (&self.slots) |*s| {
            if (s.used) self.alloc.free(s.data);
            s.* = .{};
        }
        self.count = 0;
    }

    pub fn outOfOrderCount(self: *const Receiver) usize {
        return self.count;
    }
    pub fn expected(self: *const Receiver) u16 {
        return self.expected_seq;
    }

    fn contains(self: *const Receiver, seq: u16) bool {
        for (&self.slots) |*s| {
            if (s.used and s.seq == seq) return true;
        }
        return false;
    }

    /// Buffers an incoming sample (dedup + DoS bound).
    pub fn recvData(self: *Receiver, seq: u16, payload: []const u8) Error!void {
        if (seqLt(seq, self.expected_seq)) return; // duplicate before window → drop
        if (self.contains(seq)) return; // already buffered
        if (self.count >= RECEIVER_BUFFER) return error.BufferFull;
        for (&self.slots) |*s| {
            if (!s.used) {
                const buf = self.alloc.alloc(u8, payload.len) catch return error.BufferFull;
                std.mem.copyForwards(u8, buf, payload);
                s.* = .{ .used = true, .seq = seq, .data = buf };
                self.count += 1;
                return;
            }
        }
        return error.BufferFull;
    }

    /// Delivers all contiguous samples from `expected_seq`, appending owned
    /// payload copies into `out`, and advances `expected_seq`.
    pub fn drainInto(self: *Receiver, out: *std.ArrayList([]u8)) !void {
        while (true) {
            var found = false;
            for (&self.slots) |*s| {
                if (s.used and s.seq == self.expected_seq) {
                    try out.append(s.data);
                    s.* = .{}; // ownership moves to `out`
                    self.count -= 1;
                    self.expected_seq +%= 1;
                    found = true;
                    break;
                }
            }
            if (!found) break;
        }
    }

    /// Computes the ACKNACK marking missing slots in `[expected, expected+16)`.
    pub fn pendingAckNack(self: *const Receiver, hint_last_seen: ?u16) AckNack {
        const base = self.expected_seq;
        var bitmap: u16 = 0;
        var i: u16 = 0;
        while (i < 16) : (i += 1) {
            const seq = base +% i;
            if (hint_last_seen) |h| {
                if (seqGt(seq, h)) continue;
            }
            if (!self.contains(seq)) bitmap |= (@as(u16, 1) << @intCast(i));
        }
        return .{ .first_unacked = base, .nack_bitmap = bitmap, .stream_id = RELIABLE_STREAM_ID };
    }

    pub fn reset(self: *Receiver) void {
        self.deinit();
        self.expected_seq = 0;
    }
};

// ------------------------------------------------------------------
// Async-decoupled writer: wait-free SPSC ring + a drain thread that holds the
// reliable Sender state. `write` never enters the kernel — it memcpys into a ring
// slot and publishes the tail; the drain thread does the syscalls + retransmit.
// ------------------------------------------------------------------

pub const SendFn = *const fn (ctx: *anyopaque, frame: []const u8) void;
pub const RecvFn = *const fn (ctx: *anyopaque, buf: []u8) ?usize;

pub const AsyncWriter = struct {
    pub const RING_CAP: usize = 1024;
    pub const SLOT_CAP: usize = 512;

    ring: [RING_CAP][SLOT_CAP]u8 = undefined,
    lens: [RING_CAP]u16 = [_]u16{0} ** RING_CAP,
    head: std.atomic.Value(usize) = std.atomic.Value(usize).init(0), // consumer
    tail: std.atomic.Value(usize) = std.atomic.Value(usize).init(0), // producer

    running: std.atomic.Value(bool) = std.atomic.Value(bool).init(true),
    sender: Sender,
    send_ctx: *anyopaque,
    send_fn: SendFn,
    recv_ctx: *anyopaque,
    recv_fn: RecvFn,
    thread: ?std.Thread = null,

    pub fn init(
        alloc: std.mem.Allocator,
        send_ctx: *anyopaque,
        send_fn: SendFn,
        recv_ctx: *anyopaque,
        recv_fn: RecvFn,
    ) AsyncWriter {
        return .{
            .sender = Sender.init(alloc),
            .send_ctx = send_ctx,
            .send_fn = send_fn,
            .recv_ctx = recv_ctx,
            .recv_fn = recv_fn,
        };
    }

    pub fn start(self: *AsyncWriter) !void {
        self.thread = try std.Thread.spawn(.{}, drainLoop, .{self});
    }

    /// Wait-free enqueue: bounded memcpy + one release-store. Returns false when
    /// the ring is full (backpressure — the producer decides locally).
    pub fn write(self: *AsyncWriter, sample: []const u8) bool {
        if (sample.len > SLOT_CAP) return false;
        const tail = self.tail.load(.monotonic);
        const next = (tail + 1) % RING_CAP;
        if (next == self.head.load(.acquire)) return false; // full
        std.mem.copyForwards(u8, self.ring[tail][0..sample.len], sample);
        self.lens[tail] = @intCast(sample.len);
        self.tail.store(next, .release);
        return true;
    }

    /// Copies the ring head into `out` WITHOUT advancing — the slot stays owned
    /// by the ring until `advance()`. This lets a window-full submit leave the
    /// sample in place instead of dropping it.
    fn peek(self: *AsyncWriter, out: []u8) ?usize {
        const head = self.head.load(.monotonic);
        if (head == self.tail.load(.acquire)) return null; // empty
        const l = self.lens[head];
        std.mem.copyForwards(u8, out[0..l], self.ring[head][0..l]);
        return l;
    }

    /// Releases the ring head — called only after the head was accepted into the
    /// send window (or is non-retryable).
    fn advance(self: *AsyncWriter) void {
        const head = self.head.load(.monotonic);
        self.head.store((head + 1) % RING_CAP, .release);
    }

    fn drainLoop(self: *AsyncWriter) void {
        var frame: [SLOT_CAP + 16]u8 = undefined;
        var sbuf: [SLOT_CAP]u8 = undefined;
        var rbuf: [2048]u8 = undefined;
        var timer = std.time.Timer.start() catch return;
        // Stop when signalled — the producer owns lifecycle; a lingering window
        // must not wedge `close()` (e.g. a bench with no ACKNACKs to drain it).
        while (self.running.load(.acquire)) {
            var idle = true;
            // 1) pull new samples off the ring → submit + send WRITE_DATA.
            if (self.peek(&sbuf)) |l| {
                const seq = self.sender.submit(sbuf[0..l]) catch |err| {
                    switch (err) {
                        // Window full: leave the sample in the ring (do NOT
                        // advance head — that was the data-loss bug: the slot
                        // was consumed yet never sent). Free a slot via ACKNACKs
                        // and retry next tick; back off if none arrived.
                        error.WindowFull => if (!self.drainAckNacks(&rbuf))
                            std.time.sleep(200 * std.time.ns_per_us),
                        // Non-retryable (too large): drop so it cannot wedge the
                        // ring head forever.
                        else => {
                            self.advance();
                            idle = false;
                        },
                    }
                    continue;
                };
                // Accepted into the window: only now release the ring slot.
                self.advance();
                idle = false;
                const n = writeDataFrame(&frame, seq, sbuf[0..l]);
                self.send_fn(self.send_ctx, frame[0..n]);
            }
            // 2) process ACKNACKs → prune + retransmit.
            if (self.drainAckNacks(&rbuf)) idle = false;
            // 3) periodic heartbeat.
            const now_ms: i64 = @intCast(timer.read() / std.time.ns_per_ms);
            if (self.sender.pendingHeartbeat(now_ms)) |hb| {
                const n = heartbeatFrame(&frame, hb);
                self.send_fn(self.send_ctx, frame[0..n]);
            }
            if (idle) std.time.sleep(200 * std.time.ns_per_us);
        }
    }

    fn drainAckNacks(self: *AsyncWriter, rbuf: []u8) bool {
        var any = false;
        while (self.recv_fn(self.recv_ctx, rbuf)) |n| {
            if (parseAckNack(rbuf[0..n])) |an| {
                self.sender.recvAckNack(an);
                // retransmit everything still in-flight.
                var seqs: [SENDER_WINDOW]u16 = undefined;
                const k = self.sender.inFlightSeqs(&seqs);
                var frame: [SLOT_CAP + 16]u8 = undefined;
                var i: usize = 0;
                while (i < k) : (i += 1) {
                    if (self.sender.getInFlight(seqs[i])) |data| {
                        const m = writeDataFrame(&frame, seqs[i], data);
                        self.send_fn(self.send_ctx, frame[0..m]);
                    }
                }
                any = true;
            } else break;
        }
        return any;
    }

    pub fn close(self: *AsyncWriter) void {
        self.running.store(false, .release);
        if (self.thread) |t| {
            t.join();
            self.thread = null;
        }
        self.sender.deinit();
    }
};

// ============================ tests ============================

const testing = std.testing;

test "submit assigns monotonic seqnrs" {
    var s = Sender.init(testing.allocator);
    defer s.deinit();
    try testing.expectEqual(@as(u16, 0), try s.submit(&[_]u8{ 1, 2 }));
    try testing.expectEqual(@as(u16, 1), try s.submit(&[_]u8{ 3, 4 }));
    try testing.expectEqual(@as(usize, 2), s.inFlightCount());
}

test "submit rejects payload too large" {
    var s = Sender.init(testing.allocator);
    defer s.deinit();
    const huge = try testing.allocator.alloc(u8, MAX_PAYLOAD + 1);
    defer testing.allocator.free(huge);
    try testing.expectError(error.PayloadTooLarge, s.submit(huge));
}

test "submit rejects when window full" {
    var s = Sender.init(testing.allocator);
    defer s.deinit();
    var i: usize = 0;
    while (i < SENDER_WINDOW) : (i += 1) _ = try s.submit(&[_]u8{0});
    try testing.expectError(error.WindowFull, s.submit(&[_]u8{0}));
}

test "pending heartbeat fires first time" {
    var s = Sender.init(testing.allocator);
    defer s.deinit();
    _ = try s.submit(&[_]u8{1});
    const hb = s.pendingHeartbeat(0) orelse return error.NoHeartbeat;
    try testing.expectEqual(@as(u16, 0), hb.first_unacked);
    try testing.expectEqual(@as(u16, 0), hb.last_unacked);
    try testing.expectEqual(RELIABLE_STREAM_ID, hb.stream_id);
}

test "pending heartbeat silenced until period elapsed" {
    var s = Sender.init(testing.allocator);
    defer s.deinit();
    _ = try s.submit(&[_]u8{1});
    try testing.expect(s.pendingHeartbeat(0) != null);
    try testing.expect(s.pendingHeartbeat(100) == null);
    try testing.expect(s.pendingHeartbeat(600) != null);
}

test "pending heartbeat none when window empty" {
    var s = Sender.init(testing.allocator);
    defer s.deinit();
    try testing.expect(s.pendingHeartbeat(0) == null);
}

test "recv acknack clears acked seqnrs" {
    var s = Sender.init(testing.allocator);
    defer s.deinit();
    _ = try s.submit(&[_]u8{0xA0});
    _ = try s.submit(&[_]u8{0xA1});
    _ = try s.submit(&[_]u8{0xA2});
    try testing.expectEqual(@as(usize, 3), s.inFlightCount());
    // base=2, bitmap bit0 set → seq 2 missing, 0+1 acked.
    s.recvAckNack(.{ .first_unacked = 2, .nack_bitmap = 0x0001, .stream_id = 0x80 });
    try testing.expectEqual(@as(usize, 1), s.inFlightCount());
    try testing.expect(s.getInFlight(2) != null);
}

test "recv acknack full clear when no bits set" {
    var s = Sender.init(testing.allocator);
    defer s.deinit();
    var i: usize = 0;
    while (i < 5) : (i += 1) _ = try s.submit(&[_]u8{0});
    s.recvAckNack(.{ .first_unacked = 5, .nack_bitmap = 0, .stream_id = 0x80 });
    try testing.expectEqual(@as(usize, 0), s.inFlightCount());
}

test "recv data buffers in order" {
    var r = Receiver.init(testing.allocator);
    defer r.deinit();
    try r.recvData(0, &[_]u8{10});
    try r.recvData(1, &[_]u8{11});
    var out = std.ArrayList([]u8).init(testing.allocator);
    defer {
        for (out.items) |p| testing.allocator.free(p);
        out.deinit();
    }
    try r.drainInto(&out);
    try testing.expectEqual(@as(usize, 2), out.items.len);
    try testing.expectEqual(@as(u16, 2), r.expected());
}

test "recv data reorders out of order" {
    var r = Receiver.init(testing.allocator);
    defer r.deinit();
    try r.recvData(2, &[_]u8{22});
    try r.recvData(0, &[_]u8{20});
    var out = std.ArrayList([]u8).init(testing.allocator);
    defer {
        for (out.items) |p| testing.allocator.free(p);
        out.deinit();
    }
    try r.drainInto(&out);
    try testing.expectEqual(@as(usize, 1), out.items.len); // only seq 0
    try r.recvData(1, &[_]u8{21});
    try r.drainInto(&out);
    try testing.expectEqual(@as(usize, 3), out.items.len); // 0,1,2
}

test "recv data drops duplicates" {
    var r = Receiver.init(testing.allocator);
    defer r.deinit();
    try r.recvData(0, &[_]u8{1});
    var out = std.ArrayList([]u8).init(testing.allocator);
    defer {
        for (out.items) |p| testing.allocator.free(p);
        out.deinit();
    }
    try r.drainInto(&out);
    try r.recvData(0, &[_]u8{99}); // already delivered → dropped
    try testing.expectEqual(@as(usize, 0), r.outOfOrderCount());
}

test "recv data rejects when buffer full" {
    var r = Receiver.init(testing.allocator);
    defer r.deinit();
    var i: u16 = 1;
    while (i <= RECEIVER_BUFFER) : (i += 1) try r.recvData(i, &[_]u8{1});
    try testing.expectError(error.BufferFull, r.recvData(@intCast(RECEIVER_BUFFER + 1), &[_]u8{1}));
}

test "pending acknack marks missing slots" {
    var r = Receiver.init(testing.allocator);
    defer r.deinit();
    try r.recvData(1, &[_]u8{1});
    try r.recvData(3, &[_]u8{3});
    const an = r.pendingAckNack(3);
    try testing.expect(an.nack_bitmap & (1 << 0) != 0); // seq 0 missing
    try testing.expect(an.nack_bitmap & (1 << 2) != 0); // seq 2 missing
    try testing.expect(an.nack_bitmap & (1 << 1) == 0); // seq 1 present
    try testing.expect(an.nack_bitmap & (1 << 3) == 0); // seq 3 present
}

test "reset clears state" {
    var r = Receiver.init(testing.allocator);
    defer r.deinit();
    try r.recvData(0, &[_]u8{3});
    r.reset();
    try testing.expectEqual(@as(usize, 0), r.outOfOrderCount());
    try testing.expectEqual(@as(u16, 0), r.expected());
}

test "byte-golden heartbeat + acknack" {
    // Byte-identical to golden_heartbeat_le.bin / golden_acknack_le.bin.
    var buf: [13]u8 = undefined;
    _ = heartbeatFrame(&buf, .{ .first_unacked = 1, .last_unacked = 3, .stream_id = 0x80 });
    try testing.expectEqualSlices(u8, &[_]u8{ 0x80, 0x00, 0x01, 0x00, 0x0b, 0x01, 0x05, 0x00, 0x01, 0x00, 0x03, 0x00, 0x80 }, &buf);
    _ = acknackFrame(&buf, .{ .first_unacked = 1, .nack_bitmap = 0, .stream_id = 0x80 });
    try testing.expectEqualSlices(u8, &[_]u8{ 0x80, 0x00, 0x01, 0x00, 0x0a, 0x01, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80 }, &buf);
}

test "end-to-end loss recovery in-process" {
    var snd = Sender.init(testing.allocator);
    defer snd.deinit();
    var rcv = Receiver.init(testing.allocator);
    defer rcv.deinit();
    var out = std.ArrayList([]u8).init(testing.allocator);
    defer {
        for (out.items) |p| testing.allocator.free(p);
        out.deinit();
    }
    // submit 5, receiver sees 0,2,4 (1 + 3 lost).
    var i: u8 = 0;
    while (i < 5) : (i += 1) _ = try snd.submit(&[_]u8{i});
    try rcv.recvData(0, &[_]u8{0});
    try rcv.recvData(2, &[_]u8{2});
    try rcv.recvData(4, &[_]u8{4});
    try rcv.drainInto(&out);
    try testing.expectEqual(@as(usize, 1), out.items.len); // only 0
    // receiver ACKNACKs, sender prunes + retransmits the missing.
    const an = rcv.pendingAckNack(4);
    snd.recvAckNack(an);
    try testing.expect(snd.getInFlight(1) != null);
    try testing.expect(snd.getInFlight(3) != null);
    try rcv.recvData(1, snd.getInFlight(1).?);
    try rcv.recvData(3, snd.getInFlight(3).?);
    try rcv.drainInto(&out);
    try testing.expectEqual(@as(usize, 5), out.items.len); // all recovered
}

// RFC-1982 regression: HEARTBEAT window across the 16-bit wrap. Window
// 0xFFFE,0xFFFF,0x0000,0x0001 → base 0xFFFE / end 0x0001; the old numeric
// min/max reported 0x0000 / 0xFFFF. Mirrors crates/xrce's wrap regression.
test "heartbeat window across wrap reports rfc1982 order" {
    var s = Sender.init(testing.allocator);
    defer s.deinit();
    s.next_seq = 0xFFFE;
    try testing.expectEqual(@as(u16, 0xFFFE), try s.submit(&[_]u8{1}));
    try testing.expectEqual(@as(u16, 0xFFFF), try s.submit(&[_]u8{2}));
    try testing.expectEqual(@as(u16, 0x0000), try s.submit(&[_]u8{3}));
    try testing.expectEqual(@as(u16, 0x0001), try s.submit(&[_]u8{4}));
    const hb = s.pendingHeartbeat(0) orelse return error.NoHeartbeat;
    try testing.expectEqual(@as(u16, 0xFFFE), hb.first_unacked);
    try testing.expectEqual(@as(u16, 0x0001), hb.last_unacked);
}

// RFC-1982 regression: loss recovery of a sample lost across the wrap. Window
// 0xFFFE,0xFFFF,0x0000,0x0001 with 0xFFFF lost → ACKNACK NACKs exactly 0xFFFF,
// sender keeps it retransmittable, all deliver in RFC-1982 order.
test "acknack retransmit of lost sample across wrap" {
    var snd = Sender.init(testing.allocator);
    defer snd.deinit();
    var rcv = Receiver.init(testing.allocator);
    defer rcv.deinit();
    var out = std.ArrayList([]u8).init(testing.allocator);
    defer {
        for (out.items) |p| testing.allocator.free(p);
        out.deinit();
    }
    snd.next_seq = 0xFFFE;
    rcv.expected_seq = 0xFFFE;

    const q0 = try snd.submit(&[_]u8{10}); // 0xFFFE
    const q1 = try snd.submit(&[_]u8{11}); // 0xFFFF (lost)
    const q2 = try snd.submit(&[_]u8{12}); // 0x0000
    const q3 = try snd.submit(&[_]u8{13}); // 0x0001

    try rcv.recvData(q0, &[_]u8{10});
    try rcv.recvData(q2, &[_]u8{12});
    try rcv.recvData(q3, &[_]u8{13});
    try rcv.drainInto(&out);
    try testing.expectEqual(@as(usize, 1), out.items.len); // only 0xFFFE
    try testing.expectEqual(@as(u16, 0xFFFF), rcv.expected());

    const an = rcv.pendingAckNack(q3);
    try testing.expectEqual(@as(u16, 0xFFFF), an.first_unacked);
    try testing.expect(an.nack_bitmap & 0b1 != 0); // 0xFFFF NACKed
    try testing.expect(an.nack_bitmap & 0b110 == 0); // 0x0000/0x0001 present

    snd.recvAckNack(an);
    try testing.expect(snd.getInFlight(q1) != null); // 0xFFFF retransmittable
    try testing.expect(snd.getInFlight(q0) == null);
    try testing.expectEqual(@as(usize, 1), snd.inFlightCount());

    try rcv.recvData(q1, snd.getInFlight(q1).?);
    try rcv.drainInto(&out);
    try testing.expectEqual(@as(usize, 4), out.items.len); // 0xFFFE + 0xFFFF,0x0000,0x0001
}

// In-process loopback peer for the AsyncWriter overflow test. Both callbacks run
// on the writer's drain thread; the main test thread reads counters + flips the
// gate, so all shared state is mutex-guarded.
const OverflowPeer = struct {
    mutex: std.Thread.Mutex = .{},
    rcv: Receiver,
    delivered: std.ArrayList(u8),
    acks: std.ArrayList(AckNack),
    tmp: std.ArrayList([]u8),
    alloc: std.mem.Allocator,
    write_data_seen: usize = 0,
    gate: bool = false,

    fn onSend(ctx: *anyopaque, frame: []const u8) void {
        const self: *OverflowPeer = @ptrCast(@alignCast(ctx));
        const wd = parseWriteData(frame) orelse return; // ignore HEARTBEAT etc.
        self.mutex.lock();
        defer self.mutex.unlock();
        if (seqLt(wd.seq, self.rcv.expected())) return; // retransmit of delivered → ignore
        self.rcv.recvData(wd.seq, wd.sample) catch return;
        self.rcv.drainInto(&self.tmp) catch return;
        while (self.tmp.items.len > 0) {
            const p = self.tmp.orderedRemove(0);
            self.delivered.append(p[0]) catch {};
            self.alloc.free(p);
        }
        self.acks.append(self.rcv.pendingAckNack(wd.seq)) catch {};
        self.write_data_seen += 1;
    }

    fn onRecv(ctx: *anyopaque, buf: []u8) ?usize {
        const self: *OverflowPeer = @ptrCast(@alignCast(ctx));
        self.mutex.lock();
        defer self.mutex.unlock();
        if (!self.gate or self.acks.items.len == 0) return null;
        const an = self.acks.orderedRemove(0);
        return acknackFrame(buf, an);
    }
};

test "async writer window overflow delivers every sample (no drop, no hang)" {
    const alloc = testing.allocator;
    const n: u8 = 20; // > SENDER_WINDOW (16)

    var peer = OverflowPeer{
        .rcv = Receiver.init(alloc),
        .delivered = std.ArrayList(u8).init(alloc),
        .acks = std.ArrayList(AckNack).init(alloc),
        .tmp = std.ArrayList([]u8).init(alloc),
        .alloc = alloc,
    };
    defer {
        peer.rcv.deinit();
        peer.delivered.deinit();
        peer.acks.deinit();
        for (peer.tmp.items) |p| alloc.free(p);
        peer.tmp.deinit();
    }

    const w = try alloc.create(AsyncWriter);
    defer alloc.destroy(w);
    w.* = AsyncWriter.init(alloc, &peer, OverflowPeer.onSend, &peer, OverflowPeer.onRecv);
    try w.start();

    var i: u8 = 0;
    while (i < n) : (i += 1) try testing.expect(w.write(&[_]u8{i}));

    // Wait until the 16-sample window is full (gate still closed), then give the
    // drain thread a moment to attempt the extra 4 — the pre-fix code loses them.
    var waited: usize = 0;
    while (waited < 5000) : (waited += 1) {
        peer.mutex.lock();
        const seen = peer.write_data_seen;
        peer.mutex.unlock();
        if (seen >= SENDER_WINDOW) break;
        std.time.sleep(std.time.ns_per_ms);
    }
    std.time.sleep(50 * std.time.ns_per_ms);

    peer.mutex.lock();
    peer.gate = true;
    peer.mutex.unlock();

    // Bounded wait for all n; a hang (regression) then fails via the count check.
    waited = 0;
    while (waited < 10000) : (waited += 1) {
        peer.mutex.lock();
        const d = peer.delivered.items.len;
        peer.mutex.unlock();
        if (d >= n) break;
        std.time.sleep(std.time.ns_per_ms);
    }

    w.close();

    try testing.expectEqual(@as(usize, n), peer.delivered.items.len);
    i = 0; // in-order delivery ⇒ delivered[i] == i for every sample
    while (i < n) : (i += 1) try testing.expectEqual(i, peer.delivered.items[i]);
}
