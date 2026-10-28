// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

/**
 * How a PL_CDR1 parameter list is framed.
 *
 * <p>The three ways the measured implementations differ inside an XCDR1
 * sample. Which header <i>form</i> each member takes is the fourth, and that
 * one is in the layout plan because it changes the member's extent.
 *
 * <p>{@link #defaults()} is the clause's — {@code Dialect::Specification},
 * which is this project's default dialect. See
 * {@code zerodds_ast_layout::Dialect}; they were Cyclone's while
 * {@code Dialect::Cyclone} was the default, and the two that moved are
 * {@link #padDeclaredOptionalLength()} and the header form.
 *
 * <p>Immutable. The {@code with…} methods return a new framing rather than
 * changing one a writer already holds: a wire-affecting setting that could be
 * changed halfway through a sample would produce bytes no reader can frame.
 */
public final class PlCdr1Framing {

    private final PidFlagPolicy pidFlags;
    private final int highestShortId;
    private final boolean padDeclaredLength;
    private final boolean padDeclaredOptionalLength;
    private final boolean memberMustUnderstand;
    private final boolean padAfterValue;

    /**
     * A framing with every axis named.
     *
     * @param pidFlags what flags a parameter id carries, asked per position —
     *     a member's id, {@code PID_LIST_END} and the {@code PID_EXTENDED}
     *     marker. One setting for all three, because no position's answer can
     *     be read off another's.
     * @param highestShortId the highest member id a short parameter header
     *     carries. §7.4.1.2.2 stops at {@code 0x3F00} inclusive and reserves
     *     the rest of the low fourteen bits; one implementation carries all
     *     fourteen, and a reader holding the clause's threshold as a constant
     *     reports {@code @id(16383)} as reserved and drops the member.
     *     Read-side only.
     * @param padDeclaredLength whether a {@code @mutable} member's declared
     *     length covers its trailing padding.
     * @param padDeclaredOptionalLength the same question for an
     *     {@code @optional} member of a type that is not {@code @mutable}. A
     *     separate axis because Cyclone answers the two differently:
     *     {@code 04000000} for the two-byte {@code short c} of
     *     {@code ext_mutable}, {@code 01000000} for the one-byte
     *     {@code @optional octet b} of {@code opt_final_present}. RTI pads
     *     both, Fast DDS pads neither, and the default pads both.
     * @param memberMustUnderstand whether a member's own must-understand bit
     *     goes out. §7.4.1.2.2 makes it unconditional and two of the three
     *     write it: Cyclone as {@code FLAG_2} of the extended
     *     {@code eMemberHeader}, RTI as bit 14 of the short id, Fast DDS not
     *     at all.
     * @param padAfterValue whether the parameter that just ended writes its
     *     own trailing padding to the next four-byte boundary. Every header
     *     writer aligns itself, so the two readings only part at the end of
     *     the stream: RTI pads after a final {@code @optional} parameter,
     *     Fast DDS does not.
     */
    public PlCdr1Framing(
            PidFlagPolicy pidFlags,
            int highestShortId,
            boolean padDeclaredLength,
            boolean padDeclaredOptionalLength,
            boolean memberMustUnderstand,
            boolean padAfterValue) {
        if (pidFlags == null) {
            throw new IllegalArgumentException("a PID-flag policy, not null");
        }
        if (highestShortId < 0 || highestShortId > 0x3FFF) {
            throw new IllegalArgumentException(
                    "a short parameter id inside the low fourteen bits, not " + highestShortId);
        }
        this.pidFlags = pidFlags;
        this.highestShortId = highestShortId;
        this.padDeclaredLength = padDeclaredLength;
        this.padDeclaredOptionalLength = padDeclaredOptionalLength;
        this.memberMustUnderstand = memberMustUnderstand;
        this.padAfterValue = padAfterValue;
    }

    /** The clause's framing, which is this project's default dialect. */
    public static PlCdr1Framing defaults() {
        return new PlCdr1Framing(
                PidFlagPolicy.SENTINEL_MUST_UNDERSTAND,
                Writer.PID_SHORT_MAX_ID,
                false,
                false,
                true,
                true);
    }

