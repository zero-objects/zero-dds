// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/**
 * XCDR2 encoder for language bindings.
 *
 * <p>Implements OMG XTypes 1.3 §7.4 (XCDR version 2) byte-exact:
 * <ul>
 *   <li>§7.4.1.5: padding/alignment relative to the buffer start
 *       (natural alignment up to 8 for 8-byte primitives).</li>
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

    private byte[] buf;
    private int pos;
    private final EndianMode endian;

    /** Constructs a writer with default capacity. */
    public Xcdr2Writer() {
        this(EndianMode.LITTLE_ENDIAN);
    }

    /** Constructs a writer with a given endianness. */
    public Xcdr2Writer(EndianMode endian) {
        this.buf = new byte[INITIAL_CAPACITY];
        this.pos = 0;
        this.endian = endian;
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

    /** Closes an appendable/mutable block: patches the DHEADER with the body size. */
    public void endDelimited(int dhdrPos) {
        int bodySize = pos - dhdrPos - 4;
        patchInt32At(dhdrPos, bodySize);
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
