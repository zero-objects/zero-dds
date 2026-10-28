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
    private static final int SM_NACK_FRAG = 0x12;
    private static final int SM_HEARTBEAT_FRAG = 0x13;

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
            if (body.length > 0xffff) {
                throw new IllegalArgumentException("RTPS submessage body exceeds u16 length");
            }
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
            // A DATA without serialized data is no data and no key: it carries only its
            // sequence number and inline QoS (DDSI-RTPS 2.5 Table 9.22, the end of a coherent set).
            int flags = keyFlag ? Rtps.DATA_FLAG_KEY
                    : serializedPayload.length == 0 ? 0 : Rtps.DATA_FLAG_DATA;
            if (inlineQos != null) {
                b.bytes(inlineQos.toBytesLe());
                flags |= Rtps.DATA_FLAG_INLINE_QOS;
            }
            b.bytes(serializedPayload);
            submessage(Rtps.SM_DATA, flags, b.toBytes());
            return this;
        }

        /** HEARTBEAT submessage (§8.3.7.5). */
        public Builder heartbeat(byte[] readerId4, byte[] writerId4, long firstSn,
                                long lastSn, int count, boolean finalFlag) {
            return heartbeat(readerId4, writerId4, firstSn, lastSn, count, finalFlag, false);
        }

        /** HEARTBEAT with the L flag when {@code liveliness}: the writer asserted its liveliness. */
        public Builder heartbeat(byte[] readerId4, byte[] writerId4, long firstSn,
                                long lastSn, int count, boolean finalFlag, boolean liveliness) {
            Wire b = new Wire(28);
            b.bytes(readerId4);
            b.bytes(writerId4);
            writeSn(b, firstSn);
            writeSn(b, lastSn);
            b.u32le(count & 0xFFFFFFFFL);
            int flags = (finalFlag ? Rtps.HB_FLAG_FINAL : 0) | (liveliness ? Rtps.HB_FLAG_LIVELINESS : 0);
            submessage(Rtps.SM_HEARTBEAT, flags, b.toBytes());
            return this;
        }

        /**
         * HEARTBEAT with the GroupInfoFlag (DDSI-RTPS 2.5 8.3.7.5, 9.4.5.7): the group sequence numbers of
         * the Publisher after the count and, unless {@code writerSet} is {@code null}, the writerSet and the
         * secureWriterSet of Table 8.47 (GroupDigest, four octets each; the secure one is zero).
         */
        public Builder heartbeatGroup(byte[] readerId4, byte[] writerId4, long firstSn, long lastSn,
                                      int count, boolean finalFlag, long currentGsn, long firstGsn,
                                      long lastGsn, byte[] writerSet) {
            Wire b = new Wire(60);
            b.bytes(readerId4);
            b.bytes(writerId4);
            writeSn(b, firstSn);
            writeSn(b, lastSn);
            b.u32le(count & 0xFFFFFFFFL);
            writeSn(b, currentGsn);
            writeSn(b, firstGsn);
            writeSn(b, lastGsn);
            if (writerSet != null) {
                b.bytes(writerSet);
                b.bytes(new byte[4]);
            }
            int flags = (finalFlag ? Rtps.HB_FLAG_FINAL : 0) | Rtps.HB_FLAG_GROUP_INFO;
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
            requireEntityIds(readerId4, writerId4);
            if (base < 1 || numBits < 0 || numBits > 256 || base > Long.MAX_VALUE - Math.max(0, numBits - 1)) {
                throw new IllegalArgumentException("invalid ACKNACK sequence-number set");
            }
            int nb = numBits;
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

        /** Contiguous GAP [gapStart, gapListBase), with an empty bitmap. */
        public Builder gap(byte[] readerId4, byte[] writerId4, long gapStart, long gapListBase) {
            requireEntityIds(readerId4, writerId4);
            if (gapStart < 1 || gapListBase < gapStart) {
                throw new IllegalArgumentException("invalid GAP range");
            }
            Wire b = new Wire(28);
            b.bytes(readerId4);
            b.bytes(writerId4);
            writeSn(b, gapStart);
            writeSn(b, gapListBase);
            b.u32le(0);
            submessage(Rtps.SM_GAP, 0, b.toBytes());
            return this;
        }

        /** DATA_FRAG payload is the declared fragment span, including encapsulation only in fragment 1. */
        public Builder dataFrag(byte[] readerId4, byte[] writerId4, long writerSn,
                                long fragmentStartingNum, int fragmentsInSubmessage,
                                int fragmentSize, long sampleSize, ParameterList inlineQos,
                                byte[] serializedPayload, boolean keyFlag) {
            requireEntityIds(readerId4, writerId4);
            long expected = fragmentPayloadSize(fragmentStartingNum, fragmentsInSubmessage,
                                                 fragmentSize, sampleSize);
            if (writerSn < 1 || expected < 0 || serializedPayload == null
                    || serializedPayload.length != expected || expected > 0xffff - 32) {
                throw new IllegalArgumentException("invalid DATA_FRAG bounds or payload size");
            }
            Wire b = new Wire(32 + serializedPayload.length);
            b.u16le(0);
            b.u16le(28);
            b.bytes(readerId4);
            b.bytes(writerId4);
            writeSn(b, writerSn);
            b.u32le(fragmentStartingNum);
            b.u16le(fragmentsInSubmessage);
            b.u16le(fragmentSize);
            b.u32le(sampleSize);
            int flags = keyFlag ? DATA_FRAG_KEY_FLAG : 0;
            if (inlineQos != null) {
                flags |= Rtps.DATA_FLAG_INLINE_QOS;
                b.bytes(inlineQos.toBytesLe());
            }
            b.bytes(serializedPayload);
            while ((b.position() & 3) != 0) b.u8(0);
            submessage(Rtps.SM_DATA_FRAG, flags, b.toBytes());
            return this;
        }

        /** Request a contiguous missing-fragment bitmap (at most 256 bits). */
        public Builder nackFrag(byte[] readerId4, byte[] writerId4, long writerSn,
                                long bitmapBase, int numBits, int count) {
            requireEntityIds(readerId4, writerId4);
            if (writerSn < 1 || !validFragmentSet(bitmapBase, numBits)) {
                throw new IllegalArgumentException("invalid NACK_FRAG sequence or fragment set");
            }
            Wire b = new Wire(28 + 4 * ((numBits + 31) / 32));
            b.bytes(readerId4); b.bytes(writerId4); writeSn(b, writerSn);
            b.u32le(bitmapBase); b.u32le(numBits);
            for (int bit = 0; bit < numBits; bit += 32) {
                int used = Math.min(32, numBits - bit);
                b.u32le((0xffff_ffffL << (32 - used)) & 0xffff_ffffL);
            }
            b.u32le(count & 0xffff_ffffL);
            submessage(SM_NACK_FRAG, 0, b.toBytes());
            return this;
        }

        public Builder heartbeatFrag(byte[] readerId4, byte[] writerId4, long writerSn,
                                     long lastFragmentNum, int count) {
            requireEntityIds(readerId4, writerId4);
            if (writerSn < 1 || lastFragmentNum < 1 || lastFragmentNum > 0xffff_ffffL) {
                throw new IllegalArgumentException("invalid HEARTBEAT_FRAG sequence or last fragment");
            }
            Wire b = new Wire(24);
            b.bytes(readerId4); b.bytes(writerId4); writeSn(b, writerSn);
            b.u32le(lastFragmentNum); b.u32le(count & 0xffff_ffffL);
            submessage(SM_HEARTBEAT_FRAG, 0, b.toBytes());
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

    private static void requireEntityIds(byte[] reader, byte[] writer) {
        if (reader == null || writer == null || reader.length != 4 || writer.length != 4) {
            throw new IllegalArgumentException("entity IDs must contain exactly four bytes");
        }
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
        /** The INFO_TS timestamp in ms since the epoch that preceded the DATA, or -1. */
        public long sourceTimestampMillis = -1;
    }

    public static final class Heartbeat {
        public byte[] destPrefix;
        public byte[] readerId; // 4
        public byte[] writerId; // 4
        public long firstSn;
        public long lastSn;
        public int count;
        public boolean finalFlag;
        /** The L flag: the writer asserted its liveliness manually (DDSI-RTPS 2.5 8.3.7.5). */
        public boolean livelinessFlag;
        /** The G flag: the group sequence numbers below are present (DDSI-RTPS 2.5 9.4.5.7). */
        public boolean groupInfo;
        public long currentGsn;
        public long firstGsn;
        public long lastGsn;
        /** writerSet (GroupDigest, four octets), or {@code null} if the HEARTBEAT carries none. */
        public byte[] writerSet;
    }

    public static final class Parsed {
        public final Header header = new Header();
        public final List<Data> data = new ArrayList<>();
        public final List<Heartbeat> heartbeats = new ArrayList<>();
        public final List<AckNack> ackNacks = new ArrayList<>();
        public final List<Gap> gaps = new ArrayList<>();
        public final List<DataFragment> dataFragments = new ArrayList<>();
        public final List<NackFragment> nackFragments = new ArrayList<>();
        public final List<HeartbeatFragment> heartbeatFragments = new ArrayList<>();
    }

    public static final class NackFragment {
        public byte[] destPrefix, readerId, writerId;
        public long writerSn, bitmapBase;
        public long[] bitmap;
        public int numBits, count;
    }

    public static final class HeartbeatFragment {
        public byte[] destPrefix, readerId, writerId;
        public long writerSn, lastFragmentNum;
        public int count;
    }

    // DATA_FRAG's key flag has its own position, independent of DATA D/K.
    private static final int DATA_FRAG_KEY_FLAG = 0x04;

    public static final class DataFragment {
        public byte[] destPrefix, readerId, writerId;
        public long writerSn, fragmentStartingNum, sampleSize;
        public int fragmentsInSubmessage, fragmentSize;
        public ParameterList inlineQos;
        public byte[] serializedPayload;
        public boolean keyFlag;
        /** The INFO_TS timestamp in ms since the epoch that preceded the DATA_FRAG, or -1. */
        public long sourceTimestampMillis = -1;
    }

    public static final class AckNack {
        public byte[] destPrefix, readerId, writerId;
        public long base;
        /** Unsigned 32-bit words, MSB-first bit numbering. */
        public long[] bitmap;
        public int numBits, count;
        public boolean finalFlag;
    }

    public static final class Gap {
        public byte[] destPrefix, readerId, writerId;
        public long gapStart, gapListBase;
        public long[] bitmap;
        public int numBits;
    }

    /** Decode an inbound datagram; collects DATA submessages with their INFO_DST context. */
    public static Parsed decode(byte[] b, int length) {
        Parsed out = new Parsed();
        if (b == null || length < 20 || length > b.length
                || b[0] != 'R' || b[1] != 'T' || b[2] != 'P' || b[3] != 'S') {
            return null;
        }
        out.header.guidPrefix = Wire.slice(b, 8, 12);
        out.header.vendorId = Wire.slice(b, 6, 2);
        int pos = 20;
        byte[] curDest = null;
        long curTimestamp = -1;
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
                case Rtps.SM_INFO_TS:
                    // RTPS 2.5 §8.3.7.9: the Time_t (seconds, fraction of 2^-32 s) is absent
                    // when the InvalidateFlag is set.
                    if ((flags & Rtps.INFO_TS_FLAG_INVALIDATE) != 0) {
                        curTimestamp = -1;
                    } else if (bodyLen >= 8) {
                        long seconds = le ? Wire.u32le(b, bodyStart) : Wire.u32be(b, bodyStart);
                        long fraction = le ? Wire.u32le(b, bodyStart + 4)
                                : Wire.u32be(b, bodyStart + 4);
                        curTimestamp = seconds * 1000L + ((fraction * 1000L) >>> 32);
                    }
                    break;
                case Rtps.SM_DATA:
                    Data d = decodeData(b, bodyStart, bodyLen, flags, le);
                    if (d != null) {
                        d.sourceTimestampMillis = curTimestamp;
                        d.destPrefix = curDest;
                        out.data.add(d);
                    }
                    break;
                case Rtps.SM_HEARTBEAT:
                    Heartbeat hb = decodeHeartbeat(b, bodyStart, bodyLen, flags, le);
                    if (hb != null) {
                        hb.destPrefix = curDest;
                        out.heartbeats.add(hb);
                    }
                    break;
                case Rtps.SM_ACKNACK:
                    AckNack ack = decodeAckNack(b, bodyStart, bodyLen, flags, le);
                    if (ack != null) {
                        ack.destPrefix = curDest;
                        out.ackNacks.add(ack);
                    }
                    break;
                case Rtps.SM_GAP:
                    Gap gap = decodeGap(b, bodyStart, bodyLen, flags, le);
                    if (gap != null) {
                        gap.destPrefix = curDest;
                        out.gaps.add(gap);
                    }
                    break;
                case Rtps.SM_DATA_FRAG:
                    DataFragment fragment = decodeDataFrag(b, bodyStart, bodyLen, flags, le);
                    if (fragment != null) {
                        fragment.sourceTimestampMillis = curTimestamp;
                        fragment.destPrefix = curDest;
                        out.dataFragments.add(fragment);
                    }
                    break;
                case SM_NACK_FRAG:
                    NackFragment nack = decodeNackFrag(b, bodyStart, bodyLen, le);
                    if (nack != null) {
                        nack.destPrefix = curDest;
                        out.nackFragments.add(nack);
                    }
                    break;
                case SM_HEARTBEAT_FRAG:
                    HeartbeatFragment fragmentHeartbeat = decodeHeartbeatFrag(b, bodyStart, bodyLen, le);
                    if (fragmentHeartbeat != null) {
                        fragmentHeartbeat.destPrefix = curDest;
                        out.heartbeatFragments.add(fragmentHeartbeat);
                    }
                    break;
                default:
                    // Other submessages are not consumed by this path.
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

    private static long u32(byte[] b, int off, boolean le) {
        return le ? Wire.u32le(b, off) : Wire.u32be(b, off);
    }

    private static int u16(byte[] b, int off, boolean le) {
        return le ? Wire.u16le(b, off) : Wire.u16be(b, off);
    }

    private static long sn(byte[] b, int off, boolean le) {
        return le ? readSnLe(b, off) : readSnBe(b, off);
    }

    private static long[] bitmap(byte[] b, int off, int numBits, boolean le) {
        long[] words = new long[(numBits + 31) / 32];
        for (int i = 0; i < words.length; i++) {
            words[i] = u32(b, off + 4 * i, le);
        }
        return words;
    }

    private static AckNack decodeAckNack(byte[] b, int off, int len, int flags, boolean le) {
        if (len < 24) return null;
        long base = sn(b, off + 8, le);
        long bits = u32(b, off + 16, le);
        if (base < 1 || bits > 256 || base > Long.MAX_VALUE - Math.max(0, bits - 1)) return null;
        int words = ((int) bits + 31) / 32;
        if (len != 24 + 4 * words) return null;
        AckNack ack = new AckNack();
        ack.readerId = Wire.slice(b, off, 4);
        ack.writerId = Wire.slice(b, off + 4, 4);
        ack.base = base;
        ack.numBits = (int) bits;
        ack.bitmap = bitmap(b, off + 20, ack.numBits, le);
        ack.count = (int) u32(b, off + 20 + 4 * words, le);
        ack.finalFlag = (flags & Rtps.ACKNACK_FLAG_FINAL) != 0;
        return ack;
    }

    private static long fragmentPayloadSize(long start, int count, int size, long total) {
        if (start < 1 || start > 0xffff_ffffL || count < 1 || count > 0xffff
                || size < 1 || size > 0xffff || total < 1 || total > 0xffff_ffffL) return -1;
        long totalFragments = (total + size - 1) / size;
        if (start + count - 1 > totalFragments) return -1;
        return Math.min((long) count * size, total - (start - 1) * size);
    }

    private static DataFragment decodeDataFrag(byte[] b, int off, int len, int flags, boolean le) {
        if (len < 32) return null;
        int inlineOffset = u16(b, off + 2, le);
        if (inlineOffset < 28 || inlineOffset > len - 4) return null;
        if ((flags & Rtps.DATA_FLAG_INLINE_QOS) == 0 && inlineOffset != 28) return null;
        DataFragment fragment = new DataFragment();
        fragment.readerId = Wire.slice(b, off + 4, 4);
        fragment.writerId = Wire.slice(b, off + 8, 4);
        fragment.writerSn = sn(b, off + 12, le);
        fragment.fragmentStartingNum = u32(b, off + 20, le);
        fragment.fragmentsInSubmessage = u16(b, off + 24, le);
        fragment.fragmentSize = u16(b, off + 26, le);
        fragment.sampleSize = u32(b, off + 28, le);
        fragment.keyFlag = (flags & DATA_FRAG_KEY_FLAG) != 0;
        long expected = fragmentPayloadSize(fragment.fragmentStartingNum,
                fragment.fragmentsInSubmessage, fragment.fragmentSize, fragment.sampleSize);
        if (fragment.writerSn < 1 || expected < 0) return null;
        int pos = off + 4 + inlineOffset, end = off + len;
        if ((flags & Rtps.DATA_FLAG_INLINE_QOS) != 0) {
            fragment.inlineQos = new ParameterList();
            boolean sentinel = false;
            while (pos + 4 <= end) {
                int id = u16(b, pos, le), size = u16(b, pos + 2, le);
                pos += 4;
                if (id == Rtps.PID_SENTINEL) {
                    if (size != 0) return null;
                    sentinel = true;
                    break;
                }
                if ((size & 3) != 0 || size > end - pos) return null;
                fragment.inlineQos.add(id, Wire.slice(b, pos, size));
                pos += size;
            }
            if (!sentinel) return null;
        }
        // RTPS submessage alignment may add at most three trailing octets.
        // Payload length comes from the validated fragment span, not a clamp.
        long available = end - pos;
        if (available < expected || available - expected > 3) return null;
        fragment.serializedPayload = Wire.slice(b, pos, (int) expected);
        return fragment;
    }

    private static Gap decodeGap(byte[] b, int off, int len, int flags, boolean le) {
        if (len < 28) return null;
        long start = sn(b, off + 8, le), base = sn(b, off + 16, le);
        long bits = u32(b, off + 24, le);
        if (start < 1 || base < start || bits > 256 || base > Long.MAX_VALUE - Math.max(0, bits - 1)) return null;
        int words = ((int) bits + 31) / 32;
        // RTPS 2.5 optional GroupInfo/FilteredCount trailers must be present
        // in full if their flags are set, even though this reader ignores them.
        int trailer = ((flags & 0x04) != 0 ? 16 : 0) + ((flags & 0x08) != 0 ? 4 : 0);
        if (len != 28 + 4 * words + trailer) return null;
        Gap gap = new Gap();
        gap.readerId = Wire.slice(b, off, 4);
        gap.writerId = Wire.slice(b, off + 4, 4);
        gap.gapStart = start;
        gap.gapListBase = base;
        gap.numBits = (int) bits;
        gap.bitmap = bitmap(b, off + 28, gap.numBits, le);
        return gap;
    }

    private static boolean validFragmentSet(long base, long bits) {
        return base >= 1 && base <= 0xffff_ffffL && bits >= 0 && bits <= 256
                && base <= 0xffff_ffffL - Math.max(0, bits - 1);
    }

    private static NackFragment decodeNackFrag(byte[] b, int off, int len, boolean le) {
        if (len < 28) return null;
        long writerSn = sn(b, off + 8, le), base = u32(b, off + 16, le);
        long bits = u32(b, off + 20, le);
        if (writerSn < 1 || !validFragmentSet(base, bits)) return null;
        int words = ((int) bits + 31) / 32;
        if (len != 28 + 4 * words) return null;
        NackFragment nack = new NackFragment();
        nack.readerId = Wire.slice(b, off, 4); nack.writerId = Wire.slice(b, off + 4, 4);
        nack.writerSn = writerSn; nack.bitmapBase = base; nack.numBits = (int) bits;
        nack.bitmap = bitmap(b, off + 24, nack.numBits, le);
        nack.count = (int) u32(b, off + 24 + 4 * words, le);
        return nack;
    }

    private static HeartbeatFragment decodeHeartbeatFrag(byte[] b, int off, int len, boolean le) {
        if (len != 24) return null;
        long writerSn = sn(b, off + 8, le), last = u32(b, off + 16, le);
        if (writerSn < 1 || last < 1) return null;
        HeartbeatFragment heartbeat = new HeartbeatFragment();
        heartbeat.readerId = Wire.slice(b, off, 4); heartbeat.writerId = Wire.slice(b, off + 4, 4);
        heartbeat.writerSn = writerSn; heartbeat.lastFragmentNum = last;
        heartbeat.count = (int) u32(b, off + 20, le);
        return heartbeat;
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
        hb.livelinessFlag = (flags & Rtps.HB_FLAG_LIVELINESS) != 0;
        pos += 4;
        if ((flags & Rtps.HB_FLAG_GROUP_INFO) != 0) {
            if (len < 28 + 24) {
                return null; // invalid: the flag says the numbers follow
            }
            hb.currentGsn = le ? readSnLe(b, pos) : readSnBe(b, pos);
            hb.firstGsn = le ? readSnLe(b, pos + 8) : readSnBe(b, pos + 8);
            hb.lastGsn = le ? readSnLe(b, pos + 16) : readSnBe(b, pos + 16);
            // DDSI-RTPS 2.5 8.3.7.5, Validity of a HEARTBEAT with the GroupInfoFlag
            if (hb.currentGsn <= 0 || hb.firstGsn <= 0 || hb.lastGsn < 0 || hb.lastGsn < hb.firstGsn - 1) {
                return null;
            }
            hb.groupInfo = true;
            if (len >= 28 + 24 + 8) { // writerSet and secureWriterSet (Table 8.47)
                hb.writerSet = Wire.slice(b, pos + 24, 4);
            }
        }
        return hb;
    }
}
