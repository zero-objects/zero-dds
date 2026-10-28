// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
using System;

namespace ZeroDds.Wire
{
    /// <summary>
    /// What can go wrong on the wire.
    /// </summary>
    /// <remarks>
    /// The variants of <c>crates/wire/src/error.rs</c> and of
    /// <c>runtime/java/.../WireException.java</c>, as exceptions. Every one
    /// names what was expected and where: a reader that returned a zero for a
    /// truncated stream would hand the caller a value that was never sent, and
    /// a writer that clamped an over-wide integer would put a different number
    /// on the wire than the caller asked for.
    ///
    /// The four kinds are nested rather than four top-level classes so that one
    /// <c>catch (WireException)</c> is the whole surface.
    /// </remarks>
    public class WireException : Exception
    {
        /// <summary>A failure at <paramref name="at"/> with a message.</summary>
        public WireException(int at, string message)
            : base(message)
        {
            At = at;
        }

        /// <summary>A failure with no offset, for the framework's own callers.</summary>
        public WireException()
            : base("a wire failure")
        {
            At = 0;
        }

        /// <summary>A failure with a message and no offset.</summary>
        public WireException(string message)
            : base(message)
        {
            At = 0;
        }

        /// <summary>A failure with a message and a cause.</summary>
        public WireException(string message, Exception inner)
            : base(message, inner)
        {
            At = 0;
        }

        /// <summary>The offset the failure was at.</summary>
        public int At { get; private set; }

        /// <summary>The stream ended before the value did.</summary>
        public sealed class Truncated : WireException
        {
            /// <summary>
            /// A read at <paramref name="at"/> that wanted
            /// <paramref name="needed"/> bytes with <paramref name="available"/>
            /// left.
            /// </summary>
            public Truncated(int at, long needed, long available)
                : base(at, "at " + at + ": " + needed + " bytes needed, " + available + " available")
            {
                Needed = needed;
                Available = available;
            }

            /// <summary>How many bytes the read wanted.</summary>
            public long Needed { get; private set; }

            /// <summary>How many were left.</summary>
            public long Available { get; private set; }
        }

        /// <summary>
        /// A length field claims more than the stream holds.
        /// </summary>
        /// <remarks>
        /// Separate from <see cref="Truncated"/>: a length that cannot be true
        /// is a malformed stream, not a short one, and allocating for it is how
        /// a reader turns a bad message into an out-of-memory.
        /// </remarks>
        public sealed class ImpossibleLength : WireException
        {
            /// <summary>
            /// A length field at <paramref name="at"/> claiming
            /// <paramref name="claimed"/> with <paramref name="available"/>
            /// bytes behind it.
            /// </summary>
            public ImpossibleLength(int at, long claimed, long available)
                : base(at, "at " + at + ": a length of " + claimed + " with " + available
                    + " bytes left")
            {
                Claimed = claimed;
                Available = available;
            }

            /// <summary>What the length field claimed.</summary>
            public long Claimed { get; private set; }

            /// <summary>How many bytes were left behind it.</summary>
            public long Available { get; private set; }
        }

        /// <summary>
        /// Text that is not well-formed UTF-8, in either direction.
        /// </summary>
        /// <remarks>
        /// The write side needs this one for the reason Java's does: a .NET
        /// <c>string</c> is a sequence of UTF-16 code units and admits a lone
        /// surrogate, which is not a code point. <c>Encoding.UTF8</c> replaces
        /// one with U+FFFD and reports success, which puts a different
        /// character on the wire silently; <see cref="Writer"/> encodes through
        /// a throwing encoder and raises this instead.
        /// </remarks>
        public sealed class NotUtf8 : WireException
        {
            /// <summary>Text at <paramref name="at"/> that is not UTF-8.</summary>
            public NotUtf8(int at)
                : base(at, "at " + at + ": the string is not UTF-8")
            {
            }

            /// <summary>The same, naming what was wrong.</summary>
            public NotUtf8(int at, string detail)
                : base(at, "at " + at + ": the string is not UTF-8: " + detail)
            {
            }
        }

        /// <summary>
        /// A value outside what its type admits.
        /// </summary>
        /// <remarks>
        /// Raised by the writer as well as the reader. Where the Java column
        /// checks a range on every unsigned write because Java has no unsigned
        /// type, this one does not: C# has <c>byte</c>, <c>ushort</c>,
        /// <c>uint</c> and <c>ulong</c>, so the compiler has already refused
        /// the out-of-range value and the check would be dead. What is left
        /// here is the ranges no C# type expresses — a <c>char</c> below
        /// U+0100, an <c>@bit_bound</c> between 1 and 32, a BCD field that
        /// fits <c>fixed&lt;P,S&gt;</c>.
        /// </remarks>
        public sealed class OutOfRange : WireException
        {
            /// <summary>
            /// A value at <paramref name="at"/> where
            /// <paramref name="expected"/> would have been admissible.
            /// </summary>
            public OutOfRange(int at, string expected)
                : base(at, "at " + at + ": expected " + expected)
            {
                Expected = expected;
            }

            /// <summary>What would have been admissible.</summary>
            public string Expected { get; private set; }
        }
    }
}
