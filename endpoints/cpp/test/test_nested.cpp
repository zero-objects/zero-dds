// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Byte-identity test for the C++98 DHEADER path (ADR 0013): @appendable Outer
// with a nested @appendable Inner + sequence<Inner> + string, via the C++
// facade over the C wire-core. Same vector as endpoints/golden-gen.
//
// usage: test_nested <golden_nested_le.bin> <golden_nested_be.bin>


#include "golden.hpp"

static void inner(zerodds::Writer& w, unsigned int a, unsigned long b) {
    size_t bs = w.dheader_begin();
    w.u16(a);
    w.u32(b);
    w.dheader_end(bs);
}

static int check(int endian, const char* golden_path, const char* tag) {
    unsigned char out[256];
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
    if (matches_golden(w, out, golden_path, tag, " (DHEADER/nested/seq)") != 0) return 1;

    zerodds::Reader r(out, w.size(), endian);
    r.dheader();
    unsigned long id = r.u32();
    r.dheader(); unsigned int a1 = r.u16(); unsigned long b1 = r.u32();
    r.dheader();
    unsigned long count = r.u32();
    r.dheader(); unsigned int a2 = r.u16(); unsigned long b2 = r.u32();
    r.dheader(); unsigned int a3 = r.u16(); unsigned long b3 = r.u32();
    std::string label = r.str();
    bool decoded = r.error() == ZDW_OK && id == 0xCAFEBABEuL && a1 == 0x1111u &&
                   b1 == 0x22223333uL && count == 2 && a2 == 0xAAAAu &&
                   b2 == 0xBBBBCCCCuL && a3 == 0xDDDDu && b3 == 0xEEEEFFFFuL &&
                   label == "nested";
    return round_trip(decoded, tag);
}

int main(int argc, char** argv) { return golden_main(argc, argv, check); }
