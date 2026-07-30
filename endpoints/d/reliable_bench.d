// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Producer-latency micro-bench: the async-decoupled writer's wait-free ring
// enqueue vs. an inline UDP sendto. Prints median ns for both — the syscall the
// enqueue avoids on the producer's hot path.
module reliable_bench;

import std.stdio : writefln;
import std.socket;
import std.algorithm : sort;
import core.time : MonoTime;
import reliable;

void main()
{
    enum int ITERS = 20_000;
    ubyte[] frame = reliableWriteFrame(0, [1, 2, 3, 4]);

    // Inline path: a real sendto per sample (kernel transition).
    auto sock = new UdpSocket();
    sock.bind(new InternetAddress("127.0.0.1", InternetAddress.PORT_ANY));
    auto sink = new UdpSocket();
    sink.bind(new InternetAddress("127.0.0.1", InternetAddress.PORT_ANY));
    auto sinkAddr = cast(InternetAddress) sink.localAddress;

    auto inlineNs = new long[](ITERS);
    foreach (i; 0 .. ITERS)
    {
        auto t0 = MonoTime.currTime;
        sock.sendTo(frame, sinkAddr);
        inlineNs[i] = (MonoTime.currTime - t0).total!"nsecs";
    }

    // Decoupled path: wait-free ring enqueue only (no syscall on producer).
    auto ring = new SpscRing();
    auto ringNs = new long[](ITERS);
    size_t drained;
    foreach (i; 0 .. ITERS)
    {
        auto t0 = MonoTime.currTime;
        if (!ring.enqueue(frame))
        {
            // ring full — drain one to make room (amortized; not the producer's job
            // in production, done here so the bench can push ITERS items)
            ubyte[] tmp;
            ring.dequeue(tmp);
            drained++;
            ring.enqueue(frame);
        }
        ringNs[i] = (MonoTime.currTime - t0).total!"nsecs";
    }

    inlineNs.sort();
    ringNs.sort();
    immutable inlineMed = inlineNs[ITERS / 2];
    immutable ringMed = ringNs[ITERS / 2];
    writefln("producer latency: ring-enqueue median = %d ns, inline-sendto median = %d ns (%.0fx)",
        ringMed, inlineMed, cast(double) inlineMed / (ringMed == 0 ? 1 : ringMed));
}
