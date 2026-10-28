/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The RTI Connext probe's body. `wireprobe-rti` substitutes @TYPE@ with
 * the flattened C name and compiles this against `rtiddsgen`'s output.
 *
 * The bytes go to stdout with nothing around them.
 *
 * ## Why the representation and the byte order are arguments
 *
 * They used to be two `sed` markers, decided before
 * the compiler ran, which put them into the binary's identity: the four
 * combinations of one case meant four runs of `rtiddsgen` and four
 * compiles of identical generated code. Both reach exactly two places
 * below — the call to `DDS_TypeCode_get_native_encapsulation` and the
 * line that clears the low bit — and neither is a compile-time
 * decision. So one build now answers all four, and `--matrix` prints
 * them from one process.
 *
 * ## Why this is not `<T>Plugin_serialize_to_cdr_buffer_ex`
 *
 * That wrapper is what `/root/byte-gate/gate/tpl/rti.c` calls, and it
 * serialises in the host's byte order and only that:
 * `DDS_TypeCode_get_native_encapsulation` is native by name and by
 * definition. This is the wrapper's own body — `rtiddsgen` writes it
 * into `genPlugin.c` for every type — with one change, on the line that
 * picks the encapsulation.
 *
 * RTI's encapsulation identifiers come in pairs, big endian even and
 * little endian odd: `CDR_BE` 0x0000 / `CDR_LE` 0x0001, `PL_CDR_BE`
 * 0x0002 / `PL_CDR_LE` 0x0003, `CDR2_BE` 0x0006 / `CDR2_LE` 0x0007,
 * `D_CDR2_BE` 0x0008 / `D_CDR2_LE` 0x0009, `PL_CDR2_BE` 0x000a /
 * `PL_CDR2_LE` 0x000b — `ndds/cdr/cdr_encapsulation.h` and
 * `ndds/xcdr/xcdr_stream.h`. The type code still decides which pair the
 * type takes, which is the wire question; clearing the low bit only
 * picks the other member of that pair. Everything after that line is
 * `PRESTypePlugin_interpretedSerialize`, which is RTI's serialiser.
 */
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include "ndds/ndds_c.h"
/* `DDS_TypeCode_get_native_encapsulation` is declared here and nowhere
   in the public umbrella header. */
#include "dds_c/dds_c_typecode_impl.h"
#include "cdr/cdr_encapsulation.h"
#include "cdr/cdr_stream.h"
#include "pres/pres_typePlugin.h"
#include "xcdr/xcdr_interpreter.h"

#include "gen.h"
#include "genSupport.h"
#include "genPlugin.h"

/* The plugin is told the sample's real maximum below and refuses rather
   than overruns, so this only has to be larger than the corpus. */
static char BUFFER[262144];

/* One combination, into `BUFFER`. Returns the payload length, or -1 and
 * a sentence in `refusal`.
 *
 * ## Why the buffer is cleared on every call
 *
 * RTI Connext does not write alignment fill. `RTICdrStream_setZeroOnAlign`
 * in `cdr/cdr_stream.h` is what would make it, and its own comment says
 * "By defalt this is disabled" — a copy of this probe that fills `BUFFER`
 * with `0xAA` reproduces `aa` at every fill position, in both encoding
 * versions and both byte orders. `--matrix` runs four combinations of one
 * case through this one buffer in one process, so without the clear each
 * combination after the first reports the previous one's octets as RTI's:
 * all 50 rows group G held for RTI were `plain-cdr2`, XCDR2 running third
 * and fourth over what XCDR1 left, and none was `plain-cdr1`.
 *
 * Zero and not a marker, because for RTI the marker would say less than
 * the truth. Measured over RTPS/UDP through a real DataWriter — the
 * transport RTI actually speaks — its RTPS layer puts `00` in every fill
 * span, 30 samples XCDR2 and 10 XCDR1, while the same type through
 * `rti::topic::to_cdr_buffer` leaves `0xAA` standing. `tools/wire-capture`
 * is the harness and its README the measurement. So a cleared buffer here
 * records what RTI's wire carries; that is not true of omniORB and TAO,
 * whose GIOP fill is the previous request, and their probes carry the
 * marker instead. */
