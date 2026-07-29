// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Runnable reliable-stream demo: an aggregator submits N samples on a lossy
// channel (every 3rd first-delivery dropped); the receiver reorders + ACKNACKs,
// the sender retransmits, and the reader prints the recovered contiguous
// sequence. In-process (no sockets) so it runs anywhere:  `zig run example_reliable.zig`
// (with `src/reliable.zig` importable — run from `endpoints/zig`:
//   `zig run example_reliable.zig --mod reliable::src/reliable.zig` is not needed;
//   the file imports `src/reliable.zig` by relative path).

const std = @import("std");
const rel = @import("src/reliable.zig");

pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const alloc = gpa.allocator();
    const stdout = std.io.getStdOut().writer();

    const N: u32 = 12;
    var snd = rel.Sender.init(alloc);
    defer snd.deinit();
    var rcv = rel.Receiver.init(alloc);
    defer rcv.deinit();
    var out = std.ArrayList([]u8).init(alloc);
    defer {
        for (out.items) |p| alloc.free(p);
        out.deinit();
    }

    // Aggregator submits N typed samples (payload = the sample index, u32 LE).
    var i: u32 = 0;
    while (i < N) : (i += 1) {
        var p: [4]u8 = undefined;
        std.mem.writeInt(u32, &p, i, .little);
        _ = try snd.submit(&p);
    }

    // Lossy first delivery: drop every 3rd sample once.
    var seqs: [rel.SENDER_WINDOW]u16 = undefined;
    const k = snd.inFlightSeqs(&seqs);
    var losses: usize = 0;
    var j: usize = 0;
    while (j < k) : (j += 1) {
        if ((j + 1) % 3 == 0) {
            losses += 1;
            continue; // dropped in flight
        }
        try rcv.recvData(seqs[j], snd.getInFlight(seqs[j]).?);
    }
    try rcv.drainInto(&out);

    // Recovery: ACKNACK → prune + retransmit until the window drains.
    var round: usize = 0;
    while (snd.inFlightCount() > 0 and round < 100) : (round += 1) {
        const an = rcv.pendingAckNack(null);
        snd.recvAckNack(an);
        var rs: [rel.SENDER_WINDOW]u16 = undefined;
        const rk = snd.inFlightSeqs(&rs);
        var m: usize = 0;
        while (m < rk) : (m += 1) {
            if (snd.getInFlight(rs[m])) |data| try rcv.recvData(rs[m], data);
        }
        try rcv.drainInto(&out);
    }

    try stdout.print(
        "reliable: delivered {d}/{d} gap-free (recovered {d} losses in {d} rounds)\n",
        .{ out.items.len, N, losses, round },
    );
    i = 0;
    while (i < out.items.len) : (i += 1) {
        const b = out.items[i];
        const v = @as(u32, b[0]) | @as(u32, b[1]) << 8 | @as(u32, b[2]) << 16 | @as(u32, b[3]) << 24;
        if (v != i) {
            try stdout.print("GAP: slot {d} = {d}\n", .{ i, v });
            return;
        }
    }
    try stdout.print("sequence 0..{d} verified in order\n", .{N - 1});
}
