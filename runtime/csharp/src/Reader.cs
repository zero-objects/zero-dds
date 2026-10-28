// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
using System;
using System.Text;
using ZeroDds.Idl;

namespace ZeroDds.Wire
{
    /// <summary>
    /// Taking bytes off the wire.
    /// </summary>
    /// <remarks>
    /// <para>
    /// The C# counterpart of <c>crates/wire/src/reader.rs</c> and the mirror of
    /// <see cref="Writer"/> clause for clause. Every read that cannot be
    /// satisfied raises a <see cref="WireException"/> naming what was expected
    /// and where: a reader that returned a zero for a truncated stream would
    /// hand the caller a value that was never sent.
    /// </para>
    /// <para>
    /// Where the Java column widens every unsigned result — <c>readU32</c>
    /// answers a <c>long</c> because Java has no <c>uint</c> — this one returns
    /// the type the value is. <see cref="ReadU64"/> is a <c>ulong</c> and a
    /// number, not a pattern a caller has to decode.
    /// </para>
    /// </remarks>
    public sealed class Reader
    {
        /// <summary>The low fourteen bits of a short parameter id, which are the id.</summary>
        public const int PidIdMask = 0x3FFF;

        private readonly byte[] bytes;
        private int at;
        private readonly Endianness endianness;
        private readonly int maxAlign;
        private int origin;
        private readonly PidFlagPolicy pidFlags;
        private readonly int highestShortId;
        private Status status = Status.Ok;
        private PlCdr1Framing framing = PlCdr1Framing.Defaults();

        /// <summary>
        /// A reader over <paramref name="data"/>, capped at
        /// <paramref name="maxAlignment"/>.
        /// </summary>
        /// <remarks>
        /// Four for XCDR2, eight for XCDR1 — the writer's counterpart. A reader
        /// that skipped padding the writer never wrote reads the wrong bytes
        /// from the second member on.
        /// </remarks>
        /// <param name="data">The bytes; kept by reference and never modified.</param>
        /// <param name="order">The order the encapsulation identifier announced.</param>
        /// <param name="maxAlignment">The encoding's MAXALIGN.</param>
        /// <param name="flags">
        /// The writer's flag policy. The reader needs it for one thing: under
        /// <see cref="PidFlagPolicy.ImplExtensionOnMembers"/> every member id
        /// carries <c>FLAG_IMPL_EXTENSION</c>, so the flag no longer tells a
        /// vendor-private parameter from a user member.
        /// </param>
        /// <param name="highestShort">
        /// The other thing it needs: §7.4.1.2.2 stops the short form's ids at
        /// <c>0x3F00</c> inclusive and one implementation carries the whole low
        /// fourteen bits, so a reader holding the clause's threshold as a
        /// constant reports <c>@id(16383)</c> as a reserved parameter and drops
        /// the member.
        /// </param>
        public Reader(
            byte[] data,
            Endianness order,
            int maxAlignment,
            PidFlagPolicy flags,
            int highestShort)
        {
            if (data == null)
            {
                throw new ArgumentNullException(nameof(data));
            }

            bytes = data;
            at = 0;
            endianness = order;
            maxAlign = Math.Max(1, maxAlignment);
            origin = 0;
            pidFlags = flags;
            highestShortId = highestShort;
        }

        /// <summary>
        /// A reader over <paramref name="data"/> expecting one
        /// implementation's PL_CDR1 framing whole.
        /// </summary>
        /// <remarks>
        /// The counterpart of <c>zdw_reader_set_pl_cdr1</c>. The two parts the
        /// five-argument constructor takes are read off the framing, so a
        /// caller cannot set one and forget the other.
        /// </remarks>
        public Reader(byte[] data, Endianness order, int maxAlignment, PlCdr1Framing plCdr1)
            : this(
                data,
                order,
                maxAlignment,
                plCdr1 == null ? PidFlagPolicy.SentinelMustUnderstand : plCdr1.PidFlags,
                plCdr1 == null ? Writer.PidShortMaxId : plCdr1.HighestShortId)
        {
            framing = plCdr1 ?? PlCdr1Framing.Defaults();
        }

        /// <summary>
        /// A reader over <paramref name="data"/> with XCDR1's MAXALIGN of eight
        /// and the clause's PL_CDR1 framing.
        /// </summary>
        public Reader(byte[] data, Endianness order)
            : this(data, order, 8, PidFlagPolicy.SentinelMustUnderstand, Writer.PidShortMaxId)
        {
        }

        // -- the status a generated decoder reads ------------------------------

        /// <summary>Whether a refusal has been made, so a later read does nothing.</summary>
        private bool Stopped
        {
            get { return status != Status.Ok; }
        }

        /// <summary>How the read is going.</summary>
        public Status ReaderStatus()
        {
            return status;
        }

        /// <summary>Refuse the stream from outside the runtime. The first refusal sticks.</summary>
        public void ReaderRefuse(Status refusal)
        {
            if (refusal != Status.Ok && status == Status.Ok)
            {
                status = refusal;
            }
        }

        /// <summary>
        /// Take back a <see cref="Status.Discarded"/>, and only that one.
        /// </summary>
        /// <remarks>
        /// A member whose own <c>@try_construct</c> is <c>USE_DEFAULT</c> but
        /// which also carries an explicit <c>@default</c> cannot be answered
        /// inside the type's decoder: the decoder knows Table 9's value for the
        /// type and not the literal the member's declaration gave it, and
        /// §7.2.2.4.4.4.9 puts the literal first. So the call site asks for
        /// DISCARD, and where it gets one it clears it and assigns the literal.
        ///
        /// Every other status stays: a truncated stream is not a narrowing.
        /// </remarks>
        /// <returns>Whether there was one.</returns>
        public bool ReaderClearDiscard()
        {
            if (status == Status.Discarded)
            {
                status = Status.Ok;
                return true;
            }

            return false;
        }

