#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
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

static size_t read_hex(char *out) {
    size_t n = 0;
    int hi, lo;
    while ((hi = getchar()) != EOF) {
        if (hi == '\n' || hi == ' ' || hi == '\r') continue;
        lo = getchar();
        if (lo == EOF) break;
        char pair[3] = { (char)hi, (char)lo, 0 };
        out[n++] = (char)strtoul(pair, NULL, 16);
    }
    return n;
}

int main(void) {
    struct RTIXCdrTypePluginProgramContext program_context =
        RTIXCdrTypePluginProgramContext_INTIALIZER;
    struct PRESTypePluginDefaultParticipantData participant_data;
    struct PRESTypePluginDefaultEndpointData endpoint_data;
    struct PRESTypePlugin plugin;
    struct RTICdrStream stream;
    RTIEncapsulationId encapsulation;
    size_t length;

    @TYPE@ sample;
    if (!@TYPE@_initialize(&sample)) { printf("REFUSED initialize\n"); return 0; }

    length = read_hex(BUFFER + 4);

    RTIOsapiMemory_zero(&endpoint_data, sizeof(endpoint_data));
    endpoint_data.programContext = program_context;
    endpoint_data._participantData = &participant_data;
    endpoint_data.typePlugin = &plugin;
    endpoint_data.programContext.endpointPluginData = &endpoint_data;
    plugin.typeCode = (struct RTICdrTypeCode *)@TYPE@_get_typecode();
    participant_data.programs = @TYPE@Plugin_get_programs();
    if (participant_data.programs == NULL) { printf("REFUSED no-programs\n"); return 0; }

    encapsulation = DDS_TypeCode_get_native_encapsulation(
        (DDS_TypeCode *)plugin.typeCode, @REPRESENTATION@);
    if (encapsulation == RTI_CDR_ENCAPSULATION_ID_INVALID) { printf("REFUSED no-encapsulation\n"); return 0; }
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
        printf("REFUSED deserialize\n");
        return 0;
    }
#include "dump.inc"
    return 0;
}
