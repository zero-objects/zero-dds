// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Byte-identity test for the C++98 endpoint wire-core (ADR 0013 P3).
// Encodes the fixed SensorReading (same vector as the C/Python tests + Rust
// golden generator) in LE and BE, compares to the goldens, round-trips.
//
// usage: test_byte_identity <golden_le.bin> <golden_be.bin>

#include <cstring>
#include <string>

#include "golden.hpp"

static int check(int endian, const char* golden_path, const char* tag) {
    unsigned char out[256];
    zerodds::Writer w(out, sizeof(out), endian);
    zdw_u64_t stamp; stamp.hi = 0x01020304uL; stamp.lo = 0x05060708uL;
    unsigned char raw[4]; raw[0] = 0xDE; raw[1] = 0xAD; raw[2] = 0xBE; raw[3] = 0xEF;

    w.u32(0xA1B2C3D4uL);
    w.u16(0x1234u);
    w.u8(0x5Au);
    w.f32(3.5f);
    w.u64(stamp);
    w.str(std::string("bay-12"));
    w.seq_u8(raw, 4);
    if (matches_golden(w, out, golden_path, tag, "") != 0) return 1;

    zerodds::Reader r(out, w.size(), endian);
    unsigned long id = r.u32();
    unsigned int kind = r.u16();
    unsigned char flags = r.u8();
    float value = r.f32();
    zdw_u64_t st = r.u64();
    std::string label = r.str();
    unsigned char rb[64]; size_t rn = r.seq_u8(rb, sizeof(rb));
    bool decoded = r.error() == ZDW_OK && id == 0xA1B2C3D4uL && kind == 0x1234u &&
                   flags == 0x5Au && value == 3.5f && st.hi == stamp.hi &&
                   st.lo == stamp.lo && label == "bay-12" && rn == 4 &&
                   std::memcmp(rb, raw, 4) == 0;
    return round_trip(decoded, tag);
}

int main(int argc, char** argv) { return golden_main(argc, argv, check); }