        /// <summary>Where it is.</summary>
        public int ReaderPosition()
        {
            return at;
        }

        /// <summary>How many bytes are left.</summary>
        public int ReaderRemaining()
        {
            return Remaining();
        }

        /// <summary>Move the alignment origin to the current position.</summary>
        /// <remarks>
        /// The read-side pair of <c>Writer.BeginPackedElement</c>, for
        /// <c>ElementPadding::Packed</c>. Returns the origin displaced.
        /// </remarks>
        public int ReaderBeginPackedElement()
        {
            int saved = origin;
            origin = at;
            return saved;
        }

        /// <summary>Restore the origin <see cref="ReaderBeginPackedElement"/> displaced.</summary>
        public void ReaderFinishPackedElement(int saved)
        {
            origin = saved;
        }

        /// <summary>
        /// Move the cursor forward to <paramref name="target"/>, which is never
        /// backwards: a framing word pointing behind the cursor means the reader
        /// has already read past what it framed, and seeking back would paper
        /// over it.
        /// </summary>
        public void ReaderSkipTo(long target)
        {
            if (Stopped)
            {
                return;
            }

            if (target < at)
            {
                ReaderRefuse(Status.OutOfRange);
                return;
            }

            if (target > bytes.Length)
            {
                throw new WireException.Truncated(at, target - at, Remaining());
            }

            at = (int)target;
        }

        /// <summary>The cursor, the origin and the status, to be put back later.</summary>
        public ReaderMark ReaderSave()
        {
            return new ReaderMark(at, origin, status);
        }

        /// <summary>Put back what <see cref="ReaderSave"/> took.</summary>
        public void ReaderRestore(ReaderMark mark)
        {
            at = mark.At;
            origin = mark.Origin;
            status = mark.Status;
        }

        /// <summary>
        /// A <c>string</c> the reader materialises itself.
        /// </summary>
        /// <remarks>
        /// <c>runtime/c</c> takes it out of a heap the caller set, because an
        /// embedded target decides where memory comes from. .NET has one answer
        /// to that question and this is it; the name is kept so that one walk
        /// names one operation in both columns.
        /// </remarks>
        public string ReadStringAlloc()
        {
            if (Stopped)
            {
                return string.Empty;
            }

            return ReadString();
        }

        /// <summary>
        /// A <c>wstring</c>, in whichever length convention the policy names.
        /// </summary>
        public string ReadWstringUtf8(WstringLengthPolicy policy, WstringUnitOrder order)
        {
            if (Stopped)
            {
                return string.Empty;
            }

            return ReadWstring(policy, order);
        }

        /// <summary>The inverse of <c>Writer.WriteWstringUtf8Octets</c>.</summary>
        /// <remarks>
        /// <c>WstringCodeUnits::Utf8</c>: the declared length is the UTF-8
        /// octet count, and the terminator <paramref name="policy"/> asks for is
        /// the last of those octets. A count the stream cannot hold, a missing
        /// or an interior NUL, or an ill-formed UTF-8 sequence is refused, never
        /// a replacement character or a truncated value.
        /// </remarks>
        /// <param name="policy">What the length field counts.</param>
        public string ReadWstringUtf8Octets(WstringLengthPolicy policy)
        {
            if (Stopped)
            {
                return string.Empty;
            }

            int where = at;
            uint declared = ReadU32();
            if (declared > (long)Remaining())
            {
                throw new WireException.ImpossibleLength(where, declared, Remaining());
            }

            int from = Take(declared);
            int count = (int)declared;
            if (WstringLengthPolicies.IsTerminated(policy))
            {
                if (count == 0 || bytes[from + count - 1] != 0)
                {
                    throw new WireException.OutOfRange(
                        where, "a NUL octet ending the wstring, as its count includes one");
                }

                // A NUL inside the text is the narrow string's problem in wide
                // form: every C peer stops at it.
                for (int index = 0; index < count - 1; index++)
                {
                    if (bytes[from + index] == 0)
                    {
                        throw new WireException.OutOfRange(
                            where, "no NUL inside a wstring a C peer would stop at");
                    }
                }

                count -= 1;
            }

            return DecodeUtf8(from, count, where);
        }

        /// <summary>
        /// How many octets <paramref name="text"/> is, which is what a
        /// <c>string&lt;N&gt;</c> bound counts.
        /// </summary>
        public static long Utf8Octets(string text)
        {
            return text == null ? 0L : System.Text.Encoding.UTF8.GetByteCount(text);
        }

        /// <summary>
        /// How many UTF-16 code units <paramref name="text"/> is, which is what
        /// a <c>wstring&lt;N&gt;</c> bound counts. A surrogate pair is two.
        /// </summary>
        public static long Utf16Units(string text)
        {
            return text == null ? 0L : text.Length;
        }

        /// <summary>
        /// <paramref name="text"/> cut to at most <paramref name="octets"/>
        /// bytes, never inside a character.
        /// </summary>
        /// <remarks>
        /// Table 11: "The order of the characters in the string ... is
        /// preserved", so this is a prefix. A character that would straddle the
        /// bound is left out whole: a peer holding <c>char[octets + 1]</c>
        /// cannot store half of one either, and half a UTF-8 sequence is not
        /// text.
        /// </remarks>
        public string TrimUtf8(string text, long octets)
        {
            if (text == null || octets < 0)
            {
                return string.Empty;
            }

            StringBuilder kept = new StringBuilder();
            long used = 0;
            int index = 0;
            while (index < text.Length)
            {
                int step = char.IsHighSurrogate(text[index]) && index + 1 < text.Length
                    && char.IsLowSurrogate(text[index + 1])
                    ? 2
                    : 1;
                long width = System.Text.Encoding.UTF8.GetByteCount(text.Substring(index, step));
                if (used + width > octets)
                {
                    break;
                }

                kept.Append(text, index, step);
                used += width;
                index += step;
            }

            return kept.ToString();
        }

