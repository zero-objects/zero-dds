// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Runnable example for the native D endpoint: sync (poll) and async
// (std.concurrency actor). Built alongside zerodds.d.

import zerodds;
import std.stdio;

ubyte[] sample(uint id, string label) {
    auto w = Writer(Endian.LE);
    w.putU32(id);
    w.putString(label);
    return w.bytes();
}

void main() {
    // sync
    auto t = memTransport();
    auto c = new Client(t);
    c.write(sample(0x42, "sync-hello"));
    auto body = c.poll();
    if (body !is null) {
        auto rd = Reader(body, Endian.LE);
        writefln("sync: received id=0x%x", rd.getU32());
    }

    // async
    auto r = new AsyncReader();
    foreach (i; 0 .. 3)
        r.feed(writeFrame(SessionNoKey, StreamBestEffort, i + 1, sample(0x100 + i, "async")));
    foreach (i; 0 .. 3) {
        auto b = r.recv();
        auto rd = Reader(b, Endian.LE);
        writefln("async: received id=0x%x", rd.getU32());
    }
    r.stop();

    writeln("ALL OK");
}
