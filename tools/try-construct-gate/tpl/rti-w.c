#include <stdio.h>
#include <string.h>
#include "gen.h"
#include "genSupport.h"
#include "genPlugin.h"

static void dump(const char *label, DDS_DataRepresentationId_t rep, const @TYPE@ *sample) {
    char buffer[65536];
    unsigned int length = sizeof(buffer);
    if (!@TYPE@Plugin_serialize_to_cdr_buffer_ex(buffer, &length, sample, rep)) {
        printf("%s REFUSED\n", label);
        return;
    }
    printf("%s ", label);
    for (unsigned int i = 4; i < length; i++)
        printf("%02x", (unsigned char)buffer[i]);
    printf("\n");
}

int main(void) {
    @TYPE@ sample;
    @TYPE@_initialize(&sample);
#include "fill.inc"
    dump("xcdr1", DDS_XCDR_DATA_REPRESENTATION, &sample);
    dump("xcdr2", DDS_XCDR2_DATA_REPRESENTATION, &sample);
    return 0;
}