        /// <summary>
        /// <paramref name="text"/> cut to at most <paramref name="units"/>
        /// UTF-16 code units, never between the halves of a surrogate pair.
        /// </summary>
        public string TrimUtf16(string text, long units)
        {
            if (text == null || units < 0)
            {
                return string.Empty;
            }

            if (text.Length <= units)
            {
                return text;
            }

            int end = (int)units;

            // A high surrogate at the cut has lost its pair, and half a pair is
            // not a character.
            if (end > 0 && char.IsHighSurrogate(text[end - 1]))
            {
                end -= 1;
            }

            return text.Substring(0, end);
        }

        // -- state ------------------------------------------------------------

        /// <summary>The byte order in force.</summary>
        public Endianness Order
        {
            get { return endianness; }
        }

        /// <summary>The alignment cap in force.</summary>
        public int MaxAlign
        {
            get { return maxAlign; }
        }

        /// <summary>Where it is.</summary>
        public int Position()
        {
            return at;
        }

        /// <summary>How many bytes are left.</summary>
        public int Remaining()
        {
            return Math.Max(0, bytes.Length - at);
        }

        /// <summary>Skip padding to <paramref name="align"/>, counted from the stream's origin.</summary>
        public void AlignTo(int align)
        {
            // A refusal already made stops every later read: the generated
            // decoder carries on to the end of the type, exactly as the C
            // column's does, and a read that consumed bytes here would put the
            // trailing-bytes check at the top on a position no writer produced.
            if (Stopped)
            {
                return;
            }

            int effective = Math.Min(align, maxAlign);
            if (effective <= 1)
            {
                return;
            }

            int offset = at - origin;
            at += (effective - Modulo(offset, effective)) % effective;
        }

        private static int Modulo(int value, int by)
        {
            int rest = value % by;
            return rest < 0 ? rest + by : rest;
        }

        /// <summary>Step over <paramref name="count"/> bytes, refusing to step past the end.</summary>
        public void Skip(long count)
        {
            if (Stopped)
            {
                return;
            }

            Take(count);
        }

        /// <summary>Move to an absolute offset inside the buffer.</summary>
        public void Seek(int target)
        {
            if (Stopped)
            {
                return;
            }

            if (target < 0 || target > bytes.Length)
            {
                throw new WireException.Truncated(at, target, bytes.Length);
            }

            at = target;
        }

        private int Take(long count)
        {
            if (count < 0)
            {
                throw new WireException.Truncated(at, count, Remaining());
            }

            long end = at + count;
            if (end > bytes.Length)
            {
                throw new WireException.Truncated(at, count, Remaining());
            }

            int from = at;
            at = (int)end;
            return from;
        }

        private ulong ReadInt(int width)
        {
            AlignTo(width);
            int from = Take(width);
            ulong value = 0;
            for (int index = 0; index < width; index++)
            {
                int shift = endianness == Endianness.Little ? index : width - 1 - index;
                value |= (ulong)bytes[from + index] << (8 * shift);
            }

            return value;
        }

        private ulong ReadRaw(int width, bool little)
        {
            AlignTo(width);
            int from = Take(width);
            ulong value = 0;
            for (int index = 0; index < width; index++)
            {
                int shift = little ? index : width - 1 - index;
                value |= (ulong)bytes[from + index] << (8 * shift);
            }

            return value;
        }

        // -- primitives -------------------------------------------------------

        /// <summary>
        /// <c>boolean</c>. A byte that is neither 0 nor 1 is a stream that
        /// disagrees with its own type, and is refused rather than coerced.
        /// </summary>
        public bool ReadBool()
        {
            if (Stopped)
            {
                return false;
            }

            int where = at;
            byte value = ReadU8();
            if (value == 0)
            {
                return false;
            }

            if (value == 1)
            {
                return true;
            }

            throw new WireException.OutOfRange(where, "a boolean of 0 or 1");
        }

        /// <summary><c>octet</c> and <c>uint8</c>.</summary>
        public byte ReadU8()
        {
            if (Stopped)
            {
                return 0;
            }

            return bytes[Take(1)];
        }

        /// <summary><c>int8</c>.</summary>
        public sbyte ReadI8()
        {
            if (Stopped)
            {
                return 0;
            }

            return (sbyte)bytes[Take(1)];
        }

        /// <summary><c>char</c>, one byte, as a UTF-16 code unit in [0, 255].</summary>
        public char ReadChar()
        {
            if (Stopped)
            {
                return '\0';
            }

            return (char)bytes[Take(1)];
        }

