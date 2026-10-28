/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * D6 and G16 through RTI Connext's own serialiser.
 *
 * RTI is the DDS front end whose C mapping expresses both overruns:
 * `string<4>` is a bare `DDS_Char *` and `sequence<long,2>` is a
 * `DDS_LongSeq` whose maximum is a run-time field. So unlike Cyclone and
 * Fast DDS, whose mappings answer the question before a serialiser is
 * reached, RTI's serialiser is the thing that answers it.
 */
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#include "ndds/ndds_c.h"
#include "dds_c/dds_c_typecode_impl.h"
#include "cdr/cdr_encapsulation.h"
#include "cdr/cdr_stream.h"
#include "pres/pres_typePlugin.h"
#include "xcdr/xcdr_interpreter.h"

#include "gen.h"
#include "genSupport.h"
#include "genPlugin.h"

static char BUFFER[65536];

static void write_sample(const char *label, Sample *sample) {
    struct RTIXCdrTypePluginProgramContext program_context =
        RTIXCdrTypePluginProgramContext_INTIALIZER;
    struct PRESTypePluginDefaultParticipantData participant_data;
    struct PRESTypePluginDefaultEndpointData endpoint_data;
    struct PRESTypePlugin plugin;
    struct RTICdrStream stream;
    RTIEncapsulationId encapsulation;

    RTIOsapiMemory_zero(&endpoint_data, sizeof(endpoint_data));
    endpoint_data.programContext = program_context;
    endpoint_data._participantData = &participant_data;
    endpoint_data.typePlugin = &plugin;
    endpoint_data.programContext.endpointPluginData = &endpoint_data;
    plugin.typeCode = (struct RTICdrTypeCode *)Sample_get_typecode();
    participant_data.programs = SamplePlugin_get_programs();

    encapsulation = DDS_TypeCode_get_native_encapsulation(
        (DDS_TypeCode *)plugin.typeCode, DDS_XCDR2_DATA_REPRESENTATION);
    endpoint_data._maxSizeSerializedSample =
        SamplePlugin_get_serialized_sample_max_size(
            (PRESTypePluginEndpointData)&endpoint_data, RTI_TRUE, encapsulation, 0);

    memset(BUFFER, 0, sizeof(BUFFER));
    RTICdrStream_init(&stream);
    RTICdrStream_set(&stream, BUFFER, (int)sizeof(BUFFER));
    if (!PRESTypePlugin_interpretedSerialize(
            (PRESTypePluginEndpointData)&endpoint_data, sample, &stream,
            RTI_TRUE, encapsulation, RTI_TRUE, NULL)) {
        printf("rti\t%s\tREFUSED\t\n", label);
        return;
    }
    printf("rti\t%s\twrote\t", label);
    /* Four octets of encapsulation header, then the body. */
    int length = RTICdrStream_getCurrentPositionOffset(&stream);
    for (int i = 4; i < length; i++)
        printf("%02x", (unsigned char)BUFFER[i]);
    printf("\n");
}

int main(void) {
    Sample sample;
    if (!Sample_initialize(&sample)) { printf("rti\t!could not initialise\n"); return 1; }

    /* D6. RTI's holder is a pointer, so the eight-character value is
       representable and the serialiser decides. */
    DDS_Long two[2] = { 7, 9 };
    sample.s = (DDS_Char *)"abcd";
    DDS_LongSeq_set_maximum(&sample.v, 2);
    DDS_LongSeq_set_length(&sample.v, 2);
    *DDS_LongSeq_get_reference(&sample.v, 0) = two[0];
    *DDS_LongSeq_get_reference(&sample.v, 1) = two[1];
    sample.tail = 42;
    write_sample("in-bound", &sample);

    sample.s = (DDS_Char *)"abcdefgh";
    write_sample("string-8-of-4", &sample);

    /* G16. `set_length` past the maximum is RTI's own API answering
       first; the return value is the measurement either way. */
    sample.s = (DDS_Char *)"abcd";
    printf("rti\tset_length(3)\t%s\t\n",
           DDS_LongSeq_set_length(&sample.v, 3) ? "accepted" : "REFUSED");
    if (DDS_LongSeq_set_maximum(&sample.v, 3) && DDS_LongSeq_set_length(&sample.v, 3)) {
        *DDS_LongSeq_get_reference(&sample.v, 2) = 11;
        write_sample("sequence-3-of-2", &sample);
    }
    return 0;
}
