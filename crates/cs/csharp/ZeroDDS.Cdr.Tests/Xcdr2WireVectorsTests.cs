// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Wire-vector tests against `zerodds-xcdr2-bindings-conformance-1.0` §6 V-1..V-12.
// These classes are hand-written equivalents of what
// `idl-csharp` emits as `*TypeSupport`; they verify the
// `ZeroDDS.Cdr` helper library byte for byte.

using System;
using System.Collections.Generic;
using System.Text;
using Xunit;
using ZeroDDS.Cdr;

namespace ZeroDDS.Cdr.Tests;

// ============================================================================
// Sample classes (V-1..V-12)
// ============================================================================

public sealed record class Empty;

public sealed record class Point
{
    public int X { get; init; }
    public int Y { get; init; }
}

public sealed record class All
{
    public bool B { get; init; }
    public byte O { get; init; }
    public short S { get; init; }
    public ushort Us { get; init; }
    public int L { get; init; }
    public uint Ul { get; init; }
    public long Ll { get; init; }
    public ulong Ull { get; init; }
    public float F { get; init; }
    public double D { get; init; }
}

public sealed record class Greeting
{
    public string Text { get; init; } = string.Empty;
}

public sealed record class Bag
{
    public List<int> Ids { get; init; } = new();
}

public sealed record class Tags
{
    public List<string> TagList { get; init; } = new();
}

public sealed record class S7
{
    public int X { get; init; }
}

public sealed record class Sensor
{
    public int Id { get; init; }
    public double Value { get; init; }
}

public sealed record class V9Sample
{
    public int A { get; init; }
    public int B { get; init; }
}

public sealed record class M10
{
    public int A { get; init; }
    public string B { get; init; } = string.Empty;
}

public sealed record class O11
{
    public int? Maybe { get; init; }
}

// ============================================================================
// TypeSupport classes (hand-written, codegen-equivalent)
// ============================================================================

public sealed class EmptyTypeSupport : IDdsTopicType<Empty>
{
    public static readonly EmptyTypeSupport Instance = new();
    public string TypeName => "Empty";
    public bool IsKeyed => false;
    public ExtensibilityKind Extensibility => ExtensibilityKind.Final;
    public byte[] Encode(Empty sample) => Encode(sample, EndianMode.LittleEndian);
    public byte[] Encode(Empty sample, EndianMode endian) => new Xcdr2Writer(endian).ToArray();
    public Empty Decode(ReadOnlySpan<byte> bytes) => new();
    public byte[] KeyHash(Empty sample) => new byte[16];
}

public sealed class PointTypeSupport : IDdsTopicType<Point>
{
    public static readonly PointTypeSupport Instance = new();
    public string TypeName => "Point";
    public bool IsKeyed => false;
    public ExtensibilityKind Extensibility => ExtensibilityKind.Final;
    public byte[] Encode(Point sample) => Encode(sample, EndianMode.LittleEndian);
    public byte[] Encode(Point sample, EndianMode endian)
    {
        var w = new Xcdr2Writer(endian);
        w.WriteInt32(sample.X);
        w.WriteInt32(sample.Y);
        return w.ToArray();
    }
    public Point Decode(ReadOnlySpan<byte> bytes)
    {
        var r = new Xcdr2Reader(bytes, EndianMode.LittleEndian);
        return new Point { X = r.ReadInt32(), Y = r.ReadInt32() };
    }
    public byte[] KeyHash(Point sample) => new byte[16];
}

