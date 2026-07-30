// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Native pure-D endpoint SDK (ADR 0013): a from-scratch XCDR wire-core,
// byte-identical to the Rust core and the other SDKs. Sync (poll) and async
// (std.concurrency spawn/send/receive — Tid message passing, idiomatic D).

module zerodds;

import std.concurrency;

enum Endian { LE, BE }

// --- Writer: a ubyte buffer + endian; alignment relative to buffer start, cap 4 ---

struct Writer {
    ubyte[] buf;
    Endian endian;

    this(Endian e) { endian = e; }

    void alignTo(int a) {
        int cap = a < 4 ? a : 4;
        int pad = (cap - (cast(int) buf.length % cap)) % cap;
        foreach (_; 0 .. pad) buf ~= 0;
    }

    // le: the value already encoded little-endian; reversed for BE.
    void put(int a, ubyte[] le) {
        alignTo(a);
        if (endian == Endian.BE)
            foreach_reverse (b; le) buf ~= b;
        else
            foreach (b; le) buf ~= b;
    }

    void putU8(int v) { buf ~= cast(ubyte)(v & 0xff); }
    void putU16(int v) { put(2, [cast(ubyte) v, cast(ubyte)(v >> 8)]); }

    void putU32(uint v) {
        put(4, [cast(ubyte) v, cast(ubyte)(v >> 8), cast(ubyte)(v >> 16), cast(ubyte)(v >> 24)]);
    }

    void putU64(ulong v) {
        ubyte[] b;
        foreach (i; 0 .. 8) b ~= cast(ubyte)(v >> (8 * i));
        put(4, b);
    }

    void putF32(float v) {
        uint bits = *cast(uint*)&v;
        put(4, [cast(ubyte) bits, cast(ubyte)(bits >> 8), cast(ubyte)(bits >> 16), cast(ubyte)(bits >> 24)]);
    }

    void putString(string s) {
        putU32(cast(uint)(s.length + 1));
        foreach (c; s) buf ~= cast(ubyte) c;
        putU8(0);
    }

    void putSeqU8(ubyte[] b) {
        putU32(cast(uint) b.length);
        foreach (x; b) buf ~= x;
    }

    ubyte[] bytes() { return buf; }
}

// --- Reader: a byte cursor ---

struct Reader {
    const(ubyte)[] data;
    int pos;
    Endian endian;

    this(const(ubyte)[] d, Endian e) { data = d; endian = e; }

    ubyte[] take(int a, int n) {
        int cap = a < 4 ? a : 4;
        int pad = (cap - (pos % cap)) % cap;
        pos += pad;
        ubyte[] chunk = data[pos .. pos + n].dup;
        pos += n;
        if (endian == Endian.BE) {
            ubyte[] rev;
            foreach_reverse (b; chunk) rev ~= b;
            return rev;
        }
        return chunk;
    }

    uint getU32() {
        auto b = take(4, 4);
        return cast(uint) b[0] | (cast(uint) b[1] << 8) | (cast(uint) b[2] << 16) | (cast(uint) b[3] << 24);
    }

    // --- primitives beyond getU32 — the byte-exact inverse of the Writer ---
    ubyte getU8() {
        ubyte b = data[pos];
        pos += 1;
        return b;
    }
    ushort getU16() {
        auto b = take(2, 2);
        return cast(ushort)(cast(ushort) b[0] | (cast(ushort) b[1] << 8));
    }
    ulong getU64() {
        auto b = take(4, 8);
        ulong v = 0;
        foreach (i; 0 .. 8) v |= (cast(ulong) b[i]) << (8 * i);
        return v;
    }
    float getF32() {
        uint bits = getU32();
        return *cast(float*)&bits;
    }
    string getString() {
        uint n = getU32();
        string s = cast(string) data[pos .. pos + n - 1].idup;
        pos += n;
        return s;
    }
    ubyte[] getSeqU8() {
        uint n = getU32();
        ubyte[] b = data[pos .. pos + n].dup;
        pos += n;
        return b;
    }
}

