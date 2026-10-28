// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

import java.util.Arrays;

/**
 * What the Java wire runtime has to do before generated code can use it.
 *
 * <p>No test framework: this runtime has no dependencies and adding JUnit to
 * assert a few hundred lines of framing would be the first one. A {@code main}
 * that returns non-zero is what {@code runtime/java/run-tests.sh} and the CI
 * job need, and silence is what it says when everything held.
 *
 * <p>The checks are grouped by the trap they exist for, not by the method they
 * call. The alignment ones come first because that trap has been hit four
 * times in this work: {@code ALIGN(N)} is {@code MIN(N, MAXALIGN)} counted
 * from the stream's <b>origin</b>, and a column that caps it at four
 * unconditionally reads every eight-byte field of an XCDR1 sample wrong.
 */
public final class WireSelfTest {

    private static int failures = 0;

    private static void check(boolean condition, String what) {
        if (!condition) {
            System.out.println("FAIL " + what);
            failures++;
        }
    }

    private static void equal(long actual, long expected, String what) {
        if (actual != expected) {
            System.out.println("FAIL " + what + ": " + actual + " is not " + expected);
            failures++;
        }
    }

    private static void equal(Object actual, Object expected, String what) {
        if (actual == null ? expected != null : !actual.equals(expected)) {
            System.out.println("FAIL " + what + ": " + actual + " is not " + expected);
            failures++;
        }
    }

    private static void bytesEqual(byte[] actual, String expected, String what) {
        String got = hex(actual);
        if (!got.equals(expected)) {
            System.out.println("FAIL " + what + ": " + got + " is not " + expected);
            failures++;
        }
    }

    private static String hex(byte[] data) {
        StringBuilder out = new StringBuilder(data.length * 2);
        for (byte value : data) {
            out.append(Character.forDigit((value >> 4) & 0xF, 16));
            out.append(Character.forDigit(value & 0xF, 16));
        }
        return out.toString();
    }

    private static void raises(Runnable body, String what) {
        try {
            body.run();
        } catch (WireException expected) {
            return;
        }
        System.out.println("FAIL " + what + ": nothing was raised");
        failures++;
    }

    // -- alignment ---------------------------------------------------------

    /**
     * An {@code octet} is a Java {@code byte} (CORBA 3.3 part 3 §1.5.2): the byte -56 is the
     * number 200 on the wire, and a number outside [0, 255] is still refused.
     */
    private static void anOctetIsAByteWrittenAsItsEightBits() {
        Writer w = new Writer(Endianness.LITTLE, 4, null, false);
        w.writeU8((byte) -56);
        w.writeU8((byte) 7);
        w.writeU8(200);
        bytesEqual(w.finish(), "c807c8", "a byte and an int write the same octet");
        boolean refused = false;
        try {
            new Writer(Endianness.LITTLE, 4, null, false).writeU8(256);
        } catch (WireException expected) {
            refused = true;
        }
        equal(refused ? 1 : 0, 1, "256 is no octet");
    }

    /**
     * The trap, at both caps and in both directions.
     *
     * <p>An eight-byte field after a one-byte field sits at 8 under XCDR1 and
     * at 4 under XCDR2, because {@code ALIGN(8)} is {@code MIN(8, MAXALIGN)}.
     * A column that hard-capped the cap at four — which
     * {@code endpoints/java/Zdw.java} does — writes the XCDR1 sample four
     * bytes short and every peer reads the wrong number.
     */
    private static void anEightByteFieldAfterAOneByteFieldSitsWhereMaxalignSays() {
        Writer one = new Writer(Endianness.LITTLE, 8, null, false);
        one.writeU8(0xAB);
        one.writeI64(0x0102030405060708L);
        equal(one.position(), 16, "XCDR1 pads a long long to eight");
        bytesEqual(one.finish(), "ab00000000000000" + "0807060504030201",
                "XCDR1's eight-byte alignment");

        Writer two = new Writer(Endianness.LITTLE, 4, null, false);
        two.writeU8(0xAB);
        two.writeI64(0x0102030405060708L);
        equal(two.position(), 12, "XCDR2 pads a long long to four");
        bytesEqual(two.finish(), "ab000000" + "0807060504030201",
                "XCDR2's four-byte cap");

        // And the reader has to agree, or it reads from the padding on.
        Reader readOne = new Reader(one.finish(), Endianness.LITTLE, 8, null,
                Writer.PID_SHORT_MAX_ID);
        equal(readOne.readU8(), 0xAB, "the octet at the head of an XCDR1 stream");
        equal(readOne.readI64(), 0x0102030405060708L, "the long long behind it");
        Reader readTwo = new Reader(two.finish(), Endianness.LITTLE, 4, null,
                Writer.PID_SHORT_MAX_ID);
        equal(readTwo.readU8(), 0xAB, "the octet at the head of an XCDR2 stream");
        equal(readTwo.readI64(), 0x0102030405060708L, "the long long behind it");

        // A reader at the wrong cap reads a different number, which is the
        // failure this check exists to make impossible to ship quietly.
        Reader wrong = new Reader(one.finish(), Endianness.LITTLE, 4, null,
                Writer.PID_SHORT_MAX_ID);
        wrong.readU8();
        check(wrong.readI64() != 0x0102030405060708L,
                "a reader at MAXALIGN 4 must not accidentally agree with an XCDR1 stream");

        // Every width, at both caps, after one byte.
        int[] widths = {2, 4, 8};
        for (int width : widths) {
            for (int cap : new int[] {4, 8}) {
                Writer writer = new Writer(Endianness.BIG, cap, null, false);
                writer.writeU8(1);
                writer.alignTo(width);
                equal(writer.position(), Math.min(width, cap),
                        "align " + width + " at cap " + cap);
            }
        }
    }

