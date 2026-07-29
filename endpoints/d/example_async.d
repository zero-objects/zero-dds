// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deeper ASYNC example for the native D endpoint: the same telemetry publisher,
// but the subscriber consumes via the std.concurrency actor (feed/recv) and
// decodes every field.
// Build: `gdc example_async.d zerodds.d -o example_async_bin && ./example_async_bin`

import zerodds;
import std.stdio;
import std.format : format;

struct Reading {
    uint id;
    float value;
    string label;

    ubyte[] marshal(Endian endian) {
        auto w = Writer(endian);
        w.putU32(id);
        w.putF32(value);
        w.putString(label);
        return w.bytes();
    }
}

Reading decodeReading(const(ubyte)[] body) {
    auto r = Reader(body, Endian.LE);
    return Reading(r.getU32(), r.getF32(), r.getString());
}

void main() {
    enum total = 5;
    auto reader = new AsyncReader();

    // Publisher: frame + feed 5 typed readings to the reader actor.
    foreach (i; 0 .. total) {
        auto rd = Reading(cast(uint)(0x2000 + i), 100.0f - i, format("sensor-%02d", i));
        reader.feed(writeFrame(SessionNoKey, StreamBestEffort, i + 1, rd.marshal(Endian.LE)));
    }

    // Subscriber: recv each decoded body; decode every field; stop at `total`.
    foreach (got; 0 .. total) {
        auto body = reader.recv();
        auto rd = decodeReading(body);
        writefln("async reading %d: id=0x%x value=%.1f label=\"%s\"", got, rd.id, rd.value, rd.label);
    }
    reader.stop();
    writeln("ALL OK");
}
