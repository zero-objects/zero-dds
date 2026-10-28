// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

import java.math.BigDecimal;
import java.math.BigInteger;

/**
 * IEEE 754 binary128 — the sixteen bytes CDR gives {@code long double}.
 *
 * <p>The Java counterpart of {@code crates/float128},
 * {@code runtime/c/src/float128.c} and
 * {@code runtime/python/zerodds_wire/float128.py}: a fourth implementation of
 * the same clauses, not a binding to any of them. CORBA 3.3 part 2 §9.3.1.3
 * with Figure 9.2 gives the layout — one sign bit, a fifteen-bit exponent
 * biased by 16383, a 112-bit stored fraction — and DDS-XTypes 1.3 Table 31
 * names it: quadruple precision, sixteen bytes, aligned to eight.
 *
 * <h2>Why {@link BigDecimal} and not {@code byte[16]}</h2>
 *
 * <p>Java has no primitive wider than {@code double}, so the choice looks like
 * one between a value that cannot be computed with and a number that is not
 * the bit pattern. It is not: <b>both</b> are available, because the
 * conversion between them is exact in one direction and correctly rounded in
 * the other, and this class holds the pattern and offers the number.
 *
 * <p>Every finite binary128 is {@code significand * 2^exponent} with the
 * significand an integer under 2^113. That is exactly a {@link BigDecimal} —
 * {@code BigDecimal} is arbitrary-precision decimal, every binary fraction has
 * a terminating decimal expansion, and {@link BigInteger} carries the
 * significand with no width to overflow. So {@link #toBigDecimal()} loses
 * nothing at all, and {@link #fromBigDecimal(BigDecimal)} rounds to nearest
 * with ties to even exactly as the format requires and says nothing quietly.
 *
 * <p>The one thing {@code BigDecimal} cannot carry is a value that is not a
 * number: an infinity, a NaN, and the distinction between {@code -0.0} and
 * {@code +0.0}. Those three live in the bit pattern, which is why the pattern
 * and not the {@code BigDecimal} is what this class stores and what
 * {@link #equals} compares. {@link #toBigDecimal()} refuses an infinity and a
 * NaN by name rather than inventing a decimal for them.
 *
 * <p>Round-tripping a finite value through {@code BigDecimal} and back is
 * therefore bit-exact, and {@code Float128RoundTripTest} asserts it over the
 * corners and over generated patterns. A pattern that did not survive would be
 * a finding to report with the pattern, not a reason to reach for
 * {@code byte[16]}.
 *
 * <h2>What is not here</h2>
 *
 * <p>Arithmetic. Nothing on the wire path needs it, and
 * {@code docs/float128-column.md} §6 says what adding it would take. A caller
 * that wants to compute takes the {@link BigDecimal}, computes in it, and
 * comes back — which is slower than hardware and exactly as correct.
 */
public final class Float128 {

    /** Bits in the stored fraction. IEEE 754-2008 clause 3.6. */
    private static final int FRACTION_BITS = 112;
    /** What is added to the unbiased exponent to get the stored one. */
    private static final int EXPONENT_BIAS = 16383;
    /** The stored exponent of an infinity or a NaN. */
    private static final int EXPONENT_MAX = 0x7fff;
    /** Significand bits including the implied leading one. */
    private static final int SIGNIFICAND_BITS = FRACTION_BITS + 1;

    private static final BigInteger FRACTION_MASK =
            BigInteger.ONE.shiftLeft(FRACTION_BITS).subtract(BigInteger.ONE);
    private static final BigInteger IMPLIED_ONE = BigInteger.ONE.shiftLeft(FRACTION_BITS);
    private static final BigInteger FIVE = BigInteger.valueOf(5);

    /** Decimal digits that always round-trip: ceil(113 * log10 2) + 1. */
    public static final int DIGITS_ROUND_TRIP = 36;

    /** Positive zero. */
    public static final Float128 ZERO = fromBits(BigInteger.ZERO);
    /** Negative zero. Distinct from {@link #ZERO} on the wire and here. */
    public static final Float128 NEGATIVE_ZERO = fromBits(BigInteger.ONE.shiftLeft(127));
    /** One. */
    public static final Float128 ONE =
            fromBits(BigInteger.valueOf(EXPONENT_BIAS).shiftLeft(FRACTION_BITS));
    /** Positive infinity. */
    public static final Float128 POSITIVE_INFINITY =
            fromBits(BigInteger.valueOf(EXPONENT_MAX).shiftLeft(FRACTION_BITS));
    /** Negative infinity. */
    public static final Float128 NEGATIVE_INFINITY =
            fromBits(POSITIVE_INFINITY.bits.setBit(127));
    /** A quiet NaN with no payload. */
    public static final Float128 NAN =
            fromBits(POSITIVE_INFINITY.bits.setBit(FRACTION_BITS - 1));

