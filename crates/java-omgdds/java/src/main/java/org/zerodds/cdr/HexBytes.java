// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

/** Strict, separator-free hexadecimal metadata literals, compatible with Java 8. */
public final class HexBytes {
    private static final char[] DIGITS = "0123456789abcdef".toCharArray();
    private HexBytes() {}

    public static String encode(byte[] bytes) {
        java.util.Objects.requireNonNull(bytes, "bytes");
        char[] result = new char[Math.multiplyExact(bytes.length, 2)];
        for (int i = 0; i < bytes.length; i++) {
            int value = bytes[i] & 255;
            result[2 * i] = DIGITS[value >>> 4];
            result[2 * i + 1] = DIGITS[value & 15];
        }
        return new String(result);
    }

    public static byte[] decode(String text) {
        java.util.Objects.requireNonNull(text, "text");
        if ((text.length() & 1) != 0) throw new IllegalArgumentException("Odd hexadecimal length");
        byte[] result = new byte[text.length() / 2];
        for (int i = 0; i < result.length; i++) {
            result[i] = (byte) ((digit(text.charAt(2 * i)) << 4) | digit(text.charAt(2 * i + 1)));
        }
        return result;
    }

    private static int digit(char value) {
        if (value >= '0' && value <= '9') return value - '0';
        if (value >= 'a' && value <= 'f') return value - 'a' + 10;
        if (value >= 'A' && value <= 'F') return value - 'A' + 10;
        throw new IllegalArgumentException("Non-hexadecimal character");
    }
}
