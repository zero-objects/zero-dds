// SPDX-License-Identifier: Apache-2.0
package org.zerodds.cdr;

import org.zerodds.wire.Reader;
import org.zerodds.wire.Writer;

/**
 * The wire form of a narrow {@code string} ({@code String<Char8>}) in the Java
 * Type Representation.
 *
 * <p>The default is the form of DDS-Java-PSM 1.0 §8.5.1: "each Java character shall be truncated
 * to its least-significant byte". A string of n characters is n octets and the terminator,
 * whatever the characters are; reading gives back one character per octet (U+0000 to U+00FF). It
 * loses the bits above the low octet of a character above U+00FF; that is the clause. The setting
 * {@value #PROPERTY}{@code =utf8} selects the other form: UTF-8, as DDS-XTypes 1.3 §7.2.2.2.1.2.4
 * and §7.4.1.1.2 have it ("The default encoding for String&lt;Char8&gt; shall be UTF-8") and as
 * the interoperability corpus ({@code tests/interop-corpus}: <code>été</code> is
 * <code>c3 a9 74 c3 a9</code>) and the code the IDL-Java generator emits write it. Use it when
 * peers read the strings as UTF-8. The two forms are the same for characters up to U+007F.
 *
 * <p>A single IDL {@code char} is one octet in both settings (XTypes 1.3 §7.4.1.1.2: serialized
 * like a byte).
 */
final class NarrowStrings {

    /** The system property that selects the form: {@code truncate} (default, §8.5.1) or {@code utf8}. */
    static final String PROPERTY = "zerodds.java.narrow-string";

    private NarrowStrings() {}

    private static boolean utf8() {
        String mode = System.getProperty(PROPERTY, "truncate");
        if ("utf8".equalsIgnoreCase(mode)) {
            return true;
        }
        if ("truncate".equalsIgnoreCase(mode)) {
            return false;
        }
        throw new XcdrException(PROPERTY + " is '" + mode + "'; it is 'utf8' or 'truncate'");
    }

    /** Writes {@code value}; {@code bound > 0} is the bound of a bounded string. */
    static void write(Writer w, String value, int bound) {
        if (utf8()) {
            if (bound > 0) {
                w.writeStringBounded(value, bound);
            } else {
                w.writeString(value);
            }
            return;
        }
        int n = value.length();
        if (bound > 0 && n > bound) {
            throw new XcdrException("a string of at most " + bound + " characters, and this one has "
                    + n);
        }
        w.writeU32(n + 1L);
        for (int i = 0; i < n; i++) {
            w.writeU8(value.charAt(i) & 0xff);
        }
        w.writeU8(0);
    }

    /** Reads a string; {@code bound > 0} is the bound of a bounded string. */
    static String read(Reader r, int bound) {
        boolean utf8 = utf8();
        long length = r.readU32();
        if (length < 1 || length > r.remaining()) {
            throw new XcdrException("a string of " + length + " octets does not fit in the "
                    + r.remaining() + " that are left");
        }
        if (bound > 0 && length - 1 > bound) {
            throw new XcdrException("a string of at most " + bound + " characters, and this one has "
                    + (length - 1));
        }
        if (utf8) {
            // the characters are UTF-8 (the octets of the string), refused when malformed
            byte[] octets = new byte[(int) length - 1];
            for (int i = 0; i < octets.length; i++) {
                octets[i] = (byte) r.readU8();
            }
            if (r.readU8() != 0) {
                throw new XcdrException("a string is not terminated by a NUL octet");
            }
            try {
                return java.nio.charset.StandardCharsets.UTF_8.newDecoder()
                        .onMalformedInput(java.nio.charset.CodingErrorAction.REPORT)
                        .onUnmappableCharacter(java.nio.charset.CodingErrorAction.REPORT)
                        .decode(java.nio.ByteBuffer.wrap(octets)).toString();
            } catch (java.nio.charset.CharacterCodingException malformed) {
                throw new XcdrException("a string is not UTF-8", malformed);
            }
        }
        StringBuilder text = new StringBuilder((int) length - 1);
        for (long i = 0; i < length - 1; i++) {
            text.append((char) r.readU8());
        }
        if (r.readU8() != 0) {
            throw new XcdrException("a string is not terminated by a NUL octet");
        }
        return text.toString();
    }
}
