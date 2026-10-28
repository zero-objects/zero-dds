# SPDX-License-Identifier: Apache-2.0
# Copyright 2026 ZeroDDS Contributors
"""The Python wire runtime against the clauses it implements.

Each test names the clause and, where one exists, the measured bytes in
`crates/ast-layout/tests/fixtures/vendor_bytes.md`. The tests that carry
no vendor row are the ones about Python itself — an unbounded `int`, a
`str` whose length is not its byte count, a `float` that is a double —
and those are where this runtime can differ from `crates/wire` without
either of them being wrong about the specification.
"""

from __future__ import annotations

import dataclasses
import hashlib
import unittest

from zerodds_wire import (
    EmHeaderRead,
    Encoding,
    Endianness,
    Float128,
    ImpossibleLength,
    Inexact,
    OutOfRange,
    ParameterRole,
    PlCdr1Framing,
    Reader,
    ReaderMark,
    Status,
    Truncated,
    Writer,
    encapsulation_header,
    encapsulation_padding,
    no_dheader,
)
from zerodds_wire.values import Mismatch, as_fixed, finite
from zerodds_wire.typesupport import (
    Discarded,
    TryConstruct,
    trim_utf16,
    trim_utf8,
    utf16_units,
    utf8_octets,
)
from zerodds_wire.writer import fixed_nibbles, wstring_unit_width


