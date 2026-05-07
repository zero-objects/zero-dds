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
/// XCDR2-Encoder fuer PLAIN_CDR2 / DELIMITED_CDR2 / PL_CDR2.
///
/// Alignment-Regel laut XTypes 1.3 §7.4.1.5: Member-Alignment ist
/// relativ zum Start der jeweiligen Encapsulation. DHEADER bricht
/// die Alignment-Origin (Inhalts-Body startet eine neue Origin).
/// </summary>
public sealed class Xcdr2Writer
{
    private readonly MemoryStream _buf;
    private readonly EndianMode _endian;

    /// <summary>
    /// Aktuelle Alignment-Origin in Buffer-Position. Bei `BeginAppendable`
    /// und `BeginMutable` wird sie auf `Position` nach DHEADER-Reservierung
    /// gesetzt; alle nachfolgenden `Align(N)`-Aufrufe sind relativ dazu.
    /// </summary>
    private long _alignOrigin;

    /// <summary>Konstruktor mit Default-Endianness (Little-Endian).</summary>
    public Xcdr2Writer() : this(EndianMode.LittleEndian) { }

    /// <summary>Konstruktor mit expliziter Endianness.</summary>
    public Xcdr2Writer(EndianMode endian)
    {
        _buf = new MemoryStream();
        _endian = endian;
        _alignOrigin = 0;
    }

    /// <summary>Aktive Endianness (read-only).</summary>
    public EndianMode Endian => _endian;

    /// <summary>Bisher geschriebene Byte-Anzahl.</summary>
    public long Position => _buf.Position;

    // ---------------------------------------------------------------------
    // Alignment + raw bytes
    // ---------------------------------------------------------------------

    /// <summary>
    /// Padding zu N-Byte-Alignment relativ zur aktuellen Origin.
    /// XTypes §7.4.1.5: `pad = (N - ((pos - origin) % N)) % N`.
    /// </summary>
    /// <exception cref="ArgumentOutOfRangeException">N nicht in {1,2,4,8}.</exception>
    public void Align(int alignment)
    {
        if (alignment != 1 && alignment != 2 && alignment != 4 && alignment != 8)
        {
            throw new ArgumentOutOfRangeException(nameof(alignment),
                "alignment must be one of {1,2,4,8}");
        }
        long offset = _buf.Position - _alignOrigin;
        long pad = (alignment - (offset % alignment)) % alignment;
        for (long i = 0; i < pad; i++)
        {
            _buf.WriteByte(0);
        }
    }

    /// <summary>Schreibt `value` als rohen Byte (kein Alignment).</summary>
    public void WriteByte(byte value) => _buf.WriteByte(value);

    /// <summary>Schreibt eine Byte-Sequenz (kein Alignment, kein Length-Prefix).</summary>
    public void WriteBytes(ReadOnlySpan<byte> data)
    {
#if NETSTANDARD2_0
        var arr = data.ToArray();
        _buf.Write(arr, 0, arr.Length);
#else
        _buf.Write(data);
#endif
    }

    // ---------------------------------------------------------------------
    // Primitive writers
    // ---------------------------------------------------------------------

    /// <summary>IDL `boolean` -> 1 Byte (0x00 / 0x01).</summary>
    public void WriteBool(bool value) => _buf.WriteByte(value ? (byte)1 : (byte)0);

    /// <summary>IDL `octet` / `char` -> 1 Byte.</summary>
    public void WriteOctet(byte value) => _buf.WriteByte(value);

    /// <summary>IDL `short` -> 2 Byte LE/BE Align(2).</summary>
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

    /// <summary>IDL `unsigned short` -> 2 Byte LE/BE Align(2).</summary>
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

    /// <summary>IDL `long` -> 4 Byte LE/BE Align(4).</summary>
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

    /// <summary>IDL `unsigned long` -> 4 Byte LE/BE Align(4).</summary>
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

    /// <summary>IDL `long long` -> 8 Byte LE/BE Align(8).</summary>
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

    /// <summary>IDL `unsigned long long` -> 8 Byte LE/BE Align(8).</summary>
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

    /// <summary>IDL `float` -> 4 Byte IEEE-754 LE/BE Align(4).</summary>
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

    /// <summary>IDL `double` -> 8 Byte IEEE-754 LE/BE Align(8).</summary>
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

