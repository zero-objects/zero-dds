// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

/**
 * How a {@code wstring}'s length is counted, and how wide its code units are.
 *
 * <p>Three independent questions, so eight cells: C1 — does the length prefix
 * count characters or the octets they occupy; C2 — is a terminating NUL code
 * unit written and counted; C3 — is one UTF-16 code unit two octets or four.
 * All eight crossings are named, including the three no measured
 * implementation writes, because a cell that has no name cannot be reached
 * from a command line to be measured against — which is exactly how the ROS
 * form went unreachable while two implementations were writing it.
 *
 * <p>Mirrors {@code zerodds_ast_layout::WstringLengthPolicy} and
 * {@code zdw_wstring_policy}; the names {@link #asString()} gives are that
 * type's {@code as_str}, and {@link #coordinates()} spells the same cell as
 * {@code counts/terminator/width} so that a cell nobody has named is still
 * reachable.
 */
public enum WstringLengthPolicy {
    /**
     * Octets, no terminator, two-octet units. XTypes 1.3 §7.4.1.1.2 and
     * CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2 {@code wstring} over a two-octet
     * code set. Cyclone and OpenDDS under both versions, RTI under XCDR2,
     * omniORB, JacORB. The default.
     */
    SPECIFICATION("specification", false, false, 2),
    /**
     * Characters, no terminator, two-octet units. Fast DDS.
     *
     * <p>A code point outside the Basic Multilingual Plane is refused rather
     * than truncated: reproducing a vendor's length convention is a policy,
     * reproducing its data loss is not.
     */
    CHARACTER_COUNT("character-count", true, false, 2),
    /**
     * Characters including a NUL, four-octet units. CORBA §9.3.1.6's GIOP 1.1
     * {@code wstring} over a four-octet code set, and RTI Connext 7.7 under
     * XCDR1. The four-octet characters are UTF-16 code units zero-extended,
     * so a surrogate pair survives.
     */
    CHARACTER_COUNT_TERMINATED("character-count-terminated", true, true, 4),
    /**
     * Octets, no terminator, four-octet units. CORBA §9.3.1.6's GIOP 1.2
     * {@code wstring} over a four-octet transmission code set — what TAO
     * 2.5.24 writes.
     */
    SPECIFICATION_WIDE_UNITS("specification-wide-units", false, false, 4),
    /**
     * Characters, no terminator, four-octet units.
     * {@code rosidl_typesupport_fastrtps} counts characters like Fast CDR and
     * hands each over as a four-octet {@code wchar_t};
     * {@code rmw_cyclonedds_cpp} arrives at the same octets through the
     * introspection type support.
     */
    ROS_TYPE_SUPPORT("ros-type-support", true, false, 4),
    /** One of the three cells no measured implementation writes. */
    OCTETS_TERMINATED_2("octets/terminated/2", false, true, 2),
    /** One of the three cells no measured implementation writes. */
    OCTETS_TERMINATED_4("octets/terminated/4", false, true, 4),
    /** One of the three cells no measured implementation writes. */
    CHARACTERS_TERMINATED_2("characters/terminated/2", true, true, 2);

    private final String name;
    private final boolean countsCharacters;
    private final boolean terminated;
    private final int unitWidth;

    WstringLengthPolicy(String name, boolean countsCharacters, boolean terminated, int unitWidth) {
        this.name = name;
        this.countsCharacters = countsCharacters;
        this.terminated = terminated;
        this.unitWidth = unitWidth;
    }

    /** The name {@code WstringLengthPolicy::as_str} gives. */
    public String asString() {
        return name;
    }

    /** C1 — whether the length prefix counts characters and not octets. */
    public boolean countsCharacters() {
        return countsCharacters;
    }

    /** C2 — whether a terminating NUL code unit is written and counted. */
    public boolean isTerminated() {
        return terminated;
    }

    /**
     * C3 — how many bytes one of a {@code wstring}'s code units takes.
     *
     * <p><b>Not</b> the width of a scalar {@code wchar}, which is
     * {@link WcharWidth}: {@code rmw_cyclonedds_cpp} 1.3.4 writes two octets
     * for a {@code wchar} member and four for a {@code wstring}'s units.
     */
    public int unitWidth() {
        return unitWidth;
    }

    /** The same cell spelled {@code counts/terminator/width}. */
    public String coordinates() {
        return (countsCharacters ? "characters" : "octets")
                + "/" + (terminated ? "terminated" : "unterminated")
                + "/" + unitWidth;
    }

    /**
     * The cell a name or a coordinate spelling picks out, or {@code null}.
     *
     * <p>Every cell is reachable by its coordinates as well as by its name,
     * exactly as {@code WstringLengthPolicy::parse} accepts both: a cell that
     * can only be named cannot be asked for by someone who has measured it and
     * not yet named it.
     */
    public static WstringLengthPolicy parse(String text) {
        for (WstringLengthPolicy policy : values()) {
            if (policy.name.equals(text) || policy.coordinates().equals(text)) {
                return policy;
            }
        }
        return null;
    }
}