public sealed class AllTypeSupport : IDdsTopicType<All>
{
    public static readonly AllTypeSupport Instance = new();
    public string TypeName => "All";
    public bool IsKeyed => false;
    public ExtensibilityKind Extensibility => ExtensibilityKind.Final;
    public byte[] Encode(All sample) => Encode(sample, EndianMode.LittleEndian);
    public byte[] Encode(All sample, EndianMode endian)
    {
        var w = new Xcdr2Writer(endian);
        w.WriteBool(sample.B);
        w.WriteOctet(sample.O);
        w.WriteInt16(sample.S);
        w.WriteUInt16(sample.Us);
        w.WriteInt32(sample.L);
        w.WriteUInt32(sample.Ul);
        w.WriteInt64(sample.Ll);
        w.WriteUInt64(sample.Ull);
        w.WriteFloat32(sample.F);
        w.WriteFloat64(sample.D);
        return w.ToArray();
    }
    public All Decode(ReadOnlySpan<byte> bytes)
    {
        var r = new Xcdr2Reader(bytes, EndianMode.LittleEndian);
        return new All
        {
            B = r.ReadBool(),
            O = r.ReadOctet(),
            S = r.ReadInt16(),
            Us = r.ReadUInt16(),
            L = r.ReadInt32(),
            Ul = r.ReadUInt32(),
            Ll = r.ReadInt64(),
            Ull = r.ReadUInt64(),
            F = r.ReadFloat32(),
            D = r.ReadFloat64(),
        };
    }
    public byte[] KeyHash(All sample) => new byte[16];
}

public sealed class GreetingTypeSupport : IDdsTopicType<Greeting>
{
    public static readonly GreetingTypeSupport Instance = new();
    public string TypeName => "Greeting";
    public bool IsKeyed => false;
    public ExtensibilityKind Extensibility => ExtensibilityKind.Final;
    public byte[] Encode(Greeting sample) => Encode(sample, EndianMode.LittleEndian);
    public byte[] Encode(Greeting sample, EndianMode endian)
    {
        var w = new Xcdr2Writer(endian);
        w.WriteString(sample.Text);
        return w.ToArray();
    }
    public Greeting Decode(ReadOnlySpan<byte> bytes)
    {
        var r = new Xcdr2Reader(bytes, EndianMode.LittleEndian);
        return new Greeting { Text = r.ReadString() };
    }
    public byte[] KeyHash(Greeting sample) => new byte[16];
}

public sealed class BagTypeSupport : IDdsTopicType<Bag>
{
    public static readonly BagTypeSupport Instance = new();
    public string TypeName => "Bag";
    public bool IsKeyed => false;
    public ExtensibilityKind Extensibility => ExtensibilityKind.Final;
    public byte[] Encode(Bag sample) => Encode(sample, EndianMode.LittleEndian);
    public byte[] Encode(Bag sample, EndianMode endian)
    {
        var w = new Xcdr2Writer(endian);
        w.WriteSequenceLength(sample.Ids.Count);
        foreach (var i in sample.Ids) w.WriteInt32(i);
        return w.ToArray();
    }
    public Bag Decode(ReadOnlySpan<byte> bytes)
    {
        var r = new Xcdr2Reader(bytes, EndianMode.LittleEndian);
        int n = r.ReadSequenceLength();
        var list = new List<int>(n);
        for (int i = 0; i < n; i++) list.Add(r.ReadInt32());
        return new Bag { Ids = list };
    }
    public byte[] KeyHash(Bag sample) => new byte[16];
}

public sealed class TagsTypeSupport : IDdsTopicType<Tags>
{
    public static readonly TagsTypeSupport Instance = new();
    public string TypeName => "Tags";
    public bool IsKeyed => false;
    public ExtensibilityKind Extensibility => ExtensibilityKind.Final;
    public byte[] Encode(Tags sample) => Encode(sample, EndianMode.LittleEndian);
    public byte[] Encode(Tags sample, EndianMode endian)
    {
        // XCDR2 §7.4.3.5: seq<string> has non-primitive elements -> prepend a DHEADER
        // (uint32 = byte length of [count + elements]). Verified against CycloneDDS.
        var w = new Xcdr2Writer(endian);
        using (var dh = w.BeginAppendable())
        {
            w.WriteSequenceLength(sample.TagList.Count);
            foreach (var s in sample.TagList) w.WriteString(s);
        }
        return w.ToArray();
    }
    public Tags Decode(ReadOnlySpan<byte> bytes)
    {
        var r = new Xcdr2Reader(bytes, EndianMode.LittleEndian);
        var dh = r.BeginDHeader();
        int n = r.ReadSequenceLength();
        var list = new List<string>(n);
        for (int i = 0; i < n; i++) list.Add(r.ReadString());
        r.EndDHeader(dh);
        return new Tags { TagList = list };
    }
    public byte[] KeyHash(Tags sample) => new byte[16];
}

