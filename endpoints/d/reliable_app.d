// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Reliable-sender E2E app: argv = <peer-port> <N>. Submits N samples on the
// reliable stream, sends each as WRITE_DATA over UDP to the Rust peer, then
// loops heartbeat + on-ACKNACK retransmit until the send window drains. The
// peer (zerodds-endpoint-e2e) injects loss and replies ACKNACK; loss recovery
// is proven when every sample lands gap-free in order on the peer.
module reliable_app;

import std.socket;
import std.conv : to;
import std.stdio : stderr;
import core.time : MonoTime, dur;
import reliable;

void main(string[] args)
{
    immutable ushort port = to!ushort(args[1]);
    immutable int n = to!int(args[2]);

    auto sock = new UdpSocket();
    sock.bind(new InternetAddress("127.0.0.1", InternetAddress.PORT_ANY));
    auto peer = new InternetAddress("127.0.0.1", port);
    // 50 ms receive timeout so the ACKNACK poll never blocks forever.
    sock.setOption(SocketOptionLevel.SOCKET, SocketOption.RCVTIMEO, dur!"msecs"(50));

    Sender s;
    // Sample i = its index as 4-byte little-endian (the peer/test decodes it).
    ubyte[] sampleOf(int i)
    {
        return [
            cast(ubyte)(i & 0xFF), cast(ubyte)((i >> 8) & 0xFF),
            cast(ubyte)((i >> 16) & 0xFF), cast(ubyte)((i >> 24) & 0xFF)
        ];
    }

    // Initial burst: submit + send every sample once.
    foreach (i; 0 .. n)
    {
        ushort seq;
        auto st = s.submit(sampleOf(i), seq);
        if (st != SubmitStatus.ok)
        {
            stderr.writeln("submit failed: ", st);
            return;
        }
        sock.sendTo(reliableWriteFrame(seq, sampleOf(i)), peer);
    }

    // Recovery loop: heartbeat + drain ACKNACKs + retransmit, until window empty.
    immutable deadline = MonoTime.currTime + dur!"seconds"(20);
    ubyte[4096] buf;
    while (s.inFlightCount() > 0 && MonoTime.currTime < deadline)
    {
        // Prompt the peer for an ACKNACK.
        Heartbeat hb;
        if (s.pendingHeartbeat(MonoTime.currTime, hb))
            sock.sendTo(heartbeatFrame(hb), peer);
        else
        {
            // Not yet due per the 500 ms gate — send one anyway to keep the
            // test snappy (heartbeat gating itself is covered by the unit tests).
            auto seqs = s.inFlightSeqs();
            sock.sendTo(heartbeatFrame(Heartbeat(cast(short) seqs[0],
                    cast(short) seqs[$ - 1], STREAM_RELIABLE)), peer);
        }

        Address from;
        auto got = sock.receiveFrom(buf[], from);
        if (got <= 0)
            continue;
        AckNack a;
        if (!parseAckNack(buf[0 .. got], a))
            continue;
        s.recvAcknack(a);
        // Retransmit every still-in-flight sample the peer marked missing.
        ushort base = cast(ushort) a.firstUnacked;
        ushort bitmap = cast(ushort)(a.nack[0] | (a.nack[1] << 8));
        foreach (ushort i; 0 .. 16)
        {
            if (((bitmap >> i) & 1) == 0)
                continue;
            ushort seq = cast(ushort)(base + i);
            auto p = s.getInFlight(seq);
            if (p !is null)
                sock.sendTo(reliableWriteFrame(seq, p), peer);
        }
    }

    if (s.inFlightCount() == 0)
        stderr.writeln("RELIABLE OK: all ", n, " acknowledged");
    else
        stderr.writeln("RELIABLE INCOMPLETE: ", s.inFlightCount(), " still in-flight");
}