class Alignment(unittest.TestCase):
    """XTypes 1.3 §7.4.1: ALIGN(N) aligns to MIN(N, XCDR.maxalign)."""

    def test_a_long_after_a_char_sits_at_four(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_char("Z")
        writer.write_i32(1)
        self.assertEqual(writer.finish().hex(), "5a00000001000000")

    def test_maxalign_is_four_under_xcdr2_and_eight_under_xcdr1(self) -> None:
        # `@final struct Point { long x; double y; }` with x=1, y=2.0.
        # Cyclone, RTI and Fast DDS all wrote twelve bytes under XCDR2
        # and sixteen under XCDR1.
        for max_align, expected in (
            (4, "010000000000000000000040"),
            (8, "01000000000000000000000000000040"),
        ):
            writer = Writer(Endianness.LITTLE, max_align)
            writer.write_i32(1)
            writer.write_f64(2.0)
            self.assertEqual(writer.finish().hex(), expected, f"maxalign {max_align}")

    def test_a_structure_has_no_trailing_padding(self) -> None:
        # `@final struct Odd { long a; short b; }` — six bytes, not eight.
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_i32(1)
        writer.write_i16(2)
        self.assertEqual(writer.finish().hex(), "010000000200")


class Integers(unittest.TestCase):
    """A Python `int` is unbounded and the wire is not."""

    def test_a_value_its_field_cannot_hold_is_refused_and_not_wrapped(self) -> None:
        for method, value in (
            ("write_u8", 256),
            ("write_i8", 128),
            ("write_i16", 32768),
            ("write_u16", -1),
            ("write_i32", 2**31),
            ("write_u32", 2**32),
            ("write_i64", 2**63),
            ("write_u64", 2**64),
        ):
            writer = Writer(Endianness.LITTLE, 4)
            with self.assertRaises(OutOfRange, msg=f"{method}({value})"):
                getattr(writer, method)(value)

    def test_a_negative_value_is_its_twos_complement(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_i32(-1234567)
        writer.write_i32(2147483647)
        self.assertEqual(writer.finish().hex(), "7929edffffffff7f")

    def test_byte_order_reverses_each_value_and_not_the_stream(self) -> None:
        writer = Writer(Endianness.BIG, 4)
        writer.write_i32(-1234567)
        writer.write_i32(2147483647)
        self.assertEqual(writer.finish().hex(), "ffed29797fffffff")


class Strings(unittest.TestCase):
    """XTypes 1.3 §7.4.1.1 and §7.4.1.1.2."""

    def test_the_length_counts_bytes_and_the_terminator(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_string("abc")
        self.assertEqual(writer.finish().hex(), "0400000061626300")

    def test_a_multibyte_character_counts_its_bytes_and_not_itself(self) -> None:
        # `len("ä")` is 1 in Python and 2 on the wire. Getting this
        # wrong produces a length that is right for ASCII and wrong for
        # every sample in the corpus.
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_string("ä")
        self.assertEqual(writer.finish().hex(), "03000000c3a400")

    def test_a_wstring_length_is_bytes_of_utf16_with_no_bom_and_no_nul(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_wstring("Aä")
        self.assertEqual(writer.finish().hex(), "0400000041 00e400".replace(" ", ""))

    def test_a_character_outside_the_bmp_is_two_units_and_four_bytes(self) -> None:
        # U+1D11E, the clause's own rationale for counting bytes.
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_wstring("\U0001d11e")
        self.assertEqual(writer.finish().hex(), "0400000034d81edd")

    def test_an_empty_wstring_is_a_zero_length_and_nothing_else(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_wstring("")
        self.assertEqual(writer.finish().hex(), "00000000")

    def test_a_lone_surrogate_is_refused(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        with self.assertRaises(OutOfRange):
            writer.write_wstring("\ud800")

    def test_a_narrow_char_above_u00ff_is_refused_and_not_truncated(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        with self.assertRaises(OutOfRange):
            writer.write_char("Ā")

    def test_a_bounded_string_over_its_bound_is_refused(self) -> None:
        # An over-bound value is refused rather than cut at a character
        # boundary: the bound is a promise about the source and not a
        # difference on the wire, and cutting it would put a sample on
        # the wire nobody wrote. Three two-byte characters, bounded at
        # five octets: the third would straddle the bound.
        writer = Writer(Endianness.LITTLE, 4)
        with self.assertRaises(OutOfRange):
            writer.write_string_bounded("äää", 5)
        self.assertEqual(writer.finish(), b"")  # nothing was written

    def test_a_bounded_string_exactly_at_its_bound_encodes(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_string_bounded("ää", 4)
        self.assertEqual(writer.finish().hex(), "05000000c3a4c3a400")

    def test_a_bounded_wstring_over_its_bound_is_refused(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        with self.assertRaises(OutOfRange):
            writer.write_wstring_bounded("ab", 1)

    def test_a_bounded_wstring_exactly_at_its_bound_encodes(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_wstring_bounded("a", 1)
        self.assertEqual(writer.finish().hex(), "020000006100")

    def test_a_bounded_sequence_count_over_its_bound_is_refused(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        with self.assertRaises(OutOfRange):
            writer.check_sequence_bound(4, 3)

    def test_a_bounded_sequence_count_exactly_at_its_bound_does_not_raise(self) -> None:
        Writer(Endianness.LITTLE, 4).check_sequence_bound(3, 3)

    def test_a_bounded_map_count_over_its_bound_is_refused(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        with self.assertRaises(OutOfRange):
            writer.check_map_bound(3, 2)

    def test_a_bounded_map_count_exactly_at_its_bound_does_not_raise(self) -> None:
        Writer(Endianness.LITTLE, 4).check_map_bound(2, 2)


class Floats(unittest.TestCase):
    """A Python `float` is a double; a `float` field is not."""

    def test_a_float_field_is_rounded_to_single_precision_once(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_f32(3.14159)
        self.assertEqual(writer.finish().hex(), "d00f4940")

    def test_a_magnitude_no_float_can_hold_becomes_an_infinity(self) -> None:
        # What a C `(float)` cast produces and what a peer reads back.
        for value, expected in ((1e39, "0000807f"), (-1e39, "000080ff")):
            writer = Writer(Endianness.LITTLE, 4)
            writer.write_f32(value)
            self.assertEqual(writer.finish().hex(), expected, str(value))

    def test_an_infinity_has_no_json_form_and_is_refused_by_the_path(self) -> None:
        # The infinity above is a value the wire carries and JSON does
        # not: RFC 8259 §6 has no NaN and no infinity. `json.dumps`
        # writes `Infinity` regardless, which no conforming parser reads
        # back, so `to_json` asks here before it hands one out. The Rust
        # column refuses at the same point — `canonical::number_f64`.
        self.assertEqual(finite(1.5, "sample.reading"), 1.5)
        self.assertEqual(finite(-0.0, "sample.reading"), -0.0)
        for value in (float("nan"), float("inf"), float("-inf")):
            with self.assertRaises(Mismatch) as caught:
                finite(value, "sample.reading")
            self.assertIn("sample.reading", str(caught.exception))
            self.assertIn("finite", str(caught.exception))

    def test_a_long_double_is_sixteen_bytes_of_binary128(self) -> None:
        # This used to assert eight bytes of double and eight zeros,
        # which was the right length and the wrong value. The pattern for
        # 1.5 was measured from GCC's `__float128` and from RTI Connext
        # 7.7's own serialiser; `docs/float128-column.md` §2 has the runs.
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_f128(Float128.from_float(1.5))
        self.assertEqual(writer.finish().hex(), "0000000000000000000000000080ff3f")

        writer = Writer(Endianness.BIG, 4)
        writer.write_f128(Float128.from_float(1.5))
        self.assertEqual(writer.finish().hex(), "3fff8000000000000000000000000000")

    def test_a_plain_float_widens_exactly(self) -> None:
        # Every double is a binary128, so the convenience of passing one
        # costs nothing. 2.0 is exponent 16384 and an empty fraction.
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_f128(2.0)
        self.assertEqual(writer.finish().hex(), "00" * 15 + "40")

    def test_a_value_no_double_can_hold_survives_the_round_trip(self) -> None:
        # 1 + 2^-100 needs 101 significand bits: past a double at 53 and
        # past x87 extended at 64, so it is the value that tells a real
        # binary128 column from a padded one.
        wide = Float128((0x3FFF << 112) | (1 << 12))
        writer = Writer(Endianness.BIG, 8)
        writer.write_f128(wide)
        payload = writer.finish()
        self.assertEqual(Reader(payload, Endianness.BIG, 8).read_f128(), wide)
        with self.assertRaises(Inexact):
            wide.to_float()

    def test_the_exact_decimal_is_what_the_other_columns_print(self) -> None:
        # Byte for byte the same text `crates/float128` and
        # `runtime/c/src/float128_decimal.c` produce for the same
        # pattern; the three columns are compared as text.
        self.assertEqual(Float128.from_float(1.5).to_exact_decimal(), "1.5")
        self.assertEqual(Float128.from_float(-2.75).to_exact_decimal(), "-2.75")
        self.assertEqual(Float128.zero(negative=True).to_exact_decimal(), "-0.0")
        self.assertEqual(
            Float128((0x3FFF << 112) | 1).to_exact_decimal(),
            "1.000000000000000000000000000000000192592994438723585305597794258492"
            "7318538101648215388195239938795566558837890625",
        )

    def test_the_two_byte_orders_are_the_whole_value_reversed(self) -> None:
        # CORBA 3.3 part 2 Figure 9.2, and not two eight-byte halves
        # swapped in place — which is what the old writer would have
        # produced had it written both halves.
        wide = Float128.from_float(-2.75)
        self.assertEqual(
            wide.to_bytes_ordered("little")[::-1], wide.to_bytes_ordered("big")
        )


class Fixed(unittest.TestCase):
    """[CDR] §9.3.2.8 — the only wire definition an IDL `fixed` has."""

    def test_the_digits_pack_two_to_the_byte_with_a_sign_nibble(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_fixed("123.45", 5, 2)
        self.assertEqual(writer.finish().hex(), "12345c")

    def test_a_negative_value_ends_in_0xd(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_fixed("-123.45", 5, 2)
        self.assertEqual(writer.finish().hex(), "12345d")

    def test_a_negative_zero_is_written_positive(self) -> None:
        self.assertEqual(fixed_nibbles("-0.00", 5, 2)[-1], 0x0C)

    def test_an_even_digit_count_gets_a_leading_zero_nibble(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_fixed("12.34", 4, 2)
        self.assertEqual(writer.finish().hex(), "01234c")

    def test_a_value_too_long_for_its_type_is_refused(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        with self.assertRaises(OutOfRange):
            writer.write_fixed("1234.5", 5, 2)

    def test_what_comes_back_is_the_value_and_not_the_padded_field(self) -> None:
        """The BCD field is `digits` wide; the value is not.

        `0.01` in a `fixed<5, 2>` spells `00001`, and returning the whole
        part as `000` put a decimal on the read side that the write side
        would never produce from the same value. Both other columns
        answer `0.01`; `crates/wire-probe/src/canonical.rs` is where that
        form is defined.
        """
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_fixed("0.01", 5, 2)
        reader = Reader(writer.finish(), Endianness.LITTLE, 4)
        self.assertEqual(reader.read_fixed(5, 2), "0.01")

    def test_the_value_string_digit_count_is_what_ace_writes(self) -> None:
        """ACE/TAO 2.5.24's `ACE_CDR::Fixed::from_string`, recorded in
        `crates/ast-layout/tests/fixtures/unspecified_constructs.md` §3:
        the integer digits as written plus S, made odd by one leading zero."""
        recorded = [
            ("123.45", 5, 2, "12345c"),
            ("-123.45", 5, 2, "12345d"),
            ("12.34", 4, 2, "01234c"),
            ("7", 1, 0, "7c"),
            ("9" * 31, 31, 0, "9" * 31 + "c"),
            ("0.12345", 5, 5, "0012345c"),
            ("0.00", 5, 2, "000c"),
            # The scale and not the fraction as written: one digit
            # counted would keep only the last nibble, `0c`.
            ("7", 5, 2, "700c"),
        ]
        for text, digits, scale, octets in recorded:
            writer = Writer(Endianness.LITTLE, 4)
            writer.write_fixed(text, digits, scale, "value-string-digits")
            self.assertEqual(writer.finish().hex(), octets, text)
            reader = Reader(bytes.fromhex(octets), Endianness.LITTLE, 4)
            self.assertEqual(
                reader.read_fixed(digits, scale, "value-string-digits"),
                as_fixed(text, "sample", digits, scale).lstrip("+"),
                text,
            )

    def test_a_value_string_past_31_digits_is_refused(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        with self.assertRaises(OutOfRange):
            writer.write_fixed("0" * 31 + "1", 31, 0, "value-string-digits")

    def test_a_negative_zero_reads_back_without_its_sign(self) -> None:
        reader = Reader(bytes.fromhex("00000d"), Endianness.LITTLE, 4)
        self.assertEqual(reader.read_fixed(5, 2), "0.00")

    def test_from_json_normalises_a_decimal_the_way_the_reader_returns_it(self) -> None:
        """Otherwise a `fixed` member is unequal to itself across a
        write and a read, in a binding whose values are `dataclasses`
        and whose users compare them with `==`."""
        self.assertEqual(as_fixed("-000000.001", "sample.debt", 9, 3), "-0.001")
        self.assertEqual(as_fixed("0.01", "sample.amount", 5, 2), "0.01")
        self.assertEqual(as_fixed("7", "sample.amount", 5, 2), "7.00")
        self.assertEqual(as_fixed("-0.00", "sample.amount", 5, 2), "0.00")

    def test_from_json_hands_a_non_decimal_to_the_writer_unchanged(self) -> None:
        """One validator and not two: the refusal has to keep coming
        from `Writer.write_fixed` with the message it always had."""
        self.assertEqual(as_fixed("1234.5", "sample.amount", 5, 2), "1234.5")
        self.assertEqual(as_fixed("nought", "sample.amount", 5, 2), "nought")
        writer = Writer(Endianness.LITTLE, 4)
        with self.assertRaises(OutOfRange):
            writer.write_fixed(as_fixed("1234.5", "sample.amount", 5, 2), 5, 2)


class Framing(unittest.TestCase):
    """DHEADER, EMHEADER and the PL_CDR1 parameter list."""

    def test_a_dheader_is_patched_with_what_followed_it(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        header = writer.write_dheader()
        writer.write_i32(7)
        writer.write_f64(1.5)
        writer.write_i16(-3)
        writer.finish_dheader(header)
        # `@appendable { long a; double b; short c; }` — Cyclone, RTI and
        # Fast DDS all wrote this delimiter and these fourteen bytes.
        self.assertEqual(writer.finish().hex(), "0e00000007000000000000000000f83ffdff")

    def test_length_code_four_reserves_a_nextint_and_five_does_not(self) -> None:
        # §7.4.3.4.2: for codes 5 to 7 the NEXTINT "shall be reused also
        # as part of the serialized member", so reserving one would write
        # the length twice and put every member after it at the wrong
        # offset.
        four = Writer(Endianness.LITTLE, 4)
        header = four.write_emheader(0, 4, False)
        four.write_i32(7)
        four.finish_emheader(header)
        self.assertEqual(four.finish().hex(), "000000400400000007000000")

        five = Writer(Endianness.LITTLE, 4)
        header = five.write_emheader(0, 5, False)
        five.write_string("abc")
        five.finish_emheader(header)
        self.assertEqual(five.finish().hex(), "0000005004000000" + "61626300")

    def test_the_must_understand_flag_is_the_top_bit(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_emheader(3, 2, True)
        self.assertEqual(writer.finish().hex(), "030000a0")

    def test_a_parameter_value_counts_alignment_from_its_own_block(self) -> None:
        # §7.4.1.2: "Each element, or parameter, within a parameter list
        # data structure is simply a CDR-encapsulated block of data." A
        # double at the head of one is already aligned, so the declared
        # length is 8 and not 12 — `01000800`, which is what Cyclone, RTI
        # and Fast DDS all write.
        writer = Writer(Endianness.LITTLE, 8)
        writer.write_u8(0xAB)
        header = writer.write_pid(1)
        writer.write_f64(1.5)
        writer.finish_pid(header)
        produced = writer.finish().hex()
        self.assertTrue(produced.startswith("ab00000001000800"), produced)

    def test_a_sentinel_ends_the_parameter_list(self) -> None:
        # `027f0000`, not `023f0000`: the default framing is Cyclone's,
        # which sets the must-understand bit on everything it writes.
        # Fast DDS is the one that leaves it clear.
        writer = Writer(Endianness.LITTLE, 8)
        writer.write_sentinel()
        self.assertEqual(writer.finish().hex(), "027f0000")

        plain = Writer(
            Endianness.LITTLE,
            8,
            PlCdr1Framing(
                pid_flags="bare",
                pad_declared_length=False,
            ),
        )
        plain.write_sentinel()
        self.assertEqual(plain.finish().hex(), "023f0000")

    def test_the_extended_parameter_header_states_eight_then_the_wide_pair(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        header = writer.write_pid_extended(0x4000)
        writer.write_i32(7)
        writer.finish_pid(header)
        self.assertEqual(writer.finish().hex(), "017f08000040000004000000" + "07000000")

    def test_the_declared_length_covers_the_padding_only_where_the_framing_says(
        self,
    ) -> None:
        # `short c` in `vendor_bytes.md`'s `ext_mutable`: Cyclone and RTI
        # declare four, Fast DDS declares two and leaves the padding out.
        # XTypes 1.3 §7.4.1.2: the default declares the exact length.
        for framing, expected in (
            (PlCdr1Framing(), "0200"),
            (PlCdr1Framing(pad_declared_length=True), "0400"),
            (
                PlCdr1Framing(
                    pid_flags="bare",
                    pad_declared_length=False,
                ),
                "0200",
            ),
        ):
            writer = Writer(Endianness.LITTLE, 8, framing)
            header = writer.write_pid(2)
            writer.write_i16(-3)
            writer.finish_pid(header)
            self.assertEqual(writer.finish().hex()[4:8], expected)


class ReservedParameterIds(unittest.TestCase):
    """§7.4.1.2.2: the flag bit, the reserved range and PID_IGNORE.

    Bit 15 is FLAG_IMPL_EXTENSION and a user-defined member "will never
    set" it; 0x3F01-0x3FFF are reserved; and of 0x3F03 the clause says
    "all consumers of this Data Representation shall ignore parameters
    with this ID". A reader that masked all three off with the rest of
    the flags read a foreign implementation's private parameter as the
    member with the same low fourteen bits.
    """

    def test_flag_impl_extension_is_not_a_member_of_this_type(self) -> None:
        r = Reader(bytes.fromhex("02800400efbeadde"), Endianness.LITTLE, 8)
        header = r.read_pid()
        self.assertEqual(header.pid, 2)
        self.assertTrue(header.impl_extension)
        self.assertFalse(header.is_user_member())

    def test_pid_ignore_outranks_the_must_understand_bit(self) -> None:
        # 0x7F03: PID_IGNORE with must-understand set, which is the case
        # the clause's "shall ignore" decides.
        r = Reader(bytes.fromhex("037f040011223344"), Endianness.LITTLE, 8)
        header = r.read_pid()
        self.assertTrue(header.ignore)
        self.assertTrue(header.must_understand)
        self.assertFalse(header.is_user_member())

    def test_a_reserved_id_is_not_a_member_of_this_type(self) -> None:
        # A member *can* legally carry id 0x3F0C — through the extended
        # header, where 28 bits leave no reserved range — so a short
        # parameter with that id and a real member were the same thing
        # to a reader that only compared the number.
        r = Reader(bytes.fromhex("0c3f040055667788"), Endianness.LITTLE, 8)
        header = r.read_pid()
        self.assertTrue(header.reserved)
        self.assertFalse(header.is_user_member())

    def test_an_ordinary_member_is_still_a_member(self) -> None:
        r = Reader(bytes.fromhex("020004002a000000"), Endianness.LITTLE, 8)
        header = r.read_pid()
        self.assertEqual(header.pid, 2)
        self.assertTrue(header.is_user_member())
        self.assertFalse(header.impl_extension)
        self.assertFalse(header.reserved)


class BitBounded(unittest.TestCase):
    """Enumerations, bitmasks and bitsets."""

    def test_a_negative_enumerator_is_its_twos_complement(self) -> None:
        # RTI Connext 7.7 wrote `ffffffff` for `@value(-1)`.
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_enum(-1, 32)
        self.assertEqual(writer.finish().hex(), "ffffffff")

    def test_a_narrow_bound_holds_the_full_width(self) -> None:
        # Cyclone wrote `c8` for `@value(200)` under `@bit_bound(8)`.
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_enum(200, 8)
        self.assertEqual(writer.finish().hex(), "c8")

    def test_an_enumerator_wider_than_its_bound_is_refused(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        with self.assertRaises(OutOfRange):
            writer.write_enum(256, 8)

    def test_a_bitmask_holder_follows_table_33(self) -> None:
        for bound, expected in ((8, "05"), (16, "0500"), (32, "05000000"), (64, "0500000000000000")):
            writer = Writer(Endianness.LITTLE, 4)
            writer.write_bitmask(0b101, bound)
            self.assertEqual(writer.finish().hex(), expected, f"bound {bound}")


class KeyHash(unittest.TestCase):
    """DDSI-RTPS 2.5 §9.6.3.8."""

    def test_a_short_holder_is_the_hash_zero_padded(self) -> None:
        holder = Writer(Endianness.BIG, 4)
        holder.write_i32(-1234567)
        self.assertEqual(holder.zero_padded().hex(), "ffed2979" + "00" * 12)

    def test_a_long_holder_is_hashed(self) -> None:
        holder = Writer(Endianness.BIG, 4)
        for value in range(8):
            holder.write_i64(value)
        self.assertEqual(holder.md5(), hashlib.md5(holder.as_bytes()).digest())


class Encapsulation(unittest.TestCase):
    """DDSI-RTPS 2.5 §10.5 — the identifier is big-endian whatever the
    payload behind it is."""

    def test_the_identifiers_are_the_ones_rti_put_in_front_of_its_samples(self) -> None:
        self.assertEqual(
            encapsulation_header(Encoding.PLAIN_CDR1, Endianness.LITTLE).hex(), "00010000"
        )
        self.assertEqual(
            encapsulation_header(Encoding.PLAIN_CDR2, Endianness.LITTLE).hex(), "00070000"
        )
        self.assertEqual(
            encapsulation_header(Encoding.DELIMITED_CDR, Endianness.LITTLE).hex(), "00090000"
        )
        self.assertEqual(encapsulation_header(Encoding.PL_CDR2, Endianness.BIG).hex(), "000a0000")

    def test_the_options_field_carries_the_payloads_padding_count(self) -> None:
        """XTypes 1.3 §7.6.3.1.2: the low two bits of the second options
        byte are the number of padding bytes at the end of the serialized
        data, and a receiver "shall" use them to find where it ended.

        Measured on the loopback wire on 2026-08-09: a one-byte payload
        goes out from Cyclone, RTI and Fast DDS alike as `0001 0003
        ab000000`.
        """
        for length, pad in ((1, 3), (2, 2), (3, 1), (4, 0), (30, 2)):
            self.assertEqual(encapsulation_padding(length), pad, length)
        self.assertEqual(
            encapsulation_header(Encoding.PLAIN_CDR1, Endianness.LITTLE, 3).hex(),
            "00010003",
        )
        # Only two bits wide, so a count of four is a count of zero —
        # and a payload already on the boundary pads by nothing.
        self.assertEqual(
            encapsulation_header(Encoding.PLAIN_CDR1, Endianness.LITTLE, 4).hex(),
            "00010000",
        )

    def test_a_typo_in_an_encoding_name_is_none_and_not_a_default(self) -> None:
        self.assertIsNone(Encoding.parse("XCDR2"))
        self.assertIsNone(Endianness.parse("LE"))
        self.assertIs(Encoding.parse("xcdr2"), Encoding.PLAIN_CDR2)


class ParameterHeaderPromotion(unittest.TestCase):
    """A short parameter header whose value turns out not to fit one.

    §7.4.1.2.2's `ssize` is sixteen bits. Which members overrun it is a
    property of the sample and not of the type, so the plan writes the
    short form for an unbounded member and the writer falls back. RTI
    and Fast DDS write the short form for `mut_string`'s `string s`, 24
    bytes against the 32 an always-extended plan produced.
    """

    def test_a_value_past_sixteen_bits_rewrites_the_header(self) -> None:
        # `pid_flags="bare"` is Fast DDS's, which is what leaves the
        # promoted marker as a plain `3f01`.
        writer = Writer(Endianness.LITTLE, 8, PlCdr1Framing(pid_flags="bare"))
        header = writer.write_pid(1)
        writer._out.extend(b"\xab" * 0x10000)  # noqa: SLF001
        writer.finish_pid(header)
        out = writer.finish()
        self.assertEqual(out[:4].hex(), "013f0800")
        self.assertEqual(out[4:8].hex(), "01000000")
        self.assertEqual(out[8:12].hex(), "00000100")
        self.assertEqual(len(out), 12 + 0x10000)

    def test_a_value_that_fits_keeps_the_short_header(self) -> None:
        writer = Writer(Endianness.LITTLE, 8, PlCdr1Framing(pad_declared_length=False))
        header = writer.write_pid(1)
        writer._out.extend(b"\xab" * 0xFFFF)  # noqa: SLF001
        writer.finish_pid(header)
        out = writer.finish()
        self.assertEqual(out[:4].hex(), "0100ffff")


class UnswappedUnits(unittest.TestCase):
    """Cyclone's big-endian `wstring`, which no dialect defaults to.

    Measured 2026-08-09 through `/root/byte-gate/gate-be`, case
    `wstring_member`, `w = "ab"`: Cyclone writes `00000004 61006200`
    where RTI and Fast DDS write `00000004 00610062`. The length word
    swaps and the code units do not, so a conforming peer reads U+6100
    U+6200 — two well-formed characters of the right count that nobody
    sent.
    """

    def test_the_length_swaps_and_the_content_does_not(self) -> None:
        writer = Writer(Endianness.BIG, 4)
        writer.write_wstring("ab", "specification", "host-unswapped")
        self.assertEqual(writer.finish().hex(), "0000000461006200")

    def test_little_endian_tells_the_two_forms_apart_in_nothing(self) -> None:
        for order in ("stream", "host-unswapped"):
            writer = Writer(Endianness.LITTLE, 4)
            writer.write_wstring("ab", "specification", order)
            self.assertEqual(writer.finish().hex(), "0400000061006200", order)

    def test_the_conforming_read_of_cyclones_bytes_is_a_different_string(self) -> None:
        raw = bytes.fromhex("0000000461006200")
        reader = Reader(raw, Endianness.BIG, 4)
        self.assertEqual(reader.read_wstring("specification"), "\u6100\u6200")
        reader = Reader(raw, Endianness.BIG, 4)
        self.assertEqual(
            reader.read_wstring("specification", "host-unswapped"), "ab"
        )

    def test_the_ros_cell_is_reachable_and_was_not_before(self) -> None:
        """Characters, no terminator, four-octet units.

        The cell the enumeration could not name. Both ROS type supports
        write it, measured 2026-08-12: "aZ" is a count of two over two
        four-octet units.
        """
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_wstring("ab", "ros-type-support")
        self.assertEqual(writer.finish().hex(), "020000006100000062000000")
        # And by coordinate, which is how every cell is reachable.
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_wstring("ab", "characters/unterminated/4")
        self.assertEqual(writer.finish().hex(), "020000006100000062000000")



class CorbaWideCharacters(unittest.TestCase):
    """The three CORBA ORB forms, in the third column.

    Measured 2026-08-10 on the Linux test host through
    `/root/corba-oracle/tools/wireprobe-{omniorb,tao,jacorb}` — omniORB
    4.3.3, TAO 2.5.24, JacORB 3.9. CORBA 3.3 part 2 §9.3.1.6 has two
    wide-character encodings, one per GIOP version, and XTypes Table 31
    replaced one half of the pair; these are the other half.
    `crates/wire-probe/tests/corba_wide_characters.rs` is the same run in
    the Rust column and carries the argument.
    """

    #: `"a<U+1D11E>b"` — one BMP character, one surrogate pair, one more.
    TEXT = "a\U0001d11eb"

    def test_omniorbs_wstring_carries_a_byte_order_mark(self) -> None:
        writer = Writer(Endianness.LITTLE, 8)
        writer.write_wstring(self.TEXT, "specification", "byte-order-mark")
        self.assertEqual(writer.finish().hex(), "0a000000fffe610034d81edd6200")

    def test_only_the_length_word_swaps_so_the_mark_says_so(self) -> None:
        # The mark and the units stay host order in a big-endian stream,
        # so the mark reads as FFFE — the signal that the units behind
        # it are the other way round. omniORB's output is
        # self-describing where Cyclone's identically-ordered output is
        # not.
        writer = Writer(Endianness.BIG, 8)
        writer.write_wstring(self.TEXT, "specification", "byte-order-mark")
        self.assertEqual(writer.finish().hex(), "0000000afffe610034d81edd6200")

    def test_an_empty_wstring_carries_no_mark(self) -> None:
        # Measured: omniORB writes `00000000` for one, byte-identical to
        # every other implementation.
        for endianness in (Endianness.LITTLE, Endianness.BIG):
            writer = Writer(endianness, 8)
            writer.write_wstring("", "specification", "byte-order-mark")
            self.assertEqual(writer.finish().hex(), "00000000", str(endianness))

    def test_taos_wstring_counts_octets_over_four_octet_units(self) -> None:
        # GIOP 1.2's length rule over a four-octet transmission code set
        # — the one form no DDS implementation writes. The units are
        # UTF-16 code units zero-extended, not UCS-4 code points, so the
        # surrogate pair survives.
        writer = Writer(Endianness.LITTLE, 8)
        writer.write_wstring(self.TEXT, "specification-wide-units", "stream")
        self.assertEqual(
            writer.finish().hex(), "100000006100000034d800001edd000062000000"
        )

    def test_jacorbs_units_are_big_endian_whatever_the_stream_says(self) -> None:
        writer = Writer(Endianness.LITTLE, 8)
        writer.write_wstring(self.TEXT, "specification", "big-endian-always")
        self.assertEqual(writer.finish().hex(), "080000000061d834dd1e0062")

    def test_every_orb_wchar_is_the_giop_length_prefixed_form(self) -> None:
        # omniORB and JacORB write the octets big-endian in a
        # *little-endian* stream: a scalar cannot carry a mark, so
        # CORBA's default without one applies. TAO's four octets are the
        # stream's order.
        for order, expected in (
            ("byte-order-mark", "0200e4021234"),
            ("big-endian-always", "0200e4021234"),
        ):
            writer = Writer(Endianness.LITTLE, 8)
            writer.write_wchar("\u00e4", 2, "giop-length-prefixed", order)
            writer.write_wchar("\u1234", 2, "giop-length-prefixed", order)
            self.assertEqual(writer.finish().hex(), expected, order)
        writer = Writer(Endianness.LITTLE, 8)
        for character in ("\u00e4", "\u1234"):
            writer.write_wchar(character, 4, "giop-length-prefixed", "stream")
        self.assertEqual(writer.finish().hex(), "04e40000000434120000")

    def test_the_giop_framing_aligns_a_wchar_to_one(self) -> None:
        # One octet longer *and* aligned to one, so every member behind
        # it moves: an `octet` then a `wchar` is three octets here and
        # four under Table 31.
        writer = Writer(Endianness.LITTLE, 8)
        writer.write_u8(1)
        writer.write_wchar("a", 2, "giop-length-prefixed", "byte-order-mark")
        writer.write_i32(7)
        self.assertEqual(writer.finish().hex(), "010200610700000 0".replace(" ", ""))

    def test_every_orb_form_reads_back(self) -> None:
        cases = (
            ("0a000000fffe610034d81edd6200", "specification", "byte-order-mark"),
            (
                "100000006100000034d800001edd000062000000",
                "specification-wide-units",
                "stream",
            ),
            ("080000000061d834dd1e0062", "specification", "big-endian-always"),
        )
        for raw, policy, order in cases:
            reader = Reader(bytes.fromhex(raw), Endianness.LITTLE, 8)
            self.assertEqual(reader.read_wstring(policy, order), self.TEXT, raw)

    def test_a_wstring_with_no_mark_is_big_endian(self) -> None:
        # CORBA's rule for a GIOP 1.2 `wstring` that carries none, and
        # what makes this setting read a JacORB peer as well as an
        # omniORB one.
        reader = Reader(bytes.fromhex("0400000000610062"), Endianness.LITTLE, 8)
        self.assertEqual(reader.read_wstring("specification", "byte-order-mark"), "ab")

    def test_a_giop_wchar_count_that_is_not_the_widths_is_refused(self) -> None:
        # A bare two-octet `wchar` read this way starts with 0xE4, which
        # would otherwise consume 228 octets of whatever followed. The
        # length is the one field a peer controls.
        reader = Reader(bytes.fromhex("e4003412"), Endianness.LITTLE, 8)
        with self.assertRaises(OutOfRange):
            reader.read_wchar(2, "giop-length-prefixed", "byte-order-mark")


class WcharWidth(unittest.TestCase):
    """A `Char16` is two bytes, or four under RTI Connext's XCDR1 form.

    Measured 2026-08-09, case `wchar_member`,
    `@final struct WCh { wchar c; octet tail; }` with `c = U+0061`: RTI
    writes `61000000 ab` under XCDR1 and `6100 ab` under XCDR2, which is
    exactly the split its `wstring` characters take. Cyclone and Fast
    DDS write `6100 ab` everywhere.
    """

    def test_the_wide_form_is_the_one_rti_writes_under_xcdr1(self) -> None:
        for endianness, expected in (
            (Endianness.LITTLE, "61000000ab"),
            (Endianness.BIG, "00000061ab"),
        ):
            writer = Writer(endianness, 8)
            writer.write_wchar("a", 4)
            writer.write_u8(0xAB)
            self.assertEqual(writer.finish().hex(), expected)

    def test_the_narrow_form_is_what_the_other_two_write(self) -> None:
        writer = Writer(Endianness.LITTLE, 8)
        writer.write_wchar("a")
        writer.write_u8(0xAB)
        self.assertEqual(writer.finish().hex(), "6100ab")

    def test_the_wide_form_aligns_to_four(self) -> None:
        writer = Writer(Endianness.LITTLE, 8)
        writer.write_u8(1)
        writer.write_wchar("a", 4)
        self.assertEqual(writer.finish().hex(), "0100000061000000")

    def test_a_wide_character_reads_back_and_a_wider_one_is_refused(self) -> None:
        reader = Reader(bytes.fromhex("61000000ab"), Endianness.LITTLE, 8)
        self.assertEqual(reader.read_wchar(4), "a")
        self.assertEqual(reader.read_u8(), 0xAB)

        reader = Reader(bytes.fromhex("1ed10100"), Endianness.LITTLE, 8)
        with self.assertRaises(OutOfRange):
            reader.read_wchar(4)

    def test_a_policy_that_is_not_one_is_refused(self) -> None:
        with self.assertRaises(OutOfRange):
            wstring_unit_width("byte-count")

    def test_a_width_that_is_not_two_or_four_is_refused(self) -> None:
        # The scalar width is an argument of its own since
        # `rmw_cyclonedds_cpp` was fitted — two octets for a `wchar`
        # member beside four-octet `wstring` units — so a caller can
        # name a width the wire has no form for, and it is refused
        # rather than written.
        writer = Writer(Endianness.LITTLE, 8)
        with self.assertRaises(OutOfRange):
            writer.write_wchar("a", 3)


class Reading(unittest.TestCase):
    """The reader against the writer, and against a stream that lies."""

    def test_every_primitive_round_trips(self) -> None:
        writer = Writer(Endianness.BIG, 4)
        writer.write_bool(True)
        writer.write_u8(0xAB)
        writer.write_i8(-3)
        writer.write_char("Z")
        writer.write_wchar("ሴ")
        writer.write_i16(-12345)
        writer.write_u16(54321)
        writer.write_i32(-1234567)
        writer.write_u32(2345678)
        writer.write_i64(-987654321)
        writer.write_u64(123456789)
        writer.write_f32(2.5)
        writer.write_f64(3.14159)
        writer.write_string("grüß")
        writer.write_wstring("A\U0001d11e")
        writer.write_fixed("-123.45", 5, 2)

        reader = Reader(writer.finish(), Endianness.BIG, 4)
        self.assertIs(reader.read_bool(), True)
        self.assertEqual(reader.read_u8(), 0xAB)
        self.assertEqual(reader.read_i8(), -3)
        self.assertEqual(reader.read_char(), "Z")
        self.assertEqual(reader.read_wchar(), "ሴ")
        self.assertEqual(reader.read_i16(), -12345)
        self.assertEqual(reader.read_u16(), 54321)
        self.assertEqual(reader.read_i32(), -1234567)
        self.assertEqual(reader.read_u32(), 2345678)
        self.assertEqual(reader.read_i64(), -987654321)
        self.assertEqual(reader.read_u64(), 123456789)
        self.assertEqual(reader.read_f32(), 2.5)
        self.assertEqual(reader.read_f64(), 3.14159)
        self.assertEqual(reader.read_string(), "grüß")
        self.assertEqual(reader.read_wstring(), "A\U0001d11e")
        self.assertEqual(reader.read_fixed(5, 2), "-123.45")
        self.assertEqual(reader.remaining(), 0)

    def test_a_truncated_stream_is_an_error_and_not_a_zero(self) -> None:
        reader = Reader(b"\x01\x02", Endianness.LITTLE, 4)
        with self.assertRaises(Truncated):
            reader.read_i32()

    def test_a_length_the_stream_cannot_hold_is_refused_before_allocating(self) -> None:
        reader = Reader(bytes.fromhex("ffffffff") + b"ab", Endianness.LITTLE, 4)
        with self.assertRaises(ImpossibleLength):
            reader.read_string()

    def test_a_boolean_that_is_neither_zero_nor_one_is_refused(self) -> None:
        reader = Reader(b"\x02", Endianness.LITTLE, 4)
        with self.assertRaises(OutOfRange):
            reader.read_bool()

    def test_an_emheader_of_code_five_leaves_the_length_word_for_the_member(self) -> None:
        # The writer does not reserve a NEXTINT for LC5, so the reader
        # must not consume one: that word is the string's own length.
        writer = Writer(Endianness.LITTLE, 4)
        header = writer.write_emheader(0, 5, False)
        writer.write_string("abc")
        writer.finish_emheader(header)

        reader = Reader(writer.finish(), Endianness.LITTLE, 4)
        read = reader.read_emheader()
        self.assertEqual(read.length_code, 5)
        self.assertEqual(read.next_int, 4)
        self.assertEqual(reader.read_string(), "abc")

    def test_a_member_is_skipped_by_the_length_its_code_states(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        header = writer.write_emheader(9, 5, False)
        writer.write_string("abc")
        writer.finish_emheader(header)
        writer.write_i32(7)

        reader = Reader(writer.finish(), Endianness.LITTLE, 4)
        reader.skip_member(reader.read_emheader())
        self.assertEqual(reader.read_i32(), 7)


class TryConstructSupport(unittest.TestCase):
    """DDS-XTypes 1.3 §7.2.2.7, Table 11 — the trim and its units."""

    def test_a_bound_counts_octets_for_a_string_and_units_for_a_wstring(self) -> None:
        # `string<5>` is `char[6]` in the C mapping of every measured DDS
        # implementation, so a five-scalar value of ten octets is one no
        # peer can hold. A `wstring<5>` counts UTF-16 code units, and a
        # scalar above the basic plane is two of them.
        self.assertEqual(utf8_octets("Hello World!"), 12)
        self.assertEqual(utf8_octets("äöü"), 6)
        self.assertEqual(utf16_units("äöü"), 3)
        self.assertEqual(utf16_units("a\U0001F600"), 3)

    def test_a_trim_keeps_a_prefix_and_never_splits_a_character(self) -> None:
        # Table 11: "The order of the characters in the string ... is
        # preserved", so this is a prefix. A character that would
        # straddle the bound is left out whole — half a UTF-8 sequence is
        # not text, and a peer holding `char[6]` cannot store it either.
        self.assertEqual(trim_utf8("Hello World!", 5), "Hello")
        self.assertEqual(trim_utf8("äöüäö", 5), "äö")
        self.assertEqual(trim_utf8("abc", 5), "abc")
        self.assertEqual(trim_utf16("a\U0001F600", 2), "a")
        self.assertEqual(trim_utf16("a\U0001F600", 3), "a\U0001F600")

    def test_a_discard_is_not_one_of_the_wire_errors(self) -> None:
        # A discarded sample is an absence, not an error: §7.2.4.1 says
        # the receiver drops the object, and a subscriber that logged
        # every narrowed sample as a protocol fault would drown.
        from zerodds_wire.error import WireError

        self.assertFalse(issubclass(Discarded, WireError))
        self.assertEqual(
            [behaviour.name for behaviour in TryConstruct],
            ["DISCARD", "USE_DEFAULT", "TRIM"],
        )


class ReaderStatus(unittest.TestCase):
    """The refusals a reader records instead of raising.

    A malformed stream raises. A refusal about the *type* — an
    enumerator the declaration does not have, a member that did not
    arrive, §7.2.2.7's DISCARD — sets a status the generated decoder
    reads once at the end, because the decoder has to walk to the end of
    the type to know how much of the stream the sample occupied.
    """

    def test_a_fresh_reader_has_refused_nothing(self) -> None:
        r = Reader(b"\x01\x02\x03\x04", Endianness.LITTLE, 4)
        self.assertIs(r.reader_status(), Status.OK)

    def test_the_first_refusal_sticks_and_a_later_one_does_not_replace_it(self) -> None:
        # Everything read after the first refusal is read at a position
        # no writer need have put anything at, so a second refusal
        # describes the reader's own drift rather than the fault.
        r = Reader(b"\x01\x02\x03\x04", Endianness.LITTLE, 4)
        r.reader_refuse(Status.DISCARDED)
        r.reader_refuse(Status.OUT_OF_RANGE)
        self.assertIs(r.reader_status(), Status.DISCARDED)

    def test_refusing_ok_refuses_nothing(self) -> None:
        r = Reader(b"\x01\x02\x03\x04", Endianness.LITTLE, 4)
        r.reader_refuse(Status.OK)
        self.assertIs(r.reader_status(), Status.OK)

    def test_a_discard_can_be_taken_back_and_nothing_else_can(self) -> None:
        # §7.2.2.4.4.4.9 puts a member's explicit `@default` before
        # Table 9's value for its type, which the type's own decoder
        # cannot supply. So the call site asks for DISCARD and clears it
        # here. A truncated stream is not a narrowing and stays.
        r = Reader(b"\x01\x02\x03\x04", Endianness.LITTLE, 4)
        r.reader_refuse(Status.DISCARDED)
        self.assertTrue(r.reader_clear_discard())
        self.assertIs(r.reader_status(), Status.OK)

        for refusal in (Status.TRUNCATED, Status.IMPOSSIBLE_LENGTH, Status.OUT_OF_RANGE):
            other = Reader(b"\x01\x02\x03\x04", Endianness.LITTLE, 4)
            other.reader_refuse(refusal)
            self.assertFalse(other.reader_clear_discard())
            self.assertIs(other.reader_status(), refusal)

    def test_clearing_a_reader_that_refused_nothing_clears_nothing(self) -> None:
        r = Reader(b"\x01\x02\x03\x04", Endianness.LITTLE, 4)
        self.assertFalse(r.reader_clear_discard())
        self.assertIs(r.reader_status(), Status.OK)

    def test_the_walks_names_for_the_cursor_are_the_cursor(self) -> None:
        r = Reader(b"\x01\x02\x03\x04\x05\x06\x07\x08", Endianness.LITTLE, 4)
        r.read_u32()
        self.assertEqual(r.reader_position(), r.position())
        self.assertEqual(r.reader_position(), 4)
        self.assertEqual(r.reader_remaining(), r.remaining())
        self.assertEqual(r.reader_remaining(), 4)

    def test_a_seek_forward_moves_and_a_seek_backwards_is_a_refusal(self) -> None:
        # A framing word pointing behind the cursor means the reader has
        # already read past what it framed. Seeking back would read the
        # member twice and the sample would come back plausible.
        r = Reader(b"\x01\x02\x03\x04\x05\x06\x07\x08", Endianness.LITTLE, 4)
        r.reader_skip_to(6)
        self.assertEqual(r.position(), 6)
        r.reader_skip_to(2)
        self.assertIs(r.reader_status(), Status.OUT_OF_RANGE)
        self.assertEqual(r.position(), 6)

    def test_a_seek_past_the_end_raises(self) -> None:
        r = Reader(b"\x01\x02\x03\x04", Endianness.LITTLE, 4)
        with self.assertRaises(Truncated):
            r.reader_skip_to(5)

    def test_a_refused_reader_does_not_seek(self) -> None:
        r = Reader(b"\x01\x02\x03\x04", Endianness.LITTLE, 4)
        r.reader_refuse(Status.OUT_OF_RANGE)
        r.reader_skip_to(4)
        self.assertEqual(r.position(), 0)

    def test_a_mark_carries_the_cursor_the_origin_and_the_status(self) -> None:
        # The two-pass read of a delimited `map`: the entries are read
        # once to count them and again into storage. A refusal made
        # during the counting pass is about bytes that are read again,
        # and carrying it forward would refuse the sample twice.
        r = Reader(bytes(range(16)), Endianness.LITTLE, 4)
        r.read_u32()
        saved_origin = r.begin_parameter_value()
        mark = r.reader_save()
        self.assertIsInstance(mark, ReaderMark)

        r.read_u32()
        r.reader_refuse(Status.DISCARDED)
        r.finish_parameter_value(saved_origin)
        r.reader_restore(mark)

        self.assertEqual(r.position(), 4)
        self.assertIs(r.reader_status(), Status.OK)
        # The origin came back with the rest: a `long` at offset 4 with
        # the origin at 4 is already aligned and takes no padding, which
        # is the whole reason the origin is part of the mark.
        r.read_u32()
        self.assertEqual(r.position(), 8)


class WalkNames(unittest.TestCase):
    """The names the shared codec walk calls one operation by.

    `crates/lang-rules/src/emit/{encode,decode}.rs` emits one call for
    every column. Where `runtime/c` needs two forms — a string read into
    the caller's storage and one allocated, a `wstring` converted to
    UTF-8 and one not — Python has one, and the second name is that one
    method again rather than a second implementation that could drift.
    """

    def test_the_allocating_string_read_is_the_string_read(self) -> None:
        raw = bytes.fromhex("0600000048656c6c6f00")
        self.assertEqual(
            Reader(raw, Endianness.LITTLE, 4).read_string_alloc(),
            Reader(raw, Endianness.LITTLE, 4).read_string(),
        )
        self.assertEqual(Reader(raw, Endianness.LITTLE, 4).read_string_alloc(), "Hello")

    def test_the_utf8_wstring_read_is_the_wstring_read(self) -> None:
        # The count is of octets under `specification` and of characters
        # under `character-count`, so the same two units are framed
        # differently — which is exactly what the delegation must carry.
        for raw, policy, order in (
            (bytes.fromhex("0400000061006200"), "specification", "stream"),
            (bytes.fromhex("0200000061006200"), "character-count", "stream"),
        ):
            with self.subTest(policy=policy):
                self.assertEqual(
                    Reader(raw, Endianness.LITTLE, 4).read_wstring_utf8(policy, order),
                    Reader(raw, Endianness.LITTLE, 4).read_wstring(policy, order),
                )

    def test_the_utf8_wstring_write_is_the_wstring_write(self) -> None:
        for policy, order in (
            ("specification", "stream"),
            ("character-count", "stream"),
            ("specification-wide-units", "big-endian-always"),
        ):
            with self.subTest(policy=policy, order=order):
                one = Writer(Endianness.LITTLE, 4)
                one.write_wstring_utf8("äb", policy, order)
                other = Writer(Endianness.LITTLE, 4)
                other.write_wstring("äb", policy, order)
                self.assertEqual(one.finish(), other.finish())

    def test_the_roles_read_opens_the_value_block_and_the_plain_read_does_not(self) -> None:
        # §7.4.1.2: a parameter's value is its own stream block, so a
        # member's internal alignment is counted from the first octet
        # after the header. `read_pid_as` opens that block and
        # `finish_pid_read` closes it; `read_pid` leaves the origin alone
        # because its caller opens one itself with
        # `begin_parameter_value`. The two headers therefore agree about
        # the parameter and differ about the block, which is the whole
        # of the difference.
        raw = bytes.fromhex("020004002a000000")
        opened = Reader(raw, Endianness.LITTLE, 8).read_pid_as(ParameterRole.OPTIONAL)
        self.assertEqual(opened.pid, 2)
        self.assertIs(opened.role, ParameterRole.OPTIONAL)
        self.assertEqual(opened.value_at, 4)

        plain = Reader(raw, Endianness.LITTLE, 8).read_pid(ParameterRole.OPTIONAL)
        self.assertEqual(plain.value_at, 0)
        self.assertEqual(
            dataclasses.replace(opened, value_at=0),
            plain,
            "the two disagree about more than the value block",
        )

    def test_a_parameters_value_aligns_from_its_own_first_octet(self) -> None:
        # The measurement behind it, as a round trip through this
        # runtime's own two halves. A `double` at offset 0 of its
        # parameter needs no padding — §7.4.1.2 counts its alignment
        # from the first octet after the header — and a reader that
        # aligned against the message would skip four octets the writer
        # never wrote and read the value from the wrong place. That is
        # `struct-mutable` and `struct-inheritance-mutable` under
        # PL_CDR1, where the second came back as 5.56e-309 where
        # 3.141592653589793 went in.
        writer = Writer(Endianness.LITTLE, 8)
        # A short member ahead of it, so the parameter does not begin at
        # an offset that is a multiple of eight by accident.
        first = writer.write_pid(1)
        writer.write_i16(-19)
        writer.finish_pid(first)
        second = writer.write_pid(2)
        writer.write_f64(3.141592653589793)
        writer.finish_pid(second)
        writer.write_sentinel()
        raw = writer.finish()

        reader = Reader(raw, Endianness.LITTLE, 8)
        one = reader.read_pid_as(ParameterRole.MEMBER)
        self.assertEqual(one.pid, 1)
        self.assertEqual(reader.read_i16(), -19)
        reader.finish_pid_read(one)

        two = reader.read_pid_as(ParameterRole.MEMBER)
        self.assertEqual(two.pid, 2)
        # The value starts here and is offset 0 of its own block, so no
        # padding is stepped whatever the message offset is.
        self.assertEqual(reader.position(), two.value_at)
        self.assertEqual(reader.read_f64(), 3.141592653589793)
        reader.finish_pid_read(two)
        # The origin came back, so what follows is found against the
        # message again.
        self.assertTrue(reader.read_pid_as(ParameterRole.MEMBER).end)

    def test_the_trims_are_reachable_on_the_reader(self) -> None:
        # The walk emits the trim as a call on the reader, so the module
        # function needs a method beside it. Both are Table 11's answer.
        r = Reader(b"", Endianness.LITTLE, 4)
        self.assertEqual(r.trim_utf8("äöüäö", 5), trim_utf8("äöüäö", 5))
        self.assertEqual(r.trim_utf8("äöüäö", 5), "äö")
        self.assertEqual(r.trim_utf16("a\U0001F600", 2), trim_utf16("a\U0001F600", 2))
        self.assertEqual(r.trim_utf16("a\U0001F600", 2), "a")


class ParameterFinish(unittest.TestCase):
    """§7.4.1.2: a parameter's padding is outside its declared length."""

    def test_the_next_parameter_is_found_by_alignment(self) -> None:
        # `short c` of `ext_mutable`: the value is two bytes and the next
        # header starts at four, so the finish steps over two bytes of
        # padding that no declared length covers. That is what lets one
        # reader take RTI's rounded-up length and Fast DDS's exact one.
        writer = Writer(Endianness.LITTLE, 8)
        header = writer.write_pid(2)
        writer.write_i16(-3)
        writer.finish_pid(header)

        r = Reader(writer.finish(), Endianness.LITTLE, 8)
        read = r.read_pid()
        self.assertEqual(r.read_i16(), -3)
        self.assertEqual(r.position(), 6)
        r.finish_pid_read(read)
        self.assertEqual(r.position(), 8)

    def test_nothing_follows_the_sentinel(self) -> None:
        r = Reader(bytes.fromhex("027f0000ff"), Endianness.LITTLE, 8)
        header = r.read_pid()
        self.assertTrue(header.end)
        r.read_u8()
        r.finish_pid_read(header)
        self.assertEqual(r.position(), 5)

    def test_a_refused_reader_does_not_step(self) -> None:
        # A cursor advanced after a refusal sits where no writer put
        # anything, and the trailing-octet check at the top of the
        # generated decoder would report that as a second fault.
        r = Reader(bytes.fromhex("02000400fdff0000"), Endianness.LITTLE, 8)
        header = r.read_pid()
        r.read_i16()
        r.reader_refuse(Status.OUT_OF_RANGE)
        r.finish_pid_read(header)
        self.assertEqual(r.position(), 6)


class Bitfields(unittest.TestCase):
    """A bitset's fields, packed into the anchor the planner sized."""

    def test_a_field_lands_at_its_position(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        self.assertEqual(writer.bitfield(0b101, 3, 4), 0b101 << 4)
        self.assertEqual(writer.bitfield(1, 1, 63), 1 << 63)
        self.assertEqual(writer.bitfield(0, 64, 0), 0)
        self.assertEqual(writer.bitfield((1 << 64) - 1, 64, 0), (1 << 64) - 1)

    def test_a_value_wider_than_the_field_is_refused_and_not_masked(self) -> None:
        # Masking would put a different number on the wire than the
        # caller asked for, and the caller would never find out.
        writer = Writer(Endianness.LITTLE, 4)
        with self.assertRaises(OutOfRange):
            writer.bitfield(0b1000, 3, 0)
        with self.assertRaises(OutOfRange):
            writer.bitfield(-1, 3, 0)

    def test_a_field_outside_the_anchor_is_refused(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        for width, position in ((0, 0), (65, 0), (1, -1), (4, 61), (64, 1)):
            with self.subTest(width=width, position=position):
                with self.assertRaises(OutOfRange):
                    writer.bitfield(0, width, position)

    def test_the_fields_of_one_bitset_or_together(self) -> None:
        # `bitset { bitfield<3> a; bitfield<5> b; }` with a=5, b=17 packs
        # into one octet as `10001 101`.
        writer = Writer(Endianness.LITTLE, 4)
        bits = writer.bitfield(5, 3, 0) | writer.bitfield(17, 5, 3)
        writer.write_bitset(bits, 1)
        self.assertEqual(writer.finish().hex(), "8d")


class AbsentDelimiter(unittest.TestCase):
    """§7.4.3.5.3 gives a DHEADER to @appendable and @mutable, not @final."""

    def test_the_delimiter_a_final_aggregate_does_not_write_writes_nothing(self) -> None:
        # The walk declares one DHeader per value and finishes it either
        # way, so the absence is a value of the type and not `None`.
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_i32(7)
        writer.finish_dheader(no_dheader())
        self.assertEqual(writer.finish().hex(), "07000000")

    def test_a_real_delimiter_still_states_the_length(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        header = writer.write_dheader()
        writer.write_i32(7)
        writer.finish_dheader(header)
        self.assertEqual(writer.finish().hex(), "0400000007000000")


class StatusValues(unittest.TestCase):
    """One constant, spelled the same in every column.

    The shared walk emits one `Constant::Status*` into each runtime, so
    a column that renamed a member would compare against a name no other
    column knows. `runtime/typescript/src/policy.ts` carries the same
    five.
    """

    def test_the_members_and_their_values(self) -> None:
        self.assertEqual(
            [(status.name, status.value) for status in Status],
            [
                ("OK", "ok"),
                ("TRUNCATED", "truncated"),
                ("IMPOSSIBLE_LENGTH", "impossible-length"),
                ("OUT_OF_RANGE", "out-of-range"),
                ("DISCARDED", "discarded"),
            ],
        )


class SignedEnumHolder(unittest.TestCase):
    """§7.4.3.5.3 rule (5) makes an enumeration's holder signed.

    Both directions of this runtime take that reading, and so do the
    five sibling runtimes and the shared codec walk's
    `enumerator_pattern`. The reader answered the unsigned pattern until
    it was measured against a decoder built from that walk, which
    compared the sign-extended declared value and matched no arm.
    """

    def read(self, octets: bytes, bit_bound: int) -> int:
        return Reader(octets, Endianness.LITTLE).read_enum(bit_bound)

    def test_the_top_bit_of_the_holder_reads_back_negative(self) -> None:
        # `@value(-1)` at the default bound goes out as `ffffffff`,
        # which is what `write_enum` writes, and comes back as -1.
        self.assertEqual(self.read(b"\xff\xff\xff\xff", 32), -1)
        self.assertEqual(self.read(b"\xff", 8), -1)
        self.assertEqual(self.read(b"\xff\xff", 16), -1)

    def test_the_sign_bit_is_the_holders_and_not_the_bounds(self) -> None:
        # `@bit_bound(3)` is carried in an Int8, so bit 7 is the sign and
        # bit 2 is not: 4 is 4 and not -4.
        self.assertEqual(self.read(b"\x04", 3), 4)
        self.assertEqual(self.read(b"\x80", 3), -128)

    def test_a_value_above_the_sign_bit_still_names_its_own_octets(self) -> None:
        # `@value(200)` under `@bit_bound(8)` is `c8`, and -56 + 256 is
        # 200 — nothing is lost, which is why either reading works and
        # only one of them agrees with the other five runtimes.
        self.assertEqual(self.read(b"\xc8", 8), -56)
        self.assertEqual(-56 + (1 << 8), 200)

    def test_a_round_trip_through_the_writer(self) -> None:
        for value, bound in [(-1, 8), (200, 8), (-1, 32), (0, 16), (127, 8)]:
            writer = Writer.little_endian()
            writer.write_enum(value, bound)
            back = Reader(writer.finish(), Endianness.LITTLE).read_enum(bound)
            width = 8 if bound <= 8 else 16 if bound <= 16 else 32
            expected = value if value < (1 << (width - 1)) else value - (1 << width)
            self.assertEqual(back, expected, f"{value} at bound {bound}")


class MemberExtent(unittest.TestCase):
    """`EmHeaderRead.extent`: how far the member runs past the header.

    XTypes 1.3 §7.4.3.4.2. The same arithmetic `skip_member` does, asked
    of a header the reader has already stepped past — which is what a
    decoder checks itself against once it has read a member it knows.
    """

    def header(self, length_code: int, next_int: int | None) -> EmHeaderRead:
        return EmHeaderRead(
            member_id=1, length_code=length_code, must_understand=False, next_int=next_int
        )

    def test_the_short_codes_state_their_own_size(self) -> None:
        for code, octets in [(0, 1), (1, 2), (2, 4), (3, 8)]:
            self.assertEqual(self.header(code, None).extent, octets)

    def test_code_four_is_the_nextint(self) -> None:
        self.assertEqual(self.header(4, 12).extent, 12)

    def test_the_reused_codes_span_the_nextint_they_reuse(self) -> None:
        # 5, 6 and 7 have a NEXTINT that is *also* the member's own
        # leading word, so the member spans that word plus the length it
        # states, times one, four or eight.
        self.assertEqual(self.header(5, 3).extent, 4 + 3)
        self.assertEqual(self.header(6, 3).extent, 4 + 12)
        self.assertEqual(self.header(7, 3).extent, 4 + 24)

    def test_it_agrees_with_what_skip_member_advances_by(self) -> None:
        for code, next_int in [(0, None), (2, None), (4, 5), (5, 2), (6, 2), (7, 2)]:
            header = self.header(code, next_int)
            reader = Reader(b"\x00" * 128, Endianness.LITTLE)
            reader.skip_member(header)
            self.assertEqual(reader.position(), header.extent, f"code {code}")

    def test_a_missing_nextint_is_no_extent_rather_than_a_guess(self) -> None:
        self.assertEqual(self.header(4, None).extent, 0)


class WalkFreeFunctions(unittest.TestCase):
    """The two free functions a generated codec names by bare name.

    `emit/decode.rs` reaches both through `CodecTarget::runtime_static`
    where a bounded `string` or `wstring` is measured against its bound,
    so a generated module imports them from the package root.
    """

    def test_the_package_exports_the_two_the_walk_names(self) -> None:
        import zerodds_wire

        for name in ("utf8_octets", "utf16_units"):
            self.assertIn(name, zerodds_wire.__all__)
            self.assertTrue(hasattr(zerodds_wire, name), name)
        self.assertEqual(zerodds_wire.utf8_octets("\u00e4"), 2)
        self.assertEqual(zerodds_wire.utf16_units("\U0001d11e"), 2)


class HddsAxes(unittest.TestCase):
    """The three axes `Dialect::Hdds` selects.

    Expected octets from the recorded HDDS columns:
    `tests/interop-results/wstring-member/hdds/plain-cdr1.le.bin`,
    `tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr1.le.bin`,
    `tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr1.le.bin`,
    `tests/interop-results/array-1d-struct-final/hdds/plain-cdr1.le.bin`
    and `tests/interop-results/typedef-of-struct/hdds/plain-cdr1.le.bin`.
    """

    def test_a_wstring_as_utf8_octets_counts_octets_and_a_nul(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_wstring_utf8_octets("abc", "octets/terminated/2")
        self.assertEqual(writer.finish().hex(), "0400000061626300")

    def test_a_multibyte_utf8_wstring_counts_its_octets(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_wstring_utf8_octets("äöü ሴ Z", "octets/terminated/2")
        self.assertEqual(writer.finish().hex(), "0d000000c3a4c3b6c3bc20e188b4205a00")

    def test_a_bounded_utf8_wstring_bounds_utf16_units(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        writer.write_wstring_utf8_octets_bounded("ሴäZ", 3, "octets/terminated/2")
        self.assertEqual(writer.finish().hex(), "07000000e188b4c3a45a00")

    def test_a_utf8_wstring_reads_back_as_its_octets(self) -> None:
        reader = Reader(
            bytes.fromhex("0d000000c3a4c3b6c3bc20e188b4205a00"),
            Endianness.LITTLE,
            4,
        )
        self.assertEqual(
            reader.read_wstring_utf8_octets("octets/terminated/2"),
            "äöü ሴ Z",
        )

    def test_a_packed_element_starts_where_the_last_ended(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        saved = writer.begin_packed_element()
        writer.write_i32(1)
        writer.write_i16(2)
        writer.finish_packed_element(saved)
        saved = writer.begin_packed_element()
        writer.write_i32(3)
        writer.write_i16(4)
        writer.finish_packed_element(saved)
        writer.write_u8(119)
        self.assertEqual(writer.finish().hex(), "01000000020003000000040077")

    def test_a_packed_aggregate_counts_alignment_from_its_own_start(self) -> None:
        writer = Writer(Endianness.LITTLE, 4)
        saved = writer.begin_packed_element()
        writer.write_i32(-1234567)
        writer.write_i16(-2)
        writer.finish_packed_element(saved)
        saved = writer.begin_packed_element()
        writer.write_i32(1)
        writer.write_i16(32767)
        writer.finish_packed_element(saved)
        self.assertEqual(writer.finish().hex(), "7929edfffeff01000000ff7f")

    def test_a_packed_element_reads_back_at_its_own_origin(self) -> None:
        reader = Reader(
            bytes.fromhex("01000000020003000000040077"),
            Endianness.LITTLE,
            4,
        )
        saved = reader.reader_begin_packed_element()
        self.assertEqual((reader.read_i32(), reader.read_i16()), (1, 2))
        reader.reader_finish_packed_element(saved)
        saved = reader.reader_begin_packed_element()
        self.assertEqual((reader.read_i32(), reader.read_i16()), (3, 4))
        reader.reader_finish_packed_element(saved)
        self.assertEqual(reader.read_u8(), 119)


if __name__ == "__main__":
    unittest.main()