    /// <summary>IDL `wchar` -> 2 Byte UTF-16 LE Code-Unit, Align(2).</summary>
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
    /// IDL `wstring` -> uint32 length (Code-Units, ohne NUL) + UTF-16-LE Code-Units.
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

    /// <summary>Schreibt einen `uint32`-Sequenz-Counter (4 Byte, Align(4)).</summary>
    public void WriteSequenceLength(int count)
    {
        if (count < 0) throw new ArgumentOutOfRangeException(nameof(count));
        WriteUInt32((uint)count);
    }

    // ---------------------------------------------------------------------
    // DHEADER (Appendable / Mutable)
    // ---------------------------------------------------------------------

    /// <summary>
    /// Reserviert 4 Byte fuer den DHEADER (object-size in Bytes, ohne den
    /// 4-Byte-Header selbst), startet eine neue Alignment-Origin direkt
    /// hinter dem Header und liefert ein Token fuer `EndDHeader`.
    ///
    /// XTypes §7.4.4.4 (Appendable) / §7.4.2 (Mutable).
    /// </summary>
    public DHeaderScope BeginDHeader()
    {
        Align(4);
        long headerPos = _buf.Position;
        // Placeholder fuer die spaeter-eingetragene Groesse:
        Span<byte> placeholder = stackalloc byte[4];
        WriteBytes(placeholder);
        long previousOrigin = _alignOrigin;
        _alignOrigin = _buf.Position;
        return new DHeaderScope(this, headerPos, previousOrigin, _alignOrigin);
    }

    /// <summary>
    /// Patcht die object-size im DHEADER-Slot und stellt die vorherige
    /// Alignment-Origin wieder her.
    /// </summary>
    internal void EndDHeader(DHeaderScope scope)
    {
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

    /// <summary>Convenience-Alias fuer `BeginDHeader` (semantisch: Appendable).</summary>
    public DHeaderScope BeginAppendable() => BeginDHeader();

    /// <summary>Convenience-Alias fuer `BeginDHeader` (semantisch: Mutable).</summary>
    public DHeaderScope BeginMutable() => BeginDHeader();

    // ---------------------------------------------------------------------
    // EMHEADER (Mutable PL_CDR2)
    // ---------------------------------------------------------------------

    /// <summary>
    /// Schreibt einen 4-Byte-EMHEADER fuer einen Member.
    ///
    /// XTypes 1.3 §7.4.3.4.5: EMHEADER1 folgt dem ambient Stream-Endian.
    /// In LE-Stream → LE Bytes; in BE-Stream → BE Bytes. Das ist konsistent
    /// mit dem zerodds_cdr Reference-Encoder und Cyclone-DDS.
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
    /// Schreibt einen variabel-laengen-EMHEADER (LC=4..7) inkl. der nachfolgenden
    /// 4-Byte NEXTINT length. LC=5 (next-int = size in bytes) ist die spec-
    /// konforme Variante fuer `string` und nested DHEADER-Member.
    /// </summary>
    public void WriteEmHeaderNextInt(uint memberId, int lc, bool mustUnderstand, uint nextInt)
    {
        if (lc < 4 || lc > 7) throw new ArgumentOutOfRangeException(nameof(lc));
        WriteEmHeader(memberId, lc, mustUnderstand);
        WriteUInt32(nextInt);
    }

    // ---------------------------------------------------------------------
    // Final assembly
    // ---------------------------------------------------------------------

    /// <summary>Liefert den geschriebenen Buffer als Byte-Array (Copy).</summary>
    public byte[] ToArray() => _buf.ToArray();
}

/// <summary>
/// RAII-Token fuer einen DHEADER-Scope. Bei `Dispose` wird die object-size
/// im 4-Byte-Slot patched und die alte Alignment-Origin wiederhergestellt.
/// </summary>
public readonly struct DHeaderScope : IDisposable
{
    private readonly Xcdr2Writer _writer;
    internal long HeaderPos { get; }
    internal long PreviousOrigin { get; }
    internal long BodyStart { get; }

    internal DHeaderScope(Xcdr2Writer writer, long headerPos, long previousOrigin, long bodyStart)
    {
        _writer = writer;
        HeaderPos = headerPos;
        PreviousOrigin = previousOrigin;
        BodyStart = bodyStart;
    }

    /// <summary>Patcht den DHEADER und schliesst den Scope.</summary>
    public void Dispose() => _writer.EndDHeader(this);
}