public sealed class S7TypeSupport : IDdsTopicType<S7>
{
    public static readonly S7TypeSupport Instance = new();
    public string TypeName => "Outer::Inner::S";
    public bool IsKeyed => false;
    public ExtensibilityKind Extensibility => ExtensibilityKind.Final;
    public byte[] Encode(S7 sample) => Encode(sample, EndianMode.LittleEndian);
    public byte[] Encode(S7 sample, EndianMode endian)
    {
        var w = new Xcdr2Writer(endian);
        w.WriteInt32(sample.X);
        return w.ToArray();
    }
    public S7 Decode(ReadOnlySpan<byte> bytes)
    {
        var r = new Xcdr2Reader(bytes, EndianMode.LittleEndian);
        return new S7 { X = r.ReadInt32() };
    }
    public byte[] KeyHash(S7 sample) => new byte[16];
}

public sealed class SensorTypeSupport : IDdsTopicType<Sensor>
{
    public static readonly SensorTypeSupport Instance = new();
    public string TypeName => "Sensor";
    public bool IsKeyed => true;
    public ExtensibilityKind Extensibility => ExtensibilityKind.Final;
    public byte[] Encode(Sensor sample) => Encode(sample, EndianMode.LittleEndian);
    public byte[] Encode(Sensor sample, EndianMode endian)
    {
        var w = new Xcdr2Writer(endian);
        w.WriteInt32(sample.Id);
        w.WriteFloat64(sample.Value);
        return w.ToArray();
    }
    public Sensor Decode(ReadOnlySpan<byte> bytes)
    {
        var r = new Xcdr2Reader(bytes, EndianMode.LittleEndian);
        return new Sensor { Id = r.ReadInt32(), Value = r.ReadFloat64() };
    }
    public byte[] KeyHash(Sensor sample)
    {
        // PlainCdr2BeKeyHolder per XTypes 1.3 §7.6.8.
        var kw = new Xcdr2Writer(EndianMode.BigEndian);
        kw.WriteInt32(sample.Id);
        var kb = kw.ToArray();
        if (kb.Length > 16) return Md5.Hash(kb);
        var h = new byte[16];
        Array.Copy(kb, 0, h, 0, kb.Length);
        return h;
    }
}

public sealed class V9TypeSupport : IDdsTopicType<V9Sample>
{
    public static readonly V9TypeSupport Instance = new();
    public string TypeName => "V";
    public bool IsKeyed => false;
    public ExtensibilityKind Extensibility => ExtensibilityKind.Appendable;
    public byte[] Encode(V9Sample sample) => Encode(sample, EndianMode.LittleEndian);
    public byte[] Encode(V9Sample sample, EndianMode endian)
    {
        var w = new Xcdr2Writer(endian);
        using (var __ = w.BeginAppendable())
        {
            w.WriteInt32(sample.A);
            w.WriteInt32(sample.B);
        }
        return w.ToArray();
    }
    public V9Sample Decode(ReadOnlySpan<byte> bytes)
    {
        var r = new Xcdr2Reader(bytes, EndianMode.LittleEndian);
        var s = r.BeginDHeader();
        var a = r.ReadInt32();
        var b = r.ReadInt32();
        r.EndDHeader(s);
        return new V9Sample { A = a, B = b };
    }
    public byte[] KeyHash(V9Sample sample) => new byte[16];
}

