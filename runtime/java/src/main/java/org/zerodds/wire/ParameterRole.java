// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

/**
 * Which of XTypes 1.3 §7.4.1.2's two roles a parameter header stands in.
 *
 * <p>The header is the same header. The role decides what the length field
 * counts, and whether a member that took no value writes one at all: a
 * {@code @mutable} member that is absent leaves no trace in the parameter
 * list, an {@code @optional} member of a non-mutable type writes its header
 * with a declared length of zero.
 */
public enum ParameterRole {
    /** A member of a {@code @mutable} type. Rules (24) and (25). */
    MEMBER("member"),
    /**
     * An {@code @optional} member of a type that is not {@code @mutable}.
     * Rule (19).
     */
    OPTIONAL("optional");

    private final String name;

    ParameterRole(String name) {
        this.name = name;
    }

    /** The name {@code ParameterRole::as_str} gives. */
    public String asString() {
        return name;
    }
}
