/* Local installed Cyclone serializer oracle. No networking or vendor sources changed. */
#include <dds/ddsi/ddsi_xt_typelookup.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void dump(const char *label, const void *sample, const struct dds_cdrstream_desc *desc) {
    const struct dds_cdrstream_allocator allocator = { malloc, realloc, free };
    dds_ostreamLE_t stream;
    dds_ostreamLE_init(&stream, &allocator, 0, 2);
    if (!dds_stream_write_sampleLE(&stream, &allocator, sample, desc)) exit(2);
    printf("%s=", label);
    for (uint32_t i = 0; i < stream.x.m_index; i++) printf("%02x", stream.x.m_buffer[i]);
    putchar('\n');
    dds_ostreamLE_fini(&stream, &allocator);
}

int main(void) {
    DDS_Builtin_TypeLookup_Request request = {0};
    memset(&request.header.requestId.writer_guid, 0x5a, 16);
    request.header.requestId.sequence_number.high = 0x01020304;
    request.header.requestId.sequence_number.low = 0x05060708;
    strcpy(request.header.instanceName, "dds.builtin.TOS.000000000000000000000000");
    request.data._d = DDS_Builtin_TypeLookup_getTypes_HashId;
    dump("GET_TYPES_REQUEST", &request, &DDS_Builtin_TypeLookup_Request_cdrstream_desc);
    DDS_Builtin_TypeLookup_Reply reply = {0};
    reply.header.relatedRequestId = request.header.requestId;
    reply.header.remoteEx = DDS_RPC_REMOTE_EX_OK;
    reply.return_data._d = DDS_Builtin_TypeLookup_getTypes_HashId;
    reply.return_data._u.getType._d = DDS_DDS_RETCODE_OK;
    dump("GET_TYPES_REPLY", &reply, &DDS_Builtin_TypeLookup_Reply_cdrstream_desc);
    request.data._d = DDS_Builtin_TypeLookup_getDependencies_HashId;
    dump("GET_DEPENDENCIES_REQUEST", &request, &DDS_Builtin_TypeLookup_Request_cdrstream_desc);
    reply.return_data._d = DDS_Builtin_TypeLookup_getDependencies_HashId;
    dump("GET_DEPENDENCIES_REPLY", &reply, &DDS_Builtin_TypeLookup_Reply_cdrstream_desc);
    return 0;
}