public sealed class M10TypeSupport : IDdsTopicType<M10>
{
    public static readonly M10TypeSupport Instance = new();
    public string TypeName => "M";
    public bool IsKeyed => false;
    public ExtensibilityKind Extensibility => ExtensibilityKind.Mutable;
    public byte[] Encode(M10 sample) => Encode(sample, EndianMode.LittleEndian);
    public byte[] Encode(M10 sample, EndianMode endian)
    {
        var w = new Xcdr2Writer(endian);
        using (var __ = w.BeginMutable())
        {
            // @id(1) long a -> LC=2 (4-byte fix)
            w.WriteEmHeader(1u, 2, false);
            w.WriteInt32(sample.A);
            // @id(2) string b -> LC=4 (variable, NEXTINT = body byte length).
            // LC=3 would mean a fixed 8-byte body with no NEXTINT and desyncs
            // any spec-compliant (Rust/Cyclone/FastDDS) reader.
            var sub = new Xcdr2Writer(endian);
            sub.WriteString(sample.B);
            var subBytes = sub.ToArray();
            w.WriteEmHeader(2u, 4, false);
            w.WriteUInt32((uint)subBytes.Length);
            w.WriteBytes(subBytes);
        }
        return w.ToArray();
    }
    public M10 Decode(ReadOnlySpan<byte> bytes)
    {
        var r = new Xcdr2Reader(bytes, EndianMode.LittleEndian);
        int a = 0;
        string b = string.Empty;
        var s = r.BeginDHeader();
        while (!r.DHeaderDone(s))
        {
            var (id, lc, _) = r.ReadEmHeader();
            if (lc >= 4) { _ = r.ReadUInt32(); }
            if (id == 1u) a = r.ReadInt32();
            else if (id == 2u) b = r.ReadString();
            else throw new XcdrException($"unknown id {id}");
        }
        r.EndDHeader(s);
        return new M10 { A = a, B = b };
    }
    public byte[] KeyHash(M10 sample) => new byte[16];
}

public sealed class O11TypeSupport : IDdsTopicType<O11>
{
    public static readonly O11TypeSupport Instance = new();
    public string TypeName => "O";
    public bool IsKeyed => false;
    public ExtensibilityKind Extensibility => ExtensibilityKind.Mutable;
    public byte[] Encode(O11 sample) => Encode(sample, EndianMode.LittleEndian);
    public byte[] Encode(O11 sample, EndianMode endian)
    {
        var w = new Xcdr2Writer(endian);
        using (var __ = w.BeginMutable())
        {
            if (sample.Maybe is not null)
            {
                w.WriteEmHeader(1u, 2, false);
                w.WriteInt32(sample.Maybe.Value);
            }
        }
        return w.ToArray();
    }
    public O11 Decode(ReadOnlySpan<byte> bytes)
    {
        var r = new Xcdr2Reader(bytes, EndianMode.LittleEndian);
        int? maybe = null;
        var s = r.BeginDHeader();
        while (!r.DHeaderDone(s))
        {
            var (id, lc, _) = r.ReadEmHeader();
            if (lc >= 4) { _ = r.ReadUInt32(); }
            if (id == 1u) maybe = r.ReadInt32();
            else throw new XcdrException($"unknown id {id}");
        }
        r.EndDHeader(s);
        return new O11 { Maybe = maybe };
    }
    public byte[] KeyHash(O11 sample) => new byte[16];
}

// ============================================================================
// Wire-Vector Tests
// ============================================================================

public class Xcdr2WireVectorsTests
{
    private static byte[] Hex(string spaceSeparated)
    {
        var parts = spaceSeparated.Split(' ', StringSplitOptions.RemoveEmptyEntries);
        var b = new byte[parts.Length];
        for (int i = 0; i < parts.Length; i++)
        {
            b[i] = Convert.ToByte(parts[i], 16);
        }
        return b;
    }

    private static string ToHex(byte[] bytes)
    {
        var sb = new StringBuilder();
        for (int i = 0; i < bytes.Length; i++)
        {
            if (i > 0) sb.Append(' ');
            sb.Append(bytes[i].ToString("X2"));
        }
        return sb.ToString();
    }

