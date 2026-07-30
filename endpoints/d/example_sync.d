// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Deeper SYNC example for the native D endpoint: a sensor-telemetry publisher
// writes typed Reading samples; a subscriber polls and decodes every field.
// Build: `gdc example_sync.d zerodds.d -o example_sync_bin && ./example_sync_bin`

import zerodds;
import std.stdio;
import std.format : format;

// Reading mirrors an IDL `@final struct Reading { uint32 id; float value; string label; }`.
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
    auto t = memTransport();
    auto c = new Client(t);

    // Publisher: frame + deliver 5 typed readings with varying values.
    foreach (i; 0 .. total) {
        auto rd = Reading(cast(uint)(0x1000 + i), 20.0f + i * 0.5f, format("bay-%02d", i));
        c.write(rd.marshal(Endian.LE));
    }

    // Subscriber: poll; decode every field; stop at `total`.
    int got = 0;
    while (got < total) {
        auto body = c.poll();
        if (body is null) break;
        auto rd = decodeReading(body);
        writefln("sync reading %d: id=0x%x value=%.1f label=\"%s\"", got, rd.id, rd.value, rd.label);
        got++;
    }
    if (got != total) { writeln("incomplete"); return; }
    writeln("ALL OK");
}
