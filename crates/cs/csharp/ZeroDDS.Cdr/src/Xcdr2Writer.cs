// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// XCDR2 Encoder.
// Spec: OMG XTypes 1.3 §7.4 + zerodds-xcdr2-csharp-1.0 §6/§7
// + zerodds-xcdr2-bindings-conformance-1.0 §6 (V-1..V-12).

using System;
using System.Buffers.Binary;
using System.Collections.Generic;
using System.IO;
using System.Text;

namespace ZeroDDS.Cdr;

/// <summary>
/// XCDR2 encoder for PLAIN_CDR2 / DELIMITED_CDR2 / PL_CDR2.
///
/// Per XTypes 1.3 §7.4.1.5 the alignment rule is: member alignment is
/// relative to the start of the respective encapsulation. A DHEADER breaks
/// the alignment origin (the content body starts a new origin).
/// </summary>
public sealed class Xcdr2Writer
{
    private readonly MemoryStream _buf;
    private readonly EndianMode _endian;

    /// <summary>
    /// XCDR2 maximum alignment (XTypes 1.3 §7.4.1.1.1): every member's required
    /// alignment is capped at 4, so 8-byte primitives (double / int64 / uint64)
    /// align to 4, NEVER to 8. Mirrors the cross-vendor-validated zerodds-cdr
    /// core (`crates/cdr/src/buffer.rs`: `alignment.min(self.max_alignment)`,
    /// XCDR2_MAX_ALIGNMENT == 4).
    /// </summary>
    private const int Xcdr2MaxAlignment = 4;
    private const int Xcdr1MaxAlignment = 8;

    /// <summary>XCDR1 alignment-cap value (8) for the 2-arg constructor.</summary>
    public const int Xcdr1MaxAlignmentValue = Xcdr1MaxAlignment;

    /// <summary>Effective alignment cap: 4 for XCDR2, 8 for XCDR1 / classic CDR.</summary>
    private readonly int _maxAlignment;

    /// <summary>
    /// Current alignment origin as a buffer position. On `BeginAppendable`
    /// and `BeginMutable` it is set to `Position` after the DHEADER reservation;
    /// all subsequent `Align(N)` calls are relative to it.
    /// </summary>
    private long _alignOrigin;

    /// <summary>Constructor with default endianness (little-endian), XCDR2.</summary>
    public Xcdr2Writer() : this(EndianMode.LittleEndian) { }

    /// <summary>Constructor with explicit endianness, XCDR2 representation.</summary>
    public Xcdr2Writer(EndianMode endian) : this(endian, Xcdr2MaxAlignment) { }

    /// <summary>
    /// Constructor with explicit endianness + alignment cap. Pass
    /// <see cref="Xcdr1MaxAlignmentValue"/> (8) to write the XCDR1 / classic CDR
    /// wire (no DHEADER, PL_CDR1 for @mutable); the default 4 writes XCDR2.
    /// </summary>
    public Xcdr2Writer(EndianMode endian, int maxAlignment)
    {
        _buf = new MemoryStream();
        _endian = endian;
        _alignOrigin = 0;
        _maxAlignment = maxAlignment == Xcdr1MaxAlignment ? Xcdr1MaxAlignment : Xcdr2MaxAlignment;
    }

    /// <summary>Active endianness (read-only).</summary>
    public EndianMode Endian => _endian;

    /// <summary>`true` when writing the XCDR1 / classic CDR wire.</summary>
    public bool IsXcdr1 => _maxAlignment == Xcdr1MaxAlignment;

    /// <summary>Number of bytes written so far.</summary>
    public long Position => _buf.Position;

    // ---------------------------------------------------------------------
    // Alignment + raw bytes
    // ---------------------------------------------------------------------

