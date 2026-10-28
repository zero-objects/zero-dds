// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Tests for the native D endpoint: byte-identity vs the Rust goldens, plus
// sync + async loopback. Built alongside zerodds.d (gdc/ldc2).

import zerodds;
import surface_test : surfaceTests;
import xrce_client_test : xrceClientTests;
import std.stdio;
import std.file : read;
import std.path : buildPath;
import std.process : environment;

ubyte[] fixture(Endian e) {
    auto w = Writer(e, Encoding.XCDR2);
    w.putU32(0xA1B2C3D4);
    w.putU16(0x1234);
    w.putU8(0x5A);
    w.putF32(3.5f);
    w.putU64(0x0102030405060708);
    w.putString("bay-12");
    w.putSeqU8([cast(ubyte) 0xDE, 0xAD, 0xBE, 0xEF]);
    return w.bytes();
}

ubyte[] sampleBody(uint id) {
    auto w = Writer(Endian.LE, Encoding.XCDR2);
    w.putU32(id);
    w.putU16(0);
    w.putU8(0);
    return w.bytes();
}

void checkGolden(Endian e, string file, string dir) {
    auto got = fixture(e);
    auto golden = cast(ubyte[]) read(buildPath(dir, file));
    assert(got == golden, file ~ ": not byte-identical");
    writeln(file, ": ", golden.length, " bytes byte-identical");
}

void main() {
    string dir = environment.get("GOLDEN_DIR", "build");

    // byte identity
    checkGolden(Endian.LE, "golden_le.bin", dir);
    checkGolden(Endian.BE, "golden_be.bin", dir);

    // sync loopback
    auto t = memTransport();
    auto c = new Client(t, xrceObjectId(0x001, ObjkDataWriter));
    foreach (i; 0 .. 5) c.write(sampleBody(0x3000 + i));
    foreach (i; 0 .. 5) {
        auto body = c.poll();
        assert(body !is null, "sync: no sample");
        auto rd = Reader(body, Endian.LE, Encoding.XCDR2);
        assert(rd.getU32() == 0x3000 + i, "sync: id mismatch");
    }
    writeln("sync loopback: 5 samples OK");

    // async loopback (std.concurrency actor)
    auto r = new AsyncReader();
    foreach (i; 0 .. 5)
        r.feed(writeFrame(SessionNoKey, StreamBestEffort, i + 1, xrceObjectId(0x001, ObjkDataWriter), sampleBody(0x1000 + i)));
    foreach (i; 0 .. 5) {
        auto body = r.recv();
        auto rd = Reader(body, Endian.LE, Encoding.XCDR2);
        assert(rd.getU32() == 0x1000 + i, "async: id mismatch");
    }
    r.stop();
    writeln("async loopback: 5 samples OK");

    // negative frame vectors: length bounding + malformed reject
    {
        auto first = writeFrame(SessionNoKey, StreamBestEffort, 1, xrceObjectId(0x001, ObjkDataWriter), [cast(ubyte)0xAA, 0xBB, 0xCC]);
        auto second = writeFrame(SessionNoKey, StreamBestEffort, 2, xrceObjectId(0x001, ObjkDataWriter), [cast(ubyte)0xDD, 0xEE]);
        assert(readFrame(first ~ second) == [cast(ubyte)0xAA, 0xBB, 0xCC],
            "appended submessage must not leak into sample");
        auto overlong = first.dup;
        overlong[6] = 0xFF;
        overlong[7] = 0xFF;
        assert(readFrame(overlong) is null, "over-long length must reject");
        assert(readFrame([cast(ubyte)0x80, 0x01, 0x00, 0x00, 0x07, 0x01, 0x04, 0x00]) is null,
            "truncated header must reject");
        // A refusal and no longer a null slice: `Client.write` handed the
        // null to `deliver` and an empty datagram went out with nothing said.
        bool refused = false;
        try
            writeFrame(SessionNoKey, StreamBestEffort, 1, xrceObjectId(0x001, ObjkDataWriter), new ubyte[0x10000]);
        catch (SampleTooLargeException)
            refused = true;
        assert(refused, "sample > 0xFFFF must be refused");
        // The bound is the field's and not one off it.
        assert(writeFrame(SessionNoKey, StreamBestEffort, 1, xrceObjectId(0x001, ObjkDataWriter),
                   new ubyte[0xFFFF - XrceBaseSize]).length
               == 8 + 0xFFFF, "a maximal body must still frame");
        writeln("negative frame vectors: OK");
    }

    // The XCDR surface, against octets Cyclone, RTI, Fast DDS and omniORB wrote.
    // Called from here and not from a runner of its own, because `make test` is
    // the one command CI runs and a module nothing imports goes unrun.
    surfaceTests();

    // The DDS-XRCE 1.0 client: wire-encoder byte-identity + state-machine
    // lifecycle rules, ported from crates/xrce-client.
    xrceClientTests();

    writeln("ALL OK");
}
