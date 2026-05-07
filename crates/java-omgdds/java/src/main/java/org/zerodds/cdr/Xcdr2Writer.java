// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;

/**
 * XCDR2-Encoder fuer Sprach-Bindings.
 *
 * <p>Implementiert OMG XTypes 1.3 §7.4 (XCDR Version 2) byte-genau:
 * <ul>
 *   <li>§7.4.1.5: Padding/Alignment relativ zum Buffer-Start
 *       (natuerliche Alignment bis 8 fuer 8-byte-Primitive).</li>
 *   <li>§7.4.4.4: DHEADER fuer DELIMITED_CDR2 (Appendable).</li>
 *   <li>§7.4.3.4.5: EMHEADER fuer PL_CDR2 (Mutable).</li>
 *   <li>§7.4.4.6: String mit {@code uint32 length+1} + UTF-8 + NUL.</li>
 * </ul>
 *
 * <p>Spec-Anker: zerodds-xcdr2-bindings-conformance-1.0 §3,
 * zerodds-xcdr2-java-1.0 §3.
 *
 * <p>Default-Endianness ist Little-Endian (Wire-Default per
 * Conformance-Spec §3). Big-Endian wird ausschliesslich fuer
 * Key-Hash-Berechnung (XTypes §7.6.8) genutzt.
 */
public final class Xcdr2Writer {

    /** Initial-Kapazitaet (waechst bei Bedarf). */
    private static final int INITIAL_CAPACITY = 64;

    private byte[] buf;
    private int pos;
    private final EndianMode endian;

    /** Konstruiert einen Writer mit Default-Kapazitaet. */
    public Xcdr2Writer() {
        this(EndianMode.LITTLE_ENDIAN);
    }

    /** Konstruiert einen Writer mit gegebener Endianness. */
    public Xcdr2Writer(EndianMode endian) {
        this.buf = new byte[INITIAL_CAPACITY];
        this.pos = 0;
        this.endian = endian;
    }

    /** Aktuelle Schreibposition (Anzahl bisher geschriebener Bytes). */
    public int position() {
        return pos;
    }

    /** Liefert die geschriebenen Bytes als Kopie. */
    public byte[] toByteArray() {
        return Arrays.copyOf(buf, pos);
    }

    // ------------------------------------------------------------------
    // Primitive-Writer
    // ------------------------------------------------------------------

    /** Boolean: 1 Byte (0x00 oder 0x01). */
    public void writeBoolean(boolean v) {
        ensure(1);
        buf[pos++] = (byte) (v ? 1 : 0);
    }

    /** Octet: 1 Byte (vorzeichenfrei am IDL-Layer; Java {@code byte} hat Vorzeichen). */
    public void writeOctet(byte v) {
        ensure(1);
        buf[pos++] = v;
    }

    /** uint8 mit Range-Check (Java has no unsigned). */
    public void writeUInt8(int v) {
        if (v < 0 || v > 0xFF) {
            throw new XcdrException("uint8 out of range: " + v);
        }
        ensure(1);
        buf[pos++] = (byte) v;
    }

    /** Char: 1 Byte (ASCII; non-ASCII wirft). */
    public void writeChar(char v) {
        if (v > 0x7F) {
            throw new XcdrException("char out of ASCII range: 0x" + Integer.toHexString(v));
        }
        writeUInt8(v);
    }

    /** Wchar: 2 Bytes UTF-16 LE (Endian-Flip bei BE). */
    public void writeWChar(char v) {
        align(2);
        writeShortRaw((short) v);
    }

    /** Int16. */
    public void writeInt16(short v) {
        align(2);
        writeShortRaw(v);
    }

    /** UInt16 (als int passed; Range-Check). */
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

    /** UInt32 (als long; Range-Check). */
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

    /** UInt64 (Two's-Complement; alle long-Werte erlaubt). */
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
     * §7.4.4.6 / Spec-Erratum §9.1). Length zaehlt Code-Units, NICHT
     * Bytes; kein NUL.
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

    /** Bytes ohne Alignment kopieren (z.B. raw payload). */
    public void writeBytes(byte[] data) {
        ensure(data.length);
        System.arraycopy(data, 0, buf, pos, data.length);
        pos += data.length;
    }

    // ------------------------------------------------------------------
    // Sequence-Helpers
    // ------------------------------------------------------------------

    /** Schreibt sequence-count + delegiert die Element-Encodierung an den Caller. */
    public void writeSequenceCount(int count) {
        if (count < 0) {
            throw new XcdrException("sequence count negative: " + count);
        }
        writeUInt32(count);
    }

    // ------------------------------------------------------------------
    // Extensibility-Frames
    // ------------------------------------------------------------------

    /**
     * Beginnt einen Appendable-Block: reserviert 4 Bytes fuer DHEADER
     * und liefert die Position (zum spaeteren {@link
     * #endDelimited(int)}-Aufruf).
     *
     * <p>XTypes §7.4.4.4: DHEADER ist {@code uint32} (Endianness wie
     * Body), Wert = Anzahl Bytes nach DHEADER bis Block-Ende.
     */
    public int beginAppendable() {
        align(4);
        int dhdrPos = pos;
        ensure(4);
        // Platzhalter — wird in endDelimited gepatcht.
        writeIntRaw(0);
        return dhdrPos;
    }

    /** Identisch zu {@link #beginAppendable} — Mutable nutzt selben DHEADER-Mechanismus. */
    public int beginMutable() {
        return beginAppendable();
    }

    /** Schliesst einen Appendable/Mutable-Block: patcht DHEADER mit Body-Size. */
    public void endDelimited(int dhdrPos) {
        int bodySize = pos - dhdrPos - 4;
        patchInt32At(dhdrPos, bodySize);
    }

    // ------------------------------------------------------------------
    // EMHEADER (PL_CDR2 / Mutable)
    // ------------------------------------------------------------------

    /** Length-Code-Konstanten gemaess XTypes §7.4.3.4.5. */
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
     * <p>Auf der Wire ist EMHEADER ein {@code uint32} in
     * Body-Endianness (XTypes §7.4.3.4.5).
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
     * NEXTINT (4 Byte uint32) — Member-Size-Hint fuer LC>=4.
     */
    public void writeNextInt(int size) {
        if (size < 0) {
            throw new XcdrException("NEXTINT negative: " + size);
        }
        align(4);
        writeIntRaw(size);
    }

    // ------------------------------------------------------------------
    // Optional present-byte (fuer Final/Appendable, NICHT Mutable)
    // ------------------------------------------------------------------

    /** Schreibt das Presence-Flag-Byte fuer @optional in Final/Appendable. */
    public void writePresenceFlag(boolean present) {
        writeBoolean(present);
    }

    // ------------------------------------------------------------------
    // Alignment
    // ------------------------------------------------------------------

    /**
     * Padding-Insertion gemaess XTypes §7.4.1.5 (relativ zum
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
