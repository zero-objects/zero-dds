/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * RTI Connext over RTPS/UDP: a real `DataWriter_write`.
 *
 * ## Why the C API and not the C++11 one
 *
 * `wireprobe-rti` — the in-memory probe whose octets this run is
 * compared against — is built from `rtiddsgen -language C`, and
 * `wireprobe-fill --vendor rti` writes C statements against that
 * generated structure. Using the same generator and the same fill
 * statements is what makes the two levels comparable: any difference
 * that then appears is the transport's and not two different ways of
 * spelling the sample.
 *
 * `wire-golden.sh` substitutes @TYPE@ and compiles this against
 * `rtiddsgen`'s output.
 *
 * The built-in transport mask is UDPv4 alone and the interface list is
 * `127.0.0.1`: shared memory would keep the payload off the socket, and
 * the measurement host has a real interface the corpus does not belong
 * on.
 */
/* `wireprobe-fill --vendor rti` writes `sample.flag = true;` for a
 * boolean member, and this template is C: without `<stdbool.h>` that is
 * `'true' undeclared` and the run records `!refused` — RTI's front end
 * refusing a type its `rtiddsgen` had already accepted. Six rows of the
 * 2026-08-19 sweep were that, `primitive-mixed` among them, and
 * `refusal-check.py` is what found them. */
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "ndds/ndds_c.h"
#include "gen.h"
#include "genSupport.h"

/* `@TYPE@_initialize` is what `rti-template.c` calls before the same
 * fill statements run, so the structure the wire run serialises started
 * life exactly as the in-memory run's did. */
static int zd_fill_sample(@TYPE@ *out) {
    @TYPE@ sample;
    if (!@TYPE@_initialize(&sample)) return 0;
#include "fill-sample.inc"
    memcpy(out, &sample, sizeof(sample));
    return 1;
}

static int zd_fill_poison(@TYPE@ *out) {
    @TYPE@ sample;
    if (!@TYPE@_initialize(&sample)) return 0;
#include "fill-poison.inc"
    memcpy(out, &sample, sizeof(sample));
    return 1;
}

static int zd_representation(struct DDS_DataRepresentationQosPolicy *policy, int version) {
    if (!DDS_DataRepresentationIdSeq_ensure_length(&policy->value, 1, 1)) return 0;
    *DDS_DataRepresentationIdSeq_get_reference(&policy->value, 0) =
        version == 1 ? DDS_XCDR_DATA_REPRESENTATION : DDS_XCDR2_DATA_REPRESENTATION;
    return 1;
}

