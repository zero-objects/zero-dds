// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

/**
 * What flags a PL_CDR1 parameter id carries, asked per position.
 *
 * <p>XTypes 1.3 §7.4.1.2 splits the sixteen-bit field: the low fourteen bits
 * are the id, bit 14 is {@link Writer#PID_MUST_UNDERSTAND} and bit 15 is
 * {@link Writer#PID_IMPL_EXTENSION}. The clause says nothing about what flags
 * {@code PID_LIST_END} carries, and the four measured implementations do not
 * agree — {@code struct-mutable} and
 * {@code evolution-mutable-mustunderstand} under XCDR1:
 *
 * <pre>
 *     Cyclone   member as declared    sentinel 7f02
 *     RTI       member as declared    sentinel 7f02
 *     Fast DDS  member never          sentinel 3f02
 *     OpenDDS   member c0xx, always   sentinel 3f02
 * </pre>
 *
 * <p>The sentinel's answer cannot be read off the member's, which is what
 * makes {@link Position#SENTINEL} a position on this axis rather than a
 * property of the list. {@link Position#EXTENDED_MARKER} is here for the same
 * reason and moved off the framing form —
 * {@code annotation-hashid-implicit} and {@code annotation-id-sparse},
 * XCDR1:
 *
 * <pre>
 *     Cyclone   always-extended    marker 7f01
 *     RTI       specification      marker 7f01
 *     Fast DDS  short-true-length  marker 3f01
 *     OpenDDS   short-true-length  marker ff01
 * </pre>
 *
 * <p>Two forms answer alike and two forms answer differently, so the marker's
 * flags are not a property of the form.
 *
 * <p>A member's <i>own</i> {@code must_understand} is not on this axis; it is
 * {@link PlCdr1Framing#memberMustUnderstand()}, and the writer ORs the two.
 */
public enum PidFlagPolicy {
    /**
     * A bare member id, {@code PID_MUST_UNDERSTAND} on the sentinel and on the
     * extended marker. Cyclone DDS and RTI Connext 7.7. The default.
     */
    SENTINEL_MUST_UNDERSTAND("sentinel-must-understand"),
    /** Bare everywhere. Fast DDS. */
    BARE("bare"),
    /**
     * Both flags on every member id whatever the member declared, a bare
     * sentinel, and both flags on the extended marker. OpenDDS. The
     * {@code eMemberHeader} behind the marker stays bare: OpenDDS's
     * {@code annotation-hashid-implicit} is
     * {@code ff010008 0343172c 00000004}.
     */
    IMPL_EXTENSION_ON_MEMBERS("impl-extension-members");

    /**
     * Where the parameter id being written or read sits.
     *
     * <p>The Java half of {@code zerodds_ast_layout::WirePosition}. An
     * argument and not a field, because one stream carries several positions
     * at once: a setting made when the writer was built could not answer for
     * all of them.
     */
    public enum Position {
        /** A member's own parameter id. */
        MEMBER("member"),
        /** An element of an array or a sequence, which has no parameter id. */
        ELEMENT("element"),
        /** The {@code PID_LIST_END} that closes a parameter list. */
        SENTINEL("sentinel"),
        /** The {@code PID_EXTENDED} marker that opens an extended header. */
        EXTENDED_MARKER("extended-marker");

        private final String name;

        Position(String name) {
            this.name = name;
        }

        /** The name {@code WirePosition::as_str} gives. */
        public String asString() {
            return name;
        }
    }

    private final String name;

    PidFlagPolicy(String name) {
        this.name = name;
    }

    /** The name {@code PidFlagPolicy::as_str} gives. */
    public String asString() {
        return name;
    }

    /**
     * The flag bits a parameter id carries at {@code position}.
     *
     * <p>Only the member, the sentinel and the extended marker have a
     * parameter id at all; {@link Position#ELEMENT} answers zero.
     */
    public int flagsAt(Position position) {
        switch (this) {
            case SENTINEL_MUST_UNDERSTAND:
                if (position == Position.SENTINEL || position == Position.EXTENDED_MARKER) {
                    return Writer.PID_MUST_UNDERSTAND;
                }
                return 0;
            case BARE:
                return 0;
            case IMPL_EXTENSION_ON_MEMBERS:
                if (position == Position.MEMBER || position == Position.EXTENDED_MARKER) {
                    return Writer.PID_IMPL_EXTENSION | Writer.PID_MUST_UNDERSTAND;
                }
                return 0;
            default:
                throw new IllegalStateException("unreachable: " + this);
        }
    }

    /** The named policy, or {@code null} for anything else. */
    public static PidFlagPolicy parse(String text) {
        for (PidFlagPolicy policy : values()) {
            if (policy.name.equals(text)) {
                return policy;
            }
        }
        return null;
    }
}
