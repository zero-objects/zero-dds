// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The C++ wire runtime's self-test.
//
// Silent and exit 0 is the pass. A test that printed a summary would make a
// failure something a reader has to look for.
//
// Every expectation here is a byte string, not a round-trip: a writer and a
// reader that share a misreading of §7.4.1 agree with each other perfectly.
// The round-trips are here as well, but they are the second check and never
// the only one.

#include "dds/types.hpp"
#include "zerodds/wire.hpp"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <type_traits>
#include <vector>

using namespace zerodds::wire;

namespace {

int failures = 0;

void fail(int line, const std::string &message) {
    std::fprintf(stderr, "selftest.cpp:%d: %s\n", line, message.c_str());
    failures++;
}

void check(bool condition, int line, const char *text) {
    if (!condition) {
        fail(line, std::string("expected ") + text);
    }
}

std::string hex(const std::uint8_t *data, std::size_t length) {
    static const char digits[] = "0123456789abcdef";
    std::string out;
    for (std::size_t i = 0; i < length; i++) {
        out.push_back(digits[data[i] >> 4]);
        out.push_back(digits[data[i] & 0x0Fu]);
    }
    return out;
}

std::string hex(const std::vector<std::uint8_t> &data) {
    return hex(data.data(), data.size());
}

std::vector<std::uint8_t> unhex(const std::string &text) {
    std::vector<std::uint8_t> out;
    unsigned high = 0;
    bool have_high = false;
    for (char c : text) {
        if (c == ' ' || c == '_') {
            continue;
        }
        unsigned value = 0;
        if (c >= '0' && c <= '9') {
            value = static_cast<unsigned>(c - '0');
        } else if (c >= 'a' && c <= 'f') {
            value = static_cast<unsigned>(c - 'a') + 10u;
        } else {
            std::fprintf(stderr, "unhex: `%c` is not a hex digit\n", c);
            std::exit(2);
        }
        if (have_high) {
            out.push_back(static_cast<std::uint8_t>((high << 4) | value));
            have_high = false;
        } else {
            high = value;
            have_high = true;
        }
    }
    return out;
}

/// Where two byte strings first differ, so a failure names an offset rather
/// than leaving the reader to align two hex dumps by eye.
void expect_bytes(const std::vector<std::uint8_t> &got, const char *expected, int line) {
    const std::vector<std::uint8_t> want = unhex(expected);
    if (got == want) {
        return;
    }
    std::size_t at = 0;
    while (at < got.size() && at < want.size() && got[at] == want[at]) {
        at++;
    }
    fail(line, "bytes differ at offset " + std::to_string(at) + "\n  got  " + hex(got) +
                   "\n  want " + hex(want));
}

void expect_equal(const std::string &got, const std::string &want, int line) {
    if (got != want) {
        fail(line, "got `" + got + "`, want `" + want + "`");
    }
}

template <typename T, typename F>
void expect_throws(F body, int line, const char *what) {
    try {
        body();
    } catch (const T &) {
        return;
    } catch (const std::exception &error) {
        fail(line, std::string("wrong exception for ") + what + ": " + error.what());
        return;
    }
    fail(line, std::string("no exception for ") + what);
}

#define CHECK(condition) check((condition), __LINE__, #condition)
#define EXPECT_BYTES(writer, expected) expect_bytes((writer).bytes(), (expected), __LINE__)
#define EXPECT_EQ(got, want) expect_equal((got), (want), __LINE__)
#define EXPECT_THROWS(type, body) expect_throws<type>([&] body, __LINE__, #body)

// -- alignment --------------------------------------------------------------
//
// The trap this project has hit four times: `align_to` takes
// `MIN(align, max_align)` and counts from the origin. Eight for XCDR1 and
// four for XCDR2, and a runtime that padded an eight-byte primitive to eight
// under XCDR2 puts four bytes on the wire that no peer expects.

void alignment() {
    {
        Writer w(Endianness::Little, 8);
        w.write_u8(0x01);
        w.write_u64(0x0102030405060708ull);
        EXPECT_BYTES(w, "01 00000000000000 0807060504030201");
        CHECK(w.position() == 16);
    }
    {
        Writer w(Endianness::Little, 4);
        w.write_u8(0x01);
        w.write_u64(0x0102030405060708ull);
        EXPECT_BYTES(w, "01 000000 0807060504030201");
        CHECK(w.position() == 12);
    }
    {
        Writer w(Endianness::Big, 8);
        w.write_u8(0x01);
        w.write_f64(1.0);
        EXPECT_BYTES(w, "01 00000000000000 3ff0000000000000");
    }
    {
        Writer w(Endianness::Big, 4);
        w.write_u8(0x01);
        w.write_f64(1.0);
        EXPECT_BYTES(w, "01 000000 3ff0000000000000");
    }
    {
        // `long double` aligns to eight, capped to four under XCDR2.
        Writer w(Endianness::Little, 8);
        w.write_u8(0x01);
        w.write_f128(Float128::one());
        CHECK(w.position() == 24);
        Writer v(Endianness::Little, 4);
        v.write_u8(0x01);
        v.write_f128(Float128::one());
        CHECK(v.position() == 20);
    }
    {
        // A two-byte field after a one-byte field, which the cap never
        // reaches: both encodings agree.
        Writer w(Endianness::Little, 4);
        w.write_u8(0x01);
        w.write_u16(0x0203);
        EXPECT_BYTES(w, "01 00 0302");
    }
    {
        // The reader's half. A reader that skipped padding the writer never
        // wrote reads the wrong bytes from the second member on.
        const std::vector<std::uint8_t> data = unhex("01 000000 0807060504030201");
        Reader r(data, Endianness::Little, 4);
        CHECK(r.read_u8() == 0x01);
        CHECK(r.read_u64() == 0x0102030405060708ull);
        CHECK(r.remaining() == 0);
        Reader wrong(data, Endianness::Little, 8);
        CHECK(wrong.read_u8() == 0x01);
        // The same bytes read under the other cap are a different value, and
        // then run off the end: that is the whole point of the cap.
        EXPECT_THROWS(Truncated, { (void)wrong.read_u64(); });
    }
}

// -- primitives -------------------------------------------------------------

void primitives() {
    for (Endianness order : {Endianness::Little, Endianness::Big}) {
        Writer w(order, 8);
        w.write_bool(true);
        w.write_u8(0xAB);
        w.write_i8(-13);
        w.write_char(U'Z');
        w.write_wchar(U'ä');
        w.write_i16(-12345);
        w.write_u16(48879);
        w.write_i32(-1234567);
        w.write_u32(3735928559u);
        w.write_i64(-987654321987LL);
        w.write_u64(16045690984833335998ull);
        w.write_f32(-3.75);
        w.write_f64(3.141592653589793);
        w.write_f128(Float128::from_parts(0x3FFF000000000000ull, 1));

        Reader r(w.bytes(), order, 8);
        CHECK(r.read_bool() == true);
        CHECK(r.read_u8() == 0xAB);
        CHECK(r.read_i8() == -13);
        CHECK(r.read_char() == U'Z');
        CHECK(r.read_wchar() == U'ä');
        CHECK(r.read_i16() == -12345);
        CHECK(r.read_u16() == 48879);
        CHECK(r.read_i32() == -1234567);
        CHECK(r.read_u32() == 3735928559u);
        CHECK(r.read_i64() == -987654321987LL);
        CHECK(r.read_u64() == 16045690984833335998ull);
        CHECK(r.read_f32() == -3.75f);
        CHECK(r.read_f64() == 3.141592653589793);
        CHECK(r.read_f128() == Float128::from_parts(0x3FFF000000000000ull, 1));
        CHECK(r.remaining() == 0);
    }

    // The `primitive-mixed` field sequence, as bytes. Both encodings, both
    // orders — this is the case that separates XCDR1 from XCDR2, because the
    // eight-byte members align to eight there and to four here.
    auto mixed = [](Endianness order, std::size_t max_align) {
        Writer w(order, max_align);
        w.write_bool(true);
        w.write_u8(171);
        w.write_u8(254);
        w.write_i8(-13);
        w.write_char(U'Z');
        w.write_wchar(U'ä');
        w.write_i16(-12345);
        w.write_u16(48879);
        w.write_i32(-1234567);
        w.write_u32(3735928559u);
        w.write_i64(-987654321987LL);
        w.write_u64(16045690984833335998ull);
        w.write_f32(-3.75);
        w.write_f64(3.141592653589793);
        return w.finish();
    };
    expect_bytes(mixed(Endianness::Little, 4),
                 "01abfef35a00e400c7cfefbe7929edffefbeaddebd08370b1affffffbec2addeefbe"
                 "adde000070c0182d4454fb210940",
                 __LINE__);
    expect_bytes(mixed(Endianness::Big, 8),
                 "01abfef35a0000e4cfc7beefffed2979deadbeef00000000ffffff1a0b3708bddead"
                 "beefdeadc2bec070000000000000400921fb54442d18",
                 __LINE__);

    // Out of range refused rather than truncated. The C++ writers take the
    // widest natural type so that the check happens here and not at a call
    // site the library cannot see.
    {
        Writer w(Endianness::Little, 8);
        EXPECT_THROWS(OutOfRange, { w.write_u16(70000); });
        EXPECT_THROWS(OutOfRange, { w.write_i16(40000); });
        EXPECT_THROWS(OutOfRange, { w.write_u8(256); });
        EXPECT_THROWS(OutOfRange, { w.write_i8(128); });
        EXPECT_THROWS(OutOfRange, { w.write_char(U'Ā'); });
        EXPECT_THROWS(OutOfRange, { w.write_wchar(U'\U0001F600'); });
        // Nothing was written by any of them.
        CHECK(w.position() == 0);
        try {
            w.write_u16(70000);
        } catch (const OutOfRange &error) {
            EXPECT_EQ(error.expected(), "an unsigned short in [0, 65535], not 70000");
            CHECK(error.at() == 0);
        }
    }
    {
        // A boolean of 2 is a stream that disagrees with its own type.
        const std::vector<std::uint8_t> data = unhex("02");
        Reader r(data, Endianness::Little, 4);
        EXPECT_THROWS(OutOfRange, { (void)r.read_bool(); });
    }
    // A run of octets, as generated code writes `sequence<octet>` and
    // `octet[N]`: the bytes of one `write_u8` per octet, with nothing
    // aligned before the run in either order (XTypes 1.3 §7.4.3.4.1).
    for (Endianness order : {Endianness::Little, Endianness::Big}) {
        const std::uint8_t run[] = {0x00, 0x7F, 0x80, 0xFF, 0x01};
        Writer bulk(order, 8);
        bulk.write_u8(0xAA);
        bulk.write_octets(run, sizeof run);
        bulk.write_octets(nullptr, 0);
        Writer each(order, 8);
        each.write_u8(0xAA);
        for (std::uint8_t octet : run) {
            each.write_u8(octet);
        }
        CHECK(bulk.bytes() == each.bytes());
        expect_bytes(bulk.bytes(), "aa007f80ff01", __LINE__);

        Reader r(bulk.bytes(), order, 8);
        CHECK(r.read_u8() == 0xAA);
        std::uint8_t back[sizeof run] = {};
        r.read_octets(back, sizeof back);
        CHECK(std::memcmp(back, run, sizeof run) == 0);
        r.read_octets(nullptr, 0);
        CHECK(r.remaining() == 0);
    }
    {
        // Fewer octets left than the run states: refused as `read_u8`
        // refuses, and nothing copied into the destination.
        const std::vector<std::uint8_t> data = unhex("0102");
        Reader r(data, Endianness::Little, 4);
        std::uint8_t out[3] = {0xEE, 0xEE, 0xEE};
        try {
            r.read_octets(out, sizeof out);
            fail(__LINE__, "a truncated run of octets was read");
        } catch (const Truncated &error) {
            CHECK(error.at() == 0);
            CHECK(error.needed() == 3);
            CHECK(error.available() == 2);
        }
        CHECK(out[0] == 0xEE && out[1] == 0xEE && out[2] == 0xEE);
    }
    {
        const std::vector<std::uint8_t> data = unhex("0102");
        Reader r(data, Endianness::Little, 4);
        try {
            (void)r.read_u32();
            fail(__LINE__, "a truncated u32 was read");
        } catch (const Truncated &error) {
            CHECK(error.at() == 0);
            CHECK(error.needed() == 4);
            CHECK(error.available() == 2);
        }
    }

    // The fixed-order writers, which the `wstring` axes need and which the
    // stream's own order must not touch.
    {
        Writer w(Endianness::Big, 8);
        w.write_u16_little(0x1234);
        w.write_u16_big(0x1234);
        w.write_u32_little(0x11223344u);
        w.write_u32_big(0x11223344u);
        EXPECT_BYTES(w, "3412 1234 44332211 11223344");
        Reader r(w.bytes(), Endianness::Big, 8);
        CHECK(r.read_u16_little() == 0x1234);
        CHECK(r.read_u16_big() == 0x1234);
        CHECK(r.read_u32_little() == 0x11223344u);
        CHECK(r.read_u32_big() == 0x11223344u);
    }

    // A `wchar` in [CDR] §9.3.1.6's GIOP 1.2 framing: an octet count, then
    // that many octets, aligned to one. omniORB writes `02 00e4` in either
    // stream order; TAO writes `04 e4000000` in the stream's.
    {
        Writer w(Endianness::Little, 8);
        w.write_wchar(U'ä', WcharWidth::Two, WcharFraming::GiopLengthPrefixed,
                      WstringUnitOrder::BigEndianAlways);
        EXPECT_BYTES(w, "02 00e4");
        Reader r(w.bytes(), Endianness::Little, 8);
        CHECK(r.read_wchar(WcharWidth::Two, WcharFraming::GiopLengthPrefixed,
                           WstringUnitOrder::BigEndianAlways) == U'ä');
    }
    {
        Writer w(Endianness::Little, 8);
        w.write_wchar(U'ä', WcharWidth::Four, WcharFraming::GiopLengthPrefixed,
                      WstringUnitOrder::Stream);
        EXPECT_BYTES(w, "04 e4000000");
    }
    {
        // A count that is not the configured code set's width is refused,
        // not believed.
        const std::vector<std::uint8_t> data = unhex("e400");
        Reader r(data, Endianness::Little, 8);
        EXPECT_THROWS(OutOfRange, {
            (void)r.read_wchar(WcharWidth::Two, WcharFraming::GiopLengthPrefixed);
        });
    }

    // A four-byte `wchar` carrying more than one UTF-16 code unit is refused
    // rather than narrowed.
    {
        const std::vector<std::uint8_t> data = unhex("00000100");
        Reader r(data, Endianness::Little, 8);
        EXPECT_THROWS(OutOfRange, { (void)r.read_wchar(WcharWidth::Four); });
    }

    // A `float` whose magnitude no `float` can hold becomes an infinity,
    // which is what a C `(float)` cast produces and what every peer reads.
    {
        Writer w(Endianness::Big, 8);
        w.write_f32(1e300);
        w.write_f32(-1e300);
        EXPECT_BYTES(w, "7f800000 ff800000");
    }
}

// -- strings ----------------------------------------------------------------

void strings() {
    {
        // The length counts the terminator and counts octets of UTF-8, not
        // characters: "héllo" is five characters and six octets.
        Writer w(Endianness::Little, 8);
        w.write_string("h\xc3\xa9llo");
        EXPECT_BYTES(w, "07000000 68c3a96c6c6f 00");
        Reader r(w.bytes(), Endianness::Little, 8);
        EXPECT_EQ(r.read_string(), "h\xc3\xa9llo");
        CHECK(r.remaining() == 0);
    }
    {
        Writer w(Endianness::Big, 8);
        w.write_string("");
        EXPECT_BYTES(w, "00000001 00");
    }
    {
        // An over-bound value is refused rather than cut at a character
        // boundary: the bound is a promise about the source and not a
        // difference on the wire, and cutting it would put a sample on
        // the wire nobody wrote.
        Writer w(Endianness::Little, 8);
        EXPECT_THROWS(OutOfRange, { w.write_string_bounded("a\xc3\xa9", 2); });
        CHECK(w.position() == 0);
        try {
            w.write_string_bounded("a\xc3\xa9", 2);
        } catch (const OutOfRange &error) {
            EXPECT_EQ(
                error.expected(), "at most 2 octets in a bounded string, and this one has 3");
        }
        // Exactly at the bound still writes the same bytes an unbounded
        // string would.
        Writer v(Endianness::Little, 8);
        v.write_string_bounded("a\xc3\xa9", 3);
        EXPECT_BYTES(v, "04000000 61c3a9 00");
    }
    {
        // A bounded wstring, sequence and map refuse the same way: the
        // count or the UTF-16 length is checked before the DHEADER or
        // the length word goes out, so nothing is written on refusal.
        Writer w(Endianness::Little, 8);
        EXPECT_THROWS(OutOfRange, { w.write_wstring_bounded("ab", 1); });
        CHECK(w.position() == 0);
        Writer ok_w(Endianness::Little, 8);
        ok_w.write_wstring_bounded("a", 1);
        CHECK(ok_w.position() > 0);

        Writer s(Endianness::Little, 8);
        EXPECT_THROWS(OutOfRange, { s.check_sequence_bound(4, 3); });
        try {
            s.check_sequence_bound(4, 3);
        } catch (const OutOfRange &error) {
            EXPECT_EQ(
                error.expected(),
                "at most 3 elements in a bounded sequence, and this one has 4");
        }
        s.check_sequence_bound(3, 3);  // at the bound: does not throw

        Writer m(Endianness::Little, 8);
        EXPECT_THROWS(OutOfRange, { m.check_map_bound(3, 2); });
        try {
            m.check_map_bound(3, 2);
        } catch (const OutOfRange &error) {
            EXPECT_EQ(
                error.expected(), "at most 2 entries in a bounded map, and this one has 3");
        }
        m.check_map_bound(2, 2);  // at the bound: does not throw
    }
    {
        // A length field claiming more than the stream holds is malformed,
        // not short.
        const std::vector<std::uint8_t> data = unhex("ff000000 6100");
        Reader r(data, Endianness::Little, 8);
        try {
            (void)r.read_string();
            fail(__LINE__, "an impossible string length was accepted");
        } catch (const ImpossibleLength &error) {
            CHECK(error.at() == 0);
            CHECK(error.claimed() == 255);
            CHECK(error.available() == 2);
        }
    }
    {
        const std::vector<std::uint8_t> data = unhex("03000000 ff41 00");
        Reader r(data, Endianness::Little, 8);
        EXPECT_THROWS(NotUtf8, { (void)r.read_string(); });
    }
}

void wstrings() {
    // "a" U+1D11E "b" — one code point outside the Basic Multilingual Plane,
    // which is two UTF-16 code units and which §7.4.1.1.2's byte count is
    // written the way it is because of.
    const char *clef = "a\xf0\x9d\x84\x9e" "b";

    {
        Writer w(Endianness::Little, 8);
        w.write_wstring_utf8(clef);
        EXPECT_BYTES(w, "08000000 6100 34d8 1edd 6200");
        Reader r(w.bytes(), Endianness::Little, 8);
        EXPECT_EQ(r.read_wstring(), clef);
    }
    {
        Writer w(Endianness::Big, 8);
        w.write_wstring_utf8(clef);
        EXPECT_BYTES(w, "00000008 0061 d834 dd1e 0062");
        Reader r(w.bytes(), Endianness::Big, 8);
        EXPECT_EQ(r.read_wstring(), clef);
    }
    {
        // TAO's GIOP 1.2 form over a four-octet transmission code set.
        Writer w(Endianness::Little, 8);
        w.write_wstring_utf8(clef, WstringLengthPolicy::SpecificationWideUnits);
        EXPECT_BYTES(w, "10000000 61000000 34d80000 1edd0000 62000000");
        Reader r(w.bytes(), Endianness::Little, 8);
        EXPECT_EQ(r.read_wstring(WstringLengthPolicy::SpecificationWideUnits), clef);
    }
    {
        // Fast DDS counts characters over two-octet units, which has no way
        // to carry a code point outside the BMP. Reproducing a vendor's
        // length convention is a policy; reproducing its data loss is not.
        Writer w(Endianness::Little, 8);
        EXPECT_THROWS(OutOfRange,
                      { w.write_wstring_utf8(clef, WstringLengthPolicy::CharacterCount); });
        Writer v(Endianness::Little, 8);
        v.write_wstring_utf8("ab", WstringLengthPolicy::CharacterCount);
        EXPECT_BYTES(v, "02000000 6100 6200");
    }
    {
        // A character count over four-octet units carries the surrogate pair
        // intact, and counts the two units it takes — not the one code point.
        Writer w(Endianness::Little, 8);
        w.write_wstring_utf8(clef, WstringLengthPolicy::RosTypeSupport);
        EXPECT_BYTES(w, "04000000 61000000 34d80000 1edd0000 62000000");
        Reader r(w.bytes(), Endianness::Little, 8);
        EXPECT_EQ(r.read_wstring(WstringLengthPolicy::RosTypeSupport), clef);
    }
    {
        // RTI's XCDR1 form: characters including a NUL, four-octet units.
        Writer w(Endianness::Little, 8);
        w.write_wstring_utf8("ab", WstringLengthPolicy::CharacterCountTerminated);
        EXPECT_BYTES(w, "03000000 61000000 62000000 00000000");
        Reader r(w.bytes(), Endianness::Little, 8);
        EXPECT_EQ(r.read_wstring(WstringLengthPolicy::CharacterCountTerminated), "ab");
    }
    {
        // omniORB's mark, counted in the length and never a character of the
        // string. An empty `wstring` carries none.
        Writer w(Endianness::Big, 8);
        w.write_wstring_utf8(clef, WstringLengthPolicy::Specification,
                        WstringUnitOrder::ByteOrderMark);
        EXPECT_BYTES(w, "0000000a fffe 6100 34d8 1edd 6200");
        Reader r(w.bytes(), Endianness::Big, 8);
        EXPECT_EQ(r.read_wstring(WstringLengthPolicy::Specification,
                                 WstringUnitOrder::ByteOrderMark),
                  clef);
        Writer v(Endianness::Big, 8);
        v.write_wstring_utf8("", WstringLengthPolicy::Specification,
                        WstringUnitOrder::ByteOrderMark);
        EXPECT_BYTES(v, "00000000");
    }
    {
        // A byte-swapped mark says every unit behind it is the other way
        // round. A reader that ignored it returns different characters, well
        // formed and the right length.
        const std::vector<std::uint8_t> data = unhex("00000006 feff 0061 0062");
        Reader r(data, Endianness::Big, 8);
        EXPECT_EQ(r.read_wstring(WstringLengthPolicy::Specification,
                                 WstringUnitOrder::ByteOrderMark),
                  "ab");
    }
    {
        // An octet count the unit width does not divide is a stream no
        // encoder produced. Rounding it down would leave the remainder in
        // the stream and put every later member at the wrong offset.
        const std::vector<std::uint8_t> data = unhex("03000000 610062");
        Reader r(data, Endianness::Little, 8);
        EXPECT_THROWS(ImpossibleLength, { (void)r.read_wstring(); });
    }
    {
        // Cyclone's big-endian output: the length word swapped, the content
        // not. In little endian this is byte-identical to the stream order.
        Writer w(Endianness::Big, 8);
        w.write_wstring_utf8("ab", WstringLengthPolicy::Specification,
                        WstringUnitOrder::HostUnswapped);
        EXPECT_BYTES(w, "00000004 6100 6200");
    }
    {
        // CESU-8 — a surrogate encoded as UTF-8 — is not a code point.
        Writer w(Endianness::Little, 8);
        EXPECT_THROWS(OutOfRange, { w.write_wstring_utf8("\xed\xa0\x80"); });
    }
}

void fixed_point() {
    {
        // `fixed<5,2>` = -1.5. Five digit nibbles and a sign make six, which
        // is even, so no leading zero nibble is prepended.
        Writer w(Endianness::Little, 8);
        w.write_fixed("-1.5", 5, 2);
        EXPECT_BYTES(w, "00 15 0d");
        Reader r(w.bytes(), Endianness::Little, 8);
        EXPECT_EQ(r.read_fixed(5, 2), "-1.50");
    }
    {
        // A leading-zero value: the field is `P` wide whatever the value, so
        // the whole part comes out of the nibbles as `000` and is stripped
        // on the way back. Returning "000.01" would put a decimal on the
        // read side the write side would never produce.
        Writer w(Endianness::Little, 8);
        w.write_fixed("0.01", 5, 2);
        EXPECT_BYTES(w, "00 00 1c");
        Reader r(w.bytes(), Endianness::Little, 8);
        EXPECT_EQ(r.read_fixed(5, 2), "0.01");
    }
    {
        // The other digit count: as many digits as the value has, and never
        // fewer than one. JacORB and ACE/TAO write this.
        Writer w(Endianness::Little, 8);
        w.write_fixed("0.01", 5, 2, FixedDigitCount::ValuePrecision);
        EXPECT_BYTES(w, "1c");
        Reader r(w.bytes(), Endianness::Little, 8);
        EXPECT_EQ(r.read_fixed(5, 2, FixedDigitCount::ValuePrecision), "0.01");
    }
    {
        // The third: the integer digits of the decimal as written plus S, made odd by one
        // leading zero. What ACE/TAO 2.5.24's `ACE_CDR::Fixed::from_string`
        // writes, recorded in
        // crates/ast-layout/tests/fixtures/unspecified_constructs.md §3.
        struct Recorded {
            const char *text;
            unsigned digits;
            unsigned scale;
            const char *octets;
            const char *back;
        };
        const Recorded recorded[] = {
            {"123.45", 5, 2, "12 34 5c", "123.45"}, {"-123.45", 5, 2, "12 34 5d", "-123.45"},
            {"12.34", 4, 2, "01 23 4c", "12.34"},   {"7", 1, 0, "7c", "7"},
            {"0.12345", 5, 5, "00 12 34 5c", "0.12345"}, {"0.00", 5, 2, "00 0c", "0.00"},
            // The scale and not the fraction as written: one digit counted
            // would keep only the last nibble, `0c`.
            {"7", 5, 2, "70 0c", "7.00"},
        };
        for (const Recorded &row : recorded) {
            Writer w(Endianness::Little, 8);
            w.write_fixed(row.text, row.digits, row.scale, FixedDigitCount::ValueStringDigits);
            EXPECT_BYTES(w, row.octets);
            Reader r(w.bytes(), Endianness::Little, 8);
            EXPECT_EQ(r.read_fixed(row.digits, row.scale, FixedDigitCount::ValueStringDigits),
                      row.back);
        }
        // Thirty-two digits as written: no width a reader can take.
        Writer wide(Endianness::Little, 8);
        EXPECT_THROWS(OutOfRange, {
            wide.write_fixed("00000000000000000000000000000001", 31, 0,
                             FixedDigitCount::ValueStringDigits);
        });
    }
    {
        // A negative zero is written positive, because there is one zero.
        Writer w(Endianness::Little, 8);
        w.write_fixed("-0.00", 5, 2);
        EXPECT_BYTES(w, "00 00 0c");
        Reader r(w.bytes(), Endianness::Little, 8);
        EXPECT_EQ(r.read_fixed(5, 2), "0.00");
    }
    {
        // An odd digit count gets the leading zero nibble that pairs the
        // sign up. `fixed<4,2>` = 12.34 is four digits and a sign.
        Writer w(Endianness::Little, 8);
        w.write_fixed("12.34", 4, 2);
        EXPECT_BYTES(w, "01 23 4c");
        Reader r(w.bytes(), Endianness::Little, 8);
        EXPECT_EQ(r.read_fixed(4, 2), "12.34");
    }
    {
        // A value `fixed<31,2>` can hold and no `double` can.
        Writer w(Endianness::Little, 8);
        w.write_fixed("12345678901234567890123456789.01", 31, 2);
        Reader r(w.bytes(), Endianness::Little, 8);
        EXPECT_EQ(r.read_fixed(31, 2), "12345678901234567890123456789.01");
    }
    {
        // Neither half is padded in the direction that would lose a digit.
        Writer w(Endianness::Little, 8);
        EXPECT_THROWS(OutOfRange, { w.write_fixed("1234", 5, 2); });
        EXPECT_THROWS(OutOfRange, { w.write_fixed("1.234", 5, 2); });
        EXPECT_THROWS(OutOfRange, { w.write_fixed("x", 5, 2); });
        EXPECT_THROWS(OutOfRange, { w.write_fixed("1", 5, 6); });
    }
}

// -- framing ----------------------------------------------------------------

void dheader() {
    Writer w(Endianness::Little, 4);
    const DHeader header = w.write_dheader();
    w.write_u32(0xAABBCCDDu);
    w.write_u8(0x01);
    w.finish_dheader(header);
    EXPECT_BYTES(w, "05000000 ddccbbaa 01");

    Reader r(w.bytes(), Endianness::Little, 4);
    CHECK(r.read_dheader() == 5);
    CHECK(r.read_u32() == 0xAABBCCDDu);
    CHECK(r.read_u8() == 0x01);

    // A DHEADER claiming more than the stream holds is malformed.
    const std::vector<std::uint8_t> bad = unhex("ff000000 01");
    Reader r2(bad, Endianness::Little, 4);
    EXPECT_THROWS(ImpossibleLength, { (void)r2.read_dheader(); });

    CHECK(!Writer::no_dheader().present);
}

void emheaders() {
    // LC0 through LC3 carry the length in the code itself.
    struct ShortCode {
        unsigned code;
        std::size_t size;
    };
    for (ShortCode entry : {ShortCode{0, 1}, ShortCode{1, 2}, ShortCode{2, 4}, ShortCode{3, 8}}) {
        Writer w(Endianness::Little, 4);
        const EmHeader header = w.write_emheader(3, entry.code, false);
        CHECK(!header.has_next_int);
        for (std::size_t i = 0; i < entry.size; i++) {
            w.write_u8(0xEE);
        }
        w.finish_emheader(header);
        CHECK(w.position() == 4 + entry.size);
        const std::uint32_t word = (entry.code << 28) | 3u;
        Writer expected(Endianness::Little, 4);
        expected.write_u32(word);
        CHECK(std::memcmp(w.bytes().data(), expected.bytes().data(), 4) == 0);

        Reader r(w.bytes(), Endianness::Little, 4);
        const EmHeaderRead read = r.read_emheader();
        CHECK(read.member_id == 3);
        CHECK(read.length_code == entry.code);
        CHECK(!read.must_understand);
        r.skip_member(read);
        CHECK(r.remaining() == 0);
    }

    {
        // LC4 reserves a NEXTINT of its own and it is patched on finish.
        Writer w(Endianness::Little, 4);
        const EmHeader header = w.write_emheader(5, 4, true);
        CHECK(header.has_next_int);
        w.write_u8(1);
        w.write_u8(2);
        w.write_u8(3);
        w.finish_emheader(header);
        EXPECT_BYTES(w, "050000c0 03000000 010203");
        Reader r(w.bytes(), Endianness::Little, 4);
        const EmHeaderRead read = r.read_emheader();
        CHECK(read.member_id == 5);
        CHECK(read.length_code == 4);
        CHECK(read.must_understand);
        CHECK(read.has_next_int && read.next_int == 3);
        r.skip_member(read);
        CHECK(r.remaining() == 0);
    }

    // LC5, LC6 and LC7 have a NEXTINT that **is** the member's own leading
    // word. No placeholder is reserved, and on read the position goes back
    // before it so the member's decoder sees it.
    struct BorrowedCode {
        unsigned code;
        std::size_t element;
    };
    for (BorrowedCode entry : {BorrowedCode{5, 1}, BorrowedCode{6, 4}, BorrowedCode{7, 8}}) {
        Writer w(Endianness::Little, 4);
        const EmHeader header = w.write_emheader(9, entry.code, false);
        CHECK(!header.has_next_int);
        w.write_u32(3);  // the element count, which is also the NEXTINT
        for (std::size_t i = 0; i < 3 * entry.element; i++) {
            w.write_u8(0x77);
        }
        w.finish_emheader(header);
        CHECK(w.position() == 8 + 3 * entry.element);

        Reader r(w.bytes(), Endianness::Little, 4);
        const EmHeaderRead read = r.read_emheader();
        CHECK(read.length_code == entry.code);
        CHECK(read.has_next_int && read.next_int == 3);
        // The reader is standing on the NEXTINT, not past it.
        CHECK(r.position() == 4);
        r.skip_member(read);
        CHECK(r.remaining() == 0);
    }

    {
        // The short-code rewrite: an LC4 header whose member turned out to
        // be four bytes loses its NEXTINT and states the length in the code.
        // Fast DDS makes the same choice the same way.
        Writer w(Endianness::Little, 4, PlCdr1Framing{}, true);
        const EmHeader header = w.write_emheader(7, 4, false);
        w.write_u32(0x11223344u);
        w.finish_emheader(header);
        EXPECT_BYTES(w, "07000020 44332211");
        Reader r(w.bytes(), Endianness::Little, 4);
        const EmHeaderRead read = r.read_emheader();
        CHECK(read.member_id == 7);
        CHECK(read.length_code == 2);
        CHECK(!read.has_next_int);
        r.skip_member(read);
        CHECK(r.remaining() == 0);
    }
    {
        // A length no short code spells keeps its NEXTINT.
        Writer w(Endianness::Little, 4, PlCdr1Framing{}, true);
        const EmHeader header = w.write_emheader(7, 4, false);
        w.write_u8(1);
        w.write_u8(2);
        w.write_u8(3);
        w.finish_emheader(header);
        EXPECT_BYTES(w, "07000040 03000000 010203");
    }
    {
        // The other side of `short_length_codes`: without it the header
        // stays LC4 whatever the member's size turned out to be.
        Writer w(Endianness::Little, 4);
        const EmHeader header = w.write_emheader(7, 4, false);
        w.write_u32(0x11223344u);
        w.finish_emheader(header);
        EXPECT_BYTES(w, "07000040 04000000 44332211");
    }
    {
        Writer w(Endianness::Little, 4);
        EXPECT_THROWS(OutOfRange, { (void)w.write_emheader(1, 8, false); });
    }
}

void parameters() {
    {
        // A mutable value nested in a parameter of a mutable one, the way
        // recursive-optional-external is written under XCDR1: the inner list
        // ends with its own sentinel, and a decoder stops there without
        // finish_pid_read. The enclosing parameter still measures its twelve
        // octets from where its value began.
        const std::vector<std::uint8_t> data =
            unhex("0000 0400 07000000 0100 0c00 0000 0400 08000000 027f 0000 027f 0000");
        Reader r(data, Endianness::Little, 8);
        PidRead outer = r.read_pid();
        CHECK(outer.pid == 0 && outer.length == 4);
        CHECK(r.read_u32() == 7u);
        r.finish_pid_read(outer);
        outer = r.read_pid();
        CHECK(outer.pid == 1 && outer.length == 12);
        PidRead inner = r.read_pid();
        CHECK(inner.pid == 0 && inner.length == 4);
        CHECK(r.read_u32() == 8u);
        r.finish_pid_read(inner);
        inner = r.read_pid();
        CHECK(inner.end);
        r.finish_pid_read(outer);
        CHECK(r.reader_status() == Status::Ok);
        CHECK(r.read_pid().end);
        CHECK(r.remaining() == 0);
    }
    {
        // A parameter value is its own encapsulated block, so an eight-byte
        // member at the head of one takes no padding inside its own declared
        // length — even though the block starts at offset 4 of the stream.
        Writer w(Endianness::Little, 8);
        const Pid header = w.write_pid(2);
        w.write_u64(0x1122334455667788ull);
        w.finish_pid(header);
        EXPECT_BYTES(w, "0200 0800 8877665544332211");

        Reader r(w.bytes(), Endianness::Little, 8);
        const PidRead read = r.read_pid();
        CHECK(read.pid == 2);
        CHECK(read.length == 8);
        CHECK(read.is_user_member());
        const std::size_t saved = r.begin_parameter_value();
        CHECK(r.read_u64() == 0x1122334455667788ull);
        r.finish_parameter_value(saved);
        CHECK(r.remaining() == 0);
    }
    {
        // XTypes 1.3 §7.4.1.2: the declared length is the exact length of the
        // member, and the padding behind it is measured from the enclosing
        // stream — so the origin goes back before it is written.
        Writer w(Endianness::Little, 8);
        w.write_u8(0x01);
        w.write_u8(0x02);
        const Pid header = w.write_pid(5);
        w.write_u8(0x09);
        w.finish_pid(header);
        EXPECT_BYTES(w, "0102 0000 0500 0100 09 000000");
    }
    {
        // The length rounded up to four (DDSI-RTPS 2.5 §9.4.2.11, Cyclone DDS,
        // RTI Connext) is the named option.
        PlCdr1Framing rounded;
        rounded.pad_declared_length = true;
        rounded.pad_declared_optional_length = true;
        Writer w(Endianness::Little, 8, rounded);
        w.write_u8(0x01);
        w.write_u8(0x02);
        const Pid header = w.write_pid(5);
        w.write_u8(0x09);
        w.finish_pid(header);
        EXPECT_BYTES(w, "0102 0000 0500 0400 09 000000");
    }
    {
        // Fast DDS does not pad the declared length.
        PlCdr1Framing fastdds;
        fastdds.pid_flags = PidFlagPolicy::Bare;
        fastdds.pad_declared_length = false;
        fastdds.pad_declared_optional_length = false;
        fastdds.member_must_understand = false;
        Writer w(Endianness::Little, 8, fastdds);
        const Pid header = w.write_pid(5, ParameterRole::Member, true);
        w.write_u8(0x09);
        w.finish_pid(header);
        w.write_sentinel();
        EXPECT_BYTES(w, "0500 0100 09 000000 023f 0000");
    }
    {
        // The clause's framing: a bare member id, must-understand on the
        // sentinel, the member's own bit as bit 14 of the id.
        Writer w(Endianness::Little, 8);
        const Pid header = w.write_pid(5, ParameterRole::Member, true);
        w.write_u8(0x09);
        w.finish_pid(header);
        w.write_sentinel();
        EXPECT_BYTES(w, "0540 0100 09 000000 027f 0000");
        Reader r(w.bytes(), Endianness::Little, 8);
        const PidRead read = r.read_pid();
        CHECK(read.pid == 5);
        CHECK(read.must_understand);
        CHECK(!read.end);
    }
    {
        // OpenDDS: both flags on every member id whatever the member
        // declared, and a bare sentinel. The flag then carries no
        // information, so the reader does not report it — reporting it would
        // send every member down the unknown-parameter path.
        PlCdr1Framing opendds;
        opendds.pid_flags = PidFlagPolicy::ImplExtensionMembers;
        Writer w(Endianness::Little, 8, opendds);
        const Pid header = w.write_pid(1);
        w.write_i32(0);
        w.finish_pid(header);
        w.write_sentinel();
        EXPECT_BYTES(w, "01c0 0400 00000000 023f 0000");
        Reader r(w.bytes(), Endianness::Little, 8, opendds);
        const PidRead read = r.read_pid();
        CHECK(read.pid == 1);
        CHECK(!read.impl_extension);
        CHECK(read.is_user_member());
        // The same bytes read under the clause's framing say the parameter
        // is the writer's own, which is what the bit means there.
        Reader other(w.bytes(), Endianness::Little, 8);
        CHECK(other.read_pid().impl_extension);
    }
    {
        // The extended form. The member's must-understand is `FLAG_2` of the
        // `eMemberHeader` and not the marker's bit above it.
        Writer w(Endianness::Little, 8);
        const Pid header = w.write_pid_extended(0x1234, ParameterRole::Member, true);
        w.write_u32(0xDEADBEEFu);
        w.finish_pid(header);
        EXPECT_BYTES(w, "017f 0800 34120040 04000000 efbeadde");
        Reader r(w.bytes(), Endianness::Little, 8);
        const PidRead read = r.read_pid();
        CHECK(read.pid == 0x1234);
        CHECK(read.length == 4);
        CHECK(read.must_understand);
        CHECK(!read.impl_extension);
    }
    {
        // A value the short form cannot state the length of promotes the
        // header. Which values overrun sixteen bits is a property of the
        // sample and not of the type, so the fallback lives where the length
        // is known.
        Writer w(Endianness::Little, 8);
        const Pid header = w.write_pid(3, ParameterRole::Member, true);
        for (std::size_t i = 0; i < 0x10000; i++) {
            w.write_u8(0xA5);
        }
        w.finish_pid(header);
        CHECK(w.position() == 12 + 0x10000);
        expect_bytes(std::vector<std::uint8_t>(w.bytes().begin(), w.bytes().begin() + 12),
                     "017f 0800 03000040 00000100", __LINE__);
        Reader r(w.bytes(), Endianness::Little, 8);
        const PidRead read = r.read_pid();
        CHECK(read.pid == 3);
        CHECK(read.length == 0x10000);
        CHECK(read.must_understand);
        // Every byte of the value survived the insertion unchanged.
        CHECK(w.bytes()[12] == 0xA5);
        CHECK(w.bytes().back() == 0xA5);
    }
    {
        // The sentinel and the reserved range.
        const std::vector<std::uint8_t> data = unhex("027f 0000");
        Reader r(data, Endianness::Little, 8);
        const PidRead read = r.read_pid();
        CHECK(read.end);
        CHECK(read.pid == (PID_LIST_END & PID_ID_MASK));
    }
    {
        const std::vector<std::uint8_t> data = unhex("033f 0000");
        Reader r(data, Endianness::Little, 8);
        const PidRead read = r.read_pid();
        CHECK(read.ignore);
        CHECK(!read.is_user_member());
    }
    {
        // §7.4.1.2.2 stops the short form's ids at 0x3F00 inclusive; one
        // implementation carries the whole low fourteen bits, and a reader
        // holding the clause's threshold as a constant drops `@id(16383)`.
        const std::vector<std::uint8_t> data = unhex("ff3f 0400 00000000");
        Reader strict(data, Endianness::Little, 8);
        CHECK(strict.read_pid().reserved);
        PlCdr1Framing wide;
        wide.highest_short_id = 0x3FFF;
        Reader lenient(data, Endianness::Little, 8, wide);
        CHECK(!lenient.read_pid().reserved);
    }
    {
        CHECK(pid_flags_at(PidFlagPolicy::SentinelMustUnderstand, WirePosition::Member) == 0);
        CHECK(pid_flags_at(PidFlagPolicy::SentinelMustUnderstand, WirePosition::Sentinel) ==
              PID_MUST_UNDERSTAND);
        CHECK(pid_flags_at(PidFlagPolicy::Bare, WirePosition::Sentinel) == 0);
        CHECK(pid_flags_at(PidFlagPolicy::ImplExtensionMembers, WirePosition::Member) ==
              (PID_IMPL_EXTENSION | PID_MUST_UNDERSTAND));
        CHECK(pid_flags_at(PidFlagPolicy::ImplExtensionMembers, WirePosition::Sentinel) == 0);
        CHECK(pid_flags_at(PidFlagPolicy::SentinelMustUnderstand, WirePosition::Element) == 0);
    }
}

void bit_bounded() {
    {
        // A signed holder: `@value(-1)` at the default bound is `ffffffff`.
        Writer w(Endianness::Little, 4);
        w.write_enum(32, -1);
        EXPECT_BYTES(w, "ffffffff");
    }
    {
        // What fits is everything the bound's bits express in either
        // signedness: RTI accepts `@value(-1)` at the default bound and
        // Cyclone `@value(200)` under `@bit_bound(8)`.
        Writer w(Endianness::Little, 4);
        w.write_enum(8, 200);
        w.write_enum(8, -1);
        EXPECT_BYTES(w, "c8 ff");
        Writer v(Endianness::Little, 4);
        EXPECT_THROWS(OutOfRange, { v.write_enum(8, 256); });
        EXPECT_THROWS(OutOfRange, { v.write_enum(8, -129); });
        EXPECT_THROWS(OutOfRange, { v.write_enum(33, 0); });
    }
    {
        Writer w(Endianness::Big, 4);
        w.write_bitmask(16, 0x8001u);
        EXPECT_BYTES(w, "8001");
        Reader r(w.bytes(), Endianness::Big, 4);
        CHECK(r.read_bitmask(16) == 0x8001u);
        Writer v(Endianness::Big, 4);
        EXPECT_THROWS(OutOfRange, { v.write_bitmask(8, 256); });
        EXPECT_THROWS(OutOfRange, { v.write_bitmask(65, 0); });
    }
    {
        Writer w(Endianness::Little, 4);
        w.write_bitset(2, 0x0102u);
        EXPECT_BYTES(w, "0201");
        Reader r(w.bytes(), Endianness::Little, 4);
        CHECK(r.read_bitset(2) == 0x0102u);
        Writer v(Endianness::Little, 4);
        EXPECT_THROWS(OutOfRange, { v.write_bitset(3, 0); });
        CHECK(v.bitfield(5, 3, 4) == (5u << 4));
        EXPECT_THROWS(OutOfRange, { (void)v.bitfield(8, 3, 4); });
    }
    {
        Reader r(nullptr, 0, Endianness::Little, 4);
        EXPECT_THROWS(OutOfRange, { (void)r.read_enum(0); });
        EXPECT_THROWS(OutOfRange, { (void)r.read_bitmask(65); });
        EXPECT_THROWS(OutOfRange, { (void)r.read_bitset(3); });
    }
    {
        // A sequence claiming more elements than the stream has bytes left
        // is how a reader turns a bad message into an out-of-memory.
        const std::vector<std::uint8_t> data = unhex("ffffff7f");
        Reader r(data, Endianness::Little, 4);
        EXPECT_THROWS(ImpossibleLength, { (void)r.read_sequence_length(); });
        const std::vector<std::uint8_t> ok = unhex("02000000 0102");
        Reader s(ok, Endianness::Little, 4);
        CHECK(s.read_sequence_length() == 2);
    }
}

// -- binary128 --------------------------------------------------------------

void float128() {
    // 1 + 2^-112: the smallest value above one that binary128 can hold, and
    // one no `double` can. The writer used to take a `float` and follow it
    // with eight zero bytes — the right length, the wrong value, in silence.
    const Float128 tiny = Float128::from_parts(0x3FFF000000000000ull, 1);
    {
        double narrowed = 0.0;
        CHECK(!tiny.to_double_checked(&narrowed));
        CHECK(narrowed == 1.0);
        CHECK(Float128::from_double(1.0) != tiny);
    }
    for (Endianness order : {Endianness::Little, Endianness::Big}) {
        Writer w(order, 8);
        w.write_f128(tiny);
        CHECK(w.position() == 16);
        Reader r(w.bytes(), order, 8);
        CHECK(r.read_f128() == tiny);
    }
    {
        // The two byte orders are the whole sixteen bytes reversed, not two
        // eight-byte halves swapped in place. [CDR] Figure 9.2.
        Writer big(Endianness::Big, 8);
        big.write_f128(tiny);
        EXPECT_BYTES(big, "3fff000000000000 0000000000000001");
        Writer little(Endianness::Little, 8);
        little.write_f128(tiny);
        EXPECT_BYTES(little, "0100000000000000 000000000000ff3f");
    }
    {
        // Widening a `double` is always exact; narrowing reports.
        for (double value : {0.0, -0.0, 1.0, -2.75, 3.141592653589793, 1e300, 5e-324}) {
            const Float128 wide = Float128::from_double(value);
            double back = 0.0;
            CHECK(wide.to_double_checked(&back));
            CHECK(std::memcmp(&back, &value, sizeof value) == 0);
        }
        CHECK(Float128::from_double(1.0 / 0.0).is_infinite());
        CHECK(Float128::from_double(0.0 / 0.0 * 0.0 + (0.0 / 0.0)).is_nan());
        CHECK(Float128::zero(true).is_negative());
        CHECK(Float128::zero(true).is_zero());
        CHECK(!Float128::zero(true).same_bits(Float128::zero(false)));
    }
    {
        // The exact expansion, in the canonical form all four columns print.
        EXPECT_EQ(Float128::from_double(-2.75).to_exact_decimal(), "-2.75");
        EXPECT_EQ(Float128::zero().to_exact_decimal(), "0.0");
        EXPECT_EQ(Float128::zero(true).to_exact_decimal(), "-0.0");
        EXPECT_EQ(Float128::one().to_exact_decimal(), "1.0");
        EXPECT_EQ(Float128::infinity(true).to_exact_decimal(), "-inf");
        EXPECT_EQ(Float128::quiet_nan().to_exact_decimal(), "nan");
        EXPECT_EQ(Float128::from_double(0.5).to_exact_decimal(), "0.5");
        EXPECT_EQ(Float128::from_double(0.25).to_exact_decimal(), "0.25");
    }
    {
        // Parsing is one rounding and never through a `double`: a two-step
        // parse rounds twice, and double rounding gets the last bit wrong
        // for exactly the values this type exists to carry.
        CHECK(Float128::from_decimal("-2.75") == Float128::from_double(-2.75));
        CHECK(Float128::from_decimal("1") == Float128::one());
        CHECK(Float128::from_decimal("inf").is_infinite());
        CHECK(Float128::from_decimal("-INFINITY").is_infinite());
        CHECK(Float128::from_decimal("-INFINITY").is_negative());
        CHECK(Float128::from_decimal("nan").is_nan());
        CHECK(Float128::from_decimal("0").is_zero());
        CHECK(Float128::from_decimal("-0").is_negative());
        CHECK(Float128::from_decimal("1e-100000").is_zero());
        CHECK(Float128::from_decimal("1e100000").is_infinite());
        EXPECT_THROWS(OutOfRange, { (void)Float128::from_decimal("1_5"); });
        EXPECT_THROWS(OutOfRange, { (void)Float128::from_decimal("1e"); });
        EXPECT_THROWS(OutOfRange, { (void)Float128::from_decimal("."); });
    }
    {
        // The four values `tests/interop-corpus/primitive-longdouble` names,
        // each of which needs more significand bits than a `double` has, and
        // each of which is stated to 34 decimal digits — the point where a
        // parser that rounded twice returns the neighbouring pattern.
        struct Case {
            const char *text;
            std::uint64_t high;
            std::uint64_t low;
        };
        const Case cases[] = {
            {"1.0000000000000000008673617379884035", 0x3FFF000000000000ull,
             0x0010000000000000ull},
            {"1.0000000000000000000000000000000002", 0x3FFF000000000000ull, 1ull},
            {"3.3621031431120935062626778173217526e-4932", 0x0001000000000000ull, 0ull},
            {"3.362103143112093506262677817321752e-4932", 0x0000FFFFFFFFFFFFull,
             ~std::uint64_t{0}},
            {"1.189731495357231765085759326628007e+4932", 0x7FFEFFFFFFFFFFFFull,
             ~std::uint64_t{0}},
        };
        for (const Case &entry : cases) {
            const Float128 parsed = Float128::from_decimal(entry.text);
            const Float128 want = Float128::from_parts(entry.high, entry.low);
            if (!parsed.same_bits(want)) {
                fail(__LINE__, std::string("`") + entry.text + "` parsed to " +
                                   hex(parsed.to_bytes(Endianness::Big).data(), 16) +
                                   ", want " + hex(want.to_bytes(Endianness::Big).data(), 16));
            }
        }
    }
    {
        // Print and parse are inverses over the whole range, including the
        // subnormals whose exact expansion runs to eleven thousand digits.
        const Float128 values[] = {
            tiny,
            Float128::from_parts(0x0000FFFFFFFFFFFFull, ~std::uint64_t{0}),  // largest subnormal
            Float128::from_parts(0x0001000000000000ull, 0),                  // smallest normal
            Float128::from_parts(0x7FFEFFFFFFFFFFFFull, ~std::uint64_t{0}),  // largest finite
            Float128::from_parts(0, 1),                                      // smallest subnormal
            Float128::from_parts(0x8000000000000000ull, 1),
            Float128::from_double(3.141592653589793),
        };
        for (const Float128 &value : values) {
            const std::string text = value.to_exact_decimal();
            const Float128 back = Float128::from_decimal(text);
            if (!back.same_bits(value)) {
                fail(__LINE__, "`" + text.substr(0, 60) + "...` (" +
                                   std::to_string(text.size()) + " chars) read back as " +
                                   hex(back.to_bytes(Endianness::Big).data(), 16) + ", want " +
                                   hex(value.to_bytes(Endianness::Big).data(), 16));
            }
        }
        // The smallest subnormal's exact decimal really is that long: it is
        // `2^-16494`, whose expansion has 16 494 places after the point.
        CHECK(Float128::from_parts(0, 1).to_exact_decimal().size() > 4900);
    }
}

// -- MD5 --------------------------------------------------------------------

void md5_vectors() {
    // RFC 1321 appendix A.5, the whole suite.
    struct Vector {
        const char *input;
        const char *digest;
    };
    const Vector vectors[] = {
        {"", "d41d8cd98f00b204e9800998ecf8427e"},
        {"a", "0cc175b9c0f1b6a831c399e269772661"},
        {"abc", "900150983cd24fb0d6963f7d28e17f72"},
        {"message digest", "f96b697d7cb7938d525a2f31aaf161d0"},
        {"abcdefghijklmnopqrstuvwxyz", "c3fcd3d76192e4007dfb496cca67e13b"},
        {"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
         "d174ab98d277d9f5a5611c2c9f419d9f"},
        {"123456789012345678901234567890123456789012345678901234567890123456789012"
         "34567890",
         "57edf4a22be3c955ac49da2e2107b67a"},
    };
    for (const Vector &entry : vectors) {
        const std::array<std::uint8_t, 16> digest = md5(std::string_view(entry.input));
        expect_equal(hex(digest.data(), digest.size()), entry.digest, __LINE__);
    }
    // A block boundary either side of 56, which is where the padding decides
    // whether it needs a second block.
    for (std::size_t length : {55u, 56u, 57u, 63u, 64u, 65u, 119u, 120u, 128u}) {
        const std::string input(length, 'x');
        const std::array<std::uint8_t, 16> digest = md5(std::string_view(input));
        CHECK(digest.size() == 16);
    }
    expect_equal(hex(md5(std::string_view(std::string(56, 'a'))).data(), 16),
                 "3b0c8ac703f828b04c6c197006d17218", __LINE__);
    expect_equal(hex(md5(std::string_view(std::string(64, 'a'))).data(), 16),
                 "014842d480b571495a4a0363793f7367", __LINE__);

    // The KeyHash itself: zero-padded for a holder of sixteen bytes or
    // fewer, MD5 of it when it is longer. DDSI-RTPS 2.5 §9.6.3.8.
    {
        Writer w = Writer::big_endian();
        w.write_u32(1);
        const std::array<std::uint8_t, 16> hash = w.zero_padded();
        expect_equal(hex(hash.data(), hash.size()), "00000001000000000000000000000000",
                     __LINE__);
    }
    {
        Writer w = Writer::big_endian();
        for (int i = 0; i < 5; i++) {
            w.write_u32(0);
        }
        CHECK(w.position() == 20);
        const std::array<std::uint8_t, 16> hash = w.md5();
        const std::array<std::uint8_t, 16> want = md5(w.bytes().data(), w.bytes().size());
        CHECK(hash == want);
    }
}

// -- the encapsulation header ----------------------------------------------

void encapsulation() {
    struct Case {
        Encoding encoding;
        Endianness order;
        const char *bytes;
    };
    const Case cases[] = {
        {Encoding::PlainCdr1, Endianness::Big, "00000000"},
        {Encoding::PlainCdr1, Endianness::Little, "00010000"},
        {Encoding::PlCdr1, Endianness::Big, "00020000"},
        {Encoding::PlCdr1, Endianness::Little, "00030000"},
        {Encoding::PlainCdr2, Endianness::Big, "00060000"},
        {Encoding::PlainCdr2, Endianness::Little, "00070000"},
        {Encoding::DelimitedCdr, Endianness::Big, "00080000"},
        {Encoding::DelimitedCdr, Endianness::Little, "00090000"},
        {Encoding::PlCdr2, Endianness::Big, "000a0000"},
        {Encoding::PlCdr2, Endianness::Little, "000b0000"},
    };
    for (const Case &entry : cases) {
        const std::array<std::uint8_t, 4> header =
            encapsulation_header(entry.encoding, entry.order);
        expect_equal(hex(header.data(), header.size()), entry.bytes, __LINE__);
    }
    // `@final struct One { octet a; }` goes out as `0001 0003 ab000000` from
    // Cyclone, RTI and Fast DDS alike: the padding count is stated.
    {
        const std::array<std::uint8_t, 4> header =
            encapsulation_header(Encoding::PlainCdr1, Endianness::Little,
                                 encapsulation_padding(1));
        expect_equal(hex(header.data(), header.size()), "00010003", __LINE__);
    }
    CHECK(encapsulation_padding(0) == 0);
    CHECK(encapsulation_padding(1) == 3);
    CHECK(encapsulation_padding(3) == 1);
    CHECK(encapsulation_padding(4) == 0);
    CHECK(max_alignment(Encoding::PlainCdr1) == 8);
    CHECK(max_alignment(Encoding::PlCdr1) == 8);
    CHECK(max_alignment(Encoding::PlainCdr2) == 4);
    CHECK(max_alignment(Encoding::DelimitedCdr) == 4);
    CHECK(max_alignment(Encoding::PlCdr2) == 4);
    CHECK(is_v1(Encoding::PlCdr1) && is_v2(Encoding::PlCdr2));
    CHECK(parse_encoding("xcdr2").value() == Encoding::PlainCdr2);
    CHECK(!parse_encoding("xcdr3").has_value());
    CHECK(parse_endianness("big").value() == Endianness::Big);
    CHECK(!parse_endianness("middle").has_value());
}

// -- the wstring axes as a product ------------------------------------------

void wstring_axes() {
    // All eight cells, reachable by name and by coordinate, and answering
    // the three questions consistently.
    struct Cell {
        WstringLengthPolicy policy;
        const char *coordinate;
        bool counts;
        bool terminated;
        std::size_t width;
    };
    const Cell cells[] = {
        {WstringLengthPolicy::Specification, "octets/unterminated/2", false, false, 2},
        {WstringLengthPolicy::CharacterCount, "characters/unterminated/2", true, false, 2},
        {WstringLengthPolicy::CharacterCountTerminated, "characters/terminated/4", true, true, 4},
        {WstringLengthPolicy::SpecificationWideUnits, "octets/unterminated/4", false, false, 4},
        {WstringLengthPolicy::RosTypeSupport, "characters/unterminated/4", true, false, 4},
        {WstringLengthPolicy::OctetsTerminated2, "octets/terminated/2", false, true, 2},
        {WstringLengthPolicy::OctetsTerminated4, "octets/terminated/4", false, true, 4},
        {WstringLengthPolicy::CharactersTerminated2, "characters/terminated/2", true, true, 2},
    };
    for (const Cell &cell : cells) {
        CHECK(wstring_counts_characters(cell.policy) == cell.counts);
        CHECK(wstring_terminated(cell.policy) == cell.terminated);
        CHECK(wstring_unit_width(cell.policy) == cell.width);
        CHECK(parse_wstring_policy(cell.coordinate).value() == cell.policy);
        // Every cell round-trips a BMP string in every unit order.
        for (WstringUnitOrder order :
             {WstringUnitOrder::Stream, WstringUnitOrder::BigEndianAlways,
              WstringUnitOrder::ByteOrderMark, WstringUnitOrder::HostUnswapped}) {
            for (Endianness endianness : {Endianness::Little, Endianness::Big}) {
                Writer w(endianness, 8);
                w.write_wstring_utf8("a\xc3\xa4z", cell.policy, order);
                Reader r(w.bytes(), endianness, 8);
                EXPECT_EQ(r.read_wstring(cell.policy, order), "a\xc3\xa4z");
                CHECK(r.remaining() == 0);
            }
        }
    }
    CHECK(parse_wstring_policy("specification").value() == WstringLengthPolicy::Specification);
    CHECK(parse_wstring_policy("ros-type-support").value() ==
          WstringLengthPolicy::RosTypeSupport);
    CHECK(!parse_wstring_policy("no-such-cell").has_value());
}

// -- the three axes `Dialect::Hdds` selects ---------------------------------
//
// Expected octets from the recorded HDDS columns:
// `tests/interop-results/array-1d-struct-final/hdds/plain-cdr2.le.bin`,
// `tests/interop-results/typedef-of-struct/hdds/plain-cdr2.le.bin`,
// `tests/interop-results/sequence-struct-appendable/hdds/plain-cdr2.le.bin`,
// `tests/interop-results/wstring-member/hdds/plain-cdr2.le.bin`,
// `tests/interop-results/wstring-unbounded-utf8/hdds/plain-cdr2.le.bin` and
// `tests/interop-results/wstring-bounded-utf8/hdds/plain-cdr2.le.bin`.

void hdds_axes() {
    {
        // `ElementPadding::Packed`: each collection element counts its own
        // alignment from its own first octet, so the second `Cell`'s `long`
        // lands at offset six and not eight.
        Writer w(Endianness::Little, 4);
        std::size_t saved = w.begin_packed_element();
        w.write_i32(1);
        w.write_i16(2);
        w.finish_packed_element(saved);
        saved = w.begin_packed_element();
        w.write_i32(3);
        w.write_i16(4);
        w.finish_packed_element(saved);
        w.write_u8(119);
        EXPECT_BYTES(w, "01000000 0200 03000000 0400 77");

        Reader r(w.bytes(), Endianness::Little, 4);
        saved = r.reader_begin_packed_element();
        CHECK(r.read_i32() == 1 && r.read_i16() == 2);
        r.reader_finish_packed_element(saved);
        saved = r.reader_begin_packed_element();
        CHECK(r.read_i32() == 3 && r.read_i16() == 4);
        r.reader_finish_packed_element(saved);
        CHECK(r.read_u8() == 119);
    }
    {
        // The same inside two `@final` members.
        Writer w(Endianness::Little, 4);
        std::size_t saved = w.begin_packed_element();
        w.write_i32(-1234567);
        w.write_i16(-2);
        w.finish_packed_element(saved);
        saved = w.begin_packed_element();
        w.write_i32(1);
        w.write_i16(32767);
        w.finish_packed_element(saved);
        EXPECT_BYTES(w, "7929edff feff 01000000 ff7f");
    }
    {
        // A sequence element's own length in DHEADER form: the `06 00 00 00`
        // in front of `7929edff feff` is the element's six octets, not the
        // encapsulation's.
        Writer w(Endianness::Little, 4);
        const DHeader element = w.write_dheader();
        w.write_i32(-1234567);
        w.write_i16(-2);
        w.finish_dheader(element);
        EXPECT_BYTES(w, "06000000 7929edff feff");

        Reader r(w.bytes(), Endianness::Little, 4);
        CHECK(r.read_dheader() == 6);
        CHECK(r.read_i32() == -1234567);
        CHECK(r.read_i16() == -2);
    }
    {
        // `WstringCodeUnits::Utf8`: the characters' own octets, the octet
        // count with the terminator, and C2 still reached.
        Writer w(Endianness::Little, 4);
        w.write_wstring_utf8_octets("abc", WstringLengthPolicy::OctetsTerminated2);
        EXPECT_BYTES(w, "04000000 61626300");

        Writer v(Endianness::Little, 4);
        v.write_wstring_utf8_octets("\xc3\xa4\xc3\xb6\xc3\xbc \xe1\x88\xb4 Z",
                                    WstringLengthPolicy::OctetsTerminated2);
        EXPECT_BYTES(v, "0d000000 c3a4c3b6c3bc20e188b4205a00");
    }
    {
        // A bounded UTF-8 wstring: the bound still counts the three UTF-16
        // code units, and the wire carries the six octets plus the NUL.
        Writer w(Endianness::Little, 4);
        w.write_wstring_utf8_octets_bounded("\xe1\x88\xb4\xc3\xa4Z", 3,
                                            WstringLengthPolicy::OctetsTerminated2);
        EXPECT_BYTES(w, "07000000 e188b4c3a45a00");
    }
    {
        const std::vector<std::uint8_t> data =
            unhex("0d000000 c3a4c3b6c3bc20e188b4205a00");
        Reader r(data, Endianness::Little, 4);
        EXPECT_EQ(r.read_wstring_utf8_octets(WstringLengthPolicy::OctetsTerminated2),
                  "\xc3\xa4\xc3\xb6\xc3\xbc \xe1\x88\xb4 Z");
        CHECK(r.remaining() == 0);
    }
    {
        // A malformed UTF-8 sequence and a count the stream cannot hold are
        // refused, never replaced or truncated.
        Writer w(Endianness::Little, 4);
        EXPECT_THROWS(NotUtf8,
                      { w.write_wstring_utf8_octets("\xc3",
                                                    WstringLengthPolicy::OctetsTerminated2); });
        const std::vector<std::uint8_t> bad = unhex("ff000000 6100");
        Reader r(bad, Endianness::Little, 4);
        EXPECT_THROWS(ImpossibleLength,
                      { (void)r.read_wstring_utf8_octets(); });
    }
}

// -- the cursor -------------------------------------------------------------

void cursor() {
    const std::vector<std::uint8_t> data = unhex("00010203040506070809");
    Reader r(data, Endianness::Little, 8);
    CHECK(r.position() == 0);
    CHECK(r.remaining() == 10);
    r.skip(3);
    CHECK(r.position() == 3);
    r.seek(8);
    CHECK(r.remaining() == 2);
    r.seek(0);
    CHECK(r.read_u8() == 0);
    EXPECT_THROWS(Truncated, { r.seek(11); });
    EXPECT_THROWS(Truncated, { r.skip(100); });
}

}  // namespace

