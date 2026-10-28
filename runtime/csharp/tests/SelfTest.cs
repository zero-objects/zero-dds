// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
using System;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Numerics;
using ZeroDds.Idl;
using ZeroDds.Wire;

namespace ZeroDds.Wire.Tests
{
    /// <summary>
    /// What the C# column has to do before the wire can use it.
    /// </summary>
    /// <remarks>
    /// <para>
    /// No test framework, for the reason the Java column gives: this runtime
    /// has no dependencies, and adding one to assert four hundred lines of
    /// arithmetic would be the first. A <c>Main</c> that returns non-zero is
    /// what <c>runtime/csharp/run-tests.sh</c> and the CI job need.
    /// </para>
    /// <para>
    /// Three sections, and only the last two are worth anything on their own.
    /// <c>Wire</c> asserts what this runtime believes.
    /// <c>Float128Oracle</c> reads
    /// <c>crates/float128/tests/oracle/patterns.txt</c> — 518 bit patterns
    /// whose exact decimals the Rust, C, Java and Python columns already agree
    /// on, and which GCC's own <c>__float128</c> was measured against.
    /// <c>Corpus</c> writes three pure-primitive corpus cases by hand and
    /// compares the octets with what the Rust column recorded. The last two are
    /// foreign answers; the first is this column's own opinion.
    /// </para>
    /// </remarks>
    public static class SelfTest
    {
        private static int failures;

        private static void Check(bool condition, string what)
        {
            if (!condition)
            {
                Console.WriteLine("FAIL " + what);
                failures++;
            }
        }

        private static void Equal(object left, object right, string what)
        {
            if (!Equals(left, right))
            {
                Console.WriteLine("FAIL " + what + "\n  expected " + right + "\n  got      " + left);
                failures++;
            }
        }

        private static string Hex(byte[] data)
        {
            char[] digits = new char[data.Length * 2];
            for (int index = 0; index < data.Length; index++)
            {
                digits[index * 2] = "0123456789abcdef"[data[index] >> 4];
                digits[(index * 2) + 1] = "0123456789abcdef"[data[index] & 0xF];
            }

            return new string(digits);
        }

        private static byte[] Unhex(string text)
        {
            byte[] data = new byte[text.Length / 2];
            for (int index = 0; index < data.Length; index++)
            {
                data[index] = byte.Parse(
                    text.Substring(index * 2, 2), NumberStyles.HexNumber, CultureInfo.InvariantCulture);
            }

            return data;
        }

        // -- alignment ---------------------------------------------------------

        /// <summary>
        /// MAXALIGN decides where an eight-byte field lands, and the two caps
        /// give different octets.
        /// </summary>
        /// <remarks>
        /// XTypes 1.3 §7.4.1: <c>ALIGN(N)</c> is <c>MIN(N, XCDR.maxalign)</c>,
        /// eight under XCDR1 and four under XCDR2. An <c>octet</c> then a
        /// <c>long long</c> is 16 octets under the first and 12 under the
        /// second; a writer that missed the cap matches neither.
        /// </remarks>
        private static void AlignmentIsCappedAtMaxalign()
        {
            Writer eight = new Writer(Endianness.Little, 8, null, false);
            eight.WriteU8(0xAB);
            eight.WriteI64(1);
            Equal(Hex(eight.Finish()), "ab00000000000000" + "0100000000000000", "XCDR1 pads to eight");

            Writer four = new Writer(Endianness.Little, 4, null, false);
            four.WriteU8(0xAB);
            four.WriteI64(1);
            Equal(Hex(four.Finish()), "ab000000" + "0100000000000000", "XCDR2 pads to four");

            // And in reverse. The XCDR1 stream read at XCDR2's cap finds the
            // `long long` four octets early and comes back with a different
            // number — not with a failure, which is the whole reason the cap
            // has to be an argument and not a guess.
            Reader right = new Reader(eight.Finish(), Endianness.Little, 8, null);
            Equal(right.ReadU8(), (byte)0xAB, "the octet reads back");
            Equal(right.ReadI64(), 1L, "and the long long behind it, at the writer's cap");

            Reader wrong = new Reader(eight.Finish(), Endianness.Little, 4, null);
            Equal(wrong.ReadU8(), (byte)0xAB, "the octet reads back at either cap");
            Check(wrong.ReadI64() != 1, "a reader at the wrong cap returns a different number");
        }

        // -- primitives --------------------------------------------------------

