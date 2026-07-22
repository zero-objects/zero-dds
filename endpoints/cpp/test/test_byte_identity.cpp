// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Byte-identity test for the C++98 endpoint wire-core (ADR 0013 P3).
// Encodes the fixed SensorReading (same vector as the C/Python tests + Rust
// golden generator) in LE and BE, compares to the goldens, round-trips.
//
// usage: test_byte_identity <golden_le.bin> <golden_be.bin>

#include <cstdio>
#include <cstring>
#include <string>

#include "zerodds_wire.hpp"

static long read_file(const char* path, unsigned char* buf, long cap) {
    std::FILE* f = std::fopen(path, "rb");
    if (f == 0) { std::fprintf(stderr, "cannot open %s\n", path); return -1; }
    long n = (long)std::fread(buf, 1, (size_t)cap, f);
    std::fclose(f);
    return n;
}

static int check(int endian, const char* golden_path, const char* tag) {
    unsigned char out[256];
    unsigned char golden[256];
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
    if (w.error() != ZDW_OK) { std::fprintf(stderr, "%s: encode err\n", tag); return 1; }

    long gn = read_file(golden_path, golden, (long)sizeof(golden));
    if (gn < 0) return 1;
    if ((long)w.size() != gn || std::memcmp(out, golden, (size_t)gn) != 0) {
        std::fprintf(stderr, "%s: byte mismatch (C++=%ld golden=%ld)\n",
                     tag, (long)w.size(), gn);
        return 1;
    }
    std::printf("%s: %ld bytes byte-identical to Rust golden\n", tag, (long)w.size());

    zerodds::Reader r(out, w.size(), endian);
    unsigned long id = r.u32();
    unsigned int kind = r.u16();
    unsigned char flags = r.u8();
    float value = r.f32();
    zdw_u64_t st = r.u64();
    std::string label = r.str();
    unsigned char rb[64]; size_t rn = r.seq_u8(rb, sizeof(rb));
    if (r.error() != ZDW_OK || id != 0xA1B2C3D4uL || kind != 0x1234u ||
        flags != 0x5Au || value != 3.5f || st.hi != stamp.hi || st.lo != stamp.lo ||
        label != "bay-12" || rn != 4 || std::memcmp(rb, raw, 4) != 0) {
        std::fprintf(stderr, "%s: round-trip mismatch\n", tag);
        return 1;
    }
    std::printf("%s: round-trip decode ok\n", tag);
    return 0;
}

int main(int argc, char** argv) {
    if (argc < 3) { std::fprintf(stderr, "usage: %s le be\n", argv[0]); return 2; }
    int rc = 0;
    rc |= check(ZDW_LE, argv[1], "LE");
    rc |= check(ZDW_BE, argv[2], "BE");
    if (rc == 0) std::printf("ALL OK\n");
    return rc;
}
