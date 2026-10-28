/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * RTI Connext writes kh::Pair {a=1, b=2} under XCDR1 and reads every
 * kh::Pair on the topic, printing the instance handle RTI gives each
 * sample. Run beside a ZeroDDS writer of the same key: one handle for both
 * writers is one instance; two handles is RTI separating the key by the
 * KeyHash each writer sent.
 *
 *   pair_live <domain> <seconds> <v-base>
 */
#include <stdio.h>
#include <stdlib.h>

#include "ndds/ndds_c.h"
#include "pair.h"
#include "pairSupport.h"

static void print_handle(const DDS_InstanceHandle_t *h)
{
    int i;
    for (i = 0; i < 16; i++) {
        printf("%02x", h->keyHash.value[i]);
    }
}

int main(int argc, char **argv)
{
    int domain = argc > 1 ? atoi(argv[1]) : 0;
    int seconds = argc > 2 ? atoi(argv[2]) : 10;
    int base = argc > 3 ? atoi(argv[3]) : 1000;
    struct DDS_DataWriterQos wqos = DDS_DataWriterQos_INITIALIZER;
    struct DDS_DataReaderQos rqos = DDS_DataReaderQos_INITIALIZER;
    struct DDS_Duration_t period = {0, 500000000};
    DDS_DomainParticipant *participant;
    DDS_Publisher *publisher;
    DDS_Subscriber *subscriber;
    DDS_Topic *topic;
    DDS_DataWriter *writer;
    DDS_DataReader *reader;
    kh_PairDataWriter *pair_writer;
    kh_PairDataReader *pair_reader;
    kh_Pair *sample;
    int tick;

    participant = DDS_DomainParticipantFactory_create_participant(
        DDS_TheParticipantFactory, domain, &DDS_PARTICIPANT_QOS_DEFAULT, NULL, DDS_STATUS_MASK_NONE);
    if (participant == NULL) { fprintf(stderr, "create_participant\n"); return 1; }
    if (kh_PairTypeSupport_register_type(participant, kh_PairTypeSupport_get_type_name()) != DDS_RETCODE_OK) {
        fprintf(stderr, "register_type\n"); return 1;
    }
    topic = DDS_DomainParticipant_create_topic(participant, "KhPair", kh_PairTypeSupport_get_type_name(),
        &DDS_TOPIC_QOS_DEFAULT, NULL, DDS_STATUS_MASK_NONE);
    publisher = DDS_DomainParticipant_create_publisher(participant, &DDS_PUBLISHER_QOS_DEFAULT, NULL, DDS_STATUS_MASK_NONE);
    subscriber = DDS_DomainParticipant_create_subscriber(participant, &DDS_SUBSCRIBER_QOS_DEFAULT, NULL, DDS_STATUS_MASK_NONE);
    if (topic == NULL || publisher == NULL || subscriber == NULL) { fprintf(stderr, "entities\n"); return 1; }

    DDS_Publisher_get_default_datawriter_qos(publisher, &wqos);
    wqos.reliability.kind = DDS_RELIABLE_RELIABILITY_QOS;
    DDS_DataRepresentationIdSeq_ensure_length(&wqos.representation.value, 1, 1);
    *DDS_DataRepresentationIdSeq_get_reference(&wqos.representation.value, 0) = DDS_XCDR_DATA_REPRESENTATION;
    writer = DDS_Publisher_create_datawriter(publisher, topic, &wqos, NULL, DDS_STATUS_MASK_NONE);

    DDS_Subscriber_get_default_datareader_qos(subscriber, &rqos);
    rqos.reliability.kind = DDS_RELIABLE_RELIABILITY_QOS;
    rqos.history.kind = DDS_KEEP_ALL_HISTORY_QOS;
    DDS_DataRepresentationIdSeq_ensure_length(&rqos.representation.value, 2, 2);
    *DDS_DataRepresentationIdSeq_get_reference(&rqos.representation.value, 0) = DDS_XCDR_DATA_REPRESENTATION;
    *DDS_DataRepresentationIdSeq_get_reference(&rqos.representation.value, 1) = DDS_XCDR2_DATA_REPRESENTATION;
    reader = DDS_Subscriber_create_datareader(subscriber, DDS_Topic_as_topicdescription(topic), &rqos, NULL, DDS_STATUS_MASK_NONE);
    if (writer == NULL || reader == NULL) { fprintf(stderr, "writer/reader\n"); return 1; }
    pair_writer = kh_PairDataWriter_narrow(writer);
    pair_reader = kh_PairDataReader_narrow(reader);

    sample = kh_PairTypeSupport_create_data_ex(DDS_BOOLEAN_TRUE);
    sample->a = 1;
    sample->b = 2;
    for (tick = 0; tick < seconds * 2; tick++) {
        struct kh_PairSeq data = DDS_SEQUENCE_INITIALIZER;
        struct DDS_SampleInfoSeq infos = DDS_SEQUENCE_INITIALIZER;
        int i;
        sample->v = base + tick;
        if (kh_PairDataWriter_write(pair_writer, sample, &DDS_HANDLE_NIL) != DDS_RETCODE_OK) {
            fprintf(stderr, "write\n");
        }
        if (kh_PairDataReader_take(pair_reader, &data, &infos, DDS_LENGTH_UNLIMITED,
                DDS_ANY_SAMPLE_STATE, DDS_ANY_VIEW_STATE, DDS_ANY_INSTANCE_STATE) == DDS_RETCODE_OK) {
            for (i = 0; i < kh_PairSeq_get_length(&data); i++) {
                struct DDS_SampleInfo *info = DDS_SampleInfoSeq_get_reference(&infos, i);
                kh_Pair *got = kh_PairSeq_get_reference(&data, i);
                if (!info->valid_data) continue;
                printf("rti-reader v=%d a=%d b=%lld handle=", got->v, got->a, (long long) got->b);
                print_handle(&info->instance_handle);
                printf("\n");
            }
            kh_PairDataReader_return_loan(pair_reader, &data, &infos);
        }
        fflush(stdout);
        NDDS_Utility_sleep(&period);
    }
    DDS_DomainParticipant_delete_contained_entities(participant);
    DDS_DomainParticipantFactory_delete_participant(DDS_TheParticipantFactory, participant);
    return 0;
}
