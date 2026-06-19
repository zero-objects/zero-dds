// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// XCDR2 Decoder.
// Spec: OMG XTypes 1.3 §7.4 + zerodds-xcdr2-csharp-1.0 §6/§7
// + zerodds-xcdr2-bindings-conformance-1.0 §6 (V-1..V-12).

using System;
using System.Buffers.Binary;
using System.Text;

namespace ZeroDDS.Cdr;

/// <summary>
/// XCDR2 decoder. Inverse of <see cref="Xcdr2Writer"/>.
///
/// Per XTypes 1.3 §7.4.1.5 the alignment rule is relative to the current
/// origin (initially 0; reset at the DHEADER body).
/// </summary>
public ref struct Xcdr2Reader
{
    private readonly ReadOnlySpan<byte> _buf;
    private readonly EndianMode _endian;
    private int _pos;
    private int _origin;

    /// <summary>Constructor with default endianness (little-endian).</summary>
    public Xcdr2Reader(ReadOnlySpan<byte> bytes) : this(bytes, EndianMode.LittleEndian) { }

    /// <summary>Constructor with explicit endianness.</summary>
    public Xcdr2Reader(ReadOnlySpan<byte> bytes, EndianMode endian)
    {
        _buf = bytes;
        _endian = endian;
        _pos = 0;
        _origin = 0;
    }

    /// <summary>Active endianness.</summary>
    public EndianMode Endian => _endian;

    /// <summary>Current read byte position.</summary>
    public int Position => _pos;

    /// <summary>Number of bytes still available from `Position`.</summary>
    public int Remaining => _buf.Length - _pos;

    // ---------------------------------------------------------------------
    // Alignment + raw bytes
    // ---------------------------------------------------------------------

    /// <summary>Skips padding to the N-byte boundary relative to the origin.</summary>
    public void Align(int alignment)
    {
        if (alignment != 1 && alignment != 2 && alignment != 4 && alignment != 8)
        {
            throw new ArgumentOutOfRangeException(nameof(alignment),
                "alignment must be one of {1,2,4,8}");
        }
        int offset = _pos - _origin;
        int pad = (alignment - (offset % alignment)) % alignment;
        if (_pos + pad > _buf.Length)
        {
            throw new XcdrException(
                $"alignment skip past end-of-buffer (pos={_pos}, pad={pad}, len={_buf.Length})");
        }
        _pos += pad;
    }

    /// <summary>Returns a slice of the next N bytes without endianness conversion.</summary>
    public ReadOnlySpan<byte> ReadBytes(int count)
    {
        if (count < 0) throw new ArgumentOutOfRangeException(nameof(count));
        if (_pos + count > _buf.Length)
        {
            throw new XcdrException(
                $"read past end-of-buffer (pos={_pos}, count={count}, len={_buf.Length})");
        }
        var slice = _buf.Slice(_pos, count);
        _pos += count;
        return slice;
    }

    // ---------------------------------------------------------------------
    // Primitives
    // ---------------------------------------------------------------------

    /// <summary>IDL `boolean` -> 1 byte.</summary>
    public bool ReadBool()
    {
        var b = ReadByte();
        if (b != 0 && b != 1)
        {
            throw new XcdrException($"invalid boolean encoding 0x{b:X2}");
        }
        return b != 0;
    }

    /// <summary>IDL `octet` / `char` -> 1 byte.</summary>
    public byte ReadByte()
    {
        if (_pos >= _buf.Length)
        {
            throw new XcdrException("read past end-of-buffer");
        }
        return _buf[_pos++];
    }

    /// <summary>IDL `octet` (alias of ReadByte for symmetry with the writer).</summary>
    public byte ReadOctet() => ReadByte();

    /// <summary>IDL `short` -> 2 bytes, Align(2).</summary>
    public short ReadInt16()
    {
        Align(2);
        var s = ReadBytes(2);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadInt16LittleEndian(s)
            : BinaryPrimitives.ReadInt16BigEndian(s);
    }

    /// <summary>IDL `unsigned short` -> 2 bytes, Align(2).</summary>
    public ushort ReadUInt16()
    {
        Align(2);
        var s = ReadBytes(2);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadUInt16LittleEndian(s)
            : BinaryPrimitives.ReadUInt16BigEndian(s);
    }

    /// <summary>IDL `long` -> 4 bytes, Align(4).</summary>
    public int ReadInt32()
    {
        Align(4);
        var s = ReadBytes(4);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadInt32LittleEndian(s)
            : BinaryPrimitives.ReadInt32BigEndian(s);
    }

    /// <summary>IDL `unsigned long` -> 4 bytes, Align(4).</summary>
    public uint ReadUInt32()
    {
        Align(4);
        var s = ReadBytes(4);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadUInt32LittleEndian(s)
            : BinaryPrimitives.ReadUInt32BigEndian(s);
    }

    /// <summary>IDL `long long` -> 8 bytes, Align(8).</summary>
    public long ReadInt64()
    {
        Align(8);
        var s = ReadBytes(8);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadInt64LittleEndian(s)
            : BinaryPrimitives.ReadInt64BigEndian(s);
    }

    /// <summary>IDL `unsigned long long` -> 8 bytes, Align(8).</summary>
    public ulong ReadUInt64()
    {
        Align(8);
        var s = ReadBytes(8);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadUInt64LittleEndian(s)
            : BinaryPrimitives.ReadUInt64BigEndian(s);
    }

    /// <summary>IDL `float` -> 4 bytes IEEE-754, Align(4).</summary>
    public float ReadFloat32()
    {
        Align(4);
        var s = ReadBytes(4);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadSingleLittleEndian(s)
            : BinaryPrimitives.ReadSingleBigEndian(s);
    }

    /// <summary>IDL `double` -> 8 bytes IEEE-754, Align(8).</summary>
    public double ReadFloat64()
    {
        Align(8);
        var s = ReadBytes(8);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadDoubleLittleEndian(s)
            : BinaryPrimitives.ReadDoubleBigEndian(s);
    }

    /// <summary>IDL `wchar` -> 2-byte UTF-16 code unit, Align(2).</summary>
    public char ReadWChar() => (char)ReadUInt16();

    // ---------------------------------------------------------------------
    // String / wstring
    // ---------------------------------------------------------------------

    /// <summary>
    /// IDL `string` -> uint32 length-incl-NUL + UTF-8 bytes + NUL.
    /// Throws if the terminating NUL is missing.
    /// </summary>
    public string ReadString()
    {
        uint len = ReadUInt32();
        if (len == 0)
        {
            throw new XcdrException("string length must be >= 1 (NUL terminator required)");
        }
        var bytes = ReadBytes((int)len);
        // The last byte MUST be NUL.
        if (bytes[bytes.Length - 1] != 0)
        {
            throw new XcdrException("string is not NUL-terminated");
        }
        return Encoding.UTF8.GetString(bytes.Slice(0, bytes.Length - 1));
    }

    /// <summary>
    /// IDL `wstring` -> uint32 length (code units, no NUL) + UTF-16-LE code units.
    /// </summary>
    public string ReadWString()
    {
        uint len = ReadUInt32();
        var sb = new StringBuilder((int)len);
        for (uint i = 0; i < len; i++)
        {
            sb.Append((char)ReadUInt16());
        }
        return sb.ToString();
    }

    /// <summary>Reads the sequence counter (uint32, Align(4)).</summary>
    public int ReadSequenceLength()
    {
        uint len = ReadUInt32();
        if (len > int.MaxValue)
        {
            throw new XcdrException($"sequence length overflow: {len}");
        }
        return (int)len;
    }

    // ---------------------------------------------------------------------
    // DHEADER + EMHEADER
    // ---------------------------------------------------------------------

    /// <summary>
    /// Reads the 4-byte DHEADER (object-size in bytes), sets a new
    /// alignment origin at the position right after the header, and
    /// returns a token for restore + bound check.
    /// </summary>
    public DHeaderReadScope BeginDHeader()
    {
        Align(4);
        uint size = ReadUInt32();
        int previousOrigin = _origin;
        _origin = _pos;
        if (_pos + size > _buf.Length)
        {
            throw new XcdrException(
                $"DHEADER size {size} exceeds buffer (pos={_pos}, len={_buf.Length})");
        }
        return new DHeaderReadScope(_pos, _origin + (int)size, previousOrigin);
    }

    /// <summary>
    /// Closes the DHEADER scope: skips any remaining trailing padding
    /// up to the object-size end and restores the previous origin.
    /// </summary>
    public void EndDHeader(DHeaderReadScope scope)
    {
        if (_pos > scope.BodyEnd)
        {
            throw new XcdrException(
                $"read past DHEADER body-end (pos={_pos}, body-end={scope.BodyEnd})");
        }
        _pos = scope.BodyEnd;
        _origin = scope.PreviousOrigin;
    }

    /// <summary>`true` when the current position has reached the body-end of the DHEADER scope.</summary>
    public bool DHeaderDone(DHeaderReadScope scope) => _pos >= scope.BodyEnd;

    /// <summary>
    /// Reads a 4-byte EMHEADER (ambient stream-endian per XTypes 1.3
    /// §7.4.3.4.5) and returns (memberId, lc, must_understand).
    /// </summary>
    public (uint MemberId, int Lc, bool MustUnderstand) ReadEmHeader()
    {
        uint header = ReadUInt32();
        bool mu = (header & 0x80000000u) != 0;
        int lc = (int)((header >> 28) & 0x7u);
        uint id = header & 0x0FFFFFFFu;
        return (id, lc, mu);
    }
}

/// <summary>
/// Token for a DHEADER scope while reading. Holds the absolute body-end
/// and the previous alignment origin.
/// </summary>
public readonly struct DHeaderReadScope
{
    /// <summary>Absolute buffer position of the body start (right after the 4-byte header).</summary>
    public int BodyStart { get; }

    /// <summary>Absolute buffer position right after the last body byte.</summary>
    public int BodyEnd { get; }

    /// <summary>Origin before the `BeginDHeader` call, for restore in `EndDHeader`.</summary>
    public int PreviousOrigin { get; }

    internal DHeaderReadScope(int bodyStart, int bodyEnd, int previousOrigin)
    {
        BodyStart = bodyStart;
        BodyEnd = bodyEnd;
        PreviousOrigin = previousOrigin;
    }
}
