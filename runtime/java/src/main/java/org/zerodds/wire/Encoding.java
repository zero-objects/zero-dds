// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

/**
 * Which encoding a plan and a stream are in.
 *
 * <p>The five of {@code zerodds_ast_layout::Encoding}. XCDR1 and XCDR2 differ
 * in their framing: XCDR1 has no delimiter header, and {@code @mutable} uses
 * PID headers there and EMHEADERs here.
 */
public enum Encoding {
    /** Plain XCDR version 1. */
    PLAIN_CDR1("plain-cdr1"),
    /** A parameter list under XCDR version 1. */
    PL_CDR1("pl-cdr1"),
    /** Plain XCDR version 2. */
    PLAIN_CDR2("plain-cdr2"),
    /** A delimited aggregate under XCDR version 2. */
    DELIMITED_CDR("delimited-cdr"),
    /** A parameter list under XCDR version 2. */
    PL_CDR2("pl-cdr2");

    private final String name;

    Encoding(String name) {
        this.name = name;
    }

    /** The command line's spelling. */
    public String asString() {
        return name;
    }

    /** Whether this is an XCDR version 1 encoding. */
    public boolean isV1() {
        return this == PLAIN_CDR1 || this == PL_CDR1;
    }

    /** Whether this is an XCDR version 2 encoding. */
    public boolean isV2() {
        return !isV1();
    }

    /**
     * The largest alignment any primitive takes.
     *
     * <p>XTypes 1.3 §7.4.1: {@code MAXALIGN(VERSION2) = 4},
     * {@code MAXALIGN(VERSION1) = 8}, and {@code ALIGN(N)} aligns to
     * {@code MIN(N, XCDR.maxalign)}. §7.6.8 repeats it for the key holder:
     * "the maximum alignment in PLAIN_CDR2 (i.e., 4)".
     *
     * <p>This is the number a {@link Writer} and a {@link Reader} are built
     * with. A writer given eight under XCDR2 pads an eight-byte primitive to
     * eight and puts four bytes on the wire that no peer expects.
     */
    public int maxAlignment() {
        return isV1() ? 8 : 4;
    }

    /**
     * The command line's spelling, including the two shorthands, or
     * {@code null} for anything else.
     *
     * <p>{@code xcdr1} and {@code xcdr2} name the plain form, because that is
     * what a caller who has not thought about extensibility means. An unknown
     * name is {@code null} and never a default.
     */
    public static Encoding parse(String text) {
        if ("xcdr1".equals(text)) {
            return PLAIN_CDR1;
        }
        if ("xcdr2".equals(text)) {
            return PLAIN_CDR2;
        }
        for (Encoding encoding : values()) {
            if (encoding.name.equals(text)) {
                return encoding;
            }
        }
        return null;
    }
}
