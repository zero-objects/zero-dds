// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Tests fuer Padding/Alignment-Verhalten von Xcdr2Writer/Xcdr2Reader
// per XTypes 1.3 §7.4.1.5.

using System;
using Xunit;
using ZeroDDS.Cdr;

namespace ZeroDDS.Cdr.Tests;

public class AlignmentTests
{
    [Fact]
    public void Align4_AfterByte_PadsThreeBytes()
    {
        var w = new Xcdr2Writer(EndianMode.LittleEndian);
        w.WriteOctet(0xAB);
        w.WriteInt32(1);
        var bytes = w.ToArray();
        Assert.Equal(8, bytes.Length);
        Assert.Equal(0xAB, bytes[0]);
        Assert.Equal(0, bytes[1]);
        Assert.Equal(0, bytes[2]);
        Assert.Equal(0, bytes[3]);
        Assert.Equal(1, BitConverter.ToInt32(bytes, 4));
    }

    [Fact]
    public void Align8_ForDouble_AfterFourBytes_PadsFour()
    {
        var w = new Xcdr2Writer(EndianMode.LittleEndian);
        w.WriteInt32(7);
        w.WriteFloat64(1.0);
        var bytes = w.ToArray();
        Assert.Equal(16, bytes.Length);
        Assert.Equal(7, BitConverter.ToInt32(bytes, 0));
        // Bytes 4..7 = padding
        Assert.Equal(1.0, BitConverter.ToDouble(bytes, 8));
    }

    [Fact]
    public void Reader_AlignSkipsPadding()
    {
        var w = new Xcdr2Writer();
        w.WriteOctet(1);
        w.WriteInt32(42);
        var bytes = w.ToArray();
        var r = new Xcdr2Reader(bytes);
        Assert.Equal((byte)1, r.ReadOctet());
        Assert.Equal(42, r.ReadInt32());
    }

    [Fact]
    public void DHeader_ResetsAlignmentOrigin()
    {
        // Schreibe 1 Byte, dann DHEADER, dann long. Long muss am Origin
        // direkt hinter dem DHEADER liegen, nicht relativ zum buffer-start.
        var w = new Xcdr2Writer();
        w.WriteOctet(0xAB);
        using (var s = w.BeginDHeader())
        {
            w.WriteInt32(7);
        }
        var bytes = w.ToArray();
        // Layout:
        //   0:    0xAB
        //   1..3: pad to align(4) for DHEADER
        //   4..7: DHEADER (=4)
        //   8..11: long=7
        Assert.Equal(0xAB, bytes[0]);
        Assert.Equal((uint)4, BitConverter.ToUInt32(bytes, 4));
        Assert.Equal(7, BitConverter.ToInt32(bytes, 8));
    }

    [Fact]
    public void ReadPastEnd_Throws()
    {
        bool threw = false;
        try
        {
            var r = new Xcdr2Reader(new byte[] { 1, 2 });
            _ = r.ReadInt32();
        }
        catch (XcdrException)
        {
            threw = true;
        }
        Assert.True(threw, "expected XcdrException for read-past-end");
    }

    [Fact]
    public void EndianMode_BigEndianRoundTrips()
    {
        var w = new Xcdr2Writer(EndianMode.BigEndian);
        w.WriteInt32(0x01020304);
        var bytes = w.ToArray();
        Assert.Equal(new byte[] { 0x01, 0x02, 0x03, 0x04 }, bytes);
        var r = new Xcdr2Reader(bytes, EndianMode.BigEndian);
        Assert.Equal(0x01020304, r.ReadInt32());
    }

    [Fact]
    public void String_RoundTrip_PreservesContent()
    {
        var w = new Xcdr2Writer();
        w.WriteString("hello world");
        var r = new Xcdr2Reader(w.ToArray());
        Assert.Equal("hello world", r.ReadString());
    }

    [Fact]
    public void Sequence_RoundTrip()
    {
        var w = new Xcdr2Writer();
        w.WriteSequenceLength(3);
        w.WriteInt32(10);
        w.WriteInt32(20);
        w.WriteInt32(30);
        var r = new Xcdr2Reader(w.ToArray());
        Assert.Equal(3, r.ReadSequenceLength());
        Assert.Equal(10, r.ReadInt32());
        Assert.Equal(20, r.ReadInt32());
        Assert.Equal(30, r.ReadInt32());
    }

    [Fact]
    public void EmHeader_RoundTrip()
    {
        var w = new Xcdr2Writer();
        w.WriteEmHeader(42u, 2, mustUnderstand: true);
        var r = new Xcdr2Reader(w.ToArray());
        var (id, lc, mu) = r.ReadEmHeader();
        Assert.Equal(42u, id);
        Assert.Equal(2, lc);
        Assert.True(mu);
    }
}