    /// <summary>
    /// Padding to N-byte alignment relative to the current origin.
    /// XTypes §7.4.1.5: `pad = (N - ((pos - origin) % N)) % N`.
    /// </summary>
    /// <exception cref="ArgumentOutOfRangeException">N not in {1,2,4,8}.</exception>
    public void Align(int alignment)
    {
        if (alignment != 1 && alignment != 2 && alignment != 4 && alignment != 8)
        {
            throw new ArgumentOutOfRangeException(nameof(alignment),
                "alignment must be one of {1,2,4,8}");
        }
        // Caps the effective alignment at the representation max (XCDR2 = 4,
        // XCDR1 = 8); cdr-core does the same via `alignment.min(max_alignment)`.
        int effective = alignment < _maxAlignment ? alignment : _maxAlignment;
        long offset = _buf.Position - _alignOrigin;
        long pad = (effective - (offset % effective)) % effective;
        for (long i = 0; i < pad; i++)
        {
            _buf.WriteByte(0);
        }
    }

    /// <summary>Writes `value` as a raw byte (no alignment).</summary>
    public void WriteByte(byte value) => _buf.WriteByte(value);

    /// <summary>Writes a byte sequence (no alignment, no length prefix).</summary>
    public void WriteBytes(ReadOnlySpan<byte> data)
    {
#if NETSTANDARD2_0
        var arr = data.ToArray();
        _buf.Write(arr, 0, arr.Length);
#else
        _buf.Write(data);
#endif
    }

    /// <summary>
    /// Writes an IDL <c>fixed&lt;P,S&gt;</c> value as CORBA/GIOP §9.3.2.7 packed
    /// BCD: P digit nibbles MSB-first + a sign nibble (0xC pos, 0xD neg), a
    /// leading 0x0 pad when P+1 is odd, packed 2 nibbles/byte high-first.
    /// (P+2)/2 octets, no length prefix, no alignment, endian-independent.
    /// Ported from <c>crates/cdr/src/fixed.rs</c> (oracle-validated).
    /// </summary>
    public void WriteFixedBcd(decimal value, int p, int s)
    {
        var text = value.ToString(System.Globalization.CultureInfo.InvariantCulture);
        bool positive = true;
        if (text.StartsWith("-", StringComparison.Ordinal)) { positive = false; text = text.Substring(1); }
        else if (text.StartsWith("+", StringComparison.Ordinal)) { text = text.Substring(1); }
        int dot = text.IndexOf('.');
        string intPart = dot < 0 ? text : text.Substring(0, dot);
        string fracPart = dot < 0 ? string.Empty : text.Substring(dot + 1);
        int intNeeded = p - s;
        if (intPart.Length > intNeeded)
            throw new XcdrException($"fixed: integer part '{intPart}' exceeds P-S={intNeeded}");
        if (fracPart.Length > s)
            throw new XcdrException($"fixed: fractional part '{fracPart}' exceeds S={s}");
        string digits = intPart.PadLeft(intNeeded, '0') + fracPart.PadRight(s, '0');
        var nibbles = new System.Collections.Generic.List<int>();
        if ((p + 1) % 2 == 1) nibbles.Add(0);
        foreach (char c in digits)
        {
            if (c < '0' || c > '9') throw new XcdrException($"fixed: non-digit '{c}'");
            nibbles.Add(c - '0');
        }
        nibbles.Add(positive ? 0x0C : 0x0D);
        var outBytes = new byte[nibbles.Count / 2];
        for (int b = 0; b < outBytes.Length; b++)
            outBytes[b] = (byte)((nibbles[2 * b] << 4) | nibbles[2 * b + 1]);
        WriteBytes(outBytes);
    }

    // ---------------------------------------------------------------------
    // Primitive writers
    // ---------------------------------------------------------------------

    /// <summary>IDL `boolean` -> 1 byte (0x00 / 0x01).</summary>
    public void WriteBool(bool value) => _buf.WriteByte(value ? (byte)1 : (byte)0);

    /// <summary>IDL `octet` / `char` -> 1 byte.</summary>
    public void WriteOctet(byte value) => _buf.WriteByte(value);

    /// <summary>IDL `short` -> 2 bytes LE/BE, Align(2).</summary>
    public void WriteInt16(short value)
    {
        Align(2);
        Span<byte> tmp = stackalloc byte[2];
        if (_endian == EndianMode.LittleEndian)
        {
            BinaryPrimitives.WriteInt16LittleEndian(tmp, value);
        }
        else
        {
            BinaryPrimitives.WriteInt16BigEndian(tmp, value);
        }
        WriteBytes(tmp);
    }