int main(int argc, char **argv) {
    if (argc != 6) {
        fprintf(stderr, "usage: probe <pub|sub> <domain> <1|2> <count> <topic>\n");
        return 2;
    }
    const char *role = argv[1];
    int domain = atoi(argv[2]);
    int version = atoi(argv[3]);
    int count = atoi(argv[4]);
    const char *topic_name = argv[5];

    DDS_DomainParticipantFactory *factory = DDS_DomainParticipantFactory_get_instance();
    struct DDS_DomainParticipantQos pqos = DDS_DomainParticipantQos_INITIALIZER;
    DDS_DomainParticipantFactory_get_default_participant_qos(factory, &pqos);
    pqos.transport_builtin.mask = DDS_TRANSPORTBUILTIN_UDPv4;
    DDS_PropertyQosPolicyHelper_assert_property(
        &pqos.property, "dds.transport.UDPv4.builtin.parent.allow_interfaces_list",
        "127.0.0.1", DDS_BOOLEAN_FALSE);

    DDS_DomainParticipant *participant = DDS_DomainParticipantFactory_create_participant(
        factory, domain, &pqos, NULL, DDS_STATUS_MASK_NONE);
    if (participant == NULL) { fprintf(stderr, "no participant\n"); return 1; }

    const char *type_name = @TYPE@TypeSupport_get_type_name();
    if (@TYPE@TypeSupport_register_type(participant, type_name) != DDS_RETCODE_OK) {
        fprintf(stderr, "rti refused the type\n"); return 1;
    }
    DDS_Topic *topic = DDS_DomainParticipant_create_topic(
        participant, topic_name, type_name, &DDS_TOPIC_QOS_DEFAULT, NULL,
        DDS_STATUS_MASK_NONE);
    if (topic == NULL) { fprintf(stderr, "no topic\n"); return 1; }

    if (strcmp(role, "pub") == 0) {
        DDS_Publisher *publisher = DDS_DomainParticipant_create_publisher(
            participant, &DDS_PUBLISHER_QOS_DEFAULT, NULL, DDS_STATUS_MASK_NONE);
        struct DDS_DataWriterQos wqos = DDS_DataWriterQos_INITIALIZER;
        DDS_Publisher_get_default_datawriter_qos(publisher, &wqos);
        wqos.reliability.kind = DDS_RELIABLE_RELIABILITY_QOS;
        wqos.history.kind = DDS_KEEP_ALL_HISTORY_QOS;
        wqos.resource_limits.max_samples = 64;
        if (!zd_representation(&wqos.representation, version)) {
            fprintf(stderr, "the representation policy would not take a value\n");
            return 1;
        }
        DDS_DataWriter *generic = DDS_Publisher_create_datawriter(
            publisher, topic, &wqos, NULL, DDS_STATUS_MASK_NONE);
        if (generic == NULL) { fprintf(stderr, "no writer\n"); return 1; }
        @TYPE@DataWriter *writer = @TYPE@DataWriter_narrow(generic);

        struct DDS_PublicationMatchedStatus matched = DDS_PublicationMatchedStatus_INITIALIZER;
        struct DDS_Duration_t step = { 0, 50000000 };
        for (int i = 0; i < 300; i++) {
            DDS_DataWriter_get_publication_matched_status(generic, &matched);
            if (matched.current_count > 0) break;
            NDDS_Utility_sleep(&step);
        }
        if (matched.current_count == 0) { fprintf(stderr, "no reader matched\n"); return 1; }
        struct DDS_Duration_t settle = { 0, 400000000 };
        NDDS_Utility_sleep(&settle);

        for (int i = 0; i < count; i++) {
            @TYPE@ value;
            int filled = (i % 2 == 0) ? zd_fill_poison(&value) : zd_fill_sample(&value);
            if (!filled) { fprintf(stderr, "rti would not initialise the sample\n"); return 1; }
            if (@TYPE@DataWriter_write(writer, &value, &DDS_HANDLE_NIL) != DDS_RETCODE_OK) {
                fprintf(stderr, "rti refused the sample\n"); return 1;
            }
            printf("wrote %d %s\n", i, i % 2 == 0 ? "poison" : "sample");
            fflush(stdout);
            struct DDS_Duration_t gap = { 0, 120000000 };
            NDDS_Utility_sleep(&gap);
        }
        struct DDS_Duration_t patience = { 10, 0 };
        DDS_DataWriter_wait_for_acknowledgments(generic, &patience);
        printf("matched %d\n", matched.current_count);
        return 0;
    }

    DDS_Subscriber *subscriber = DDS_DomainParticipant_create_subscriber(
        participant, &DDS_SUBSCRIBER_QOS_DEFAULT, NULL, DDS_STATUS_MASK_NONE);
    struct DDS_DataReaderQos rqos = DDS_DataReaderQos_INITIALIZER;
    DDS_Subscriber_get_default_datareader_qos(subscriber, &rqos);
    rqos.reliability.kind = DDS_RELIABLE_RELIABILITY_QOS;
    rqos.history.kind = DDS_KEEP_ALL_HISTORY_QOS;
    rqos.resource_limits.max_samples = 64;
    if (!DDS_DataRepresentationIdSeq_ensure_length(&rqos.representation.value, 2, 2)) {
        fprintf(stderr, "the representation policy would not take a value\n"); return 1;
    }
    *DDS_DataRepresentationIdSeq_get_reference(&rqos.representation.value, 0) =
        DDS_XCDR_DATA_REPRESENTATION;
    *DDS_DataRepresentationIdSeq_get_reference(&rqos.representation.value, 1) =
        DDS_XCDR2_DATA_REPRESENTATION;
    DDS_DataReader *generic = DDS_Subscriber_create_datareader(
        subscriber, DDS_Topic_as_topicdescription(topic), &rqos, NULL,
        DDS_STATUS_MASK_NONE);
    if (generic == NULL) { fprintf(stderr, "no reader\n"); return 1; }
    @TYPE@DataReader *reader = @TYPE@DataReader_narrow(generic);

    int seen = 0;
    struct DDS_Duration_t step = { 0, 50000000 };
    for (int i = 0; i < 600 && seen < count; i++) {
        @TYPE@ value;
        struct DDS_SampleInfo info;
        if (!@TYPE@_initialize(&value)) { fprintf(stderr, "no sample\n"); return 1; }
        while (@TYPE@DataReader_take_next_sample(reader, &value, &info) == DDS_RETCODE_OK)
            if (info.valid_data) seen++;
        @TYPE@_finalize(&value);
        NDDS_Utility_sleep(&step);
    }
    printf("read %d\n", seen);
    return seen == count ? 0 : 1;
}
