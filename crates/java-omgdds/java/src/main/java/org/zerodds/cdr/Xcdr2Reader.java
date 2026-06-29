// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.nio.charset.StandardCharsets;

/**
 * XCDR2 decoder for language bindings.
 *
 * <p>Inverse of {@link Xcdr2Writer}. Bounds checks on every read;
 * Stream-Underflow wirft {@link XcdrException}.
 */
public final class Xcdr2Reader {

    /** XCDR2 (4) / XCDR1 (8) maximum alignment (XTypes 1.3 §7.4.1.1.1). */
    private static final int XCDR2_MAX_ALIGN = 4;
    private static final int XCDR1_MAX_ALIGN = 8;

    private final byte[] buf;
    // Non-final: PL_CDR1 (@mutable XCDR1) shifts the alignment origin to each
    // member's body start so member bodies align member-relative, then restores.
    private int origin;
    private final int limit;
    private int pos;
    private final EndianMode endian;
    private final int maxAlign;

    /** Reads from {@code buf[0..buf.length]} with default LE. */
    public Xcdr2Reader(byte[] buf) {
        this(buf, 0, buf.length, EndianMode.LITTLE_ENDIAN);
    }

    /** Reads with the chosen endianness. */
    public Xcdr2Reader(byte[] buf, EndianMode endian) {
        this(buf, 0, buf.length, endian);
    }

    /** Reads from a subrange (XCDR2). Alignment is measured relative to {@code offset}. */
    public Xcdr2Reader(byte[] buf, int offset, int length, EndianMode endian) {
        this(buf, offset, length, endian, XCDR2_MAX_ALIGN);
    }

    /**
     * Reads from a subrange with an explicit alignment cap. Pass
     * {@link #XCDR1_MAX_ALIGN_VALUE} (8) to decode the XCDR1 / classic-CDR wire
     * (no DHEADER on aggregates, PL_CDR1 for {@code @mutable}); the default 4
     * reads XCDR2.
     */
    public Xcdr2Reader(byte[] buf, int offset, int length, EndianMode endian, int maxAlign) {
        if (buf == null) {
            throw new XcdrException("buffer is null");
        }
        if (offset < 0 || length < 0 || offset + length > buf.length) {
            throw new XcdrException(
                    "invalid sub-range: off=" + offset + " len=" + length + " buf.len=" + buf.length);
        }
        this.buf = buf;
        this.origin = offset;
        this.limit = offset + length;
        this.pos = offset;
        this.endian = endian;
        this.maxAlign = (maxAlign == XCDR1_MAX_ALIGN) ? XCDR1_MAX_ALIGN : XCDR2_MAX_ALIGN;
    }

    /** XCDR1 alignment-cap value (8) for the 5-arg constructor. */
    public static final int XCDR1_MAX_ALIGN_VALUE = XCDR1_MAX_ALIGN;

    /** {@code true} when reading the XCDR1 / classic CDR wire. */
    public boolean isXcdr1() {
        return maxAlign == XCDR1_MAX_ALIGN;
    }

    /** Current read position relative to origin. */
    public int position() {
        return pos - origin;
    }

    /** Verbleibende Bytes. */
    public int remaining() {
        return limit - pos;
    }

    // ------------------------------------------------------------------
    // Primitive Reader
    // ------------------------------------------------------------------

    public boolean readBoolean() {
        ensure(1);
        return buf[pos++] != 0;
    }

    public byte readOctet() {
        ensure(1);
        return buf[pos++];
    }

    public int readUInt8() {
        ensure(1);
        return buf[pos++] & 0xFF;
    }

    public char readChar() {
        return (char) readUInt8();
    }

    public char readWChar() {
        align(2);
        ensure(2);
        return (char) readShortRaw();
    }

    public short readInt16() {
        align(2);
        ensure(2);
        return readShortRaw();
    }

    public int readUInt16() {
        return readInt16() & 0xFFFF;
    }

    public int readInt32() {
        align(4);
        ensure(4);
        return readIntRaw();
    }

    public long readUInt32() {
        return readInt32() & 0xFFFF_FFFFL;
    }

    public long readInt64() {
        // 8-byte primitive: align to its natural 8; the central cap reduces it
        // to 4 for XCDR2 (Bug XW) and keeps 8 for XCDR1 / classic CDR.
        align(8);
        ensure(8);
        return readLongRaw();
    }

    public long readUInt64() {
        return readInt64();
    }

    public float readFloat32() {
        align(4);
        ensure(4);
        return Float.intBitsToFloat(readIntRaw());
    }

    public double readFloat64() {
        // 8-byte primitive: align(8); the central cap reduces it to 4 for XCDR2.
        align(8);
        ensure(8);
        return Double.longBitsToDouble(readLongRaw());
    }

