// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

/**
 * The Java column against a foreign answer.
 *
 * <p>{@code WireSelfTest} asserts what this runtime believes. That is worth
 * exactly as much as the belief, which is why the interop corpus exists: three
 * cases whose types are pure primitives are written here by hand from
 * {@code tests/interop-corpus/&lt;case&gt;/type.idl} and
 * {@code values.json}, and the bytes are compared against what the Rust column
 * recorded in {@code tests/interop-results/&lt;case&gt;/zerodds-rust/}.
 *
 * <p>Two encodings each, chosen because they disagree about the one rule this
 * column is most likely to have got wrong:
 *
 * <ul>
 *   <li>{@code plain-cdr2.le.bin} — MAXALIGN 4, so an eight-byte field aligns
 *       to four;</li>
 *   <li>{@code plain-cdr1.be.bin} — MAXALIGN 8 and the other byte order, so
 *       the same field aligns to eight and every multi-byte value is
 *       reversed.</li>
 * </ul>
 *
 * <p>Neither file carries an encapsulation header: they are the payload alone.
 *
 * <p>Run from the repository root. Silent when every case matches; on a
 * mismatch it names the case, the offset of the first differing byte, and both
 * bytes.
 */
public final class CorpusCrossCheck {

    private static int failures = 0;

    private CorpusCrossCheck() {
    }

    /** Where the recorded bytes are, relative to the repository root. */
    private static final String RESULTS = "tests/interop-results";

    // -- the three cases ---------------------------------------------------

    /**
     * {@code primitive-mixed}: every primitive in one aggregate, declared in
     * ascending alignment order.
     */
    private static void primitiveMixed(Writer w) {
        w.writeBool(true);
        w.writeU8(171);
        w.writeU8(254);
        w.writeI8(-13);
        w.writeChar('Z');
        w.writeWchar(0x00E4, WcharWidth.OCTETS_2, WcharFraming.FIXED, WstringUnitOrder.STREAM);
        w.writeI16(-12345);
        w.writeU16(48879);
        w.writeI32(-1234567);
        w.writeU32(3735928559L);
        w.writeI64(-987654321987L);
        w.writeU64(Long.parseUnsignedLong("16045690984833335998"));
        w.writeF32(-3.75);
        w.writeF64(3.141592653589793);
    }

    /** {@code primitive-longlong}: two {@code long long} members. */
    private static void primitiveLonglong(Writer w) {
        w.writeI64(-987654321987L);
        w.writeI64(9223372036854775807L);
    }

    /**
     * {@code primitive-mixed-descending-alignment}: the same primitives,
     * widest first, which is the layout that puts the padding at the end
     * instead of between the members.
     */
    private static void primitiveMixedDescending(Writer w) {
        w.writeF64(-2.5);
        w.writeF32(2.5);
        w.writeU64(7L);
        w.writeI64(9223372036854775807L);
        w.writeU32(5L);
        w.writeI32(2147483647);
        w.writeU16(3);
        w.writeI16(32767);
        w.writeWchar(0x1234, WcharWidth.OCTETS_2, WcharFraming.FIXED, WstringUnitOrder.STREAM);
        w.writeChar('~');
        w.writeI8(127);
        w.writeU8(3);
        w.writeU8(1);
        w.writeBool(false);
    }

    private static void write(String name, Writer w) {
        if ("primitive-mixed".equals(name)) {
            primitiveMixed(w);
        } else if ("primitive-longlong".equals(name)) {
            primitiveLonglong(w);
        } else if ("primitive-mixed-descending-alignment".equals(name)) {
            primitiveMixedDescending(w);
        } else {
            throw new IllegalArgumentException("no hand-written encoder for " + name);
        }
    }

    // -- the comparison ----------------------------------------------------

    private static void compare(String name, Encoding encoding, Endianness order)
            throws IOException {
        String file = encoding.asString() + "." + order.asString() + ".bin";
        Path path = Paths.get(RESULTS, name, "zerodds-rust", file);
        if (!Files.isReadable(path)) {
            System.out.println("FAIL " + name + " " + file + ": not readable at "
                    + path.toAbsolutePath());
            failures++;
            return;
        }
        byte[] expected = Files.readAllBytes(path);
        Writer writer = new Writer(order, encoding.maxAlignment(), null, false);
        write(name, writer);
        byte[] actual = writer.finish();

        int limit = Math.min(expected.length, actual.length);
        for (int at = 0; at < limit; at++) {
            if (expected[at] != actual[at]) {
                System.out.println("FAIL " + name + " " + file
                        + ": first difference at offset " + at
                        + ": Java " + hex(actual[at]) + ", Rust " + hex(expected[at]));
                System.out.println("     Java " + hex(actual));
                System.out.println("     Rust " + hex(expected));
                failures++;
                return;
            }
        }
        if (expected.length != actual.length) {
            System.out.println("FAIL " + name + " " + file + ": Java wrote " + actual.length
                    + " bytes, Rust " + expected.length + "; they agree to offset " + limit);
            System.out.println("     Java " + hex(actual));
            System.out.println("     Rust " + hex(expected));
            failures++;
        }
    }

    private static String hex(byte value) {
        return "" + Character.forDigit((value >> 4) & 0xF, 16) + Character.forDigit(value & 0xF, 16);
    }

    private static String hex(byte[] data) {
        StringBuilder out = new StringBuilder(data.length * 2);
        for (byte value : data) {
            out.append(hex(value));
        }
        return out.toString();
    }

    public static void main(String[] args) throws IOException {
        String[] cases = {
            "primitive-mixed", "primitive-longlong", "primitive-mixed-descending-alignment",
        };
        for (String name : cases) {
            compare(name, Encoding.PLAIN_CDR2, Endianness.LITTLE);
            compare(name, Encoding.PLAIN_CDR1, Endianness.BIG);
        }
        if (failures != 0) {
            System.out.println(failures + " case(s) do not match the Rust column");
            System.exit(1);
        }
    }
}
