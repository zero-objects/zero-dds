// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// ZdwEndpoint -- XRCE + serial framing for the pure-Java endpoint SDK
// (ADR 0013). Mirrors endpoints/c/src/zerodds_endpoint.c byte-for-byte: XRCE
// WRITE_DATA/DATA, HEARTBEAT/ACKNACK, Annex-C serial HDLC framing
// (byte-stuffing + CRC-16-CCITT-FALSE). Pure Java 8, no JNI.

import java.io.ByteArrayOutputStream;

public final class ZdwEndpoint {

    public static final int SESSION_NOKEY = 0x80;
    public static final int STREAM_BEST_EFFORT = 0x01;

    private static final int SM_WRITE_DATA = 0x07;
    private static final int SM_DATA = 0x09;
    private static final int SM_ACKNACK = 0x0A;
    private static final int SM_HEARTBEAT = 0x0B;
    private static final int WRITE_FLAGS = 0x03;
    private static final int FLAG = 0x7E;
    private static final int ESC = 0x7D;
    private static final int STUFF = 0x20;

    private ZdwEndpoint() {}

    public static byte[] xrceWriteFrame(int session, int stream, int seq, byte[] sample) {
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        o.write(session & 0xFF);
        o.write(stream & 0xFF);
        o.write(seq & 0xFF);
        o.write((seq >> 8) & 0xFF);
        o.write(SM_WRITE_DATA);
        o.write(WRITE_FLAGS);
        o.write(sample.length & 0xFF);
        o.write((sample.length >> 8) & 0xFF);
        o.write(sample, 0, sample.length);
        return o.toByteArray();
    }

    public static byte[] xrceReadBody(byte[] frame) {
        if (frame.length < 8 || (frame[0] & 0xFF) < 0x80) {
            throw new IllegalArgumentException("not a data frame");
        }
        int id = frame[4] & 0xFF;
        if (id != SM_WRITE_DATA && id != SM_DATA) {
            throw new IllegalArgumentException("not WRITE_DATA/DATA");
        }
        int smLen = (frame[6] & 0xFF) | ((frame[7] & 0xFF) << 8);
        byte[] body = new byte[smLen];
        System.arraycopy(frame, 8, body, 0, smLen);
        return body;
    }

    public static int crc16CcittFalse(byte[] data) {
        int crc = 0xFFFF;
        for (byte value : data) {
            crc ^= (value & 0xFF) << 8;
            for (int i = 0; i < 8; i++) {
                if ((crc & 0x8000) != 0) crc = ((crc << 1) ^ 0x1021) & 0xFFFF;
                else crc = (crc << 1) & 0xFFFF;
            }
        }
        return crc & 0xFFFF;
    }

    private static void stuff(ByteArrayOutputStream o, byte[] data) {
        for (byte value : data) {
            int b = value & 0xFF;
            if (b == FLAG || b == ESC) {
                o.write(ESC);
                o.write(b ^ STUFF);
            } else {
                o.write(b);
            }
        }
    }

    public static byte[] serialFrame(byte[] payload) {
        int crc = crc16CcittFalse(payload);
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        o.write(FLAG);
        stuff(o, payload);
        stuff(o, new byte[] { (byte) ((crc >> 8) & 0xFF), (byte) (crc & 0xFF) });
        o.write(FLAG);
        return o.toByteArray();
    }

    public static byte[] serialDeframe(byte[] frame) {
        int i = (frame.length >= 1 && (frame[0] & 0xFF) == FLAG) ? 1 : 0;
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        while (i < frame.length && (frame[i] & 0xFF) != FLAG) {
            int b = frame[i] & 0xFF;
            if (b == ESC) {
                i++;
                b = (frame[i] & 0xFF) ^ STUFF;
            }
            o.write(b);
            i++;
        }
        byte[] out = o.toByteArray();
        if (out.length < 2) throw new IllegalArgumentException("short frame");
        int crc = crc16CcittFalse(java.util.Arrays.copyOf(out, out.length - 2));
        if ((out[out.length - 2] & 0xFF) != ((crc >> 8) & 0xFF)
                || (out[out.length - 1] & 0xFF) != (crc & 0xFF)) {
            throw new IllegalArgumentException("crc mismatch");
        }
        return java.util.Arrays.copyOf(out, out.length - 2);
    }

    public static byte[] acknackFrame(int session, int stream, int seq, int firstUnacked,
                                      int nackLo, int nackHi, int payloadStream) {
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        o.write(session & 0xFF); o.write(stream & 0xFF);
        o.write(seq & 0xFF); o.write((seq >> 8) & 0xFF);
        o.write(SM_ACKNACK); o.write(0x01); o.write(5); o.write(0);
        o.write(firstUnacked & 0xFF); o.write((firstUnacked >> 8) & 0xFF);
        o.write(nackLo & 0xFF); o.write(nackHi & 0xFF); o.write(payloadStream & 0xFF);
        return o.toByteArray();
    }

    /** Returns {first, last, stream} from a HEARTBEAT frame. */
    public static int[] heartbeatRead(byte[] frame) {
        if (frame.length < 13 || (frame[4] & 0xFF) != SM_HEARTBEAT) {
            throw new IllegalArgumentException("not a HEARTBEAT");
        }
        return new int[] { i16(frame, 8), i16(frame, 10), frame[12] & 0xFF };
    }

    private static int i16(byte[] b, int off) {
        int v = (b[off] & 0xFF) | ((b[off + 1] & 0xFF) << 8);
        return (v & 0x8000) != 0 ? v - 0x10000 : v;
    }
}
