/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * D6 and G16 through Cyclone's own serialiser.
 *
 * The string half is answered by the generated header and never reaches
 * a serialiser: `idlc` maps `string<4>` to `char s[5]`, so a value the
 * bound cannot hold is not representable in Cyclone's C mapping at all.
 * The probe prints the width rather than asserting it, because that is
 * where the answer is.
 *
 * The sequence half does reach the serialiser: `dds_sequence_long`
 * carries `_maximum`, `_length` and `_buffer` and nothing in the C
 * mapping stops `_length` from exceeding the declared bound.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "dds/dds.h"
#include "dds/cdr/dds_cdrstream.h"
#include "gen.h"

static void *zm(size_t n){return malloc(n);}
static void *zr(void*p,size_t n){return realloc(p,n);}
static void zf(void*p){free(p);}
static const struct dds_cdrstream_allocator A = {.malloc=zm,.realloc=zr,.free=zf};

static void write_sample(const char *label, Sample *s) {
    struct dds_cdrstream_desc desc;
    dds_cdrstream_desc_from_topic_desc(&desc, &Sample_desc);
    dds_ostream_t os;
    dds_ostream_init(&os, &A, 0, DDSI_RTPS_CDR_ENC_VERSION_2);
    bool ok = dds_stream_write_sample(&os, &A, s, &desc);
    printf("cyclone\t%s\t%s\t", label, ok ? "wrote" : "REFUSED");
    if (ok) for (uint32_t i = 0; i < os.m_index; i++) printf("%02x", os.m_buffer[i]);
    printf("\n");
    dds_ostream_fini(&os, &A);
    dds_cdrstream_desc_fini(&desc, &A);
}

int main(void) {
    /* D6: the holder itself is the answer. */
    printf("cyclone\tstring-holder\tsizeof(Sample.s)=%zu\t"
           "idlc maps string<4> to a fixed array of bound+1\n",
           sizeof(((Sample *)0)->s));

    int32_t two[2] = { 7, 9 };
    int32_t three[3] = { 7, 9, 11 };

    Sample in_bound;
    memset(&in_bound, 0, sizeof in_bound);
    strcpy(in_bound.s, "abcd");
    in_bound.v._maximum = 2; in_bound.v._length = 2;
    in_bound.v._buffer = two; in_bound.v._release = false;
    in_bound.tail = 42;
    write_sample("sequence-2-of-2", &in_bound);

    /* G16: three elements in a `sequence<long,2>`. The C mapping has no
       field that would stop it, so whether the bound is enforced is the
       serialiser's answer and not the holder's. */
    Sample over;
    memset(&over, 0, sizeof over);
    strcpy(over.s, "abcd");
    over.v._maximum = 3; over.v._length = 3;
    over.v._buffer = three; over.v._release = false;
    over.tail = 42;
    write_sample("sequence-3-of-2", &over);
    return 0;
}
