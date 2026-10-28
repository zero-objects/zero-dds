/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The TAO probe's read direction. The payload comes in on stdin with no
 * encapsulation header and the values go to stdout as canonical JSON.
 *
 * Unlike the write direction this one takes both byte orders: swapping
 * on *read* is unconditional in ACE, so `TAO_InputCDR`'s byte-order
 * argument does what it says.
 */
#include <cstdio>
#include <cstring>
#include <vector>

#include <tao/CDR.h>

#include "genC.h"
#include "corbajson.hpp"

/* The byte order is an argument: it used to be a `sed` substitution and
 * so part of the binary's identity, which made the two orders of one
 * case two runs of `tao_idl` over identical generated code. */
int main(int argc, char **argv) {
    if (argc != 2) {
        std::fprintf(stderr, "usage: probe <le|be>\n");
        return 2;
    }
    const bool zd_big = std::strcmp(argv[1], "be") == 0;

    std::vector<char> raw;
    char chunk[4096];
    std::size_t got;
    while ((got = std::fread(chunk, 1, sizeof chunk, stdin)) > 0) {
        raw.insert(raw.end(), chunk, chunk + got);
    }

    @TYPE@ sample;
    TAO_InputCDR cdr(raw.empty() ? chunk : &raw[0], raw.size(),
                     zd_big ? (int)ACE_CDR::BYTE_ORDER_BIG_ENDIAN
                                  : (int)ACE_CDR::BYTE_ORDER_LITTLE_ENDIAN);
    if (!(cdr >> sample)) {
        std::fprintf(stderr, "tao refused the payload\n");
        return 1;
    }

#include "dump.inc"
    return 0;
}
