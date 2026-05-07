// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// MD5-Hash fuer DDS-Key-Hash gemaess XTypes 1.3 §7.6.8.
// Spec: zerodds-xcdr2-csharp-1.0 §7.

using System;

namespace ZeroDDS.Cdr;

/// <summary>
/// RFC 1321 MD5-Hash, deterministisch.  Wird ausschliesslich als
/// Key-Hash-Berechnung im DDS-XCDR2-Stack verwendet (nicht als
/// kryptographisch starke Funktion).
///
/// Implementation laut RFC 1321; vendor-side damit FIPS-mode-Hosts
/// (in denen `System.Security.Cryptography.MD5` deaktiviert ist)
/// das DDS-Binding weiter benutzen koennen.
/// </summary>
public static class Md5
{
    /// <summary>Hashed `data` und liefert die 16 Output-Bytes.</summary>
    public static byte[] Hash(ReadOnlySpan<byte> data)
    {
        // Initial state per RFC 1321 §3.3.
        uint a = 0x67452301u;
        uint b = 0xefcdab89u;
        uint c = 0x98badcfeu;
        uint d = 0x10325476u;

        // Padding berechnen: append 0x80, dann 0x00 bis len % 64 == 56,
        // dann 8-Byte little-endian original-bit-length.
        long bitLen = (long)data.Length * 8;
        int padLen;
        int rem = data.Length % 64;
        if (rem < 56) padLen = 56 - rem;
        else padLen = 56 + (64 - rem);

        int totalLen = data.Length + padLen + 8;
        var padded = new byte[totalLen];
        data.CopyTo(padded);
        padded[data.Length] = 0x80;
        // padLen-1 weitere Null-Bytes sind bereits 0 (default).
        // 8-Byte Bit-Length LE:
        for (int i = 0; i < 8; i++)
        {
            padded[data.Length + padLen + i] = (byte)((bitLen >> (8 * i)) & 0xff);
        }

        // 64-Byte-Bloecke verarbeiten.
        for (int blockStart = 0; blockStart < totalLen; blockStart += 64)
        {
            ProcessBlock(padded, blockStart, ref a, ref b, ref c, ref d);
        }

        var output = new byte[16];
        WriteUInt32LittleEndian(output, 0, a);
        WriteUInt32LittleEndian(output, 4, b);
        WriteUInt32LittleEndian(output, 8, c);
        WriteUInt32LittleEndian(output, 12, d);
        return output;
    }