    /**
     * Alignment is counted from the origin, and a PL_CDR1 parameter's value
     * moves it. §7.4.1.2: a parameter is its own CDR-encapsulated block, so an
     * eight-byte member at the head of one takes no padding inside its own
     * length — and {@code finishPid} puts the origin back <i>before</i>
     * aligning to four, so the padding between parameters is measured from the
     * enclosing stream.
     */
    private static void aParameterValueMovesTheOriginAndFinishPutsItBack() {
        Writer writer = new Writer(Endianness.LITTLE, 8, null, false);
        Writer.Pid header = writer.writePid(1, ParameterRole.MEMBER, false);
        equal(writer.position(), 4, "a short parameter header is four bytes");
        writer.writeI64(1);
        equal(writer.position(), 12,
                "an eight-byte member at the head of a parameter block takes no padding");
        writer.finishPid(header);
        bytesEqual(writer.finish(), "01000800" + "0100000000000000",
                "the parameter and its declared length");

        // The same block one byte in: now the member does need padding, and it
        // is counted from the block and not from the stream.
        Writer inset = new Writer(Endianness.LITTLE, 8, null, false);
        Writer.Pid two = inset.writePid(2, ParameterRole.MEMBER, false);
        inset.writeU8(0xAB);
        inset.writeI64(1);
        equal(inset.position(), 20, "seven bytes of padding inside the block");
        inset.finishPid(two);

        // A one-byte member: XTypes 1.3 §7.4.1.2 declares its exact length,
        // and the next parameter starts four-aligned.
        Writer odd = new Writer(Endianness.LITTLE, 8, null, false);
        Writer.Pid three = odd.writePid(1, ParameterRole.MEMBER, false);
        odd.writeU8(0xAB);
        odd.finishPid(three);
        bytesEqual(odd.finish(), "01000100" + "ab000000",
                "an exact declared length and the padding behind the value");
        equal(odd.position(), 8, "the next parameter starts four-aligned");

        // The length rounded up to four (DDSI-RTPS 2.5 §9.4.2.11, Cyclone DDS,
        // RTI Connext) is the named option.
        PlCdr1Framing padded = PlCdr1Framing.defaults().withPadDeclaredLength(true);
        Writer rounded = new Writer(Endianness.LITTLE, 8, padded, false);
        Writer.Pid roundedHeader = rounded.writePid(1, ParameterRole.MEMBER, false);
        rounded.writeU8(0xAB);
        rounded.finishPid(roundedHeader);
        bytesEqual(rounded.finish(), "01000400" + "ab000000", "a padded declared length");

        // Fast DDS does not pad the declared length either; the padding is
        // still written, because it is the next parameter's alignment.
        PlCdr1Framing unpadded = PlCdr1Framing.defaults().withPadDeclaredLength(false);
        Writer bare = new Writer(Endianness.LITTLE, 8, unpadded, false);
        Writer.Pid four = bare.writePid(1, ParameterRole.MEMBER, false);
        bare.writeU8(0xAB);
        bare.finishPid(four);
        bytesEqual(bare.finish(), "01000100" + "ab000000", "an unpadded declared length");

        // The reader's half.
        Reader reader = new Reader(odd.finish(), Endianness.LITTLE, 8, null,
                Writer.PID_SHORT_MAX_ID);
        Reader.PidRead read = reader.readPid();
        equal(read.pid(), 1, "the member id");
        equal(read.length(), 1, "the declared length");
        int saved = reader.beginParameterValue();
        equal(reader.readU8(), 0xAB, "the member's value");
        reader.finishParameterValue(saved);
        equal(reader.position(), 8, "the next parameter's offset");
    }

    // -- primitives --------------------------------------------------------

    /**
     * Every primitive, both orders, out and back — including
     * {@code 0xFFFFFFFFFFFFFFFF}, which Java has no number for.
     */
    private static void everyPrimitiveSurvivesBothByteOrders() {
        for (Endianness order : Endianness.values()) {
            Writer writer = new Writer(order, 8, null, false);
            writer.writeBool(true);
            writer.writeU8(254);
            writer.writeI8(-13);
            writer.writeChar('Z');
            writer.writeWchar(0x00E4, WcharWidth.OCTETS_2, WcharFraming.FIXED,
                    WstringUnitOrder.STREAM);
            writer.writeI16(-12345);
            writer.writeU16(48879);
            writer.writeI32(-1234567);
            writer.writeU32(3735928559L);
            writer.writeI64(-987654321987L);
            writer.writeU64(0xFFFFFFFFFFFFFFFFL);
            writer.writeF32(-3.75);
            writer.writeF64(3.141592653589793);
            byte[] out = writer.finish();

            Reader reader = new Reader(out, order, 8, null, Writer.PID_SHORT_MAX_ID);
            check(reader.readBool(), "boolean in " + order);
            equal(reader.readU8(), 254, "octet in " + order);
            equal(reader.readI8(), -13, "int8 in " + order);
            equal(reader.readChar(), 'Z', "char in " + order);
            equal(reader.readWchar(WcharWidth.OCTETS_2, WcharFraming.FIXED,
                    WstringUnitOrder.STREAM), 0x00E4, "wchar in " + order);
            equal(reader.readI16(), -12345, "short in " + order);
            equal(reader.readU16(), 48879, "unsigned short in " + order);
            equal(reader.readI32(), -1234567, "long in " + order);
            equal(reader.readU32(), 3735928559L, "unsigned long in " + order);
            equal(reader.readI64(), -987654321987L, "long long in " + order);
            equal(reader.readU64(), 0xFFFFFFFFFFFFFFFFL,
                    "unsigned long long at its maximum in " + order);
            check(reader.readF32() == -3.75f, "float in " + order);
            check(reader.readF64() == 3.141592653589793, "double in " + order);
            equal(reader.remaining(), 0, "the stream is consumed in " + order);
        }

        // The maximum is the pattern and not a number: `writeU64(-1L)` is
        // 18446744073709551615 on the wire, and the two orders are reversals
        // of one another.
        Writer little = new Writer(Endianness.LITTLE, 8, null, false);
        little.writeU64(0xFFFFFFFFFFFFFFFFL);
        bytesEqual(little.finish(), "ffffffffffffffff", "u64 at its maximum");
        equal(Long.toUnsignedString(0xFFFFFFFFFFFFFFFFL), "18446744073709551615",
                "the unsigned spelling of the pattern");

        Writer big = new Writer(Endianness.BIG, 8, null, false);
        big.writeU64(0x0102030405060708L);
        bytesEqual(big.finish(), "0102030405060708", "u64 big-endian");
    }

    /** A value a field cannot hold is refused, never masked. */
    private static void anOverWideValueIsRefusedAndNotMasked() {
        final Writer writer = new Writer(Endianness.LITTLE, 8, null, false);
        raises(new Runnable() {
            @Override
            public void run() {
                writer.writeU8(256);
            }
        }, "an octet of 256");
        raises(new Runnable() {
            @Override
            public void run() {
                writer.writeU16(70000);
            }
        }, "an unsigned short of 70000");
        raises(new Runnable() {
            @Override
            public void run() {
                writer.writeU32(0x1_0000_0000L);
            }
        }, "an unsigned long of 2^32");
        raises(new Runnable() {
            @Override
            public void run() {
                writer.writeU32(-1L);
            }
        }, "an unsigned long of -1");
        raises(new Runnable() {
            @Override
            public void run() {
                writer.writeI16(40000);
            }
        }, "a short of 40000");
        raises(new Runnable() {
            @Override
            public void run() {
                writer.writeChar(0xE4);
                writer.writeChar(0x100);
            }
        }, "a char above U+00FF");
        equal(writer.position(), 1, "nothing was written by a refused call");
    }

