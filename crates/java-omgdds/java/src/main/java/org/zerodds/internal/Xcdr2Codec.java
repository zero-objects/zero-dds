// SPDX-License-Identifier: Apache-2.0
package org.zerodds.internal;

import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.StandardCharsets;

/**
 * Pure-Java XCDR2 primitive encoder + decoder.
 *
 * <p>Conforms to OMG DDS-XTYPES 1.3 §7.4.3 (XCDR Version 2). Subset:
 * primitive types + length-prefixed UTF-8 string. Aggregated types
 * are caller-responsibility (compose primitives in the order the IDL
 * declares).
 *
 * <p>Default encoding: little-endian (XCDR2 representation
 * {@code 0x00 0x07} per Spec §7.4.3.2).
 */
public final class Xcdr2Codec {
    private Xcdr2Codec() {}

    public static ByteBuffer encoder(int initialCapacity) {
        return ByteBuffer.allocate(initialCapacity).order(ByteOrder.LITTLE_ENDIAN);
    }

    public static ByteBuffer decoder(byte[] bytes) {
        return ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);
    }

    public static void writeBoolean(ByteBuffer buf, boolean v) {
        buf.put(v ? (byte) 1 : (byte) 0);
    }

    public static boolean readBoolean(ByteBuffer buf) {
        return buf.get() != 0;
    }

    public static void writeByte(ByteBuffer buf, byte v) {
        buf.put(v);
    }

    public static byte readByte(ByteBuffer buf) {
        return buf.get();
    }

    public static void writeShort(ByteBuffer buf, short v) {
        align(buf, 2);
        buf.putShort(v);
    }

    public static short readShort(ByteBuffer buf) {
        align(buf, 2);
        return buf.getShort();
    }

    public static void writeInt(ByteBuffer buf, int v) {
        align(buf, 4);
        buf.putInt(v);
    }

    public static int readInt(ByteBuffer buf) {
        align(buf, 4);
        return buf.getInt();
    }

    public static void writeLong(ByteBuffer buf, long v) {
        // XCDR2: alignment-cap = 4, NOT 8.
        align(buf, 4);
        buf.putLong(v);
    }

    public static long readLong(ByteBuffer buf) {
        align(buf, 4);
        return buf.getLong();
    }

    public static void writeFloat(ByteBuffer buf, float v) {
        align(buf, 4);
        buf.putFloat(v);
    }

    public static float readFloat(ByteBuffer buf) {
        align(buf, 4);
        return buf.getFloat();
    }

    public static void writeDouble(ByteBuffer buf, double v) {
        align(buf, 4);
        buf.putDouble(v);
    }

    public static double readDouble(ByteBuffer buf) {
        align(buf, 4);
        return buf.getDouble();
    }

    public static void writeString(ByteBuffer buf, String s) {
        byte[] bytes = s.getBytes(StandardCharsets.UTF_8);
        align(buf, 4);
        buf.putInt(bytes.length + 1); // +1 for trailing NUL.
        buf.put(bytes);
        buf.put((byte) 0);
    }

    public static String readString(ByteBuffer buf) {
        align(buf, 4);
        int len = buf.getInt();
        byte[] tmp = new byte[len];
        buf.get(tmp);
        // Strip trailing NUL.
        int end = tmp.length;
        if (end > 0 && tmp[end - 1] == 0) end--;
        return new String(tmp, 0, end, StandardCharsets.UTF_8);
    }

    /** XCDR2 §7.4.3.4.6 — alignment relative to sub-message origin. */
    private static void align(ByteBuffer buf, int boundary) {
        int pos = buf.position();
        int rem = pos % boundary;
        if (rem != 0) {
            buf.position(pos + (boundary - rem));
        }
    }

    public static byte[] copyToBytes(ByteBuffer buf) {
        byte[] out = new byte[buf.position()];
        buf.flip();
        buf.get(out);
        return out;
    }
}