    /** Reads a string per XTypes §7.4.4.6. */
    public String readString() {
        long total = readUInt32();
        if (total > Integer.MAX_VALUE) {
            throw new XcdrException("string length exceeds int range: " + total);
        }
        int len = (int) total;
        ensure(len);
        // Strip trailing NUL.
        int textLen = len;
        if (textLen > 0 && buf[pos + textLen - 1] == 0) {
            textLen--;
        }
        String s = new String(buf, pos, textLen, StandardCharsets.UTF_8);
        pos += len;
        return s;
    }

    /** Reads a WString (UTF-16-LE). */
    public String readWString() {
        long count = readUInt32();
        if (count > Integer.MAX_VALUE / 2) {
            throw new XcdrException("wstring length exceeds int range: " + count);
        }
        int n = (int) count;
        ensure(n * 2);
        char[] chars = new char[n];
        for (int i = 0; i < n; i++) {
            chars[i] = (char) readShortRaw();
        }
        return new String(chars);
    }

    /** Reads the sequence count (uint32). */
    public int readSequenceCount() {
        long count = readUInt32();
        if (count > Integer.MAX_VALUE) {
            throw new XcdrException("sequence count exceeds int range: " + count);
        }
        return (int) count;
    }

    /** Reads {@code len} raw bytes (no alignment). */
    public byte[] readBytes(int len) {
        if (len < 0) {
            throw new XcdrException("readBytes negative len: " + len);
        }
        ensure(len);
        byte[] out = new byte[len];
        System.arraycopy(buf, pos, out, 0, len);
        pos += len;
        return out;
    }

    /**
     * Reads an IDL {@code fixed<P,S>}: (P+2)/2 packed-BCD octets back into a
     * {@link java.math.BigDecimal}. Inverse of {@code Xcdr2Writer.writeFixedBcd}.
     */
    public java.math.BigDecimal readFixedBcd(int p, int s) {
        int n = (p + 2) / 2;
        byte[] raw = readBytes(n);
        StringBuilder chars = new StringBuilder();
        char sign = '+';
        for (int i = 0; i < n; i++) {
            int hi = (raw[i] >> 4) & 0x0F;
            int lo = raw[i] & 0x0F;
            chars.append((char) ('0' + (hi % 10)));
            if (i == n - 1) {
                sign = lo == 0x0D ? '-' : '+';
            } else {
                chars.append((char) ('0' + (lo % 10)));
            }
        }
        while (chars.length() > s + 1 && chars.charAt(0) == '0') chars.deleteCharAt(0);
        StringBuilder outStr = new StringBuilder();
        if (sign == '-') outStr.append('-');
        if (s > 0) {
            int dot = Math.max(chars.length() - s, 0);
            for (int i = 0; i < chars.length(); i++) {
                if (i == dot) outStr.append('.');
                outStr.append(chars.charAt(i));
            }
        } else {
            outStr.append(chars);
        }
        return new java.math.BigDecimal(outStr.toString());
    }

    // ------------------------------------------------------------------
    // Extensibility-Frames
    // ------------------------------------------------------------------

    /** Reads the DHEADER and returns the body size; the caller sets its own limit. */
    public int readDHeader() {
        // XCDR1 / classic CDR has no DHEADER — return 0 so the generated
        // `__endPos = position() + 0` equals the current position: the trailing
        // `while (position() < __endPos) skip` loop then does nothing, and an
        // @appendable/@final aggregate (top-level or nested) reads its members
        // inline without consuming a phantom DHEADER. (@mutable branches to the
        // PL_CDR1 reader in the generated decode.)
        if (isXcdr1()) {
            return 0;
        }
        long size = readUInt32();
        if (size > (long) (limit - pos)) {
            throw new XcdrException(
                    "DHEADER size " + size + " exceeds remaining " + (limit - pos));
        }
        return (int) size;
    }

    /** EMHEADER-Decoded Form. */
    public static final class EmHeader {
        public final int memberId;
        public final int lc;
        public final boolean mustUnderstand;

        EmHeader(int memberId, int lc, boolean mustUnderstand) {
            this.memberId = memberId;
            this.lc = lc;
            this.mustUnderstand = mustUnderstand;
        }
    }

    /** Reads the EMHEADER (XTypes §7.4.3.4.5). */
    public EmHeader readEmHeader() {
        align(4);
        ensure(4);
        int header = readIntRaw();
        boolean mu = (header & 0x8000_0000) != 0;
        int lc = (header >>> 28) & 0x7;
        int id = header & 0x0FFF_FFFF;
        return new EmHeader(id, lc, mu);
    }

    public int readNextInt() {
        align(4);
        ensure(4);
        return readIntRaw();
    }

    /** A PL_CDR1 (@mutable XCDR1) member: its id, the absolute end of its body,
     * and the alignment origin to restore when the member is closed. */
    public static final class PlCdr1Member {
        public final int memberId;
        public final int bodyEnd; // absolute buffer position right after the body
        final int prevOrigin;