    /** {@code writeF32} rounds once, and an overflow becomes an infinity. */
    private static void aFloatIsRoundedOnceAndOverflowsToInfinity() {
        Writer writer = new Writer(Endianness.BIG, 8, null, false);
        writer.writeF32(1e300);
        writer.writeF32(-1e300);
        writer.writeF32(3.141592653589793);
        bytesEqual(writer.finish(), "7f800000" + "ff800000" + "40490fdb",
                "the single-precision patterns");
        Reader reader = new Reader(writer.finish(), Endianness.BIG, 8, null,
                Writer.PID_SHORT_MAX_ID);
        check(reader.readF32() == Float.POSITIVE_INFINITY, "an overflow reads back as infinity");
        check(reader.readF32() == Float.NEGATIVE_INFINITY, "and the negative one too");
        check(reader.readF32() == (float) 3.141592653589793, "one rounding and not two");
    }

    // -- text --------------------------------------------------------------

    /**
     * A string's length counts the NUL and counts octets of UTF-8, not
     * {@code String.length()}.
     */
    private static void aStringLengthIsOctetsPlusTheTerminator() {
        Writer writer = new Writer(Endianness.LITTLE, 8, null, false);
        writer.writeString("äZ");
        // Two characters, three octets, four with the terminator.
        bytesEqual(writer.finish(), "04000000" + "c3a45a" + "00",
                "a string whose octet count is not its character count");

        Reader reader = new Reader(writer.finish(), Endianness.LITTLE, 8, null,
                Writer.PID_SHORT_MAX_ID);
        equal(reader.readString(), "äZ", "the string comes back");
        equal(reader.remaining(), 0, "including its terminator");

        // An over-bound value is refused rather than cut at a character
        // boundary: the bound is a promise about the source and not a
        // difference on the wire, and cutting it would put a sample on
        // the wire nobody wrote.
        final Writer bounded = new Writer(Endianness.LITTLE, 8, null, false);
        raises(new Runnable() {
            @Override
            public void run() {
                bounded.writeStringBounded("abcdefgh", 5);
            }
        }, "a string longer than its bound");

        // Never cut inside a character either: "äää" is six octets and a
        // bound of five refuses the same way.
        final Writer cut = new Writer(Endianness.LITTLE, 8, null, false);
        raises(new Runnable() {
            @Override
            public void run() {
                cut.writeStringBounded("äää", 5);
            }
        }, "a string whose bound would cut a character");

        // Exactly at the bound still writes the same bytes an unbounded
        // string would.
        Writer atBound = new Writer(Endianness.LITTLE, 8, null, false);
        atBound.writeStringBounded("ää", 4);
        bytesEqual(atBound.finish(), "05000000" + "c3a4c3a4" + "00",
                "a string exactly at its bound encodes unchanged");

        // Java's own encoder would put a `?` here and report success.
        final Writer lone = new Writer(Endianness.LITTLE, 8, null, false);
        raises(new Runnable() {
            @Override
            public void run() {
                lone.writeString("a\uD834b");
            }
        }, "a lone surrogate in a string");
    }

    /**
     * A {@code wstring}'s units are UTF-16 — which a Java {@code String}
     * already holds — but the length field under
     * {@link WstringLengthPolicy#SPECIFICATION} is the octet count, and a code
     * point outside the BMP is two units and four octets.
     */
    private static void aWstringCountsOctetsAndCarriesASurrogatePair() {
        Writer writer = new Writer(Endianness.LITTLE, 8, null, false);
        writer.writeWstringUtf8("a𝄞b", WstringLengthPolicy.SPECIFICATION,
                WstringUnitOrder.STREAM);
        // The bytes `runtime/c/include/zerodds_wire.h` records for this
        // project against TAO's four-octet form.
        bytesEqual(writer.finish(), "08000000" + "6100" + "34d8" + "1edd" + "6200",
                "a non-BMP wstring under the specification policy");

        Reader reader = new Reader(writer.finish(), Endianness.LITTLE, 8, null,
                Writer.PID_SHORT_MAX_ID);
        equal(reader.readWstring(WstringLengthPolicy.SPECIFICATION, WstringUnitOrder.STREAM),
                "a𝄞b", "the surrogate pair comes back as one code point");

        // Fast DDS counts characters over two-octet units, which cannot carry
        // a code point outside the BMP. The length convention is a policy; the
        // data loss is not.
        final Writer counted = new Writer(Endianness.LITTLE, 8, null, false);
        raises(new Runnable() {
            @Override
            public void run() {
                counted.writeWstringUtf8("a𝄞b", WstringLengthPolicy.CHARACTER_COUNT,
                        WstringUnitOrder.STREAM);
            }
        }, "a non-BMP code point under a two-octet character count");
        counted.writeWstringUtf8("ab", WstringLengthPolicy.CHARACTER_COUNT,
                WstringUnitOrder.STREAM);
        bytesEqual(counted.finish(), "02000000" + "6100" + "6200",
                "a character count over two-octet units");

        // TAO's four-octet form, and RTI's terminated one.
        Writer wide = new Writer(Endianness.LITTLE, 8, null, false);
        wide.writeWstringUtf8("a𝄞b", WstringLengthPolicy.SPECIFICATION_WIDE_UNITS,
                WstringUnitOrder.STREAM);
        bytesEqual(wide.finish(),
                "10000000" + "61000000" + "34d80000" + "1edd0000" + "62000000",
                "TAO's four-octet transmission code set");

        Writer terminated = new Writer(Endianness.LITTLE, 8, null, false);
        terminated.writeWstringUtf8("ab", WstringLengthPolicy.CHARACTER_COUNT_TERMINATED,
                WstringUnitOrder.STREAM);
        bytesEqual(terminated.finish(), "03000000" + "61000000" + "62000000" + "00000000",
                "a terminated character count over four-octet units");
        Reader terminatedBack = new Reader(terminated.finish(), Endianness.LITTLE, 8, null,
                Writer.PID_SHORT_MAX_ID);
        equal(terminatedBack.readWstring(WstringLengthPolicy.CHARACTER_COUNT_TERMINATED,
                WstringUnitOrder.STREAM), "ab", "the terminator is not a character");

        // A lone surrogate is a `char` Java admits and is not a code point.
        final Writer lone = new Writer(Endianness.LITTLE, 8, null, false);
        raises(new Runnable() {
            @Override
            public void run() {
                lone.writeWstringUtf8("a\uDD1E", WstringLengthPolicy.SPECIFICATION,
                        WstringUnitOrder.STREAM);
            }
        }, "a lone low surrogate in a wstring");
        raises(new Runnable() {
            @Override
            public void run() {
                lone.writeWstringUtf8("\uD834a", WstringLengthPolicy.SPECIFICATION,
                        WstringUnitOrder.STREAM);
            }
        }, "a lone high surrogate in a wstring");

        // omniORB's mark, and the swapped mark a big-endian peer writes.
        Writer marked = new Writer(Endianness.LITTLE, 8, null, false);
        marked.writeWstringUtf8("ab", WstringLengthPolicy.SPECIFICATION,
                WstringUnitOrder.BYTE_ORDER_MARK);
        bytesEqual(marked.finish(), "06000000" + "fffe" + "6100" + "6200",
                "a byte-order mark inside the counted octets");
        Reader markedBack = new Reader(marked.finish(), Endianness.LITTLE, 8, null,
                Writer.PID_SHORT_MAX_ID);
        equal(markedBack.readWstring(WstringLengthPolicy.SPECIFICATION,
                WstringUnitOrder.BYTE_ORDER_MARK), "ab", "the mark is not a character");
    }

