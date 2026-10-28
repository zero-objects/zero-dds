/* Writer side: one sample of the WRITER's type, as Cyclone serialises it. */
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

static void dump(const char *label, uint16_t version,
                 const struct dds_cdrstream_desc *desc, const void *sample) {
    dds_ostreamLE_t os;
    dds_ostreamLE_init(&os, &ALLOC, 0, version);
    if (!dds_stream_write_sampleLE(&os, &ALLOC, sample, desc)) {
        printf("%s REFUSED\n", label);
        return;
    }
    printf("%s ", label);
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
