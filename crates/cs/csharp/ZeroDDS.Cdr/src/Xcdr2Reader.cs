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
/// XCDR2-Decoder.  Inverse zu <see cref="Xcdr2Writer"/>.
///
/// Alignment-Regel laut XTypes 1.3 §7.4.1.5 ist relativ zur aktuellen
/// Origin (initial 0; bei DHEADER-Body neu gesetzt).
/// </summary>
public ref struct Xcdr2Reader
{
    private readonly ReadOnlySpan<byte> _buf;
    private readonly EndianMode _endian;
    private int _pos;
    private int _origin;

    /// <summary>Konstruktor mit Default-Endianness (Little-Endian).</summary>
    public Xcdr2Reader(ReadOnlySpan<byte> bytes) : this(bytes, EndianMode.LittleEndian) { }

    /// <summary>Konstruktor mit expliziter Endianness.</summary>
    public Xcdr2Reader(ReadOnlySpan<byte> bytes, EndianMode endian)
    {
        _buf = bytes;
        _endian = endian;
        _pos = 0;
        _origin = 0;
    }

    /// <summary>Aktive Endianness.</summary>
    public EndianMode Endian => _endian;

    /// <summary>Gelesene Byte-Position.</summary>
    public int Position => _pos;

    /// <summary>Anzahl noch verfuegbarer Bytes ab `Position`.</summary>
    public int Remaining => _buf.Length - _pos;

    // ---------------------------------------------------------------------
    // Alignment + raw bytes
    // ---------------------------------------------------------------------

    /// <summary>Padding-Skip zur N-Byte-Boundary relativ zur Origin.</summary>
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

    /// <summary>Liefert einen Slice der naechsten N Bytes ohne Endianness-Konvertierung.</summary>
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

    /// <summary>IDL `boolean` -> 1 Byte.</summary>
    public bool ReadBool()
    {
        var b = ReadByte();
        if (b != 0 && b != 1)
        {
            throw new XcdrException($"invalid boolean encoding 0x{b:X2}");
        }
        return b != 0;
    }

    /// <summary>IDL `octet` / `char` -> 1 Byte.</summary>
    public byte ReadByte()
    {
        if (_pos >= _buf.Length)
        {
            throw new XcdrException("read past end-of-buffer");
        }
        return _buf[_pos++];
    }

    /// <summary>IDL `octet` (alias auf ReadByte fuer Symmetrie zu Writer).</summary>
    public byte ReadOctet() => ReadByte();

    /// <summary>IDL `short` -> 2 Byte Align(2).</summary>
    public short ReadInt16()
    {
        Align(2);
        var s = ReadBytes(2);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadInt16LittleEndian(s)
            : BinaryPrimitives.ReadInt16BigEndian(s);
    }

    /// <summary>IDL `unsigned short` -> 2 Byte Align(2).</summary>
    public ushort ReadUInt16()
    {
        Align(2);
        var s = ReadBytes(2);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadUInt16LittleEndian(s)
            : BinaryPrimitives.ReadUInt16BigEndian(s);
    }

    /// <summary>IDL `long` -> 4 Byte Align(4).</summary>
    public int ReadInt32()
    {
        Align(4);
        var s = ReadBytes(4);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadInt32LittleEndian(s)
            : BinaryPrimitives.ReadInt32BigEndian(s);
    }

    /// <summary>IDL `unsigned long` -> 4 Byte Align(4).</summary>
    public uint ReadUInt32()
    {
        Align(4);
        var s = ReadBytes(4);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadUInt32LittleEndian(s)
            : BinaryPrimitives.ReadUInt32BigEndian(s);
    }

    /// <summary>IDL `long long` -> 8 Byte Align(8).</summary>
    public long ReadInt64()
    {
        Align(8);
        var s = ReadBytes(8);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadInt64LittleEndian(s)
            : BinaryPrimitives.ReadInt64BigEndian(s);
    }

    /// <summary>IDL `unsigned long long` -> 8 Byte Align(8).</summary>
    public ulong ReadUInt64()
    {
        Align(8);
        var s = ReadBytes(8);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadUInt64LittleEndian(s)
            : BinaryPrimitives.ReadUInt64BigEndian(s);
    }

    /// <summary>IDL `float` -> 4 Byte IEEE-754 Align(4).</summary>
    public float ReadFloat32()
    {
        Align(4);
        var s = ReadBytes(4);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadSingleLittleEndian(s)
            : BinaryPrimitives.ReadSingleBigEndian(s);
    }

    /// <summary>IDL `double` -> 8 Byte IEEE-754 Align(8).</summary>
    public double ReadFloat64()
    {
        Align(8);
        var s = ReadBytes(8);
        return _endian == EndianMode.LittleEndian
            ? BinaryPrimitives.ReadDoubleLittleEndian(s)
            : BinaryPrimitives.ReadDoubleBigEndian(s);
    }

    /// <summary>IDL `wchar` -> 2 Byte UTF-16 Code-Unit, Align(2).</summary>
    public char ReadWChar() => (char)ReadUInt16();

    // ---------------------------------------------------------------------
    // String / wstring
    // ---------------------------------------------------------------------

    /// <summary>
    /// IDL `string` -> uint32 length-incl-NUL + UTF-8 bytes + NUL.
    /// Wirft bei fehlendem terminierenden NUL.
    /// </summary>
    public string ReadString()
    {
        uint len = ReadUInt32();
        if (len == 0)
        {
            throw new XcdrException("string length must be >= 1 (NUL terminator required)");
        }
        var bytes = ReadBytes((int)len);
        // Letztes Byte MUSS NUL sein.
        if (bytes[bytes.Length - 1] != 0)
        {
            throw new XcdrException("string is not NUL-terminated");
        }
        return Encoding.UTF8.GetString(bytes.Slice(0, bytes.Length - 1));
    }

    /// <summary>
    /// IDL `wstring` -> uint32 length (Code-Units, ohne NUL) + UTF-16-LE Code-Units.
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

    /// <summary>Liest den Sequenz-Counter (uint32, Align(4)).</summary>
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
    /// Liest 4-Byte DHEADER (object-size in Bytes), setzt eine neue
    /// Alignment-Origin auf die Position direkt hinter dem Header und
    /// liefert ein Token zum Restore + Bound-Check.
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
    /// Schliesst den DHEADER-Scope: skipt verbleibendes Trailing-Padding
    /// bis zum object-size-Ende und stellt die alte Origin wieder her.
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

    /// <summary>`true` wenn die aktuelle Position das Body-Ende des DHEADER-Scopes erreicht hat.</summary>
    public bool DHeaderDone(DHeaderReadScope scope) => _pos >= scope.BodyEnd;

    /// <summary>
    /// Liest einen 4-Byte EMHEADER (ambient Stream-Endian gemaess XTypes 1.3
    /// §7.4.3.4.5) und liefert (memberId, lc, must_understand).
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
/// Token fuer einen DHEADER-Scope beim Lesen. Haelt das absolute Body-Ende
/// und die vorherige Alignment-Origin.
/// </summary>
public readonly struct DHeaderReadScope
{
    /// <summary>Absolute Buffer-Position des Body-Starts (direkt hinter dem 4-Byte-Header).</summary>
    public int BodyStart { get; }

    /// <summary>Absolute Buffer-Position direkt hinter dem letzten Body-Byte.</summary>
    public int BodyEnd { get; }

    /// <summary>Origin vor dem `BeginDHeader`-Call, fuer Restore in `EndDHeader`.</summary>
    public int PreviousOrigin { get; }

    internal DHeaderReadScope(int bodyStart, int bodyEnd, int previousOrigin)
    {
        BodyStart = bodyStart;
        BodyEnd = bodyEnd;
        PreviousOrigin = previousOrigin;
    }
}