    /** The 128-bit pattern, as a non-negative {@link BigInteger}. */
    private final BigInteger bits;

    private Float128(BigInteger bits) {
        this.bits = bits;
    }

    // -- the pattern ------------------------------------------------------

    /** From the raw 128-bit pattern. */
    public static Float128 fromBits(BigInteger bits) {
        if (bits.signum() < 0 || bits.bitLength() > 128) {
            throw new IllegalArgumentException("a binary128 pattern is 128 bits");
        }
        return new Float128(bits);
    }

    /** From the pattern as two 64-bit halves, high first. */
    public static Float128 fromHalves(long high, long low) {
        BigInteger value = BigInteger.valueOf(high & 0x7fffffffffffffffL);
        if (high < 0) {
            value = value.setBit(63);
        }
        BigInteger lower = BigInteger.valueOf(low & 0x7fffffffffffffffL);
        if (low < 0) {
            lower = lower.setBit(63);
        }
        return fromBits(value.shiftLeft(64).or(lower));
    }

    /** The raw 128-bit pattern. */
    public BigInteger toBits() {
        return bits;
    }

    /**
     * From the sixteen bytes in the order named.
     *
     * <p>The two orders are the whole sixteen bytes reversed — CORBA
     * Figure 9.2 — and not two eight-byte halves swapped in place.
     */
    public static Float128 fromBytes(byte[] data, boolean bigEndian) {
        if (data.length != 16) {
            throw new IllegalArgumentException("a binary128 is exactly sixteen bytes");
        }
        byte[] ordered = bigEndian ? data.clone() : reversed(data);
        // A leading zero byte, so the sign bit is never read as a
        // two's-complement negative.
        byte[] unsigned = new byte[17];
        System.arraycopy(ordered, 0, unsigned, 1, 16);
        return fromBits(new BigInteger(unsigned));
    }

    /** The sixteen bytes in the order named. */
    public byte[] toBytes(boolean bigEndian) {
        byte[] out = new byte[16];
        byte[] magnitude = bits.toByteArray();
        int from = Math.max(0, magnitude.length - 16);
        int length = magnitude.length - from;
        System.arraycopy(magnitude, from, out, 16 - length, length);
        return bigEndian ? out : reversed(out);
    }

    private static byte[] reversed(byte[] data) {
        byte[] out = new byte[data.length];
        for (int i = 0; i < data.length; i++) {
            out[i] = data[data.length - 1 - i];
        }
        return out;
    }

    // -- fields -----------------------------------------------------------

    /** The sign bit, as stored. True for {@code -0.0} and a negative NaN. */
    public boolean isNegative() {
        return bits.testBit(127);
    }

    private int biasedExponent() {
        return bits.shiftRight(FRACTION_BITS).intValue() & EXPONENT_MAX;
    }

    private BigInteger fraction() {
        return bits.and(FRACTION_MASK);
    }

    public boolean isNaN() {
        return biasedExponent() == EXPONENT_MAX && fraction().signum() != 0;
    }

    public boolean isInfinite() {
        return biasedExponent() == EXPONENT_MAX && fraction().signum() == 0;
    }

    public boolean isFinite() {
        return biasedExponent() != EXPONENT_MAX;
    }

    public boolean isZero() {
        return bits.clearBit(127).signum() == 0;
    }

    public boolean isSubnormal() {
        return biasedExponent() == 0 && fraction().signum() != 0;
    }

    // -- BigDecimal, in both directions -----------------------------------

    /**
     * The exact value.
     *
     * <p>Exact and never rounded: a finite binary128 is
     * {@code significand * 2^exponent}, and multiplying by {@code 5^k} turns
     * {@code 2^-k} into a terminating decimal that {@code BigDecimal} holds
     * in full. No {@code MathContext} is consulted anywhere on this path,
     * because a precision would be a place for the value to be shortened.
     *
     * @throws ArithmeticException for an infinity or a NaN, which have no
     *     decimal. They are in the bit pattern, which {@link #toBits()} gives.
     */
    public BigDecimal toBigDecimal() {
        if (!isFinite()) {
            throw new ArithmeticException(
                    "an infinity or a NaN has no decimal value; use toBits()");
        }
        BigInteger significand = significand();
        int exponent = exponent();
        BigDecimal value;
        if (exponent >= 0) {
            value = new BigDecimal(significand.shiftLeft(exponent));
        } else {
            // m * 2^-k == (m * 5^k) * 10^-k, which is exact.
            value = new BigDecimal(significand.multiply(FIVE.pow(-exponent)), -exponent);
        }
        return isNegative() ? value.negate() : value;
    }

