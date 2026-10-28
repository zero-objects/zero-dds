/* Feed each vendor a boolean octet that is neither 0 nor 1 and see what
   comes back: narrowed to true, kept as 2, or refused. C2. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "dds/dds.h"
#include "dds/cdr/dds_cdrstream.h"
#include "gen.h"

static void *zm(size_t n){return malloc(n);} static void *zr(void*p,size_t n){return realloc(p,n);} static void zf(void*p){free(p);}
static const struct dds_cdrstream_allocator A = {.malloc=zm,.realloc=zr,.free=zf};

static void try_bool(const char *label, unsigned char raw) {
    struct dds_cdrstream_desc desc;
    dds_cdrstream_desc_from_topic_desc(&desc, &B_desc);
    unsigned char buf[8] = { raw, 0, 0, 0, 0x07, 0, 0, 0 };
    dds_istream_t is; dds_istream_init(&is, sizeof buf, buf, DDSI_RTPS_CDR_ENC_VERSION_2);
    B out; memset(&out, 0, sizeof out);
    dds_stream_read_sample(&is, &out, &A, &desc);
    bool ok = true;
    printf("cyclone boolean=0x%02x  read=%s  flag=%u (as int) n=%d\n",
           raw, ok ? "accepted" : "REFUSED", (unsigned)out.flag, out.n);
    printf("           normalize=%s\n", label);
    dds_cdrstream_desc_fini(&desc, &A);
}

static void try_normalize(unsigned char raw) {
    struct dds_cdrstream_desc desc;
    dds_cdrstream_desc_from_topic_desc(&desc, &B_desc);
    unsigned char buf[8] = { raw, 0, 0, 0, 0x07, 0, 0, 0 };
    uint32_t size = sizeof buf;
    bool ok = dds_stream_normalize(buf, size, false, DDSI_RTPS_CDR_ENC_VERSION_2, &desc, false, &size);
    printf("cyclone boolean=0x%02x  dds_stream_normalize=%s\n", raw, ok ? "accepted" : "REFUSED");
    dds_cdrstream_desc_fini(&desc, &A);
}

int main(void) {
    for (unsigned r = 0; r < 3; r++) { try_normalize((unsigned char)r); try_bool("see above", (unsigned char)r); }
    try_normalize(0xff); try_bool("see above", 0xff);
    printf("sizeof(B.flag) = %zu, sizeof(C.letter) = %zu\n", sizeof(((B*)0)->flag), sizeof(((C*)0)->letter));
    return 0;
}