        private static void EveryPrimitiveGoesOutInBothOrders()
        {
            foreach (Endianness order in new[] { Endianness.Little, Endianness.Big })
            {
                bool little = order == Endianness.Little;
                Writer w = new Writer(order, 8, null, false);
                w.WriteU16(0xBEEF);
                Equal(Hex(w.Finish()), little ? "efbe" : "beef", "u16 " + order);

                w = new Writer(order, 8, null, false);
                w.WriteU32(0xDEADBEEF);
                Equal(Hex(w.Finish()), little ? "efbeadde" : "deadbeef", "u32 " + order);

                w = new Writer(order, 8, null, false);
                w.WriteU64(ulong.MaxValue);
                Equal(Hex(w.Finish()), "ffffffffffffffff", "u64 max " + order);

                w = new Writer(order, 8, null, false);
                w.WriteU64(0x0123456789ABCDEFUL);
                Equal(
                    Hex(w.Finish()),
                    little ? "efcdab8967452301" : "0123456789abcdef",
                    "u64 " + order);

                w = new Writer(order, 8, null, false);
                w.WriteI8(-13);
                Equal(Hex(w.Finish()), "f3", "i8 " + order);

                w = new Writer(order, 8, null, false);
                w.WriteF32(-3.75f);
                Equal(Hex(w.Finish()), little ? "000070c0" : "c0700000", "f32 " + order);

                w = new Writer(order, 8, null, false);
                w.WriteF64(3.141592653589793);
                Equal(
                    Hex(w.Finish()),
                    little ? "182d4454fb210940" : "400921fb54442d18",
                    "f64 " + order);
            }

            // And back. A `ulong` is a number here and not a pattern, which is
            // the whole difference from the Java column.
            Reader r = new Reader(Unhex("ffffffffffffffff"), Endianness.Little, 8, null);
            Equal(r.ReadU64(), ulong.MaxValue, "u64 max reads back as a number");
        }

        private static void ACharAboveAnOctetIsRefused()
        {
            Writer w = new Writer(Endianness.Little, 8, null, false);
            try
            {
                // U+20AC, which is a `char` here and is not an octet there.
                w.WriteChar('€');
                Check(false, "a char above U+00FF is refused");
            }
            catch (WireException.OutOfRange)
            {
            }
        }

        // -- strings -----------------------------------------------------------

        private static void AStringCountsUtf8OctetsAndTheTerminator()
        {
            Writer w = new Writer(Endianness.Little, 8, null, false);
            // Four UTF-16 code units, six UTF-8 octets, so a length of seven.
            w.WriteString("aä€");
            Equal(Hex(w.Finish()), "0700000061c3a4e282ac00", "a string's length is octets plus NUL");

            Reader r = new Reader(Unhex("0700000061c3a4e282ac00"), Endianness.Little, 8, null);
            Equal(r.ReadString(), "aä€", "the string reads back");
        }

        private static void AWstringCountsOctetsUnderTheSpecification()
        {
            Writer w = new Writer(Endianness.Little, 8, null, false);
            // U+1D11E is one code point and two UTF-16 units, so four octets.
            w.WriteWstringUtf8(
                "𝄞", WstringLengthPolicy.Specification, WstringUnitOrder.Stream);
            Equal(
                Hex(w.Finish()),
                "0400000034d81edd",
                "a wstring's specification length is octets, and a pair is two units");

            Reader r = new Reader(Unhex("0400000034d81edd"), Endianness.Little, 8, null);
            Equal(
                r.ReadWstring(WstringLengthPolicy.Specification, WstringUnitOrder.Stream),
                "𝄞",
                "the wstring reads back whole");

            // The one refusal the character-count policy has to make: a code
            // point outside the BMP has nowhere to go in a two-octet unit
            // counted as one character.
            Writer refused = new Writer(Endianness.Little, 8, null, false);
            try
            {
                refused.WriteWstringUtf8(
                    "𝄞",
                    WstringLengthPolicy.CharacterCount,
                    WstringUnitOrder.Stream);
                Check(false, "a non-BMP code point under (characters, two octets) is refused");
            }
            catch (WireException.OutOfRange)
            {
            }
        }

        private static void ABoundedStringWstringSequenceAndMapRefuseAnOverrun()
        {
            // An over-bound value is refused rather than cut at a character
            // boundary: the bound is a promise about the source and not a
            // difference on the wire, and cutting it would put a sample on
            // the wire nobody wrote.
            Writer overBoundString = new Writer(Endianness.Little, 8, null, false);
            try
            {
                overBoundString.WriteStringBounded("abcdefgh", 5);
                Check(false, "a string past its bound is refused");
            }
            catch (WireException.OutOfRange)
            {
            }

            // Exactly at the bound still writes the same bytes an
            // unbounded string would.
            Writer atBoundString = new Writer(Endianness.Little, 8, null, false);
            atBoundString.WriteStringBounded("ab", 5);
            Equal(Hex(atBoundString.Finish()), "03000000616200", "a string at its bound encodes");

            Writer overBoundWstring = new Writer(Endianness.Little, 8, null, false);
            try
            {
                overBoundWstring.WriteWstringBounded(
                    "ab", 1, WstringLengthPolicy.Specification, WstringUnitOrder.Stream);
                Check(false, "a wstring past its bound is refused");
            }
            catch (WireException.OutOfRange)
            {
            }
            new Writer(Endianness.Little, 8, null, false).WriteWstringBounded(
                "a", 1, WstringLengthPolicy.Specification, WstringUnitOrder.Stream);

            Writer overBoundSequence = new Writer(Endianness.Little, 8, null, false);
            try
            {
                overBoundSequence.CheckSequenceBound(4, 3);
                Check(false, "a sequence count past its bound is refused");
            }
            catch (WireException.OutOfRange)
            {
            }
            new Writer(Endianness.Little, 8, null, false).CheckSequenceBound(3, 3);

            Writer overBoundMap = new Writer(Endianness.Little, 8, null, false);
            try
            {
                overBoundMap.CheckMapBound(3, 2);
                Check(false, "a map entry count past its bound is refused");
            }
            catch (WireException.OutOfRange)
            {
            }
            new Writer(Endianness.Little, 8, null, false).CheckMapBound(2, 2);
        }