        /// <summary>
        /// <c>wchar</c>, at <paramref name="width"/> octets and in
        /// <paramref name="framingForm"/>'s shape, as a UTF-16 code unit.
        /// </summary>
        /// <remarks>
        /// The four-byte form is RTI Connext's under XCDR1, where the high
        /// sixteen bits are padding around one UTF-16 code unit. A value that
        /// uses them is refused rather than narrowed: narrowing would return a
        /// character the peer never sent.
        ///
        /// <see cref="WcharFraming.GiopLengthPrefixed"/> is CORBA 3.3 part 2
        /// §9.3.1.6's GIOP 1.2 form, which all three ORBs write. The octet count
        /// is checked against the configured code-set width rather than
        /// believed: a stream whose first octet says <c>0xE4</c> — which is what
        /// a bare two-octet <c>wchar</c> looks like read this way — would
        /// otherwise consume 228 octets of whatever followed and return a
        /// character assembled out of the next four members.
        /// </remarks>
        public char ReadWchar(
            WcharWidth width, WcharFraming framingForm, WstringUnitOrder order)
        {
            if (Stopped)
            {
                return '\0';
            }

            int where = at;
            ulong value;
            int octets = WcharWidths.Octets(width);
            if (framingForm == WcharFraming.GiopLengthPrefixed)
            {
                byte declared = ReadU8();
                if (declared != octets)
                {
                    throw new WireException.OutOfRange(
                        where,
                        "a GIOP 1.2 wchar of " + octets
                        + " octets, the configured transmission code set, and not " + declared);
                }

                // A scalar cannot carry a byte-order mark, so `byte-order-mark`
                // means CORBA's default here — big-endian — exactly as
                // `big-endian-always` does.
                bool little = order == WstringUnitOrder.Stream
                    && endianness == Endianness.Little;
                int from = Take(octets);
                value = 0;
                for (int index = 0; index < octets; index++)
                {
                    int shift = little ? index : octets - 1 - index;
                    value |= (ulong)bytes[from + index] << (8 * shift);
                }
            }
            else if (width == WcharWidth.Octets4)
            {
                value = ReadU32();
                if (value > 0xFFFF)
                {
                    throw new WireException.OutOfRange(
                        where, "a four-byte wchar holding one UTF-16 code unit");
                }
            }
            else
            {
                value = ReadU16();
            }

            if (value >= 0xD800 && value <= 0xDFFF)
            {
                throw new WireException.OutOfRange(where, "a Unicode code point");
            }

            return (char)value;
        }

        /// <summary><c>short</c>.</summary>
        public short ReadI16()
        {
            if (Stopped)
            {
                return 0;
            }

            return (short)(ushort)ReadInt(2);
        }

        /// <summary><c>unsigned short</c>.</summary>
        public ushort ReadU16()
        {
            if (Stopped)
            {
                return 0;
            }

            return (ushort)ReadInt(2);
        }

        /// <summary><c>long</c> — an IDL <c>long</c> is 32 bits.</summary>
        public int ReadI32()
        {
            if (Stopped)
            {
                return 0;
            }

            return (int)(uint)ReadInt(4);
        }

        /// <summary><c>unsigned long</c>.</summary>
        public uint ReadU32()
        {
            if (Stopped)
            {
                return 0;
            }

            return (uint)ReadInt(4);
        }

        /// <summary><c>long long</c>.</summary>
        public long ReadI64()
        {
            if (Stopped)
            {
                return 0;
            }

            return (long)ReadInt(8);
        }

        /// <summary><c>unsigned long long</c>, as the number it is.</summary>
        public ulong ReadU64()
        {
            if (Stopped)
            {
                return 0;
            }

            return ReadInt(8);
        }

        /// <summary><c>float</c>.</summary>
        public float ReadF32()
        {
            if (Stopped)
            {
                return 0.0f;
            }

            return BitConverter.ToSingle(BitConverter.GetBytes((uint)ReadInt(4)), 0);
        }

        /// <summary><c>double</c>.</summary>
        public double ReadF64()
        {
            if (Stopped)
            {
                return 0.0;
            }

            return BitConverter.Int64BitsToDouble((long)ReadInt(8));
        }

        /// <summary>
        /// <c>long double</c>: all sixteen bytes, as IEEE 754 binary128.
        /// </summary>
        /// <remarks>
        /// The counterpart of <see cref="Writer.WriteF128"/>. Reading the first
        /// eight bytes as a <c>double</c> and skipping the rest would decode
        /// every value a <c>double</c> cannot hold as a different number and say
        /// nothing.
        /// </remarks>
        public Float128 ReadF128()
        {
            if (Stopped)
            {
                return Float128.Zero;
            }

            AlignTo(8);
            int from = Take(16);
            byte[] sixteen = new byte[16];
            Array.Copy(bytes, from, sixteen, 0, 16);
            return Float128.FromBytes(sixteen, endianness == Endianness.Big);
        }

        /// <summary>A 16-bit value little-endian whatever the stream's order is.</summary>
        public ushort ReadU16Little()
        {
            if (Stopped)
            {
                return 0;
            }

            return (ushort)ReadRaw(2, true);
        }

        /// <summary>A 16-bit value big-endian whatever the stream's order is.</summary>
        public ushort ReadU16Big()
        {
            if (Stopped)
            {
                return 0;
            }

            return (ushort)ReadRaw(2, false);
        }

        /// <summary>A 32-bit value little-endian whatever the stream's order is.</summary>
        public uint ReadU32Little()
        {
            if (Stopped)
            {
                return 0;
            }

            return (uint)ReadRaw(4, true);
        }

        /// <summary>A 32-bit value big-endian whatever the stream's order is.</summary>
        public uint ReadU32Big()
        {
            if (Stopped)
            {
                return 0;
            }

            return (uint)ReadRaw(4, false);
        }

        // -- strings ----------------------------------------------------------

        /// <summary>
        /// A <c>string</c>: a length including the terminator, the bytes, the
        /// NUL.
        /// </summary>
        public string ReadString()
        {
            if (Stopped)
            {
                return string.Empty;
            }

            int where = at;
            uint declared = ReadU32();
            if (declared == 0 || declared > (long)Remaining() + 1L)
            {
                throw new WireException.ImpossibleLength(where, declared, Remaining());
            }

            int from = Take(declared - 1);
            string text = DecodeUtf8(from, (int)(declared - 1), where);
            ReadU8();
            return text;
        }

