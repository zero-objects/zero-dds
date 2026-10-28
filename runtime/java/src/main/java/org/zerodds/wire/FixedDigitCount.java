// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

/**
 * How many digit nibbles a {@code fixed<P,S>} puts on the wire.
 *
 * <p>[CDR] §9.3.2.8 gives the packing and says nothing about the count:
 * omniORB 4.3.3 and this project write the declared {@code P}, JacORB 3.9 as
 * many digits as the value has, ACE/TAO 2.5.24 the digits of the string the
 * value was parsed from.
 *
 * <p>The counterpart of {@code zerodds_ast_layout::FixedDigitCount} and
 * {@code zdw_fixed_digits}.
 */
public enum FixedDigitCount {
    /** {@code P} digits, whatever the value is. The default. */
    DECLARED("declared"),
    /**
     * As many digits as the value has, and never fewer than one:
     * {@code new BigDecimal("0.00").precision()} is 1 in Java, so a zero is
     * the single nibble {@code 0} and the sign.
     */
    VALUE_PRECISION("value-precision"),
    /**
     * The integer digits of the decimal as written, leading zeros
     * included, plus the S fraction digits, made odd by one leading zero: what ACE/TAO 2.5.24's
     * {@code ACE_CDR::Fixed::from_string} writes through its own CDR operator; OpenDDS writes the declared count.
     * {@code 0.12345} under {@code fixed<5,5>} is {@code 0012345c}, wider than
     * {@code P}; a count past 31 is refused. Read like {@link #VALUE_PRECISION}.
     */
    VALUE_STRING_DIGITS("value-string-digits");

    /**
     * The lowest nibble value that is a sign and not a digit. [CDR] §9.3.2.8:
     * a digit is {@code 0} to {@code 9}. It is what makes
     * {@link #VALUE_PRECISION} self-delimiting on the read side.
     */
    public static final int FIRST_SIGN_NIBBLE = 0x0A;

    private final String name;

    FixedDigitCount(String name) {
        this.name = name;
    }

    /** The name {@code FixedDigitCount::as_str} gives. */
    public String asString() {
        return name;
    }

    /** The named count, or {@code null} for anything else. */
    public static FixedDigitCount parse(String text) {
        for (FixedDigitCount count : values()) {
            if (count.name.equals(text)) {
                return count;
            }
        }
        return null;
    }
}
