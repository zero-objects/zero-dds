/* SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 ZeroDDS Contributors
 *
 * Fast DDS over RTPS/UDP: a real `DataWriter::write`, so a capture
 * holds what the product sends rather than what `serialize` returns
 * into a buffer the probe owns.
 *
 * `wire-golden.sh` substitutes @TYPE@ and compiles this against
 * `fastddsgen`'s output, once, into a binary that is `probe pub` or
 * `probe sub`.
 *
 * ## `data_sharing().off()` is not tidying
 *
 * With data sharing at its default, two Fast DDS endpoints on one host
 * exchange the payload through mapped memory and nothing reaches a
 * socket. The capture then holds discovery and ACKNACKs and no user
 * `DATA` — a run that looks like it worked and measured nothing. It is
 * off on both endpoints for that reason and no other.
 *
 * The built-in transports are replaced by one UDPv4 descriptor pinned
 * to 127.0.0.1 for the same reason the other runners are: the
 * measurement host has a real interface and the corpus does not belong
 * on it.
 */
#include <chrono>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <string>
#include <thread>

#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/publisher/DataWriter.hpp>
#include <fastdds/dds/publisher/Publisher.hpp>
#include <fastdds/dds/subscriber/DataReader.hpp>
#include <fastdds/dds/subscriber/SampleInfo.hpp>
#include <fastdds/dds/subscriber/Subscriber.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include <fastdds/rtps/transport/UDPv4TransportDescriptor.hpp>

#include "gen.hpp"
#include "genPubSubTypes.hpp"

using namespace eprosima::fastdds::dds;

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
    if (argc != 6) {
        std::cerr << "usage: probe <pub|sub> <domain> <1|2> <count> <topic>\n";
        return 2;
    }
    const std::string role = argv[1];
    const int domain = std::atoi(argv[2]);
    const std::string version = argv[3];
    const int count = std::atoi(argv[4]);
    const std::string topic_name = argv[5];

    DomainParticipantQos pqos;
    pqos.transport().use_builtin_transports = false;
    auto udp = std::make_shared<eprosima::fastdds::rtps::UDPv4TransportDescriptor>();
    udp->interfaceWhiteList.push_back("127.0.0.1");
    pqos.transport().user_transports.push_back(udp);

    DomainParticipant *participant =
        DomainParticipantFactory::get_instance()->create_participant(domain, pqos);
    if (!participant) { std::cerr << "no participant\n"; return 1; }

    TypeSupport type(new @TYPE@PubSubType());
    if (type.register_type(participant) != RETCODE_OK) {
        std::cerr << "fast dds refused the type\n"; return 1;
    }
    Topic *topic = participant->create_topic(topic_name, type.get_type_name(),
                                             TOPIC_QOS_DEFAULT);
    if (!topic) { std::cerr << "no topic\n"; return 1; }

    if (role == "pub") {
        Publisher *publisher = participant->create_publisher(PUBLISHER_QOS_DEFAULT);
        DataWriterQos wqos = DATAWRITER_QOS_DEFAULT;
        wqos.reliability().kind = RELIABLE_RELIABILITY_QOS;
        wqos.history().kind = KEEP_ALL_HISTORY_QOS;
        wqos.data_sharing().off();
        wqos.representation().m_value.clear();
        wqos.representation().m_value.push_back(
            version == "1" ? XCDR_DATA_REPRESENTATION : XCDR2_DATA_REPRESENTATION);
        DataWriter *writer = publisher->create_datawriter(topic, wqos);
        if (!writer) { std::cerr << "no writer\n"; return 1; }

        PublicationMatchedStatus matched;
        for (int i = 0; i < 300; i++) {
            writer->get_publication_matched_status(matched);
            if (matched.current_count > 0) break;
            std::this_thread::sleep_for(std::chrono::milliseconds(50));
        }
        writer->get_publication_matched_status(matched);
        if (matched.current_count == 0) { std::cerr << "no reader matched\n"; return 1; }
        std::this_thread::sleep_for(std::chrono::milliseconds(400));

        for (int i = 0; i < count; i++) {
            @TYPE@ value;
            if (i % 2 == 0) zd_fill_poison(value); else zd_fill_sample(value);
            if (writer->write(&value) != RETCODE_OK) {
                std::cerr << "fast dds refused the sample\n"; return 1;
            }
            std::cout << "wrote " << i << (i % 2 == 0 ? " poison" : " sample") << std::endl;
            std::this_thread::sleep_for(std::chrono::milliseconds(120));
        }
        writer->wait_for_acknowledgments(eprosima::fastdds::dds::Duration_t(10, 0));
        std::cout << "matched " << matched.current_count << std::endl;
        return 0;
    }

    Subscriber *subscriber = participant->create_subscriber(SUBSCRIBER_QOS_DEFAULT);
    DataReaderQos rqos = DATAREADER_QOS_DEFAULT;
    rqos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    rqos.history().kind = KEEP_ALL_HISTORY_QOS;
    rqos.data_sharing().off();
    rqos.representation().m_value.clear();
    rqos.representation().m_value.push_back(XCDR_DATA_REPRESENTATION);
    rqos.representation().m_value.push_back(XCDR2_DATA_REPRESENTATION);
    DataReader *reader = subscriber->create_datareader(topic, rqos);
    if (!reader) { std::cerr << "no reader\n"; return 1; }

    int seen = 0;
    for (int i = 0; i < 600 && seen < count; i++) {
        @TYPE@ value;
        SampleInfo info;
        while (reader->take_next_sample(&value, &info) == RETCODE_OK)
            if (info.valid_data) seen++;
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }
    std::cout << "read " << seen << std::endl;
    return seen == count ? 0 : 1;
}
