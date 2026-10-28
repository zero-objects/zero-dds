// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
namespace ZeroDds.Wire
{
    /// <summary>
    /// How wide a scalar <c>wchar</c> is.
    /// </summary>
    /// <remarks>
    /// Two octets under XTypes 1.3 Table 31 and for seven of the nine measured
    /// implementations; four under RTI Connext's XCDR1 — measured 2026-08-09,
    /// <c>wchar c = U+0061</c> gives <c>61000000</c> under XCDR1 and
    /// <c>6100</c> under XCDR2 — under TAO, and under
    /// <c>rosidl_typesupport_fastrtps_c</c>.
    ///
    /// An argument of its own and not a reading of the <c>wstring</c> policy:
    /// <c>rmw_cyclonedds_cpp</c> 1.3.4 writes two octets here and four inside
    /// a <c>wstring</c>.
    ///
    /// The four-octet form zero-extends a code unit; it does not carry a code
    /// point, so §7.2.2.2.1.2.5's Basic-Multilingual-Plane restriction holds at
    /// both widths.
    /// </remarks>
    public enum WcharWidth
    {
        /// <summary>Two octets. XTypes 1.3 Table 31.</summary>
        Octets2,

        /// <summary>Four octets. RTI Connext under XCDR1, TAO, one ROS type support.</summary>
        Octets4,
    }

    /// <summary>What a <see cref="WcharWidth"/> answers.</summary>
    public static class WcharWidths
    {
        /// <summary>The width in octets, which is 2 or 4.</summary>
        public static int Octets(WcharWidth width)
        {
            return width == WcharWidth.Octets4 ? 4 : 2;
        }

        /// <summary>The width a number names, or <c>null</c> for anything else.</summary>
        public static WcharWidth? Of(int octets)
        {
            if (octets == 2)
            {
                return WcharWidth.Octets2;
            }

            if (octets == 4)
            {
                return WcharWidth.Octets4;
            }

            return null;
        }
    }

    /// <summary>
    /// How a scalar <c>wchar</c> is framed.
    /// </summary>
    /// <remarks>
    /// A <c>wstring</c> is delimited as a whole and a scalar <c>wchar</c> is
    /// not, and CORBA 3.3 part 2 §9.3.1.6 gives the scalar its own framing.
    /// All five DDS implementations write XTypes 1.3 Table 31's bare code
    /// unit; all three ORBs write an octet count and then that many octets,
    /// aligned to one. Measured 2026-08-10, <c>wchar a = U+00E4; wchar b =
    /// U+1234</c>, little endian: <c>e400 3412</c> here, <c>02 00e4 02
    /// 1234</c> from omniORB, <c>04 e4000000 04 34120000</c> from TAO.
    ///
    /// It is not a width. A <c>wchar</c> that carries its own count is one
    /// octet longer and aligns to one, so every member behind it moves.
    /// </remarks>
    public enum WcharFraming
    {
        /// <summary>A bare code unit, aligned to its own width. XTypes 1.3 Table 31.</summary>
        Fixed,

        /// <summary>
        /// One octet of length, then that many octets, aligned to one.
        /// CORBA §9.3.1.6's GIOP 1.2 <c>wchar</c>.
        /// </summary>
        GiopLengthPrefixed,
    }

    /// <summary>What a <see cref="WcharFraming"/> answers.</summary>
    public static class WcharFramings
    {
        /// <summary>The command line's spelling.</summary>
        public static string AsString(WcharFraming framing)
        {
            return framing == WcharFraming.Fixed ? "fixed" : "giop-length-prefixed";
        }

        /// <summary>The named framing, or <c>null</c> for anything else.</summary>
        public static WcharFraming? Parse(string text)
        {
            if (text == "fixed")
            {
                return WcharFraming.Fixed;
            }

            if (text == "giop-length-prefixed")
            {
                return WcharFraming.GiopLengthPrefixed;
            }

            return null;
        }
    }

    /// <summary>
    /// In what byte order a <c>wstring</c>'s code units go out.
    /// </summary>
    /// <remarks>
    /// A third axis, because CORBA makes it one: omniORB and JacORB decode a
    /// <c>wstring</c>'s units as UTF-16 big-endian whatever the stream says,
    /// and omniORB writes a byte-order mark so that its own reader knows.
    ///
    /// Mirrors <c>zerodds_ast_layout::WstringUnitOrder</c> and
    /// <c>zdw_wstring_unit_order</c>.
    /// </remarks>
    public enum WstringUnitOrder
    {
        /// <summary>The stream's own order. XTypes 1.3 §7.4.1.1.2 through ESWAP.</summary>
        Stream,

