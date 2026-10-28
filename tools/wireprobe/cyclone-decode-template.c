/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The Cyclone probe's read direction. `wireprobe-cyclone` substitutes
 * @TYPE@ with the flattened C name and compiles this against `idlc`'s
 * output.
 *
 * The byte order and the encoding version come in on the command line
 * and not from `sed`, for the reason the write template gives: they used
 * to be part of the binary's identity, so four combinations of one case
 * meant four runs of `idlc` and four compiles of identical generated
 * code. Cyclone takes both at run time — `dds_stream_normalize` and
 * `dds_istream_init` are arguments, not build settings — so one build
 * per case answers all four.
 *
 * The payload comes in on stdin with no encapsulation header — the same
 * bytes `--emit payload` writes out — and the values go to stdout as the
 * canonical JSON of `crates/wire-probe/src/canonical.rs`.
 *
 * ## Why `dds_stream_normalize` comes first
 *
 * `dds_istream_t` carries no byte order: Cyclone byte-swaps a foreign
 * stream in place on receipt and reads natively afterwards. That is what
 * `dds_stream_normalize` is, and it also validates the framing, so a
 * payload it rejects is reported rather than read as whatever the bytes
 * happen to say. Skipping it and reading a big-endian payload directly
 * would produce a sample of plausible-looking wrong values.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <stdint.h>
#include "dds/dds.h"
#include "dds/cdr/dds_cdrstream.h"
#include "gen.h"
#include "vendor_crash.h"
#include "dumpjson.h"

static void *zd_malloc(size_t n) { return malloc(n); }
static void *zd_realloc(void *p, size_t n) { return realloc(p, n); }
static void zd_free(void *p) { free(p); }

/* Larger than the corpus; a payload that filled it would be truncated,
   and a truncated payload has to be refused rather than decoded. */
#define ZD_CAPACITY 262144
static unsigned char BUFFER[ZD_CAPACITY];

static int host_is_big_endian(void) {
    const uint32_t probe = 1;
    return ((const unsigned char *)&probe)[0] == 0;
}

int main(int argc, char **argv) {
    const struct dds_cdrstream_allocator alloc = {
        .malloc = zd_malloc, .realloc = zd_realloc, .free = zd_free
    };
    struct dds_cdrstream_desc desc;
    dds_istream_t is;
    uint32_t actual;
    size_t length;
    bool bswap;
    int big;
    uint32_t version;

    if (argc != 3) {
        fprintf(stderr, "usage: probe <le|be> <1|2>\n");
        return 2;
    }
    big = strcmp(argv[1], "be") == 0;
    version = strcmp(argv[2], "2") == 0 ? DDSI_RTPS_CDR_ENC_VERSION_2
                                        : DDSI_RTPS_CDR_ENC_VERSION_1;

    length = fread(BUFFER, 1, sizeof(BUFFER), stdin);
    if (!feof(stdin)) {
        fprintf(stderr, "the payload is larger than the probe's buffer\n");
        return 1;
    }

    dds_cdrstream_desc_from_topic_desc(&desc, &@TYPE@_desc);

    bswap = (big != 0) != (host_is_big_endian() != 0);
    actual = (uint32_t)length;
    zd_vendor_call("cyclone", "reading the payload");
    bool valid = dds_stream_normalize(BUFFER, (uint32_t)length, bswap, version, &desc, false, &actual);
    zd_vendor_return();
    if (!valid) {
        fprintf(stderr, "cyclone refused the payload: it is not a valid sample of this type\n");
        return 1;
    }

    @TYPE@ sample;
    memset(&sample, 0, sizeof(sample));
    dds_istream_init(&is, actual, BUFFER, version);
    zd_vendor_call("cyclone", "reading the payload");
    dds_stream_read_sample(&is, &sample, &alloc, &desc);
    zd_vendor_return();
    dds_istream_fini(&is);

#include "dump.inc"
    return 0;
}