    /// <summary>IDL `unsigned short` -> 2 bytes LE/BE, Align(2).</summary>
    public void WriteUInt16(ushort value)
    {
        Align(2);
        Span<byte> tmp = stackalloc byte[2];
        if (_endian == EndianMode.LittleEndian)
        {
            BinaryPrimitives.WriteUInt16LittleEndian(tmp, value);
        }
        else
        {
            BinaryPrimitives.WriteUInt16BigEndian(tmp, value);
        }
        WriteBytes(tmp);
    }

    /// <summary>IDL `long` -> 4 bytes LE/BE, Align(4).</summary>
    public void WriteInt32(int value)
    {
        Align(4);
        Span<byte> tmp = stackalloc byte[4];
        if (_endian == EndianMode.LittleEndian)
        {
            BinaryPrimitives.WriteInt32LittleEndian(tmp, value);
        }
        else
        {
            BinaryPrimitives.WriteInt32BigEndian(tmp, value);
        }
        WriteBytes(tmp);
    }

    /// <summary>IDL `unsigned long` -> 4 bytes LE/BE, Align(4).</summary>
    public void WriteUInt32(uint value)
    {
        Align(4);
        Span<byte> tmp = stackalloc byte[4];
        if (_endian == EndianMode.LittleEndian)
        {
            BinaryPrimitives.WriteUInt32LittleEndian(tmp, value);
        }
        else
        {
            BinaryPrimitives.WriteUInt32BigEndian(tmp, value);
        }
        WriteBytes(tmp);
    }

    /// <summary>IDL `long long` -> 8 bytes LE/BE, Align(8).</summary>
    public void WriteInt64(long value)
    {
        Align(8);
        Span<byte> tmp = stackalloc byte[8];
        if (_endian == EndianMode.LittleEndian)
        {
            BinaryPrimitives.WriteInt64LittleEndian(tmp, value);
        }
        else
        {
            BinaryPrimitives.WriteInt64BigEndian(tmp, value);
        }
        WriteBytes(tmp);
    }

    /// <summary>IDL `unsigned long long` -> 8 bytes LE/BE, Align(8).</summary>
    public void WriteUInt64(ulong value)
    {
        Align(8);
        Span<byte> tmp = stackalloc byte[8];
        if (_endian == EndianMode.LittleEndian)
        {
            BinaryPrimitives.WriteUInt64LittleEndian(tmp, value);
        }
        else
        {
            BinaryPrimitives.WriteUInt64BigEndian(tmp, value);
        }
        WriteBytes(tmp);
    }

    /// <summary>IDL `float` -> 4 bytes IEEE-754 LE/BE, Align(4).</summary>
    public void WriteFloat32(float value)
    {
        Align(4);
        Span<byte> tmp = stackalloc byte[4];
        if (_endian == EndianMode.LittleEndian)
        {
            BinaryPrimitives.WriteSingleLittleEndian(tmp, value);
        }
        else
        {
            BinaryPrimitives.WriteSingleBigEndian(tmp, value);
        }
        WriteBytes(tmp);
    }

    /// <summary>IDL `double` -> 8 bytes IEEE-754 LE/BE, Align(8).</summary>
    public void WriteFloat64(double value)
    {
        Align(8);
        Span<byte> tmp = stackalloc byte[8];
        if (_endian == EndianMode.LittleEndian)
        {
            BinaryPrimitives.WriteDoubleLittleEndian(tmp, value);
        }
        else
        {
            BinaryPrimitives.WriteDoubleBigEndian(tmp, value);
        }
        WriteBytes(tmp);
    }

    /// <summary>IDL `wchar` -> 2-byte UTF-16 LE code unit, Align(2).</summary>
    public void WriteWChar(char value) => WriteUInt16(value);

    // ---------------------------------------------------------------------
    // String / wstring
    // ---------------------------------------------------------------------

