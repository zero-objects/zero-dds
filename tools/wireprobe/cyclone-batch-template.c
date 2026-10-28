/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The whole corpus through Cyclone's serialiser, in one program.
 *
 * `wireprobe-batch cyclone` writes every case's IDL into one `gen.idl`,
 * each in a module of its own, runs `idlc` once over it, and generates
 * one `zd_case_<n>` function per case from this template's `@CASES@`
 * marker. One compile, one link, one process, every row.
 *
 * The alternative was one build per case: 335 runs of `idlc` and 335
 * compiles to answer questions that differ only in which sample is
 * filled in. Nothing about Cyclone required that — `idlc` takes as many
 * types in a file as it is given, and `dds_stream_write_sample` takes
 * the descriptor as an argument.
 *
 * Nothing here decides a wire question. The encoding version and the
 * byte order are arguments to `zd_emit`; which framing that produces is
 * Cyclone's answer, printed.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <stdint.h>
#include "dds/dds.h"
#include "dds/cdr/dds_cdrstream.h"
#include "gen.h"

static void *zd_malloc(size_t n) { return malloc(n); }
static void *zd_realloc(void *p, size_t n) { return realloc(p, n); }
static void zd_free(void *p) { free(p); }

static const struct dds_cdrstream_allocator zd_alloc = {
    .malloc = zd_malloc, .realloc = zd_realloc, .free = zd_free
};

/* One case, one combination. A stream per combination and not one
 * reused: `dds_ostream*_init` takes the encoding version, and reusing a
 * stream across versions would carry the first one's alignment state
 * into the second. */
static void zd_row(const char *name, const char *version, const char *order,
                   int big, uint32_t value,
                   const void *sample, struct dds_cdrstream_desc *desc) {
    unsigned char *bytes = NULL;
    uint32_t length = 0;
    bool ok;
    uint32_t i;

    if (big) {
        dds_ostreamBE_t os;
        dds_ostreamBE_init(&os, &zd_alloc, 0, value);
        ok = dds_stream_write_sampleBE(&os, &zd_alloc, sample, desc);
        bytes = os.x.m_buffer;
        length = os.x.m_index;
    } else {
        dds_ostreamLE_t os;
        dds_ostreamLE_init(&os, &zd_alloc, 0, value);
        ok = dds_stream_write_sampleLE(&os, &zd_alloc, sample, desc);
        bytes = os.x.m_buffer;
        length = os.x.m_index;
    }
    /* A refusal is a row. An absent line would leave the reader to
     * decide what the absence meant, and "Cyclone would not take it"
     * and "nobody asked" are the two meanings this whole tree keeps
     * apart. */
    if (!ok) {
        printf("%s\t%s\t%s\t!cyclone refused to serialise the sample\n",
               name, version, order);
        return;
    }
    printf("%s\t%s\t%s\t", name, version, order);
    for (i = 0; i < length; i++) printf("%02x", bytes[i]);
    printf("\n");
}

/* All four combinations of one case. */
static void zd_emit(const char *name, const void *sample,
                    struct dds_cdrstream_desc *desc) {
    zd_row(name, "xcdr1", "le", 0, DDSI_RTPS_CDR_ENC_VERSION_1, sample, desc);
    zd_row(name, "xcdr1", "be", 1, DDSI_RTPS_CDR_ENC_VERSION_1, sample, desc);
    zd_row(name, "xcdr2", "le", 0, DDSI_RTPS_CDR_ENC_VERSION_2, sample, desc);
    zd_row(name, "xcdr2", "be", 1, DDSI_RTPS_CDR_ENC_VERSION_2, sample, desc);
}

@CASES@

int main(void) {
    /* Line buffered, so a case that kills the process cannot take the
     * rows of the cases before it down with it. The batch driver reads
     * what was printed to decide where the run stopped, and buffered
     * output would make it blame a case that had already answered. */
    setvbuf(stdout, NULL, _IOLBF, 0);
@CALLS@
    return 0;
}