        /// <summary>
        /// UTF-16 big-endian whatever the stream says, and no mark. What
        /// omniORB and JacORB decode in the absence of one.
        /// </summary>
        BigEndianAlways,

        /// <summary>
        /// A byte-order mark, then the units, both little-endian. omniORB. The
        /// mark's two octets are counted in the length and it is not a
        /// character of the string. On read the mark decides: <c>FEFF</c>
        /// means the units follow in the order it was read in, <c>FFFE</c>
        /// means they are the other way round, and no mark at all means
        /// big-endian.
        /// </summary>
        ByteOrderMark,

        /// <summary>
        /// The units always little-endian, whatever the stream says, and no
        /// mark.
        /// </summary>
        /// <remarks>
        /// This reproduces a peer misreading its own output, and it is on this
        /// axis because it is a byte order. Cyclone swaps a <c>wstring</c>'s
        /// length word for a big-endian stream and does not swap its content,
        /// so its big-endian <c>"ab"</c> is <c>00000004 61006200</c> where RTI
        /// and Fast DDS write <c>00000004 00610062</c>. Never a default and no
        /// dialect selects it; without it nothing can exchange a
        /// <c>wstring</c> with Cyclone over big endian. In little endian it is
        /// <see cref="Stream"/> byte for byte.
        /// </remarks>
        HostUnswapped,
    }

    /// <summary>What a <see cref="WstringUnitOrder"/> answers.</summary>
    public static class WstringUnitOrders
    {
        /// <summary>The name <c>WstringUnitOrder::as_str</c> gives.</summary>
        public static string AsString(WstringUnitOrder order)
        {
            switch (order)
            {
                case WstringUnitOrder.Stream: return "stream";
                case WstringUnitOrder.BigEndianAlways: return "big-endian-always";
                case WstringUnitOrder.ByteOrderMark: return "byte-order-mark";
                default: return "host-unswapped";
            }
        }

        /// <summary>
        /// Whether a code unit goes out little-endian whatever the stream's
        /// order.
        /// </summary>
        public static bool IsFixedLittle(WstringUnitOrder order)
        {
            return order == WstringUnitOrder.ByteOrderMark
                || order == WstringUnitOrder.HostUnswapped;
        }

        /// <summary>The named order, or <c>null</c> for anything else.</summary>
        public static WstringUnitOrder? Parse(string text)
        {
            switch (text)
            {
                case "stream": return WstringUnitOrder.Stream;
                case "big-endian-always": return WstringUnitOrder.BigEndianAlways;
                case "byte-order-mark": return WstringUnitOrder.ByteOrderMark;
                case "host-unswapped": return WstringUnitOrder.HostUnswapped;
                default: return null;
            }
        }
    }

    /// <summary>
    /// How a <c>wstring</c>'s length is counted, and how wide its code units
    /// are.
    /// </summary>
    /// <remarks>
    /// Three independent questions, so eight cells: C1 — does the length
    /// prefix count characters or the octets they occupy; C2 — is a
    /// terminating NUL code unit written and counted; C3 — is one UTF-16 code
    /// unit two octets or four. All eight crossings are named, including the
    /// three no measured implementation writes, because a cell that has no
    /// name cannot be reached from a command line to be measured against —
    /// which is exactly how the ROS form went unreachable while two
    /// implementations were writing it.
    /// </remarks>
    public enum WstringLengthPolicy
    {
        /// <summary>
        /// Octets, no terminator, two-octet units. XTypes 1.3 §7.4.1.1.2 and
        /// CORBA 3.3 part 2 §9.3.1.6's GIOP 1.2 <c>wstring</c> over a
        /// two-octet code set. Cyclone and OpenDDS under both versions, RTI
        /// under XCDR2, omniORB, JacORB. The default.
        /// </summary>
        Specification,

        /// <summary>
        /// Characters, no terminator, two-octet units. Fast DDS.
        /// </summary>
        CharacterCount,

        /// <summary>
        /// Characters including a NUL, four-octet units. CORBA §9.3.1.6's GIOP
        /// 1.1 <c>wstring</c> over a four-octet code set, and RTI Connext 7.7
        /// under XCDR1.
        /// </summary>
        CharacterCountTerminated,

        /// <summary>
        /// Octets, no terminator, four-octet units. CORBA §9.3.1.6's GIOP 1.2
        /// <c>wstring</c> over a four-octet transmission code set — what TAO
        /// 2.5.24 writes.
        /// </summary>
        SpecificationWideUnits,

