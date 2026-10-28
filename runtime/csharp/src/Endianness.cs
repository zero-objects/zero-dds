// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
namespace ZeroDds.Wire
{
    /// <summary>
    /// Which end of a multi-byte value goes out first.
    /// </summary>
    /// <remarks>
    /// The C# counterpart of <c>crates/wire/src/endianness.rs</c> and
    /// <c>runtime/java/.../Endianness.java</c>. Its own type because the
    /// writer and the reader both need it and neither should own it.
    /// </remarks>
    public enum Endianness
    {
        /// <summary>The low byte goes out first.</summary>
        Little,

        /// <summary>The high byte goes out first.</summary>
        Big,
    }

    /// <summary>The two spellings a byte order answers to.</summary>
    public static class EndiannessNames
    {
        /// <summary>Whether the low byte goes out first.</summary>
        public static bool IsLittle(Endianness order)
        {
            return order == Endianness.Little;
        }

        /// <summary>The command line's spelling.</summary>
        public static string AsString(Endianness order)
        {
            return order == Endianness.Little ? "le" : "be";
        }

        /// <summary>
        /// The command line's spelling, or <c>null</c> for anything else.
        /// </summary>
        /// <remarks>
        /// <c>null</c> and not a default: a typo in a wire-affecting flag has
        /// to be loud, because the bytes it produces look plausible.
        /// </remarks>
        public static Endianness? Parse(string text)
        {
            if (text == "le" || text == "little")
            {
                return Endianness.Little;
            }

            if (text == "be" || text == "big")
            {
                return Endianness.Big;
            }

            return null;
        }
    }
}
