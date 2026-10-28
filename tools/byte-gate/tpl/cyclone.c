/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * One sample through Cyclone DDS's own serialiser, printed as bytes.
 *
 * The gate compares against what a foreign implementation puts on the
 * wire, never against what this project thinks it should be. Nothing
 * here decides a wire question: `dds_stream_write_sample@STREAM@` is
 * Cyclone's, and the only thing this file chooses is which of Cyclone's
 * two entry points to call.
 *
 * `gate.sh` substitutes @TYPE@ with the generated C name and @STREAM@
 * with `LE` or `BE`. Cyclone's little- and big-endian output streams are
 * distinct types, not one type with a flag, so the byte order is a
 * compile-time substitution and not a runtime argument.
 *
 * `dds_stream_write_sample` writes no encapsulation header, so every
 * byte printed here is body.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "dds/dds.h"
#include "dds/cdr/dds_cdrstream.h"
#include "gen.h"

static void *zd_malloc(size_t n) { return malloc(n); }
static void *zd_realloc(void *p, size_t n) { return realloc(p, n); }
static void zd_free(void *p) { free(p); }

static const struct dds_cdrstream_allocator ALLOC = {
    .malloc = zd_malloc, .realloc = zd_realloc, .free = zd_free
};

/* One TSV row: encoding, byte order, status, length, encapsulation
   header, body. The header column is empty because Cyclone writes
   none — an empty cell here means "this vendor wrote no header", which
   is a measurement, and is why the column exists for all four vendors
   rather than only for the two that fill it. */
static void dump(const char *encoding, uint16_t version,
                 const struct dds_cdrstream_desc *desc, const void *sample) {
    dds_ostream@STREAM@_t os;
    dds_ostream@STREAM@_init(&os, &ALLOC, 0, version);
    if (!dds_stream_write_sample@STREAM@(&os, &ALLOC, sample, desc)) {
        printf("%s\t@ENDIAN@\trefused\t\t\tCyclone refused to serialise the sample\n",
               encoding);
        return;
    }
    printf("%s\t@ENDIAN@\twrote\t%u\t\t", encoding, os.x.m_index);
    for (uint32_t i = 0; i < os.x.m_index; i++)
        printf("%02x", os.x.m_buffer[i]);
    printf("\n");
}

int main(void) {
    struct dds_cdrstream_desc desc;
    dds_cdrstream_desc_from_topic_desc(&desc, &@TYPE@_desc);
    @TYPE@ sample;
    memset(&sample, 0, sizeof(sample));
#include "fill.inc"
    dump("xcdr1", DDSI_RTPS_CDR_ENC_VERSION_1, &desc, &sample);
    dump("xcdr2", DDSI_RTPS_CDR_ENC_VERSION_2, &desc, &sample);
    return 0;
}
