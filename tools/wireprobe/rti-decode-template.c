/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * The RTI Connext probe's read direction. `wireprobe-rti` substitutes
 * @TYPE@ with the flattened C name, then compiles this
 * against `rtiddsgen`'s output.
 *
 * The payload comes in on stdin with no encapsulation header and the
 * values go to stdout as canonical JSON.
 *
 * ## Where the encapsulation header comes from
 *
 * `PRESTypePlugin_interpretedDeserialize` reads one and takes the byte
 * order from it, so the probe has to put one back in front of the
 * payload that `--emit payload` stripped. It is not written from a table
 * here: `DDS_TypeCode_get_native_encapsulation` is asked, exactly as the
 * write direction asks it, so the identifier is the vendor's own choice
 * for this type under this representation — CDR2 or PL_CDR2 as the
 * type's extensibility dictates. Only the low bit is set or cleared, and
 * that bit is the byte order, which is the one thing the caller chose.
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
#include "dumpjson.h"

#define ZD_CAPACITY 262144
static char BUFFER[ZD_CAPACITY];

/* The representation and the byte order come in on the command line, so
 * one build per case answers all four combinations rather than
 * `rtiddsgen` running four times over identical generated code. */
int main(int argc, char **argv) {
    int zd_big;
    DDS_DataRepresentationId_t zd_representation;

    if (argc != 3) {
        fprintf(stderr, "usage: probe <le|be> <1|2>\n");
        return 2;
    }
    zd_big = strcmp(argv[1], "be") == 0;
    zd_representation = strcmp(argv[2], "2") == 0 ? DDS_XCDR2_DATA_REPRESENTATION
                                                  : DDS_XCDR_DATA_REPRESENTATION;

    struct RTIXCdrTypePluginProgramContext program_context =
        RTIXCdrTypePluginProgramContext_INTIALIZER;
    struct PRESTypePluginDefaultParticipantData participant_data;
    struct PRESTypePluginDefaultEndpointData endpoint_data;
    struct PRESTypePlugin plugin;
    struct RTICdrStream stream;
    RTIEncapsulationId encapsulation;
    size_t length;

    @TYPE@ sample;
    if (!@TYPE@_initialize(&sample)) {
        fprintf(stderr, "rti could not initialise the sample\n");
        return 1;
    }

    length = fread(BUFFER + 4, 1, sizeof(BUFFER) - 4, stdin);
    if (!feof(stdin)) {
        fprintf(stderr, "the payload is larger than the probe's buffer\n");
        return 1;
    }

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

    encapsulation = DDS_TypeCode_get_native_encapsulation(
        (DDS_TypeCode *)plugin.typeCode, zd_representation);
    if (encapsulation == RTI_CDR_ENCAPSULATION_ID_INVALID) {
        fprintf(stderr, "rti has no encapsulation for this representation\n");
        return 1;
    }
    if (zd_big) {
        encapsulation = (RTIEncapsulationId)(encapsulation & (RTIEncapsulationId)~1u);
    }
    /* The identifier is big-endian on the wire whatever the payload
       behind it is: it is what tells a reader which order to switch to.
       DDSI-RTPS 2.5 §10.5. */
    BUFFER[0] = (char)(((unsigned)encapsulation >> 8) & 0xFFu);
    BUFFER[1] = (char)((unsigned)encapsulation & 0xFFu);
    BUFFER[2] = 0;
    BUFFER[3] = 0;

    endpoint_data._maxSizeSerializedSample =
        @TYPE@Plugin_get_serialized_sample_max_size(
            (PRESTypePluginEndpointData)&endpoint_data, RTI_TRUE, encapsulation, 0);

    RTICdrStream_init(&stream);
    RTICdrStream_set(&stream, BUFFER, (int)(length + 4));
    if (!PRESTypePlugin_interpretedDeserialize(
            (PRESTypePluginEndpointData)&endpoint_data, &sample, &stream,
            RTI_TRUE, RTI_TRUE, NULL)) {
        fprintf(stderr, "rti refused to deserialise the payload\n");
        return 1;
    }

#include "dump.inc"
    return 0;
}
