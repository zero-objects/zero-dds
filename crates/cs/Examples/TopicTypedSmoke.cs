// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// TopicTypedSmoke.cs — Referenz-Smoke fuer zerodds-xcdr2-csharp-1.0 §10.
//
// Zeigt encode/decode-Roundtrip eines hand-implementierten
// `IDdsTopicType<Point>` (entspricht dem was idl-csharp pro IDL-`struct`
// emittieren wuerde). Kein Live-DDS — nur das XCDR2-Wire-Layer.
//
// Run:
//   cd crates/cs/Examples
//   dotnet run --project TopicTypedSmoke.csproj

using System;
using ZeroDDS.Cdr;

namespace ZeroDDS.Examples;

public sealed class Point : IEquatable<Point>
{
    public int X { get; init; }
    public int Y { get; init; }

    public bool Equals(Point? other) => other is not null && X == other.X && Y == other.Y;
    public override bool Equals(object? obj) => obj is Point p && Equals(p);
    public override int GetHashCode() => HashCode.Combine(X, Y);
    public override string ToString() => $"Point(x={X}, y={Y})";
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

public static class Program
{
    public static int Main()
    {
        var p = new Point { X = 42, Y = -7 };
        var ts = PointTypeSupport.Instance;

        Console.WriteLine($"type_name = {ts.TypeName}");
        Console.WriteLine($"extensibility = {ts.Extensibility}");
        Console.WriteLine($"sample = {p}");

        var bytes = ts.Encode(p);
        Console.Write("wire = ");
        foreach (var b in bytes) Console.Write($"{b:X2} ");
        Console.WriteLine();

        var roundtripped = ts.Decode(bytes);
        Console.WriteLine($"roundtrip = {roundtripped}");

        if (!p.Equals(roundtripped))
        {
            Console.Error.WriteLine("FAIL: roundtrip mismatch");
            return 1;
        }

        Console.WriteLine("OK");
        return 0;
    }
}
