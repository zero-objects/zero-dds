// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// TopicTypedSmoke.cs — reference smoke for zerodds-xcdr2-csharp-1.0 §10.
//
// Shows the encode/decode roundtrip of a hand-implemented
// `IDdsTopicType<Point>` (equivalent to what `zerodds-idlc --lang 'c#'`
// emits per IDL `struct` — see `tools/idlc/src/csharp_support.rs`), calling
// straight into the `ZeroDds.Wire` runtime (`runtime/csharp`), the one
// XCDR2 codec this binding has. No live DDS — only the XCDR2 wire layer.
//
// Run:
//   cd crates/cs/Examples
//   dotnet run --project TopicTypedSmoke.csproj

using System;
using ZeroDDS.Cdr;
using ZeroDds.Wire;
using Omg.Types; // ExtensibilityKind

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

    private static Endianness Order(EndianMode endian) => endian switch
    {
        EndianMode.LittleEndian => Endianness.Little,
        EndianMode.BigEndian => Endianness.Big,
        _ => throw new ArgumentOutOfRangeException(nameof(endian)),
    };

    public byte[] Encode(Point sample) => Encode(sample, EndianMode.LittleEndian);

    public byte[] Encode(Point sample, EndianMode endian)
    {
        var w = new Writer(Order(endian), 4, PlCdr1Framing.Defaults(), false);
        w.WriteI32(sample.X);
        w.WriteI32(sample.Y);
        return w.Finish();
    }

    public Point Decode(ReadOnlySpan<byte> bytes)
    {
        var r = new Reader(bytes.ToArray(), Endianness.Little, 4, PlCdr1Framing.Defaults());
        return new Point { X = r.ReadI32(), Y = r.ReadI32() };
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
