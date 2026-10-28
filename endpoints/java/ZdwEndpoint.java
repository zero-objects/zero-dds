// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// ZdwEndpoint -- XRCE + serial framing for the pure-Java endpoint SDK
// (ADR 0013). Mirrors endpoints/c/src/zerodds_endpoint.c byte-for-byte: XRCE
// WRITE_DATA/DATA, HEARTBEAT/ACKNACK, DDS-XRCE 1.0 Annex C serial framing
// (addresses, length, byte-stuffing, RFC 1662 FCS-16). Pure Java 8, no JNI.

import java.io.ByteArrayOutputStream;

public final class ZdwEndpoint {

    /**
     * Extensibility of the DDS-XRCE 1.0 Annex A types without @extensibility
     * (DDS-XTypes 1.3 7.3.1.2.1.8: appendable). Own type of this file, so the
     * file compiles alone; the reliable classes of org.zerodds.endpoint have
     * theirs of the same name and values.
     */
    public enum UnannotatedTypes { APPENDABLE, FINAL }

    public static final int SESSION_NOKEY = 0x80;
    public static final int STREAM_BEST_EFFORT = 0x01;

    private static final int SM_WRITE_DATA = 0x07;
    private static final int SM_DATA = 0x09;
    private static final int SM_ACKNACK = 0x0A;
    private static final int SM_HEARTBEAT = 0x0B;
    // E flag (little-endian body) + DataFormat FORMAT_DATA (0b000) in bits
    // 1..3 -- DDS-XRCE 1.0 Sec 8.3.5.8. FORMAT_SAMPLE (0b001, i.e. 0x03) would
    // additionally require a SampleInfo in the body, which this profile does
    // not carry.
    private static final int WRITE_FLAGS = 0x01;
    /** Wire size of the BaseObjectRequest preceding the sample (Sec 7.7.8). */
    public static final int BASE_SIZE = 4;
    /** OBJK_DATAWRITER -- the ObjectKind of a DataWriter (Sec 7.2.2). */
    public static final int OBJK_DATAWRITER = 0x05;

    /** An ObjectId (Sec 7.2.1) from a 12-bit raw id and a 4-bit ObjectKind. */
    public static int objectId(int rawId, int kind) {
        return ((rawId & 0x0FFF) << 4) | (kind & 0x0F);
    }
    private static final int FLAG = 0x7E;
    private static final int ESC = 0x7D;
    private static final int STUFF = 0x20;
    private static final int SERIAL_HEADER_LEN = 4;

    private ZdwEndpoint() {}

    /**
     * An XRCE WRITE_DATA frame. The submessage body is the BaseObjectRequest
     * (Sec 7.7.8: request_id then object_id, each two octets most significant
     * first) followed by the sample. {@code objectId} names the target
     * DataWriter -- without it no XRCE agent can route the sample. The
     * request_id is the message's own {@code seq}, which Sec 7.7.8 only
     * requires to be unique per stream.
     */
    public static byte[] xrceWriteFrame(int session, int stream, int seq, int objectId,
                                        byte[] sample) {
        // The 16-bit submessage_length field cannot describe a body larger
        // than 0xFFFF; refuse rather than truncate the length while writing the
        // full payload (mirrors the C SDK's rejection).
        int bodyLen = BASE_SIZE + sample.length;
        if (bodyLen > 0xFFFF) {
            throw new IllegalArgumentException("sample exceeds 16-bit submessage_length");
        }
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        o.write(session & 0xFF);
        o.write(stream & 0xFF);
        o.write(seq & 0xFF);
        o.write((seq >> 8) & 0xFF);
        o.write(SM_WRITE_DATA);
        o.write(WRITE_FLAGS);
        o.write(bodyLen & 0xFF);
        o.write((bodyLen >> 8) & 0xFF);
        o.write((seq >> 8) & 0xFF);
        o.write(seq & 0xFF);
        o.write((objectId >> 8) & 0xFF);
        o.write(objectId & 0xFF);
        o.write(sample, 0, sample.length);
        return o.toByteArray();
    }