    [Fact]
    public void V1_EmptyFinal_EncodesToZeroBytes()
    {
        var bytes = EmptyTypeSupport.Instance.Encode(new Empty());
        Assert.Empty(bytes);
        Assert.Equal("Empty", EmptyTypeSupport.Instance.TypeName);
        Assert.Equal(ExtensibilityKind.Final, EmptyTypeSupport.Instance.Extensibility);
        // Roundtrip:
        var decoded = EmptyTypeSupport.Instance.Decode(bytes);
        Assert.NotNull(decoded);
    }

    [Fact]
    public void V2_PlainPrimitivesFinal_MatchesWire()
    {
        var sample = new Point { X = 1, Y = -2 };
        var expected = Hex("01 00 00 00 FE FF FF FF");
        var bytes = PointTypeSupport.Instance.Encode(sample);
        Assert.Equal(ToHex(expected), ToHex(bytes));
        // Roundtrip:
        Assert.Equal(sample, PointTypeSupport.Instance.Decode(bytes));
        Assert.Equal("Point", PointTypeSupport.Instance.TypeName);
    }

    [Fact]
    public void V3_MixedPrimitivesFinal_MatchesWire()
    {
        // Note on a spec discrepancy: §6 V-3 lists an extra
        // 1-byte pad at offset 2 plus a total length of 40 bytes.
        // Per OMG XTypes 1.3 §7.4.1.5 those values are not reachable
        // (b+o are at offsets 0..1, then short follows at offset 2,
        // already 2-aligned). Instead we verify the correct
        // XTypes 1.3 form via roundtrip + spot checks at the well-
        // defined positions (long/long long/double).
        var sample = new All
        {
            B = true,
            O = 0xAB,
            S = -12345,
            Us = 54321,
            L = -1234567,
            Ul = 2345678,
            Ll = -987654321,
            Ull = 123456789,
            F = 2.5f,
            D = 3.14159,
        };
        var bytes = AllTypeSupport.Instance.Encode(sample);

        // Spot checks: every primitive field must sit exactly where
        // XTypes 1.3 §7.4.1.5 prescribes (origin = 0).
        // Layout: b(0) o(1) s(2..3) us(4..5) l(8..11) ul(12..15)
        //         ll(16..23) ull(24..31) f(32..35) d(40..47).
        Assert.Equal((byte)1, bytes[0]);
        Assert.Equal((byte)0xAB, bytes[1]);
        Assert.Equal((short)-12345, BitConverter.ToInt16(bytes, 2));
        Assert.Equal((ushort)54321, BitConverter.ToUInt16(bytes, 4));
        Assert.Equal(-1234567, BitConverter.ToInt32(bytes, 8));
        Assert.Equal((uint)2345678, BitConverter.ToUInt32(bytes, 12));
        Assert.Equal(-987654321L, BitConverter.ToInt64(bytes, 16));
        Assert.Equal(123456789UL, BitConverter.ToUInt64(bytes, 24));
        Assert.Equal(2.5f, BitConverter.ToSingle(bytes, 32));
        Assert.Equal(3.14159, BitConverter.ToDouble(bytes, 40));

        Assert.Equal(sample, AllTypeSupport.Instance.Decode(bytes));
        Assert.Equal("All", AllTypeSupport.Instance.TypeName);
    }

    [Fact]
    public void V4_StringFinal_MatchesWire()
    {
        var sample = new Greeting { Text = "hello" };
        var expected = Hex("06 00 00 00 68 65 6C 6C 6F 00");
        var bytes = GreetingTypeSupport.Instance.Encode(sample);
        Assert.Equal(ToHex(expected), ToHex(bytes));
        Assert.Equal(sample, GreetingTypeSupport.Instance.Decode(bytes));
        Assert.Equal("Greeting", GreetingTypeSupport.Instance.TypeName);
    }

