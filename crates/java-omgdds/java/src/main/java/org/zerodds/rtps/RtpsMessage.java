// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.util.ArrayList;
import java.util.List;

/**
 * RTPS datagram framing (§8.3.3, §8.3.4, §8.3.7). Encodes the 20-byte RTPS
 * header plus submessages (INFO_TS, INFO_DST, DATA, HEARTBEAT, ACKNACK) and
 * decodes an inbound datagram into its DATA/HEARTBEAT/ACKNACK submessages.
 * All bodies are emitted little-endian (E flag set). Ported from
 * {@code crates/rtps/src/{header,submessages,datagram,message_builder}.rs}.
 */
public final class RtpsMessage {
    private RtpsMessage() {}

    // ---------- encode ----------

    /** Builder that accumulates submessages after an RTPS header. */
    public static final class Builder {
        private final Wire w = new Wire(256);

        public Builder(byte[] guidPrefix12) {
            w.bytes(Rtps.MAGIC);
            w.u8(Rtps.VERSION_MAJOR);
            w.u8(Rtps.VERSION_MINOR);
            w.bytes(Rtps.VENDOR_ID);
            w.bytes(guidPrefix12, 0, 12);
        }

        private void submessage(int id, int flags, byte[] body) {
            w.u8(id);
            w.u8(flags | Rtps.FLAG_E);
            w.u16le(body.length);
            w.bytes(body);
        }

        /** INFO_TS with a Time_t (seconds + 2^-32 fraction). */
        public Builder infoTimestamp(int seconds, long fraction) {
            Wire b = new Wire(8);
            b.u32le(seconds & 0xFFFFFFFFL);
            b.u32le(fraction);
            submessage(Rtps.SM_INFO_TS, 0, b.toBytes());
            return this;
        }

        /** INFO_DST setting the destination GuidPrefix (§8.3.7.7). */
        public Builder infoDestination(byte[] guidPrefix12) {
            submessage(Rtps.SM_INFO_DST, 0, guidPrefix12.clone());
            return this;
        }

        /**
         * DATA submessage (§8.3.7.2). {@code serializedPayload} already carries
         * its 4-byte encapsulation header. {@code inlineQos} may be null.
         */
        public Builder data(byte[] readerId4, byte[] writerId4, long writerSn,
                            ParameterList inlineQos, byte[] serializedPayload, boolean keyFlag) {
            Wire b = new Wire(24 + serializedPayload.length);
            b.u16le(0); // extraFlags
            b.u16le(16); // octetsToInlineQos (constant: readerId+writerId+writerSN)
            b.bytes(readerId4);
            b.bytes(writerId4);
            writeSn(b, writerSn);
            int flags = Rtps.DATA_FLAG_DATA;
            if (inlineQos != null) {
                b.bytes(inlineQos.toBytesLe());
                flags |= Rtps.DATA_FLAG_INLINE_QOS;
            }
            if (keyFlag) {
                flags |= Rtps.DATA_FLAG_KEY;
            }
            b.bytes(serializedPayload);
            submessage(Rtps.SM_DATA, flags, b.toBytes());
            return this;
        }

        /** HEARTBEAT submessage (§8.3.7.5). */
        public Builder heartbeat(byte[] readerId4, byte[] writerId4, long firstSn,
                                long lastSn, int count, boolean finalFlag) {
            Wire b = new Wire(28);
            b.bytes(readerId4);
            b.bytes(writerId4);
            writeSn(b, firstSn);
            writeSn(b, lastSn);
            b.u32le(count & 0xFFFFFFFFL);
            int flags = finalFlag ? Rtps.HB_FLAG_FINAL : 0;
            submessage(Rtps.SM_HEARTBEAT, flags, b.toBytes());
            return this;
        }

        /**
         * ACKNACK submessage (§8.3.7.1). {@code numBits} bits from {@code base}
         * are all NACKed (set) — pulls samples [base .. base+numBits-1] from a
         * reliable writer. {@code numBits==0} is a pure positive ACK.
         */
        public Builder ackNack(byte[] readerId4, byte[] writerId4, long base,
                              int numBits, int count, boolean finalFlag) {
            int nb = Math.max(0, Math.min(256, numBits));
            int words = (nb + 31) / 32;
            Wire b = new Wire(28 + words * 4);
            b.bytes(readerId4);
            b.bytes(writerId4);
            writeSn(b, base); // reader_sn_state bitmap base
            b.u32le(nb & 0xFFFFFFFFL);
            for (int wi = 0; wi < words; wi++) {
                long word = 0;
                for (int bit = 0; bit < 32; bit++) {
                    int idx = wi * 32 + bit;
                    if (idx < nb) {
                        word |= (1L << (31 - bit)); // RTPS: bit 0 = MSB
                    }
                }
                b.u32le(word);
            }
            b.u32le(count & 0xFFFFFFFFL);
            int flags = finalFlag ? Rtps.ACKNACK_FLAG_FINAL : 0;
            submessage(Rtps.SM_ACKNACK, flags, b.toBytes());
            return this;
        }

        public byte[] toBytes() {
            return w.toBytes();
        }
    }

    private static void writeSn(Wire w, long sn) {
        int high = (int) (sn >> 32);
        long low = sn & 0xFFFFFFFFL;
        w.u32le(high & 0xFFFFFFFFL);
        w.u32le(low);
    }

    private static long readSnLe(byte[] b, int off) {
        int high = (int) Wire.u32le(b, off);
        long low = Wire.u32le(b, off + 4);
        return ((long) high << 32) | low;
    }

    // ---------- decode ----------

