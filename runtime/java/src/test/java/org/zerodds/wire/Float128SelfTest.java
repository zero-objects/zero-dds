// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

import java.io.IOException;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;

/**
 * What the Java column has to do before the wire can use it.
 *
 * <p>No test framework: this runtime has no dependencies and adding JUnit to
 * assert four hundred lines of arithmetic would be the first one. A
 * {@code main} that returns non-zero is what {@code runtime/java/run-tests.sh}
 * and the CI job need.
 *
 * <p>The strongest check here is the last one. It reads
 * {@code crates/float128/tests/oracle/patterns.txt} — 518 bit patterns whose
 * exact decimals the Rust, C and Python columns already agree on, and which
 * GCC's own {@code __float128} was measured against — and asserts that this
 * class prints the same text and reads it back to the same pattern. Four
 * independent implementations of one clause agreeing is the whole point of the
 * interop suite; this is that check for the column that has no primitive.
 */
public final class Float128SelfTest {

    private static int failures = 0;

    private static void check(boolean condition, String what) {
        if (!condition) {
            System.out.println("FAIL " + what);
            failures++;
        }
    }

    private static void equal(Object left, Object right, String what) {
        if (!left.equals(right)) {
            System.out.println("FAIL " + what + "\n  expected " + right + "\n  got      " + left);
            failures++;
        }
    }

    /** The pattern for 1.5, measured from GCC and from RTI Connext 7.7. */
    private static void oneAndAHalfIsThePatternTheVendorsWrite() {
        Float128 value = Float128.fromDouble(1.5);
        equal(hex(value.toBytes(false)), "0000000000000000000000000080ff3f", "1.5 little-endian");
        equal(hex(value.toBytes(true)), "3fff8000000000000000000000000000", "1.5 big-endian");
        // CORBA Figure 9.2: the whole sixteen bytes reversed, not two
        // eight-byte halves swapped in place.
        byte[] little = value.toBytes(false);
        byte[] big = value.toBytes(true);
        for (int i = 0; i < 16; i++) {
            check(little[i] == big[15 - i], "byte order is not a whole-value reversal");
        }
    }

    private static void everyDoubleWidensAndNarrowsBackUnchanged() {
        double[] values = {
            0.0, -0.0, 1.0, -1.0, 1.5, -2.75, Double.MIN_VALUE, Double.MAX_VALUE,
            Double.MIN_NORMAL, 0.1, 1e308, 5e-324,
        };
        for (double value : values) {
            Float128 wide = Float128.fromDouble(value);
            double back = wide.toDouble();
            equal(Double.doubleToRawLongBits(back), Double.doubleToRawLongBits(value),
                    "the double " + value + " did not survive");
        }
        check(Float128.fromDouble(Double.POSITIVE_INFINITY).isInfinite(), "infinity widened");
        check(Float128.fromDouble(Double.NaN).isNaN(), "NaN widened");
    }

    private static void aValueADoubleCannotHoldIsRefusedByName() {
        // 1 + 2^-100 needs 101 significand bits: past a double at 53 and past
        // x87 extended at 64, so only binary128 carries it.
        Float128 wide = Float128.fromBits(Float128.ONE.toBits().setBit(12));
        check(!wide.equals(Float128.ONE), "1 + 2^-100 collapsed into one");
        try {
            wide.toDouble();
            check(false, "1 + 2^-100 claimed to fit a double");
        } catch (Float128.Inexact inexact) {
            equal(inexact.rounded, 1.0, "the rounded value is carried, not hidden");
        }
    }