        private static void ALoneSurrogateIsNotWritten()
        {
            Writer w = new Writer(Endianness.Little, 8, null, false);
            try
            {
                w.WriteString("\uD834");
                Check(false, "a lone surrogate is not UTF-8");
            }
            catch (WireException.NotUtf8)
            {
            }
        }

        // -- the three axes `Dialect::Hdds` selects ---------------------------
        //
        // Expected octets from the recorded HDDS columns:
        // tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin,
        // tests/interop-results/typedef-of-struct/hdds/plain-cdr2.le.bin,
        // tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin,
        // tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin,
        // tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr2.le.bin and
        // tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr2.le.bin.
        private static void HddsAxes()
        {
            // ElementPadding::Packed: each collection element counts its own
            // alignment from its own first octet, so the second Cell's long lands
            // at offset six and not eight.
            Writer packed = new Writer(Endianness.Little, 4, null, false);
            int saved = packed.BeginPackedElement();
            packed.WriteI32(1);
            packed.WriteI16(2);
            packed.FinishPackedElement(saved);
            saved = packed.BeginPackedElement();
            packed.WriteI32(3);
            packed.WriteI16(4);
            packed.FinishPackedElement(saved);
            packed.WriteU8(119);
            Equal(Hex(packed.Finish()), "01000000020003000000040077", "a packed element starts where the last ended");

            Reader packedReader = new Reader(packed.Finish(), Endianness.Little, 4, null);
            saved = packedReader.ReaderBeginPackedElement();
            Equal(packedReader.ReadI32(), 1, "the first packed long");
            Equal(packedReader.ReadI16(), (short)2, "the first packed short");
            packedReader.ReaderFinishPackedElement(saved);
            saved = packedReader.ReaderBeginPackedElement();
            Equal(packedReader.ReadI32(), 3, "the second packed long");
            Equal(packedReader.ReadI16(), (short)4, "the second packed short");
            packedReader.ReaderFinishPackedElement(saved);
            Equal(packedReader.ReadU8(), (byte)119, "the byte behind the array");

            // The same inside two @final members.
            Writer aggregate = new Writer(Endianness.Little, 4, null, false);
            saved = aggregate.BeginPackedElement();
            aggregate.WriteI32(-1234567);
            aggregate.WriteI16(-2);
            aggregate.FinishPackedElement(saved);
            saved = aggregate.BeginPackedElement();
            aggregate.WriteI32(1);
            aggregate.WriteI16(32767);
            aggregate.FinishPackedElement(saved);
            Equal(Hex(aggregate.Finish()), "7929edfffeff01000000ff7f", "a packed aggregate counts from its own start");

            // A sequence element's own length in DHEADER form: the 06000000 in
            // front of 7929edfffeff is the element's six octets, not the
            // encapsulation's.
            Writer element = new Writer(Endianness.Little, 4, null, false);
            DHeader header = element.WriteDheader();
            element.WriteI32(-1234567);
            element.WriteI16(-2);
            element.FinishDheader(header);
            Equal(Hex(element.Finish()), "060000007929edfffeff", "a sequence element carries its own length");

            // WstringCodeUnits::Utf8: the characters' own octets, the octet count
            // with the terminator, and C2 still reached.
            Writer text = new Writer(Endianness.Little, 4, null, false);
            text.WriteWstringUtf8Octets("abc", WstringLengthPolicy.OctetsTerminated2);
            Equal(Hex(text.Finish()), "0400000061626300", "a UTF-8 wstring is its octets and a NUL");

            Writer multibyte = new Writer(Endianness.Little, 4, null, false);
            multibyte.WriteWstringUtf8Octets("äöü ሴ Z", WstringLengthPolicy.OctetsTerminated2);
            Equal(Hex(multibyte.Finish()), "0d000000c3a4c3b6c3bc20e188b4205a00", "a multi-byte UTF-8 wstring counts its octets");

            Writer bounded = new Writer(Endianness.Little, 4, null, false);
            bounded.WriteWstringUtf8OctetsBounded("ሴäZ", 3, WstringLengthPolicy.OctetsTerminated2);
            Equal(Hex(bounded.Finish()), "07000000e188b4c3a45a00", "a bounded UTF-8 wstring holds three units");

            Reader textReader = new Reader(
                Unhex("0d000000c3a4c3b6c3bc20e188b4205a00"), Endianness.Little, 4, null);
            Equal(
                textReader.ReadWstringUtf8Octets(WstringLengthPolicy.OctetsTerminated2),
                "äöü ሴ Z",
                "a UTF-8 wstring reads back as its octets");

            // Malformed UTF-8 and a count the stream cannot hold are refused,
            // never replaced or truncated.
            try
            {
                new Writer(Endianness.Little, 4, null, false).WriteWstringUtf8Octets(
                    "\uD834", WstringLengthPolicy.OctetsTerminated2);
                Check(false, "a lone surrogate is not written as UTF-8 octets");
            }
            catch (WireException.NotUtf8)
            {
            }

            try
            {
                Reader impossible = new Reader(Unhex("ff0000006100"), Endianness.Little, 4, null);
                impossible.ReadWstringUtf8Octets(WstringLengthPolicy.OctetsTerminated2);
                Check(false, "a wstring length past the buffer is refused");
            }
            catch (WireException.ImpossibleLength)
            {
            }
        }

