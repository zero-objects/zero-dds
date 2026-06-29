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

    /// <summary>
    /// XCDR2 maximum alignment (XTypes 1.3 §7.4.1.1.1): 8-byte primitives align
    /// to 4, never 8. Mirrors the zerodds-cdr core
    /// (`crates/cdr/src/buffer.rs`: `alignment.min(self.max_alignment)`).
    /// </summary>
    private const int Xcdr2MaxAlignment = 4;
    private const int Xcdr1MaxAlignment = 8;

    /// <summary>
    /// Effective alignment cap: 4 for XCDR2 (8-byte primitives align to 4), 8 for
    /// XCDR1 / classic CDR. Set from the representation at construction.
    /// </summary>
    private readonly int _maxAlignment;

    /// <summary>Constructor with default endianness (little-endian), XCDR2.</summary>
    public Xcdr2Reader(ReadOnlySpan<byte> bytes) : this(bytes, EndianMode.LittleEndian) { }

    /// <summary>Constructor with explicit endianness, XCDR2 representation.</summary>
    public Xcdr2Reader(ReadOnlySpan<byte> bytes, EndianMode endian)
        : this(bytes, endian, Xcdr2MaxAlignment) { }

    /// <summary>
    /// Constructor with explicit endianness and alignment cap. Pass
    /// <see cref="Xcdr1MaxAlignmentValue"/> (8) to read the XCDR1 / classic CDR
    /// wire (no DHEADER, PL_CDR1 for @mutable); the default 4 reads XCDR2.
    /// </summary>
    public Xcdr2Reader(ReadOnlySpan<byte> bytes, EndianMode endian, int maxAlignment)
    {
        _buf = bytes;
        _endian = endian;
        _pos = 0;
        _origin = 0;
        _maxAlignment = maxAlignment == Xcdr1MaxAlignment ? Xcdr1MaxAlignment : Xcdr2MaxAlignment;
    }

    /// <summary>XCDR1 alignment-cap value (8) for the 3-arg constructor.</summary>
    public const int Xcdr1MaxAlignmentValue = Xcdr1MaxAlignment;

    /// <summary>Active endianness.</summary>
    public EndianMode Endian => _endian;

    /// <summary>`true` when reading the XCDR1 / classic CDR wire (alignment cap 8,
    /// no DHEADER, PL_CDR1 for @mutable). Generated decoders branch on it.</summary>
    public bool IsXcdr1 => _maxAlignment == Xcdr1MaxAlignment;

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
        // XCDR2 caps the effective alignment at 4 (§7.4.1.1.1); XCDR1 at 8.
        int effective = alignment < _maxAlignment ? alignment : _maxAlignment;
        int offset = _pos - _origin;
        int pad = (effective - (offset % effective)) % effective;
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

    /// <summary>
    /// Reads an IDL <c>fixed&lt;P,S&gt;</c>: (P+2)/2 packed-BCD octets back into a
    /// <see cref="decimal"/>. Inverse of <c>Xcdr2Writer.WriteFixedBcd</c>.
    /// </summary>
    public decimal ReadFixedBcd(int p, int s)
    {
        int n = (p + 2) / 2;
        var raw = ReadBytes(n);
        var chars = new System.Collections.Generic.List<char>();
        char sign = '+';
        for (int i = 0; i < n; i++)
        {
            int hi = (raw[i] >> 4) & 0x0F;
            int lo = raw[i] & 0x0F;
            chars.Add((char)('0' + (hi % 10)));
            if (i == n - 1) sign = lo == 0x0D ? '-' : '+';
            else chars.Add((char)('0' + (lo % 10)));
        }
        while (chars.Count > s + 1 && chars[0] == '0') chars.RemoveAt(0);
        var sb = new System.Text.StringBuilder();
        if (sign == '-') sb.Append('-');
        if (s > 0)
        {
            int dot = Math.Max(chars.Count - s, 0);
            for (int i = 0; i < chars.Count; i++)
            {
                if (i == dot) sb.Append('.');
                sb.Append(chars[i]);
            }
        }
        else
        {
            sb.Append(chars.ToArray());
        }
        return decimal.Parse(sb.ToString(), System.Globalization.CultureInfo.InvariantCulture);
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
        // XCDR1 / classic CDR has no DHEADER — an @appendable/@final aggregate
        // continues in the same stream. Return a frame-less scope so the matching
        // EndDHeader is a no-op and member reads run over the current stream.
        if (_maxAlignment != Xcdr2MaxAlignment)
        {
            return new DHeaderReadScope(_pos, _buf.Length, _origin, hasFrame: false);
        }
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
        if (!scope.HasFrame)
        {
            return; // XCDR1: no frame to close.
        }
        if (_pos > scope.BodyEnd)
        {
            throw new XcdrException(
                $"read past DHEADER body-end (pos={_pos}, body-end={scope.BodyEnd})");
        }
        _pos = scope.BodyEnd;
        _origin = scope.PreviousOrigin;
    }

    /// <summary>
    /// Begins one PL_CDR1 (@mutable XCDR1) member: a 4-byte aligned header
    /// [u16 PID][u16 length], then shifts the alignment origin to the body start
    /// (PL_CDR1 bodies align member-relative, like the cdr-core fresh-reader
    /// path) so the member's own decoder reads inline from this reader. Returns
    /// <c>false</c> at the PID_LIST_END sentinel. PID_EXTENDED (length 8) carries
    /// a 32-bit member id + 32-bit body length. Pair with
    /// <see cref="EndPlCdr1Member"/>. Mirrors cdr-core `xcdr1::read_pl_cdr1_member`.
    /// </summary>
    public bool BeginPlCdr1Member(out uint memberId, out DHeaderReadScope scope)
    {
        const ushort PidListEnd = 0x3F02;
        const ushort PidExtended = 0x3F01;
        Align(4);
        if (_pos + 4 > _buf.Length)
        {
            memberId = 0;
            scope = default;
            return false;
        }
        ushort pid = ReadUInt16();
        ushort lenU16 = ReadUInt16();
        if (pid == PidListEnd)
        {
            memberId = 0;
            scope = default;
            return false;
        }
        int bodyLen;
        if (pid == PidExtended)
        {
            memberId = ReadUInt32();
            bodyLen = (int)ReadUInt32();
        }
        else
        {
            memberId = pid;
            bodyLen = lenU16;
        }
        if (_pos + bodyLen > _buf.Length)
        {
            throw new XcdrException(
                $"PL_CDR1 member body {bodyLen} exceeds buffer (pos={_pos}, len={_buf.Length})");
        }
        int prevOrigin = _origin;
        _origin = _pos; // member-relative alignment
        scope = new DHeaderReadScope(_pos, _pos + bodyLen, prevOrigin);
        return true;
    }

    /// <summary>Closes a <see cref="BeginPlCdr1Member"/> scope: positions at the
    /// body end, skips the trailing 4-byte pad, and restores the origin.</summary>
    public void EndPlCdr1Member(DHeaderReadScope scope)
    {
        _pos = scope.BodyEnd;
        _origin = scope.PreviousOrigin;
        int bodyLen = scope.BodyEnd - scope.BodyStart;
        int pad = (4 - (bodyLen % 4)) % 4;
        for (int i = 0; i < pad && _pos < _buf.Length; i++)
        {
            _pos++;
        }
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

    /// <summary>`false` for an XCDR1 reader, where aggregates carry no DHEADER —
    /// `BeginDHeader`/`EndDHeader` are then no-ops over the same stream.</summary>
    public bool HasFrame { get; }

    internal DHeaderReadScope(int bodyStart, int bodyEnd, int previousOrigin, bool hasFrame = true)
    {
        BodyStart = bodyStart;
        BodyEnd = bodyEnd;
        PreviousOrigin = previousOrigin;
        HasFrame = hasFrame;
    }
}
