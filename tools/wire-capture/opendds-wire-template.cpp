/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * OpenDDS over RTPS/UDP: a real `write` through a generated
 * `<T>DataWriter`.
 *
 * `wireprobe-opendds` — the in-memory probe — calls `Serializer` over a
 * buffer of its own and never builds a participant. This builds one,
 * with RTPS discovery and the `rtps_udp` transport, both pinned to
 * 127.0.0.1 by the ini file `wire-golden.sh` writes and passes as
 * `-DCPSConfigFile`.
 *
 * The fill statements are the same ones the in-memory probe uses —
 * `wireprobe-fill --vendor omniorb`, the classic IDL-to-C++ mapping,
 * which is what `tao_idl -Sa -St` generates for OpenDDS too.
 *
 * @TYPE@ is substituted by `wire-golden.sh`.
 */
#include <chrono>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <string>
#include <thread>

#include <dds/DCPS/Marked_Default_Qos.h>
#include <dds/DCPS/Service_Participant.h>
#include <dds/DCPS/WaitSet.h>
#include <dds/DCPS/RTPS/RtpsDiscovery.h>
#include <dds/DCPS/transport/framework/TransportRegistry.h>
#include <dds/DdsDcpsInfrastructureC.h>

#include "genTypeSupportImpl.h"

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

int main(int argc, char **argv) {
    DDS::DomainParticipantFactory_var factory =
        TheParticipantFactoryWithArgs(argc, argv);
    if (argc != 6) {
        std::cerr << "usage: probe <pub|sub> <domain> <1|2> <count> <topic>\n";
        return 2;
    }
    const std::string role = argv[1];
    const int domain = std::atoi(argv[2]);
    const std::string version = argv[3];
    const int count = std::atoi(argv[4]);
    const std::string topic_name = argv[5];

    DDS::DomainParticipant_var participant = factory->create_participant(
        domain, PARTICIPANT_QOS_DEFAULT, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    if (!participant) { std::cerr << "no participant\n"; return 1; }

    @TYPE@TypeSupport_var support = new @TYPE@TypeSupportImpl();
    if (support->register_type(participant.in(), "") != DDS::RETCODE_OK) {
        std::cerr << "opendds refused the type\n"; return 1;
    }
    CORBA::String_var type_name = support->get_type_name();
    DDS::Topic_var topic = participant->create_topic(
        topic_name.c_str(), type_name.in(), TOPIC_QOS_DEFAULT, 0,
        OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    if (!topic) { std::cerr << "no topic\n"; return 1; }

    const DDS::DataRepresentationId_t representation =
        version == "1" ? DDS::XCDR_DATA_REPRESENTATION
                       : DDS::XCDR2_DATA_REPRESENTATION;

    if (role == "pub") {
        DDS::Publisher_var publisher = participant->create_publisher(
            PUBLISHER_QOS_DEFAULT, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
        DDS::DataWriterQos wqos;
        publisher->get_default_datawriter_qos(wqos);
        wqos.reliability.kind = DDS::RELIABLE_RELIABILITY_QOS;
        wqos.history.kind = DDS::KEEP_ALL_HISTORY_QOS;
        wqos.representation.value.length(1);
        wqos.representation.value[0] = representation;
        DDS::DataWriter_var generic = publisher->create_datawriter(
            topic.in(), wqos, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
        if (!generic) { std::cerr << "no writer\n"; return 1; }
        @TYPE@DataWriter_var writer = @TYPE@DataWriter::_narrow(generic.in());
        if (!writer) { std::cerr << "the writer would not narrow\n"; return 1; }

        DDS::PublicationMatchedStatus matched;
        // 60 s and not 15: OpenDDS's RTPS discovery over the loopback
        // takes longer than the other three, and nine of the first
        // sweep's runs reported "no reader matched" on cases that pass
        // with more patience. A run that gives up early records a
        // vendor difference that is the harness's clock.
        for (int i = 0; i < 1200; i++) {
            if (generic->get_publication_matched_status(matched) == DDS::RETCODE_OK &&
                matched.current_count > 0)
                break;
            std::this_thread::sleep_for(std::chrono::milliseconds(50));
        }
        if (matched.current_count == 0) { std::cerr << "no reader matched\n"; return 1; }
        std::this_thread::sleep_for(std::chrono::milliseconds(400));

        for (int i = 0; i < count; i++) {
            @TYPE@ value;
            if (i % 2 == 0) zd_fill_poison(value); else zd_fill_sample(value);
            if (writer->write(value, DDS::HANDLE_NIL) != DDS::RETCODE_OK) {
                std::cerr << "opendds refused the sample\n"; return 1;
            }
            std::cout << "wrote " << i << (i % 2 == 0 ? " poison" : " sample") << std::endl;
            std::this_thread::sleep_for(std::chrono::milliseconds(120));
        }
        DDS::Duration_t patience = { 10, 0 };
        generic->wait_for_acknowledgments(patience);
        std::cout << "matched " << matched.current_count << std::endl;
        return 0;
    }

    DDS::Subscriber_var subscriber = participant->create_subscriber(
        SUBSCRIBER_QOS_DEFAULT, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    DDS::DataReaderQos rqos;
    subscriber->get_default_datareader_qos(rqos);
    rqos.reliability.kind = DDS::RELIABLE_RELIABILITY_QOS;
    rqos.history.kind = DDS::KEEP_ALL_HISTORY_QOS;
    rqos.representation.value.length(2);
    rqos.representation.value[0] = DDS::XCDR_DATA_REPRESENTATION;
    rqos.representation.value[1] = DDS::XCDR2_DATA_REPRESENTATION;
    DDS::DataReader_var generic = subscriber->create_datareader(
        topic.in(), rqos, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    if (!generic) { std::cerr << "no reader\n"; return 1; }
    @TYPE@DataReader_var reader = @TYPE@DataReader::_narrow(generic.in());
    if (!reader) { std::cerr << "the reader would not narrow\n"; return 1; }

    int seen = 0;
    for (int i = 0; i < 1800 && seen < count; i++) {
        @TYPE@ value;
        DDS::SampleInfo info;
        while (reader->take_next_sample(value, info) == DDS::RETCODE_OK)
            if (info.valid_data) seen++;
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }
    std::cout << "read " << seen << std::endl;
    return seen == count ? 0 : 1;
}