    [Fact]
    public void V5_SeqInt32Final_MatchesWire()
    {
        var sample = new Bag { Ids = new List<int> { 1, 2, 3 } };
        var expected = Hex("03 00 00 00 01 00 00 00 02 00 00 00 03 00 00 00");
        var bytes = BagTypeSupport.Instance.Encode(sample);
        Assert.Equal(ToHex(expected), ToHex(bytes));
        var decoded = BagTypeSupport.Instance.Decode(bytes);
        Assert.Equal(sample.Ids, decoded.Ids);
        Assert.Equal("Bag", BagTypeSupport.Instance.TypeName);
    }

    [Fact]
    public void V6_SeqStringFinal_MatchesWire()
    {
        var sample = new Tags { TagList = new List<string> { "a", "bc" } };
        // XCDR2 §7.4.3.5: DHEADER (= 19) before seq<string>.
        var expected = Hex(
            "13 00 00 00 " +
            "02 00 00 00 " +
            "02 00 00 00 61 00 " +
            "00 00 " +
            "03 00 00 00 62 63 00");
        var bytes = TagsTypeSupport.Instance.Encode(sample);
        Assert.Equal(ToHex(expected), ToHex(bytes));
        var decoded = TagsTypeSupport.Instance.Decode(bytes);
        Assert.Equal(sample.TagList, decoded.TagList);
        Assert.Equal("Tags", TagsTypeSupport.Instance.TypeName);
    }

    [Fact]
    public void V7_NestedModulesFinal_MatchesWire()
    {
        var sample = new S7 { X = 1234 };
        var expected = Hex("D2 04 00 00");
        var bytes = S7TypeSupport.Instance.Encode(sample);
        Assert.Equal(ToHex(expected), ToHex(bytes));
        Assert.Equal(sample, S7TypeSupport.Instance.Decode(bytes));
        Assert.Equal("Outer::Inner::S", S7TypeSupport.Instance.TypeName);
    }

    [Fact]
    public void V8_KeyedFinal_MatchesWire()
    {
        var sample = new Sensor { Id = 42, Value = 3.14 };
        var expected = Hex("2A 00 00 00 00 00 00 00 1F 85 EB 51 B8 1E 09 40");
        var bytes = SensorTypeSupport.Instance.Encode(sample);
        Assert.Equal(ToHex(expected), ToHex(bytes));
        Assert.Equal(sample, SensorTypeSupport.Instance.Decode(bytes));
        Assert.True(SensorTypeSupport.Instance.IsKeyed);
        // Key hash: PlainCdr2BeKeyHolder is 4 bytes (`00 00 00 2A`) -> since
        // <= 16 bytes, it is zero-padded per XTypes 1.3 §7.6.8 instead of MD5.
        var keyHash = SensorTypeSupport.Instance.KeyHash(sample);
        Assert.Equal(16, keyHash.Length);
        var expectedHash = Hex("00 00 00 2A 00 00 00 00 00 00 00 00 00 00 00 00");
        Assert.Equal(ToHex(expectedHash), ToHex(keyHash));
    }

    [Fact]
    public void V9_Appendable_MatchesWire()
    {
        var sample = new V9Sample { A = 1, B = 2 };
        var expected = Hex("08 00 00 00 01 00 00 00 02 00 00 00");
        var bytes = V9TypeSupport.Instance.Encode(sample);
        Assert.Equal(ToHex(expected), ToHex(bytes));
        Assert.Equal(sample, V9TypeSupport.Instance.Decode(bytes));
        Assert.Equal(ExtensibilityKind.Appendable, V9TypeSupport.Instance.Extensibility);
    }

