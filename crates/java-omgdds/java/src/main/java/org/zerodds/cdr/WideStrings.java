// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.zerodds.wire.Reader;
import org.zerodds.wire.Writer;
import org.zerodds.wire.WstringLengthPolicy;
import org.zerodds.wire.WstringUnitOrder;

/**
 * The wire form of a string of wide characters (a Java {@code String} marked
 * {@code @SerializeAs(STRING_TYPE, collectionElementKind = CHAR_32_TYPE)}).
 *
 * <p>The default is the form of DDS-Java-PSM 1.0 §8.5.1: "each Java code point shall become a
 * single DDS wide character". A DDS wide character is 32 bits (CHAR_32_TYPE), so a string of n code
 * points is the length in octets (4n) and n unsigned 32-bit values, one per code point, without a
 * terminator. A code point above U+FFFF is one character, not a surrogate pair. The setting
 * {@value #PROPERTY}{@code =utf16} selects the wstring of DDS-XTypes 1.3 §7.4.1.1.2, a sequence of
 * UTF-16 units, where such a code point is two units; use it when peers read a {@code wstring}.
 */
final class WideStrings {

    /** The system property that selects the form: {@code char32} (default, §8.5.1) or {@code utf16}. */
    static final String PROPERTY = "zerodds.java.wide-string";

    private WideStrings() {}

    private static boolean utf16() {
        String mode = System.getProperty(PROPERTY, "char32");
        if ("char32".equalsIgnoreCase(mode)) {
            return false;
        }
        if ("utf16".equalsIgnoreCase(mode)) {
            return true;
        }
        throw new XcdrException(PROPERTY + " is '" + mode + "'; it is 'char32' or 'utf16'");
    }

    /** Writes {@code value}; {@code bound > 0} is the bound of a bounded string. */
    static void write(Writer w, String value, int bound) {
        if (utf16()) {
            if (bound > 0) {
                w.writeWstringBounded(value, bound, WstringLengthPolicy.SPECIFICATION,
                        WstringUnitOrder.STREAM);
            } else {
                w.writeWstringUtf8(value, WstringLengthPolicy.SPECIFICATION, WstringUnitOrder.STREAM);
            }
            return;
        }
        int n = value.codePointCount(0, value.length());
        if (bound > 0 && n > bound) {
            throw new XcdrException("a string of at most " + bound + " characters, and this one has "
                    + n);
        }
        w.writeU32(4L * n);
        for (int i = 0; i < value.length(); ) {
            int cp = value.codePointAt(i);
            w.writeU32(cp);
            i += Character.charCount(cp);
        }
    }

    /** Reads a string; {@code bound > 0} is the bound of a bounded string. */
    static String read(Reader r, int bound) {
        if (utf16()) {
            return r.readWstring(WstringLengthPolicy.SPECIFICATION, WstringUnitOrder.STREAM);
        }
        long octets = r.readU32();
        if (octets % 4 != 0 || octets > r.remaining()) {
            throw new XcdrException("a string of " + octets + " octets of 32-bit characters does not fit in"
                    + " the " + r.remaining() + " that are left");
        }
        long n = octets / 4;
        if (bound > 0 && n > bound) {
            throw new XcdrException("a string of at most " + bound + " characters, and this one has " + n);
        }
        StringBuilder text = new StringBuilder((int) n);
        for (long i = 0; i < n; i++) {
            long cp = r.readU32();
            if (cp > 0x10ffffL || (cp >= 0xd800L && cp <= 0xdfffL)) {
                throw new XcdrException("the value " + cp + " is no Unicode code point");
            }
            text.appendCodePoint((int) cp);
        }
        return text.toString();
    }
}
