/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The omniORB probe's read direction. The payload comes in on stdin
 * with no encapsulation header and the values go to stdout as canonical
 * JSON.
 *
 * A payload omniORB refuses is exit 2 and not exit 1: the stream is
 * wrong, which is a different finding from the probe being unable to
 * run. It is caught rather than left to terminate — a probe that
 * aborted would be recorded as a crash where the truth is that the ORB
 * rejected the sample.
 *
 * The catch is `CORBA::Exception` and not `CORBA::SystemException`,
 * because the narrower one did not hold: `sequence<sequence<long>>`
 * made omniORB throw `CORBA::BAD_PARAM` out of the unmarshal and the
 * probe called `terminate` — recorded as exit 134 over eight cases,
 * which read as a crash and was a refusal. Measured 2026-08-10.
 */
#include <cstdio>
#include <cstring>
#include <vector>

/* omniORB has `CORBA::Fixed`; TAO does not. `corbajson.hpp`
 * declares the fixed printer only behind this. */
#define ZDJ_HAS_CORBA_FIXED 1

#include <omniORB4/CORBA.h>

#include "gen.hh"
#include "corbajson.hpp"

/* `ORB_init` and not just a `cdrMemoryStream`.
 *
 * omniORB's wide code sets are registered by the ORB, and a bare memory
 * stream has none: `wchar` and `wstring` segfault in `omniCodeSet`
 * without one — measured 2026-08-10, eight corpus cases. After
 * `ORB_init` omniORB chooses its own default, which is UTF-16 with a
 * byte-order mark, and that choice is the ORB's and not this probe's.
 * No option is passed, for exactly that reason. */
int main(int argc, char **argv) {
    /* As in the write template: the byte order is this probe's first
     * argument, read and removed before `ORB_init` sees the vector, so
     * one build per case answers both orders. */
    if (argc < 2) {
        std::fprintf(stderr, "usage: probe <le|be>\n");
        return 2;
    }
    const int zd_big = std::strcmp(argv[1], "be") == 0;
    int orb_argc = argc - 1;
    char **orb_argv = argv + 1;
    orb_argv[0] = argv[0];

    CORBA::ORB_var orb = CORBA::ORB_init(orb_argc, orb_argv);
    std::vector<unsigned char> raw;
    unsigned char chunk[4096];
    std::size_t got;
    while ((got = std::fread(chunk, 1, sizeof chunk, stdin)) > 0) {
        raw.insert(raw.end(), chunk, chunk + got);
    }

    @TYPE@ sample;
    try {
        cdrMemoryStream stream(raw.empty() ? (void *)chunk : (void *)&raw[0], raw.size());
        stream.setByteSwapFlag(zd_big ? 0 : 1);
        sample <<= stream;
    } catch (const CORBA::Exception &error) {
        std::fprintf(stderr, "omniorb refused the payload: %s\n", error._name());
        return 1;
    }

#include "dump.inc"
    return 0;
}