static long zd_write(int big, DDS_DataRepresentationId_t representation,
                     @TYPE@ *sample, struct PRESTypePluginDefaultEndpointData *endpoint_data,
                     struct PRESTypePlugin *plugin, const char **refusal) {
    struct RTICdrStream stream;
    RTIEncapsulationId encapsulation;
    unsigned int length;

    encapsulation = DDS_TypeCode_get_native_encapsulation(
        (DDS_TypeCode *)plugin->typeCode, representation);
    if (encapsulation == RTI_CDR_ENCAPSULATION_ID_INVALID) {
        *refusal = "rti has no encapsulation for this representation";
        return -1;
    }
    if (big) {
        encapsulation = (RTIEncapsulationId)(encapsulation & (RTIEncapsulationId)~1u);
    }

    endpoint_data->_maxSizeSerializedSample =
        @TYPE@Plugin_get_serialized_sample_max_size(
            (PRESTypePluginEndpointData)endpoint_data, RTI_TRUE, encapsulation, 0);

    memset(BUFFER, 0, sizeof(BUFFER));
    RTICdrStream_init(&stream);
    RTICdrStream_set(&stream, BUFFER, (int)sizeof(BUFFER));
    if (!PRESTypePlugin_interpretedSerialize(
            (PRESTypePluginEndpointData)endpoint_data, sample, &stream,
            RTI_TRUE, encapsulation, RTI_TRUE, NULL)) {
        *refusal = "rti refused to serialise the sample";
        return -1;
    }
    length = (unsigned int)RTICdrStream_getCurrentPositionOffset(&stream);
    if (length < 4) {
        *refusal = "rti wrote no encapsulation header";
        return -1;
    }
    /* The plugin wrote the four-byte encapsulation header. `--emit
       payload` is the payload, so strip exactly what the vendor put in
       front of it and nothing else. */
    return (long)length - 4;
}

int main(int argc, char **argv) {
    struct RTIXCdrTypePluginProgramContext program_context =
        RTIXCdrTypePluginProgramContext_INTIALIZER;
    struct PRESTypePluginDefaultParticipantData participant_data;
    struct PRESTypePluginDefaultEndpointData endpoint_data;
    struct PRESTypePlugin plugin;
    const char *refusal = "";
    long length;

    @TYPE@ sample;
    if (!@TYPE@_initialize(&sample)) {
        fprintf(stderr, "rti could not initialise the sample\n");
        return 1;
    }
#include "fill.inc"

    RTIOsapiMemory_zero(&endpoint_data, sizeof(endpoint_data));
    endpoint_data.programContext = program_context;
    endpoint_data._participantData = &participant_data;
    endpoint_data.typePlugin = &plugin;
    endpoint_data.programContext.endpointPluginData = &endpoint_data;
    plugin.typeCode = (struct RTICdrTypeCode *)@TYPE@_get_typecode();
    participant_data.programs = @TYPE@Plugin_get_programs();
    if (participant_data.programs == NULL) {
        fprintf(stderr, "rti has no interpreter programs for this type\n");
        return 1;
    }

    if (argc == 2 && strcmp(argv[1], "--matrix") == 0) {
        static const char *const names[2] = { "xcdr1", "xcdr2" };
        /* Not `static`: `DDS_XCDR_DATA_REPRESENTATION` is an extern
           constant in RTI's headers rather than a literal, so a
           static initialiser is `initializer element is not constant`
           and the probe does not compile. */
        const DDS_DataRepresentationId_t representations[2] = {
            DDS_XCDR_DATA_REPRESENTATION, DDS_XCDR2_DATA_REPRESENTATION
        };
        static const char *const orders[2] = { "le", "be" };
        int v, o;
        for (v = 0; v < 2; v++) {
            for (o = 0; o < 2; o++) {
                long i;
                length = zd_write(o, representations[v], &sample, &endpoint_data,
                                  &plugin, &refusal);
                if (length < 0) {
                    printf("%s\t%s\t!%s\n", names[v], orders[o], refusal);
                    continue;
                }
                printf("%s\t%s\t", names[v], orders[o]);
                for (i = 0; i < length; i++)
                    printf("%02x", (unsigned char)BUFFER[4 + i]);
                printf("\n");
            }
        }
        return 0;
    }

    if (argc != 3) {
        fprintf(stderr, "usage: probe <le|be> <1|2>   or   probe --matrix\n");
        return 2;
    }
    length = zd_write(strcmp(argv[1], "be") == 0,
                      strcmp(argv[2], "2") == 0 ? DDS_XCDR2_DATA_REPRESENTATION
                                                : DDS_XCDR_DATA_REPRESENTATION,
                      &sample, &endpoint_data, &plugin, &refusal);
    if (length < 0) {
        fprintf(stderr, "%s\n", refusal);
        return 1;
    }
    if (fwrite(BUFFER + 4, 1, (size_t)length, stdout) != (size_t)length) {
        fprintf(stderr, "the bytes could not be written\n");
        return 1;
    }
    return 0;
}