        // -- fixed -------------------------------------------------------------

        // ACE/TAO 2.5.24's `ACE_CDR::Fixed::from_string`, recorded in
        // crates/ast-layout/tests/fixtures/unspecified_constructs.md §3: the
        // integer digits as written plus S, made odd by one leading zero.
        private static void AFixedCanCarryTheDigitsOfItsString()
        {
            string[][] recorded =
            {
                new[] { "123.45", "5", "2", "12345c", "123.45" },
                new[] { "-123.45", "5", "2", "12345d", "-123.45" },
                new[] { "12.34", "4", "2", "01234c", "12.34" },
                new[] { "7", "1", "0", "7c", "7" },
                new[] { "0.12345", "5", "5", "0012345c", "0.12345" },
                new[] { "0.00", "5", "2", "000c", "0.00" },
                // The scale and not the fraction as written: one digit
                // counted would keep only the last nibble, `0c`.
                new[] { "7", "5", "2", "700c", "7.00" },
            };
            foreach (string[] row in recorded)
            {
                int digits = int.Parse(row[1], System.Globalization.CultureInfo.InvariantCulture);
                int scale = int.Parse(row[2], System.Globalization.CultureInfo.InvariantCulture);
                Writer w = new Writer(Endianness.Little, 8, null, false);
                w.WriteFixed(row[0], digits, scale, FixedDigitCount.ValueStringDigits);
                Equal(Hex(w.Finish()), row[3], "fixed " + row[0] + " as ACE writes it");
                Reader r = new Reader(Unhex(row[3]), Endianness.Little, 8, null);
                Equal(
                    r.ReadFixed(digits, scale, FixedDigitCount.ValueStringDigits),
                    row[4],
                    "fixed " + row[0] + " as ACE writes it reads back");
            }

            Writer wide = new Writer(Endianness.Little, 8, null, false);
            try
            {
                // Thirty-two digits as written: no width a reader can take.
                wide.WriteFixed(
                    "00000000000000000000000000000001", 31, 0, FixedDigitCount.ValueStringDigits);
                Check(false, "thirty-two digits as written are refused");
            }
            catch (WireException.OutOfRange)
            {
            }
        }

        private static void AFixedIsPackedBcdWithASignNibble()
        {
            Writer w = new Writer(Endianness.Little, 8, null, false);
            w.WriteFixed("-123.45", 5, 2, FixedDigitCount.Declared);
            Equal(Hex(w.Finish()), "12345d", "fixed<5,2> -123.45 declared");

            w = new Writer(Endianness.Little, 8, null, false);
            w.WriteFixed("-123.45", 5, 2, FixedDigitCount.ValuePrecision);
            Equal(Hex(w.Finish()), "12345d", "fixed<5,2> -123.45 at the value's precision");

            w = new Writer(Endianness.Little, 8, null, false);
            w.WriteFixed("0.01", 5, 2, FixedDigitCount.ValuePrecision);
            Equal(Hex(w.Finish()), "1c", "fixed<5,2> 0.01 is one digit at the value's precision");

            Reader r = new Reader(Unhex("12345d"), Endianness.Little, 8, null);
            Equal(r.ReadFixed(5, 2, FixedDigitCount.Declared), "-123.45", "fixed reads back");

            // Thirty significant digits — the corpus case `fixed-scales`, and
            // the measurement that refuses `decimal` as a holder.
            Writer wide = new Writer(Endianness.Little, 8, null, false);
            wide.WriteFixed("-12345678901234567890.1234567890", 31, 10, FixedDigitCount.Declared);
            byte[] octets = wide.Finish();
            Equal(octets.Length, 16, "fixed<31,10> is sixteen octets");
            Reader back = new Reader(octets, Endianness.Little, 8, null);
            Equal(
                back.ReadFixed(31, 10, FixedDigitCount.Declared),
                "-12345678901234567890.1234567890",
                "thirty significant digits survive, which a decimal would not");
        }

        // -- framing -----------------------------------------------------------

        private static void ADheaderCarriesWhatFollowedIt()
        {
            Writer w = new Writer(Endianness.Little, 4, null, false);
            DHeader d = w.WriteDheader();
            w.WriteU32(7);
            w.FinishDheader(d);
            Equal(Hex(w.Finish()), "0400000007000000", "a DHEADER is the length behind it");

            Equal(Writer.NoDheader().Present, false, "a DHEADER the plan declined writes nothing");
        }

