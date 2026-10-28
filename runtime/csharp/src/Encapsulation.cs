// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
using System;

namespace ZeroDds.Wire
{
    /// <summary>
    /// Encodings, and the four bytes that announce one.
    /// </summary>
    /// <remarks>
    /// DDSI-RTPS 2.5 §10.5 and XTypes 1.3 §7.4.1.1. Kept out of
    /// <see cref="Writer"/> because it frames a sample rather than being part
    /// of one: a KeyHash holder is written by the same writer and carries no
    /// header at all.
    /// </remarks>
    public static class Encapsulation
    {
        /// <summary>
        /// The four-byte encapsulation header.
        /// </summary>
        /// <remarks>
        /// The identifier itself is always big-endian on the wire, whatever the
        /// payload behind it is — it is what tells a reader which order to
        /// switch to.
        ///
        /// The two option bytes carry the payload's trailing padding count in
        /// the low two bits of the second one. XTypes 1.3 §7.6.3.1.2: those
        /// bits "shall be interpreted by the receiver to determine where the
        /// serialized data exactly ended". Measured on the loopback wire on
        /// 2026-08-09: Cyclone, RTI and Fast DDS all pad the payload to four
        /// and all three state the count — <c>@final struct One { octet a; }</c>
        /// goes out as <c>0001 0003 ab000000</c> from every one of them.
        /// </remarks>
        public static byte[] Header(Encoding encoding, Endianness endianness, int padding)
        {
            int identifier = Identifier(encoding, endianness);
            return new byte[]
            {
                (byte)(identifier >> 8),
                (byte)(identifier & 0xFF),
                0,
                (byte)(padding & 0x3),
            };
        }

        /// <summary>
        /// How many zero bytes bring <paramref name="length"/> up to the next
        /// multiple of four.
        /// </summary>
        public static int Padding(int length)
        {
            return (4 - (length & 3)) & 3;
        }

        /// <summary>The two-byte encapsulation identifier, as an unsigned number.</summary>
        public static int Identifier(Encoding encoding, Endianness endianness)
        {
            bool little = endianness == Endianness.Little;
            switch (encoding)
            {
                case Encoding.PlainCdr1: return little ? 0x0001 : 0x0000;
                case Encoding.PlCdr1: return little ? 0x0003 : 0x0002;
                case Encoding.PlainCdr2: return little ? 0x0007 : 0x0006;
                case Encoding.DelimitedCdr: return little ? 0x0009 : 0x0008;
                case Encoding.PlCdr2: return little ? 0x000B : 0x000A;
                default:
                    throw new ArgumentOutOfRangeException(nameof(encoding), "an encoding");
            }
        }
    }
}
