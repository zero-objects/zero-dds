// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

/**
 * How a scalar {@code wchar} is framed.
 *
 * <p>A {@code wstring} is delimited as a whole and a scalar {@code wchar} is
 * not, and CORBA 3.3 part 2 §9.3.1.6 gives the scalar its own framing. All
 * five DDS implementations write XTypes 1.3 Table 31's bare code unit; all
 * three ORBs write an octet count and then that many octets, aligned to one.
 * Measured 2026-08-10, {@code wchar a = U+00E4; wchar b = U+1234}, little
 * endian: {@code e400 3412} here, {@code 02 00e4 02 1234} from omniORB,
 * {@code 04 e4000000 04 34120000} from TAO.
 *
 * <p>It is not a width. A {@code wchar} that carries its own count is one
 * octet longer and aligns to one, so every member behind it moves.
 */
public enum WcharFraming {
    /** A bare code unit, aligned to its own width. XTypes 1.3 Table 31. */
    FIXED("fixed"),
    /**
     * One octet of length, then that many octets, aligned to one.
     * CORBA §9.3.1.6's GIOP 1.2 {@code wchar}.
     */
    GIOP_LENGTH_PREFIXED("giop-length-prefixed");

    private final String name;

    WcharFraming(String name) {
        this.name = name;
    }

    /** The command line's spelling. */
    public String asString() {
        return name;
    }

    /** The named framing, or {@code null} for anything else. */
    public static WcharFraming parse(String text) {
        for (WcharFraming framing : values()) {
            if (framing.name.equals(text)) {
                return framing;
            }
        }
        return null;
    }
}