        /// <summary>
        /// Characters, no terminator, four-octet units.
        /// <c>rosidl_typesupport_fastrtps</c> counts characters like Fast CDR
        /// and hands each over as a four-octet <c>wchar_t</c>.
        /// </summary>
        RosTypeSupport,

        /// <summary>One of the three cells no measured implementation writes.</summary>
        OctetsTerminated2,

        /// <summary>One of the three cells no measured implementation writes.</summary>
        OctetsTerminated4,

        /// <summary>One of the three cells no measured implementation writes.</summary>
        CharactersTerminated2,
    }

    /// <summary>The three axes a <see cref="WstringLengthPolicy"/> crosses.</summary>
    public static class WstringLengthPolicies
    {
        /// <summary>The name <c>WstringLengthPolicy::as_str</c> gives.</summary>
        public static string AsString(WstringLengthPolicy policy)
        {
            switch (policy)
            {
                case WstringLengthPolicy.Specification: return "specification";
                case WstringLengthPolicy.CharacterCount: return "character-count";
                case WstringLengthPolicy.CharacterCountTerminated:
                    return "character-count-terminated";
                case WstringLengthPolicy.SpecificationWideUnits:
                    return "specification-wide-units";
                case WstringLengthPolicy.RosTypeSupport: return "ros-type-support";
                case WstringLengthPolicy.OctetsTerminated2: return "octets/terminated/2";
                case WstringLengthPolicy.OctetsTerminated4: return "octets/terminated/4";
                default: return "characters/terminated/2";
            }
        }

        /// <summary>C1 — whether the length prefix counts characters and not octets.</summary>
        public static bool CountsCharacters(WstringLengthPolicy policy)
        {
            switch (policy)
            {
                case WstringLengthPolicy.CharacterCount:
                case WstringLengthPolicy.CharacterCountTerminated:
                case WstringLengthPolicy.RosTypeSupport:
                case WstringLengthPolicy.CharactersTerminated2:
                    return true;
                default:
                    return false;
            }
        }

        /// <summary>C2 — whether a terminating NUL code unit is written and counted.</summary>
        public static bool IsTerminated(WstringLengthPolicy policy)
        {
            switch (policy)
            {
                case WstringLengthPolicy.CharacterCountTerminated:
                case WstringLengthPolicy.OctetsTerminated2:
                case WstringLengthPolicy.OctetsTerminated4:
                case WstringLengthPolicy.CharactersTerminated2:
                    return true;
                default:
                    return false;
            }
        }

        /// <summary>
        /// C3 — how many bytes one of a <c>wstring</c>'s code units takes.
        /// </summary>
        /// <remarks>
        /// <b>Not</b> the width of a scalar <c>wchar</c>, which is
        /// <see cref="WcharWidth"/>: <c>rmw_cyclonedds_cpp</c> 1.3.4 writes
        /// two octets for a <c>wchar</c> member and four for a
        /// <c>wstring</c>'s units.
        /// </remarks>
        public static int UnitWidth(WstringLengthPolicy policy)
        {
            switch (policy)
            {
                case WstringLengthPolicy.CharacterCountTerminated:
                case WstringLengthPolicy.SpecificationWideUnits:
                case WstringLengthPolicy.RosTypeSupport:
                case WstringLengthPolicy.OctetsTerminated4:
                    return 4;
                default:
                    return 2;
            }
        }

        /// <summary>The same cell spelled <c>counts/terminator/width</c>.</summary>
        public static string Coordinates(WstringLengthPolicy policy)
        {
            return (CountsCharacters(policy) ? "characters" : "octets")
                + "/" + (IsTerminated(policy) ? "terminated" : "unterminated")
                + "/" + UnitWidth(policy).ToString(System.Globalization.CultureInfo.InvariantCulture);
        }

        /// <summary>Every cell, so a caller can walk them.</summary>
        public static readonly WstringLengthPolicy[] All =
        {
            WstringLengthPolicy.Specification,
            WstringLengthPolicy.CharacterCount,
            WstringLengthPolicy.CharacterCountTerminated,
            WstringLengthPolicy.SpecificationWideUnits,
            WstringLengthPolicy.RosTypeSupport,
            WstringLengthPolicy.OctetsTerminated2,
            WstringLengthPolicy.OctetsTerminated4,
            WstringLengthPolicy.CharactersTerminated2,
        };

        /// <summary>The cell a name or a coordinate spelling picks out, or <c>null</c>.</summary>
        public static WstringLengthPolicy? Parse(string text)
        {
            foreach (WstringLengthPolicy policy in All)
            {
                if (AsString(policy) == text || Coordinates(policy) == text)
                {
                    return policy;
                }
            }

            return null;
        }
    }