    /// <summary>
    /// IDL `string` -> uint32 length-incl-NUL + UTF-8 bytes + NUL.
    /// XTypes §7.4.4.6.
    /// </summary>
    public void WriteString(string value)
    {
        if (value is null) throw new ArgumentNullException(nameof(value));
        var bytes = Encoding.UTF8.GetBytes(value);
        WriteUInt32((uint)(bytes.Length + 1));
        WriteBytes(bytes);
        _buf.WriteByte(0);
    }

    /// <summary>
    /// IDL `wstring` -> uint32 length (code units, no NUL) + UTF-16-LE code units.
    /// XTypes §9.1 / Conformance §9.1.
    /// </summary>
    public void WriteWString(string value)
    {
        if (value is null) throw new ArgumentNullException(nameof(value));
        WriteUInt32((uint)value.Length);
        for (int i = 0; i < value.Length; i++)
        {
            WriteUInt16(value[i]);
        }
    }

    // ---------------------------------------------------------------------
    // Sequence / array length-prefix
    // ---------------------------------------------------------------------

    /// <summary>Writes a `uint32` sequence counter (4 bytes, Align(4)).</summary>
    public void WriteSequenceLength(int count)
    {
        if (count < 0) throw new ArgumentOutOfRangeException(nameof(count));
        WriteUInt32((uint)count);
    }

    // ---------------------------------------------------------------------
    // DHEADER (Appendable / Mutable)
    // ---------------------------------------------------------------------

    /// <summary>
    /// Reserves 4 bytes for the DHEADER (object-size in bytes, excluding the
    /// 4-byte header itself), starts a new alignment origin right after the
    /// header, and returns a token for `EndDHeader`.
    ///
    /// XTypes §7.4.4.4 (Appendable) / §7.4.2 (Mutable).
    /// </summary>
    public DHeaderScope BeginDHeader()
    {
        // XCDR1 / classic CDR has no DHEADER — write the body inline in the same
        // stream. Return a frame-less scope so EndDHeader patches nothing.
        if (IsXcdr1)
        {
            return new DHeaderScope(this, 0, _alignOrigin, _buf.Position, hasFrame: false);
        }
        Align(4);
        long headerPos = _buf.Position;
        // Placeholder for the size written in later:
        Span<byte> placeholder = stackalloc byte[4];
        WriteBytes(placeholder);
        long previousOrigin = _alignOrigin;
        _alignOrigin = _buf.Position;
        return new DHeaderScope(this, headerPos, previousOrigin, _alignOrigin);
    }

    /// <summary>
    /// Patches the object-size into the DHEADER slot and restores the
    /// previous alignment origin.
    /// </summary>
    internal void EndDHeader(DHeaderScope scope)
    {
        if (!scope.HasFrame)
        {
            return; // XCDR1: no frame to patch.
        }
        long endPos = _buf.Position;
        long bodyStart = scope.BodyStart;
        long size = endPos - bodyStart;
        if (size < 0 || size > uint.MaxValue)
        {
            throw new XcdrException("DHEADER object-size overflow");
        }
        long current = _buf.Position;
        _buf.Position = scope.HeaderPos;
        Span<byte> tmp = stackalloc byte[4];
        if (_endian == EndianMode.LittleEndian)
        {
            BinaryPrimitives.WriteUInt32LittleEndian(tmp, (uint)size);
        }
        else
        {
            BinaryPrimitives.WriteUInt32BigEndian(tmp, (uint)size);
        }
        WriteBytes(tmp);
        _buf.Position = current;
        _alignOrigin = scope.PreviousOrigin;
    }

    /// <summary>Convenience alias for `BeginDHeader` (semantically: Appendable).</summary>
    public DHeaderScope BeginAppendable() => BeginDHeader();

    /// <summary>Convenience alias for `BeginDHeader` (semantically: Mutable).</summary>
    public DHeaderScope BeginMutable() => BeginDHeader();

    // ---------------------------------------------------------------------
    // EMHEADER (Mutable PL_CDR2)
    // ---------------------------------------------------------------------

