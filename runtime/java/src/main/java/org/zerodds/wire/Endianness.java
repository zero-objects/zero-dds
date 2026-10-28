// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

/**
 * Which end of a multi-byte value goes out first.
 *
 * <p>The Java counterpart of {@code crates/wire/src/endianness.rs} and
 * {@code runtime/python/zerodds_wire/endianness.py}. Its own type because the
 * writer and the reader both need it and neither should own it.
 */
public enum Endianness {
    /** The low byte goes out first. */
    LITTLE("le"),
    /** The high byte goes out first. */
    BIG("be");

    private final String name;

    Endianness(String name) {
        this.name = name;
    }

    /** Whether the low byte goes out first. */
    public boolean isLittle() {
        return this == LITTLE;
    }

    /** The command line's spelling. */
    public String asString() {
        return name;
    }

    /**
     * The command line's spelling, or {@code null} for anything else.
     *
     * <p>{@code null} and not a default: a typo in a wire-affecting flag has to
     * be loud, because the bytes it produces look plausible.
     */
    public static Endianness parse(String text) {
        if ("le".equals(text) || "little".equals(text)) {
            return LITTLE;
        }
        if ("be".equals(text) || "big".equals(text)) {
            return BIG;
        }
        return null;
    }
}