    /** {@code fixed<P,S>} as packed BCD, both counts. [CDR] §9.3.2.8. */
    private static void aFixedIsPackedBcdWithASignNibble() {
        Writer writer = new Writer(Endianness.LITTLE, 8, null, false);
        writer.writeFixed("-3.14", 5, 2, FixedDigitCount.DECLARED);
        bytesEqual(writer.finish(), "00314d", "a negative fixed<5,2>");

        Writer zeros = new Writer(Endianness.LITTLE, 8, null, false);
        zeros.writeFixed("0.01", 5, 2, FixedDigitCount.DECLARED);
        bytesEqual(zeros.finish(), "00001c", "a leading-zero fixed<5,2>");

        Writer precision = new Writer(Endianness.LITTLE, 8, null, false);
        precision.writeFixed("0.01", 5, 2, FixedDigitCount.VALUE_PRECISION);
        bytesEqual(precision.finish(), "1c", "the value's own precision");

        Writer zero = new Writer(Endianness.LITTLE, 8, null, false);
        zero.writeFixed("-0.00", 5, 2, FixedDigitCount.DECLARED);
        bytesEqual(zero.finish(), "00000c", "there is one zero and it is positive");

        // An odd nibble count takes a leading zero nibble so the nibbles pair.
        Writer odd = new Writer(Endianness.LITTLE, 8, null, false);
        odd.writeFixed("12.34", 4, 2, FixedDigitCount.DECLARED);
        bytesEqual(odd.finish(), "01234c", "four digits and a sign need a leading zero");

        String[][] cases = {
            {"-3.14", "5", "2"}, {"0.01", "5", "2"}, {"12.34", "4", "2"},
            {"0.00", "5", "2"}, {"999.99", "5", "2"}, {"7", "3", "0"},
        };
        for (String[] one : cases) {
            int digits = Integer.parseInt(one[1]);
            int scale = Integer.parseInt(one[2]);
            for (FixedDigitCount count : FixedDigitCount.values()) {
                Writer out = new Writer(Endianness.LITTLE, 8, null, false);
                out.writeFixed(one[0], digits, scale, count);
                Reader back = new Reader(out.finish(), Endianness.LITTLE, 8, null,
                        Writer.PID_SHORT_MAX_ID);
                equal(back.readFixed(digits, scale, count), one[0],
                        "fixed<" + digits + "," + scale + "> " + one[0] + " under " + count);
            }
        }

        final Writer refuses = new Writer(Endianness.LITTLE, 8, null, false);
        raises(new Runnable() {
            @Override
            public void run() {
                refuses.writeFixed("1234", 5, 2, FixedDigitCount.DECLARED);
            }
        }, "an integer part wider than the type");
        raises(new Runnable() {
            @Override
            public void run() {
                refuses.writeFixed("1.234", 5, 2, FixedDigitCount.DECLARED);
            }
        }, "a fraction wider than the scale");

        // ACE/TAO 2.5.24's ACE_CDR::Fixed::from_string, recorded in
        // crates/ast-layout/tests/fixtures/unspecified_constructs.md §3: the
        // integer digits as written plus S, made odd by one leading zero.
        String[][] ace = {
            {"123.45", "5", "2", "12345c"}, {"-123.45", "5", "2", "12345d"},
            {"12.34", "4", "2", "01234c"}, {"7", "1", "0", "7c"},
            {"0.12345", "5", "5", "0012345c"}, {"0.00", "5", "2", "000c"},
            // The scale and not the fraction as written: one digit counted
            // would keep only the last nibble, `0c`.
            {"7", "5", "2", "700c"},
        };
        for (String[] one : ace) {
            Writer out = new Writer(Endianness.LITTLE, 8, null, false);
            out.writeFixed(one[0], Integer.parseInt(one[1]), Integer.parseInt(one[2]),
                    FixedDigitCount.VALUE_STRING_DIGITS);
            bytesEqual(out.finish(), one[3], "fixed " + one[0] + " as ACE writes it");
        }
        raises(new Runnable() {
            @Override
            public void run() {
                refuses.writeFixed("00000000000000000000000000000001", 31, 0,
                        FixedDigitCount.VALUE_STRING_DIGITS);
            }
        }, "thirty-two digits as written");
    }

    // -- framing -----------------------------------------------------------

    /** A DHEADER is patched with the length of what followed it. */
    private static void aDheaderCarriesTheLengthOfWhatFollows() {
        Writer writer = new Writer(Endianness.LITTLE, 4, null, false);
        Writer.DHeader header = writer.writeDheader();
        writer.writeU32(0x11223344L);
        writer.writeU8(0xAB);
        writer.finishDheader(header);
        bytesEqual(writer.finish(), "05000000" + "44332211" + "ab",
                "a DHEADER of five bytes");

        Reader reader = new Reader(writer.finish(), Endianness.LITTLE, 4, null,
                Writer.PID_SHORT_MAX_ID);
        equal(reader.readDheader(), 5, "the announced length");

        // A DHEADER does not move the origin: XCDR counts alignment from the
        // encapsulation, not from a nested value's start (§7.4.1.1).
        Writer nested = new Writer(Endianness.LITTLE, 8, null, false);
        nested.writeU8(1);
        Writer.DHeader inner = nested.writeDheader();
        nested.writeI64(1);
        nested.finishDheader(inner);
        equal(nested.position(), 16, "a long long inside a DHEADER aligns to the stream");
    }

