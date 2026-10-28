// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
using System;

namespace ZeroDds.Wire
{
    /// <summary>
    /// The MD5 a KeyHash is made of.
    /// </summary>
    /// <remarks>
    /// <para>
    /// DDSI-RTPS 2.5 §9.6.3.8 names MD5 for the KeyHash of a key holder longer
    /// than sixteen bytes. It is <b>not</b> a security claim: the KeyHash
    /// identifies an instance and is not a signature, and nothing in this
    /// project relies on MD5 being hard to invert.
    /// </para>
    /// <para>
    /// RFC 1321 by hand, where the Java column calls
    /// <c>MessageDigest.getInstance("MD5")</c>. The reason is not distrust of
    /// the platform: <c>System.Security.Cryptography.MD5</c> is unavailable
    /// under a FIPS-enforcing policy — .NET throws
    /// <c>InvalidOperationException</c> from the managed implementation and
    /// the OS refuses the native one — and a KeyHash that stops working
    /// because a machine was put in FIPS mode is a wire feature that depends
    /// on a security policy it has nothing to do with. The C and Rust columns
    /// carry RFC 1321 by hand for the freestanding-target reason;
    /// <c>WireSelfTest</c> checks this one against RFC 1321's own vectors, so
    /// the arithmetic is asserted rather than assumed.
    /// </para>
    /// </remarks>
    public static class Md5
    {
        /// <summary>A digest is sixteen bytes.</summary>
        public const int Length = 16;

        /// <summary>Per-round shift amounts. RFC 1321 §3.4.</summary>
        private static readonly int[] Shifts =
        {
            7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
            5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
            4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
            6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
        };

        /// <summary>
        /// The sine table of RFC 1321 §3.4: <c>floor(abs(sin(i + 1)) * 2^32)</c>.
        /// </summary>
        /// <remarks>
        /// Written out rather than computed, because computing it would make
        /// the digest depend on the platform's <c>Math.Sin</c>.
        /// </remarks>
        private static readonly uint[] Sine =
        {
            0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
            0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
            0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
            0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
            0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
            0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
            0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
            0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
            0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
            0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
            0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
            0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
            0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
            0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
            0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
            0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
        };

        /// <summary>The MD5 digest of <paramref name="input"/>.</summary>
        public static byte[] Digest(byte[] input)
        {
            if (input == null)
            {
                throw new ArgumentNullException(nameof(input));
            }

            return Digest(input, 0, input.Length);
        }

        /// <summary>
        /// The MD5 digest of <paramref name="length"/> bytes of
        /// <paramref name="input"/> from <paramref name="from"/>.
        /// </summary>
        public static byte[] Digest(byte[] input, int from, int length)
        {
            if (input == null)
            {
                throw new ArgumentNullException(nameof(input));
            }

            if (from < 0 || length < 0 || from + length > input.Length)
            {
                throw new ArgumentOutOfRangeException(nameof(length), "a range inside the input");
            }

            uint a = 0x67452301;
            uint b = 0xefcdab89;
            uint c = 0x98badcfe;
            uint d = 0x10325476;

            // The message, padded: a 0x80 octet, zeros to 56 mod 64, then the
            // bit length little-endian. Built whole rather than streamed,
            // because a KeyHash holder is a sample and not a file.
            int padded = length + 1;
            padded += (56 - (padded % 64) + 64) % 64;
            byte[] message = new byte[padded + 8];
            Array.Copy(input, from, message, 0, length);
            message[length] = 0x80;
            ulong bits = (ulong)length * 8UL;
            for (int index = 0; index < 8; index++)
            {
                message[padded + index] = (byte)(bits >> (8 * index));
            }

            uint[] block = new uint[16];
            for (int start = 0; start < message.Length; start += 64)
            {
                for (int word = 0; word < 16; word++)
                {
                    int at = start + (word * 4);
                    block[word] = message[at]
                        | ((uint)message[at + 1] << 8)
                        | ((uint)message[at + 2] << 16)
                        | ((uint)message[at + 3] << 24);
                }

                uint aa = a;
                uint bb = b;
                uint cc = c;
                uint dd = d;
                for (int step = 0; step < 64; step++)
                {
                    uint mixed;
                    int which;
                    if (step < 16)
                    {
                        mixed = (bb & cc) | (~bb & dd);
                        which = step;
                    }
                    else if (step < 32)
                    {
                        mixed = (dd & bb) | (~dd & cc);
                        which = ((5 * step) + 1) % 16;
                    }
                    else if (step < 48)
                    {
                        mixed = bb ^ cc ^ dd;
                        which = ((3 * step) + 5) % 16;
                    }
                    else
                    {
                        mixed = cc ^ (bb | ~dd);
                        which = (7 * step) % 16;
                    }

                    uint rotated = aa + mixed + Sine[step] + block[which];
                    aa = dd;
                    dd = cc;
                    cc = bb;
                    bb = bb + RotateLeft(rotated, Shifts[step]);
                }

                a += aa;
                b += bb;
                c += cc;
                d += dd;
            }

            byte[] digest = new byte[Length];
            Store(digest, 0, a);
            Store(digest, 4, b);
            Store(digest, 8, c);
            Store(digest, 12, d);
            return digest;
        }

        private static uint RotateLeft(uint value, int count)
        {
            return (value << count) | (value >> (32 - count));
        }

        private static void Store(byte[] into, int at, uint value)
        {
            into[at] = (byte)value;
            into[at + 1] = (byte)(value >> 8);
            into[at + 2] = (byte)(value >> 16);
            into[at + 3] = (byte)(value >> 24);
        }
    }
}
