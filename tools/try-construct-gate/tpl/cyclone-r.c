/* Reader side: the payload on stdin as hex, read back as the READER's
   type. What the reader does with a value its own bound cannot hold is
   the measurement — a value, a refusal, or a crash. */
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
static const struct dds_cdrstream_allocator ALLOC = {
    .malloc = zd_malloc, .realloc = zd_realloc, .free = zd_free
};

static unsigned char BUFFER[65536];

static size_t read_hex(void) {
    size_t n = 0;
    int hi, lo;
    while ((hi = getchar()) != EOF) {
        if (hi == '\n' || hi == ' ' || hi == '\r') continue;
        lo = getchar();
        if (lo == EOF) break;
        char pair[3] = { (char)hi, (char)lo, 0 };
        BUFFER[n++] = (unsigned char)strtoul(pair, NULL, 16);
    }
    return n;
}

int main(void) {
    struct dds_cdrstream_desc desc;
    dds_istream_t is;
    uint32_t actual;
    size_t length = read_hex();

    dds_cdrstream_desc_from_topic_desc(&desc, &@TYPE@_desc);
    actual = (uint32_t)length;
    if (!dds_stream_normalize(BUFFER, (uint32_t)length, false, @VERSION@, &desc, false, &actual)) {
        printf("REFUSED normalize\n");
        return 0;
    }
    @TYPE@ sample;
    memset(&sample, 0, sizeof(sample));
    dds_istream_init(&is, actual, BUFFER, @VERSION@);
    dds_stream_read_sample(&is, &sample, &ALLOC, &desc);
    dds_istream_fini(&is);
#include "dump.inc"
    return 0;
}
