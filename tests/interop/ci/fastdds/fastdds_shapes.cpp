// SPDX-License-Identifier: Apache-2.0
// Local QoS test endpoint. Wire support comes from Fast-DDS-Gen, not a private codec.
#include "shapePubSubTypes.hpp"
#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/publisher/Publisher.hpp>
#include <fastdds/dds/publisher/DataWriter.hpp>
#include <fastdds/dds/subscriber/Subscriber.hpp>
#include <fastdds/dds/subscriber/DataReader.hpp>
#include <fastdds/dds/subscriber/SampleInfo.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include <fastdds/rtps/transport/TCPv4TransportDescriptor.hpp>
#include <fastdds/utils/IPLocator.hpp>
#include <chrono>
#include <iostream>
#include <string>
#include <thread>

using namespace eprosima::fastdds::dds;
using eprosima::fastdds::rtps::IPLocator;
using eprosima::fastdds::rtps::Locator_t;
using eprosima::fastdds::rtps::TCPv4TransportDescriptor;

// `tcp <listening port> [<address>:<physical port>[:<logical port>]]...`:
// the participant has Fast DDS's TCPv4 transport alone (no UDP, no shared
// memory), listens at the port (0: none) and has the peers as its initial
// peers. A peer without a logical port is asked at the metatraffic unicast
// ports of the first participant ids, as Fast DDS does.
static bool tcp_only(DomainParticipantQos& qos, int argc, char** argv) {
    auto tcp = std::make_shared<TCPv4TransportDescriptor>();
    try {
        size_t end = 0;
        const int listening = std::stoi(argv[7], &end);
        if (argv[7][end] || listening < 0 || listening > 65535) return false;
        if (listening != 0) tcp->add_listener_port(static_cast<uint16_t>(listening));
        for (int i = 8; i < argc; ++i) {
            const std::string peer = argv[i];
            const auto first = peer.find(':');
            if (first == std::string::npos) return false;
            const auto second = peer.find(':', first + 1);
            Locator_t locator;
            locator.kind = LOCATOR_KIND_TCPv4;
            if (!IPLocator::setIPv4(locator, peer.substr(0, first))) return false;
            const int physical = std::stoi(peer.substr(first + 1, second - first - 1));
            if (physical <= 0 || physical > 65535) return false;
            IPLocator::setPhysicalPort(locator, static_cast<uint16_t>(physical));
            if (second != std::string::npos) {
                const int logical = std::stoi(peer.substr(second + 1));
                if (logical <= 0 || logical > 65535) return false;
                IPLocator::setLogicalPort(locator, static_cast<uint16_t>(logical));
            }
            qos.wire_protocol().builtin.initialPeersList.push_back(locator);
        }
    } catch (const std::exception&) { return false; }
    qos.transport().use_builtin_transports = false;
    qos.transport().user_transports.push_back(tcp);
    return true;
}

int main(int argc, char** argv) {
    const bool tcp = argc >= 8 && std::string(argv[6]) == "tcp";
    if (argc != 6 && !tcp) {
        std::cerr << "usage: fastdds_shapes pub|sub topic domain reliability durability"
                     " [tcp <listening port> <address>:<physical>[:<logical>]...]\n";
        return 2;
    }
    const std::string mode = argv[1], reliability = argv[4], durability = argv[5];
    if ((mode != "pub" && mode != "sub") ||
        (reliability != "besteffort" && reliability != "reliable") ||
        (durability != "volatile" && durability != "transient_local")) return 2;
    int domain;
    try { size_t end = 0; domain = std::stoi(argv[3], &end); if (argv[3][end] || domain < 0 || domain > 232) return 2; }
    catch (const std::exception&) { return 2; }
    auto* factory = DomainParticipantFactory::get_instance();
    DomainParticipantQos participant_qos = PARTICIPANT_QOS_DEFAULT;
    if (tcp && !tcp_only(participant_qos, argc, argv)) return 2;
    auto* participant = factory->create_participant(domain, participant_qos);
    if (!participant) return 2;
    TypeSupport support(new ShapeTypePubSubType());
    if (support.register_type(participant) != RETCODE_OK) return 2;
    auto* topic = participant->create_topic(argv[2], support.get_type_name(), TOPIC_QOS_DEFAULT);
    if (!topic) return 2;
    const auto rel = reliability == "reliable" ? RELIABLE_RELIABILITY_QOS : BEST_EFFORT_RELIABILITY_QOS;
    const auto dur = durability == "transient_local" ? TRANSIENT_LOCAL_DURABILITY_QOS : VOLATILE_DURABILITY_QOS;
    DataWriter* writer = nullptr;
    DataReader* reader = nullptr;
    if (mode == "pub") {
        auto* publisher = participant->create_publisher(PUBLISHER_QOS_DEFAULT);
        if (!publisher) return 2;
        auto qos = DATAWRITER_QOS_DEFAULT;
        qos.reliability().kind = rel; qos.durability().kind = dur;
        qos.history().kind = KEEP_ALL_HISTORY_QOS;
        qos.representation().m_value = {XCDR2_DATA_REPRESENTATION};
        writer = publisher->create_datawriter(topic, qos);
        if (!writer) return 2;
    } else {
        auto* subscriber = participant->create_subscriber(SUBSCRIBER_QOS_DEFAULT);
        if (!subscriber) return 2;
        auto qos = DATAREADER_QOS_DEFAULT;
        qos.reliability().kind = rel; qos.durability().kind = dur;
        qos.history().kind = KEEP_ALL_HISTORY_QOS;
        qos.representation().m_value = {XCDR_DATA_REPRESENTATION, XCDR2_DATA_REPRESENTATION};
        reader = subscriber->create_datareader(topic, qos);
        if (!reader) return 2;
    }
    std::cout << "READY mode=" << mode << " domain=" << domain << " reliability=" << reliability
              << " durability=" << durability << " transport=" << (tcp ? "tcp" : "default") << std::endl;
    const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(60);
    unsigned sent = 0;
    while (std::chrono::steady_clock::now() < deadline) {
        if (writer && sent < 100) {
            PublicationMatchedStatus status;
            if (writer->get_publication_matched_status(status) != RETCODE_OK) return 3;
            if (status.current_count > 0) {
                ShapeType sample; sample.color("RED"); sample.x(50); sample.y(60); sample.shapesize(30);
                if (writer->write(&sample) != RETCODE_OK) return 3;
                ++sent;
            }
        }
        if (reader) {
            ShapeType sample; SampleInfo info;
            ReturnCode_t result;
            while ((result = reader->take_next_sample(&sample, &info)) == RETCODE_OK) {
                if (!info.valid_data) continue;
                if (sample.color() != "RED" || sample.x() < 50 || sample.x() > 149 ||
                    sample.y() < 60 || sample.y() > 159 || sample.shapesize() != 30 ||
                    !sample.additional_payload_size().empty()) return 3;
                std::cout << "SAMPLE color=RED x=" << sample.x() << " y=" << sample.y()
                          << " shapesize=" << sample.shapesize() << std::endl;
            }
            if (result != RETCODE_NO_DATA) return 3;
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }
    participant->delete_contained_entities();
    return factory->delete_participant(participant) == RETCODE_OK ? 0 : 3;
}
