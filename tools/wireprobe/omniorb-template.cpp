/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The omniORB probe's write direction. `wireprobe-omniorb` substitutes
 * @TYPE@ with the C++ name, then compiles
 * this against `omniidl -bcxx`'s output.
 *
 * The payload goes to stdout with **no encapsulation header**. That is
 * not a simplification: CORBA CDR has no encapsulation of its own for a
 * top-level parameter — GIOP §9.4 carries the byte order in the message
 * header — so `--emit payload` is the only thing an ORB can answer, and
 * `--emit payload-with-encapsulation` is refused by the shell script
 * rather than fabricated here.
 *
 * `cdrMemoryStream::setByteSwapFlag` takes "the data is little-endian",
 * so it is the inverse of the requested order. omniORB is the only one of the
 * three ORBs that can write both orders.
 */
#include <cstdio>
#include <cstring>
#include <type_traits>

/* omniORB has `CORBA::Fixed`; TAO does not. `corbajson.hpp`
 * declares the fixed printer only behind this. */
#define ZDJ_HAS_CORBA_FIXED 1

#include <omniORB4/CORBA.h>

#include "gen.hh"

/* `ORB_init` and not just a `cdrMemoryStream`.
 *
 * omniORB's wide code sets are registered by the ORB, and a bare memory
 * stream has none: `wchar` and `wstring` segfault in `omniCodeSet`
 * without one — measured 2026-08-10, eight corpus cases. After
 * `ORB_init` omniORB chooses its own default, which is UTF-16 with a
 * byte-order mark, and that choice is the ORB's and not this probe's.
 * No option is passed, for exactly that reason. */
/* The byte order is this probe's own first argument and not a `sed`
 * substitution: it used to be part of the binary's identity, so the two
 * orders of one case meant two runs of `omniidl` and two compiles of
 * identical generated code. It is read and removed before `ORB_init`
 * sees the vector, because `ORB_init` consumes the arguments it
 * recognises and would be handed one it does not. */
int main(int argc, char **argv) {
    if (argc < 2) {
        std::fprintf(stderr, "usage: probe <le|be>\n");
        return 2;
    }
    const int zd_big = std::strcmp(argv[1], "be") == 0;
    int orb_argc = argc - 1;
    char **orb_argv = argv + 1;
    orb_argv[0] = argv[0];

    CORBA::ORB_var orb = CORBA::ORB_init(orb_argc, orb_argv);
    @TYPE@ sample;
#include "fill.inc"

    /* The fill marker. omniORB writes nothing at an alignment gap —
     * `cdrStream::alignOutput` moves the pointer and copies no octet —
     * so whatever the buffer holds is what the recording would call
     * omniORB's. `cdrMemoryStream(0, false)` leaves that uninitialised
     * heap, which differs on every run; `cdrMemoryStream(0, true)` would
     * make it `00`, byte-identical to this project's, and that is the
     * option this probe deliberately does not take.
     *
     * Neither states the truth about omniORB's wire. Measured over
     * GIOP/TCP (`tools/wire-capture`), the fill of a request carries the
     * **previous request's** octets from the same body offset — with a
     * long and a short call alternating through one connection, body
     * 25–27 of call n is `03 02 01` from call n-1's `b = 0x01020304` and
     * body 33–39 is `00 00 00 00 00 04 40` from its `d = 2.5`. No buffer
     * content is a function of the sample, so no buffer content records
     * it. `0xAA` marks the octets omniORB does not determine: the
     * recording stays a function of the sample, and every position where
     * it shows through is a position the peer left alone.
     *
     * Written through the stream and rewound rather than memset through
     * `bufPtr()`: `put_octet_array` is the public surface, it grows the
     * buffer the same way marshalling does, and `rewindPtrs` moves the
     * write pointer back to the start without clearing anything — the
     * header says so above it. The pattern therefore covers exactly the
     * capacity the stream has, with no assumption about where `bufPtr()`
     * sits inside the allocation. */
    static const _CORBA_ULong ZD_MARKER_SPAN = 262144;
    static _CORBA_Octet zd_marker[ZD_MARKER_SPAN];
    std::memset(zd_marker, 0xAA, sizeof(zd_marker));

    cdrMemoryStream stream(ZD_MARKER_SPAN, false);
    stream.put_octet_array(zd_marker, (int)ZD_MARKER_SPAN);
    stream.rewindPtrs();
    stream.setByteSwapFlag(zd_big ? 0 : 1);
    sample >>= stream;

    const unsigned char *bytes = (const unsigned char *)stream.bufPtr();
    std::fwrite(bytes, 1, (std::size_t)stream.bufSize(), stdout);
    return 0;
}