    // Frame layout: [session, stream, seq_lo, seq_hi, sm_id, flags, len_lo,
    // len_hi], then the 4-octet BaseObjectRequest (Sec 7.7.8), then the sample.
    // The 16-bit little-endian submessage_length (bytes 6..7) bounds the body
    // exactly: the sample is copied out as frame[12 .. 8+smLen], so trailing
    // padding or an appended submessage is never folded into it. A frame whose
    // declared length runs past the datagram (truncation / wrong length), or
    // whose body is too short to hold the BaseObjectRequest, is rejected --
    // never read out of bounds, never handed back truncated.
    public static byte[] xrceReadBody(byte[] frame) {
        int smLen = checkedBodyLength(frame);
        byte[] body = new byte[smLen - BASE_SIZE];
        System.arraycopy(frame, 8 + BASE_SIZE, body, 0, body.length);
        return body;
    }

    /** {@code {request_id, object_id}} of a data frame (Sec 7.7.8). */
    public static int[] xrceFrameBase(byte[] frame) {
        checkedBodyLength(frame);
        return new int[] {
            ((frame[8] & 0xFF) << 8) | (frame[9] & 0xFF),
            ((frame[10] & 0xFF) << 8) | (frame[11] & 0xFF),
        };
    }

    private static int checkedBodyLength(byte[] frame) {
        if (frame.length < 8 + BASE_SIZE || (frame[0] & 0xFF) < 0x80) {
            throw new IllegalArgumentException("not a data frame");
        }
        int id = frame[4] & 0xFF;
        if (id != SM_WRITE_DATA && id != SM_DATA) {
            throw new IllegalArgumentException("not WRITE_DATA/DATA");
        }
        int smLen = (frame[6] & 0xFF) | ((frame[7] & 0xFF) << 8);
        if (8 + smLen > frame.length) {
            throw new IllegalArgumentException("submessage_length runs past frame");
        }
        if (smLen < BASE_SIZE) {
            throw new IllegalArgumentException("body too short for a BaseObjectRequest");
        }
        return smLen;
    }

    /** FCS-16 of RFC 1662 (reflected 0x8408, init 0xFFFF, complemented); 0x906E for "123456789". */
    public static int fcs16(byte[] data) {
        return fcs16(data, 0, data.length);
    }

