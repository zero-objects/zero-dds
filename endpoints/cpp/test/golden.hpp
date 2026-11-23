// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The frame of the byte-identity tests of the C++98 facade: the octets a
// test encoded against the golden file endpoints/golden-gen wrote, the
// report of the round trip, and a main that runs the test for the
// little-endian and the big-endian golden file.

#ifndef ZERODDS_TEST_GOLDEN_HPP
#define ZERODDS_TEST_GOLDEN_HPP

#include <cstdio>
#include <cstring>

#include "zerodds_wire.hpp"

// 0 when `w` encoded without error and its octets at `out` are the golden
// file `golden_path`; `what` follows "byte-identical to Rust golden" in
// the report.
inline int matches_golden(const zerodds::Writer& w, const unsigned char* out,
                          const char* golden_path, const char* tag, const char* what) {
    if (w.error() != ZDW_OK) { std::fprintf(stderr, "%s: encode err\n", tag); return 1; }
    unsigned char golden[256];
    std::FILE* f = std::fopen(golden_path, "rb");
    if (f == 0) { std::fprintf(stderr, "cannot open %s\n", golden_path); return 1; }
    long gn = (long)std::fread(golden, 1, sizeof(golden), f);
    std::fclose(f);
    if ((long)w.size() != gn || std::memcmp(out, golden, (size_t)gn) != 0) {
        std::fprintf(stderr, "%s: byte mismatch (C++=%ld golden=%ld)\n",
                     tag, (long)w.size(), gn);
        return 1;
    }
    std::printf("%s: %ld bytes byte-identical to Rust golden%s\n", tag, (long)w.size(), what);
    return 0;
}

// 0 when the decoded values are the encoded ones (`decoded`).
inline int round_trip(bool decoded, const char* tag) {
    if (!decoded) { std::fprintf(stderr, "%s: round-trip mismatch\n", tag); return 1; }
    std::printf("%s: round-trip decode ok\n", tag);
    return 0;
}

// The main of a test: `check` against the LE and then the BE golden file
// named on the command line.
inline int golden_main(int argc, char** argv, int (*check)(int, const char*, const char*)) {
    if (argc < 3) { std::fprintf(stderr, "usage: %s le be\n", argv[0]); return 2; }
    int rc = check(ZDW_LE, argv[1], "LE") | check(ZDW_BE, argv[2], "BE");
    if (rc == 0) std::printf("ALL OK\n");
    return rc;
}

#endif
