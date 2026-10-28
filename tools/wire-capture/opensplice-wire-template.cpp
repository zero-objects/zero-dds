/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * OpenSplice Community Edition 6.9 over DDSI2: a real `write` through a
 * generated `<T>DataWriter` (SACPP).
 *
 * `wireprobe-opensplice` — the in-memory probe — calls
 * `DDS::OpenSplice::CdrTypeSupport::serialize` under the configuration
 * without a network. This one runs the single-process deployment with
 * the `ddsi2` service, pinned to 127.0.0.1 by the configuration
 * `wire-golden.sh` writes and names in `OSPL_URI`, so what a capture
 * holds is what the product sends.
 *
 * OpenSplice 6.9 writes one representation, XCDR1, so the version
 * argument is checked rather than mapped: asked for 2, the probe says so
 * and exits instead of writing XCDR1 under that name.
 *
 * The fill statements are the in-memory probe's — `wireprobe-fill
 * --vendor omniorb`, the CORBA C++ mapping SACPP is.
 *
 * @TYPE@ is substituted by `wire-golden.sh`.
 */
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <unistd.h>

#include "ccpp_dds_dcps.h"
#include "ccpp_gen.h"

static void zd_fill_sample(@TYPE@ &out) {
    @TYPE@ sample;
#include "fill-sample.inc"
    out = sample;
}

static void zd_fill_poison(@TYPE@ &out) {
    @TYPE@ sample;
#include "fill-poison.inc"
    out = sample;
}

static void zd_sleep_ms(int ms) { usleep(static_cast<useconds_t>(ms) * 1000); }

int main(int argc, char **argv) {
    if (argc != 6) {
        fprintf(stderr, "usage: probe <pub|sub> <domain> <1|2> <count> <topic>\n");
        return 2;
    }
    const std::string role = argv[1];
    const DDS::DomainId_t domain = static_cast<DDS::DomainId_t>(atoi(argv[2]));
    const std::string version = argv[3];
    const int count = atoi(argv[4]);
    const char *topic_name = argv[5];
    if (version != "1") {
        fprintf(stderr, "OpenSplice 6.9 writes XCDR1 only\n");
        return 2;
    }

    DDS::DomainParticipantFactory_var factory = DDS::DomainParticipantFactory::get_instance();
    DDS::DomainParticipant_var participant = factory->create_participant(
        domain, PARTICIPANT_QOS_DEFAULT, NULL, DDS::STATUS_MASK_NONE);
    if (!participant.in()) {
        fprintf(stderr, "no participant on domain %d (OSPL_URI?)\n", static_cast<int>(domain));
        return 1;
    }
    @TYPE@TypeSupport_var support = new @TYPE@TypeSupport;
    CORBA::String_var type_name = support->get_type_name();
    if (support->register_type(participant.in(), type_name.in()) != DDS::RETCODE_OK) {
        fprintf(stderr, "opensplice refused the type\n");
        return 1;
    }
    DDS::TopicQos tqos;
    participant->get_default_topic_qos(tqos);
    tqos.reliability.kind = DDS::RELIABLE_RELIABILITY_QOS;
    tqos.history.kind = DDS::KEEP_ALL_HISTORY_QOS;
    DDS::Topic_var topic = participant->create_topic(
        topic_name, type_name.in(), tqos, NULL, DDS::STATUS_MASK_NONE);
    if (!topic.in()) {
        fprintf(stderr, "no topic\n");
        return 1;
    }

    int status = 0;
    if (role == "pub") {
        DDS::Publisher_var publisher = participant->create_publisher(
            PUBLISHER_QOS_DEFAULT, NULL, DDS::STATUS_MASK_NONE);
        DDS::DataWriterQos wqos;
        publisher->get_default_datawriter_qos(wqos);
        publisher->copy_from_topic_qos(wqos, tqos);
        DDS::DataWriter_var generic = publisher->create_datawriter(
            topic.in(), wqos, NULL, DDS::STATUS_MASK_NONE);
        if (!generic.in()) {
            fprintf(stderr, "no writer\n");
            return 1;
        }
        @TYPE@DataWriter_var writer = @TYPE@DataWriter::_narrow(generic.in());
        if (!writer.in()) {
            fprintf(stderr, "the writer would not narrow\n");
            return 1;
        }
        /* A write before the reader is matched goes nowhere and the
           capture would be short by exactly that many samples. */
        DDS::PublicationMatchedStatus matched;
        matched.current_count = 0;
        for (int i = 0; i < 1200; i++) {
            if (generic->get_publication_matched_status(matched) == DDS::RETCODE_OK &&
                matched.current_count > 0)
                break;
            zd_sleep_ms(50);
        }
        if (matched.current_count == 0) {
            fprintf(stderr, "no reader matched in 60 s\n");
            return 1;
        }
        zd_sleep_ms(400);
        for (int i = 0; i < count; i++) {
            @TYPE@ value;
            if (i % 2 == 0) zd_fill_poison(value); else zd_fill_sample(value);
            /* `write` returns a code and not a byte count, so a refusal
               is reported by name rather than by an absent sample. */
            DDS::ReturnCode_t rc = writer->write(value, DDS::HANDLE_NIL);
            if (rc != DDS::RETCODE_OK) {
                fprintf(stderr, "opensplice refused the sample: return code %d\n",
                        static_cast<int>(rc));
                return 1;
            }
            printf("wrote %d %s\n", i, i % 2 == 0 ? "poison" : "sample");
            fflush(stdout);
            zd_sleep_ms(120);
        }
        DDS::Duration_t patience = { 10, 0 };
        generic->wait_for_acknowledgments(patience);
        printf("matched %d\n", static_cast<int>(matched.current_count));
        fflush(stdout);
    } else {
        DDS::Subscriber_var subscriber = participant->create_subscriber(
            SUBSCRIBER_QOS_DEFAULT, NULL, DDS::STATUS_MASK_NONE);
        DDS::DataReaderQos rqos;
        subscriber->get_default_datareader_qos(rqos);
        subscriber->copy_from_topic_qos(rqos, tqos);
        DDS::DataReader_var generic = subscriber->create_datareader(
            topic.in(), rqos, NULL, DDS::STATUS_MASK_NONE);
        if (!generic.in()) {
            fprintf(stderr, "no reader\n");
            return 1;
        }
        @TYPE@DataReader_var reader = @TYPE@DataReader::_narrow(generic.in());
        if (!reader.in()) {
            fprintf(stderr, "the reader would not narrow\n");
            return 1;
        }
        int seen = 0;
        for (int i = 0; i < 1800 && seen < count; i++) {
            @TYPE@Seq values;
            DDS::SampleInfoSeq infos;
            DDS::ReturnCode_t rc = reader->take(values, infos, DDS::LENGTH_UNLIMITED,
                                                DDS::ANY_SAMPLE_STATE, DDS::ANY_VIEW_STATE,
                                                DDS::ANY_INSTANCE_STATE);
            if (rc == DDS::RETCODE_OK) {
                for (DDS::ULong k = 0; k < infos.length(); k++)
                    if (infos[k].valid_data) seen++;
                reader->return_loan(values, infos);
            } else {
                zd_sleep_ms(50);
            }
        }
        printf("read %d\n", seen);
        fflush(stdout);
        status = seen == count ? 0 : 1;
    }
    participant->delete_contained_entities();
    factory->delete_participant(participant.in());
    return status;
}