        private static void EveryEmheaderLengthCodeFramesItsMember()
        {
            // LC4 with a NEXTINT.
            Writer w = new Writer(Endianness.Little, 4, null, false);
            EmHeader h = w.WriteEmheader(3, 4, false);
            w.WriteU32(0x11223344);
            w.FinishEmheader(h);
            Equal(Hex(w.Finish()), "0300004004000000" + "44332211", "LC4 states the length");

            // The same member with the short-code rewrite: the NEXTINT goes and
            // the code says four bytes. Fast DDS's form.
            Writer shortened = new Writer(Endianness.Little, 4, null, true);
            EmHeader s = shortened.WriteEmheader(3, 4, false);
            shortened.WriteU32(0x11223344);
            shortened.FinishEmheader(s);
            Equal(
                Hex(shortened.Finish()),
                "0300002044332211",
                "an LC4 header whose member is four bytes becomes LC2");

            // The must-understand bit is bit 31.
            Writer must = new Writer(Endianness.Little, 4, null, false);
            EmHeader m = must.WriteEmheader(1, 2, true);
            must.WriteU32(1);
            must.FinishEmheader(m);
            Equal(Hex(must.Finish()), "010000a001000000", "the EMHEADER carries must-understand");

            Reader r = new Reader(Unhex("0300004004000000" + "44332211"), Endianness.Little, 4, null);
            EmHeaderRead read = r.ReadEmheader();
            Equal(read.MemberId, 3U, "the member id comes back");
            Equal(read.LengthCode, 4, "the length code comes back");
            Equal(read.Extent, 4L, "LC4's extent is its NEXTINT");
        }

        private static void AParameterHeaderPromotesWhenItsValueOverruns()
        {
            Writer w = new Writer(Endianness.Big, 8, null, false);
            Pid p = w.WritePid(1, ParameterRole.Member, false);
            w.WriteU32(0x11223344);
            w.FinishPid(p);
            Equal(Hex(w.Finish()), "0001000411223344", "a short parameter header");

            // Over 0xFFFF: the short form cannot state the length, so the header
            // is rewritten as the extended one and the value does not move
            // relative to its own start.
            Writer big = new Writer(Endianness.Big, 8, null, false);
            Pid over = big.WritePid(1, ParameterRole.Member, false);
            for (int index = 0; index < 0x4001; index++)
            {
                big.WriteU32(0);
            }

            big.FinishPid(over);
            byte[] bytes = big.Finish();
            // `7f01` and not `3f01`: the default flag policy puts
            // PID_MUST_UNDERSTAND on the extended marker, which is what Cyclone
            // and RTI write. Fast DDS's `3f01` is the `bare` policy.
            Equal(Hex(new byte[] { bytes[0], bytes[1] }), "7f01", "the promoted marker is PID_EXTENDED");
            Equal(bytes.Length, 12 + (0x4001 * 4), "the promotion inserts eight octets");

            Writer sentinel = new Writer(Endianness.Big, 8, null, false);
            sentinel.WriteSentinel();
            Equal(Hex(sentinel.Finish()), "7f020000", "the sentinel carries must-understand by default");

            Writer bare = new Writer(
                Endianness.Big,
                8,
                PlCdr1Framing.Defaults().WithPidFlags(PidFlagPolicy.Bare),
                false);
            bare.WriteSentinel();
            Equal(Hex(bare.Finish()), "3f020000", "Fast DDS's sentinel is bare");
        }

        private static void AParameterValueCountsAlignmentFromItsOwnStart()
        {
            // §7.4.1.2: "Each element, or parameter, ... is simply a
            // CDR-encapsulated block of data." An eight-byte member at the head
            // of one takes no padding inside its own length.
            Writer w = new Writer(Endianness.Big, 8, null, false);
            Pid p = w.WritePid(1, ParameterRole.Member, false);
            w.WriteI64(1);
            w.FinishPid(p);
            Equal(
                Hex(w.Finish()),
                "00010008" + "0000000000000001",
                "a parameter's value aligns from the parameter, not from the message");
        }

        // -- the bit-bounded kinds ---------------------------------------------

        private static void AnEnumHolderIsSignedAndAsWideAsItsBound()
        {
            Writer w = new Writer(Endianness.Little, 4, null, false);
            w.WriteEnum(8, 200);
            Equal(Hex(w.Finish()), "c8", "@value(200) under @bit_bound(8)");

            w = new Writer(Endianness.Little, 4, null, false);
            w.WriteEnum(32, -1);
            Equal(Hex(w.Finish()), "ffffffff", "@value(-1) at the default bound");

            Reader r = new Reader(Unhex("c8"), Endianness.Little, 4, null);
            Equal(r.ReadEnum(8), -56L, "the holder is signed, so 0xc8 reads as -56");
        }

        private static void ABitmaskTakesTheHolderTable33Gives()
        {
            Writer w = new Writer(Endianness.Little, 4, null, false);
            w.WriteBitmask(32, 0x80000001UL);
            Equal(Hex(w.Finish()), "01000080", "a 32-bit bitmask is a uint");

            w = new Writer(Endianness.Little, 4, null, false);
            w.WriteBitmask(64, ulong.MaxValue);
            Equal(Hex(w.Finish()), "ffffffffffffffff", "a 64-bit bitmask is a ulong, with no pattern trick");

            try
            {
                new Writer(Endianness.Little, 4, null, false).WriteBitmask(8, 0x100);
                Check(false, "a bit past the bound is refused");
            }
            catch (WireException.OutOfRange)
            {
            }
        }