    /**
     * Every EMHEADER length code, including the three whose NEXTINT is the
     * member's own leading word and the LC4 that gets rewritten short.
     */
    private static void everyEmheaderLengthCodeIsFramedAsTheClauseSays() {
        // LC0 to LC3 carry the length in the code and reserve nothing.
        for (int code = 0; code <= 3; code++) {
            Writer writer = new Writer(Endianness.LITTLE, 4, null, false);
            Writer.EmHeader header = writer.writeEmheader(7, code, true);
            check(!header.hasNextInt(), "LC" + code + " reserves no NEXTINT");
            int size = 1 << code;
            for (int index = 0; index < size; index++) {
                writer.writeU8(0xAB);
            }
            writer.finishEmheader(header);
            equal(writer.position(), 4 + size, "LC" + code + " writes only the word");
            long word = 0x80000000L | ((long) code << 28) | 7L;
            bytesEqual(Arrays.copyOf(writer.finish(), 4),
                    hexOf(word, true), "the LC" + code + " header word");

            Reader reader = new Reader(writer.finish(), Endianness.LITTLE, 4, null,
                    Writer.PID_SHORT_MAX_ID);
            Reader.EmHeaderRead read = reader.readEmheader();
            equal(read.memberId(), 7, "the member id under LC" + code);
            equal(read.lengthCode(), code, "the length code");
            check(read.mustUnderstand(), "the must-understand bit");
            reader.skipMember(read);
            equal(reader.position(), 4 + size, "skipping an LC" + code + " member");
        }

        // LC4 reserves a NEXTINT and it is patched with the member's length.
        Writer four = new Writer(Endianness.LITTLE, 4, null, false);
        Writer.EmHeader header = four.writeEmheader(3, 4, false);
        check(header.hasNextInt(), "LC4 reserves a NEXTINT");
        four.writeU8(1);
        four.writeU8(2);
        four.writeU8(3);
        four.finishEmheader(header);
        bytesEqual(four.finish(), "03000040" + "03000000" + "010203", "an LC4 member");
        Reader fourBack = new Reader(four.finish(), Endianness.LITTLE, 4, null,
                Writer.PID_SHORT_MAX_ID);
        Reader.EmHeaderRead readFour = fourBack.readEmheader();
        equal(readFour.nextInt(), 3, "LC4's NEXTINT");
        equal(fourBack.position(), 8, "LC4's NEXTINT is consumed");
        fourBack.skipMember(readFour);
        equal(fourBack.position(), 11, "skipping an LC4 member");

        // LC5 to LC7: the NEXTINT *is* the member's own leading word. No
        // placeholder is reserved, and the reader is put back before it.
        int[] scales = {1, 4, 8};
        for (int code = 5; code <= 7; code++) {
            Writer writer = new Writer(Endianness.LITTLE, 4, null, false);
            Writer.EmHeader emheader = writer.writeEmheader(9, code, false);
            check(!emheader.hasNextInt(),
                    "LC" + code + " reserves no NEXTINT: the member's own word is it");
            equal(writer.position(), 4, "only the header word was written");
            int elements = 2;
            writer.writeU32(elements);
            int scale = scales[code - 5];
            for (int index = 0; index < elements * scale; index++) {
                writer.writeU8(0xCD);
            }
            writer.finishEmheader(emheader);
            equal(writer.position(), 8 + elements * scale,
                    "LC" + code + " wrote the length once");

            Reader reader = new Reader(writer.finish(), Endianness.LITTLE, 4, null,
                    Writer.PID_SHORT_MAX_ID);
            Reader.EmHeaderRead read = reader.readEmheader();
            equal(read.nextInt(), elements, "LC" + code + " reports the peeked word");
            equal(reader.position(), 4, "and does not consume it");
            reader.skipMember(read);
            equal(reader.position(), 8 + elements * scale,
                    "skipping an LC" + code + " member");
        }

        // The short-code rewrite: an LC4 header whose member turns out to be
        // 1, 2, 4 or 8 bytes loses its NEXTINT and states the length in the
        // code. Safe only because MAXALIGN is four under XCDR2.
        int[] lengths = {1, 2, 4, 8};
        int[] codes = {0, 1, 2, 3};
        for (int index = 0; index < lengths.length; index++) {
            Writer writer = new Writer(Endianness.LITTLE, 4, null, true);
            Writer.EmHeader emheader = writer.writeEmheader(5, 4, false);
            for (int byteIndex = 0; byteIndex < lengths[index]; byteIndex++) {
                writer.writeU8(0xEF);
            }
            writer.finishEmheader(emheader);
            equal(writer.position(), 4 + lengths[index],
                    "the NEXTINT was removed for a " + lengths[index] + "-byte member");
            long word = ((long) codes[index] << 28) | 5L;
            bytesEqual(Arrays.copyOf(writer.finish(), 4), hexOf(word, true),
                    "the rewritten length code for " + lengths[index] + " bytes");
            Reader reader = new Reader(writer.finish(), Endianness.LITTLE, 4, null,
                    Writer.PID_SHORT_MAX_ID);
            Reader.EmHeaderRead read = reader.readEmheader();
            equal(read.lengthCode(), codes[index], "the code the reader sees");
            reader.skipMember(read);
            equal(reader.position(), 4 + lengths[index], "and what it skips");
        }

        // A member the rewrite does not fit keeps its NEXTINT.
        Writer three = new Writer(Endianness.LITTLE, 4, null, true);
        Writer.EmHeader kept = three.writeEmheader(5, 4, false);
        three.writeU8(1);
        three.writeU8(2);
        three.writeU8(3);
        three.finishEmheader(kept);
        bytesEqual(three.finish(), "05000040" + "03000000" + "010203",
                "a three-byte member keeps its NEXTINT");
    }

