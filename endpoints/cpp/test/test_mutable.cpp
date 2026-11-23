// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Byte-identity test for the C++98 EMHEADER / @mutable path (ADR 0013), via
// the facade over the C wire-core. Same vector as endpoints/golden-gen.
//
// usage: test_mutable <golden_mutable_le.bin> <golden_mutable_be.bin>


#include "golden.hpp"

static int check(int endian, const char* golden_path, const char* tag) {
    unsigned char out[256];
    zerodds::Writer w(out, sizeof(out), endian);
    size_t bs = w.dheader_begin();
    size_t e;
    e = w.emheader_begin(10, false); w.u32(0xDEADBEEFuL);  w.emheader_end(e);
    e = w.emheader_begin(20, false); w.str("mut");          w.emheader_end(e);
    e = w.emheader_begin(30, false); w.u16(0x0777u);        w.emheader_end(e);
    w.dheader_end(bs);
    if (matches_golden(w, out, golden_path, tag, " (EMHEADER/@mutable)") != 0) return 1;

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
    bool decoded = r.error() == ZDW_OK && dh != 0 && x == 0xDEADBEEFuL &&
                   s == "mut" && k == 0x0777u;
    return round_trip(decoded, tag);
}

int main(int argc, char** argv) { return golden_main(argc, argv, check); }