    /**
     * From an exact decimal, rounding to nearest with ties to even.
     *
     * <p>Correctly rounded against the exact rational, never through a
     * {@code double}: a two-step conversion rounds twice, and double rounding
     * gets the last bit wrong for the values this class exists to carry.
     */
    public static Float128 fromBigDecimal(BigDecimal value) {
        if (value.signum() == 0) {
            // `BigDecimal` has no negative zero, so a zero is positive here.
            // A caller that means -0.0 has a pattern for it.
            return ZERO;
        }
        boolean negative = value.signum() < 0;
        BigDecimal magnitude = value.abs();
        BigInteger numerator = magnitude.unscaledValue();
        BigInteger denominator = BigInteger.ONE;
        int scale = magnitude.scale();
        if (scale > 0) {
            denominator = BigInteger.TEN.pow(scale);
        } else if (scale < 0) {
            numerator = numerator.multiply(BigInteger.TEN.pow(-scale));
        }
        return fromRatio(negative, numerator, denominator);
    }

    /** As {@link #fromBigDecimal}, from the decimal's text. */
    public static Float128 fromDecimalString(String text) {
        String trimmed = text.trim();
        String lowered = trimmed.toLowerCase(java.util.Locale.ROOT);
        if (lowered.equals("inf") || lowered.equals("+inf") || lowered.equals("infinity")) {
            return POSITIVE_INFINITY;
        }
        if (lowered.equals("-inf") || lowered.equals("-infinity")) {
            return NEGATIVE_INFINITY;
        }
        if (lowered.equals("nan")) {
            return NAN;
        }
        if (lowered.equals("-nan")) {
            return fromBits(NAN.bits.setBit(127));
        }
        if (trimmed.indexOf('_') >= 0) {
            // Java would not accept it here either, but saying so by name
            // beats a NumberFormatException a caller has to decode.
            throw new NumberFormatException("an underscore is not part of a decimal");
        }
        BigDecimal parsed = new BigDecimal(trimmed);
        Float128 value = fromBigDecimal(parsed);
        // `BigDecimal` cannot hold a negative zero, so a "-0.0" in a value
        // file would come back as +0.0 and change the sixteen bytes. The
        // sign is taken from the text, which is the only place it survives.
        if (parsed.signum() == 0 && trimmed.startsWith("-")) {
            return NEGATIVE_ZERO;
        }
        return value;
    }

    /**
     * Every digit the value has, in the corpus's canonical spelling.
     *
     * <p>Byte for byte the same text {@code crates/float128},
     * {@code runtime/c/src/float128_decimal.c} and
     * {@code runtime/python/zerodds_wire/float128.py} produce for the same
     * pattern: the columns are compared as text, so the renderers have to
     * agree on the form as well as on the value.
     */
    public String toExactDecimal() {
        if (isNaN()) {
            return "nan";
        }
        if (isInfinite()) {
            return isNegative() ? "-inf" : "inf";
        }
        BigInteger significand = significand();
        if (significand.signum() == 0) {
            return isNegative() ? "-0.0" : "0.0";
        }
        int exponent = exponent();
        String digits;
        int point;
        if (exponent >= 0) {
            digits = significand.shiftLeft(exponent).toString();
            point = digits.length();
        } else {
            digits = significand.multiply(FIVE.pow(-exponent)).toString();
            point = digits.length() + exponent;
        }
        int end = digits.length();
        while (end > 0 && digits.charAt(end - 1) == '0') {
            end--;
        }
        digits = end == 0 ? "0" : digits.substring(0, end);
        return render(isNegative(), digits, point);
    }

    // -- double, with the inexact cases reported --------------------------

    /** From a {@code double}. Always exact: binary128 covers every binary64. */
    public static Float128 fromDouble(double value) {
        long raw = Double.doubleToRawLongBits(value);
        boolean negative = raw < 0;
        int biased = (int) ((raw >>> 52) & 0x7ff);
        long fraction = raw & 0x000fffffffffffffL;
        BigInteger sign = negative ? BigInteger.ONE.shiftLeft(127) : BigInteger.ZERO;
        if (biased == 0x7ff) {
            // A NaN keeps its payload, aligned so the double's quiet bit
            // lands on binary128's.
            BigInteger payload = BigInteger.valueOf(fraction).shiftLeft(60);
            return fromBits(sign.or(BigInteger.valueOf(EXPONENT_MAX).shiftLeft(FRACTION_BITS))
                    .or(payload));
        }
        if (biased == 0 && fraction == 0) {
            return fromBits(sign);
        }
        if (biased == 0) {
            // A subnormal double becomes a binary128 *normal*: binary128's
            // exponent range swallows binary64's whole subnormal range.
            return (Float128) compose(
                    negative, BigInteger.valueOf(fraction), 1 - 1023 - 52)[0];
        }
        BigInteger significand = BigInteger.valueOf(fraction | (1L << 52));
        return (Float128) compose(negative, significand, biased - 1023 - 52)[0];
    }