        /// <summary>
        /// A <c>wstring</c>, in the form <paramref name="policy"/> and
        /// <paramref name="order"/> name.
        /// </summary>
        /// <remarks>
        /// Under <see cref="WstringLengthPolicy.Specification"/> the count is
        /// bytes and not characters — §7.4.1.1.2 — so a count the code-unit
        /// width does not divide is a stream no encoder produced and is refused
        /// rather than rounded down: rounding would leave the remainder in the
        /// stream and put every later member at the wrong offset.
        ///
        /// The form is never inferred from the stream. The three are not
        /// separable: a Fast DDS <c>"ab"</c> followed by a <c>long</c> is
        /// byte-identical to a specification <c>"a"</c> followed by the same
        /// <c>long</c>, differing only in padding no clause fixes. Choosing
        /// between them would mean letting the length field — the one field a
        /// hostile peer controls — decide how much is read.
        /// </remarks>
        public string ReadWstring(WstringLengthPolicy policy, WstringUnitOrder order)
        {
            if (Stopped)
            {
                return string.Empty;
            }

            int where = at;
            uint declared = ReadU32();
            int width = WstringLengthPolicies.UnitWidth(policy);
            long count;
            if (WstringLengthPolicies.CountsCharacters(policy))
            {
                count = declared;
            }
            else
            {
                if (declared % (uint)width != 0)
                {
                    throw new WireException.ImpossibleLength(where, declared, Remaining());
                }

                count = declared / (uint)width;
            }

            int[] units = ReadWstringUnits(where, count, width, order);
            if (WstringLengthPolicies.IsTerminated(policy))
            {
                // The count includes the NUL, so a count of zero is a frame that
                // does not hold its own terminator.
                if (declared < 1 || units.Length == 0)
                {
                    throw new WireException.ImpossibleLength(where, declared, Remaining());
                }

                if (units[units.Length - 1] != 0)
                {
                    throw new WireException.OutOfRange(
                        where, "a NUL character ending the wstring, as its count includes one");
                }

                int[] shorter = new int[units.Length - 1];
                Array.Copy(units, shorter, shorter.Length);
                units = shorter;

                // A NUL inside the text is the narrow string's problem in wide
                // form: every C peer stops at it. Only where a terminator is
                // expected — a cell without one is entitled to carry U+0000 as
                // a character.
                foreach (int unit in units)
                {
                    if (unit == 0)
                    {
                        throw new WireException.OutOfRange(
                            where, "no NUL inside a wstring a C peer would stop at");
                    }
                }
            }

            if (order == WstringUnitOrder.ByteOrderMark)
            {
                units = StripByteOrderMark(units);
            }

            StringBuilder text = new StringBuilder(units.Length);
            int index = 0;
            while (index < units.Length)
            {
                int unit = units[index];
                if (unit >= 0xD800 && unit <= 0xDBFF && index + 1 < units.Length)
                {
                    int low = units[index + 1];
                    if (low >= 0xDC00 && low <= 0xDFFF)
                    {
                        text.Append((char)unit).Append((char)low);
                        index += 2;
                        continue;
                    }
                }

                if (unit >= 0xD800 && unit <= 0xDFFF)
                {
                    throw new WireException.OutOfRange(
                        where, "a UTF-16 sequence with no lone surrogate");
                }

                text.Append((char)unit);
                index++;
            }

            return text.ToString();
        }

        /// <summary>
        /// <paramref name="count"/> <c>wstring</c> characters of
        /// <paramref name="width"/> bytes, as UTF-16 units.
        /// </summary>
        /// <remarks>
        /// The four-byte width is RTI's XCDR1 form, where a character is a code
        /// unit zero-extended. A value above <c>0xFFFF</c> there is not a form
        /// this reader knows how to narrow, so it is refused rather than
        /// truncated — truncating is the defect this whole policy exists
        /// because of.
        /// </remarks>
        private int[] ReadWstringUnits(
            int where, long count, int width, WstringUnitOrder order)
        {
            if (count < 0 || count * width > Remaining())
            {
                throw new WireException.ImpossibleLength(where, count * width, Remaining());
            }

            int[] units = new int[count];
            bool little = WstringUnitOrders.IsFixedLittle(order);
            for (int index = 0; index < units.Length; index++)
            {
                uint unit;
                if (width == 4)
                {
                    if (order == WstringUnitOrder.BigEndianAlways)
                    {
                        unit = ReadU32Big();
                    }
                    else if (little)
                    {
                        unit = ReadU32Little();
                    }
                    else
                    {
                        unit = ReadU32();
                    }

                    if (unit > 0xFFFF)
                    {
                        throw new WireException.OutOfRange(
                            where, "a four-byte wstring character holding one UTF-16 unit");
                    }
                }
                else if (order == WstringUnitOrder.BigEndianAlways)
                {
                    unit = ReadU16Big();
                }
                else if (little)
                {
                    unit = ReadU16Little();
                }
                else
                {
                    unit = ReadU16();
                }

                units[index] = (int)unit;
            }

            return units;
        }

        /// <summary>
        /// Take the byte-order mark off the front and apply what it said.
        /// </summary>
        /// <remarks>
        /// The units arrived little-endian, because that is what omniORB writes
        /// and what the <c>byte-order-mark</c> order therefore reads. Three
        /// cases, and the third is the one that matters:
        /// <list type="bullet">
        /// <item><c>FEFF</c> — the mark is the right way up, the units are as
        /// read.</item>
        /// <item><c>FFFE</c> — the mark is byte-swapped, so every unit behind it
        /// is too, and each one is swapped back. This is what a conforming
        /// big-endian omniORB peer produces and what a reader that ignored the
        /// mark would return as different characters, well formed and the right
        /// length.</item>
        /// <item>no mark — CORBA's rule for a GIOP 1.2 <c>wstring</c> without
        /// one is UTF-16 big-endian, so every unit read little-endian is
        /// swapped.</item>
        /// </list>
        /// The mark is never a character of the returned string in any of the
        /// three, which is the half §7.4.1.1.2 cares about.
        /// </remarks>
        private static int[] StripByteOrderMark(int[] units)
        {
            if (units.Length == 0)
            {
                return units;
            }

            if (units[0] == Writer.ByteOrderMarkUnit)
            {
                int[] rest = new int[units.Length - 1];
                Array.Copy(units, 1, rest, 0, rest.Length);
                return rest;
            }

            int from = units[0] == Writer.SwappedByteOrderMarkUnit ? 1 : 0;
            int[] swapped = new int[units.Length - from];
            for (int index = 0; index < swapped.Length; index++)
            {
                int unit = units[from + index];
                swapped[index] = ((unit & 0xFF) << 8) | (unit >> 8);
            }

            return swapped;
        }

