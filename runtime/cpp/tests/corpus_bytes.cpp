// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The C++ column against a foreign answer.
//
// The self-test proves this runtime agrees with itself and with the byte
// strings its author wrote down. This proves it agrees with a payload
// another implementation produced and recorded: three cases of
// `tests/interop-corpus` whose types are pure primitives, written here with
// this `Writer` and compared byte for byte against
// `tests/interop-results/<case>/zerodds-rust/plain-cdr2.le.bin` and
// `plain-cdr1.be.bin`.
//
// The two files are the two cases that separate the encodings: XCDR1's
// MAXALIGN is eight and XCDR2's is four, so the same fourteen members are
// 56 bytes in one and 48 in the other, and a column that missed the cap
// matches neither.
//
// The field sequences below are read off `type.idl` and the values off
// `values.json` by hand, on purpose. A program that parsed the corpus would
// be a second implementation of the corpus reader and would fail for its own
// reasons; here the only thing under test is the wire runtime.
//
//     ./corpus_bytes [corpus-root]     default ../../tests

#include "zerodds/wire.hpp"

#include <cstdio>
#include <fstream>
#include <string>
#include <vector>

using namespace zerodds::wire;

namespace {

std::string hex(const std::vector<std::uint8_t> &data, std::size_t from, std::size_t count) {
    static const char digits[] = "0123456789abcdef";
    std::string out;
    for (std::size_t i = from; i < data.size() && i < from + count; i++) {
        out.push_back(digits[data[i] >> 4]);
        out.push_back(digits[data[i] & 0x0Fu]);
    }
    return out;
}

bool read_file(const std::string &path, std::vector<std::uint8_t> &out) {
    std::ifstream file(path, std::ios::binary);
    if (!file) {
        return false;
    }
    out.assign(std::istreambuf_iterator<char>(file), std::istreambuf_iterator<char>());
    return true;
}

// -- the three cases --------------------------------------------------------

/// `primitive-mixed` — every primitive in one aggregate, ascending
/// declaration order, so every internal padding rule fires at least once.
void primitive_mixed(Writer &w) {
    w.write_bool(true);                     // boolean flag
    w.write_u8(171);                        // octet raw_octet
    w.write_u8(254);                        // uint8 unsigned_byte
    w.write_i8(-13);                        // int8 signed_byte
    w.write_char(U'Z');                     // char letter
    w.write_wchar(U'ä');               // wchar wide_letter
    w.write_i16(-12345);                    // short short_signed
    w.write_u16(48879);                     // unsigned short short_unsigned
    w.write_i32(-1234567);                  // long long_signed
    w.write_u32(3735928559u);               // unsigned long long_unsigned
    w.write_i64(-987654321987LL);           // long long llong_signed
    w.write_u64(16045690984833335998ull);   // unsigned long long llong_unsigned
    w.write_f32(-3.75);                     // float single_float
    w.write_f64(3.141592653589793);         // double double_float
}

/// `primitive-mixed-descending-alignment` — the same primitives widest
/// first, so the padding falls in the other places.
void primitive_mixed_descending(Writer &w) {
    w.write_f64(-2.5);                              // double double_float
    w.write_f32(2.5);                               // float single_float
    w.write_u64(7ull);                              // unsigned long long llong_unsigned
    w.write_i64(9223372036854775807LL);             // long long llong_signed
    w.write_u32(5u);                                // unsigned long long_unsigned
    w.write_i32(2147483647);                        // long long_signed
    w.write_u16(3);                                 // unsigned short short_unsigned
    w.write_i16(32767);                             // short short_signed
    w.write_wchar(U'ሴ');                       // wchar wide_letter
    w.write_char(U'~');                             // char letter
    w.write_i8(127);                                // int8 signed_byte
    w.write_u8(3);                                  // uint8 unsigned_byte
    w.write_u8(1);                                  // octet raw_octet
    w.write_bool(false);                            // boolean flag
}

/// `primitive-longdouble` — six binary128 values, five of which no `double`
/// can hold. The decimals are the corpus's, parsed here rather than reduced
/// to bit patterns: a column that parsed them a hundredth of an ulp
/// differently would produce sixteen bytes that look entirely plausible.
void primitive_longdouble(Writer &w) {
    w.write_f128(Float128::from_decimal("-2.75"));
    w.write_f128(Float128::from_decimal("1.0000000000000000008673617379884035"));
    w.write_f128(Float128::from_decimal("1.0000000000000000000000000000000002"));
    w.write_f128(Float128::from_decimal("3.3621031431120935062626778173217526e-4932"));
    w.write_f128(Float128::from_decimal("3.362103143112093506262677817321752e-4932"));
    w.write_f128(Float128::from_decimal("1.189731495357231765085759326628007e+4932"));
}

struct Case {
    const char *name;
    void (*write)(Writer &);
};

int compare(const std::string &root, const Case &entry, const char *file,
            Endianness order, std::size_t max_align) {
    const std::string path =
        root + "/interop-results/" + entry.name + "/zerodds-rust/" + file;
    std::vector<std::uint8_t> recorded;
    if (!read_file(path, recorded)) {
        std::fprintf(stderr, "%-40s %-18s MISSING %s\n", entry.name, file, path.c_str());
        return 1;
    }
    Writer w(order, max_align);
    entry.write(w);
    const std::vector<std::uint8_t> produced = w.finish();
    if (produced == recorded) {
        std::printf("%-40s %-18s MATCH  %zu bytes\n", entry.name, file, produced.size());
        return 0;
    }
    std::size_t at = 0;
    while (at < produced.size() && at < recorded.size() && produced[at] == recorded[at]) {
        at++;
    }
    std::printf("%-40s %-18s DIFFER at offset %zu (%zu bytes produced, %zu recorded)\n",
                entry.name, file, at, produced.size(), recorded.size());
    std::printf("    produced %s\n", hex(produced, at, 16).c_str());
    std::printf("    recorded %s\n", hex(recorded, at, 16).c_str());
    return 1;
}

}  // namespace

int main(int argc, char **argv) {
    const std::string root = argc > 1 ? argv[1] : "../../tests";
    const Case cases[] = {
        {"primitive-mixed", primitive_mixed},
        {"primitive-mixed-descending-alignment", primitive_mixed_descending},
        {"primitive-longdouble", primitive_longdouble},
    };
    int bad = 0;
    for (const Case &entry : cases) {
        bad += compare(root, entry, "plain-cdr2.le.bin", Endianness::Little, 4);
        bad += compare(root, entry, "plain-cdr1.be.bin", Endianness::Big, 8);
    }
    if (bad != 0) {
        std::fprintf(stderr, "%d of 6 comparisons did not match\n", bad);
    }
    return bad == 0 ? 0 : 1;
}
