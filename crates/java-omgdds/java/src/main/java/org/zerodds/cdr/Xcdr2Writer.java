// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/**
 * XCDR2 encoder for language bindings.
 *
 * <p>Implements OMG XTypes 1.3 §7.4 (XCDR version 2) byte-exact:
 * <ul>
 *   <li>§7.4.1.5: padding/alignment relative to the buffer start; XCDR2 caps
 *       the maximum alignment at 4 (§7.4.1.1.1 MAXALIGN(2)=4), so an 8-byte
 *       primitive aligns to min(sizeof, 4) = 4, NOT 8.</li>
 *   <li>§7.4.4.4: DHEADER for DELIMITED_CDR2 (Appendable).</li>
 *   <li>§7.4.3.4.5: EMHEADER for PL_CDR2 (Mutable).</li>
 *   <li>§7.4.4.6: string with {@code uint32 length+1} + UTF-8 + NUL.</li>
 * </ul>
 *
 * <p>Spec anchors: zerodds-xcdr2-bindings-conformance-1.0 §3,
 * zerodds-xcdr2-java-1.0 §3.
 *
 * <p>The default endianness is little-endian (wire default per
 * Conformance spec §3). Big-endian is used exclusively for
 * key-hash computation (XTypes §7.6.8).
 */
public final class Xcdr2Writer {

    /** Initial capacity (grows as needed). */
    private static final int INITIAL_CAPACITY = 64;

    /** XCDR2 maximum alignment (§7.4.1.1.1 MAXALIGN(2)=4). */
    private static final int XCDR2_MAX_ALIGN = 4;
    /** XCDR1 / classic-CDR maximum alignment (8: 8-byte primitives align to 8). */
    private static final int XCDR1_MAX_ALIGN = 8;

    /** Public XCDR1 max-align selector for {@link #Xcdr2Writer(EndianMode, int)}. */
    public static final int XCDR1_MAX_ALIGN_VALUE = XCDR1_MAX_ALIGN;

    private byte[] buf;
    private int pos;
    private final EndianMode endian;
    /** 4 for XCDR2, 8 for the classic-CDR / PL_CDR1 wire. */
    private final int maxAlign;

    /** Constructs a writer with default capacity. */
    public Xcdr2Writer() {
        this(EndianMode.LITTLE_ENDIAN);
    }

    /** Constructs a writer with a given endianness (XCDR2 max-align). */
    public Xcdr2Writer(EndianMode endian) {
        this(endian, XCDR2_MAX_ALIGN);
    }

    /**
     * Constructs a writer with a given endianness and maximum alignment. Pass
     * {@link #XCDR1_MAX_ALIGN_VALUE} (8) to emit the XCDR1 / classic-CDR wire
     * (no DHEADER on @final/@appendable, 8-byte primitive alignment, PL_CDR1
     * for @mutable); any other value selects XCDR2 (max-align 4).
     */
    public Xcdr2Writer(EndianMode endian, int maxAlign) {
        this.buf = new byte[INITIAL_CAPACITY];
        this.pos = 0;
        this.endian = endian;
        this.maxAlign = (maxAlign == XCDR1_MAX_ALIGN) ? XCDR1_MAX_ALIGN : XCDR2_MAX_ALIGN;
    }

    /** The endianness this writer emits in. */
    public EndianMode endian() {
        return endian;
    }

    /** {@code true} when emitting the XCDR1 / classic-CDR wire (max-align 8). */
    public boolean isXcdr1() {
        return maxAlign == XCDR1_MAX_ALIGN;
    }

    /** Current write position (number of bytes written so far). */
    public int position() {
        return pos;
    }

    /** Returns the written bytes as a copy. */
    public byte[] toByteArray() {
        return Arrays.copyOf(buf, pos);
    }

    // ------------------------------------------------------------------
    // Primitive writers
    // ------------------------------------------------------------------

    /** Boolean: 1 byte (0x00 or 0x01). */
    public void writeBoolean(boolean v) {
        ensure(1);
        buf[pos++] = (byte) (v ? 1 : 0);
    }

    /** Octet: 1 byte (unsigned at the IDL layer; Java {@code byte} is signed). */
    public void writeOctet(byte v) {
        ensure(1);
        buf[pos++] = v;
    }

    /** uint8 with range check (Java has no unsigned). */
    public void writeUInt8(int v) {
        if (v < 0 || v > 0xFF) {
            throw new XcdrException("uint8 out of range: " + v);
        }
        ensure(1);
        buf[pos++] = (byte) v;
    }

    /** Char: 1 byte (ASCII; non-ASCII throws). */
    public void writeChar(char v) {
        if (v > 0x7F) {
            throw new XcdrException("char out of ASCII range: 0x" + Integer.toHexString(v));
        }
        writeUInt8(v);
    }

