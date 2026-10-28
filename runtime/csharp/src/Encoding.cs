// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
namespace ZeroDds.Wire
{
    /// <summary>
    /// Which encoding a plan and a stream are in.
    /// </summary>
    /// <remarks>
    /// The five of <c>zerodds_ast_layout::Encoding</c>. XCDR1 and XCDR2 differ
    /// in their framing: XCDR1 has no delimiter header, and <c>@mutable</c>
    /// uses PID headers there and EMHEADERs here.
    /// </remarks>
    public enum Encoding
    {
        /// <summary>Plain XCDR version 1.</summary>
        PlainCdr1,

        /// <summary>A parameter list under XCDR version 1.</summary>
        PlCdr1,

        /// <summary>Plain XCDR version 2.</summary>
        PlainCdr2,

        /// <summary>A delimited aggregate under XCDR version 2.</summary>
        DelimitedCdr,

        /// <summary>A parameter list under XCDR version 2.</summary>
        PlCdr2,
    }

    /// <summary>What an encoding answers about itself.</summary>
    public static class EncodingFacts
    {
        /// <summary>The command line's spelling.</summary>
        public static string AsString(Encoding encoding)
        {
            switch (encoding)
            {
                case Encoding.PlainCdr1: return "plain-cdr1";
                case Encoding.PlCdr1: return "pl-cdr1";
                case Encoding.PlainCdr2: return "plain-cdr2";
                case Encoding.DelimitedCdr: return "delimited-cdr";
                default: return "pl-cdr2";
            }
        }

        /// <summary>Whether this is an XCDR version 1 encoding.</summary>
        public static bool IsV1(Encoding encoding)
        {
            return encoding == Encoding.PlainCdr1 || encoding == Encoding.PlCdr1;
        }

        /// <summary>
        /// The largest alignment any primitive takes.
        /// </summary>
        /// <remarks>
        /// XTypes 1.3 §7.4.1: <c>MAXALIGN(VERSION2) = 4</c>,
        /// <c>MAXALIGN(VERSION1) = 8</c>, and <c>ALIGN(N)</c> aligns to
        /// <c>MIN(N, XCDR.maxalign)</c>. §7.6.8 repeats it for the key holder:
        /// "the maximum alignment in PLAIN_CDR2 (i.e., 4)".
        ///
        /// This is the number a <see cref="Writer"/> and a <see cref="Reader"/>
        /// are built with. A writer given eight under XCDR2 pads an eight-byte
        /// primitive to eight and puts four bytes on the wire that no peer
        /// expects.
        /// </remarks>
        public static int MaxAlignment(Encoding encoding)
        {
            return IsV1(encoding) ? 8 : 4;
        }

        /// <summary>
        /// The command line's spelling, including the two shorthands, or
        /// <c>null</c> for anything else.
        /// </summary>
        public static Encoding? Parse(string text)
        {
            switch (text)
            {
                case "xcdr1":
                case "plain-cdr1": return Encoding.PlainCdr1;
                case "pl-cdr1": return Encoding.PlCdr1;
                case "xcdr2":
                case "plain-cdr2": return Encoding.PlainCdr2;
                case "delimited-cdr": return Encoding.DelimitedCdr;
                case "pl-cdr2": return Encoding.PlCdr2;
                default: return null;
            }
        }
    }
}