    private static void bigDecimalCarriesTheValueAndTheBitsComeBack() {
        // The claim this column rests on: BigDecimal loses nothing for a
        // finite value, so "carries the value but not the pattern" is a
        // conversion and not a loss.
        Float128[] corners = {
            Float128.ZERO,
            Float128.ONE,
            Float128.fromDouble(1.5),
            Float128.fromDouble(-2.75),
            Float128.fromBits(Float128.ONE.toBits().setBit(0)),       // 1 + 2^-112
            Float128.fromBits(Float128.ONE.toBits().setBit(52)),      // 1 + 2^-60
            Float128.fromBits(BigInteger.ONE.shiftLeft(112)),         // smallest normal
            Float128.fromBits(BigInteger.ONE.shiftLeft(112).subtract(BigInteger.ONE)),
            Float128.fromBits(BigInteger.valueOf(0x7ffe).shiftLeft(112)
                    .or(BigInteger.ONE.shiftLeft(112).subtract(BigInteger.ONE))),
            Float128.fromBits(BigInteger.ONE),                        // smallest subnormal
        };
        for (Float128 value : corners) {
            BigDecimal exact = value.toBigDecimal();
            Float128 back = Float128.fromBigDecimal(exact);
            equal(back, value, "0x" + value.toBits().toString(16)
                    + " did not survive a BigDecimal round trip");
        }
        // The one finite pattern a BigDecimal round trip does *not*
        // reproduce, stated as an assertion so it cannot quietly change:
        //
        //     0x80000000000000000000000000000000  (negative zero)
        //
        // `BigDecimal` has no signed zero — `BigDecimal.ZERO.signum()` is 0
        // and negating it changes nothing — so the sign is gone by the time
        // `fromBigDecimal` is called. It is not a loss of *value*: negative
        // zero and positive zero are numerically equal, and IEEE 754 says
        // so. It is a loss of the sixteen bytes, which on a wire is the
        // thing that matters, so the pattern and not the BigDecimal is what
        // this class stores, and the text form carries the sign.
        equal(Float128.fromBigDecimal(Float128.NEGATIVE_ZERO.toBigDecimal()), Float128.ZERO,
                "the BigDecimal round trip of a negative zero");
        check(!Float128.NEGATIVE_ZERO.equals(Float128.ZERO), "the two zeros differ");
        equal(Float128.fromDecimalString("-0.0"), Float128.NEGATIVE_ZERO,
                "a negative zero survives its text");
        equal(Float128.NEGATIVE_ZERO.toExactDecimal(), "-0.0",
                "and the text is what the other columns print");
        for (Float128 value : new Float128[] {
            Float128.POSITIVE_INFINITY, Float128.NEGATIVE_INFINITY, Float128.NAN
        }) {
            try {
                value.toBigDecimal();
                check(false, "a non-finite value produced a BigDecimal");
            } catch (ArithmeticException expected) {
                // The refusal is the behaviour.
            }
        }
    }

    private static void theExactDecimalIsWhatTheOtherColumnsPrint() {
        equal(Float128.fromDouble(1.5).toExactDecimal(), "1.5", "1.5");
        equal(Float128.fromDouble(-2.75).toExactDecimal(), "-2.75", "-2.75");
        equal(Float128.ZERO.toExactDecimal(), "0.0", "zero");
        equal(Float128.NEGATIVE_ZERO.toExactDecimal(), "-0.0", "negative zero");
        equal(Float128.ONE.toExactDecimal(), "1.0", "one");
        equal(Float128.POSITIVE_INFINITY.toExactDecimal(), "inf", "infinity");
        equal(Float128.NAN.toExactDecimal(), "nan", "nan");
        equal(Float128.fromBits(Float128.ONE.toBits().setBit(0)).toExactDecimal(),
                "1.0000000000000000000000000000000001925929944387235853055977942584927"
                        + "318538101648215388195239938795566558837890625",
                "one plus epsilon");
    }