    /**
     * To a {@code double} when the value fits one exactly.
     *
     * @throws ArithmeticException naming the value, and carrying the rounded
     *     result in {@link Inexact#rounded}. This is the refusal the
     *     {@code long double} column needed: writing a {@code double} and
     *     eight zero bytes was the right length and the wrong value.
     */
    public double toDouble() {
        double rounded = toDoubleRounded();
        if (!fromDouble(rounded).equals(this)) {
            throw new Inexact(this, rounded);
        }
        return rounded;
    }

    /** To a {@code double}, rounding to nearest with ties to even. */
    public double toDoubleRounded() {
        if (isNaN()) {
            return Double.NaN;
        }
        if (isInfinite()) {
            return isNegative() ? Double.NEGATIVE_INFINITY : Double.POSITIVE_INFINITY;
        }
        if (isZero()) {
            return isNegative() ? -0.0 : 0.0;
        }
        // Through the exact decimal rather than through a hand-written
        // narrowing: `Double.parseDouble` is correctly rounded, and a second
        // narrowing written here would be a second place to get it wrong.
        return Double.parseDouble(toExactDecimal());
    }

    /** A conversion that could not be exact, carrying what it rounds to. */
    public static final class Inexact extends ArithmeticException {
        private static final long serialVersionUID = 1L;

        /** The source value. */
        public final transient Float128 source;
        /** What it rounds to in the destination type. */
        public final double rounded;

        Inexact(Float128 source, double rounded) {
            super("the binary128 value 0x" + source.bits.toString(16)
                    + " does not fit a double exactly");
            this.source = source;
            this.rounded = rounded;
        }
    }

    // -- comparison -------------------------------------------------------

    /**
     * Bit-pattern equality, which is what a key holder and a corpus
     * expectation compare: {@code -0.0} differs from {@code +0.0} and a NaN
     * equals itself. IEEE equality is {@link #ieeeEquals}, spelled out so
     * neither meaning is reached by accident.
     */
    @Override
    public boolean equals(Object other) {
        return other instanceof Float128 && bits.equals(((Float128) other).bits);
    }

    @Override
    public int hashCode() {
        return bits.hashCode();
    }

    /** IEEE 754-2008 {@code compareQuietEqual}. */
    public boolean ieeeEquals(Float128 other) {
        if (isNaN() || other.isNaN()) {
            return false;
        }
        if (isZero() && other.isZero()) {
            return true;
        }
        return bits.equals(other.bits);
    }

