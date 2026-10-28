// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
using System;
using System.Globalization;
using System.Text;
using ZeroDds.Idl;

namespace ZeroDds.Wire
{
    /// <summary>
    /// Putting bytes on the wire.
    /// </summary>
    /// <remarks>
    /// <para>
    /// The C# counterpart of <c>crates/wire/src/writer.rs</c>,
    /// <c>runtime/c/src/writer.c</c>, <c>runtime/cpp</c>,
    /// <c>runtime/java/.../Writer.java</c> and
    /// <c>runtime/python/zerodds_wire/writer.py</c> — a sixth implementation of
    /// the same clauses, not a binding to any of them. Where two of the six
    /// disagree, one of them has misread DDS-XTypes 1.3, and that is the point
    /// of having six.
    /// </para>
    /// <para>
    /// Alignment is relative to the start of the stream, not to the start of
    /// the value: XCDR aligns each primitive to its own size counted from the
    /// encapsulation, which is why a <c>long</c> after a <c>char</c> sits at 4
    /// and not at 1. XTypes 1.3 §7.4.1.1.
    /// </para>
    /// <h2>What C# makes easy, and where that stops</h2>
    /// <para>
    /// <b>The unsigned types are real.</b> This is the whole difference from
    /// the Java column, which has to take a <c>long</c> for an
    /// <c>unsigned long</c> and range-check it because an <c>int</c> parameter
    /// would make <c>3735928559</c> arrive as <c>-559038737</c> with the loss
    /// already done. Here <see cref="WriteU32(uint)"/> takes a <c>uint</c>, the
    /// compiler refuses the value that does not fit, and there is no runtime
    /// check to get wrong. <see cref="WriteU64(ulong)"/> is a <i>number</i> and
    /// not a pattern for the same reason.
    /// </para>
    /// <para>
    /// <b>A <c>string</c> is UTF-16 and admits a lone surrogate.</b>
    /// <c>Encoding.UTF8.GetBytes</c> replaces one with U+FFFD and reports
    /// success — a different character on the wire, silently. This class
    /// encodes through an encoder built with
    /// <c>EncoderFallback.ExceptionFallback</c> and raises
    /// <see cref="WireException.NotUtf8"/> instead.
    /// </para>
    /// <para>
    /// <b><c>string.Length</c> is neither bytes nor characters.</b> It is
    /// UTF-16 code units, so it is the length of neither a <c>string</c> nor a
    /// <c>wstring</c> on the wire. A <c>string</c>'s length counts octets of
    /// UTF-8 and includes the NUL; a <c>wstring</c>'s counts, under the
    /// specification policy, the octets its code units occupy, so <c>"𝄞"</c> is
    /// a length of four.
    /// </para>
    /// <para>
    /// <b>A <c>float</c> field is rounded by the caller.</b>
    /// <see cref="WriteF32(float)"/> takes a <c>float</c> and not a
    /// <c>double</c>, where the Java column takes the wider type and narrows
    /// once itself. C# has no implicit <c>double</c> to <c>float</c>
    /// conversion, so a caller that has a <c>double</c> writes the cast and the
    /// narrowing is visible in the generated source rather than hidden in a
    /// parameter type.
    /// </para>
    /// <para>
    /// This class makes no decision about a type. Which members are written, in
    /// what order, with which header and which length code is the layout plan's
    /// answer and arrives here as arguments.
    /// </para>
    /// </remarks>
    public sealed class Writer
    {
        /// <summary>The parameter id that introduces the extended form. §7.4.1.2.2.</summary>
        public const int PidExtended = 0x3F01;

        /// <summary>The parameter id that ends a parameter list. §7.4.1.2.4.</summary>
        public const int PidListEnd = 0x3F02;

        /// <summary>
        /// The highest member id a short parameter header can carry. §7.4.1.2.2
        /// gives the range as 0 to <c>0x3F00</c> <i>inclusive</i>, and
        /// <see cref="PidExtended"/> is one above it. RTI Connext and Fast DDS
        /// both write the short form for <c>0x3F00</c> (case <c>pid_3f00</c>,
        /// 2026-08-10).
        /// </summary>
        public const int PidShortMaxId = 0x3F00;

        /// <summary>
        /// <c>FLAG_IMPL_EXTENSION</c>, bit 15 of a short parameter id.
        /// §7.4.1.2.2: a parameter that sets it is the writing
        /// implementation's own, and user-defined members "will never set the
        /// FLAG_IMPL_EXTENSION bit".
        /// </summary>
        public const int PidImplExtension = 0x8000;

        /// <summary>
        /// <c>PID_IGNORE</c>. §7.4.1.2.2: "All consumers of this Data
        /// Representation shall ignore parameters with this ID" — including a
        /// consumer that would otherwise discard the sample over the
        /// must-understand bit.
        /// </summary>
        public const int PidIgnore = 0x3F03;

        /// <summary>
        /// The longest value a short parameter header can state the length of.
        /// §7.4.1.2.2 gives <c>ssize</c> sixteen bits; <see cref="FinishPid"/>
        /// rewrites the header rather than truncating a longer length into two
        /// bytes.
        /// </summary>
        public const int PidShortMaxLen = 0xFFFF;

        /// <summary>The must-understand bit of a parameter id. DDSI-RTPS 2.5 §9.4.2.11.2.</summary>
        public const int PidMustUnderstand = 0x4000;

        /// <summary>
        /// <c>FLAG_2</c> of the extended <c>eMemberHeader</c>, which is the
        /// <b>member's</b> must-understand. §7.4.1.2.1.
        /// </summary>
        public const uint EFlagMustUnderstand = 0x40000000;

        /// <summary>
        /// <c>FLAG_1</c> of the extended <c>eMemberHeader</c>, which is
        /// <c>FLAG_IMPL_EXTENSION</c> — the same property as
        /// <see cref="PidImplExtension"/> in the short form. §7.4.1.2.1.
        /// </summary>
        public const uint EFlagImplExtension = 0x80000000;

        /// <summary>The mask of the extended header's 28-bit member id. §7.4.1.2.1.</summary>
        public const uint EMemberIdMask = 0x0FFFFFFF;

        /// <summary>A KeyHash is sixteen bytes. DDSI-RTPS 2.5 §9.6.3.8.</summary>
        public const int KeyHashLen = 16;

        /// <summary>The UTF-16 byte-order mark.</summary>
        internal const int ByteOrderMarkUnit = 0xFEFF;

        /// <summary>
        /// The byte-swapped byte-order mark. <c>U+FFFE</c> is not a character:
        /// a mark that reads as one says the units behind it are the other way
        /// round from the order they were read in.
        /// </summary>
        internal const int SwappedByteOrderMarkUnit = 0xFFFE;

        private byte[] outBytes;
        private int length;
        private readonly Endianness endianness;

        /// <summary>
        /// Where alignment is counted from. Zero for a sample; a PL_CDR1
        /// parameter value moves it, because a parameter is its own
        /// encapsulated block. XTypes 1.3 §7.4.1.2.
        /// </summary>
        private int origin;

        private readonly int maxAlign;
        private readonly PlCdr1Framing plCdr1;
        private readonly bool shortLengthCodes;

        /// <summary>
        /// A writer in <paramref name="order"/> whose alignment is capped at
        /// <paramref name="maxAlignment"/>.
        /// </summary>
        /// <remarks>
        /// Four for XCDR2, eight for XCDR1. XTypes 1.3 §7.4.1: <c>ALIGN(N)</c>
        /// aligns to <c>MIN(N, XCDR.maxalign)</c>. A writer that did not know
        /// this padded an eight-byte primitive to eight under XCDR2 and put
        /// four bytes on the wire that no peer expects.
        /// </remarks>
        /// <param name="order">Which end of a multi-byte value goes out first.</param>
        /// <param name="maxAlignment">The encoding's MAXALIGN.</param>
        /// <param name="framing">
        /// How a PL_CDR1 parameter list is framed; <c>null</c> for
        /// <see cref="PlCdr1Framing.Defaults"/>.
        /// </param>
        /// <param name="shortCodes">
        /// Whether an LC4 header whose member turns out to be 1, 2, 4 or 8
        /// bytes is rewritten with a short length code. Set it from
        /// <c>LengthCodePolicy::short_code_where_it_fits</c>.
        /// </param>
        public Writer(
            Endianness order,
            int maxAlignment,
            PlCdr1Framing framing,
            bool shortCodes)
        {
            outBytes = new byte[64];
            length = 0;
            endianness = order;
            origin = 0;
            maxAlign = Math.Max(1, maxAlignment);
            plCdr1 = framing ?? PlCdr1Framing.Defaults();
            shortLengthCodes = shortCodes;
        }