        PlCdr1Member(int memberId, int bodyEnd, int prevOrigin) {
            this.memberId = memberId;
            this.bodyEnd = bodyEnd;
            this.prevOrigin = prevOrigin;
        }
    }

    /**
     * Begins one PL_CDR1 member: a 4-byte aligned {@code [u16 PID][u16 length]}
     * header; the member body then follows inline. Returns {@code null} at the
     * PID_LIST_END sentinel. PID_EXTENDED (length 8) carries a 32-bit member id +
     * 32-bit body length. Mirrors cdr-core {@code xcdr1::read_pl_cdr1_member}.
     */
    public PlCdr1Member beginPlCdr1Member() {
        final int PID_LIST_END = 0x3F02;
        final int PID_EXTENDED = 0x3F01;
        align(4);
        if (pos + 4 > limit) {
            return null;
        }
        int pid = readUInt16();
        int lenU16 = readUInt16();
        if (pid == PID_LIST_END) {
            return null;
        }
        int memberId;
        int bodyLen;
        if (pid == PID_EXTENDED) {
            memberId = (int) readUInt32();
            bodyLen = (int) readUInt32();
        } else {
            memberId = pid;
            bodyLen = lenU16;
        }
        if (pos + bodyLen > limit) {
            throw new XcdrException("PL_CDR1 member body " + bodyLen + " exceeds remaining " + (limit - pos));
        }
        // Member-relative alignment: shift the origin to the body start so an
        // 8-byte member inside aligns relative to the member, like the cdr-core
        // fresh-reader path. Restored by endPlCdr1Member.
        int prevOrigin = origin;
        origin = pos;
        return new PlCdr1Member(memberId, pos + bodyLen, prevOrigin);
    }

    /** Closes a {@link #beginPlCdr1Member}: restores the origin, positions at the
     * body end, and skips the 4-byte (stream-relative) pad. */
    public void endPlCdr1Member(PlCdr1Member m) {
        origin = m.prevOrigin;
        pos = m.bodyEnd;
        int rel = (pos - origin) & 3;
        int pad = (4 - rel) & 3;
        for (int i = 0; i < pad && pos < limit; i++) {
            pos++;
        }
    }

    public boolean readPresenceFlag() {
        return readBoolean();
    }

    // ------------------------------------------------------------------
    // Alignment
    // ------------------------------------------------------------------

    public void align(int boundary) {
        // Cap at the representation's max alignment (XCDR2 = 4, XCDR1 = 8).
        if (boundary > maxAlign) {
            boundary = maxAlign;
        }
        int rem = (pos - origin) % boundary;
        if (rem != 0) {
            int padBytes = boundary - rem;
            ensure(padBytes);
            pos += padBytes;
        }
    }

    /** Skip {@code n} bytes (no alignment). */
    public void skip(int n) {
        if (n < 0) {
            throw new XcdrException("skip negative: " + n);
        }
        ensure(n);
        pos += n;
    }

    private void ensure(int n) {
        if (pos + n > limit) {
            throw new XcdrException(
                    "underflow: need " + n + ", have " + (limit - pos) + " (pos=" + (pos - origin) + ")");
        }
    }

    private short readShortRaw() {
        if (endian == EndianMode.LITTLE_ENDIAN) {
            int b0 = buf[pos++] & 0xFF;
            int b1 = buf[pos++] & 0xFF;
            return (short) ((b1 << 8) | b0);
        } else {
            int b0 = buf[pos++] & 0xFF;
            int b1 = buf[pos++] & 0xFF;
            return (short) ((b0 << 8) | b1);
        }
    }

    private int readIntRaw() {
        if (endian == EndianMode.LITTLE_ENDIAN) {
            int b0 = buf[pos++] & 0xFF;
            int b1 = buf[pos++] & 0xFF;
            int b2 = buf[pos++] & 0xFF;
            int b3 = buf[pos++] & 0xFF;
            return (b3 << 24) | (b2 << 16) | (b1 << 8) | b0;
        } else {
            int b0 = buf[pos++] & 0xFF;
            int b1 = buf[pos++] & 0xFF;
            int b2 = buf[pos++] & 0xFF;
            int b3 = buf[pos++] & 0xFF;
            return (b0 << 24) | (b1 << 16) | (b2 << 8) | b3;
        }
    }

    private long readLongRaw() {
        if (endian == EndianMode.LITTLE_ENDIAN) {
            long v = 0;
            for (int i = 0; i < 8; i++) {
                v |= ((long) (buf[pos++] & 0xFF)) << (i * 8);
            }
            return v;
        } else {
            long v = 0;
            for (int i = 7; i >= 0; i--) {
                v |= ((long) (buf[pos++] & 0xFF)) << (i * 8);
            }
            return v;
        }
    }
}
