// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Byte-identity test for the C++98 EMHEADER / @mutable path (ADR 0013), via
// the facade over the C wire-core. Same vector as endpoints/golden-gen.
//
// usage: test_mutable <golden_mutable_le.bin> <golden_mutable_be.bin>

#include <cstdio>
#include <cstring>

#include "zerodds_wire.hpp"

static long read_file(const char* path, unsigned char* buf, long cap) {
    std::FILE* f = std::fopen(path, "rb");
    if (f == 0) { std::fprintf(stderr, "cannot open %s\n", path); return -1; }
    long n = (long)std::fread(buf, 1, (size_t)cap, f);
    std::fclose(f);
    return n;
}

static int check(int endian, const char* golden_path, const char* tag) {
    unsigned char out[256], golden[256];
    zerodds::Writer w(out, sizeof(out), endian);
    size_t bs = w.dheader_begin();
    size_t e;
    e = w.emheader_begin(10, false); w.u32(0xDEADBEEFuL);  w.emheader_end(e);
    e = w.emheader_begin(20, false); w.str("mut");          w.emheader_end(e);
    e = w.emheader_begin(30, false); w.u16(0x0777u);        w.emheader_end(e);
    w.dheader_end(bs);
    if (w.error() != ZDW_OK) { std::fprintf(stderr, "%s: encode err\n", tag); return 1; }

    long gn = read_file(golden_path, golden, (long)sizeof(golden));
    if (gn < 0) return 1;
    if ((long)w.size() != gn || std::memcmp(out, golden, (size_t)gn) != 0) {
        std::fprintf(stderr, "%s: byte mismatch (C++=%ld golden=%ld)\n",
                     tag, (long)w.size(), gn);
        return 1;
    }
    std::printf("%s: %ld bytes byte-identical to Rust golden (EMHEADER/@mutable)\n",
                tag, (long)w.size());

    zerodds::Reader r(out, w.size(), endian);
    unsigned long dh = r.dheader();
    unsigned long x = 0, k = 0, ni = 0;
    std::string s;
    for (int i = 0; i < 3; i++) {
        unsigned long id = r.emheader(&ni);
        if (id == 10) x = r.u32();
        else if (id == 20) s = r.str();
        else if (id == 30) k = r.u16();
    }
    if (r.error() != ZDW_OK || dh == 0 || x != 0xDEADBEEFuL || s != "mut" || k != 0x0777u) {
        std::fprintf(stderr, "%s: round-trip mismatch\n", tag);
        return 1;
    }
    std::printf("%s: round-trip decode ok\n", tag);
    return 0;
}

int main(int argc, char** argv) {
    if (argc < 3) { std::fprintf(stderr, "usage: %s le be\n", argv[0]); return 2; }
    int rc = check(ZDW_LE, argv[1], "LE") | check(ZDW_BE, argv[2], "BE");
    if (rc == 0) std::printf("ALL OK\n");
    return rc;
}