// DDS-XTypes 1.3 §7.5.1.1.2, Table 43: the typedefs the Service provides,
// with the sizes the table's C99 column gives. `Int8` and `UInt8` are
// distinct types, as are `Int16` and `UInt16`: "all other mappings for basic
// types shall be distinguishable for the purposes of overloading".
static_assert(sizeof(DDS::Int32) == 4, "DDS::Int32");
static_assert(sizeof(DDS::Int64) == 8, "DDS::Int64");
static_assert(sizeof(DDS::Float128) == 16, "DDS::Float128");
static_assert(!std::is_same<DDS::Int16, DDS::UInt16>::value, "Int16, UInt16");
static_assert(!std::is_same<DDS::Int32, DDS::Int64>::value, "Int32, Int64");
static_assert(!std::is_same<DDS::Float32, DDS::Float64>::value, "Float32, Float64");

// §7.5.1.1.2: "Types DDS::Boolean, DDS::Char8, and DDS::Byte may all map to the same underlying
// C++ type. [...] All other mappings for basic types shall be distinguishable for the purposes
// of overloading. That is, one can safely write overloaded C++ functions for DDS::Int16,
// DDS::UInt16, DDS::Int32, and so on." One overload per type of Table 43 but Char16, which "may
// not be distinguishable from integer types" (it is `UInt16`'s type). Two of them of one type
// would not compile, so the existence of the program is the check; the values show each call
// went where its type is.
int overloaded(DDS::Int32) { return 1; }
int overloaded(DDS::UInt32) { return 2; }
int overloaded(DDS::Int8) { return 3; }
int overloaded(DDS::UInt8) { return 4; }
int overloaded(DDS::Int16) { return 5; }
int overloaded(DDS::UInt16) { return 6; }
int overloaded(DDS::Int64) { return 7; }
int overloaded(DDS::UInt64) { return 8; }
int overloaded(DDS::Float32) { return 9; }
int overloaded(DDS::Float64) { return 10; }
int overloaded(DDS::Float128) { return 11; }
int overloaded(DDS::Byte) { return 12; }
int overloaded(DDS::Char8) { return 13; }
int overloaded(DDS::Boolean) { return 14; }

