// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// ReliableWire -- XRCE reliable-stream wire codec (Spec §8.4.10/§8.4.11) for
// the pure-Java endpoint SDK (ADR 0013). Byte-identical to crates/xrce and
// endpoints/c/d/rust: WRITE_DATA carries the RFC-1982 sample sequence in the
// 2-byte header field; HEARTBEAT/ACKNACK use the golden control-frame layout
// (session 0x80, stream NONE, msg-seq 1) that endpoints/golden-gen emits --
// the peer only inspects byte[4] (submessage id) and the body at offset 8,
// so any stream/seq bytes at [1..4) are accepted, but this layout is the one
// asserted byte-identical against golden_heartbeat_le.bin/golden_acknack_le.bin.
//
// Frame = 8-byte header + body, little-endian:
//   [0]=session [1]=stream [2..4)=seq LE [4]=submsg id [5]=flags
//   [6..8)=body-len LE [8..)=body

package org.zerodds.endpoint;

import java.io.ByteArrayOutputStream;
import java.util.Arrays;

public final class ReliableWire {

    public static final int SESSION_NOKEY = 0x80;
    public static final int STREAM_RELIABLE = 0x80; // reliable stream id (bit 7 set)
    public static final int STREAM_NONE = 0x00; // control-frame stream in the golden layout

    public static final int SM_WRITE_DATA = 0x07;
    public static final int SM_ACKNACK = 0x0A;
    public static final int SM_HEARTBEAT = 0x0B;
    public static final int FLAGS_WRITE = 0x03;
    public static final int FLAGS_CTRL = 0x01;

    /** Sender window cap: 16 in-flight samples (matches the 16-bit ACKNACK bitmap). */
    public static final int WINDOW = 16;
    /** Receiver out-of-order buffer cap (DoS bound). */
    public static final int RECV_BUF = 64;
    /** Per-sample payload cap: 64 KiB (u16 submessage length limit). */
    public static final int MAX_PAYLOAD = 65535;
    /** Heartbeat period (spec recommends 100ms; 500ms here, no Tx pacing layer underneath). */
    public static final long HEARTBEAT_PERIOD_MS = 500;

    private ReliableWire() {}

    // -----------------------------------------------------------------
    // RFC-1982 16-bit circular sequence-number comparisons.
    // -----------------------------------------------------------------

    /** {@code a < b} per RFC-1982 (wrapping), for {@code a,b} in [0,65535]. */
    public static boolean seqLt(int a, int b) {
        return ((a - b) & 0xFFFF) >= 0x8000;
    }

    /** {@code a > b} per RFC-1982 (wrapping). */
    public static boolean seqGt(int a, int b) {
        int d = (a - b) & 0xFFFF;
        return d != 0 && d < 0x8000;
    }

    // -----------------------------------------------------------------
    // Control-message payloads.
    // -----------------------------------------------------------------

    public static final class Heartbeat {
        public final int first;
        public final int last;
        public final int stream;

        public Heartbeat(int first, int last, int stream) {
            this.first = first & 0xFFFF;
            this.last = last & 0xFFFF;
            this.stream = stream & 0xFF;
        }
    }

    public static final class AckNack {
        public final int firstUnacked;
        public final int nackLo;
        public final int nackHi;
        public final int stream;

        public AckNack(int firstUnacked, int nackLo, int nackHi, int stream) {
            this.firstUnacked = firstUnacked & 0xFFFF;
            this.nackLo = nackLo & 0xFF;
            this.nackHi = nackHi & 0xFF;
            this.stream = stream & 0xFF;
        }

        public int bitmap() {
            return (nackLo & 0xFF) | ((nackHi & 0xFF) << 8);
        }
    }

    public static final class WriteData {
        public final int seq;
        public final byte[] sample;

        public WriteData(int seq, byte[] sample) {
            this.seq = seq;
            this.sample = sample;
        }
    }

    // -----------------------------------------------------------------
    // Frame builders.
    // -----------------------------------------------------------------

    /** Reliable WRITE_DATA frame: the header seq carries the RFC-1982 sample seq. */
    public static byte[] writeFrame(int seq, byte[] sample) {
        ByteArrayOutputStream o = new ByteArrayOutputStream(8 + sample.length);
        o.write(SESSION_NOKEY);
        o.write(STREAM_RELIABLE);
        o.write(seq & 0xFF);
        o.write((seq >> 8) & 0xFF);
        o.write(SM_WRITE_DATA);
        o.write(FLAGS_WRITE);
        o.write(sample.length & 0xFF);
        o.write((sample.length >> 8) & 0xFF);
        o.write(sample, 0, sample.length);
        return o.toByteArray();
    }

    private static byte[] ctrlHeader(int submsg) {
        // golden layout: session, stream=NONE, msg-seq=1, submsg, flags, len=5
        return new byte[] {
            (byte) SESSION_NOKEY, (byte) STREAM_NONE, 0x01, 0x00,
            (byte) submsg, (byte) FLAGS_CTRL, 0x05, 0x00
        };
    }

    public static byte[] heartbeatFrame(Heartbeat h) {
        ByteArrayOutputStream o = new ByteArrayOutputStream(13);
        o.write(ctrlHeader(SM_HEARTBEAT), 0, 8);
        o.write(h.first & 0xFF);
        o.write((h.first >> 8) & 0xFF);
        o.write(h.last & 0xFF);
        o.write((h.last >> 8) & 0xFF);
        o.write(h.stream & 0xFF);
        return o.toByteArray();
    }

    public static byte[] acknackFrame(AckNack a) {
        ByteArrayOutputStream o = new ByteArrayOutputStream(13);
        o.write(ctrlHeader(SM_ACKNACK), 0, 8);
        o.write(a.firstUnacked & 0xFF);
        o.write((a.firstUnacked >> 8) & 0xFF);
        o.write(a.nackLo & 0xFF);
        o.write(a.nackHi & 0xFF);
        o.write(a.stream & 0xFF);
        return o.toByteArray();
    }

    // -----------------------------------------------------------------
    // Frame parsers.
    // -----------------------------------------------------------------

    public static WriteData parseWrite(byte[] f) {
        if (f.length < 8 || (f[4] & 0xFF) != SM_WRITE_DATA) {
            return null;
        }
        int seq = i16(f, 2);
        byte[] sample = Arrays.copyOfRange(f, 8, f.length);
        return new WriteData(seq, sample);
    }

    /** Parses a control frame by byte[4]=submsg-id, body at offset 8 (peer contract). */
    public static Heartbeat parseHeartbeat(byte[] f) {
        if (f.length < 13 || (f[4] & 0xFF) != SM_HEARTBEAT) {
            return null;
        }
        return new Heartbeat(i16(f, 8), i16(f, 10), f[12] & 0xFF);
    }

    public static AckNack parseAckNack(byte[] f) {
        if (f.length < 13 || (f[4] & 0xFF) != SM_ACKNACK) {
            return null;
        }
        return new AckNack(i16(f, 8), f[10] & 0xFF, f[11] & 0xFF, f[12] & 0xFF);
    }

    private static int i16(byte[] b, int off) {
        return (b[off] & 0xFF) | ((b[off + 1] & 0xFF) << 8);
    }
}