    /** What flags a parameter id carries, asked per position. */
    public PidFlagPolicy pidFlags() {
        return pidFlags;
    }

    /** The highest member id a short parameter header carries. */
    public int highestShortId() {
        return highestShortId;
    }

    /** Whether a {@code @mutable} member's declared length covers padding. */
    public boolean padDeclaredLength() {
        return padDeclaredLength;
    }

    /** The same question for a non-mutable type's {@code @optional} member. */
    public boolean padDeclaredOptionalLength() {
        return padDeclaredOptionalLength;
    }

    /** Whether a member's own must-understand bit goes out. */
    public boolean memberMustUnderstand() {
        return memberMustUnderstand;
    }

    /** Whether the ended parameter writes its own trailing padding. */
    public boolean padAfterValue() {
        return padAfterValue;
    }

    /** Whether a parameter in this role declares a padded length. */
    public boolean padsDeclaredLength(ParameterRole role) {
        return role == ParameterRole.OPTIONAL ? padDeclaredOptionalLength : padDeclaredLength;
    }

    /** The same framing with another flag policy. */
    public PlCdr1Framing withPidFlags(PidFlagPolicy flags) {
        return new PlCdr1Framing(
                flags,
                highestShortId,
                padDeclaredLength,
                padDeclaredOptionalLength,
                memberMustUnderstand,
                padAfterValue);
    }

    /** The same framing with another short-id ceiling. */
    public PlCdr1Framing withHighestShortId(int id) {
        return new PlCdr1Framing(
                pidFlags,
                id,
                padDeclaredLength,
                padDeclaredOptionalLength,
                memberMustUnderstand,
                padAfterValue);
    }

    /** The same framing with another answer for a {@code @mutable} member. */
    public PlCdr1Framing withPadDeclaredLength(boolean pad) {
        return new PlCdr1Framing(
                pidFlags, highestShortId, pad, padDeclaredOptionalLength, memberMustUnderstand,
                padAfterValue);
    }

    /** The same framing with another answer for an {@code @optional} member. */
    public PlCdr1Framing withPadDeclaredOptionalLength(boolean pad) {
        return new PlCdr1Framing(
                pidFlags, highestShortId, padDeclaredLength, pad, memberMustUnderstand,
                padAfterValue);
    }

    /** The same framing writing, or not writing, a member's own flag. */
    public PlCdr1Framing withMemberMustUnderstand(boolean write) {
        return new PlCdr1Framing(
                pidFlags, highestShortId, padDeclaredLength, padDeclaredOptionalLength, write,
                padAfterValue);
    }

    @Override
    public boolean equals(Object other) {
        if (!(other instanceof PlCdr1Framing)) {
            return false;
        }
        PlCdr1Framing that = (PlCdr1Framing) other;
        return pidFlags == that.pidFlags
                && highestShortId == that.highestShortId
                && padDeclaredLength == that.padDeclaredLength
                && padDeclaredOptionalLength == that.padDeclaredOptionalLength
                && memberMustUnderstand == that.memberMustUnderstand
                && padAfterValue == that.padAfterValue;
    }

    @Override
    public int hashCode() {
        int hash = pidFlags.hashCode();
        hash = hash * 31 + highestShortId;
        hash = hash * 31 + (padDeclaredLength ? 1 : 0);
        hash = hash * 31 + (padDeclaredOptionalLength ? 1 : 0);
        hash = hash * 31 + (memberMustUnderstand ? 1 : 0);
        hash = hash * 31 + (padAfterValue ? 1 : 0);
        return hash;
    }

    @Override
    public String toString() {
        return "PlCdr1Framing[" + pidFlags.asString()
                + ", highestShortId=0x" + Integer.toHexString(highestShortId)
                + ", padDeclaredLength=" + padDeclaredLength
                + ", padDeclaredOptionalLength=" + padDeclaredOptionalLength
                + ", memberMustUnderstand=" + memberMustUnderstand
                + ", padAfterValue=" + padAfterValue + "]";
    }
}