        private static void ABitsetPacksItsFieldsIntoTheAnchor()
        {
            Writer w = new Writer(Endianness.Little, 4, null, false);
            ulong bits = 0;
            bits |= w.Bitfield(1, 1, 0);
            bits |= w.Bitfield(5, 3, 1);
            bits |= w.Bitfield(0x2A, 6, 4);
            w.WriteBitset(2, bits);
            Equal(Hex(w.Finish()), "ab02", "the bitfields pack low bit first");

            try
            {
                new Writer(Endianness.Little, 4, null, false).Bitfield(8, 3, 0);
                Check(false, "a value wider than its field is refused, never masked");
            }
            catch (WireException.OutOfRange)
            {
            }
        }

        // -- MD5 ---------------------------------------------------------------

        /// <summary>RFC 1321 §A.5's own test suite.</summary>
        private static void Md5AnswersRfc1321sOwnVectors()
        {
            (string, string)[] vectors =
            {
                (string.Empty, "d41d8cd98f00b204e9800998ecf8427e"),
                ("a", "0cc175b9c0f1b6a831c399e269772661"),
                ("abc", "900150983cd24fb0d6963f7d28e17f72"),
                ("message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
                ("abcdefghijklmnopqrstuvwxyz", "c3fcd3d76192e4007dfb496cca67e13b"),
                (
                    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                    "d174ab98d277d9f5a5611c2c9f419d9f"),
                (
                    "123456789012345678901234567890123456789012345678901234567890"
                    + "12345678901234567890",
                    "57edf4a22be3c955ac49da2e2107b67a"),
            };
            foreach ((string input, string want) in vectors)
            {
                byte[] octets = System.Text.Encoding.ASCII.GetBytes(input);
                Equal(Hex(Md5.Digest(octets)), want, "MD5 of `" + input + "`");
            }
        }

        // -- binary128, against the oracle the other columns agree on ----------

        /// <summary>
        /// The 518 patterns of <c>crates/float128/tests/oracle/patterns.txt</c>.
        /// </summary>
        /// <remarks>
        /// The second column is the <b>shortest</b> form the Rust column
        /// printed and not the exact decimal, so what is asserted is the
        /// direction that form fixes: it has to read back here as the very same
        /// pattern. A parser that rounded differently would land a neighbour,
        /// and sixteen entirely plausible bytes is what that looks like on the
        /// wire. The sixteen bytes are asserted both ways round beside it.
        ///
        /// Read from the repository root, which is where <c>run-tests.sh</c>
        /// runs this.
        /// </remarks>
        private static void Float128AgreesWithTheOracle()
        {
            string path = Path.Combine("crates", "float128", "tests", "oracle", "patterns.txt");
            if (!File.Exists(path))
            {
                Console.WriteLine("FAIL the float128 oracle is not at " + Path.GetFullPath(path));
                failures++;
                return;
            }

            int checkedLines = 0;
            int reported = 0;
            foreach (string line in File.ReadLines(path))
            {
                string trimmed = line.Trim();
                if (trimmed.Length == 0 || trimmed[0] == '#')
                {
                    continue;
                }

                string[] parts = trimmed.Split(' ');
                if (parts.Length != 2)
                {
                    continue;
                }

                Float128 value = Float128.FromBytes(Unhex(parts[0]), true);
                Float128 back = Float128.FromDecimalString(parts[1]);
                if (!back.Equals(value))
                {
                    if (reported < 10)
                    {
                        Console.WriteLine(
                            "FAIL `" + parts[1] + "` read back as "
                            + Hex(back.ToBytes(true)) + ", not " + parts[0]);
                        reported++;
                    }

                    failures++;
                    continue;
                }

                if (!Float128.FromBytes(value.ToBytes(false), false).Equals(value)
                    || !Float128.FromBytes(value.ToBytes(true), true).Equals(value))
                {
                    Console.WriteLine("FAIL " + parts[0] + " did not survive its own bytes");
                    failures++;
                }

                checkedLines++;
            }

            Check(checkedLines == 518, "the oracle holds 518 patterns, not " + checkedLines);
        }

        /// <summary>
        /// The exact decimal, on the values every column prints the same text
        /// for.
        /// </summary>
        private static void TheExactDecimalIsWhatTheOtherColumnsPrint()
        {
            Equal(Float128.Zero.ToExactDecimal(), "0.0", "positive zero");
            Equal(Float128.NegativeZero.ToExactDecimal(), "-0.0", "negative zero");
            Equal(Float128.One.ToExactDecimal(), "1.0", "one");
            Equal(Float128.PositiveInfinity.ToExactDecimal(), "inf", "infinity");
            Equal(Float128.NegativeInfinity.ToExactDecimal(), "-inf", "negative infinity");
            Equal(Float128.NaN.ToExactDecimal(), "nan", "nan");
            Equal(
                Float128.FromBits(Float128.One.ToBits() | BigInteger.One).ToExactDecimal(),
                "1.0000000000000000000000000000000001925929944387235853055977942584927"
                + "318538101648215388195239938795566558837890625",
                "one plus epsilon, every digit of it");
            Equal(Float128.FromDouble(1.5).ToExactDecimal(), "1.5", "one and a half");
            Equal(Float128.FromDouble(-2.75).ToExactDecimal(), "-2.75", "minus two and three quarters");
        }

        private static void Float128CarriesWhatADoubleAndADecimalCannot()
        {
            // 1.5, measured from GCC and from RTI Connext 7.7.
            Float128 value = Float128.FromDouble(1.5);
            Equal(Hex(value.ToBytes(false)), "0000000000000000000000000080ff3f", "1.5 little-endian");
            Equal(Hex(value.ToBytes(true)), "3fff8000000000000000000000000000", "1.5 big-endian");

            // CORBA Figure 9.2: the whole sixteen bytes reversed, not two
            // eight-byte halves swapped in place.
            byte[] little = value.ToBytes(false);
            byte[] big = value.ToBytes(true);
            for (int index = 0; index < 16; index++)
            {
                Check(little[index] == big[15 - index], "byte order is a whole-value reversal");
            }

            // The three a decimal has no room for at all.
            Check(Float128.FromDecimalString("1e4000").IsFinite, "1e4000 is a finite binary128");
            Check(Float128.FromDecimalString("1e5000").IsInfinite, "1e5000 overflows to infinity");
            Check(Float128.FromDecimalString("1e-5000").IsZero, "1e-5000 underflows to zero");

            // A negative zero survives the text and is a different sixteen
            // bytes from a positive one, which is the thing no decimal type in
            // .NET can hold.
            Float128 negativeZero = Float128.FromDecimalString("-0.0");
            Check(negativeZero.IsZero && negativeZero.IsNegative, "-0.0 keeps its sign");
            Check(!negativeZero.Equals(Float128.Zero), "-0.0 is not +0.0 on the wire");
            Check(negativeZero.IeeeEquals(Float128.Zero), "-0.0 is numerically +0.0");

            // A value no double holds: 1 + 2^-100 needs 101 significand bits.
            Float128 tiny = Float128.FromBits(Float128.One.ToBits() | (BigInteger.One << 12));
            try
            {
                tiny.ToDouble();
                Check(false, "a value no double holds is refused by name");
            }
            catch (OverflowException)
            {
            }
        }

        private static void AF128MemberIsSixteenOctetsAlignedLikeTheClauseSays()
        {
            Writer w = new Writer(Endianness.Little, 4, null, false);
            w.WriteU8(1);
            w.WriteF128(Float128.One);
            byte[] octets = w.Finish();
            Equal(octets.Length, 20, "under XCDR2 a binary128 aligns to four");

            Writer eight = new Writer(Endianness.Little, 8, null, false);
            eight.WriteU8(1);
            eight.WriteF128(Float128.One);
            Equal(eight.Finish().Length, 24, "under XCDR1 it aligns to eight");

            Reader r = new Reader(octets, Endianness.Little, 4, null);
            Equal(r.ReadU8(), (byte)1, "the octet in front reads back");
            Check(r.ReadF128().Equals(Float128.One), "the binary128 reads back whole");
        }

        // -- against a foreign answer -------------------------------------------

        /// <summary>
        /// The runtime's support namespace is a superset of the one the
        /// materialiser emits.
        /// </summary>
        /// <remarks>
        /// <c>crates/lang-rules/src/adapter/csharp.rs</c> emits
        /// <c>ZeroDds.Idl.IdlNameAttribute</c> and
        /// <c>ZeroDds.Idl.Float128</c> into every generated file, so that one
        /// source is one compiler argument. The probe compiles that file
        /// <i>with</i> this assembly and drops the generated copy, because two
        /// declarations of one name is CS0101 — which is only sound while what
        /// is here has every member that one has. Asserted rather than
        /// commented: a field renamed here would leave the generated
        /// declarations compiling and the probe's not, one corpus sweep later.
        /// </remarks>
        private static void TheSupportNamespaceIsASupersetOfTheMaterialisersOwn()
        {
            System.Type float128 = typeof(Float128);
            Equal(float128.FullName, "ZeroDds.Idl.Float128", "the Float128 the codec names");
            Check(float128.IsValueType, "Float128 is a struct, as the materialiser declares it");
            foreach (string field in new[] { "Low", "High" })
            {
                var found = float128.GetField(field);
                Check(found != null && found.FieldType == typeof(ulong),
                    "Float128 carries a public ulong " + field);
            }

            Check(
                float128.GetConstructor(new[] { typeof(ulong), typeof(ulong) }) != null,
                "Float128 takes (ulong low, ulong high), as the materialiser writes it");

            System.Type attribute = typeof(IdlNameAttribute);
            Equal(
                attribute.FullName,
                "ZeroDds.Idl.IdlNameAttribute",
                "the attribute every generated member carries");
            Check(
                attribute.GetConstructor(new[] { typeof(string) }) != null,
                "IdlNameAttribute takes the IDL spelling");
            var name = attribute.GetProperty("Name");
            Check(
                name != null && name.PropertyType == typeof(string),
                "IdlNameAttribute publishes Name, which is what a hashed member ID is computed from");
        }

        /// <summary>
        /// Three pure-primitive corpus cases, written by hand here and compared
        /// with what the Rust column recorded.
        /// </summary>
        /// <remarks>
        /// The two encodings disagree about the one rule this column is most
        /// likely to have got wrong: <c>plain-cdr2.le</c> has MAXALIGN 4 and
        /// <c>plain-cdr1.be</c> has MAXALIGN 8 and the other byte order.
        /// </remarks>
        private static void CorpusCasesMatchTheRustColumn()
        {
            Dictionary<string, Action<Writer>> cases = new Dictionary<string, Action<Writer>>
            {
                ["primitive-mixed"] = w =>
                {
                    w.WriteBool(true);
                    w.WriteU8(171);
                    w.WriteU8(254);
                    w.WriteI8(-13);
                    w.WriteChar('Z');
                    w.WriteWchar(
                        'ä', WcharWidth.Octets2, WcharFraming.Fixed, WstringUnitOrder.Stream);
                    w.WriteI16(-12345);
                    w.WriteU16(48879);
                    w.WriteI32(-1234567);
                    w.WriteU32(3735928559U);
                    w.WriteI64(-987654321987L);
                    w.WriteU64(16045690984833335998UL);
                    w.WriteF32(-3.75f);
                    w.WriteF64(3.141592653589793);
                },
                ["primitive-longlong"] = w =>
                {
                    w.WriteI64(-987654321987L);
                    w.WriteI64(9223372036854775807L);
                },
                ["primitive-mixed-descending-alignment"] = w =>
                {
                    w.WriteF64(-2.5);
                    w.WriteF32(2.5f);
                    w.WriteU64(7UL);
                    w.WriteI64(9223372036854775807L);
                    w.WriteU32(5U);
                    w.WriteI32(2147483647);
                    w.WriteU16(3);
                    w.WriteI16(32767);
                    w.WriteWchar(
                        'ሴ', WcharWidth.Octets2, WcharFraming.Fixed, WstringUnitOrder.Stream);
                    w.WriteChar('~');
                    w.WriteI8(127);
                    w.WriteU8(3);
                    w.WriteU8(1);
                    w.WriteBool(false);
                },
            };

            foreach (KeyValuePair<string, Action<Writer>> entry in cases)
            {
                Compare(entry.Key, entry.Value, Encoding.PlainCdr2, Endianness.Little);
                Compare(entry.Key, entry.Value, Encoding.PlainCdr1, Endianness.Big);
            }
        }

        private static void Compare(
            string name, Action<Writer> write, Encoding encoding, Endianness order)
        {
            string file = EncodingFacts.AsString(encoding) + "." + EndiannessNames.AsString(order)
                + ".bin";
            string path = Path.Combine("tests", "interop-results", name, "zerodds-rust", file);
            if (!File.Exists(path))
            {
                Console.WriteLine("FAIL " + name + " " + file + ": not readable at "
                    + Path.GetFullPath(path));
                failures++;
                return;
            }

            byte[] expected = File.ReadAllBytes(path);
            Writer w = new Writer(order, EncodingFacts.MaxAlignment(encoding), null, false);
            write(w);
            byte[] produced = w.Finish();
            if (Hex(produced) != Hex(expected))
            {
                Console.WriteLine("FAIL " + name + " " + file);
                Console.WriteLine("     C#   " + Hex(produced));
                Console.WriteLine("     Rust " + Hex(expected));
                failures++;
            }
        }

        /// <summary>Every check, with the exit code the shell reads.</summary>
        public static int Main()
        {
            AlignmentIsCappedAtMaxalign();
            EveryPrimitiveGoesOutInBothOrders();
            ACharAboveAnOctetIsRefused();
            AStringCountsUtf8OctetsAndTheTerminator();
            AWstringCountsOctetsUnderTheSpecification();
            ABoundedStringWstringSequenceAndMapRefuseAnOverrun();
            ALoneSurrogateIsNotWritten();
            HddsAxes();
            AFixedIsPackedBcdWithASignNibble();
            AFixedCanCarryTheDigitsOfItsString();
            ADheaderCarriesWhatFollowedIt();
            EveryEmheaderLengthCodeFramesItsMember();
            AParameterHeaderPromotesWhenItsValueOverruns();
            AParameterValueCountsAlignmentFromItsOwnStart();
            AnEnumHolderIsSignedAndAsWideAsItsBound();
            ABitmaskTakesTheHolderTable33Gives();
            ABitsetPacksItsFieldsIntoTheAnchor();
            Md5AnswersRfc1321sOwnVectors();
            Float128CarriesWhatADoubleAndADecimalCannot();
            TheExactDecimalIsWhatTheOtherColumnsPrint();
            AF128MemberIsSixteenOctetsAlignedLikeTheClauseSays();
            Float128AgreesWithTheOracle();
            TheSupportNamespaceIsASupersetOfTheMaterialisersOwn();
            CorpusCasesMatchTheRustColumn();

            if (failures != 0)
            {
                Console.WriteLine(failures + " check(s) failed");
                return 1;
            }

            return 0;
        }
    }
}