    /**
     * A parameter header in both forms, and the promotion at a length over
     * {@code 0xFFFF}.
     */
    private static void aParameterHeaderPromotesWhenTheLengthOverrunsSixteenBits() {
        // The extended form as written on purpose.
        Writer extended = new Writer(Endianness.LITTLE, 8, null, false);
        Writer.Pid header = extended.writePidExtended(0x1234, ParameterRole.MEMBER, true);
        extended.writeU32(0xAABBCCDDL);
        extended.finishPid(header);
        bytesEqual(extended.finish(),
                "017f" + "0800" + "34120040" + "04000000" + "ddccbbaa",
                "the extended form with FLAG_2 on the eMemberHeader");
        Reader reader = new Reader(extended.finish(), Endianness.LITTLE, 8, null,
                Writer.PID_SHORT_MAX_ID);
        Reader.PidRead read = reader.readPid();
        equal(read.pid(), 0x1234, "the extended member id");
        equal(read.length(), 4, "the extended length");
        check(read.mustUnderstand(), "FLAG_2 is the member's must-understand");
        check(read.isUserMember(), "and it is a user member");

        // The sentinel, whose flags come from the framing and not from a
        // member's declaration.
        Writer sentinel = new Writer(Endianness.LITTLE, 8, null, false);
        sentinel.writeSentinel();
        bytesEqual(sentinel.finish(), "027f" + "0000", "PID_LIST_END under the clause's framing");
        Writer bare = new Writer(Endianness.LITTLE, 8,
                PlCdr1Framing.defaults().withPidFlags(PidFlagPolicy.BARE), false);
        bare.writeSentinel();
        bytesEqual(bare.finish(), "023f" + "0000", "PID_LIST_END under Fast DDS's framing");
        Reader sentinelBack = new Reader(sentinel.finish(), Endianness.LITTLE, 8, null,
                Writer.PID_SHORT_MAX_ID);
        check(sentinelBack.readPid().isEnd(), "the sentinel is recognised through its flags");

        // The promotion. A short header states a sixteen-bit length; a value
        // longer than that is rewritten as the extended form rather than
        // truncated into two bytes.
        StringBuilder long_ = new StringBuilder();
        for (int index = 0; index < 70000; index++) {
            long_.append('x');
        }
        Writer promoted = new Writer(Endianness.LITTLE, 8, null, false);
        Writer.Pid short_ = promoted.writePid(5, ParameterRole.MEMBER, true);
        check(!short_.isExtended(), "it began as the short form");
        promoted.writeString(long_.toString());
        promoted.finishPid(short_);
        byte[] out = promoted.finish();
        bytesEqual(Arrays.copyOf(out, 12),
                "017f" + "0800" + "05000040" + "75110100",
                "the promoted header: marker, 8, eMemberHeader, length");
        equal(out.length, 12 + 70005 + 3, "the value and its padding");

        Reader promotedBack = new Reader(out, Endianness.LITTLE, 8, null,
                Writer.PID_SHORT_MAX_ID);
        Reader.PidRead readPromoted = promotedBack.readPid();
        equal(readPromoted.pid(), 5, "the promoted member id");
        equal(readPromoted.length(), 70005, "the promoted, exact length");
        check(readPromoted.mustUnderstand(), "the member's flag moved to FLAG_2");
        int saved = promotedBack.beginParameterValue();
        equal(promotedBack.readString().length(), 70000, "the value survived the insertion");
        promotedBack.finishParameterValue(saved);
        equal(promotedBack.position(), out.length, "and the padding is stepped over");

        // A reserved id and PID_IGNORE are not user members.
        Writer reserved = new Writer(Endianness.LITTLE, 8, null, false);
        reserved.writeU16(0x3F10);
        reserved.writeU16(0);
        Reader reservedBack = new Reader(reserved.finish(), Endianness.LITTLE, 8, null,
                Writer.PID_SHORT_MAX_ID);
        Reader.PidRead readReserved = reservedBack.readPid();
        check(readReserved.reserved(), "0x3f10 is inside the reserved range");
        check(!readReserved.isUserMember(), "and is not a member of the reading type");
    }

    // -- binary128 ---------------------------------------------------------

    /** A {@code long double} no {@code double} can hold, out and back. */
    private static void aBinary128NoDoubleCanHoldSurvivesTheWire() {
        // 1 + 2^-100 needs 101 significand bits: past a double at 53 and past
        // x87 extended at 64, so only binary128 carries it.
        Float128 value = Float128.fromBits(Float128.ONE.toBits().setBit(12));
        check(!Float128.fromDouble(value.toDoubleRounded()).equals(value),
                "the test value must be one no double holds");

        for (Endianness order : Endianness.values()) {
            Writer writer = new Writer(order, 8, null, false);
            writer.writeU8(1);
            writer.writeF128(value);
            equal(writer.position(), 24, "a binary128 aligns to eight under XCDR1");
            Reader reader = new Reader(writer.finish(), order, 8, null,
                    Writer.PID_SHORT_MAX_ID);
            equal(reader.readU8(), 1, "the octet before it");
            equal(reader.readF128().toBits(), value.toBits(),
                    "the sixteen bytes come back in " + order);
        }

        // Under XCDR2 the alignment is capped at four, so the same value sits
        // four bytes earlier.
        Writer capped = new Writer(Endianness.LITTLE, 4, null, false);
        capped.writeU8(1);
        capped.writeF128(value);
        equal(capped.position(), 20, "a binary128 aligns to four under XCDR2");

        // The two orders are the whole sixteen bytes reversed, not two halves
        // swapped in place. CORBA Figure 9.2.
        Writer little = new Writer(Endianness.LITTLE, 8, null, false);
        little.writeF128(value);
        Writer big = new Writer(Endianness.BIG, 8, null, false);
        big.writeF128(value);
        byte[] first = little.finish();
        byte[] second = big.finish();
        for (int index = 0; index < 16; index++) {
            check(first[index] == second[15 - index], "the byte order is a whole-value reversal");
        }
    }

    // -- MD5 ---------------------------------------------------------------