        /// <summary>
        /// <c>fixed&lt;digits, scale&gt;</c>, as the decimal string the writer
        /// took.
        /// </summary>
        /// <remarks>
        /// <para>
        /// The mirror of <see cref="Writer.WriteFixed"/>:
        /// <c>ceil((digits + 1) / 2)</c> bytes of BCD, digits most-significant
        /// first, sign nibble last.
        /// </para>
        /// <para>
        /// The leading zeros go, because the BCD field is <c>digits</c> wide
        /// whatever the value: the nibbles for <c>0.01</c> in a
        /// <c>fixed&lt;5,2&gt;</c> spell <c>00001</c>, and returning
        /// <c>"000.01"</c> would put a decimal on the read side that the write
        /// side would never produce from the same value. A negative zero loses
        /// its sign for the same reason.
        /// </para>
        /// <para>
        /// Under <see cref="FixedDigitCount.ValuePrecision"/> the field's end is
        /// read out of the octets: [CDR] §9.3.2.8 makes a digit <c>0</c> to
        /// <c>9</c> and a sign <c>0xa</c> upwards, and the nibble count is even
        /// by construction, so the field ends at the first octet whose <b>low</b>
        /// nibble is a sign. A field that never presents one within the sixteen
        /// octets of the widest <c>fixed</c> IDL admits is refused.
        /// </para>
        /// </remarks>
        public string ReadFixed(int digits, int scale, FixedDigitCount count)
        {
            if (Stopped)
            {
                return string.Empty;
            }

            // The same bound as the writer's, on this side too: OMG IDL 4.2
            // rule (42) caps the BCD rule at 31 digits, and a reader asked for
            // a wider field would take octets no declaration can name.
            if (digits <= 0 || digits > 31 || scale > digits)
            {
                throw new WireException.OutOfRange(
                    at, "a fixed<P, S> with S <= P and 1 <= P <= 31, not fixed<"
                        + digits + ", " + scale + ">");
            }

            int[] nibbles;
            int used;
            int sign;
            if (count == FixedDigitCount.ValuePrecision
                || count == FixedDigitCount.ValueStringDigits)
            {
                int[] buffer = new int[32];
                used = 0;
                while (true)
                {
                    if (used >= 32)
                    {
                        throw new WireException.OutOfRange(
                            at, "a sign nibble within sixteen octets");
                    }

                    byte octet = ReadU8();
                    buffer[used++] = octet >> 4;
                    buffer[used++] = octet & 0x0F;
                    if ((octet & 0x0F) >= FixedDigitCounts.FirstSignNibble)
                    {
                        break;
                    }
                }

                sign = buffer[--used];
                nibbles = new int[used];
                Array.Copy(buffer, nibbles, used);
            }
            else
            {
                int octets = (digits + 2) / 2;
                int from = Take(octets);
                int[] buffer = new int[octets * 2];
                for (int index = 0; index < octets; index++)
                {
                    buffer[index * 2] = bytes[from + index] >> 4;
                    buffer[(index * 2) + 1] = bytes[from + index] & 0x0F;
                }

                sign = buffer[buffer.Length - 1];
                int start = (digits + 1) % 2 == 1 ? 1 : 0;
                nibbles = new int[buffer.Length - 1 - start];
                Array.Copy(buffer, start, nibbles, 0, nibbles.Length);
            }

            StringBuilder text = new StringBuilder(nibbles.Length);
            foreach (int nibble in nibbles)
            {
                if (nibble > 9)
                {
                    throw new WireException.OutOfRange(at, "BCD digits");
                }

                text.Append((char)('0' + nibble));
            }

            // The point sits `scale` digits from the right and not
            // `digits - scale` from the left: under the value's count the field
            // carries fewer than `digits`, and one that carries fewer than the
            // scale — `1c` under `fixed<5,2>` — needs its integer part written
            // as a zero.
            while (text.Length < scale + 1)
            {
                text.Insert(0, '0');
            }

            int split = text.Length - scale;
            string whole = TrimLeadingZeros(text.ToString(0, split));
            string fraction = text.ToString(split, text.Length - split);
            string result = scale == 0 ? whole : whole + "." + fraction;
            bool zero = whole == "0" && IsAllZeros(fraction);
            return sign == 0x0D && !zero ? "-" + result : result;
        }

        private static string TrimLeadingZeros(string text)
        {
            int first = 0;
            while (first < text.Length && text[first] == '0')
            {
                first++;
            }

            return first == text.Length ? "0" : text.Substring(first);
        }

        private static bool IsAllZeros(string text)
        {
            for (int index = 0; index < text.Length; index++)
            {
                if (text[index] != '0')
                {
                    return false;
                }
            }

            return true;
        }

        // -- the bit-bounded kinds --------------------------------------------