    /** Wchar: 2 bytes UTF-16 LE (endian flip on BE). */
    public void writeWChar(char v) {
        align(2);
        writeShortRaw((short) v);
    }

    /** Int16. */
    public void writeInt16(short v) {
        align(2);
        writeShortRaw(v);
    }

    /** UInt16 (passed as int; range check). */
    public void writeUInt16(int v) {
        if (v < 0 || v > 0xFFFF) {
            throw new XcdrException("uint16 out of range: " + v);
        }
        writeInt16((short) v);
    }

    /** Int32. */
    public void writeInt32(int v) {
        align(4);
        writeIntRaw(v);
    }

    /** UInt32 (as long; range check). */
    public void writeUInt32(long v) {
        if (v < 0 || v > 0xFFFF_FFFFL) {
            throw new XcdrException("uint32 out of range: " + v);
        }
        writeInt32((int) v);
    }

    /** Int64. */
    public void writeInt64(long v) {
        // Request the natural 8-byte alignment; align() caps it at maxAlign, so
        // this is min(sizeof, 4) = 4 under XCDR2 (XTypes 1.3 §7.4.1.1.1,
        // MAXALIGN(2)=4 — avoiding the Bug XW over-alignment) and a true 8-byte
        // alignment under XCDR1 / classic CDR.
        align(8);
        writeLongRaw(v);
    }

    /** UInt64 (two's complement; all long values allowed). */
    public void writeUInt64(long v) {
        writeInt64(v);
    }

    /** Float32 IEEE-754. */
    public void writeFloat32(float v) {
        align(4);
        writeIntRaw(Float.floatToRawIntBits(v));
    }

    /** Float64 IEEE-754. */
    public void writeFloat64(double v) {
        // align(8) capped to maxAlign — 4 under XCDR2 (XTypes §7.4.1.1.1), 8
        // under XCDR1. See writeInt64.
        align(8);
        writeLongRaw(Double.doubleToRawLongBits(v));
    }

    /**
     * String: {@code uint32 length+1} + UTF-8 + NUL (XTypes §7.4.4.6).
     */
    public void writeString(String s) {
        if (s == null) {
            throw new XcdrException("string must not be null");
        }
        byte[] utf = s.getBytes(StandardCharsets.UTF_8);
        long total = (long) utf.length + 1L;
        writeUInt32(total);
        ensure(utf.length + 1);
        System.arraycopy(utf, 0, buf, pos, utf.length);
        pos += utf.length;
        buf[pos++] = 0; // NUL
    }

    /**
     * WString: {@code uint32 length} + UTF-16-LE Code-Units (XTypes
     * §7.4.4.6 / spec erratum §9.1). Length counts code units, NOT
     * bytes; no NUL.
     */
    public void writeWString(String s) {
        if (s == null) {
            throw new XcdrException("wstring must not be null");
        }
        char[] units = s.toCharArray();
        writeUInt32(units.length);
        ensure(units.length * 2);
        for (char c : units) {
            writeShortRaw((short) c);
        }
    }

    /** Copy bytes without alignment (e.g. raw payload). */
    public void writeBytes(byte[] data) {
        ensure(data.length);
        System.arraycopy(data, 0, buf, pos, data.length);
        pos += data.length;
    }

    /**
     * Writes an IDL {@code fixed<P,S>} value as CORBA/GIOP §9.3.2.7 packed BCD:
     * P digit nibbles MSB-first + a sign nibble (0xC pos, 0xD neg), a leading
     * 0x0 pad when P+1 is odd, packed 2 nibbles/byte high-first. (P+2)/2 octets,
     * no length prefix, no alignment, endian-independent. Ported from the
     * oracle-validated {@code crates/cdr/src/fixed.rs}.
     */
    public void writeFixedBcd(java.math.BigDecimal value, int p, int s) {
        String text = value.toPlainString();
        boolean positive = true;
        if (text.startsWith("-")) { positive = false; text = text.substring(1); }
        else if (text.startsWith("+")) { text = text.substring(1); }
        int dot = text.indexOf('.');
        String intPart = dot < 0 ? text : text.substring(0, dot);
        String fracPart = dot < 0 ? "" : text.substring(dot + 1);
        int intNeeded = p - s;
        if (intPart.length() > intNeeded) {
            throw new XcdrException("fixed: integer part '" + intPart + "' exceeds P-S=" + intNeeded);
        }
        if (fracPart.length() > s) {
            throw new XcdrException("fixed: fractional part '" + fracPart + "' exceeds S=" + s);
        }
        StringBuilder digits = new StringBuilder();
        for (int i = intPart.length(); i < intNeeded; i++) digits.append('0');
        digits.append(intPart).append(fracPart);
        for (int i = fracPart.length(); i < s; i++) digits.append('0');
        int[] nibbles = new int[p + 2];
        int n = 0;
        if ((p + 1) % 2 == 1) nibbles[n++] = 0;
        for (int i = 0; i < digits.length(); i++) {
            char c = digits.charAt(i);
            if (c < '0' || c > '9') throw new XcdrException("fixed: non-digit '" + c + "'");
            nibbles[n++] = c - '0';
        }
        nibbles[n++] = positive ? 0x0C : 0x0D;
        byte[] out = new byte[n / 2];
        for (int b = 0; b < out.length; b++) {
            out[b] = (byte) ((nibbles[2 * b] << 4) | nibbles[2 * b + 1]);
        }
        writeBytes(out);
    }

