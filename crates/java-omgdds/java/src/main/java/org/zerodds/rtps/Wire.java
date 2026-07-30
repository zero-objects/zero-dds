// SPDX-License-Identifier: Apache-2.0
package org.zerodds.rtps;

import java.util.Arrays;

/**
 * Growable little/big-endian wire buffer + cursor reader for the pure-Java
 * RTPS stack. RTPS scalars are written with an explicit endianness that must
 * match the active submessage/encapsulation E-flag (DDSI-RTPS 2.5 §8.3.4).
 *
 * <p>Ported byte-for-byte from {@code crates/rtps/src} — see
 * {@code wire_types.rs}, {@code submessage_header.rs}, {@code submessages.rs}.
 */
public final class Wire {

    // ---- writer -----------------------------------------------------------

    private byte[] buf;
    private int len;

    public Wire() {
        this(256);
    }

    public Wire(int capacity) {
        this.buf = new byte[Math.max(16, capacity)];
        this.len = 0;
    }

    private void ensure(int extra) {
        if (len + extra <= buf.length) {
            return;
        }
        int cap = buf.length;
        while (cap < len + extra) {
            cap <<= 1;
        }
        buf = Arrays.copyOf(buf, cap);
    }

    public int position() {
        return len;
    }

    public void u8(int v) {
        ensure(1);
        buf[len++] = (byte) v;
    }

    public void bytes(byte[] b) {
        if (b == null || b.length == 0) {
            return;
        }
        ensure(b.length);
        System.arraycopy(b, 0, buf, len, b.length);
        len += b.length;
    }

    public void bytes(byte[] b, int off, int n) {
        if (n == 0) {
            return;
        }
        ensure(n);
        System.arraycopy(b, off, buf, len, n);
        len += n;
    }

    public void u16le(int v) {
        ensure(2);
        buf[len++] = (byte) (v & 0xFF);
        buf[len++] = (byte) ((v >>> 8) & 0xFF);
    }

    public void u16be(int v) {
        ensure(2);
        buf[len++] = (byte) ((v >>> 8) & 0xFF);
        buf[len++] = (byte) (v & 0xFF);
    }

    public void u32le(long v) {
        ensure(4);
        buf[len++] = (byte) (v & 0xFF);
        buf[len++] = (byte) ((v >>> 8) & 0xFF);
        buf[len++] = (byte) ((v >>> 16) & 0xFF);
        buf[len++] = (byte) ((v >>> 24) & 0xFF);
    }

    public void u32be(long v) {
        ensure(4);
        buf[len++] = (byte) ((v >>> 24) & 0xFF);
        buf[len++] = (byte) ((v >>> 16) & 0xFF);
        buf[len++] = (byte) ((v >>> 8) & 0xFF);
        buf[len++] = (byte) (v & 0xFF);
    }

    /** Pad with zero bytes until {@code position()} is a multiple of 4. */
    public void padTo4() {
        while ((len & 3) != 0) {
            u8(0);
        }
    }

    /** Overwrite the 2 bytes at {@code at} with a little-endian u16 (back-patch). */
    public void patchU16le(int at, int v) {
        buf[at] = (byte) (v & 0xFF);
        buf[at + 1] = (byte) ((v >>> 8) & 0xFF);
    }

    public byte[] toBytes() {
        return Arrays.copyOf(buf, len);
    }

    // ---- reader (static cursor helpers over a byte[]) ---------------------

    public static int u16le(byte[] b, int off) {
        return (b[off] & 0xFF) | ((b[off + 1] & 0xFF) << 8);
    }

    public static int u16be(byte[] b, int off) {
        return ((b[off] & 0xFF) << 8) | (b[off + 1] & 0xFF);
    }

    public static long u32le(byte[] b, int off) {
        return (b[off] & 0xFFL)
                | ((b[off + 1] & 0xFFL) << 8)
                | ((b[off + 2] & 0xFFL) << 16)
                | ((b[off + 3] & 0xFFL) << 24);
    }

    public static long u32be(byte[] b, int off) {
        return ((b[off] & 0xFFL) << 24)
                | ((b[off + 1] & 0xFFL) << 16)
                | ((b[off + 2] & 0xFFL) << 8)
                | (b[off + 3] & 0xFFL);
    }

    public static byte[] slice(byte[] b, int off, int n) {
        return Arrays.copyOfRange(b, off, off + n);
    }
}