    /// <summary>
    /// Writes a 4-byte EMHEADER for a member.
    ///
    /// XTypes 1.3 §7.4.3.4.5: EMHEADER1 follows the ambient stream-endian.
    /// In an LE stream → LE bytes; in a BE stream → BE bytes. This is consistent
    /// with the zerodds_cdr reference encoder and CycloneDDS.
    /// </summary>
    public void WriteEmHeader(uint memberId, int lc, bool mustUnderstand)
    {
        if (lc < 0 || lc > 7) throw new ArgumentOutOfRangeException(nameof(lc));
        if (memberId > 0x0FFFFFFF) throw new ArgumentOutOfRangeException(nameof(memberId));
        Align(4);
        uint header = (memberId & 0x0FFFFFFFu)
            | (((uint)lc & 0x7u) << 28)
            | (mustUnderstand ? 0x80000000u : 0u);
        WriteUInt32(header);
    }

    /// <summary>
    /// Writes a variable-length EMHEADER (LC=4..7) including the following
    /// 4-byte NEXTINT length. LC=5 (next-int = size in bytes) is the
    /// spec-conformant variant for `string` and nested DHEADER members.
    /// </summary>
    public void WriteEmHeaderNextInt(uint memberId, int lc, bool mustUnderstand, uint nextInt)
    {
        if (lc < 4 || lc > 7) throw new ArgumentOutOfRangeException(nameof(lc));
        WriteEmHeader(memberId, lc, mustUnderstand);
        WriteUInt32(nextInt);
    }

    // ---------------------------------------------------------------------
    // PL_CDR1 (Mutable XCDR1)
    // ---------------------------------------------------------------------

    /// <summary>
    /// Writes one PL_CDR1 (@mutable XCDR1) member: a 4-byte aligned
    /// [u16 PID][u16 length] header (or the PID_EXTENDED long form for ids
    /// >= 0x3F00 / bodies > 0xFFFF), the member <paramref name="body"/> bytes
    /// (built by a fresh member-relative sub-writer), then zero-pad to the next
    /// 4-byte boundary (not counted in length). Mirrors cdr-core
    /// `xcdr1::encode_pl_cdr1_member`.
    /// </summary>
    public void WritePlCdr1Member(uint memberId, byte[] body)
    {
        Align(4);
        int bodyLen = body.Length;
        if (memberId >= 0x3F00 || bodyLen > 0xFFFF)
        {
            WriteUInt16(0x3F01); // PID_EXTENDED
            WriteUInt16(8);
            WriteUInt32(memberId);
            WriteUInt32((uint)bodyLen);
        }
        else
        {
            WriteUInt16((ushort)memberId);
            WriteUInt16((ushort)bodyLen);
        }
        WriteBytes(body);
        int pad = (4 - (bodyLen % 4)) % 4;
        for (int i = 0; i < pad; i++) _buf.WriteByte(0);
    }

    /// <summary>Writes the PID_LIST_END (0x3F02) terminator of a PL_CDR1 list.</summary>
    public void WritePlCdr1Sentinel()
    {
        Align(4);
        WriteUInt16(0x3F02);
        WriteUInt16(0);
    }

    // ---------------------------------------------------------------------
    // Final assembly
    // ---------------------------------------------------------------------

    /// <summary>Returns the written buffer as a byte array (copy).</summary>
    public byte[] ToArray() => _buf.ToArray();
}

/// <summary>
/// RAII token for a DHEADER scope. On `Dispose` the object-size is patched
/// into the 4-byte slot and the previous alignment origin is restored.
/// </summary>
public readonly struct DHeaderScope : IDisposable
{
    private readonly Xcdr2Writer _writer;
    internal long HeaderPos { get; }
    internal long PreviousOrigin { get; }
    internal long BodyStart { get; }
    internal bool HasFrame { get; }

    internal DHeaderScope(Xcdr2Writer writer, long headerPos, long previousOrigin, long bodyStart, bool hasFrame = true)
    {
        _writer = writer;
        HeaderPos = headerPos;
        PreviousOrigin = previousOrigin;
        BodyStart = bodyStart;
        HasFrame = hasFrame;
    }

    /// <summary>Patches the DHEADER and closes the scope.</summary>
    public void Dispose() => _writer.EndDHeader(this);
}