    private static void ProcessBlock(byte[] block, int offset, ref uint a, ref uint b, ref uint c, ref uint d)
    {
        // 16 32-bit-Worte LE.
        Span<uint> m = stackalloc uint[16];
        for (int i = 0; i < 16; i++)
        {
            m[i] = ReadUInt32LittleEndian(block, offset + i * 4);
        }

        uint aa = a, bb = b, cc = c, dd = d;

        // Round 1.
        Step(ref aa, bb, cc, dd, m[0],   7, 0xd76aa478);
        Step(ref dd, aa, bb, cc, m[1],  12, 0xe8c7b756);
        Step(ref cc, dd, aa, bb, m[2],  17, 0x242070db);
        Step(ref bb, cc, dd, aa, m[3],  22, 0xc1bdceee);
        Step(ref aa, bb, cc, dd, m[4],   7, 0xf57c0faf);
        Step(ref dd, aa, bb, cc, m[5],  12, 0x4787c62a);
        Step(ref cc, dd, aa, bb, m[6],  17, 0xa8304613);
        Step(ref bb, cc, dd, aa, m[7],  22, 0xfd469501);
        Step(ref aa, bb, cc, dd, m[8],   7, 0x698098d8);
        Step(ref dd, aa, bb, cc, m[9],  12, 0x8b44f7af);
        Step(ref cc, dd, aa, bb, m[10], 17, 0xffff5bb1);
        Step(ref bb, cc, dd, aa, m[11], 22, 0x895cd7be);
        Step(ref aa, bb, cc, dd, m[12],  7, 0x6b901122);
        Step(ref dd, aa, bb, cc, m[13], 12, 0xfd987193);
        Step(ref cc, dd, aa, bb, m[14], 17, 0xa679438e);
        Step(ref bb, cc, dd, aa, m[15], 22, 0x49b40821);

        // Round 2.
        StepG(ref aa, bb, cc, dd, m[1],   5, 0xf61e2562);
        StepG(ref dd, aa, bb, cc, m[6],   9, 0xc040b340);
        StepG(ref cc, dd, aa, bb, m[11], 14, 0x265e5a51);
        StepG(ref bb, cc, dd, aa, m[0],  20, 0xe9b6c7aa);
        StepG(ref aa, bb, cc, dd, m[5],   5, 0xd62f105d);
        StepG(ref dd, aa, bb, cc, m[10],  9, 0x02441453);
        StepG(ref cc, dd, aa, bb, m[15], 14, 0xd8a1e681);
        StepG(ref bb, cc, dd, aa, m[4],  20, 0xe7d3fbc8);
        StepG(ref aa, bb, cc, dd, m[9],   5, 0x21e1cde6);
        StepG(ref dd, aa, bb, cc, m[14],  9, 0xc33707d6);
        StepG(ref cc, dd, aa, bb, m[3],  14, 0xf4d50d87);
        StepG(ref bb, cc, dd, aa, m[8],  20, 0x455a14ed);
        StepG(ref aa, bb, cc, dd, m[13],  5, 0xa9e3e905);
        StepG(ref dd, aa, bb, cc, m[2],   9, 0xfcefa3f8);
        StepG(ref cc, dd, aa, bb, m[7],  14, 0x676f02d9);
        StepG(ref bb, cc, dd, aa, m[12], 20, 0x8d2a4c8a);

        // Round 3.
        StepH(ref aa, bb, cc, dd, m[5],   4, 0xfffa3942);
        StepH(ref dd, aa, bb, cc, m[8],  11, 0x8771f681);
        StepH(ref cc, dd, aa, bb, m[11], 16, 0x6d9d6122);
        StepH(ref bb, cc, dd, aa, m[14], 23, 0xfde5380c);
        StepH(ref aa, bb, cc, dd, m[1],   4, 0xa4beea44);
        StepH(ref dd, aa, bb, cc, m[4],  11, 0x4bdecfa9);
        StepH(ref cc, dd, aa, bb, m[7],  16, 0xf6bb4b60);
        StepH(ref bb, cc, dd, aa, m[10], 23, 0xbebfbc70);
        StepH(ref aa, bb, cc, dd, m[13],  4, 0x289b7ec6);
        StepH(ref dd, aa, bb, cc, m[0],  11, 0xeaa127fa);
        StepH(ref cc, dd, aa, bb, m[3],  16, 0xd4ef3085);
        StepH(ref bb, cc, dd, aa, m[6],  23, 0x04881d05);
        StepH(ref aa, bb, cc, dd, m[9],   4, 0xd9d4d039);
        StepH(ref dd, aa, bb, cc, m[12], 11, 0xe6db99e5);
        StepH(ref cc, dd, aa, bb, m[15], 16, 0x1fa27cf8);
        StepH(ref bb, cc, dd, aa, m[2],  23, 0xc4ac5665);

        // Round 4.
        StepI(ref aa, bb, cc, dd, m[0],   6, 0xf4292244);
        StepI(ref dd, aa, bb, cc, m[7],  10, 0x432aff97);
        StepI(ref cc, dd, aa, bb, m[14], 15, 0xab9423a7);
        StepI(ref bb, cc, dd, aa, m[5],  21, 0xfc93a039);
        StepI(ref aa, bb, cc, dd, m[12],  6, 0x655b59c3);
        StepI(ref dd, aa, bb, cc, m[3],  10, 0x8f0ccc92);
        StepI(ref cc, dd, aa, bb, m[10], 15, 0xffeff47d);
        StepI(ref bb, cc, dd, aa, m[1],  21, 0x85845dd1);
        StepI(ref aa, bb, cc, dd, m[8],   6, 0x6fa87e4f);
        StepI(ref dd, aa, bb, cc, m[15], 10, 0xfe2ce6e0);
        StepI(ref cc, dd, aa, bb, m[6],  15, 0xa3014314);
        StepI(ref bb, cc, dd, aa, m[13], 21, 0x4e0811a1);
        StepI(ref aa, bb, cc, dd, m[4],   6, 0xf7537e82);
        StepI(ref dd, aa, bb, cc, m[11], 10, 0xbd3af235);
        StepI(ref cc, dd, aa, bb, m[2],  15, 0x2ad7d2bb);
        StepI(ref bb, cc, dd, aa, m[9],  21, 0xeb86d391);

        a += aa;
        b += bb;
        c += cc;
        d += dd;
    }

    // Round-1 mix: F(x,y,z) = (x & y) | (~x & z).
    private static void Step(ref uint a, uint b, uint c, uint d, uint mi, int s, uint ki)
    {
        uint f = (b & c) | ((~b) & d);
        a = b + RotateLeft(a + f + mi + ki, s);
    }

    // Round-2 mix: G(x,y,z) = (x & z) | (y & ~z).
    private static void StepG(ref uint a, uint b, uint c, uint d, uint mi, int s, uint ki)
    {
        uint g = (b & d) | (c & (~d));
        a = b + RotateLeft(a + g + mi + ki, s);
    }

    // Round-3 mix: H(x,y,z) = x ^ y ^ z.
    private static void StepH(ref uint a, uint b, uint c, uint d, uint mi, int s, uint ki)
    {
        uint h = b ^ c ^ d;
        a = b + RotateLeft(a + h + mi + ki, s);
    }

    // Round-4 mix: I(x,y,z) = y ^ (x | ~z).
    private static void StepI(ref uint a, uint b, uint c, uint d, uint mi, int s, uint ki)
    {
        uint ii = c ^ (b | (~d));
        a = b + RotateLeft(a + ii + mi + ki, s);
    }

    private static uint RotateLeft(uint x, int n) => (x << n) | (x >> (32 - n));

    private static uint ReadUInt32LittleEndian(byte[] buf, int offset)
    {
        return (uint)buf[offset]
            | ((uint)buf[offset + 1] << 8)
            | ((uint)buf[offset + 2] << 16)
            | ((uint)buf[offset + 3] << 24);
    }

    private static void WriteUInt32LittleEndian(byte[] buf, int offset, uint value)
    {
        buf[offset]     = (byte)(value & 0xff);
        buf[offset + 1] = (byte)((value >> 8) & 0xff);
        buf[offset + 2] = (byte)((value >> 16) & 0xff);
        buf[offset + 3] = (byte)((value >> 24) & 0xff);
    }
}