void the_types_of_table_43_are_distinguishable_for_overloading() {
    CHECK(overloaded(DDS::Int32(0)) == 1 && overloaded(DDS::UInt32(0)) == 2);
    CHECK(overloaded(DDS::Int8(0)) == 3 && overloaded(DDS::UInt8(0)) == 4);
    CHECK(overloaded(DDS::Int16(0)) == 5 && overloaded(DDS::UInt16(0)) == 6);
    CHECK(overloaded(DDS::Int64(0)) == 7 && overloaded(DDS::UInt64(0)) == 8);
    CHECK(overloaded(DDS::Float32(0)) == 9 && overloaded(DDS::Float64(0)) == 10);
    CHECK(overloaded(DDS::Float128::one()) == 11);
    // The one the table leaves free to coincide with `UInt8`'s type was the trap.
    CHECK(overloaded(DDS::Byte(0)) == 12 && overloaded(DDS::UInt8(0)) == 4);
    CHECK(overloaded(DDS::Char8('a')) == 13 && overloaded(DDS::Boolean(true)) == 14);
    static_assert(!std::is_same<DDS::Byte, DDS::UInt8>::value, "Byte, UInt8");
    static_assert(!std::is_same<DDS::Byte, DDS::Int8>::value, "Byte, Int8");
    // A byte converts to and from the octet the wire runtime reads and writes.
    DDS::Byte byte = 0xa5u;
    const std::uint8_t octet = byte;
    CHECK(octet == 0xa5u && DDS::Byte(octet) == byte);
}

