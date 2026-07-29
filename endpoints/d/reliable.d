// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Reliable XRCE stream (stream_id >= 128, spec §8.4.10/§8.4.11) for the D
// endpoint SDK. Self-contained: state machine + HEARTBEAT/ACKNACK/WRITE_DATA
// wire codec + a lock-free SPSC ring with a drain thread (the async-decoupled
// writer). Byte-identical to endpoints/c + crates/xrce.
module reliable;

import core.time : Duration, MonoTime, dur;
import core.thread : Thread;
import core.atomic : atomicLoad, atomicStore, MemoryOrder;

enum int WINDOW = 16;          // in-flight cap (= 16-bit ACKNACK bitmap)
enum int RECV_BUF = 64;        // out-of-order buffer cap (DoS bound)
enum int MAX_PAYLOAD = 65535;  // u16 submessage length limit
enum Duration HEARTBEAT_PERIOD = dur!"msecs"(500);

enum ubyte SESSION = 0x80;
enum ubyte STREAM_RELIABLE = 0x80; // reliable stream id (bit 7 set)
enum ubyte STREAM_NONE = 0x00;     // control-frame stream in golden layout
enum ubyte SM_WRITE = 0x07;
enum ubyte SM_ACKNACK = 0x0A;
enum ubyte SM_HEARTBEAT = 0x0B;
enum ubyte FLAGS_WRITE = 0x03;
enum ubyte FLAGS_CTRL = 0x01;

/// RFC-1982 16-bit "a < b" (wrapping).
bool seqLt(ushort a, ushort b) @safe pure nothrow @nogc
{
    return cast(short)(a - b) < 0;
}

/// RFC-1982 16-bit "a > b".
bool seqGt(ushort a, ushort b) @safe pure nothrow @nogc
{
    return cast(short)(a - b) > 0;
}

struct Heartbeat
{
    short first;
    short last;
    ubyte stream;
}

struct AckNack
{
    short firstUnacked;
    ubyte[2] nack; // little-endian 16-bit bitmap
    ubyte stream;
}

enum SubmitStatus
{
    ok,
    payloadTooLarge,
    windowFull
}

// ---------------------------------------------------------------------------
// Wire codec (little-endian; byte-identical to golden_*.bin)
// ---------------------------------------------------------------------------

/// Reliable WRITE_DATA frame: header seq carries the RFC-1982 sample seq.
ubyte[] reliableWriteFrame(ushort seq, const(ubyte)[] sample)
{
    auto n = cast(ushort) sample.length;
    ubyte[] f = [
        SESSION, STREAM_RELIABLE,
        cast(ubyte)(seq & 0xFF), cast(ubyte)(seq >> 8),
        SM_WRITE, FLAGS_WRITE,
        cast(ubyte)(n & 0xFF), cast(ubyte)(n >> 8),
    ];
    f ~= sample;
    return f;
}

private ubyte[] ctrlHeader(ubyte submsg)
{
    // golden layout: session, stream=NONE, msg-seq = 1, submsg, flags, len=5
    return [SESSION, STREAM_NONE, 0x01, 0x00, submsg, FLAGS_CTRL, 0x05, 0x00];
}

ubyte[] heartbeatFrame(Heartbeat h)
{
    auto f = ctrlHeader(SM_HEARTBEAT);
    f ~= cast(ubyte)(h.first & 0xFF);
    f ~= cast(ubyte)((h.first >> 8) & 0xFF);
    f ~= cast(ubyte)(h.last & 0xFF);
    f ~= cast(ubyte)((h.last >> 8) & 0xFF);
    f ~= h.stream;
    return f;
}

ubyte[] acknackFrame(AckNack a)
{
    auto f = ctrlHeader(SM_ACKNACK);
    f ~= cast(ubyte)(a.firstUnacked & 0xFF);
    f ~= cast(ubyte)((a.firstUnacked >> 8) & 0xFF);
    f ~= a.nack[0];
    f ~= a.nack[1];
    f ~= a.stream;
    return f;
}

/// Parse a control frame's submessage id (byte 4), body at offset 8.
bool parseAckNack(const(ubyte)[] f, ref AckNack a) @safe pure nothrow @nogc
{
    if (f.length < 13 || f[4] != SM_ACKNACK)
        return false;
    a.firstUnacked = cast(short)(f[8] | (f[9] << 8));
    a.nack[0] = f[10];
    a.nack[1] = f[11];
    a.stream = f[12];
    return true;
}

bool parseHeartbeat(const(ubyte)[] f, ref Heartbeat h) @safe pure nothrow @nogc
{
    if (f.length < 13 || f[4] != SM_HEARTBEAT)
        return false;
    h.first = cast(short)(f[8] | (f[9] << 8));
    h.last = cast(short)(f[10] | (f[11] << 8));
    h.stream = f[12];
    return true;
}

/// Extract (seq, sample) from a reliable WRITE_DATA frame.
bool parseWrite(const(ubyte)[] f, ref ushort seq, ref const(ubyte)[] sample) @safe pure nothrow @nogc
{
    if (f.length < 8 || f[4] != SM_WRITE)
        return false;
    seq = cast(ushort)(f[2] | (f[3] << 8));
    sample = f[8 .. $];
    return true;
}

// ---------------------------------------------------------------------------
// Sender state machine (mirror of crates/xrce reliable.rs)
// ---------------------------------------------------------------------------