    /**
     * A key giving IEEE 754-2008 {@code totalOrder} when compared as an
     * unsigned 128-bit integer: {@code -NaN < -inf < ... < -0.0 < +0.0 < ...
     * < +inf < +NaN}, NaNs ordered by payload.
     */
    public BigInteger totalOrderKey() {
        if (bits.testBit(127)) {
            return bits.xor(BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE));
        }
        return bits.setBit(127);
    }

    @Override
    public String toString() {
        return toExactDecimal();
    }

    // -- the shared decomposition -----------------------------------------

    /** The integral significand, with the implied one materialised. */
    private BigInteger significand() {
        BigInteger fraction = fraction();
        return biasedExponent() == 0 ? fraction : fraction.or(IMPLIED_ONE);
    }

    /** The binary exponent that scales {@link #significand()}. */
    private int exponent() {
        int biased = biasedExponent();
        return (biased == 0 ? 1 : biased) - EXPONENT_BIAS - FRACTION_BITS;
    }

    /**
     * Round {@code numerator / denominator} to binary128, nearest, ties to
     * even. The quotient is taken with enough extra bits that the single
     * rounding at the end is the correct one, and everything discarded is
     * folded into one sticky bit.
     */
    private static Float128 fromRatio(
            boolean negative, BigInteger numerator, BigInteger denominator) {
        if (numerator.signum() == 0) {
            return negative ? NEGATIVE_ZERO : ZERO;
        }
        int shift = 130 - (numerator.bitLength() - denominator.bitLength());
        if (shift < 0) {
            shift = 0;
        }
        BigInteger[] divided = numerator.shiftLeft(shift).divideAndRemainder(denominator);
        BigInteger quotient = divided[0];
        if (divided[1].signum() != 0) {
            quotient = quotient.setBit(0);
        }
        return (Float128) compose(negative, quotient, -shift)[0];
    }

    /**
     * Build a value from a sign, an integral significand and a binary
     * exponent, rounding to nearest with ties to even.
     *
     * @return {@code {Float128, Boolean}} — the value, and whether the
     *     rounding discarded anything.
     */
    private static Object[] compose(boolean negative, BigInteger significand, int exponent) {
        BigInteger sign = negative ? BigInteger.ONE.shiftLeft(127) : BigInteger.ZERO;
        if (significand.signum() == 0) {
            return new Object[] {new Float128(sign), Boolean.FALSE};
        }
        if (exponent > EXPONENT_MAX) {
            return new Object[] {
                negative ? NEGATIVE_INFINITY : POSITIVE_INFINITY, Boolean.TRUE
            };
        }
        if (exponent < -(SIGNIFICAND_BITS + EXPONENT_BIAS + 200)) {
            return new Object[] {new Float128(sign), Boolean.TRUE};
        }

        boolean lost = false;
        int leading = significand.bitLength() - 1;
        if (leading > FRACTION_BITS) {
            int drop = leading - FRACTION_BITS;
            Object[] shifted = shiftRound(significand, drop);
            significand = (BigInteger) shifted[0];
            lost = (Boolean) shifted[1];
            exponent += drop;
            if (significand.bitLength() - 1 > FRACTION_BITS) {
                significand = significand.shiftRight(1);
                exponent += 1;
            }
        } else {
            int shift = FRACTION_BITS - leading;
            significand = significand.shiftLeft(shift);
            exponent -= shift;
        }

        int biased = exponent + EXPONENT_BIAS + FRACTION_BITS;
        if (biased >= EXPONENT_MAX) {
            return new Object[] {
                negative ? NEGATIVE_INFINITY : POSITIVE_INFINITY, Boolean.TRUE
            };
        }
        if (biased <= 0) {
            // Subnormal. A value that rounds up to the smallest normal
            // carries out of the fraction into the exponent field on its
            // own, so the fraction is deliberately not masked here.
            Object[] shifted = shiftRound(significand, 1 - biased);
            BigInteger rounded = (BigInteger) shifted[0];
            return new Object[] {
                new Float128(sign.or(rounded)), lost || (Boolean) shifted[1]
            };
        }
        BigInteger stored = sign
                .or(BigInteger.valueOf(biased).shiftLeft(FRACTION_BITS))
                .or(significand.and(FRACTION_MASK));
        return new Object[] {new Float128(stored), lost};
    }

    /** Shift right, rounding to nearest with ties to even. */
    private static Object[] shiftRound(BigInteger value, int shift) {
        if (shift <= 0) {
            return new Object[] {value, Boolean.FALSE};
        }
        BigInteger kept = value.shiftRight(shift);
        BigInteger dropped = value.subtract(kept.shiftLeft(shift));
        if (dropped.signum() == 0) {
            return new Object[] {kept, Boolean.FALSE};
        }
        BigInteger half = BigInteger.ONE.shiftLeft(shift - 1);
        int order = dropped.compareTo(half);
        if (order > 0 || (order == 0 && kept.testBit(0))) {
            kept = kept.add(BigInteger.ONE);
        }
        return new Object[] {kept, Boolean.TRUE};
    }

    /** {@code 0.<digits> * 10^point}, in the three forms all columns use. */
    private static String render(boolean negative, String digits, int point) {
        StringBuilder out = new StringBuilder();
        if (negative) {
            out.append('-');
        }
        if (point > 0 && point <= 34) {
            if (digits.length() <= point) {
                out.append(digits);
                for (int i = digits.length(); i < point; i++) {
                    out.append('0');
                }
                out.append(".0");
            } else {
                out.append(digits, 0, point).append('.').append(digits.substring(point));
            }
            return out.toString();
        }
        if (point <= 0 && point > -6) {
            out.append("0.");
            for (int i = 0; i < -point; i++) {
                out.append('0');
            }
            return out.append(digits).toString();
        }
        out.append(digits.charAt(0)).append('.');
        out.append(digits.length() == 1 ? "0" : digits.substring(1));
        int exponent = point - 1;
        out.append('e').append(exponent >= 0 ? '+' : '-').append(Math.abs(exponent));
        return out.toString();
    }
}