    private static int fcs16(byte[] data, int from, int to) {
        int crc = 0xFFFF;
        for (int j = from; j < to; j++) {
            crc ^= data[j] & 0xFF;
            for (int i = 0; i < 8; i++) {
                crc = ((crc & 1) != 0) ? ((crc >>> 1) ^ 0x8408) : (crc >>> 1);
            }
        }
        return (~crc) & 0xFFFF;
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

    /** A frame of DDS-XRCE 1.0 Annex C.1.1, taken apart. */
    public static final class SerialFrame {
        public final int source;
        public final int remote;
        public final byte[] payload;

        SerialFrame(int source, int remote, byte[] payload) {
            this.source = source;
            this.remote = remote;
            this.payload = payload;
        }
    }

    /**
     * Frames {@code payload} from {@code source} to {@code remote} (Annex C.1.1):
     * 7E | source | remote | length LE | payload | FCS-16 LE, stuffed after the flag.
     */
    public static byte[] serialFrame(int source, int remote, byte[] payload) {
        if (payload.length > 0xFFFF) throw new IllegalArgumentException("payload over 65535 bytes");
        byte[] raw = new byte[SERIAL_HEADER_LEN + payload.length + 2];
        raw[0] = (byte) source;
        raw[1] = (byte) remote;
        raw[2] = (byte) (payload.length & 0xFF);
        raw[3] = (byte) ((payload.length >> 8) & 0xFF);
        System.arraycopy(payload, 0, raw, SERIAL_HEADER_LEN, payload.length);
        int crc = fcs16(raw, 0, SERIAL_HEADER_LEN + payload.length);
        raw[raw.length - 2] = (byte) (crc & 0xFF);
        raw[raw.length - 1] = (byte) ((crc >> 8) & 0xFF);
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        o.write(FLAG);
        stuff(o, raw);
        return o.toByteArray();
    }

    /**
     * De-frames the frame at the start of {@code frame} (beginning with its 0x7E). Refuses a
     * wrong frameCheck, a bad escape, a 0x7E before the frame is complete (Annex C.1.3) and a
     * frame that ends early.
     */
    public static SerialFrame serialDeframe(byte[] frame) {
        if (frame.length < 1 || (frame[0] & 0xFF) != FLAG) throw new IllegalArgumentException("no BEGIN_FLAG");
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        int total = SERIAL_HEADER_LEN + 2;
        int got = 0;
        byte[] head = new byte[SERIAL_HEADER_LEN];
        for (int i = 1; i < frame.length && got < total; i++) {
            int b = frame[i] & 0xFF;
            if (b == FLAG) throw new IllegalArgumentException("BEGIN_FLAG inside the frame");
            if (b == ESC) {
                i++;
                if (i >= frame.length) throw new IllegalArgumentException("dangling escape");
                b = (frame[i] & 0xFF) ^ STUFF;
                if (b != FLAG && b != ESC) throw new IllegalArgumentException("invalid escape");
            }
            if (got < SERIAL_HEADER_LEN) {
                head[got] = (byte) b;
                if (got + 1 == SERIAL_HEADER_LEN) {
                    total = SERIAL_HEADER_LEN + ((head[2] & 0xFF) | ((head[3] & 0xFF) << 8)) + 2;
                }
            }
            o.write(b);
            got++;
        }
        if (got < total) throw new IllegalArgumentException("short frame");
        byte[] raw = o.toByteArray();
        int crc = fcs16(raw, 0, raw.length - 2);
        if ((raw[raw.length - 2] & 0xFF) != (crc & 0xFF)
                || (raw[raw.length - 1] & 0xFF) != ((crc >> 8) & 0xFF)) {
            throw new IllegalArgumentException("frameCheck mismatch");
        }
        return new SerialFrame(head[0] & 0xFF, head[1] & 0xFF,
                java.util.Arrays.copyOfRange(raw, SERIAL_HEADER_LEN, raw.length - 2));
    }

    /** ACKNACK frame in the APPENDABLE form, 17 octets. */
    public static byte[] acknackFrame(int session, int stream, int seq, int firstUnacked,
                                      int nackLo, int nackHi, int payloadStream) {
        return acknackFrame(session, stream, seq, firstUnacked, nackLo, nackHi, payloadStream,
                UnannotatedTypes.APPENDABLE);
    }

    /**
     * ACKNACK frame. ACKNACK_Payload has no @extensibility. APPENDABLE
     * (DDS-XTypes 1.3 7.3.1.2.1.8): 9-octet body with a 4-octet DHEADER (= 5)
     * before the 5 payload octets, 17-octet frame. FINAL: 5-octet body, 13-octet frame.
     */
    public static byte[] acknackFrame(int session, int stream, int seq, int firstUnacked,
                                      int nackLo, int nackHi, int payloadStream,
                                      UnannotatedTypes mode) {
        boolean appendable = mode != UnannotatedTypes.FINAL;
        ByteArrayOutputStream o = new ByteArrayOutputStream();
        o.write(session & 0xFF); o.write(stream & 0xFF);
        o.write(seq & 0xFF); o.write((seq >> 8) & 0xFF);
        o.write(SM_ACKNACK); o.write(0x01); o.write(appendable ? 9 : 5); o.write(0);
        if (appendable) { o.write(5); o.write(0); o.write(0); o.write(0); } // DHEADER = 5
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