        /// <summary>
        /// A writer in <paramref name="order"/> with XCDR1's MAXALIGN of eight,
        /// the clause's PL_CDR1 framing, and no short-code rewriting.
        /// </summary>
        /// <remarks>
        /// Eight and not four, for the same reason the Python column's default
        /// is eight: it is the value that pads nothing away, so a caller that
        /// has not thought about the cap gets a stream that is wrong under
        /// XCDR2 in a way a reader notices, rather than one that silently drops
        /// padding an XCDR1 peer expects.
        /// </remarks>
        public Writer(Endianness order)
            : this(order, 8, null, false)
        {
        }

        /// <summary>A big-endian writer, which is what a KeyHash holder is.</summary>
        public static Writer BigEndian()
        {
            return new Writer(Endianness.Big);
        }

        /// <summary>A little-endian writer.</summary>
        public static Writer LittleEndian()
        {
            return new Writer(Endianness.Little);
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

        /// <summary>How a PL_CDR1 parameter list is framed here.</summary>
        public PlCdr1Framing Framing
        {
            get { return plCdr1; }
        }

        /// <summary>Whether an LC4 header is shortened when its member fits a code.</summary>
        public bool ShortLengthCodes
        {
            get { return shortLengthCodes; }
        }

        /// <summary>How many bytes have been written.</summary>
        public int Position()
        {
            return length;
        }

        /// <summary>The bytes, as a copy the caller owns.</summary>
        public byte[] Finish()
        {
            byte[] copy = new byte[length];
            Array.Copy(outBytes, copy, length);
            return copy;
        }

        // -- alignment --------------------------------------------------------

        /// <summary>
        /// Pad to <paramref name="align"/>, counted from the stream's origin and
        /// capped at MAXALIGN.
        /// </summary>
        /// <remarks>
        /// Both halves matter. The cap is the encoding's, so an eight-byte
        /// primitive aligns to four under XCDR2. The origin is where a PL_CDR1
        /// parameter's block began and zero everywhere else — never the value's
        /// own start, and never zero when a parameter is open.
        /// </remarks>
        public void AlignTo(int align)
        {
            int effective = Math.Min(align, maxAlign);
            if (effective <= 1)
            {
                return;
            }

            int offset = length - origin;
            int padding = (effective - Modulo(offset, effective)) % effective;
            if (padding != 0)
            {
                Need(padding);
                Array.Clear(outBytes, length, padding);
                length += padding;
            }
        }

        /// <summary>
        /// A non-negative remainder, which C#'s <c>%</c> is not for a negative
        /// left operand.
        /// </summary>
        private static int Modulo(int value, int by)
        {
            int rest = value % by;
            return rest < 0 ? rest + by : rest;
        }

        // -- packed collection elements ---------------------------------------

        /// <summary>Move the alignment origin to the current position.</summary>
        /// <remarks>
        /// <c>ElementPadding::Packed</c> is HDDS's measured departure from
        /// XTypes 1.3 §7.4.1, which counts every element's alignment from the
        /// encapsulation: each collection element and each aggregate value
        /// counts its own alignment from its own first octet instead.
        /// <c>array-1d-struct-final</c> is <c>01000000 0200 03000000 0400
        /// 77</c>, the second <c>Cell</c>'s <c>long</c> at offset six where the
        /// clause puts it at eight, and <c>typedef-of-struct</c> is the same
        /// inside two <c>@final</c> members.
        /// </remarks>
        /// <returns>The origin displaced, for <see cref="FinishPackedElement"/>.</returns>
        public int BeginPackedElement()
        {
            int saved = origin;
            origin = length;
            return saved;
        }

        /// <summary>Restore the origin <see cref="BeginPackedElement"/> displaced.</summary>
        public void FinishPackedElement(int saved)
        {
            origin = saved;
        }

        // -- primitives -------------------------------------------------------

        /// <summary><c>boolean</c>, one byte, 0 or 1.</summary>
        public void WriteBool(bool value)
        {
            Append((byte)(value ? 1 : 0));
        }

        /// <summary><c>octet</c> and <c>uint8</c>.</summary>
        public void WriteU8(byte value)
        {
            Append(value);
        }

        /// <summary><c>int8</c>.</summary>
        public void WriteI8(sbyte value)
        {
            Append((byte)value);
        }

        /// <summary>
        /// <c>char</c>, one byte, from a UTF-16 code unit.
        /// </summary>
        /// <remarks>
        /// A code unit above 255 is refused rather than truncated:
        /// §7.2.2.2.1.2.2 says a <c>CHAR_8_TYPE</c> "shall be representable
        /// using 8 bits", and a <c>char</c> field holding <c>'ä'</c> in a UTF-8
        /// source file is a mistake worth stopping on — <c>0xe4</c> is a
        /// different character.
        /// </remarks>
        public void WriteChar(char value)
        {
            if (value > 0xFF)
            {
                throw new WireException.OutOfRange(
                    length, "a character below U+0100, not U+" + Hex(value));
            }

            Append((byte)value);
        }

        /// <summary>
        /// <c>wchar</c>, at <paramref name="width"/> octets and in the shape
        /// <paramref name="framing"/> gives it.
        /// </summary>
        /// <remarks>
        /// Two octets under XTypes 1.3 Table 31 and for seven of the nine
        /// measured implementations; four under RTI Connext's XCDR1, under TAO,
        /// and under <c>rosidl_typesupport_fastrtps_c</c>. The width is an
        /// argument of its own and not a reading of the <c>wstring</c> policy,
        /// because <c>rmw_cyclonedds_cpp</c> writes two here and four inside a
        /// <c>wstring</c>.
        ///
        /// The Basic-Multilingual-Plane restriction of §7.2.2.2.1.2.5 is a
        /// property of the type and not of the width: the four-byte form
        /// zero-extends a code unit, it does not carry a code point. A C#
        /// <c>char</c> is one UTF-16 code unit, so it cannot hold a code point
        /// outside the BMP and there is nothing to check here that the type has
        /// not already refused — except a lone surrogate, which
        /// <see cref="WriteWstringUtf8"/> refuses and a scalar cannot be.
        /// </remarks>
        public void WriteWchar(
            char value, WcharWidth width, WcharFraming framing, WstringUnitOrder order)
        {
            if (framing == WcharFraming.GiopLengthPrefixed)
            {
                // CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2 form: the octet count
                // first, then the code unit's octets, all aligned to one. The
                // count is the transmission code set's width and not the
                // value's, so it is the same octet for every character.
                // Measured 2026-08-10: omniORB writes `02 00e4` for U+00E4 in
                // either stream order and TAO `04 e4000000`.
                int octets = WcharWidths.Octets(width);
                Append((byte)octets);
                bool little = order == WstringUnitOrder.Stream
                    && endianness == Endianness.Little;
                AppendRaw(value, octets, little);
                return;
            }

            if (width == WcharWidth.Octets4)
            {
                WriteU32(value);
            }
            else
            {
                WriteU16(value);
            }
        }

        /// <summary><c>short</c>.</summary>
        public void WriteI16(short value)
        {
            WriteInt((ulong)(ushort)value, 2);
        }

        /// <summary><c>unsigned short</c>.</summary>
        public void WriteU16(ushort value)
        {
            WriteInt(value, 2);
        }

        /// <summary><c>long</c> — an IDL <c>long</c> is 32 bits, so a C# <c>int</c>.</summary>
        public void WriteI32(int value)
        {
            WriteInt((ulong)(uint)value, 4);
        }

        /// <summary><c>unsigned long</c>.</summary>
        public void WriteU32(uint value)
        {
            WriteInt(value, 4);
        }

        /// <summary><c>long long</c> — 64 bits, so a C# <c>long</c>.</summary>
        public void WriteI64(long value)
        {
            WriteInt((ulong)value, 8);
        }

        /// <summary>
        /// <c>unsigned long long</c>, as a <i>number</i>.
        /// </summary>
        /// <remarks>
        /// Where the Java column has to document this parameter as a bit
        /// pattern because Java has no unsigned integer,
        /// <c>18446744073709551615</c> is simply a <c>ulong</c> here and every
        /// value of the type is admissible with nothing to check.
        /// </remarks>
        public void WriteU64(ulong value)
        {
            WriteInt(value, 8);
        }

        /// <summary>
        /// The Cyclone union overlay: the low <c>octets</c> of
        /// <c>pattern</c>, written as one unsigned integer of that width in
        /// the stream's byte order.
        /// </summary>
        /// <remarks>
        /// A discriminator that carries a label the <c>union-labels</c>
        /// dialect axis says selects nothing makes the peer fall through to
        /// <c>default:</c> and serialise its one C union storage <i>as that
        /// arm</i> — the selected member's octets, reinterpreted.
        /// <c>pattern</c> is the <c>ulong</c> whose little-endian bytes are
        /// the selected member's little-endian storage octets, zero-extended;
        /// the <c>BitPattern*</c> statics produce it.
        /// </remarks>
        public void WriteUnionOverlay(ulong pattern, int octets)
        {
            switch (octets)
            {
                case 1:
                    WriteU8((byte)(pattern & 0xFF));
                    return;
                case 2:
                    WriteU16((ushort)(pattern & 0xFFFF));
                    return;
                case 4:
                    WriteU32((uint)(pattern & 0xFFFFFFFF));
                    return;
                case 8:
                    WriteU64(pattern);
                    return;
                default:
                    throw new WireException.OutOfRange(
                        length, "a union arm of 1, 2, 4 or 8 octets, not " + octets);
            }
        }

        /// <summary>
        /// The low <c>octets</c> of <c>value</c>'s two's-complement bits,
        /// zero-extended — <see cref="WriteUnionOverlay(ulong, int)"/>'s
        /// pattern for an integer, character, enumeration or bitmask arm.
        /// The mask is the method: a plain widening of a negative value
        /// sign-extends, and <c>-2</c> in four octets has to be
        /// <c>0x00000000FFFFFFFE</c>.
        /// </summary>
        public static ulong BitPatternInt(long value, int octets)
        {
            ulong pattern = (ulong)value;
            return octets >= 8 ? pattern : pattern & ((1UL << (octets * 8)) - 1);
        }

        /// <summary>One octet, 0 or 1 — the boolean overlay pattern.</summary>
        public static ulong BitPatternBool(bool value)
        {
            return value ? 1UL : 0UL;
        }

        /// <summary>The IEEE 754 binary32 bits, zero-extended.</summary>
        public static ulong BitPatternF32(float value)
        {
            return BitConverter.ToUInt32(BitConverter.GetBytes(value), 0);
        }

        /// <summary>The IEEE 754 binary64 bits.</summary>
        public static ulong BitPatternF64(double value)
        {
            return (ulong)BitConverter.DoubleToInt64Bits(value);
        }

        /// <summary><c>float</c>, binary32.</summary>
        public void WriteF32(float value)
        {
            WriteInt(BitConverter.ToUInt32(BitConverter.GetBytes(value), 0), 4);
        }

        /// <summary><c>double</c>, binary64.</summary>
        public void WriteF64(double value)
        {
            WriteInt((ulong)BitConverter.DoubleToInt64Bits(value), 8);
        }

        /// <summary>
        /// <c>long double</c>: sixteen bytes of IEEE 754 binary128.
        /// </summary>
        /// <remarks>
        /// CORBA 3.3 part 2 §9.3.1.3 with Figure 9.2 gives the layout and shows
        /// the two byte orders as the whole sixteen bytes reversed, not two
        /// eight-byte halves swapped in place. XTypes 1.3 Table 31 gives the
        /// alignment: eight, capped to four by MAXALIGN under XCDR2, which
        /// <see cref="AlignTo"/> already does.
        ///
        /// The value is a <see cref="Float128"/> and never a <c>double</c>
        /// followed by eight zero bytes, and never a <c>decimal</c>: the first
        /// is the right length and the wrong value, and the second is a
        /// base-ten type of 28 digits where the corpus carries 30.
        /// </remarks>
        public void WriteF128(Float128 value)
        {
            AlignTo(8);
            byte[] bytes = value.ToBytes(endianness == Endianness.Big);
            Need(16);
            Array.Copy(bytes, 0, outBytes, length, 16);
            length += 16;
        }

        /// <summary>
        /// A 16-bit value little-endian whatever the stream's order is.
        /// </summary>
        /// <remarks>
        /// Two callers: the code units of the <c>host-unswapped</c>
        /// <c>wstring</c> form, and the <c>byte-order-mark</c> unit order.
        /// Aligned to two like any other 16-bit value.
        /// </remarks>
        public void WriteU16Little(ushort value)
        {
            AlignTo(2);
            AppendRaw(value, 2, true);
        }

        /// <summary>
        /// A 16-bit value big-endian whatever the stream's order is.
        /// </summary>
        /// <remarks>
        /// The other side of the same axis: what omniORB and JacORB decode a
        /// <c>wstring</c>'s code units as, whatever the stream says.
        /// </remarks>
        public void WriteU16Big(ushort value)
        {
            AlignTo(2);
            AppendRaw(value, 2, false);
        }

        /// <summary>A 32-bit value little-endian whatever the stream's order is.</summary>
        public void WriteU32Little(uint value)
        {
            AlignTo(4);
            AppendRaw(value, 4, true);
        }

        /// <summary>A 32-bit value big-endian whatever the stream's order is.</summary>
        public void WriteU32Big(uint value)
        {
            AlignTo(4);
            AppendRaw(value, 4, false);
        }

        // -- strings ----------------------------------------------------------

        /// <summary>
        /// A <c>string</c>: a four-byte length including the terminator, the
        /// bytes, then a NUL. XTypes 1.3 §7.4.1.1.
        /// </summary>
        /// <remarks>
        /// The length counts the terminator, which is the one place XCDR
        /// differs from every other length-prefixed encoding. It counts
        /// <b>bytes</b> of UTF-8: <c>value.Length</c> is UTF-16 code units and
        /// would be right only for an ASCII sample, and every corpus string is
        /// deliberately not one.
        /// </remarks>
        public void WriteString(string value)
        {
            byte[] encoded = Utf8(value ?? string.Empty);
            WriteU32((uint)(encoded.Length + 1));
            AppendAll(encoded);
            Append(0);
        }

        /// <summary>
        /// A bounded <c>string</c>.
        /// </summary>
        /// <remarks>
        /// The same bytes as an unbounded one: the bound is a promise about the
        /// source, not a difference on the wire — so this is the only place it
        /// can be enforced. An over-long value is refused rather than cut at a
        /// character boundary: XTypes 1.3 §7.2.2.7's TRIM is a <i>read</i>-side
        /// behaviour and belongs to the receiver's own bound, and a writer that
        /// cut the value would put a sample on the wire nobody wrote.
        /// </remarks>
        /// <param name="value">The text.</param>
        /// <param name="bound">
        /// The bound in <b>octets</b>, which is what a <c>string&lt;N&gt;</c>
        /// bound counts for every measured implementation.
        /// </param>
        /// <exception cref="WireException.OutOfRange">
        /// The value is longer than <paramref name="bound"/>.
        /// </exception>
        public void WriteStringBounded(string value, int bound)
        {
            byte[] encoded = Utf8(value ?? string.Empty);
            if (encoded.Length > bound)
            {
                throw new WireException.OutOfRange(
                    length,
                    $"at most {bound} octets in a bounded string, and this one has "
                    + $"{encoded.Length}");
            }

            WriteU32((uint)(encoded.Length + 1));
            AppendAll(encoded);
            Append(0);
        }

        /// <summary>A bounded <c>wstring</c>.</summary>
        /// <remarks>
        /// As <see cref="WriteStringBounded"/>, counted the way
        /// <see cref="WriteWstringUtf8"/>'s own remarks give: UTF-16 code
        /// units, not characters and not octets, whatever the length policy
        /// puts on the wire.
        /// </remarks>
        /// <param name="value">The text.</param>
        /// <param name="bound">The bound in UTF-16 code units.</param>
        /// <param name="policy">What the length field counts.</param>
        /// <param name="order">Which order the code units go in.</param>
        /// <exception cref="WireException.OutOfRange">
        /// The value holds more than <paramref name="bound"/> UTF-16 code
        /// units.
        /// </exception>
        public void WriteWstringBounded(
            string value, int bound, WstringLengthPolicy policy, WstringUnitOrder order)
        {
            string text = value ?? string.Empty;
            int units = Utf16Units(text).Length;
            if (units > bound)
            {
                throw new WireException.OutOfRange(
                    length,
                    $"at most {bound} UTF-16 code units in a bounded wstring, and this "
                    + $"one has {units}");
            }

            WriteWstringUtf8(text, policy, order);
        }

        /// <summary>
        /// The count check every bounded <c>sequence</c> makes before a byte of
        /// it is written.
        /// </summary>
        /// <remarks>
        /// Table 17 makes <c>O2.length &lt;= T1.length</c> the condition for
        /// constructing a <c>sequence&lt;T, N&gt;</c>, and the wire carries no
        /// bound to compare against: a sender whose count already exceeds its
        /// own declaration has written a value of a type nobody declared.
        /// </remarks>
        /// <param name="actual">How many elements the value holds.</param>
        /// <param name="bound">The declared bound.</param>
        /// <exception cref="WireException.OutOfRange">
        /// <paramref name="actual"/> is greater than <paramref name="bound"/>.
        /// </exception>
        public void CheckSequenceBound(uint actual, uint bound)
        {
            if (actual > bound)
            {
                throw new WireException.OutOfRange(
                    length,
                    $"at most {bound} elements in a bounded sequence, and this one has "
                    + $"{actual}");
            }
        }

        /// <summary>
        /// The count check every bounded <c>map</c> makes before a byte of it
        /// is written.
        /// </summary>
        /// <remarks>
        /// Its own message and not <see cref="CheckSequenceBound"/>'s: the unit
        /// is an entry and not an element, per XTypes 1.3 §7.4.1.1.4's map-as-a-
        /// sequence-of-pairs equivalence.
        /// </remarks>
        /// <param name="actual">How many entries the value holds.</param>
        /// <param name="bound">The declared bound.</param>
        /// <exception cref="WireException.OutOfRange">
        /// <paramref name="actual"/> is greater than <paramref name="bound"/>.
        /// </exception>
        public void CheckMapBound(uint actual, uint bound)
        {
            if (actual > bound)
            {
                throw new WireException.OutOfRange(
                    length,
                    $"at most {bound} entries in a bounded map, and this one has {actual}");
            }
        }

        /// <summary>
        /// A <c>wstring</c>, in the form <paramref name="policy"/> and
        /// <paramref name="order"/> name.
        /// </summary>
        /// <remarks>
        /// <para>
        /// <see cref="WstringLengthPolicy.Specification"/> is a four-byte byte
        /// count and then UTF-16 code units. No BOM and no terminator — XTypes
        /// 1.3 §7.4.1.1.2 forbids both outright: "The UTF-16 representation …
        /// shall not include a Byte Order Mark (BOM). The representation shall
        /// also not include any terminating NUL character(s)." The order the
        /// units go out in is the stream's, which the encapsulation identifier
        /// already announced.
        /// </para>
        /// <para>
        /// The count is of <b>octets</b> under that policy and not of
        /// characters: §7.4.1.1.2 says "This is twice the number of characters
        /// in the string", with the clause's own rationale being that a code
        /// point outside the Basic Multilingual Plane takes two units and a
        /// byte count still reaches the end of the string. So <c>"𝄞"</c> is a
        /// length of four.
        /// </para>
        /// <para>
        /// A .NET <c>string</c> already holds UTF-16 code units, which is the
        /// one thing this column gets for free. What it does not get for free
        /// is the surrogate rule: a lone surrogate is a <c>char</c> .NET admits
        /// and is not a code point, so it is refused rather than written.
        /// </para>
        /// </remarks>
        public void WriteWstringUtf8(
            string value, WstringLengthPolicy policy, WstringUnitOrder order)
        {
            string text = value ?? string.Empty;
            int width = WstringLengthPolicies.UnitWidth(policy);
            bool countsCharacters = WstringLengthPolicies.CountsCharacters(policy);
            if (countsCharacters && width == 2)
            {
                // Fast CDR counts characters and writes each four-byte
                // `wchar_t` as two bytes, so a code point outside the BMP goes
                // out as its low sixteen bits and arrives as a different
                // character. A vendor's length convention is a policy; its data
                // loss is not, so the value is refused. The refusal is on the
                // pair (characters, two octets) and not on a named form: a
                // character count over four-octet units carries the surrogate
                // pair intact.
                for (int index = 0; index < text.Length; index++)
                {
                    if (char.IsHighSurrogate(text[index]))
                    {
                        throw new WireException.OutOfRange(
                            index,
                            "a code point inside the BMP: a character count over two-octet "
                            + "units has no way to carry one outside it");
                    }
                }
            }

            int[] units = Utf16Units(text);
            int count = units.Length;
            bool terminated = WstringLengthPolicies.IsTerminated(policy);
            bool marked = order == WstringUnitOrder.ByteOrderMark && count > 0;

            // CORBA §9.3.1.6's GIOP 1.1 rule, and RTI's XCDR1 form: a NUL code
            // unit at the end, inside whatever the length counts. Nothing is
            // lost above the BMP — both units are written.
            //
            // An empty `wstring` carries no mark: omniORB writes `00000000` for
            // one, measured 2026-08-10, byte-identical to every other
            // implementation. A mark on an empty string would be a two-octet
            // string that decodes to nothing, and no peer writes it.
            long total = count + (terminated ? 1 : 0) + (marked ? 1 : 0);
            WriteU32((uint)(countsCharacters ? total : total * width));
            if (marked)
            {
                WriteWstringUnit(ByteOrderMarkUnit, width, order);
            }

            for (int index = 0; index < count; index++)
            {
                WriteWstringUnit(units[index], width, order);
            }

            if (terminated)
            {
                WriteWstringUnit(0, width, order);
            }
        }

        /// <summary>A <c>wstring</c> whose characters go out as their own UTF-8 octets.</summary>
        /// <remarks>
        /// <c>WstringCodeUnits::Utf8</c> is the measured departure of one
        /// product's generated serialiser: XTypes 1.3 Table 31 gives a
        /// <c>wstring</c>'s code unit as <c>Char16</c> and §7.4.1.1.2 makes the
        /// length "twice the number of characters", so the clause has no UTF-8
        /// form at all. The declared length is the UTF-8 octet count and only C2
        /// — the terminator — still reaches the wire. <c>wstring-member</c> is
        /// <c>04 00 00 00 61 62 63 00</c> for <c>"abc"</c>: three octets, a NUL,
        /// and the four declares both.
        /// </remarks>
        /// <param name="value">The text.</param>
        /// <param name="policy">What the length field counts.</param>
        /// <exception cref="WireException.NotUtf8">
        /// The value holds an unpaired surrogate, which is not a code point.
        /// </exception>
        public void WriteWstringUtf8Octets(string value, WstringLengthPolicy policy)
        {
            string text = value ?? string.Empty;
            byte[] encoded = Utf8(text);
            bool terminated = WstringLengthPolicies.IsTerminated(policy);
            long total = encoded.Length + (terminated ? 1 : 0);
            if (total > uint.MaxValue)
            {
                throw new WireException.OutOfRange(
                    length, "a wstring length the UInt32 field can hold");
            }

            WriteU32((uint)total);
            AppendAll(encoded);
            if (terminated)
            {
                Append(0);
            }
        }

        /// <summary>A bounded UTF-8-octet <c>wstring</c>.</summary>
        /// <remarks>
        /// The bound still counts UTF-16 code units, which is what a
        /// <c>wstring&lt;N&gt;</c> bound means, while the wire carries the UTF-8
        /// octets.
        /// </remarks>
        /// <param name="value">The text.</param>
        /// <param name="bound">The bound in UTF-16 code units.</param>
        /// <param name="policy">What the length field counts.</param>
        /// <exception cref="WireException.OutOfRange">
        /// The value holds more than <paramref name="bound"/> UTF-16 code units.
        /// </exception>
        public void WriteWstringUtf8OctetsBounded(
            string value, int bound, WstringLengthPolicy policy)
        {
            string text = value ?? string.Empty;
            int units = Utf16Units(text).Length;
            if (units > bound)
            {
                throw new WireException.OutOfRange(
                    length,
                    $"at most {bound} UTF-16 code units in a bounded wstring, and this "
                    + $"one has {units}");
            }

            WriteWstringUtf8Octets(text, policy);
        }

        /// <summary>One <c>wstring</c> code unit, at its width and in its order.</summary>
        private void WriteWstringUnit(int unit, int width, WstringUnitOrder order)
        {
            if (width == 4)
            {
                if (order == WstringUnitOrder.BigEndianAlways)
                {
                    WriteU32Big((uint)unit);
                }
                else if (WstringUnitOrders.IsFixedLittle(order))
                {
                    WriteU32Little((uint)unit);
                }
                else
                {
                    WriteU32((uint)unit);
                }
            }
            else if (order == WstringUnitOrder.BigEndianAlways)
            {
                WriteU16Big((ushort)unit);
            }
            else if (WstringUnitOrders.IsFixedLittle(order))
            {
                WriteU16Little((ushort)unit);
            }
            else
            {
                WriteU16((ushort)unit);
            }
        }

        /// <summary>
        /// <c>fixed&lt;digits, scale&gt;</c> as packed BCD. [CDR] §9.3.2.8.
        /// </summary>
        /// <remarks>
        /// <para>
        /// DDS-XTypes 1.3 does not carry the type at all — §7.2.2 has no
        /// fixed-point kind and §7.4.3.5.3 has no rule whose left-hand side
        /// matches one — so the form is CORBA's, which is where an IDL
        /// <c>fixed</c> came from and the only wire definition it has.
        /// </para>
        /// <para>
        /// The value arrives as a decimal <b>string</b> and not as a
        /// <c>decimal</c>. A .NET <c>decimal</c> carries 28 to 29 significant
        /// digits and IDL 4.2 §7.4.1.4.4 allows 31; the corpus case
        /// <c>fixed-scales</c> declares <c>fixed&lt;31,10&gt;</c> and its value
        /// <c>-12345678901234567890.1234567890</c> has thirty, which a
        /// <c>decimal</c> rounds. Beyond that, the
        /// <see cref="FixedDigitCount.ValuePrecision"/> axis exists to measure
        /// which digit count a peer writes into the BCD run, and a holder that
        /// normalises its leading zeros away has already decided that answer.
        /// It is not parsed through <c>decimal.Parse</c> either: that would
        /// accept an exponent and normalise a scale, and neither belongs on
        /// this path.
        /// </para>
        /// </remarks>
        public void WriteFixed(string value, int digits, int scale, FixedDigitCount count)
        {
            int[] nibbles = FixedNibbles(value, digits, scale, count);
            for (int index = 0; index < nibbles.Length; index += 2)
            {
                Append((byte)((nibbles[index] << 4) | nibbles[index + 1]));
            }
        }

        /// <summary>
        /// The BCD nibbles of <paramref name="text"/> under
        /// <c>fixed&lt;digits, scale&gt;</c>.
        /// </summary>
        /// <remarks>
        /// [CDR] §9.3.2.8: digit nibbles most-significant first, then a sign
        /// nibble — <c>0xc</c> for positive or zero, <c>0xd</c> for negative —
        /// with a leading zero nibble prepended when the nibble count is odd so
        /// the nibbles pair up.
        ///
        /// Neither half is padded in the direction that would lose a digit: a
        /// value too long for its integer part or its fraction is one this type
        /// cannot represent, and is refused.
        /// </remarks>
        public static int[] FixedNibbles(
            string text, int digits, int scale, FixedDigitCount count)
        {
            if (text == null)
            {
                throw new WireException.OutOfRange(0, "a decimal, not null");
            }

            if (digits == 0)
            {
                throw new WireException.OutOfRange(0, "a fixed with at least one digit");
            }

            // OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and
            // fixed<31, S> is the widest declaration IDL admits. A wider one
            // has no packing defined for it, so it is refused rather than
            // packed into octets no reader can be told the width of.
            if (digits > 31)
            {
                throw new WireException.OutOfRange(
                    0, "a fixed of at most 31 digits, not " + digits);
            }

            if (scale > digits)
            {
                throw new WireException.OutOfRange(0, "a scale no larger than the digit count");
            }

            bool negative = false;
            string magnitude = text;
            if (magnitude.StartsWith("-", StringComparison.Ordinal))
            {
                negative = true;
                magnitude = magnitude.Substring(1);
            }
            else if (magnitude.StartsWith("+", StringComparison.Ordinal))
            {
                magnitude = magnitude.Substring(1);
            }

            int dot = magnitude.IndexOf('.');
            string whole = dot < 0 ? magnitude : magnitude.Substring(0, dot);
            string fraction = dot < 0 ? string.Empty : magnitude.Substring(dot + 1);
            string bad = "a decimal that fits fixed<" + digits + ", " + scale + ">, not `"
                + text + "`";
            if (whole.Length == 0 && fraction.Length == 0)
            {
                throw new WireException.OutOfRange(0, bad);
            }

            if (!IsAsciiDigits(whole) || !IsAsciiDigits(fraction))
            {
                throw new WireException.OutOfRange(0, bad);
            }

            int asWritten = whole.Length + scale;
            int integralWidth = digits - scale;
            int first = 0;
            while (first < whole.Length && whole[first] == '0')
            {
                first++;
            }

            whole = whole.Substring(first);
            if (whole.Length > integralWidth)
            {
                throw new WireException.OutOfRange(
                    0,
                    "a decimal whose integer part fits " + integralWidth + " digits, not `"
                    + text + "`");
            }

            if (fraction.Length > scale)
            {
                throw new WireException.OutOfRange(
                    0, "a decimal whose fraction fits " + scale + " digits, not `" + text + "`");
            }

            // The full `digits`-wide field first, whatever the count: the
            // value's precision is that field with its leading zeros removed,
            // so both counts are read off one construction.
            int[] every = new int[digits];
            int at = integralWidth - whole.Length;
            for (int index = 0; index < whole.Length; index++)
            {
                every[at++] = whole[index] - '0';
            }

            for (int index = 0; index < fraction.Length; index++)
            {
                every[at++] = fraction[index] - '0';
            }

            int leadingZeros = 0;
            while (leadingZeros < every.Length && every[leadingZeros] == 0)
            {
                leadingZeros++;
            }

            int significant = every.Length - leadingZeros;
            int wire = digits;
            if (count == FixedDigitCount.ValuePrecision)
            {
                wire = Math.Max(significant, 1);
            }
            else if (count == FixedDigitCount.ValueStringDigits)
            {
                // ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one
                // leading zero. Past 31 no reader can be told the width.
                wire = asWritten + (asWritten % 2 == 0 ? 1 : 0);
                if (wire > 31)
                {
                    throw new WireException.OutOfRange(
                        0, "a fixed of at most 31 digits on the wire, not " + wire + " for `"
                        + text + "`");
                }
            }

            bool odd = (wire + 1) % 2 == 1;
            int[] nibbles = new int[wire + 1 + (odd ? 1 : 0)];
            int write = 0;
            if (odd)
            {
                nibbles[write++] = 0;
            }

            // A count wider than P is filled with leading zeros (the array
            // starts zeroed).
            int kept = Math.Min(wire, every.Length);
            write += wire - kept;
            Array.Copy(every, every.Length - kept, nibbles, write, kept);
            write += kept;

            // A negative zero is written positive, because there is one zero.
            nibbles[write] = negative && significant > 0 ? 0x0D : 0x0C;
            return nibbles;
        }

        private static bool IsAsciiDigits(string text)
        {
            for (int index = 0; index < text.Length; index++)
            {
                if (text[index] < '0' || text[index] > '9')
                {
                    return false;
                }
            }

            return true;
        }

        // -- the bit-bounded kinds --------------------------------------------

        /// <summary>
        /// An enumerator, as the integer its bit bound calls for.
        /// </summary>
        /// <remarks>
        /// <para>
        /// XTypes 1.3 §7.4.3.5.3 rule (5) is
        /// <c>XCDR &lt;&lt; {O : ENUM_TYPE} = XCDR &lt;&lt; {O.value :
        /// O.holder_type}</c>, and the ENUM_TYPE row of the same clause's table
        /// says "O.holder_type is either Int8, Int16 or Int32 depending on the
        /// value of the @bit_bound annotation". A <b>signed</b> holder:
        /// <c>@value(-1)</c> is the two's complement of one, so at the default
        /// bound it goes out as <c>ffffffff</c>.
        /// </para>
        /// <para>
        /// What fits is everything expressible in the bound's bits, signed or
        /// unsigned — <c>-2^(n-1) ..= 2^n - 1</c>. RTI Connext 7.7 accepts
        /// <c>@value(-1)</c> at the default bound and Cyclone accepts
        /// <c>@value(200)</c> under <c>@bit_bound(8)</c>; a signed-only range
        /// would reject the second and a wider one would accept values neither
        /// can express.
        /// </para>
        /// <para>
        /// §7.3.1.2.1.5 caps the bound itself at 1 to 32 for an enumerated
        /// type, so a bound outside that has no holder and is refused rather
        /// than widened.
        /// </para>
        /// </remarks>
        public void WriteEnum(int bitBound, long value)
        {
            if (bitBound <= 0 || bitBound > 32)
            {
                throw new WireException.OutOfRange(
                    length, "an @bit_bound between 1 and 32, not " + bitBound);
            }

            long low = -(1L << (bitBound - 1));
            long high = (1L << bitBound) - 1;
            if (value < low || value > high)
            {
                throw new WireException.OutOfRange(
                    length,
                    "an enumerator a " + bitBound + "-bit holder can hold, not " + value);
            }

            ulong pattern = (ulong)value & ((1UL << bitBound) - 1UL);
            if (bitBound <= 8)
            {
                WriteU8((byte)pattern);
            }
            else if (bitBound <= 16)
            {
                WriteU16((ushort)pattern);
            }
            else
            {
                WriteU32((uint)pattern);
            }
        }

        /// <summary>
        /// A bitmask, as the unsigned integer its bit bound calls for.
        /// </summary>
        /// <remarks>
        /// XTypes 1.3 §7.4.3.5.3 rule (6) writes <c>O.value</c> as
        /// <c>O.holder_type</c>, and Table 33 of §7.4.1.1.3.2 gives the holder:
        /// <c>UInt8</c> for a bound of 1 to 8, <c>UInt16</c> to 16,
        /// <c>UInt32</c> to 32 and <c>UInt64</c> to 64. The same clause fixes
        /// the direction — "Bit indexes are counted from zero starting at the
        /// least-significant bit of the full byte size of the bitmask" — so bit
        /// <c>n</c> is <c>1 &lt;&lt; n</c> whatever the holder's width.
        ///
        /// A bit at or past the bound is refused: the clause says those bits
        /// "are not considered to be part of the bitmask", so setting one would
        /// put a value on the wire no peer must read back. C# has a
        /// <c>ulong</c>, so unlike the Java column the bound of exactly 64
        /// needs no exception to that rule — there is simply nothing above it.
        /// </remarks>
        public void WriteBitmask(int bitBound, ulong bits)
        {
            if (bitBound <= 0 || bitBound > 64)
            {
                throw new WireException.OutOfRange(
                    length, "an @bit_bound between 1 and 64, not " + bitBound);
            }

            if (bitBound < 64 && (bits >> bitBound) != 0)
            {
                throw new WireException.OutOfRange(
                    length, "a bitmask below 2^" + bitBound + ", not " + bits);
            }

            if (bitBound <= 8)
            {
                WriteU8((byte)bits);
            }
            else if (bitBound <= 16)
            {
                WriteU16((ushort)bits);
            }
            else if (bitBound <= 32)
            {
                WriteU32((uint)bits);
            }
            else
            {
                WriteU64(bits);
            }
        }

        /// <summary>
        /// One bitfield, shifted into the accumulator a bitset packs into.
        /// </summary>
        /// <remarks>
        /// The counterpart of <c>zdw_bitfield</c>. A value wider than the field
        /// is refused rather than masked: masking would put a different number
        /// on the wire than the caller asked for, and the caller would never
        /// find out.
        /// </remarks>
        /// <param name="value">The field's value.</param>
        /// <param name="width">How many bits the field declared.</param>
        /// <param name="position">The field's low bit inside the anchor.</param>
        public ulong Bitfield(ulong value, int width, int position)
        {
            if (width <= 0 || width > 64 || position < 0 || position + width > 64)
            {
                throw new WireException.OutOfRange(
                    length, "a bitfield of " + width + " bits at " + position + ", inside 64");
            }

            ulong limit = width >= 64 ? ulong.MaxValue : (1UL << width) - 1UL;
            if (value > limit)
            {
                throw new WireException.OutOfRange(
                    length, "a value inside " + width + " bits, not " + value);
            }

            return value << position;
        }

        /// <summary>
        /// A bitset, as the anchor integer its bitfields pack into.
        /// </summary>
        /// <remarks>
        /// DDS-XTypes 1.3 has no serialisation rule for a bitset — §7.4.3.5.3
        /// runs from rule (1) to rule (30) and no left-hand side matches one.
        /// How wide the anchor is comes from the layout planner and arrives
        /// here as a number, the same as every other framing decision.
        ///
        /// A value the anchor cannot hold is refused rather than masked:
        /// masking would drop the bitfields above the cut and report success.
        /// </remarks>
        public void WriteBitset(int anchorBytes, ulong bits)
        {
            switch (anchorBytes)
            {
                case 1:
                    Check(bits, 0xFFUL, "a one-byte bitset anchor");
                    WriteU8((byte)bits);
                    return;
                case 2:
                    Check(bits, 0xFFFFUL, "a two-byte bitset anchor");
                    WriteU16((ushort)bits);
                    return;
                case 4:
                    Check(bits, 0xFFFFFFFFUL, "a four-byte bitset anchor");
                    WriteU32((uint)bits);
                    return;
                case 8:
                    WriteU64(bits);
                    return;
                default:
                    throw new WireException.OutOfRange(
                        length, "a bitset anchor of 1, 2, 4 or 8 bytes, not " + anchorBytes);
            }
        }

        // -- framing ----------------------------------------------------------

        /// <summary>
        /// A DHEADER: a four-byte length the members that follow occupy.
        /// </summary>
        /// <remarks>
        /// Written as a placeholder and patched by
        /// <see cref="FinishDheader"/> when the members are done, because the
        /// length is not known until then.
        /// </remarks>
        public DHeader WriteDheader()
        {
            AlignTo(4);
            int at = length;
            AppendZeros(4);
            return new DHeader(at, true);
        }

        /// <summary>
        /// A DHEADER that is not written at all, so that generated code can
        /// hold one unconditionally and let the plan decide.
        /// </summary>
        public static DHeader NoDheader()
        {
            return new DHeader(0, false);
        }

        /// <summary>Patch a DHEADER with the length of what followed it.</summary>
        public void FinishDheader(DHeader header)
        {
            if (!header.Present)
            {
                return;
            }

            Patch(header.At, (ulong)(length - header.At - 4), 4);
        }

        /// <summary>
        /// An XCDR2 EMHEADER: the member id, its length code and the
        /// must-understand flag in one 32-bit word. XTypes 1.3 §7.4.3.4.2.
        /// </summary>
        /// <remarks>
        /// The length code is the layout plan's decision and arrives here as a
        /// number; this runtime does not choose one.
        ///
        /// LC0 through LC3 carry the length in the code itself. LC4 is followed
        /// by a NEXTINT holding the member's length. LC5 through LC7 also have
        /// a NEXTINT, but §7.4.3.4.2 says it "shall be reused also as part of
        /// the serialized member … because the serialization of certain members
        /// also starts with an integer length, which would take exactly the same
        /// value as NEXTINT" — so <b>no placeholder is reserved for them</b>:
        /// the member's own leading word lands in that position and <i>is</i>
        /// the NEXTINT. Reserving one would write the length twice.
        /// </remarks>
        public EmHeader WriteEmheader(uint memberId, int lengthCode, bool mustUnderstand)
        {
            if (lengthCode < 0 || lengthCode > 7)
            {
                throw new WireException.OutOfRange(
                    length, "a length code between 0 and 7, not " + lengthCode);
            }

            // §7.2.2.4.4.4.4 runs member ids from 0 to 0x0FFFFFFF and
            // §7.4.3.4.2 leaves them the low 28 bits of the word. Masking a
            // larger one writes a well-formed header for `id & 0x0FFFFFFF`:
            // the peer reads a member the caller did not name, and nothing at
            // either end says so.
            if (memberId > EMemberIdMask)
            {
                throw new WireException.OutOfRange(
                    length,
                    "a member id inside the 28 bits of an EMHEADER, not " + memberId);
            }

            AlignTo(4);
            uint word = (mustUnderstand ? 0x80000000U : 0U)
                | ((uint)(lengthCode & 0x7) << 28)
                | (memberId & EMemberIdMask);
            WriteU32(word);
            int wordAt = length - 4;
            if (lengthCode == 4)
            {
                int at = length;
                AppendZeros(4);
                return new EmHeader(at, true, wordAt, lengthCode);
            }

            return new EmHeader(0, false, wordAt, lengthCode);
        }

        /// <summary>
        /// Patch an EMHEADER's NEXTINT with the length of what followed.
        /// </summary>
        /// <remarks>
        /// Under <see cref="ShortLengthCodes"/> an LC4 header whose member
        /// turned out to be 1, 2, 4 or 8 bytes long instead loses its NEXTINT
        /// and states the length in the code, which is what Fast DDS does.
        /// </remarks>
        public void FinishEmheader(EmHeader header)
        {
            if (!header.HasNextInt)
            {
                // Under codes 0 to 3 the code *is* the length — 1, 2, 4 or 8
                // octets, §7.4.3.4.2. A value of another size misdeclares it
                // and the reader's skip lands inside data, so the mismatch is
                // refused here rather than written. Codes 5 to 7 declare
                // nothing of their own: the member's leading length word is
                // the NEXTINT.
                if (header.LengthCode >= 0 && header.LengthCode <= 3
                    && header.Word + 4 <= length)
                {
                    int declared = 1 << header.LengthCode;
                    int body = length - header.Word - 4;
                    if (body != declared)
                    {
                        throw new WireException.OutOfRange(
                            length,
                            "a member of " + declared + " octets under length code "
                                + header.LengthCode + ", not " + body);
                    }
                }

                return;
            }

            int written = length - header.NextInt - 4;
            if (shortLengthCodes
                && (written == 1 || written == 2 || written == 4 || written == 8))
            {
                ShortenEmheader(header, written);
                return;
            }

            Patch(header.NextInt, (ulong)written, 4);
        }

        /// <summary>
        /// Remove an LC4 header's NEXTINT and raise its length code.
        /// </summary>
        /// <remarks>
        /// Every byte of the member's value is unchanged and only its position
        /// moves, which is safe for one reason and only under XCDR2: MAXALIGN
        /// is four there, so the value began at a four-aligned offset with the
        /// NEXTINT in front of it and begins at a four-aligned offset without
        /// it.
        /// </remarks>
        private void ShortenEmheader(EmHeader header, int written)
        {
            int code;
            switch (written)
            {
                case 1: code = 0; break;
                case 2: code = 1; break;
                case 4: code = 2; break;
                default: code = 3; break;
            }

            Delete(header.NextInt, 4);
            ulong previous = ReadBack(header.Word, 4);
            Patch(header.Word, (previous & ~(0x7UL << 28)) | ((ulong)code << 28), 4);
        }

        /// <summary>
        /// A PL_CDR1 parameter header, short form: <c>[id:u16][length:u16]</c>.
        /// </summary>
        /// <remarks>
        /// <paramref name="mustUnderstand"/> is bit 14 of the id. §7.4.1.2.2:
        /// the flag "shall be set if and only if the must_understand property of
        /// the member is set to true". Two claimants on the same word,
        /// deliberately: the member's own declaration, and whatever the framing
        /// puts on every id regardless — OpenDDS's <c>c0010004</c> is the second
        /// alone.
        ///
        /// The parameter's value is its own encapsulated block, so this moves
        /// the alignment origin; <see cref="FinishPid"/> puts it back before
        /// aligning to four.
        /// </remarks>
        public Pid WritePid(uint pid, ParameterRole role, bool mustUnderstand)
        {
            if (pid > 0xFFFF)
            {
                throw new WireException.OutOfRange(length, "a parameter id in [0, 65535]");
            }

            // §7.4.1.2.2 gives the short form the ids 0 to HighestShortId
            // inclusive and reserves what is above it — PID_EXTENDED is 0x3F01
            // itself. A short header with a reserved id collides with the
            // protocol's own parameters, so such an id takes the extended form
            // or none.
            if (pid > (uint)plCdr1.HighestShortId)
            {
                throw new WireException.OutOfRange(
                    length,
                    "a parameter id the short header carries, not " + pid
                        + " — the extended header takes it");
            }

            AlignTo(4);
            int identifier = (int)pid;
            if (mustUnderstand && plCdr1.MemberMustUnderstand)
            {
                identifier |= PidMustUnderstand;
            }

            identifier |= PidFlagPolicies.FlagsAt(plCdr1.PidFlags, WirePosition.Member);
            int bare = identifier & ~(PidMustUnderstand | PidImplExtension);
            WriteU16((ushort)identifier);
            int at = length;
            AppendZeros(2);
            return new Pid(at, false, BeginParameterValue(), role, bare, mustUnderstand);
        }

        /// <summary>
        /// A PL_CDR1 parameter header, extended form.
        /// </summary>
        /// <remarks>
        /// <c>[PID_EXTENDED][8][id:u32][length:u32]</c>, required when the id
        /// reaches the reserved range or the value is longer than
        /// <c>0xFFFF</c>, and used throughout under Cyclone DDS's framing.
        /// XTypes 1.3 §7.4.1.2.
        ///
        /// The member's must-understand is <c>FLAG_2</c> of the
        /// <c>eMemberHeader</c> (§7.4.1.2.1) and not the marker's bit above it:
        /// Cyclone sets the marker's on every parameter and <c>FLAG_2</c> only
        /// where the member declares it.
        /// </remarks>
        public Pid WritePidExtended(uint memberId, ParameterRole role, bool mustUnderstand)
        {
            // The same 28 bits as an EMHEADER's, here in the
            // <c>eMemberHeader</c> of §7.4.1.2.1. Masking aliases the
            // parameter onto another member exactly as it does there.
            if (memberId > EMemberIdMask)
            {
                throw new WireException.OutOfRange(
                    length,
                    "a member id inside the 28 bits of an eMemberHeader, not " + memberId);
            }

            AlignTo(4);
            int marker = PidExtended
                | PidFlagPolicies.FlagsAt(plCdr1.PidFlags, WirePosition.ExtendedMarker);
            WriteU16((ushort)marker);
            WriteU16(8);
            uint memberHeader = memberId & EMemberIdMask;
            if (mustUnderstand && plCdr1.MemberMustUnderstand)
            {
                memberHeader |= EFlagMustUnderstand;
            }

            WriteU32(memberHeader);
            int at = length;
            AppendZeros(4);
            return new Pid(
                at,
                true,
                BeginParameterValue(),
                role,
                (int)(memberId & EMemberIdMask),
                mustUnderstand);
        }

        /// <summary>
        /// Start a parameter's value, returning the origin it replaces.
        /// </summary>
        /// <remarks>
        /// "Each element, or parameter, within a parameter list data structure
        /// is simply a CDR-encapsulated block of data" — XTypes 1.3 §7.4.1.2. A
        /// block counts alignment from its own start, so an eight-byte member at
        /// the head of one is already aligned and takes no padding inside its
        /// own length.
        /// </remarks>
        private int BeginParameterValue()
        {
            int saved = origin;
            origin = length;
            return saved;
        }

        /// <summary>
        /// Patch a parameter header with its value's length.
        /// </summary>
        /// <remarks>
        /// The declared length covers the value's trailing padding or stops at
        /// the value, depending on the framing — Cyclone and RTI round it up to
        /// four, Fast DDS does not. The padding itself starts the next parameter
        /// at a four-byte offset measured from the enclosing stream, so the
        /// origin goes back before it is written. XTypes 1.3 §7.4.1.2.
        /// </remarks>
        public void FinishPid(Pid header)
        {
            int width = header.Extended ? 4 : 2;
            int written = length - header.At - width;
            if (plCdr1.PadsDeclaredLength(header.Role))
            {
                int past = Modulo(length - header.SavedOrigin, 4);
                if (past != 0)
                {
                    written += 4 - past;
                }
            }

            if (!header.Extended && written > PidShortMaxLen)
            {
                PromotePid(header, written);
            }
            else
            {
                Patch(header.At, (ulong)written, width);
            }

            origin = header.SavedOrigin;
            // Under Fast DDS's framing the parameter writes no padding of
            // its own — the next header aligns itself, and a final
            // parameter ends the stream where its value does.
            if (plCdr1.PadAfterValue)
            {
                AlignTo(4);
            }
        }

        /// <summary>
        /// Rewrite a short parameter header as the extended form.
        /// </summary>
        /// <remarks>
        /// §7.4.1.2.2's <c>ssize</c> is sixteen bits, and which values overrun
        /// it is a property of the sample rather than of the type — a
        /// <c>string</c> member is short in almost every sample and over 64 KiB
        /// in some. So the plan writes the short form for an unbounded member
        /// and the fallback lives here, where the length is known.
        ///
        /// Eight bytes are inserted between the header and the value. The
        /// value's own bytes do not change: XCDR1's MAXALIGN is eight and the
        /// insertion is eight, so every offset inside it keeps its remainder.
        /// </remarks>
        private void PromotePid(Pid header, int written)
        {
            int start = header.At - 2;
            InsertZeros(header.At + 2, 8);
            int marker = PidExtended
                | PidFlagPolicies.FlagsAt(plCdr1.PidFlags, WirePosition.ExtendedMarker);
            Patch(start, (ulong)(marker & 0xFFFF), 2);
            Patch(start + 2, 8, 2);
            uint memberHeader = (uint)header.Id & EMemberIdMask;
            if (header.MustUnderstand && plCdr1.MemberMustUnderstand)
            {
                memberHeader |= EFlagMustUnderstand;
            }

            Patch(start + 4, memberHeader, 4);
            Patch(start + 8, (ulong)written, 4);
        }

        /// <summary>
        /// The <c>PID_LIST_END</c> sentinel a PL_CDR1 parameter list ends with.
        /// </summary>
        /// <remarks>
        /// A reader that stops at the last member it recognises instead of at
        /// the sentinel misreads whatever follows.
        ///
        /// The flags come from the framing asked at the sentinel position, which
        /// is the only thing that decides them: <c>PID_LIST_END</c> is not a
        /// member and has no declaration of its own.
        /// </remarks>
        public void WriteSentinel()
        {
            AlignTo(4);
            int sentinel = PidListEnd
                | PidFlagPolicies.FlagsAt(plCdr1.PidFlags, WirePosition.Sentinel);
            WriteU16((ushort)sentinel);
            WriteU16(0);
        }

        // -- the KeyHash ------------------------------------------------------

        /// <summary>
        /// The holder as a KeyHash, zero-padded to sixteen bytes.
        /// </summary>
        /// <remarks>For a holder of sixteen bytes or fewer. DDSI-RTPS 2.5 §9.6.3.8.</remarks>
        public byte[] ZeroPadded()
        {
            byte[] hash = new byte[KeyHashLen];
            Array.Copy(outBytes, 0, hash, 0, Math.Min(length, KeyHashLen));
            return hash;
        }

        /// <summary>
        /// The MD5 of the holder, for one longer than sixteen bytes.
        /// </summary>
        /// <remarks>
        /// MD5 because DDSI-RTPS 2.5 §9.6.3.8 names it, not as a security
        /// claim: the KeyHash identifies an instance and is not a signature.
        /// </remarks>
        public byte[] Md5Digest()
        {
            return Md5.Digest(outBytes, 0, length);
        }

        // -- the buffer -------------------------------------------------------

        private void Check(ulong value, ulong high, string expected)
        {
            if (value > high)
            {
                throw new WireException.OutOfRange(
                    length, expected + " in [0, " + high + "], not " + value);
            }
        }

        /// <summary>Align to the value's own width, then write it.</summary>
        private void WriteInt(ulong pattern, int width)
        {
            AlignTo(width);
            AppendRaw(pattern, width, endianness == Endianness.Little);
        }

        private void AppendRaw(ulong pattern, int width, bool little)
        {
            Need(width);
            for (int index = 0; index < width; index++)
            {
                int shift = little ? index : width - 1 - index;
                outBytes[length + index] = (byte)(pattern >> (8 * shift));
            }

            length += width;
        }

        /// <summary>The <paramref name="width"/> bytes at <paramref name="at"/>, in the stream's order.</summary>
        private ulong ReadBack(int at, int width)
        {
            ulong value = 0;
            for (int index = 0; index < width; index++)
            {
                int shift = endianness == Endianness.Little ? index : width - 1 - index;
                value |= (ulong)outBytes[at + index] << (8 * shift);
            }

            return value;
        }

        private void Patch(int at, ulong value, int width)
        {
            if (width < 8 && value >= 1UL << (width * 8))
            {
                throw new WireException.OutOfRange(
                    at, "a " + width + "-byte length, not " + value);
            }

            for (int index = 0; index < width; index++)
            {
                int shift = endianness == Endianness.Little ? index : width - 1 - index;
                outBytes[at + index] = (byte)(value >> (8 * shift));
            }
        }

        private void Append(byte value)
        {
            Need(1);
            outBytes[length++] = value;
        }

        private void AppendAll(byte[] values)
        {
            Need(values.Length);
            Array.Copy(values, 0, outBytes, length, values.Length);
            length += values.Length;
        }

        private void AppendZeros(int count)
        {
            Need(count);
            Array.Clear(outBytes, length, count);
            length += count;
        }

        private void InsertZeros(int at, int count)
        {
            Need(count);
            Array.Copy(outBytes, at, outBytes, at + count, length - at);
            Array.Clear(outBytes, at, count);
            length += count;
        }

        private void Delete(int at, int count)
        {
            Array.Copy(outBytes, at + count, outBytes, at, length - at - count);
            length -= count;
        }

        private void Need(int more)
        {
            if (length + more <= outBytes.Length)
            {
                return;
            }

            int capacity = outBytes.Length;
            while (capacity < length + more)
            {
                capacity *= 2;
            }

            byte[] grown = new byte[capacity];
            Array.Copy(outBytes, grown, length);
            outBytes = grown;
        }

        // -- text -------------------------------------------------------------

        /// <summary>
        /// The UTF-8 of <paramref name="text"/>, refusing anything that is not
        /// UTF-8.
        /// </summary>
        /// <remarks>
        /// <c>System.Text.Encoding.UTF8</c> would replace an unpaired surrogate
        /// with U+FFFD and report success — a different character on the wire,
        /// silently. The encoder here is built with
        /// <c>EncoderFallback.ExceptionFallback</c> and throws instead.
        /// </remarks>
        private byte[] Utf8(string text)
        {
            System.Text.Encoding strict = new UTF8Encoding(
                encoderShouldEmitUTF8Identifier: false, throwOnInvalidBytes: true);
            try
            {
                return strict.GetBytes(text);
            }
            catch (EncoderFallbackException)
            {
                throw new WireException.NotUtf8(
                    length, "an unpaired surrogate is not a code point");
            }
        }

        /// <summary>
        /// The UTF-16 code units of <paramref name="text"/>, without a BOM.
        /// </summary>
        /// <remarks>
        /// A .NET <c>string</c> is already UTF-16, so this is a copy — but it is
        /// also the one place a lone surrogate can be caught. .NET admits one in
        /// a <c>string</c>; it is not a code point, and writing it would put a
        /// sequence on the wire that is not UTF-16 at all.
        /// </remarks>
        private static int[] Utf16Units(string text)
        {
            int[] units = new int[text.Length];
            int index = 0;
            while (index < text.Length)
            {
                char unit = text[index];
                if (char.IsHighSurrogate(unit)
                    && index + 1 < text.Length
                    && char.IsLowSurrogate(text[index + 1]))
                {
                    // A well-formed pair: both units go out, and a byte count
                    // still reaches the end of the string. §7.4.1.1.2.
                    units[index] = unit;
                    units[index + 1] = text[index + 1];
                    index += 2;
                    continue;
                }

                if (char.IsSurrogate(unit))
                {
                    throw new WireException.OutOfRange(
                        index * 2, "a Unicode code point, not a lone surrogate");
                }

                units[index] = unit;
                index++;
            }

            return units;
        }

        private static string Hex(int codePoint)
        {
            return codePoint.ToString("X4", CultureInfo.InvariantCulture);
        }
    }
}
