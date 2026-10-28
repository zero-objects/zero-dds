/* fastdds-wire.cpp — publisher and subscriber for one of the probe types,
 * over UDPv4 on 127.0.0.1 only (shared memory disabled) so tcpdump sees the
 * DATA submessages.
 *
 *   ./wire pub|sub Foo|NoKeyFinal|NoKeyMutable
 */
#include <chrono>
#include <cstring>
#include <iostream>
#include <thread>

#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/publisher/DataWriter.hpp>
#include <fastdds/dds/publisher/Publisher.hpp>
#include <fastdds/dds/subscriber/DataReader.hpp>
#include <fastdds/dds/subscriber/Subscriber.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include <fastdds/rtps/transport/UDPv4TransportDescriptor.hpp>

#include "keyhashPubSubTypes.hpp"

using namespace eprosima::fastdds::dds;

static void udp_only(DomainParticipantQos& qos)
{
    qos.transport().use_builtin_transports = false;
    auto udp = std::make_shared<eprosima::fastdds::rtps::UDPv4TransportDescriptor>();
    udp->interfaceWhiteList.push_back("127.0.0.1");
    qos.transport().user_transports.push_back(udp);
}

int main(int argc, char** argv)
{
    if (argc != 3) { std::cerr << "usage: wire pub|sub <type>\n"; return 2; }
    const char* role = argv[1];
    const char* tname = argv[2];

    if (std::strcmp(tname, "Foo") == 0)
    {
        DomainParticipantQos pqos = PARTICIPANT_QOS_DEFAULT;
        udp_only(pqos);
        DomainParticipant* pp =
            DomainParticipantFactory::get_instance()->create_participant(0, pqos);
        TypeSupport ts(new FooPubSubType());
        ts.register_type(pp);
        Topic* topic = pp->create_topic("Foo", ts.get_type_name(), TOPIC_QOS_DEFAULT);
        if (std::strcmp(role, "sub") == 0)
        {
            Subscriber* sub = pp->create_subscriber(SUBSCRIBER_QOS_DEFAULT);
            DataReaderQos rqos = DATAREADER_QOS_DEFAULT;
            rqos.reliability().kind = RELIABLE_RELIABILITY_QOS;
            rqos.durability().kind = TRANSIENT_LOCAL_DURABILITY_QOS;
            rqos.data_sharing().off();
            sub->create_datareader(topic, rqos);
            std::cout << "sub Foo ready\n" << std::flush;
            std::this_thread::sleep_for(std::chrono::seconds(15));
            return 0;
        }
        Publisher* pub = pp->create_publisher(PUBLISHER_QOS_DEFAULT);
        DataWriterQos wqos = DATAWRITER_QOS_DEFAULT;
        wqos.reliability().kind = RELIABLE_RELIABILITY_QOS;
        wqos.durability().kind = TRANSIENT_LOCAL_DURABILITY_QOS;
        wqos.data_sharing().off();
        DataWriter* wr = pub->create_datawriter(topic, wqos);
        for (int i = 0; i < 100; i++) {
            PublicationMatchedStatus ms;
            wr->get_publication_matched_status(ms);
            if (ms.current_count > 0) break;
            std::this_thread::sleep_for(std::chrono::milliseconds(100));
        }
        Foo s; s.id(0x12345678); s.x(10); s.y(20);
        for (int i = 0; i < 5; i++) { wr->write(&s); std::this_thread::sleep_for(std::chrono::milliseconds(300)); }
        std::cout << "pub Foo wrote\n" << std::flush;
        std::this_thread::sleep_for(std::chrono::milliseconds(500));
        return 0;
    }

    if (std::strcmp(tname, "NoKeyFinal") == 0)
    {
        DomainParticipantQos pqos = PARTICIPANT_QOS_DEFAULT;
        udp_only(pqos);
        DomainParticipant* pp =
            DomainParticipantFactory::get_instance()->create_participant(0, pqos);
        TypeSupport ts(new NoKeyFinalPubSubType());
        ts.register_type(pp);
        Topic* topic = pp->create_topic("NoKeyFinal", ts.get_type_name(), TOPIC_QOS_DEFAULT);
        if (std::strcmp(role, "sub") == 0)
        {
            Subscriber* sub = pp->create_subscriber(SUBSCRIBER_QOS_DEFAULT);
            DataReaderQos rqos = DATAREADER_QOS_DEFAULT;
            rqos.reliability().kind = RELIABLE_RELIABILITY_QOS;
            rqos.durability().kind = TRANSIENT_LOCAL_DURABILITY_QOS;
            rqos.data_sharing().off();
            sub->create_datareader(topic, rqos);
            std::cout << "sub NoKeyFinal ready\n" << std::flush;
            std::this_thread::sleep_for(std::chrono::seconds(15));
            return 0;
        }
        Publisher* pub = pp->create_publisher(PUBLISHER_QOS_DEFAULT);
        DataWriterQos wqos = DATAWRITER_QOS_DEFAULT;
        wqos.reliability().kind = RELIABLE_RELIABILITY_QOS;
        wqos.durability().kind = TRANSIENT_LOCAL_DURABILITY_QOS;
        wqos.data_sharing().off();
        DataWriter* wr = pub->create_datawriter(topic, wqos);
        for (int i = 0; i < 100; i++) {
            PublicationMatchedStatus ms;
            wr->get_publication_matched_status(ms);
            if (ms.current_count > 0) break;
            std::this_thread::sleep_for(std::chrono::milliseconds(100));
        }
        NoKeyFinal s; s.a(0x12345678); s.b(20);
        { PublicationMatchedStatus ms; wr->get_publication_matched_status(ms);
          std::cout << "matched=" << ms.current_count << std::endl; }
        for (int i = 0; i < 5; i++) {
            eprosima::fastdds::dds::ReturnCode_t rc =
                wr->write(&s, eprosima::fastdds::rtps::InstanceHandle_t());
            std::cout << "write " << i << " -> rc=" << static_cast<int>(rc) << std::endl;
            std::this_thread::sleep_for(std::chrono::milliseconds(300));
        }
        std::cout << "pub NoKeyFinal wrote\n" << std::flush;
        std::this_thread::sleep_for(std::chrono::milliseconds(500));
        return 0;
    }

    std::cerr << "unknown type\n";
    return 2;
}
