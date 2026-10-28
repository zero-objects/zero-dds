// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// DDS-AMQP 1.0 §7 / §8.1 — the Java runtime the generated helpers call.
//
// Standard-library-only: a generated file imports this and needs nothing
// else. The octets it writes are OASIS AMQP 1.0 Part 1 §1.6 primitives;
// the framing decisions belong to the generated helper.
//
// A runtime and not generated code, for the reason the C++ header gives.

package org.zerodds.amqp;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

public final class AmqpCodec {
    private AmqpCodec() {}

    // -- primitives --------------------------------------------------

    private static byte[] fixed(int code, long value, int width) {
        byte[] out = new byte[1 + width];
        out[0] = (byte) code;
        for (int i = 0; i < width; i++) {
            out[1 + i] = (byte) ((value >>> (8 * (width - 1 - i))) & 0xFFL);
        }
        return out;
    }

    /** §1.6.1 */
    public static byte[] encodeNull() {
        return new byte[] {(byte) 0x40};
    }

    /** §1.6.2 — the compact form. */
    public static byte[] encodeBoolean(boolean value) {
        return new byte[] {(byte) (value ? 0x41 : 0x42)};
    }

    public static byte[] encodeUbyte(int value) {
        return fixed(0x50, value & 0xFFL, 1);
    }

    public static byte[] encodeByte(int value) {
        return fixed(0x51, value & 0xFFL, 1);
    }

    public static byte[] encodeUshort(int value) {
        return fixed(0x60, value & 0xFFFFL, 2);
    }

    public static byte[] encodeShort(int value) {
        return fixed(0x61, value & 0xFFFFL, 2);
    }

    /**
     * §7.1's table gives these four rows two or three codes each —
     * "compact form for -128 &lt;= n &lt;= 127", "compact / zero" — and
     * the choice is part of the encoding a peer compares. The narrow
     * form whenever it fits, in every column.
     */
    public static byte[] encodeUint(long value) {
        long v = value & 0xFFFFFFFFL;
        if (v == 0L) {
            return new byte[] {(byte) 0x43};
        }
        if (v <= 0xFFL) {
            return fixed(0x52, v, 1);
        }
        return fixed(0x70, v, 4);
    }

    public static byte[] encodeInt(int value) {
        if (value >= -128 && value <= 127) {
            return fixed(0x54, value & 0xFFL, 1);
        }
        return fixed(0x71, value & 0xFFFFFFFFL, 4);
    }

    public static byte[] encodeUlong(long value) {
        if (value == 0L) {
            return new byte[] {(byte) 0x44};
        }
        if (Long.compareUnsigned(value, 0xFFL) <= 0) {
            return fixed(0x53, value, 1);
        }
        return fixed(0x80, value, 8);
    }

    public static byte[] encodeLong(long value) {
        if (value >= -128L && value <= 127L) {
            return fixed(0x55, value & 0xFFL, 1);
        }
        return fixed(0x81, value, 8);
    }

    /** §1.6.11 — IEEE 754 binary32, big-endian. */
    public static byte[] encodeFloat(float value) {
        return fixed(0x72, Float.floatToRawIntBits(value) & 0xFFFFFFFFL, 4);
    }

    /** §1.6.12 — IEEE 754 binary64, big-endian. */
    public static byte[] encodeDouble(double value) {
        return fixed(0x82, Double.doubleToRawLongBits(value), 8);
    }

    /** §1.6.16 — a UTF-32 code point. */
    public static byte[] encodeChar(int codepoint) {
        return fixed(0x73, codepoint & 0xFFFFFFFFL, 4);
    }

    /** §1.6.17 — milliseconds since the Unix epoch. */
    public static byte[] encodeTimestamp(long millis) {
        return fixed(0x83, millis, 8);
    }

    /** The narrow form for up to 255 octets and the wide one beyond. */
    private static byte[] lengthPrefixed(int narrow, int wide, byte[] data) {
        if (data.length <= 0xFF) {
            byte[] out = new byte[2 + data.length];
            out[0] = (byte) narrow;
            out[1] = (byte) data.length;
            System.arraycopy(data, 0, out, 2, data.length);
            return out;
        }
        byte[] out = new byte[5 + data.length];
        out[0] = (byte) wide;
        for (int i = 0; i < 4; i++) {
            out[1 + i] = (byte) ((data.length >>> (8 * (3 - i))) & 0xFF);
        }
        System.arraycopy(data, 0, out, 5, data.length);
        return out;
    }

    /** §1.6.20 — str8 / str32-utf8. */
    public static byte[] encodeStr(String value) {
        return lengthPrefixed(0xA1, 0xB1, value.getBytes(StandardCharsets.UTF_8));
    }

    /** §1.6.21 — sym8 / sym32. */
    public static byte[] encodeSymbol(String value) {
        return lengthPrefixed(0xA3, 0xB3, value.getBytes(StandardCharsets.US_ASCII));
    }

    /** §1.6.19 — vbin8 / vbin32. */
    public static byte[] encodeBinary(byte[] value) {
        return lengthPrefixed(0xA0, 0xB0, value);
    }

    // -- compound ----------------------------------------------------

    /**
     * §1.6.22 — list0 / list8 / list32.
     *
     * <p>§7.1.7: "An empty sequence or array SHALL be encoded as the
     * list0 type-code 0x45, not as an empty 0xC0."
     */
    public static byte[] encodeList(List<byte[]> items) {
        if (items.isEmpty()) {
            return new byte[] {(byte) 0x45};
        }
        return compound(0xC0, 0xD0, items, items.size());
    }