    // ------------------------------------------------------------------
    // Sequence helpers
    // ------------------------------------------------------------------

    /** Writes the sequence count + delegates element encoding to the caller. */
    public void writeSequenceCount(int count) {
        if (count < 0) {
            throw new XcdrException("sequence count negative: " + count);
        }
        writeUInt32(count);
    }

    // ------------------------------------------------------------------
    // Extensibility frames
    // ------------------------------------------------------------------

    /**
     * Begins an appendable block: reserves 4 bytes for the DHEADER
     * and returns the position (for the later {@link
     * #endDelimited(int)} call).
     *
     * <p>XTypes §7.4.4.4: DHEADER is {@code uint32} (endianness like the
     * body), value = number of bytes after the DHEADER until block end.
     */
    public int beginAppendable() {
        // XCDR1 / classic CDR has no DHEADER on @final/@appendable aggregates:
        // a frame-less no-op (token -1, ignored by endDelimited).
        if (isXcdr1()) {
            return -1;
        }
        align(4);
        int dhdrPos = pos;
        ensure(4);
        // Placeholder — patched in endDelimited.
        writeIntRaw(0);
        return dhdrPos;
    }

    /** Identical to {@link #beginAppendable} — Mutable uses the same DHEADER mechanism. */
    public int beginMutable() {
        return beginAppendable();
    }

    /** Closes an appendable/mutable block: patches the DHEADER with the body size.
     *  A negative token (XCDR1, no DHEADER was written) is a no-op. */
    public void endDelimited(int dhdrPos) {
        if (dhdrPos < 0) {
            return;
        }
        int bodySize = pos - dhdrPos - 4;
        patchInt32At(dhdrPos, bodySize);
    }

    // ------------------------------------------------------------------
    // PL_CDR1 (@mutable, XCDR1 / classic CDR) — XTypes §7.4.3.3
    // ------------------------------------------------------------------

    /** PID_EXTENDED — long-form member header (32-bit id + 32-bit length). */
    private static final int PID_EXTENDED = 0x3F01;
    /** PID_LIST_END — sentinel terminating a PL_CDR1 member list. */
    private static final int PID_LIST_END = 0x3F02;

    /**
     * Writes one PL_CDR1 (@mutable XCDR1) member: a 4-byte aligned
     * {@code [u16 PID][u16 length]} header (or the {@link #PID_EXTENDED}
     * long form for ids &gt;= 0x3F00 / bodies &gt; 0xFFFF), the member
     * {@code body} bytes, then zero-pad to the next 4-byte boundary (the pad
     * is NOT counted in {@code length}). The body must have been built by a
     * fresh member-relative sub-writer. Mirrors cdr-core
     * {@code xcdr1::encode_pl_cdr1_member} (verified byte-identical).
     */
    public void writePlCdr1Member(int memberId, byte[] body) {
        align(4);
        int bodyLen = body.length;
        if (memberId >= 0x3F00 || bodyLen > 0xFFFF) {
            writeUInt16(PID_EXTENDED);
            writeUInt16(8);
            writeUInt32(memberId & 0xFFFF_FFFFL);
            writeUInt32(bodyLen & 0xFFFF_FFFFL);
        } else {
            writeUInt16(memberId);
            writeUInt16(bodyLen);
        }
        writeBytes(body);
        int pad = (4 - (bodyLen % 4)) % 4;
        for (int i = 0; i < pad; i++) {
            writeOctet((byte) 0);
        }
    }

    /** Writes the {@link #PID_LIST_END} terminator of a PL_CDR1 member list. */
    public void writePlCdr1Sentinel() {
        align(4);
        writeUInt16(PID_LIST_END);
        writeUInt16(0);
    }

    // ------------------------------------------------------------------
    // EMHEADER (PL_CDR2 / Mutable)
    // ------------------------------------------------------------------

    /** Length-code constants per XTypes §7.4.3.4.5. */
    public static final int LC_BYTE = 0;        // 1-byte member
    public static final int LC_SHORT = 1;       // 2-byte member
    public static final int LC_INT32 = 2;       // 4-byte member
    public static final int LC_INT64 = 3;       // 8-byte member
    public static final int LC_NEXTINT = 4;     // NEXTINT-prefix follows
    public static final int LC_NEXTINT_4 = 5;   // NEXTINT, multiple of 4
    public static final int LC_NEXTINT_4_4 = 6; // NEXTINT, multiple of 4, also at offset 4
    public static final int LC_NEXTINT_8_8 = 7; // NEXTINT, multiple of 8

