/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * A small OPC-UA client against libopen62541's public C API, driving the
 * three operations `open62541_interop.rs` measures against a running
 * `zerodds-opcua-bridged`: Browse, Read, Write. See README.md in this
 * directory for build prerequisites and exactly what each step does.
 *
 * Every step prints one line to stdout: "<STEP>_OK ..." on success,
 * "<STEP>_FAIL status=0x<hex>" on failure — the harness parses these
 * lines, not open62541's own log output (which goes to stderr).
 *
 * Usage:
 *   open62541_client <endpoint_url> <read_node_identifier> \
 *                     <write_node_identifier> <write_int32_value>
 *
 * <read_node_identifier> / <write_node_identifier> are the STRING part of
 * an ns=2 NodeId (e.g. "Topic:Telemetry/Value/value") — this daemon's own
 * fixed namespace (NS=2, `crate::daemon::server::NS`) and NodeId scheme.
 */

#include <open62541/client.h>
#include <open62541/client_config_default.h>
#include <open62541/client_highlevel.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* This daemon's own fixed namespace index for every gateway-mapped node
 * (`crate::daemon::server::NS`). */
#define ZERODDS_NS 2

static void print_status(const char *step, UA_StatusCode status) {
    printf("%s_FAIL status=0x%08x (%s)\n", step, (unsigned)status,
           UA_StatusCode_name(status));
}

int main(int argc, char **argv) {
    if (argc != 5) {
        fprintf(stderr,
                "usage: %s <endpoint_url> <read_node_identifier> "
                "<write_node_identifier> <write_int32_value>\n",
                argv[0]);
        return 2;
    }
    const char *endpoint = argv[1];
    const char *read_ident = argv[2];
    const char *write_ident = argv[3];
    const long write_value_long = strtol(argv[4], NULL, 10);
    const UA_Int32 write_value = (UA_Int32)write_value_long;

    UA_Client *client = UA_Client_new();
    if (client == NULL) {
        fprintf(stderr, "UA_Client_new failed\n");
        return 1;
    }
    UA_ClientConfig_setDefault(UA_Client_getConfig(client));

    /* Step 1: connect (SecurityMode None — this daemon's own default
     * `opcua.server.security_policies: ["none"]`). */
    UA_StatusCode status = UA_Client_connect(client, endpoint);
    if (status != UA_STATUSCODE_GOOD) {
        print_status("CONNECT", status);
        UA_Client_delete(client);
        return 1;
    }
    printf("CONNECT_OK\n");

    /* Step 2: Browse the well-known Objects folder (Part 5, ns=0;i=85) —
     * every domain/topic this daemon maps lives under it
     * (`crate::address_space_build::OBJECTS_FOLDER`). */
    UA_BrowseRequest bReq;
    UA_BrowseRequest_init(&bReq);
    bReq.requestedMaxReferencesPerNode = 0;
    bReq.nodesToBrowse = UA_BrowseDescription_new();
    bReq.nodesToBrowseSize = 1;
    bReq.nodesToBrowse[0].nodeId = UA_NODEID_NUMERIC(0, 85 /* ObjectsFolder */);
    bReq.nodesToBrowse[0].resultMask = UA_BROWSERESULTMASK_ALL;

    UA_BrowseResponse bResp = UA_Client_Service_browse(client, bReq);
    if (bResp.responseHeader.serviceResult == UA_STATUSCODE_GOOD &&
        bResp.resultsSize > 0 &&
        bResp.results[0].statusCode == UA_STATUSCODE_GOOD) {
        printf("BROWSE_OK count=%lu\n",
               (unsigned long)bResp.results[0].referencesSize);
    } else {
        UA_StatusCode browse_status = bResp.responseHeader.serviceResult;
        if (browse_status == UA_STATUSCODE_GOOD && bResp.resultsSize > 0) {
            browse_status = bResp.results[0].statusCode;
        }
        print_status("BROWSE", browse_status);
    }
    UA_BrowseRequest_clear(&bReq);
    UA_BrowseResponse_clear(&bResp);

    /* Step 3: Read the Value attribute of a dds_to_ua_connection leaf
     * (a Double in this harness's own Telemetry{double value} type). */
    UA_NodeId read_node = UA_NODEID_STRING_ALLOC(ZERODDS_NS, read_ident);
    UA_Variant read_value;
    UA_Variant_init(&read_value);
    status = UA_Client_readValueAttribute(client, read_node, &read_value);
    if (status == UA_STATUSCODE_GOOD &&
        UA_Variant_hasScalarType(&read_value, &UA_TYPES[UA_TYPES_DOUBLE])) {
        printf("READ_OK value=%f\n", *(UA_Double *)read_value.data);
    } else if (status == UA_STATUSCODE_GOOD) {
        fprintf(stderr,
                "READ_OK but the value was not a Double (unexpected "
                "topic type — check the harness's own IDL)\n");
        printf("READ_FAIL status=0x%08x (unexpected type)\n", (unsigned)status);
    } else {
        print_status("READ", status);
    }
    UA_Variant_clear(&read_value);
    UA_NodeId_clear(&read_node);

    /* Step 4: Write an Int32 to a ua_to_dds_connection leaf, then read it
     * back to confirm the AddressSpace actually holds the new value (this
     * daemon's own Write hook / AddressSpace::write_attribute path, not
     * just that the service call returned Good). */
    UA_NodeId write_node = UA_NODEID_STRING_ALLOC(ZERODDS_NS, write_ident);
    UA_Variant write_variant;
    UA_Variant_init(&write_variant);
    UA_Variant_setScalarCopy(&write_variant, &write_value, &UA_TYPES[UA_TYPES_INT32]);
    status = UA_Client_writeValueAttribute(client, write_node, &write_variant);
    UA_Variant_clear(&write_variant);
    if (status != UA_STATUSCODE_GOOD) {
        print_status("WRITE", status);
    } else {
        UA_Variant back;
        UA_Variant_init(&back);
        status = UA_Client_readValueAttribute(client, write_node, &back);
        if (status == UA_STATUSCODE_GOOD &&
            UA_Variant_hasScalarType(&back, &UA_TYPES[UA_TYPES_INT32]) &&
            *(UA_Int32 *)back.data == write_value) {
            printf("WRITE_OK value=%d\n", (int)write_value);
        } else if (status == UA_STATUSCODE_GOOD) {
            fprintf(stderr, "WRITE reported Good but the read-back value differs\n");
            printf("WRITE_FAIL status=0x%08x (read-back mismatch)\n", (unsigned)status);
        } else {
            print_status("WRITE", status);
        }
        UA_Variant_clear(&back);
    }
    UA_NodeId_clear(&write_node);

    UA_Client_disconnect(client);
    UA_Client_delete(client);
    return 0;
}
