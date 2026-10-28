/* SPDX-License-Identifier: CC0-1.0
 *
 * A foreign verifier for the UADP publish direction.
 *
 * Built against open62541 only — no ZeroDDS code takes part. It connects to
 * open62541's `tutorial_pubsub_subscribe` over opc.tcp and reads the target
 * variables the subscriber maps its received DataSet fields onto
 * (`ns=1;i=50001` Int32, `ns=1;i=50002` Int64, `ns=1;i=50003` Boolean;
 * tutorial_pubsub_subscribe.c:141). Exits 0 only if they hold the values the
 * ZeroDDS publisher sent, which are passed in on the command line.
 *
 * Derived from open62541's own tutorial_client_firststeps.c.
 */

#include <open62541/client_config_default.h>
#include <open62541/client_highlevel.h>
#include <open62541/plugin/log_stdout.h>

#include <stdio.h>
#include <stdlib.h>

static int
readScalar(UA_Client *client, UA_UInt32 numericId, UA_Variant *out) {
    UA_Variant_init(out);
    UA_NodeId nodeId = UA_NODEID_NUMERIC(1, numericId);
    UA_StatusCode rv = UA_Client_readValueAttribute(client, nodeId, out);
    if(rv != UA_STATUSCODE_GOOD) {
        printf("VERIFY-FAIL: read ns=1;i=%u returned %s\n", numericId,
               UA_StatusCode_name(rv));
        return 0;
    }
    return 1;
}

int
main(int argc, char *argv[]) {
    if(argc < 4) {
        printf("usage: %s <endpoint> <int32> <int64>\n", argv[0]);
        return 2;
    }
    const char *endpoint = argv[1];
    UA_Int32 wantInt32 = (UA_Int32)strtol(argv[2], NULL, 10);
    UA_Int64 wantInt64 = (UA_Int64)strtoll(argv[3], NULL, 10);

    UA_Client *client = UA_Client_new();
    UA_ClientConfig_setDefault(UA_Client_getConfig(client));

    UA_StatusCode rv = UA_Client_connect(client, endpoint);
    if(rv != UA_STATUSCODE_GOOD) {
        printf("VERIFY-FAIL: connect to %s returned %s\n", endpoint,
               UA_StatusCode_name(rv));
        UA_Client_delete(client);
        return 1;
    }

    int ok = 1;
    UA_Variant v;

    if(readScalar(client, 50001, &v)) {
        if(UA_Variant_hasScalarType(&v, &UA_TYPES[UA_TYPES_INT32])) {
            UA_Int32 got = *(UA_Int32 *)v.data;
            printf("ns=1;i=50001 Int32 = %d (want %d)\n", got, wantInt32);
            if(got != wantInt32)
                ok = 0;
        } else {
            printf("VERIFY-FAIL: ns=1;i=50001 is not a scalar Int32\n");
            ok = 0;
        }
    } else {
        ok = 0;
    }
    UA_Variant_clear(&v);

    if(readScalar(client, 50002, &v)) {
        if(UA_Variant_hasScalarType(&v, &UA_TYPES[UA_TYPES_INT64])) {
            UA_Int64 got = *(UA_Int64 *)v.data;
            printf("ns=1;i=50002 Int64 = %lld (want %lld)\n", (long long)got,
                   (long long)wantInt64);
            if(got != wantInt64)
                ok = 0;
        } else {
            printf("VERIFY-FAIL: ns=1;i=50002 is not a scalar Int64\n");
            ok = 0;
        }
    } else {
        ok = 0;
    }
    UA_Variant_clear(&v);

    if(readScalar(client, 50003, &v)) {
        if(UA_Variant_hasScalarType(&v, &UA_TYPES[UA_TYPES_BOOLEAN])) {
            UA_Boolean got = *(UA_Boolean *)v.data;
            printf("ns=1;i=50003 Boolean = %s (want true)\n", got ? "true" : "false");
            if(!got)
                ok = 0;
        } else {
            printf("VERIFY-FAIL: ns=1;i=50003 is not a scalar Boolean\n");
            ok = 0;
        }
    } else {
        ok = 0;
    }
    UA_Variant_clear(&v);

    UA_Client_disconnect(client);
    UA_Client_delete(client);

    if(ok) {
        printf("VERIFY-PASS: the foreign subscriber holds what ZeroDDS published\n");
        return 0;
    }
    printf("VERIFY-FAIL\n");
    return 1;
}
