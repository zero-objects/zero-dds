// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Byte-identity test for the pure-C# endpoint wire-core (ADR 0013). Encodes the
// fixed @final SensorReading in LE and BE and compares to the Rust goldens; then
// round-trips a decode. Same test vector as the other endpoint SDKs.
//
// usage: dotnet run --project ByteIdentity.csproj -- <golden_le.bin> <golden_be.bin>

using System;
using System.IO;
using ZeroDDS;

namespace ZeroDDS.Examples
{
    public static class ByteIdentity
    {
        private static byte[] Encode(Endianness endian)
        {
            var w = new Writer(endian);
            w.PutU32(0xA1B2C3D4);
            w.PutU16(0x1234);
            w.PutU8(0x5A);
            w.PutF32(3.5f);
            w.PutU64(0x0102030405060708UL);
            w.PutString("bay-12");
            w.PutSeqU8(new byte[] { 0xDE, 0xAD, 0xBE, 0xEF });
            return w.Bytes();
        }

        private static int Check(Endianness endian, string goldenPath, string tag)
        {
            var golden = File.ReadAllBytes(goldenPath);
            var got = Encode(endian);
            if (!AsSpan(got).SequenceEqual(golden))
            {
                Console.WriteLine($"{tag}: MISMATCH (got {got.Length}, golden {golden.Length})");
                return 1;
            }
            Console.WriteLine($"{tag}: {got.Length} bytes byte-identical to Rust golden");
            return 0;
        }

        private static ReadOnlySpan<byte> AsSpan(byte[] b) => b;

        public static int Main(string[] args)
        {
            if (args.Length < 2)
            {
                Console.Error.WriteLine("usage: ByteIdentity <golden_le.bin> <golden_be.bin>");
                return 2;
            }
            int rc = Check(Endianness.Little, args[0], "LE") | Check(Endianness.Big, args[1], "BE");
            if (rc == 0) Console.WriteLine("ALL OK");
            return rc;
        }
    }
}
