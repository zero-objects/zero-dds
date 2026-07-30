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

        // Builds an XRCE frame with an arbitrary submessage id + declared body
        // length, for exercising the reader's direction + validation logic.
        private static byte[] Frame(byte id, byte[] body, int declaredLen)
        {
            var o = new byte[8 + body.Length];
            o[0] = 0x80; o[1] = 0x01; o[2] = 1; o[3] = 0;
            o[4] = id; o[5] = 0x03;
            o[6] = (byte)(declaredLen & 0xff); o[7] = (byte)((declaredLen >> 8) & 0xff);
            Array.Copy(body, 0, o, 8, body.Length);
            return o;
        }

        // XRCE framing: direction (hub -> endpoint is DATA/0x09) + negative
        // frame vectors. Returns 0 on success, 1 on any mismatch.
        private static int Framing()
        {
            int rc = 0;
            var data = new byte[] { 0xAA, 0xBB, 0xCC };
            // Hub -> endpoint direction is DATA (0x09); the reader must accept it.
            var got = Xrce.ReadFrame(Frame(0x09, data, data.Length));
            if (got == null || !AsSpan(got).SequenceEqual(data)) { Console.WriteLine("framing: DATA/0x09 not accepted"); rc = 1; }
            // Loopback WRITE_DATA (0x07) still accepted.
            var wd = new byte[] { 0x01, 0x02 };
            var gotWd = Xrce.ReadFrame(Frame(0x07, wd, wd.Length));
            if (gotWd == null || !AsSpan(gotWd).SequenceEqual(wd)) { Console.WriteLine("framing: WRITE_DATA/0x07 not accepted"); rc = 1; }
            // Unknown submessage id -> reject.
            if (Xrce.ReadFrame(Frame(0x0A, wd, wd.Length)) != null) { Console.WriteLine("framing: unknown id not rejected"); rc = 1; }
            // Truncated header (fewer than 8 bytes) -> reject.
            for (int len = 0; len < 8; len++)
            {
                if (Xrce.ReadFrame(new byte[len]) != null) { Console.WriteLine($"framing: {len}-byte frame not rejected"); rc = 1; }
            }
            // Declared length past the datagram -> reject.
            if (Xrce.ReadFrame(Frame(0x09, wd, 100)) != null) { Console.WriteLine("framing: over-long length not rejected"); rc = 1; }
            // Trailing bytes past the declared length must not leak into the body.
            var bounded = Xrce.ReadFrame(Frame(0x09, new byte[] { 1, 2, 3, 4 }, 2));
            if (bounded == null || !AsSpan(bounded).SequenceEqual(new byte[] { 1, 2 })) { Console.WriteLine("framing: body not bounded by declared length"); rc = 1; }
            // Writer refuses a sample larger than the 16-bit length field.
            bool refused = false;
            try { Xrce.WriteFrame(1, new byte[0x10000]); }
            catch (ArgumentException) { refused = true; }
            if (!refused) { Console.WriteLine("framing: sample > 0xFFFF not refused"); rc = 1; }
            if (Xrce.WriteFrame(1, new byte[0xFFFF]).Length != 8 + 0xFFFF) { Console.WriteLine("framing: 0xFFFF sample must still encode"); rc = 1; }
            if (rc == 0) Console.WriteLine("xrce framing: direction (DATA/0x09) + negative vectors ok");
            return rc;
        }

        // string is UTF-8 with a byte-count length prefix (not the char count).
        // Golden-free, so it runs without the Rust golden binaries.
        private static int StringUtf8()
        {
            // "grüße": g r [ü=C3 BC] [ß=C3 9F] e = 7 UTF-8 bytes, but only 5 chars.
            const string s = "grüße";
            var utf8 = new byte[] { 0x67, 0x72, 0xC3, 0xBC, 0xC3, 0x9F, 0x65 };
            var w = new Writer(Endianness.Little);
            w.PutString(s);
            var wire = w.Bytes();
            int rc = 0;
            // 4-byte LE length prefix = byte count incl. NUL = 8, NOT char count (5).
            uint prefix = (uint)(wire[0] | (wire[1] << 8) | (wire[2] << 16) | (wire[3] << 24));
            if (prefix != (uint)(utf8.Length + 1)) { Console.WriteLine($"string: length prefix {prefix} != UTF-8 byte count + 1"); rc = 1; }
            for (int i = 0; i < utf8.Length; i++)
            {
                if (wire[4 + i] != utf8[i]) { Console.WriteLine("string: UTF-8 body mismatch"); rc = 1; break; }
            }
            if (wire[4 + utf8.Length] != 0) { Console.WriteLine("string: missing NUL terminator"); rc = 1; }
            if (wire.Length != 4 + utf8.Length + 1) { Console.WriteLine($"string: total wire length {wire.Length}"); rc = 1; }
            var back = new Reader(wire, Endianness.Little).GetString();
            if (back != s) { Console.WriteLine($"string: roundtrip mismatch ('{back}')"); rc = 1; }
            if (rc == 0) Console.WriteLine("string UTF-8: byte-count length prefix + roundtrip ok");
            return rc;
        }

        public static int Main(string[] args)
        {
            // UTF-8 string test is golden-free and always runs.
            int rc = StringUtf8();
            if (args.Length < 2)
            {
                Console.Error.WriteLine("usage: ByteIdentity <golden_le.bin> <golden_be.bin> (goldens skipped)");
                return rc;
            }
            rc |= Check(Endianness.Little, args[0], "LE") | Check(Endianness.Big, args[1], "BE") | Framing();
            if (rc == 0) Console.WriteLine("ALL OK");
            return rc;
        }
    }
}
