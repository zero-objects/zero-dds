// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors

using System;
using System.Linq;
using Xunit;
using ZeroDDS.Cdr;
using Omg.Types; // ExtensibilityKind

namespace ZeroDDS.Cdr.SourceGenerators.Tests;

[DdsType]
public partial class Point
{
    public int X { get; set; }
    public int Y { get; set; }
}

[DdsType]
public partial class Sensor
{
    [DdsKey]
    public int Id { get; set; }
    public double Value { get; set; }
}

[DdsType(TypeName = "namespaced::Custom")]
public partial class Custom
{
    public uint N { get; set; }
}

public class SourceGenSmoke
{
    [Fact]
    public void GeneratorEmitsTypeSupportClass()
    {
        // PointTypeSupport must exist after source-gen.
        var ts = PointTypeSupport.Instance;
        Assert.Equal("Point", ts.TypeName);
        Assert.False(ts.IsKeyed);
        Assert.Equal(ExtensibilityKind.Final, ts.Extensibility);
    }

    [Fact]
    public void PointEncodesV2WireBytes()
    {
        var p = new Point { X = 1, Y = -2 };
        var bytes = PointTypeSupport.Instance.Encode(p);
        // V-2 wire: 01 00 00 00 fe ff ff ff
        var expected = new byte[] { 0x01, 0, 0, 0, 0xfe, 0xff, 0xff, 0xff };
        Assert.Equal(expected, bytes);
    }

    [Fact]
    public void PointRoundtrip()
    {
        var p = new Point { X = 42, Y = -7 };
        var bytes = PointTypeSupport.Instance.Encode(p);
        var p2 = PointTypeSupport.Instance.Decode(bytes);
        Assert.Equal(p.X, p2.X);
        Assert.Equal(p.Y, p2.Y);
    }

    [Fact]
    public void SensorMarkedKeyed()
    {
        Assert.True(SensorTypeSupport.Instance.IsKeyed);
    }

    [Fact]
    public void SensorKeyHashZeroPadForSmallHolder()
    {
        var s = new Sensor { Id = 42, Value = 3.14 };
        var hash = SensorTypeSupport.Instance.KeyHash(s);
        // BE-encoded id=42 -> 00 00 00 2A; zero-pad to 16 bytes.
        var expected = new byte[16];
        expected[3] = 0x2A;
        Assert.Equal(expected, hash);
    }

    [Fact]
    public void CustomTypeNameOverride()
    {
        Assert.Equal("namespaced::Custom", CustomTypeSupport.Instance.TypeName);
    }
}