    /** §1.6.23 — map8 / map32. The count is entries times two. */
    public static byte[] encodeMap(List<byte[]> flattened) {
        return compound(0xC1, 0xD1, flattened, flattened.size());
    }

    private static byte[] compound(int narrow, int wide, List<byte[]> parts, int count) {
        int size = 0;
        for (byte[] part : parts) {
            size += part.length;
        }
        if (size <= 254 && count <= 255) {
            byte[] out = new byte[3 + size];
            out[0] = (byte) narrow;
            out[1] = (byte) (size + 1);
            out[2] = (byte) count;
            int at = 3;
            for (byte[] part : parts) {
                System.arraycopy(part, 0, out, at, part.length);
                at += part.length;
            }
            return out;
        }
        byte[] out = new byte[9 + size];
        out[0] = (byte) wide;
        writeUint(out, 1, size + 4);
        writeUint(out, 5, count);
        int at = 9;
        for (byte[] part : parts) {
            System.arraycopy(part, 0, out, at, part.length);
            at += part.length;
        }
        return out;
    }

    private static void writeUint(byte[] out, int at, int value) {
        for (int i = 0; i < 4; i++) {
            out[at + i] = (byte) ((value >>> (8 * (3 - i))) & 0xFF);
        }
    }

    public static byte[] concat(List<byte[]> parts) {
        int size = 0;
        for (byte[] part : parts) {
            size += part.length;
        }
        byte[] out = new byte[size];
        int at = 0;
        for (byte[] part : parts) {
            System.arraycopy(part, 0, out, at, part.length);
            at += part.length;
        }
        return out;
    }

    /** §7.2.1.1 — `0x00`, a `ulong` descriptor, the body. */
    public static byte[] describedUlong(long descriptor, byte[] body) {
        List<byte[]> parts = new ArrayList<byte[]>();
        parts.add(new byte[] {(byte) 0x00});
        parts.add(encodeUlong(descriptor));
        parts.add(body);
        return concat(parts);
    }

    /** §7.2.1.2 — a `symbol` and the body, as AMQP §1.3.4's two-element list. */
    public static byte[] describedSymbol(String symbol, byte[] body) {
        List<byte[]> items = new ArrayList<byte[]>();
        items.add(encodeSymbol(symbol));
        items.add(body);
        return encodeList(items);
    }

    /** §8.1.2 — the symbolic name of an enumerator. */
    public static String enumName(long ordinal, String[] literals) {
        if (ordinal < 0 || ordinal >= literals.length) {
            throw new IndexOutOfBoundsException("no enumerator has that ordinal");
        }
        return literals[(int) ordinal];
    }

    // -- §8.1.2 JSON -------------------------------------------------

    /** RFC 8259 §7. */
    public static String jsonStr(String value) {
        StringBuilder out = new StringBuilder();
        out.append('"');
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            switch (c) {
                case '"': out.append("\\\""); break;
                case '\\': out.append("\\\\"); break;
                case '\n': out.append("\\n"); break;
                case '\r': out.append("\\r"); break;
                case '\t': out.append("\\t"); break;
                case '\b': out.append("\\b"); break;
                case '\f': out.append("\\f"); break;
                default:
                    if (c < 0x20) {
                        out.append(String.format("\\u%04x", (int) c));
                    } else {
                        out.append(c);
                    }
            }
        }
        out.append('"');
        return out.toString();
    }

    public static String jsonBool(boolean value) {
        return value ? "true" : "false";
    }

    public static String jsonI64(long value) {
        return Long.toString(value);
    }

    public static String jsonU64(long value) {
        return Long.toUnsignedString(value);
    }

    /**
     * §8.1.2 — "Floating-point NaN and infinities are not representable
     * in JSON and SHALL cause the encoder to raise an error."
     */
    public static String jsonDouble(double value) {
        if (Double.isNaN(value) || Double.isInfinite(value)) {
            throw new ArithmeticException("DDS-AMQP 1.0 8.1.2: NaN and infinities are not JSON");
        }
        // The shortest representation that round-trips, which is what
        // the C++, Rust and TypeScript columns write too. `%.17g` would
        // write `1.5000000000000000` where they write `1.5`; RFC 8259
        // admits both and the four are compared against each other.
        String written = Double.toString(value);
        if (written.indexOf('.') < 0 && written.indexOf('e') < 0 && written.indexOf('E') < 0) {
            return written + ".0";
        }
        return written;
    }

    public static String jsonChar(int codepoint) {
        return jsonStr(new String(Character.toChars(codepoint)));
    }

    /** §8.1.2 — "octet sequences: Base64 (RFC 4648) string." */
    public static String jsonBase64(byte[] bytes) {
        return jsonStr(java.util.Base64.getEncoder().encodeToString(bytes));
    }

    /** §8.1.2 — "Time_t: JSON object {"sec": n, "nanosec": m}." */
    public static String jsonTime(long sec, long nanosec) {
        return "{\"sec\":" + sec + ",\"nanosec\":" + nanosec + "}";
    }

    /** §7.1.3 — the standard Time_t mapping, in milliseconds. */
    public static long timeMillis(long sec, long nanosec) {
        return sec * 1000L + nanosec / 1000000L;
    }
}