struct Sender
{
    private ushort nextSeq = 0;
    private ubyte[][ushort] inFlight;
    private bool haveHeartbeat = false;
    private MonoTime lastHeartbeat;
    ubyte stream = STREAM_RELIABLE;

    SubmitStatus submit(const(ubyte)[] payload, ref ushort seqOut)
    {
        if (payload.length > MAX_PAYLOAD)
            return SubmitStatus.payloadTooLarge;
        if (inFlight.length >= WINDOW)
            return SubmitStatus.windowFull;
        auto seq = nextSeq;
        inFlight[seq] = payload.dup;
        nextSeq = cast(ushort)(nextSeq + 1);
        seqOut = seq;
        return SubmitStatus.ok;
    }

    size_t inFlightCount() const
    {
        return inFlight.length;
    }

    const(ubyte)[] getInFlight(ushort seq) const
    {
        auto p = seq in inFlight;
        return p ? *p : null;
    }

    /// Sorted in-flight seqs (for deterministic retransmit).
    ushort[] inFlightSeqs() const
    {
        import std.algorithm : sort;
        import std.array : array;

        auto ks = inFlight.keys.dup;
        ks.sort();
        return ks;
    }

    bool pendingHeartbeat(MonoTime now, ref Heartbeat hb)
    {
        if (inFlight.length == 0)
            return false;
        bool due = !haveHeartbeat || (now - lastHeartbeat) >= HEARTBEAT_PERIOD;
        if (!due)
            return false;
        haveHeartbeat = true;
        lastHeartbeat = now;
        auto seqs = inFlightSeqs();
        hb = Heartbeat(cast(short) seqs[0], cast(short) seqs[$ - 1], stream);
        return true;
    }

    void recvAcknack(AckNack a)
    {
        ushort base = cast(ushort) a.firstUnacked;
        ushort bitmap = cast(ushort)(a.nack[0] | (a.nack[1] << 8));
        // 1) everything strictly before base is acknowledged
        foreach (k; inFlight.keys.dup)
        {
            if (seqLt(k, base))
                inFlight.remove(k);
        }
        // 2) within [base, base+16): clear bit => acked => drop
        foreach (ushort i; 0 .. 16)
        {
            ushort seq = cast(ushort)(base + i);
            if (((bitmap >> i) & 1) == 0)
                inFlight.remove(seq);
        }
    }
}

// ---------------------------------------------------------------------------
// Receiver state machine
// ---------------------------------------------------------------------------

struct Receiver
{
    private ushort expectedSeq = 0;
    private ubyte[][ushort] received;
    ubyte stream = STREAM_RELIABLE;

    enum RecvStatus
    {
        ok,
        bufferFull
    }

    RecvStatus recvData(ushort seq, const(ubyte)[] payload)
    {
        if (seqLt(seq, expectedSeq))
            return RecvStatus.ok; // duplicate, drop
        if (seq in received)
            return RecvStatus.ok; // already buffered
        if (received.length >= RECV_BUF)
            return RecvStatus.bufferFull;
        received[seq] = payload.dup;
        return RecvStatus.ok;
    }

    /// Contiguous samples from expected; advances expected.
    ubyte[][] drainInOrder()
    {
        ubyte[][] outp;
        while (true)
        {
            auto p = expectedSeq in received;
            if (p is null)
                break;
            outp ~= *p;
            received.remove(expectedSeq);
            expectedSeq = cast(ushort)(expectedSeq + 1);
        }
        return outp;
    }

    ushort expected() const
    {
        return expectedSeq;
    }

    size_t outOfOrderCount() const
    {
        return received.length;
    }

    /// Bitmap of missing slots in [expected, expected+16); slots > hint not marked.
    AckNack pendingAcknack(bool hasHint, ushort hint)
    {
        ushort bitmap = 0;
        foreach (ushort i; 0 .. 16)
        {
            ushort seq = cast(ushort)(expectedSeq + i);
            if (hasHint && seqGt(seq, hint))
                continue;
            if ((seq in received) is null)
                bitmap |= cast(ushort)(1 << i);
        }
        AckNack a;
        a.firstUnacked = cast(short) expectedSeq;
        a.nack[0] = cast(ubyte)(bitmap & 0xFF);
        a.nack[1] = cast(ubyte)(bitmap >> 8);
        a.stream = stream;
        return a;
    }

    void reset()
    {
        expectedSeq = 0;
        received.clear();
    }
}

// ---------------------------------------------------------------------------
// Lock-free SPSC ring + drain thread — the async-decoupled writer
// ---------------------------------------------------------------------------

/// Single-producer/single-consumer wait-free ring. The producer's enqueue is a
/// slot store + one release-store of head; no lock, no syscall.
struct SpscRing
{
    enum size_t CAP = 1024;
    private ubyte[][CAP] slots;
    private shared size_t head; // producer
    private shared size_t tail; // consumer

    bool enqueue(ubyte[] item)
    {
        immutable h = atomicLoad!(MemoryOrder.raw)(head);
        immutable t = atomicLoad!(MemoryOrder.acq)(tail);
        if (h - t >= CAP)
            return false; // full
        slots[h % CAP] = item;
        atomicStore!(MemoryOrder.rel)(head, h + 1);
        return true;
    }

    bool dequeue(ref ubyte[] item)
    {
        immutable t = atomicLoad!(MemoryOrder.raw)(tail);
        immutable h = atomicLoad!(MemoryOrder.acq)(head);
        if (t == h)
            return false; // empty
        item = slots[t % CAP];
        atomicStore!(MemoryOrder.rel)(tail, t + 1);
        return true;
    }
}
