// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Byte-identity test for the C++98 DHEADER path (ADR 0013): @appendable Outer
// with a nested @appendable Inner + sequence<Inner> + string, via the C++
// facade over the C wire-core. Same vector as endpoints/golden-gen.
//
// usage: test_nested <golden_nested_le.bin> <golden_nested_be.bin>

#include <cstdio>
#include <cstring>

#include "zerodds_wire.hpp"

static void inner(zerodds::Writer& w, unsigned int a, unsigned long b) {
    size_t bs = w.dheader_begin();
    w.u16(a);
    w.u32(b);
    w.dheader_end(bs);
}

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
    w.u32(0xCAFEBABEuL);
    inner(w, 0x1111u, 0x22223333uL);
    size_t cbs = w.dheader_begin();
    w.u32(2);
    inner(w, 0xAAAAu, 0xBBBBCCCCuL);
    inner(w, 0xDDDDu, 0xEEEEFFFFuL);
    w.dheader_end(cbs);
    w.str("nested");
    w.dheader_end(bs);
    if (w.error() != ZDW_OK) { std::fprintf(stderr, "%s: encode err\n", tag); return 1; }

    long gn = read_file(golden_path, golden, (long)sizeof(golden));
    if (gn < 0) return 1;
    if ((long)w.size() != gn || std::memcmp(out, golden, (size_t)gn) != 0) {
        std::fprintf(stderr, "%s: byte mismatch (C++=%ld golden=%ld)\n",
                     tag, (long)w.size(), gn);
        return 1;
    }
    std::printf("%s: %ld bytes byte-identical to Rust golden (DHEADER/nested/seq)\n",
                tag, (long)w.size());

    zerodds::Reader r(out, w.size(), endian);
    r.dheader();
    unsigned long id = r.u32();
    r.dheader(); unsigned int a1 = r.u16(); unsigned long b1 = r.u32();
    r.dheader();
    unsigned long count = r.u32();
    r.dheader(); unsigned int a2 = r.u16(); unsigned long b2 = r.u32();
    r.dheader(); unsigned int a3 = r.u16(); unsigned long b3 = r.u32();
    std::string label = r.str();
    if (r.error() != ZDW_OK || id != 0xCAFEBABEuL || a1 != 0x1111u ||
        b1 != 0x22223333uL || count != 2 || a2 != 0xAAAAu || b2 != 0xBBBBCCCCuL ||
        a3 != 0xDDDDu || b3 != 0xEEEEFFFFuL || label != "nested") {
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