    public static final class Header {
        public byte[] guidPrefix; // 12 bytes
        public byte[] vendorId; // 2 bytes
    }

    public static final class Data {
        public byte[] destPrefix; // from INFO_DST, or null
        public byte[] readerId; // 4 bytes
        public byte[] writerId; // 4 bytes
        public long writerSn;
        public boolean keyFlag;
        public ParameterList inlineQos; // may be null
        public byte[] serializedPayload; // incl. encapsulation header
    }

    public static final class Heartbeat {
        public byte[] readerId; // 4
        public byte[] writerId; // 4
        public long firstSn;
        public long lastSn;
        public int count;
        public boolean finalFlag;
    }

    public static final class Parsed {
        public final Header header = new Header();
        public final List<Data> data = new ArrayList<>();
        public final List<Heartbeat> heartbeats = new ArrayList<>();
    }

    /** Decode an inbound datagram; collects DATA submessages with their INFO_DST context. */
    public static Parsed decode(byte[] b, int length) {
        Parsed out = new Parsed();
        if (length < 20 || b[0] != 'R' || b[1] != 'T' || b[2] != 'P' || b[3] != 'S') {
            return null;
        }
        out.header.guidPrefix = Wire.slice(b, 8, 12);
        out.header.vendorId = Wire.slice(b, 6, 2);
        int pos = 20;
        byte[] curDest = null;
        while (pos + 4 <= length) {
            int id = b[pos] & 0xFF;
            int flags = b[pos + 1] & 0xFF;
            boolean le = (flags & Rtps.FLAG_E) != 0;
            int octets = le ? Wire.u16le(b, pos + 2) : Wire.u16be(b, pos + 2);
            int bodyStart = pos + 4;
            int bodyLen = octets;
            if (octets == 0 && id != Rtps.SM_PAD) {
                bodyLen = length - bodyStart; // last-submessage marker
            }
            if (bodyStart + bodyLen > length) {
                break;
            }
            switch (id) {
                case Rtps.SM_INFO_DST:
                    if (bodyLen >= 12) {
                        curDest = Wire.slice(b, bodyStart, 12);
                    }
                    break;
                case Rtps.SM_DATA:
                    Data d = decodeData(b, bodyStart, bodyLen, flags, le);
                    if (d != null) {
                        d.destPrefix = curDest;
                        out.data.add(d);
                    }
                    break;
                case Rtps.SM_HEARTBEAT:
                    Heartbeat hb = decodeHeartbeat(b, bodyStart, bodyLen, flags, le);
                    if (hb != null) {
                        out.heartbeats.add(hb);
                    }
                    break;
                default:
                    // INFO_TS / HEARTBEAT / ACKNACK / others: skipped for this path.
                    break;
            }
            pos = bodyStart + bodyLen;
        }
        return out;
    }

    private static Data decodeData(byte[] b, int off, int len, int flags, boolean le) {
        if (len < 20) {
            return null;
        }
        Data d = new Data();
        int pos = off;
        // extraFlags(2)
        pos += 2;
        int octetsToInlineQos = le ? Wire.u16le(b, pos) : Wire.u16be(b, pos);
        pos += 2;
        int idStart = pos; // start of readerId, base for octetsToInlineQos
        d.readerId = Wire.slice(b, pos, 4);
        pos += 4;
        d.writerId = Wire.slice(b, pos, 4);
        pos += 4;
        d.writerSn = le ? readSnLe(b, pos) : readSnBe(b, pos);
        pos += 8;
        // Honor octetsToInlineQos: it measures from idStart to inlineQos/payload.
        int afterFixed = idStart + octetsToInlineQos;
        if (afterFixed > pos && afterFixed <= off + len) {
            pos = afterFixed;
        }
        d.keyFlag = (flags & Rtps.DATA_FLAG_KEY) != 0;
        int end = off + len;
        if ((flags & Rtps.DATA_FLAG_INLINE_QOS) != 0) {
            d.inlineQos = ParameterList.fromBytesLe(b, pos, end);
            pos += inlineQosConsumed(b, pos, end);
        }
        d.serializedPayload = Wire.slice(b, pos, end - pos);
        return d;
    }

    /** Byte length of the inline-QoS ParameterList incl. its sentinel. */
    private static int inlineQosConsumed(byte[] b, int off, int end) {
        int pos = off;
        while (pos + 4 <= end) {
            int id = Wire.u16le(b, pos);
            int len = Wire.u16le(b, pos + 2);
            pos += 4;
            if (id == Rtps.PID_SENTINEL) {
                return pos - off;
            }
            if (pos + len > end) {
                break;
            }
            pos += len;
        }
        return pos - off;
    }

    private static long readSnBe(byte[] b, int off) {
        int high = (int) Wire.u32be(b, off);
        long low = Wire.u32be(b, off + 4);
        return ((long) high << 32) | low;
    }

    private static Heartbeat decodeHeartbeat(byte[] b, int off, int len, int flags, boolean le) {
        if (len < 28) {
            return null;
        }
        Heartbeat hb = new Heartbeat();
        int pos = off;
        hb.readerId = Wire.slice(b, pos, 4);
        pos += 4;
        hb.writerId = Wire.slice(b, pos, 4);
        pos += 4;
        hb.firstSn = le ? readSnLe(b, pos) : readSnBe(b, pos);
        pos += 8;
        hb.lastSn = le ? readSnLe(b, pos) : readSnBe(b, pos);
        pos += 8;
        hb.count = le ? (int) Wire.u32le(b, pos) : (int) Wire.u32be(b, pos);
        hb.finalFlag = (flags & Rtps.HB_FLAG_FINAL) != 0;
        return hb;
    }
}