        /// <summary>
        /// An enumerator's holder, sign-extended from its bound.
        /// </summary>
        /// <remarks>
        /// Signed, because XTypes 1.3 §7.4.3.5.3 rule (5) makes the holder
        /// signed: <c>@value(200)</c> under a bound of eight goes out as
        /// <c>c8</c>, and that byte read as the type says is <c>-56</c>.
        /// <c>@value(-1)</c> and <c>@value(255)</c> are the same byte and
        /// nothing in the stream separates them — which one was meant is a
        /// question about the <i>type</i>, and the generated decoder answers it
        /// by comparing this against each declared enumerator's own pattern,
        /// taken the same way.
        /// </remarks>
        public long ReadEnum(int bitBound)
        {
            if (Stopped)
            {
                return 0;
            }

            if (bitBound <= 0 || bitBound > 32)
            {
                throw new WireException.OutOfRange(
                    at, "an @bit_bound between 1 and 32, not " + bitBound);
            }

            // The holder's width is the bound rounded up to 8, 16 or 32, and the
            // sign bit is the top one of *that* width and not of the bound:
            // `@bit_bound(5)` still occupies an octet, and rule (5) makes the
            // octet signed.
            int width = bitBound <= 8 ? 8 : bitBound <= 16 ? 16 : 32;
            ulong pattern = width == 8 ? ReadU8() : width == 16 ? ReadU16() : ReadU32();
            ulong sign = 1UL << (width - 1);
            return (pattern & sign) == 0 ? (long)pattern : (long)pattern - (1L << width);
        }

        /// <summary>A bitmask's holder. Table 33 of XTypes 1.3 §7.4.1.1.3.2.</summary>
        public ulong ReadBitmask(int bitBound)
        {
            if (Stopped)
            {
                return 0;
            }

            if (bitBound <= 0 || bitBound > 64)
            {
                throw new WireException.OutOfRange(
                    at, "an @bit_bound between 1 and 64, not " + bitBound);
            }

            if (bitBound <= 8)
            {
                return ReadU8();
            }

            if (bitBound <= 16)
            {
                return ReadU16();
            }

            if (bitBound <= 32)
            {
                return ReadU32();
            }

            return ReadU64();
        }

        /// <summary>A bitset's anchor integer, of the width the planner sized.</summary>
        public ulong ReadBitset(int anchorBytes)
        {
            if (Stopped)
            {
                return 0;
            }

            switch (anchorBytes)
            {
                case 1: return ReadU8();
                case 2: return ReadU16();
                case 4: return ReadU32();
                case 8: return ReadU64();
                default:
                    throw new WireException.OutOfRange(
                        at, "a bitset anchor of 1, 2, 4 or 8 bytes, not " + anchorBytes);
            }
        }

        // -- framing ----------------------------------------------------------

        /// <summary>
        /// A sequence's element count, refusing one the stream cannot hold.
        /// </summary>
        /// <remarks>
        /// A count is one element or more, so a stream with fewer bytes left
        /// than elements claimed cannot be honest about it. Allocating for it is
        /// how a reader turns a bad message into an out-of-memory.
        /// </remarks>
        public uint ReadSequenceLength()
        {
            if (Stopped)
            {
                return 0;
            }

            int where = at;
            uint count = ReadU32();
            if (count > Remaining())
            {
                throw new WireException.ImpossibleLength(where, count, Remaining());
            }

            return count;
        }

        /// <summary>A DHEADER: the length the members that follow occupy.</summary>
        public uint ReadDheader()
        {
            if (Stopped)
            {
                return 0;
            }

            int where = at;
            uint declared = ReadU32();
            if (declared > Remaining())
            {
                throw new WireException.ImpossibleLength(where, declared, Remaining());
            }

            return declared;
        }

        /// <summary>
        /// An EMHEADER, unpacked into its parts and the NEXTINT it carries.
        /// </summary>
        /// <remarks>
        /// Codes 5 to 7 report the NEXTINT they read, but the reader is put back
        /// before it: that word is the member's own leading length and the
        /// member's decoder has to see it. XTypes 1.3 §7.4.3.4.2.
        /// </remarks>
        public EmHeaderRead ReadEmheader()
        {
            if (Stopped)
            {
                return default;
            }

            uint word = ReadU32();
            int lengthCode = (int)((word >> 28) & 0x7);
            uint memberId = word & 0x0FFFFFFFU;
            bool mustUnderstand = (word >> 31) != 0;
            if (lengthCode == 4)
            {
                return new EmHeaderRead(memberId, lengthCode, mustUnderstand, ReadU32(), true);
            }

            if (lengthCode >= 5)
            {
                int where = at;
                uint peeked = ReadU32();
                at = where;
                return new EmHeaderRead(memberId, lengthCode, mustUnderstand, peeked, true);
            }

            return new EmHeaderRead(memberId, lengthCode, mustUnderstand, 0, false);
        }

        /// <summary>A PL_CDR1 parameter header in the <see cref="ParameterRole.Member"/> role.</summary>
        public PidRead ReadPid()
        {
            return ReadPidAs(ParameterRole.Member);
        }

