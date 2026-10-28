/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The same sample through RTI Connext's own serialiser, in the host's
 * byte order.
 *
 * `<T>Plugin_serialize_to_cdr_buffer_ex` writes the four-byte
 * encapsulation header itself; the body is what the gate compares, and
 * the header goes in its own column.
 *
 * This entry point is native-endian by definition —
 * `DDS_TypeCode_get_native_encapsulation` — so it can only produce the
 * little-endian rows on an x86-64 host. The big-endian rows come from
 * `rti-be.c`, which is the same wrapper's own body with one line
 * changed.
 */
#include <stdio.h>
#include <string.h>
#include "gen.h"
#include "genSupport.h"
#include "genPlugin.h"

static void dump(const char *encoding, DDS_DataRepresentationId_t rep,
                 const @TYPE@ *sample) {
    char buffer[65536];
    unsigned int length = sizeof(buffer);
    if (!@TYPE@Plugin_serialize_to_cdr_buffer_ex(buffer, &length, sample, rep)) {
        printf("%s\tle\trefused\t\t\tRTI refused to serialise the sample\n", encoding);
        return;
    }
    printf("%s\tle\twrote\t%u\t%02x%02x\t", encoding, length - 4,
           (unsigned char)buffer[0], (unsigned char)buffer[1]);
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