    /** RFC 1321 §A.5's own test suite, and the KeyHash forms around it. */
    private static void md5AgreesWithRfc1321() {
        String[][] vectors = {
            {"", "d41d8cd98f00b204e9800998ecf8427e"},
            {"a", "0cc175b9c0f1b6a831c399e269772661"},
            {"abc", "900150983cd24fb0d6963f7d28e17f72"},
            {"message digest", "f96b697d7cb7938d525a2f31aaf161d0"},
            {"abcdefghijklmnopqrstuvwxyz", "c3fcd3d76192e4007dfb496cca67e13b"},
            {"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                "d174ab98d277d9f5a5611c2c9f419d9f"},
            {"1234567890123456789012345678901234567890"
                + "1234567890123456789012345678901234567890",
                "57edf4a22be3c955ac49da2e2107b67a"},
        };
        for (String[] vector : vectors) {
            byte[] input = new byte[vector[0].length()];
            for (int index = 0; index < input.length; index++) {
                input[index] = (byte) vector[0].charAt(index);
            }
            bytesEqual(Md5.digest(input), vector[1], "MD5(\"" + vector[0] + "\")");
        }

        // A holder of sixteen bytes or fewer is the KeyHash, zero-padded;
        // a longer one is its MD5. DDSI-RTPS 2.5 §9.6.3.8.
        Writer holder = Writer.bigEndian();
        holder.writeU32(1);
        bytesEqual(holder.zeroPadded(), "00000001" + "000000000000000000000000",
                "a short key holder is zero-padded");
        Writer wide = Writer.bigEndian();
        for (int index = 0; index < 5; index++) {
            wide.writeU32(index);
        }
        bytesEqual(wide.md5(), hex(Md5.digest(wide.finish())),
                "a long key holder is hashed");
    }

    // -- the encapsulation header -----------------------------------------

    /** The four bytes that announce an encoding. DDSI-RTPS 2.5 §10.5. */
    private static void theEncapsulationHeaderIsAlwaysBigEndian() {
        bytesEqual(Encapsulation.header(Encoding.PLAIN_CDR1, Endianness.LITTLE, 3),
                "00010003", "PLAIN_CDR1 little-endian with three bytes of padding");
        bytesEqual(Encapsulation.header(Encoding.PLAIN_CDR2, Endianness.BIG, 0),
                "00060000", "PLAIN_CDR2 big-endian");
        bytesEqual(Encapsulation.header(Encoding.PL_CDR2, Endianness.LITTLE, 0),
                "000b0000", "PL_CDR2 little-endian");
        equal(Encapsulation.padding(1), 3, "one byte needs three");
        equal(Encapsulation.padding(4), 0, "four needs none");
        equal(Encoding.PLAIN_CDR1.maxAlignment(), 8, "XCDR1's MAXALIGN");
        equal(Encoding.PLAIN_CDR2.maxAlignment(), 4, "XCDR2's MAXALIGN");
        equal(Encoding.DELIMITED_CDR.maxAlignment(), 4, "and every version-2 encoding's");
    }

    // -- the bit-bounded kinds --------------------------------------------

    /** Enumerations, bitmasks and bitsets, at the bounds their clauses give. */
    private static void theBitBoundedKindsTakeTheHoldersTheirClausesGive() {
        Writer writer = new Writer(Endianness.LITTLE, 4, null, false);
        writer.writeEnum(32, -1);
        bytesEqual(writer.finish(), "ffffffff", "@value(-1) at the default bound");

        Writer eight = new Writer(Endianness.LITTLE, 4, null, false);
        eight.writeEnum(8, 200);
        bytesEqual(eight.finish(), "c8", "@value(200) under @bit_bound(8)");

        Writer mask = new Writer(Endianness.LITTLE, 4, null, false);
        mask.writeBitmask(64, 0x8000000000000001L);
        bytesEqual(mask.finish(), "0100000000000080", "bit 0 and bit 63 of a 64-bit bitmask");
        Reader maskBack = new Reader(mask.finish(), Endianness.LITTLE, 4, null,
                Writer.PID_SHORT_MAX_ID);
        equal(maskBack.readBitmask(64), 0x8000000000000001L, "the pattern comes back");

        final Writer refuses = new Writer(Endianness.LITTLE, 4, null, false);
        raises(new Runnable() {
            @Override
            public void run() {
                refuses.writeBitmask(8, 0x100);
            }
        }, "a bit past the bound");
        raises(new Runnable() {
            @Override
            public void run() {
                refuses.writeEnum(8, 256);
            }
        }, "an enumerator past an eight-bit holder");
        raises(new Runnable() {
            @Override
            public void run() {
                refuses.writeBitset(1, 0x100);
            }
        }, "a bitset value the anchor cannot hold");

        for (int anchor : new int[] {1, 2, 4, 8}) {
            Writer bitset = new Writer(Endianness.BIG, 4, null, false);
            bitset.writeBitset(anchor, 0x55);
            equal(bitset.position(), anchor, "a " + anchor + "-byte anchor");
            Reader back = new Reader(bitset.finish(), Endianness.BIG, 4, null,
                    Writer.PID_SHORT_MAX_ID);
            equal(back.readBitset(anchor), 0x55, "the anchor comes back");
        }
    }

    // -- the reader's refusals ---------------------------------------------

    /** A stream that cannot be true is refused, never guessed at. */
    private static void aReaderRefusesWhatCannotBeTrue() {
        final Reader truncated = new Reader(new byte[] {1, 2}, Endianness.LITTLE, 8, null,
                Writer.PID_SHORT_MAX_ID);
        raises(new Runnable() {
            @Override
            public void run() {
                truncated.readU32();
            }
        }, "four bytes out of two");

        final Reader impossible = new Reader(
                new byte[] {(byte) 0xFF, (byte) 0xFF, 0, 0, 1, 2},
                Endianness.LITTLE, 8, null, Writer.PID_SHORT_MAX_ID);
        raises(new Runnable() {
            @Override
            public void run() {
                impossible.readString();
            }
        }, "a string longer than the stream");

        final Reader notUtf8 = new Reader(
                new byte[] {2, 0, 0, 0, (byte) 0xC3, 0},
                Endianness.LITTLE, 8, null, Writer.PID_SHORT_MAX_ID);
        raises(new Runnable() {
            @Override
            public void run() {
                notUtf8.readString();
            }
        }, "a truncated UTF-8 sequence");

        final Reader notBoolean = new Reader(new byte[] {2}, Endianness.LITTLE, 8, null,
                Writer.PID_SHORT_MAX_ID);
        raises(new Runnable() {
            @Override
            public void run() {
                notBoolean.readBool();
            }
        }, "a boolean of 2");
    }

    private static String hexOf(long word, boolean little) {
        Writer writer = new Writer(little ? Endianness.LITTLE : Endianness.BIG, 8, null, false);
        writer.writeU32(word);
        return hex(writer.finish());
    }

    /**
     * A bounded {@code wstring}, {@code sequence} and {@code map} refuse
     * an over-bound value the same way a bounded {@code string} does —
     * before a byte of it is written — and still encode exactly at the
     * bound.
     */
    private static void aBoundedWstringSequenceAndMapRefuseAnOverrun() {
        final Writer wstring = new Writer(Endianness.LITTLE, 8, null, false);
        raises(new Runnable() {
            @Override
            public void run() {
                wstring.writeWstringBounded(
                        "ab", 1, WstringLengthPolicy.SPECIFICATION, WstringUnitOrder.STREAM);
            }
        }, "a wstring longer than its bound, in UTF-16 code units");
        Writer wstringOk = new Writer(Endianness.LITTLE, 8, null, false);
        wstringOk.writeWstringBounded(
                "a", 1, WstringLengthPolicy.SPECIFICATION, WstringUnitOrder.STREAM);

        final Writer sequence = new Writer(Endianness.LITTLE, 8, null, false);
        raises(new Runnable() {
            @Override
            public void run() {
                sequence.checkSequenceBound(4, 3);
            }
        }, "a sequence count past its bound");
        new Writer(Endianness.LITTLE, 8, null, false).checkSequenceBound(3, 3);

        final Writer map = new Writer(Endianness.LITTLE, 8, null, false);
        raises(new Runnable() {
            @Override
            public void run() {
                map.checkMapBound(3, 2);
            }
        }, "a map entry count past its bound");
        new Writer(Endianness.LITTLE, 8, null, false).checkMapBound(2, 2);
    }

    /**
     * The three axes {@code Dialect::Hdds} selects, with expected octets from
     * the recorded HDDS columns:
     * {@code tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin},
     * {@code tests/interop-results/typedef-of-struct/hdds/plain-cdr2.le.bin},
     * {@code tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin},
     * {@code tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin},
     * {@code tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr2.le.bin}
     * and {@code tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr2.le.bin}.
     */
    private static void theThreeHddsAxesAreCarried() {
        // ElementPadding::Packed: each collection element counts its own
        // alignment from its own first octet, so the second Cell's long lands
        // at offset six and not eight.
        Writer packed = new Writer(Endianness.LITTLE, 4, null, false);
        int saved = packed.beginPackedElement();
        packed.writeI32(1);
        packed.writeI16(2);
        packed.finishPackedElement(saved);
        saved = packed.beginPackedElement();
        packed.writeI32(3);
        packed.writeI16(4);
        packed.finishPackedElement(saved);
        packed.writeU8(119);
        bytesEqual(packed.finish(), "01000000020003000000040077",
                "a packed array element starts where the last ended");

        Reader packedBack = new Reader(packed.finish(), Endianness.LITTLE, 4, null,
                Writer.PID_SHORT_MAX_ID);
        saved = packedBack.readerBeginPackedElement();
        equal(packedBack.readI32(), 1, "the first packed long");
        equal(packedBack.readI16(), 2, "the first packed short");
        packedBack.readerFinishPackedElement(saved);
        saved = packedBack.readerBeginPackedElement();
        equal(packedBack.readI32(), 3, "the second packed long");
        equal(packedBack.readI16(), 4, "the second packed short");
        packedBack.readerFinishPackedElement(saved);
        equal(packedBack.readU8(), 119, "the byte behind the array");

        // The same inside two @final members.
        Writer aggregate = new Writer(Endianness.LITTLE, 4, null, false);
        saved = aggregate.beginPackedElement();
        aggregate.writeI32(-1234567);
        aggregate.writeI16(-2);
        aggregate.finishPackedElement(saved);
        saved = aggregate.beginPackedElement();
        aggregate.writeI32(1);
        aggregate.writeI16(32767);
        aggregate.finishPackedElement(saved);
        bytesEqual(aggregate.finish(), "7929edfffeff01000000ff7f",
                "a packed aggregate counts alignment from its own start");

        // A sequence element's own length in DHEADER form: the 06000000 in
        // front of 7929edfffeff is the element's six octets, not the
        // encapsulation's.
        Writer element = new Writer(Endianness.LITTLE, 4, null, false);
        Writer.DHeader header = element.writeDheader();
        element.writeI32(-1234567);
        element.writeI16(-2);
        element.finishDheader(header);
        bytesEqual(element.finish(), "060000007929edfffeff",
                "a sequence element carries its own length");

        // WstringCodeUnits::Utf8: the characters' own octets, the octet count
        // with the terminator, and C2 still reached.
        Writer text = new Writer(Endianness.LITTLE, 4, null, false);
        text.writeWstringUtf8Octets("abc", WstringLengthPolicy.OCTETS_TERMINATED_2);
        bytesEqual(text.finish(), "0400000061626300", "a UTF-8 wstring is its octets and a NUL");

        Writer multibyte = new Writer(Endianness.LITTLE, 4, null, false);
        multibyte.writeWstringUtf8Octets("äöü ሴ Z", WstringLengthPolicy.OCTETS_TERMINATED_2);
        bytesEqual(multibyte.finish(), "0d000000c3a4c3b6c3bc20e188b4205a00",
                "a multi-byte UTF-8 wstring counts its octets");

        Writer bounded = new Writer(Endianness.LITTLE, 4, null, false);
        bounded.writeWstringUtf8OctetsBounded("ሴäZ", 3, WstringLengthPolicy.OCTETS_TERMINATED_2);
        bytesEqual(bounded.finish(), "07000000e188b4c3a45a00",
                "a bounded UTF-8 wstring holds three units");

        Reader textBack = new Reader(
                new byte[] {0x0d, 0x00, 0x00, 0x00, (byte) 0xc3, (byte) 0xa4, (byte) 0xc3,
                        (byte) 0xb6, (byte) 0xc3, (byte) 0xbc, 0x20, (byte) 0xe1, (byte) 0x88,
                        (byte) 0xb4, 0x20, 0x5a, 0x00},
                Endianness.LITTLE, 4, null, Writer.PID_SHORT_MAX_ID);
        equal(textBack.readWstringUtf8Octets(WstringLengthPolicy.OCTETS_TERMINATED_2),
                "äöü ሴ Z", "a UTF-8 wstring reads back as its octets");

        // Malformed UTF-8 and a count the stream cannot hold are refused,
        // never replaced or truncated.
        final Writer lone = new Writer(Endianness.LITTLE, 4, null, false);
        raises(new Runnable() {
            @Override
            public void run() {
                lone.writeWstringUtf8Octets("\uD834", WstringLengthPolicy.OCTETS_TERMINATED_2);
            }
        }, "a lone surrogate is not written as UTF-8 octets");
        raises(new Runnable() {
            @Override
            public void run() {
                Reader impossible = new Reader(
                        new byte[] {(byte) 0xff, 0x00, 0x00, 0x00, 0x61, 0x00},
                        Endianness.LITTLE, 4, null, Writer.PID_SHORT_MAX_ID);
                impossible.readWstringUtf8Octets(WstringLengthPolicy.OCTETS_TERMINATED_2);
            }
        }, "a wstring length past the buffer");
    }

    public static void main(String[] args) {
        anEightByteFieldAfterAOneByteFieldSitsWhereMaxalignSays();
        anOctetIsAByteWrittenAsItsEightBits();
        aParameterValueMovesTheOriginAndFinishPutsItBack();
        everyPrimitiveSurvivesBothByteOrders();
        anOverWideValueIsRefusedAndNotMasked();
        aFloatIsRoundedOnceAndOverflowsToInfinity();
        aStringLengthIsOctetsPlusTheTerminator();
        aBoundedWstringSequenceAndMapRefuseAnOverrun();
        aWstringCountsOctetsAndCarriesASurrogatePair();
        theThreeHddsAxesAreCarried();
        aFixedIsPackedBcdWithASignNibble();
        aDheaderCarriesTheLengthOfWhatFollows();
        everyEmheaderLengthCodeIsFramedAsTheClauseSays();
        aParameterHeaderPromotesWhenTheLengthOverrunsSixteenBits();
        aBinary128NoDoubleCanHoldSurvivesTheWire();
        md5AgreesWithRfc1321();
        theEncapsulationHeaderIsAlwaysBigEndian();
        theBitBoundedKindsTakeTheHoldersTheirClausesGive();
        aReaderRefusesWhatCannotBeTrue();
        if (failures != 0) {
            System.out.println(failures + " expectation(s) did not hold");
            System.exit(1);
        }
    }

    private WireSelfTest() {
    }
}