    /**
     * EMHEADER = M-Bit (must-understand) + LC (3 bits) + ID (28 bits).
     *
     * <p>On the wire EMHEADER is a {@code uint32} in
     * body endianness (XTypes §7.4.3.4.5).
     */
    public void writeEmHeader(int memberId, int lc, boolean mustUnderstand) {
        if (memberId < 0 || memberId > 0x0FFF_FFFF) {
            throw new XcdrException("EMHEADER member-id out of 28-bit range: " + memberId);
        }
        if (lc < 0 || lc > 7) {
            throw new XcdrException("EMHEADER LC out of 3-bit range: " + lc);
        }
        int header = memberId & 0x0FFF_FFFF;
        header |= (lc & 0x7) << 28;
        if (mustUnderstand) {
            header |= 0x8000_0000;
        }
        align(4);
        writeIntRaw(header);
    }

    /**
     * NEXTINT (4 byte uint32) — member size hint for LC>=4.
     */
    public void writeNextInt(int size) {
        if (size < 0) {
            throw new XcdrException("NEXTINT negative: " + size);
        }
        align(4);
        writeIntRaw(size);
    }

    // ------------------------------------------------------------------
    // Optional present-byte (for Final/Appendable, NOT Mutable)
    // ------------------------------------------------------------------

    /** Writes the presence-flag byte for @optional in Final/Appendable. */
    public void writePresenceFlag(boolean present) {
        writeBoolean(present);
    }

    // ------------------------------------------------------------------
    // Alignment
    // ------------------------------------------------------------------

    /**
     * Padding insertion per XTypes §7.4.1.5 (relative to the
     * Buffer-Start). Boundary {@code 1, 2, 4, 8}.
     */
    public void align(int boundary) {
        // Cap at the representation's max alignment (XCDR2 = 4, XCDR1 = 8) — an
        // 8-byte primitive aligns to 4 under XCDR2 (XTypes 1.3 §7.4.1.1.1). This
        // mirrors Xcdr2Reader.align(); the int64/uint64/double writers request
        // align(8) and rely on this cap (see their javadoc).
        if (boundary > maxAlign) {
            boundary = maxAlign;
        }
        int rem = pos % boundary;
        if (rem != 0) {
            int padBytes = boundary - rem;
            ensure(padBytes);
            for (int i = 0; i < padBytes; i++) {
                buf[pos++] = 0;
            }
        }
    }

    // ------------------------------------------------------------------
    // Internal raw I/O (Endian-aware)
    // ------------------------------------------------------------------

    private void writeShortRaw(short v) {
        ensure(2);
        if (endian == EndianMode.LITTLE_ENDIAN) {
            buf[pos++] = (byte) (v & 0xFF);
            buf[pos++] = (byte) ((v >> 8) & 0xFF);
        } else {
            buf[pos++] = (byte) ((v >> 8) & 0xFF);
            buf[pos++] = (byte) (v & 0xFF);
        }
    }

    private void writeIntRaw(int v) {
        ensure(4);
        if (endian == EndianMode.LITTLE_ENDIAN) {
            buf[pos++] = (byte) (v & 0xFF);
            buf[pos++] = (byte) ((v >> 8) & 0xFF);
            buf[pos++] = (byte) ((v >> 16) & 0xFF);
            buf[pos++] = (byte) ((v >> 24) & 0xFF);
        } else {
            buf[pos++] = (byte) ((v >> 24) & 0xFF);
            buf[pos++] = (byte) ((v >> 16) & 0xFF);
            buf[pos++] = (byte) ((v >> 8) & 0xFF);
            buf[pos++] = (byte) (v & 0xFF);
        }
    }

    private void writeLongRaw(long v) {
        ensure(8);
        if (endian == EndianMode.LITTLE_ENDIAN) {
            for (int i = 0; i < 8; i++) {
                buf[pos++] = (byte) ((v >> (i * 8)) & 0xFF);
            }
        } else {
            for (int i = 7; i >= 0; i--) {
                buf[pos++] = (byte) ((v >> (i * 8)) & 0xFF);
            }
        }
    }

    private void patchInt32At(int patchPos, int v) {
        int saved = pos;
        pos = patchPos;
        writeIntRaw(v);
        pos = saved;
    }

    private void ensure(int extra) {
        int need = pos + extra;
        if (need > buf.length) {
            int newCap = buf.length * 2;
            while (newCap < need) {
                newCap *= 2;
            }
            buf = Arrays.copyOf(buf, newCap);
        }
    }
}