    /// <summary>
    /// How many digit nibbles a <c>fixed&lt;P,S&gt;</c> puts on the wire.
    /// </summary>
    /// <remarks>
    /// [CDR] §9.3.2.8 gives the packing and says nothing about the count, and
    /// the four implementations that write one split two and two: omniORB
    /// 4.3.3 and this project write the declared <c>P</c>, JacORB 3.9 as many
    /// digits as the value has, ACE/TAO 2.5.24 the digits of the string the
    /// value was parsed from.
    /// </remarks>
    public enum FixedDigitCount
    {
        /// <summary><c>P</c> digits, whatever the value is. The default.</summary>
        Declared,

        /// <summary>
        /// As many digits as the value has, and never fewer than one: a zero
        /// is the single nibble <c>0</c> and the sign.
        /// </summary>
        ValuePrecision,

        /// <summary>
        /// The integer digits of the decimal as written, leading zeros
        /// included, plus the S fraction digits, made odd by one leading zero: what ACE/TAO 2.5.24's
        /// <c>ACE_CDR::Fixed::from_string</c> writes through its own CDR operator; OpenDDS writes the declared count.
        /// <c>0.12345</c> under <c>fixed&lt;5,5&gt;</c> is <c>0012345c</c>, wider
        /// than <c>P</c>; a count past 31 is refused. Read like
        /// <see cref="ValuePrecision"/>.
        /// </summary>
        ValueStringDigits,
    }

    /// <summary>What a <see cref="FixedDigitCount"/> answers.</summary>
    public static class FixedDigitCounts
    {
        /// <summary>
        /// The lowest nibble value that is a sign and not a digit. [CDR]
        /// §9.3.2.8: a digit is <c>0</c> to <c>9</c>. It is what makes
        /// <see cref="FixedDigitCount.ValuePrecision"/> self-delimiting on the
        /// read side.
        /// </summary>
        public const int FirstSignNibble = 0x0A;

        /// <summary>The name <c>FixedDigitCount::as_str</c> gives.</summary>
        public static string AsString(FixedDigitCount count)
        {
            return count == FixedDigitCount.Declared ? "declared"
                : count == FixedDigitCount.ValuePrecision ? "value-precision"
                : "value-string-digits";
        }

        /// <summary>The named count, or <c>null</c> for anything else.</summary>
        public static FixedDigitCount? Parse(string text)
        {
            if (text == "declared")
            {
                return FixedDigitCount.Declared;
            }

            if (text == "value-precision")
            {
                return FixedDigitCount.ValuePrecision;
            }

            if (text == "value-string-digits")
            {
                return FixedDigitCount.ValueStringDigits;
            }

            return null;
        }
    }

    /// <summary>
    /// Which of XTypes 1.3 §7.4.1.2's two roles a parameter header stands in.
    /// </summary>
    /// <remarks>
    /// The header is the same header. The role decides what the length field
    /// counts, and whether a member that took no value writes one at all: a
    /// <c>@mutable</c> member that is absent leaves no trace in the parameter
    /// list, an <c>@optional</c> member of a non-mutable type writes its
    /// header with a declared length of zero.
    /// </remarks>
    public enum ParameterRole
    {
        /// <summary>A member of a <c>@mutable</c> type. Rules (24) and (25).</summary>
        Member,

        /// <summary>
        /// An <c>@optional</c> member of a type that is not <c>@mutable</c>.
        /// Rule (19).
        /// </summary>
        Optional,
    }

    /// <summary>
    /// What a receiver does with a value its own type cannot hold.
    /// </summary>
    /// <remarks>
    /// DDS-XTypes 1.3 §7.2.2.7. The behaviour belongs to the <i>position</i>
    /// and not to the type: the clause attaches it to the member, the element,
    /// the map key or the discriminator, and one <c>typedef string&lt;5&gt;</c>
    /// may be used at two positions that want different answers. That is why a
    /// generated decoder takes it as a parameter rather than reading it off
    /// the type it decodes.
    ///
    /// The whole value is always read off the wire first. Stopping at the
    /// bound would leave the rest of it in the stream and put every following
    /// member at the wrong offset; what these three decide is only what is
    /// kept.
    /// </remarks>
    public enum TryConstruct
    {
        /// <summary>The sample is dropped and the receiver carries on.</summary>
        Discard,

        /// <summary>
        /// The value is cut to what fits. Table 11 defines it for a string, a
        /// wide string and a collection and for nothing else.
        /// </summary>
        Trim,

        /// <summary>The member takes Table 9's value.</summary>
        UseDefault,
    }
}
