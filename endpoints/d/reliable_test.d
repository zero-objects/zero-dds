// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Unit suite mirroring crates/xrce reliable.rs + byte-golden assertion.
// Usage: reliable_test [golden_heartbeat_le.bin golden_acknack_le.bin]
// Prints "ALL OK" and exits 0 on success.
module reliable_test;

import std.stdio : writeln, File;
import std.file : read;
import core.time : MonoTime, dur;
import reliable;

private void check(bool cond, string msg)
{
    if (!cond)
    {
        writeln("FAIL: ", msg);
        assert(false, msg);
    }
}

void main(string[] args)
{
    // --- Sender ---
    {
        Sender s;
        ushort a, b;
        check(s.submit([1, 2], a) == SubmitStatus.ok && a == 0, "monotonic seq 0");
        check(s.submit([3, 4], b) == SubmitStatus.ok && b == 1, "monotonic seq 1");
        check(s.inFlightCount() == 2, "in-flight count");
    }
    {
        Sender s;
        ushort seq;
        auto huge = new ubyte[](MAX_PAYLOAD + 1);
        check(s.submit(huge, seq) == SubmitStatus.payloadTooLarge, "payload too large");
    }
    {
        Sender s;
        ushort seq;
        foreach (i; 0 .. WINDOW)
            check(s.submit([0], seq) == SubmitStatus.ok, "fill window");
        check(s.submit([0], seq) == SubmitStatus.windowFull, "window full");
    }
    {
        Sender s;
        ushort seq;
        s.submit([1], seq);
        auto base = MonoTime.currTime;
        Heartbeat hb;
        check(s.pendingHeartbeat(base, hb), "heartbeat fires first");
        check(hb.first == 0 && hb.last == 0 && hb.stream == 0x80, "heartbeat body");
        check(!s.pendingHeartbeat(base + dur!"msecs"(100), hb), "heartbeat silenced <500ms");
        check(s.pendingHeartbeat(base + dur!"msecs"(600), hb), "heartbeat after 500ms");
    }
    {
        Sender s;
        Heartbeat hb;
        check(!s.pendingHeartbeat(MonoTime.currTime, hb), "no heartbeat when empty");
    }
    {
        Sender s;
        ushort seq;
        s.submit([0xA0], seq); // 0
        s.submit([0xA1], seq); // 1
        s.submit([0xA2], seq); // 2
        // base=2, bitmap=0b1 => seq2 missing, 0+1 acked
        s.recvAcknack(AckNack(2, [0x01, 0x00], 0x80));
        check(s.inFlightCount() == 1, "acknack clears acked");
        check(s.getInFlight(2) !is null, "seq2 retransmittable");
    }
    {
        Sender s;
        ushort seq;
        foreach (i; 0 .. 5)
            s.submit([0], seq);
        s.recvAcknack(AckNack(5, [0, 0], 0x80)); // full clear
        check(s.inFlightCount() == 0, "acknack full clear");
    }
    // --- Receiver ---
    {
        Receiver r;
        r.recvData(0, [10]);
        r.recvData(1, [11]);
        auto d = r.drainInOrder();
        check(d.length == 2 && d[0][0] == 10 && d[1][0] == 11, "in-order drain");
        check(r.expected() == 2, "expected advanced");
    }
    {
        Receiver r;
        r.recvData(2, [22]);
        r.recvData(0, [20]);
        auto d1 = r.drainInOrder();
        check(d1.length == 1 && d1[0][0] == 20, "reorder: only seq0");
        r.recvData(1, [21]);
        auto d2 = r.drainInOrder();
        check(d2.length == 2 && d2[0][0] == 21 && d2[1][0] == 22, "reorder: 1+2");
    }
    {
        Receiver r;
        r.recvData(0, [1]);
        r.drainInOrder();
        r.recvData(0, [99]); // duplicate
        check(r.outOfOrderCount() == 0, "duplicate dropped");
    }
    {
        Receiver r;
        foreach (ushort i; 1 .. cast(ushort)(RECV_BUF + 1))
            check(r.recvData(i, [1]) == Receiver.RecvStatus.ok, "fill recv buffer");
        check(r.recvData(cast(ushort)(RECV_BUF + 1), [1]) == Receiver.RecvStatus.bufferFull,
            "recv buffer full");
    }
    {
        Receiver r;
        r.recvData(1, [1]);
        r.recvData(3, [3]);
        auto a = r.pendingAcknack(true, 3);
        ushort bm = cast(ushort)(a.nack[0] | (a.nack[1] << 8));
        check((bm & 1) != 0, "slot 0 missing");
        check((bm & (1 << 2)) != 0, "slot 2 missing");
        check((bm & (1 << 1)) == 0, "slot 1 present");
        check((bm & (1 << 3)) == 0, "slot 3 present");
    }
    {
        Sender s;
        Receiver r;
        ushort seq;
        s.submit([1, 2], seq);
        r.recvData(0, [3]);
        // reset both
        Sender s2;
        r.reset();
        check(r.expected() == 0 && r.outOfOrderCount() == 0, "reset clears receiver");
    }
    // --- end-to-end loss recovery (in-proc) ---
    {
        Sender s;
        Receiver r;
        ushort[] seqs;
        foreach (i; 0 .. 3)
        {
            ushort seq;
            s.submit([cast(ubyte) i], seq);
            seqs ~= seq;
        }
        r.recvData(seqs[0], [0]); // seq1 lost
        r.recvData(seqs[2], [2]);
        auto d = r.drainInOrder();
        check(d.length == 1, "only seq0 before recovery");
        auto ack = r.pendingAcknack(true, seqs[2]);
        s.recvAcknack(ack);
        check(s.getInFlight(seqs[1]) !is null, "seq1 retransmittable");
        r.recvData(seqs[1], [1]);
        auto d2 = r.drainInOrder();
        check(d2.length == 2, "seq1+2 after recovery");
    }

    // --- byte-golden ---
    auto hb = heartbeatFrame(Heartbeat(1, 3, 0x80));
    immutable ubyte[] hbExpect = [
        0x80, 0x00, 0x01, 0x00, 0x0b, 0x01, 0x05, 0x00, 0x01, 0x00, 0x03, 0x00, 0x80
    ];
    check(hb == hbExpect, "heartbeat byte-golden (hardcoded)");
    auto ak = acknackFrame(AckNack(1, [0, 0], 0x80));
    immutable ubyte[] akExpect = [
        0x80, 0x00, 0x01, 0x00, 0x0a, 0x01, 0x05, 0x00, 0x01, 0x00, 0x00, 0x00, 0x80
    ];
    check(ak == akExpect, "acknack byte-golden (hardcoded)");

    if (args.length >= 3)
    {
        auto gHb = cast(ubyte[]) read(args[1]);
        auto gAk = cast(ubyte[]) read(args[2]);
        check(hb == gHb, "heartbeat byte-identical to golden file");
        check(ak == gAk, "acknack byte-identical to golden file");
        writeln("golden files matched");
    }

    writeln("ALL OK");
}
