// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// In-process reliable demo: a sender submits N samples, a lossy channel drops
// every 3rd, and ACKNACK-driven retransmit recovers the full contiguous
// sequence. Prints "delivered: 0 1 … / RELIABLE OK".
module example_reliable;

import std.stdio : write, writeln;
import reliable;

void main()
{
    enum int N = 12;
    Sender s;
    Receiver r;

    // Submit + first delivery attempt, dropping every 3rd sample.
    foreach (i; 0 .. N)
    {
        ushort seq;
        s.submit([cast(ubyte) i], seq);
        if ((i % 3) != 2) // drop every 3rd
            r.recvData(seq, [cast(ubyte) i]);
    }

    // Recover: peer's ACKNACK reveals the gaps, sender retransmits.
    foreach (round; 0 .. N)
    {
        auto delivered = r.drainInOrder();
        if (r.expected() == N)
            break;
        auto ack = r.pendingAcknack(true, cast(ushort)(N - 1));
        s.recvAcknack(ack);
        ushort base = cast(ushort) ack.firstUnacked;
        ushort bitmap = cast(ushort)(ack.nack[0] | (ack.nack[1] << 8));
        foreach (ushort b; 0 .. 16)
        {
            if (((bitmap >> b) & 1) == 0)
                continue;
            ushort seq = cast(ushort)(base + b);
            auto p = s.getInFlight(seq);
            if (p !is null)
                r.recvData(seq, p);
        }
    }
    r.drainInOrder();

    write("delivered:");
    foreach (i; 0 .. r.expected())
        write(" ", i);
    writeln();
    if (r.expected() == N)
        writeln("RELIABLE OK");
    else
        writeln("RELIABLE INCOMPLETE at ", r.expected());
}
