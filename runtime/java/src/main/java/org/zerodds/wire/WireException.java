// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
package org.zerodds.wire;

/**
 * What can go wrong on the wire.
 *
 * <p>The variants of {@code crates/wire/src/error.rs} and of
 * {@code runtime/python/zerodds_wire/error.py}, as exceptions. Every one names
 * what was expected and where: a reader that returned a zero for a truncated
 * stream would hand the caller a value that was never sent, and a writer that
 * clamped an over-wide integer would put a different number on the wire than
 * the caller asked for.
 *
 * <p>Unchecked, because every one of them is a defect in the caller or in the
 * peer and none is a condition a generated encoder recovers from. The four
 * kinds are nested rather than four top-level files so that one {@code catch}
 * of this class is the whole surface.
 */
public class WireException extends RuntimeException {

    private static final long serialVersionUID = 1L;

    /** Where in the stream the failure was. */
    private final int at;

    WireException(int at, String message) {
        super(message);
        this.at = at;
    }

    /** The offset the failure was at. */
    public int at() {
        return at;
    }

    /** The stream ended before the value did. */
    public static final class Truncated extends WireException {
        private static final long serialVersionUID = 1L;

        private final long needed;
        private final long available;

        /** @param at where the read began, {@code needed} how many bytes it
         *     wanted, {@code available} how many were left. */
        public Truncated(int at, long needed, long available) {
            super(at, "at " + at + ": " + needed + " bytes needed, " + available + " available");
            this.needed = needed;
            this.available = available;
        }

        /** How many bytes the read wanted. */
        public long needed() {
            return needed;
        }

        /** How many were left. */
        public long available() {
            return available;
        }
    }

    /**
     * A length field claims more than the stream holds.
     *
     * <p>Separate from {@link Truncated}: a length that cannot be true is a
     * malformed stream, not a short one, and allocating for it is how a reader
     * turns a bad message into an out-of-memory.
     */
    public static final class ImpossibleLength extends WireException {
        private static final long serialVersionUID = 1L;

        private final long claimed;
        private final long available;

        /** @param at where the length field was, {@code claimed} what it said,
         *     {@code available} how many bytes were left behind it. */
        public ImpossibleLength(int at, long claimed, long available) {
            super(at, "at " + at + ": a length of " + claimed + " with " + available
                    + " bytes left");
            this.claimed = claimed;
            this.available = available;
        }

        /** What the length field claimed. */
        public long claimed() {
            return claimed;
        }

        /** How many bytes were left behind it. */
        public long available() {
            return available;
        }
    }

    /**
     * Text that is not well-formed UTF-8, in either direction.
     *
     * <p>The write side needs this one where Python's does not: Java's
     * {@code String.getBytes(UTF_8)} replaces an unpaired surrogate with a
     * {@code ?} and reports success, which puts a different character on the
     * wire silently. {@link Writer} encodes through a reporting
     * {@code CharsetEncoder} and raises this instead.
     */
    public static final class NotUtf8 extends WireException {
        private static final long serialVersionUID = 1L;

        /** @param at where the text began. */
        public NotUtf8(int at) {
            super(at, "at " + at + ": the string is not UTF-8");
        }

        /** @param at where the text began, {@code detail} what was wrong. */
        public NotUtf8(int at, String detail) {
            super(at, "at " + at + ": the string is not UTF-8: " + detail);
        }
    }

    /**
     * A value outside what its type admits.
     *
     * <p>Raised by the writer as well as the reader, which is where Java
     * differs from the Rust runtime: there a {@code u32} is a {@code u32} and
     * the compiler has already refused the value, while here it arrives as a
     * {@code long} and the check has to happen at the point of the write.
     */
    public static final class OutOfRange extends WireException {
        private static final long serialVersionUID = 1L;

        private final String expected;

        /** @param at where the value was, {@code expected} what would have
         *     been admissible. */
        public OutOfRange(int at, String expected) {
            super(at, "at " + at + ": expected " + expected);
            this.expected = expected;
        }

        /** What would have been admissible. */
        public String expected() {
            return expected;
        }
    }
}