        /// <summary>
        /// A PL_CDR1 parameter header, in whichever form it was written.
        /// </summary>
        /// <remarks>
        /// <paramref name="role"/> is the caller's, not the stream's: the bytes
        /// are the same bytes and only what the declared length means differs.
        /// §7.4.1.2.
        /// </remarks>
        public PidRead ReadPidAs(ParameterRole role)
        {
            if (Stopped)
            {
                return new PidRead(0, 0, true, false, false, false, false, role, origin, at);
            }

            AlignTo(4);
            ushort pid = ReadU16();
            ushort declared = ReadU16();
            if ((pid & PidIdMask) == (Writer.PidExtended & PidIdMask))
            {
                // The four flag bits are not part of the id: §7.4.1.2.1 splits
                // the `eMemberHeader` into `FLAG_1` to `FLAG_4` and a 28-bit
                // member id, and `FLAG_2` is the member's must-understand.
                // Taking the whole word gave a Cyclone member declared
                // `@must_understand` an id of `0x40000001`, which matches
                // nothing, and the member was dropped.
                uint memberHeader = ReadU32();
                uint wideLength = ReadU32();

                // The parameter's value is its own stream block: §7.4.1.2 counts
                // a member's internal alignment from the first octet after the
                // header, not from the start of the message. `FinishPidRead`
                // puts the enclosing origin back.
                int savedExtended = origin;
                origin = at;
                return new PidRead(
                    memberHeader & Writer.EMemberIdMask,
                    wideLength,
                    false,
                    (memberHeader & Writer.EFlagMustUnderstand) != 0,

                    // FLAG_1 is FLAG_IMPL_EXTENSION here exactly as bit 15 is in
                    // the short header. The extended id is 28 bits and has no
                    // reserved range.
                    (memberHeader & Writer.EFlagImplExtension) != 0,
                    false,
                    false,
                    role,
                    savedExtended,
                    at);
            }

            uint identifier = (uint)(pid & PidIdMask);
            bool end = identifier == (Writer.PidListEnd & PidIdMask);

            // Under a framing that puts `FLAG_IMPL_EXTENSION` on every member id
            // the bit no longer tells a vendor-private parameter from a user
            // member — OpenDDS writes `c0010004` for `@id(1) long a`. It is
            // reported absent there because under that framing it carries no
            // information, and reporting it would send every member down the
            // unknown-parameter path.
            bool flagged = (pid & Writer.PidImplExtension) != 0
                && (PidFlagPolicies.FlagsAt(pidFlags, WirePosition.Member)
                    & Writer.PidImplExtension) == 0;
            int saved = origin;
            origin = at;
            return new PidRead(
                identifier,
                declared,
                end,
                (pid & Writer.PidMustUnderstand) != 0,
                flagged,
                !end && identifier > (uint)highestShortId,
                identifier == Writer.PidIgnore,
                role,
                saved,
                at);
        }

        /// <summary>
        /// Whether this parameter can be one of the reading type's members.
        /// </summary>
        /// <remarks>
        /// False for a vendor-private parameter and for a reserved id; both are
        /// legal for a peer to send and neither is a member, so both take the
        /// unknown-parameter path. §7.4.1.2.2, §7.2.2.4.4.4.6.
        /// </remarks>
        public static bool PidIsUserMember(PidRead header)
        {
            return header.IsUserMember;
        }

        /// <summary>
        /// End a parameter's value and step to the next header.
        /// </summary>
        /// <remarks>
        /// The padding to four follows the value and is outside the declared
        /// length, so the next parameter is found by alignment and not by adding
        /// the length — which is what lets one reader take RTI's rounded-up
        /// length and Fast DDS's exact one.
        ///
        /// The declared length is checked against the configured framing rather
        /// than believed: a length belonging to the other convention is a stream
        /// this reader was pointed at with the wrong framing, and reading on
        /// would put every later member at an offset no writer produced.
        /// </remarks>
        public void FinishPidRead(PidRead header)
        {
            origin = header.SavedOrigin;
            if (Stopped || header.End)
            {
                return;
            }

            long used = at - header.ValueStart;
            long declared = header.Length;
            long padded = (used + 3L) & ~3L;
            bool pads = framing.PadsDeclaredLength(header.Role);

            // Both readings of the same value are accepted where they coincide,
            // which is every value already a multiple of four.
            if (declared != (pads ? padded : used) && declared != used && declared != padded)
            {
                ReaderRefuse(Status.OutOfRange);
            }

            AlignTo(4);
        }

        /// <summary>
        /// Step over a member this reader does not know.
        /// </summary>
        /// <remarks>
        /// The whole point of the length code. XTypes 1.3 §7.4.3.4.2: codes 0 to
        /// 3 state a size of one, two, four or eight bytes outright; code 4 is
        /// followed by a NEXTINT holding the member's length; and codes 5, 6 and
        /// 7 have a NEXTINT that is <i>also</i> the member's own leading word, so
        /// the member spans that word plus the length it states, times one, four
        /// or eight.
        /// </remarks>
        public void SkipMember(EmHeaderRead header)
        {
            if (Stopped)
            {
                return;
            }

            if (header.LengthCode <= 3)
            {
                Skip(1L << header.LengthCode);
                return;
            }

            if (!header.HasNextInt)
            {
                throw new WireException.OutOfRange(
                    at, "a NEXTINT for a length code of 4 or above");
            }

            if (header.LengthCode == 4)
            {
                Skip(header.NextInt);
                return;
            }

            long scale = header.LengthCode == 5 ? 1L : header.LengthCode == 6 ? 4L : 8L;
            Skip(4L + ((long)header.NextInt * scale));
        }

        /// <summary>
        /// Start reading a parameter's value, returning the origin it replaces.
        /// The reader's half of the writer's parameter block.
        /// </summary>
        public int BeginParameterValue()
        {
            int saved = origin;
            origin = at;
            return saved;
        }

        /// <summary>End a parameter's value and step over its trailing padding.</summary>
        public void FinishParameterValue(int savedOrigin)
        {
            origin = savedOrigin;
            AlignTo(4);
        }

        // -- text -------------------------------------------------------------

        private string DecodeUtf8(int from, int count, int where)
        {
            System.Text.Encoding strict = new UTF8Encoding(
                encoderShouldEmitUTF8Identifier: false, throwOnInvalidBytes: true);
            try
            {
                return strict.GetString(bytes, from, count);
            }
            catch (DecoderFallbackException)
            {
                throw new WireException.NotUtf8(where);
            }
        }
    }
}
