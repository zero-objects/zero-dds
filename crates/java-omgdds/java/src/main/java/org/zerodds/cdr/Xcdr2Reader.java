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

    private final byte[] buf;
    private final int origin;
    private final int limit;
    private int pos;
    private final EndianMode endian;

    /** Reads from {@code buf[0..buf.length]} with default LE. */
    public Xcdr2Reader(byte[] buf) {
        this(buf, 0, buf.length, EndianMode.LITTLE_ENDIAN);
    }

    /** Reads with the chosen endianness. */
    public Xcdr2Reader(byte[] buf, EndianMode endian) {
        this(buf, 0, buf.length, endian);
    }

    /** Reads from a subrange. Alignment is measured relative to {@code offset}. */
    public Xcdr2Reader(byte[] buf, int offset, int length, EndianMode endian) {
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

    // ------------------------------------------------------------------
    // Extensibility-Frames
    // ------------------------------------------------------------------

    /** Reads the DHEADER and returns the body size; the caller sets its own limit. */
    public int readDHeader() {
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

    public boolean readPresenceFlag() {
        return readBoolean();
    }

    // ------------------------------------------------------------------
    // Alignment
    // ------------------------------------------------------------------

    public void align(int boundary) {
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