void the_primitive_typedefs_of_table_43_exist() {
    DDS::Int32 i32 = -1;
    DDS::UInt32 u32 = 0xffffffffu;
    DDS::Int8 i8 = -1;
    DDS::UInt8 u8 = 255u;
    DDS::Int16 i16 = -1;
    DDS::UInt16 u16 = 65535u;
    DDS::Int64 i64 = -1;
    DDS::UInt64 u64 = 0xffffffffffffffffull;
    DDS::Float32 f32 = 1.5f;
    DDS::Float64 f64 = 2.5;
    DDS::Float128 f128 = DDS::Float128::one();
    DDS::Char8 c8 = 'a';
    DDS::Char16 c16 = 0x20acu;
    DDS::Boolean flag = true;
    DDS::Byte byte = 0xa5u;
    CHECK(i32 < 0 && i8 < 0 && i16 < 0 && i64 < 0);
    CHECK(u32 == 0xffffffffu && u8 == 255u && u16 == 65535u);
    CHECK(u64 == 0xffffffffffffffffull);
    CHECK(f32 == 1.5f && f64 == 2.5);
    CHECK(f128 == DDS::Float128::one());
    CHECK(c8 == 'a' && c16 == 0x20acu && flag && byte == 0xa5u);
}

int main() {
    the_primitive_typedefs_of_table_43_exist();
    the_types_of_table_43_are_distinguishable_for_overloading();
    alignment();
    primitives();
    strings();
    wstrings();
    fixed_point();
    dheader();
    emheaders();
    parameters();
    bit_bounded();
    float128();
    md5_vectors();
    encapsulation();
    wstring_axes();
    hdds_axes();
    cursor();
    return failures == 0 ? 0 : 1;
}
