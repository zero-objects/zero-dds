// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// ReliableWire -- XRCE reliable-stream wire codec (Spec §8.4.10/§8.4.11) for
// the pure-Java endpoint SDK (ADR 0013). Byte-identical to crates/xrce and
// endpoints/c/d/rust: WRITE_DATA carries the RFC-1982 sample sequence in the
// 2-byte header field; HEARTBEAT/ACKNACK use the golden control-frame layout
// (session 0x80, stream NONE, msg-seq 1) that endpoints/golden-gen emits --
// the peer only inspects byte[4] (submessage id) and the body at offset 8
// (HEARTBEAT) or 12 (ACKNACK, after the DHEADER), so any stream/seq bytes at [1..4) are accepted, but this layout is the one
// asserted byte-identical against golden_heartbeat_le.bin/golden_acknack_le.bin.
//
// Frame = 8-byte header + body, little-endian:
//   [0]=session [1]=stream [2..4)=seq LE [4]=submsg id [5]=flags
//   [6..8)=body-len LE [8..)=body
//
// ACKNACK_Payload has no @extensibility. With UnannotatedTypes.APPENDABLE
// (default; DDS-XTypes 1.3 7.3.1.2.1.8) the body is a 4-octet DHEADER (= 5)
// followed by the 5 payload octets (first_unacked i16, nack_bitmap[2],
// stream_id), body-len 9, frame 17 octets. With UnannotatedTypes.FINAL (the
// eProsima form) there is no DHEADER: body-len 5, frame 13 octets. The parser
// accepts both forms in either setting. HEARTBEAT_Payload is FINAL in both.

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
    // E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits
    // 1..3 -- DDS-XRCE 1.0 Sec 8.3.5.8.
    public static final int FLAGS_WRITE = 0x01;
    /** Wire size of the BaseObjectRequest preceding the sample (Sec 7.7.8). */
    public static final int BASE_SIZE = 4;
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

    /**
     * Reliable WRITE_DATA frame: the header seq carries the RFC-1982 sample
     * seq, and the submessage body is the BaseObjectRequest (Sec 7.7.8:
     * request_id then object_id, each two octets most significant first)
     * followed by the sample. {@code objectId} names the target DataWriter;
     * the request_id is the sample's own sequence number.
     */
    public static byte[] writeFrame(int seq, int objectId, byte[] sample) {
        int bodyLen = BASE_SIZE + sample.length;
        ByteArrayOutputStream o = new ByteArrayOutputStream(8 + bodyLen);
        o.write(SESSION_NOKEY);
        o.write(STREAM_RELIABLE);
        o.write(seq & 0xFF);
        o.write((seq >> 8) & 0xFF);
        o.write(SM_WRITE_DATA);
        o.write(FLAGS_WRITE);
        o.write(bodyLen & 0xFF);
        o.write((bodyLen >> 8) & 0xFF);
        o.write((seq >> 8) & 0xFF);
        o.write(seq & 0xFF);
        o.write((objectId >> 8) & 0xFF);
        o.write(objectId & 0xFF);
        o.write(sample, 0, sample.length);
        return o.toByteArray();
    }

    private static byte[] ctrlHeader(int submsg, int bodyLen) {
        // golden layout: session, stream=NONE, msg-seq=1, submsg, flags, len
        return new byte[] {
            (byte) SESSION_NOKEY, (byte) STREAM_NONE, 0x01, 0x00,
            (byte) submsg, (byte) FLAGS_CTRL, (byte) bodyLen, 0x00
        };
    }

    public static byte[] heartbeatFrame(Heartbeat h) {
        ByteArrayOutputStream o = new ByteArrayOutputStream(13);
        o.write(ctrlHeader(SM_HEARTBEAT, 5), 0, 8);
        o.write(h.first & 0xFF);
        o.write((h.first >> 8) & 0xFF);
        o.write(h.last & 0xFF);
        o.write((h.last >> 8) & 0xFF);
        o.write(h.stream & 0xFF);
        return o.toByteArray();
    }

    /** ACKNACK frame in the APPENDABLE form (17 octets). */
    public static byte[] acknackFrame(AckNack a) {
        return acknackFrame(a, UnannotatedTypes.APPENDABLE);
    }

    /**
     * ACKNACK frame. {@code APPENDABLE}: DHEADER = 5, body-len 9, 17 octets.
     * {@code FINAL}: no DHEADER, body-len 5, 13 octets.
     */
    public static byte[] acknackFrame(AckNack a, UnannotatedTypes mode) {
        boolean appendable = mode != UnannotatedTypes.FINAL;
        ByteArrayOutputStream o = new ByteArrayOutputStream(17);
        o.write(ctrlHeader(SM_ACKNACK, appendable ? 9 : 5), 0, 8);
        if (appendable) {
            o.write(5);
            o.write(0);
            o.write(0);
            o.write(0);
        }
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
        if (f.length < 8 + BASE_SIZE || (f[4] & 0xFF) != SM_WRITE_DATA) {
            return null;
        }
        int smLen = i16(f, 6);
        if (smLen < BASE_SIZE || 8 + smLen > f.length) {
            return null;
        }
        int seq = i16(f, 2);
        byte[] sample = Arrays.copyOfRange(f, 8 + BASE_SIZE, 8 + smLen);
        return new WriteData(seq, sample);
    }

    /** Parses a control frame by byte[4]=submsg-id, body at offset 8 (peer contract). */
    public static Heartbeat parseHeartbeat(byte[] f) {
        if (f.length < 13 || (f[4] & 0xFF) != SM_HEARTBEAT) {
            return null;
        }
        return new Heartbeat(i16(f, 8), i16(f, 10), f[12] & 0xFF);
    }

    /**
     * Parses an ACKNACK frame: body-len 9 (4-octet DHEADER, payload at offset 12)
     * or FINAL body-len 5 (payload at offset 8). Shorter frames are rejected.
     */
    public static AckNack parseAckNack(byte[] f) {
        if (f.length < 13 || (f[4] & 0xFF) != SM_ACKNACK) {
            return null;
        }
        int off = 8;
        if (i16(f, 6) >= 9) {
            if (f.length < 17) {
                return null;
            }
            off = 12;
        }
        return new AckNack(i16(f, off), f[off + 2] & 0xFF, f[off + 3] & 0xFF, f[off + 4] & 0xFF);
    }

    private static int i16(byte[] b, int off) {
        return (b[off] & 0xFF) | ((b[off + 1] & 0xFF) << 8);
    }
}