// --- XRCE WRITE_DATA framing ---

enum ubyte SessionNoKey = 0x80;
enum ubyte StreamBestEffort = 0x01;

ubyte[] writeFrame(ubyte session, ubyte stream, int seqNo, const(ubyte)[] sample) {
    // The 16-bit submessage_length field cannot describe a sample larger than
    // 0xFFFF; refuse (null) rather than truncate the length while appending the
    // full payload (mirrors the C SDK's rejection).
    if (sample.length > 0xFFFF)
        return null;
    ubyte[] r = [
        session, stream,
        cast(ubyte)(seqNo & 0xff), cast(ubyte)((seqNo >> 8) & 0xff),
        0x07, 0x03,
        cast(ubyte)(sample.length & 0xff), cast(ubyte)((sample.length >> 8) & 0xff)
    ];
    foreach (x; sample) r ~= x;
    return r;
}

// Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo, len_hi]
// then the sample body. The 16-bit little-endian submessage_length (bytes 6..7)
// bounds the body exactly: frame[8 .. 8+smLen], never frame[8 .. $], so trailing
// padding or an appended submessage is not folded into the sample.
//
// Accepts WRITE_DATA (0x07, endpoint->hub / loopback) and DATA (0x09,
// hub->endpoint) — the hub pushes samples to a client with DATA. See DDS-XRCE
// spec section 8.3.5. Returns null for a short header, a wrong submessage id, or
// a declared length that runs past the datagram (truncation / wrong length).
ubyte[] readFrame(const(ubyte)[] frame) {
    if (frame.length < 8 || (frame[4] != 0x07 && frame[4] != 0x09))
        return null;
    size_t smLen = cast(size_t)frame[6] | (cast(size_t)frame[7] << 8);
    if (8 + smLen > frame.length)
        return null;
    return frame[8 .. 8 + smLen].dup;
}

// --- transport ---

struct Transport {
    void delegate(ubyte[]) deliver;
    ubyte[] delegate() receive; // null if nothing
}

// --- sync client ---

class Client {
    Transport transport;
    ubyte session = SessionNoKey;
    ubyte stream = StreamBestEffort;
    int seqNo = 1;

    this(Transport t) { transport = t; }

    void write(ubyte[] sample) {
        auto frame = writeFrame(session, stream, seqNo, sample);
        transport.deliver(frame);
        seqNo = (seqNo + 1) & 0xffff;
    }

    // One non-blocking receive: returns the sample body, or null.
    ubyte[] poll() {
        auto f = transport.receive();
        if (f is null) return null;
        return readFrame(f);
    }
}

// In-memory FIFO transport for tests + example (sync path).
Transport memTransport() {
    ubyte[][] q;
    Transport t;
    t.deliver = (ubyte[] frame) { q ~= frame.dup; };
    t.receive = () {
        if (q.length) { auto f = q[0]; q = q[1 .. $]; return f; }
        return cast(ubyte[]) null;
    };
    return t;
}

// --- async reader: a std.concurrency actor (spawn/send/receive) ---

// The reader thread receives raw frames as messages and sends decoded sample
// bodies back to its owner. Message passing is D's idiomatic concurrency; the
// payloads are immutable so they cross the thread boundary safely.
void readerLoop(Tid owner) {
    bool running = true;
    while (running) {
        receive(
            (immutable(ubyte)[] frame) {
                auto b = readFrame(frame);
                if (b.length) send(owner, b.idup);
            },
            (bool _stop) { running = false; }
        );
    }
}

class AsyncReader {
    Tid tid;

    this() { tid = spawn(&readerLoop, thisTid); }

    void feed(ubyte[] frame) { send(tid, frame.idup); }
    immutable(ubyte)[] recv() { return receiveOnly!(immutable(ubyte)[])(); }
    void stop() { send(tid, true); }
}
