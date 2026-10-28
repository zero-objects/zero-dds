// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

/**
 * How wide a scalar {@code wchar} is.
 *
 * <p>Two octets under XTypes 1.3 Table 31 and for seven of the nine measured
 * implementations; four under RTI Connext's XCDR1 — measured 2026-08-09,
 * {@code wchar c = U+0061} gives {@code 61000000} under XCDR1 and {@code 6100}
 * under XCDR2 — under TAO, and under
 * {@code rosidl_typesupport_fastrtps_c}.
 *
 * <p>An argument of its own and not a reading of the {@code wstring} policy:
 * {@code rmw_cyclonedds_cpp} 1.3.4 writes two octets here and four inside a
 * {@code wstring}.
 *
 * <p>The four-octet form zero-extends a code unit; it does not carry a code
 * point, so §7.2.2.2.1.2.5's Basic-Multilingual-Plane restriction holds at
 * both widths.
 */
public enum WcharWidth {
    /** Two octets. XTypes 1.3 Table 31. */
    OCTETS_2(2),
    /** Four octets. RTI Connext under XCDR1, TAO, one ROS type support. */
    OCTETS_4(4);

    private final int octets;

    WcharWidth(int octets) {
        this.octets = octets;
    }

    /** The width in octets, which is 2 or 4. */
    public int octets() {
        return octets;
    }

    /** The width a number names, or {@code null} for anything else. */
    public static WcharWidth of(int octets) {
        if (octets == 2) {
            return OCTETS_2;
        }
        if (octets == 4) {
            return OCTETS_4;
        }
        return null;
    }
}