    [Fact]
    public void V10_Mutable_MatchesWire()
    {
        // Note on a spec discrepancy: §6 V-10 lists DHEADER=20, but the
        // body is actually 23 bytes (4 EMHEADER + 4 value + 4 EMHEADER
        // + 4 NEXTINT + 7 string). XTypes 1.3 §7.4.4.4 defines DHEADER as
        // the body length excluding the 4-byte header itself -> we set DHEADER=23.
        var sample = new M10 { A = 42, B = "hi" };
        var bytes = M10TypeSupport.Instance.Encode(sample);

        // [0..4]   = DHEADER  (LE uint32 = body-len = 23)
        // [4..8]   = EMHEADER id=1 LC=2 (ambient-LE uint32 per XTypes §7.4.3.4.5)
        // [8..12]  = a-value LE
        // [12..16] = EMHEADER id=2 LC=4 (ambient-LE; variable member + NEXTINT)
        // [16..20] = NEXTINT (LE uint32 = string-byte-len)
        // [20..27] = string-prefix(4) + "hi\0"(3) = 7 bytes
        Assert.Equal((uint)23, BitConverter.ToUInt32(bytes, 0));
        var em1 = BitConverter.ToUInt32(bytes, 4);
        Assert.Equal((uint)1, em1 & 0x0FFFFFFFu);
        Assert.Equal(2, (int)((em1 >> 28) & 0x7));
        Assert.Equal(42, BitConverter.ToInt32(bytes, 8));
        var em2 = BitConverter.ToUInt32(bytes, 12);
        Assert.Equal((uint)2, em2 & 0x0FFFFFFFu);
        Assert.Equal(4, (int)((em2 >> 28) & 0x7));
        Assert.Equal((uint)7, BitConverter.ToUInt32(bytes, 16));
        Assert.Equal((uint)3, BitConverter.ToUInt32(bytes, 20)); // string length-incl-NUL
        Assert.Equal((byte)'h', bytes[24]);
        Assert.Equal((byte)'i', bytes[25]);
        Assert.Equal((byte)0, bytes[26]);
        // Roundtrip:
        Assert.Equal(sample, M10TypeSupport.Instance.Decode(bytes));
        Assert.Equal(ExtensibilityKind.Mutable, M10TypeSupport.Instance.Extensibility);
    }

    [Fact]
    public void V11_OptionalSome_MatchesWire()
    {
        // Note on a spec discrepancy: §6 V-11 lists DHEADER=12, but the
        // body is actually 8 bytes (EMHEADER 4 + value 4). XTypes 1.3
        // §7.4.4.4 defines DHEADER as the body length -> 8.
        var sample = new O11 { Maybe = 7 };
        // EMHEADER ambient-LE per XTypes §7.4.3.4.5: u32=0x20000001 -> bytes 01 00 00 20.
        var expected = Hex("08 00 00 00 01 00 00 20 07 00 00 00");
        var bytes = O11TypeSupport.Instance.Encode(sample);
        Assert.Equal(ToHex(expected), ToHex(bytes));
        Assert.Equal(sample, O11TypeSupport.Instance.Decode(bytes));
    }

    [Fact]
    public void V11_OptionalNone_MatchesWire()
    {
        var sample = new O11 { Maybe = null };
        var expected = Hex("00 00 00 00");
        var bytes = O11TypeSupport.Instance.Encode(sample);
        Assert.Equal(ToHex(expected), ToHex(bytes));
        Assert.Equal(sample, O11TypeSupport.Instance.Decode(bytes));
    }

    [Fact]
    public void V12_MutableSentinel_NoExplicitSentinelEmitted()
    {
        // §6 V-12: XCDR2 emits NO explicit PID_LIST_END sentinel.
        // Instead the DHEADER size bounds the read.
        var bytes = M10TypeSupport.Instance.Encode(new M10 { A = 1, B = "x" });
        // Check: the tail contains no PID_LIST_END (0x3F02) at the end.
        // The last bytes are the bytes of the string "x\0", not a 4-byte sentinel.
        Assert.False(bytes.Length >= 4 &&
                     bytes[^4] == 0x3F &&
                     bytes[^3] == 0x02 &&
                     bytes[^2] == 0x00 &&
                     bytes[^1] == 0x00,
                     "XCDR2 mutable stream MUST NOT emit explicit sentinel");
    }
}
