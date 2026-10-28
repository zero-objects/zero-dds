#include <iostream>
#include <string>
#include <thread>
#include <chrono>
#include <cstdlib>
#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/publisher/Publisher.hpp>
#include <fastdds/dds/publisher/DataWriter.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include <fastdds/rtps/transport/UDPv4TransportDescriptor.hpp>
#include "gapprobePubSubTypes.hpp"

using namespace eprosima::fastdds::dds;

int main(int argc, char **argv)
{
    int domain_id = (argc > 1) ? std::atoi(argv[1]) : 0;
    std::string rep = (argc > 2) ? argv[2] : "2";

    DomainParticipantQos pqos;
    pqos.transport().use_builtin_transports = false;
    auto udp = std::make_shared<eprosima::fastdds::rtps::UDPv4TransportDescriptor>();
    udp->interfaceWhiteList.push_back("127.0.0.1");
    pqos.transport().user_transports.push_back(udp);

    DomainParticipant *participant =
        DomainParticipantFactory::get_instance()->create_participant(domain_id, pqos);
    if (!participant) { std::cerr << "no participant" << std::endl; return 1; }

    TypeSupport type(new GapProbePubSubType());
    type.register_type(participant);
    Topic *topic = participant->create_topic("GapProbeTopic", type.get_type_name(), TOPIC_QOS_DEFAULT);
    Publisher *pub = participant->create_publisher(PUBLISHER_QOS_DEFAULT);

    DataWriterQos wqos = DATAWRITER_QOS_DEFAULT;
    wqos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    wqos.history().kind = KEEP_LAST_HISTORY_QOS;
    wqos.history().depth = 1;
    wqos.data_sharing().off();
    wqos.representation().m_value.clear();
    wqos.representation().m_value.push_back(rep == "1" ? XCDR_DATA_REPRESENTATION
                                                       : XCDR2_DATA_REPRESENTATION);
    DataWriter *writer = pub->create_datawriter(topic, wqos);
    if (!writer) { std::cerr << "no writer" << std::endl; return 1; }

    PublicationMatchedStatus ms;
    for (int i = 0; i < 200; i++) {
        writer->get_publication_matched_status(ms);
        if (ms.current_count > 0) break;
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    }
    writer->get_publication_matched_status(ms);
    if (ms.current_count == 0) { std::cerr << "no reader matched" << std::endl; return 1; }
    std::this_thread::sleep_for(std::chrono::milliseconds(500));

    GapProbe a; a.s(std::string(60, 0x41)); a.b(0x11223344); a.c(0x55);
    a.d(1.5); a.e(0x66); a.f(0x7788);
    GapProbe b; b.s(std::string(4, 0x42)); b.b(0x01020304); b.c(0x05);
    b.d(2.5); b.e(0x06); b.f(0x0708);

    for (int i = 0; i < 15; i++) {
        writer->write(&a);
        std::this_thread::sleep_for(std::chrono::milliseconds(200));
        writer->write(&b);
        std::this_thread::sleep_for(std::chrono::milliseconds(200));
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(600));
    std::cout << "wrote 30 alternating samples, xcdr" << rep << std::endl;
    return 0;
}