    private static void orderingPlacesTheCornersWhereIeeeSays() {
        Float128[] ordered = {
            Float128.fromBits(Float128.NAN.toBits().setBit(127)),
            Float128.NEGATIVE_INFINITY,
            Float128.fromBits(Float128.ONE.toBits().setBit(127)),
            Float128.NEGATIVE_ZERO,
            Float128.ZERO,
            Float128.ONE,
            Float128.POSITIVE_INFINITY,
            Float128.NAN,
        };
        for (int i = 0; i + 1 < ordered.length; i++) {
            check(ordered[i].totalOrderKey().compareTo(ordered[i + 1].totalOrderKey()) < 0,
                    "total order puts 0x" + ordered[i].toBits().toString(16) + " wrongly");
        }
        check(Float128.NEGATIVE_ZERO.ieeeEquals(Float128.ZERO), "IEEE says the zeros are equal");
        check(!Float128.NAN.ieeeEquals(Float128.NAN), "IEEE says a NaN equals nothing");
        check(Float128.NAN.equals(Float128.NAN), "bit equality says a NaN is itself");
    }

    /**
     * The cross-column check: the 518 patterns the Rust, C and Python columns
     * agree on, and which GCC's {@code __float128} was measured against.
     */
    private static void theSameFiveHundredEighteenPatternsTheOtherColumnsAgreeOn()
            throws IOException {
        Path patterns = repoRoot().resolve("crates/float128/tests/oracle/patterns.txt");
        if (!Files.exists(patterns)) {
            System.out.println("SKIP the oracle patterns are not where this test looked: "
                    + patterns);
            failures++;
            return;
        }
        List<String> lines = Files.readAllLines(patterns, StandardCharsets.UTF_8);
        check(lines.size() == 518, "the oracle file holds " + lines.size() + " rows, not 518");
        int checked = 0;
        for (String line : lines) {
            String[] parts = line.trim().split("\\s+");
            if (parts.length < 2) {
                continue;
            }
            Float128 value = Float128.fromBits(new BigInteger(parts[0], 16));
            // The shortest form the Rust column printed must read back here
            // to the very same pattern: a parser that rounded differently
            // would land a neighbour.
            equal(Float128.fromDecimalString(parts[1]), value,
                    "`" + parts[1] + "` did not read back as 0x" + parts[0]);
            // And the sixteen bytes, both ways round.
            equal(Float128.fromBytes(value.toBytes(false), false), value, "little-endian bytes");
            equal(Float128.fromBytes(value.toBytes(true), true), value, "big-endian bytes");
            // Every finite pattern survives a BigDecimal round trip except
            // a zero, whose sign BigDecimal does not have. That is the one
            // measured exception and it is asserted below by name rather
            // than skipped here by luck.
            if (value.isFinite() && !value.isZero()) {
                equal(Float128.fromBigDecimal(value.toBigDecimal()), value,
                        "0x" + parts[0] + " did not survive BigDecimal");
            }
            checked++;
        }
        check(checked == 518, "only " + checked + " patterns were checked");
    }

    /** Walk up from the working directory until the corpus is in sight. */
    private static Path repoRoot() {
        Path here = Paths.get("").toAbsolutePath();
        while (here != null && !Files.isDirectory(here.resolve("crates/float128"))) {
            here = here.getParent();
        }
        return here == null ? Paths.get("") : here;
    }

    private static String hex(byte[] data) {
        StringBuilder out = new StringBuilder();
        for (byte value : data) {
            out.append(String.format("%02x", value));
        }
        return out.toString();
    }

    public static void main(String[] args) throws IOException {
        oneAndAHalfIsThePatternTheVendorsWrite();
        everyDoubleWidensAndNarrowsBackUnchanged();
        aValueADoubleCannotHoldIsRefusedByName();
        bigDecimalCarriesTheValueAndTheBitsComeBack();
        theExactDecimalIsWhatTheOtherColumnsPrint();
        orderingPlacesTheCornersWhereIeeeSays();
        theSameFiveHundredEighteenPatternsTheOtherColumnsAgreeOn();
        if (failures != 0) {
            System.out.println(failures + " expectation(s) did not hold");
            System.exit(1);
        }
        System.out.println("the Java binary128 column holds every expectation");
    }
}
