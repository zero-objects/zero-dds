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
    // --- RFC-1982 regression: HEARTBEAT window + loss recovery across the
    //     16-bit wrap (mirrors crates/xrce's wrap regression tests). Seeds
    //     sender/receiver up to the wrap via the public API only (submit +
    //     full-ack / recvData + drain), then straddles 0x0000.
    {
        Sender s;
        ushort seq;
        do // walk nextSeq to 0xFFFE: submit one, fully-ack it, repeat.
        {
            s.submit([0], seq);
            s.recvAcknack(AckNack(cast(short)((seq + 1) & 0xFFFF), [0, 0], 0x80));
        }
        while (seq != 0xFFFD);
        check(s.inFlightCount() == 0, "wrap seed: sender window drained");

        ushort q0, q1, q2, q3;
        s.submit([10], q0); // 0xFFFE
        s.submit([11], q1); // 0xFFFF (lost)
        s.submit([12], q2); // 0x0000
        s.submit([13], q3); // 0x0001
        check(q0 == 0xFFFE && q1 == 0xFFFF && q2 == 0x0000 && q3 == 0x0001, "wrap seqs");

        Heartbeat hb;
        check(s.pendingHeartbeat(MonoTime.currTime, hb), "wrap heartbeat fires");
        check(cast(ushort) hb.first == 0xFFFE && cast(ushort) hb.last == 0x0001,
            "heartbeat window across wrap = [0xFFFE,0x0001] (not numeric 0,0xFFFF)");

        Receiver r; // seed expected to 0xFFFE
        foreach (k; 0 .. 0xFFFE)
        {
            r.recvData(cast(ushort) k, [0]);
            r.drainInOrder();
        }
        check(r.expected() == 0xFFFE, "wrap seed: receiver expects 0xFFFE");

        r.recvData(q0, [10]); // 0xFFFF lost
        r.recvData(q2, [12]);
        r.recvData(q3, [13]);
        auto d1 = r.drainInOrder();
        check(d1.length == 1 && d1[0][0] == 10, "only 0xFFFE before recovery");
        check(r.expected() == 0xFFFF, "receiver blocked at 0xFFFF");

        auto ack = r.pendingAcknack(true, q3);
        check(cast(ushort) ack.firstUnacked == 0xFFFF, "acknack base = 0xFFFF across wrap");
        ushort bm = cast(ushort)(ack.nack[0] | (ack.nack[1] << 8));
        check((bm & 0b1) != 0 && (bm & 0b110) == 0, "only 0xFFFF NACKed");

        s.recvAcknack(ack);
        check(s.getInFlight(q1) !is null, "0xFFFF retransmittable");
        check(s.getInFlight(q0) is null && s.inFlightCount() == 1, "others acked");

        r.recvData(q1, s.getInFlight(q1));
        auto d2 = r.drainInOrder();
        check(d2.length == 3 && d2[0][0] == 11 && d2[1][0] == 12 && d2[2][0] == 13,
            "0xFFFF,0x0000,0x0001 deliver in RFC-1982 order");
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
