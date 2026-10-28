/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The Cyclone probe's body. `wireprobe-cyclone` substitutes @TYPE@ with
 * the flattened C name and compiles this against `idlc`'s output.
 *
 * ## Why the encoding and the byte order are arguments and not
 * ## substitutions
 *
 * They used to be two `sed` markers, decided before
 * the compiler ran, which made the encoding version and the byte order
 * part of the binary's identity: four combinations meant four calls to
 * `idlc`, four compiles and four links of the same type. Over the corpus
 * that was four builds per case per direction where the sample, the
 * type and the generated code were identical in all four — the vendor's
 * code generator ran 2,680 times to answer 2,680 questions that 670
 * runs could answer.
 *
 * Cyclone decides both at run time: `dds_ostream*_init` takes the
 * encoding version as an argument, and the two byte orders are two
 * functions in the same library, callable from one translation unit. So
 * one build now answers every combination, and `--matrix` prints all
 * four rows from one process.
 *
 * The C column's driver made this same move earlier — byte order and
 * emit form from compile time to `argv`, 82,075 slots out of 5,507
 * translation units — and this is the same move for the vendor columns.
 *
 * ## The output
 *
 *   probe le 1            the payload, raw, on stdout
 *   probe --matrix        four `<version> <endian> <hex>` rows
 *
 * The bytes go out with nothing around them: which encoding was used is
 * what the caller asked for, so an encapsulation header here would only
 * repeat it.
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

static void *zd_malloc(size_t n) { return malloc(n); }
static void *zd_realloc(void *p, size_t n) { return realloc(p, n); }
static void zd_free(void *p) { free(p); }

static const struct dds_cdrstream_allocator zd_alloc = {
    .malloc = zd_malloc, .realloc = zd_realloc, .free = zd_free
};

/* One combination, into a freshly initialised stream.
 *
 * A stream per combination and not one reused: `dds_ostream*_init`
 * takes the encoding version, and reusing a stream across versions
 * would carry the first one's alignment state into the second. Returns
 * the length, or -1 when Cyclone refused the sample. */
static long zd_write(int big, uint32_t version, const @TYPE@ *sample,
                     struct dds_cdrstream_desc *desc,
                     unsigned char **bytes) {
    if (big) {
        dds_ostreamBE_t os;
        dds_ostreamBE_init(&os, &zd_alloc, 0, version);
        zd_vendor_call("cyclone", "serialising the sample");
        bool written = dds_stream_write_sampleBE(&os, &zd_alloc, sample, desc);
        zd_vendor_return();
        if (!written) return -1;
        *bytes = os.x.m_buffer;
        return (long)os.x.m_index;
    }
    dds_ostreamLE_t os;
    dds_ostreamLE_init(&os, &zd_alloc, 0, version);
    zd_vendor_call("cyclone", "serialising the sample");
    bool written = dds_stream_write_sampleLE(&os, &zd_alloc, sample, desc);
    zd_vendor_return();
    if (!written) return -1;
    *bytes = os.x.m_buffer;
    return (long)os.x.m_index;
}

int main(int argc, char **argv) {
    struct dds_cdrstream_desc desc;
    dds_cdrstream_desc_from_topic_desc(&desc, &@TYPE@_desc);

    @TYPE@ sample;
    memset(&sample, 0, sizeof(sample));
#include "fill.inc"

    if (argc == 2 && strcmp(argv[1], "--matrix") == 0) {
        /* Every combination from this one build. A refusal is a row and
         * not a silence: a missing line would leave the caller to decide
         * what an absent combination meant, and the two meanings —
         * "Cyclone would not take it" and "nobody asked" — are the two
         * this whole tree exists to keep apart. */
        static const char *const versions[2] = { "xcdr1", "xcdr2" };
        static const uint32_t values[2] = {
            DDSI_RTPS_CDR_ENC_VERSION_1, DDSI_RTPS_CDR_ENC_VERSION_2
        };
        static const char *const orders[2] = { "le", "be" };
        for (int v = 0; v < 2; v++) {
            for (int o = 0; o < 2; o++) {
                unsigned char *bytes = NULL;
                long length = zd_write(o, values[v], &sample, &desc, &bytes);
                if (length < 0) {
                    printf("%s\t%s\t!cyclone refused to serialise the sample\n",
                           versions[v], orders[o]);
                    continue;
                }
                printf("%s\t%s\t", versions[v], orders[o]);
                for (long i = 0; i < length; i++) printf("%02x", bytes[i]);
                printf("\n");
            }
        }
        return 0;
    }

    if (argc != 3) {
        fprintf(stderr, "usage: probe <le|be> <1|2>   or   probe --matrix\n");
        return 2;
    }
    int big = strcmp(argv[1], "be") == 0;
    uint32_t version = strcmp(argv[2], "2") == 0 ? DDSI_RTPS_CDR_ENC_VERSION_2
                                                 : DDSI_RTPS_CDR_ENC_VERSION_1;
    unsigned char *bytes = NULL;
    long length = zd_write(big, version, &sample, &desc, &bytes);
    if (length < 0) {
        fprintf(stderr, "cyclone refused to serialise the sample\n");
        return 1;
    }
    if (fwrite(bytes, 1, (size_t)length, stdout) != (size_t)length) {
        fprintf(stderr, "the bytes could not be written\n");
        return 1;
    }
    return 0;
}
