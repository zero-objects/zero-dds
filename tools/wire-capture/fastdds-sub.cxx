#include <iostream>
#include <thread>
#include <chrono>
#include <cstdlib>
#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/subscriber/Subscriber.hpp>
#include <fastdds/dds/subscriber/DataReader.hpp>
#include <fastdds/dds/subscriber/SampleInfo.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include <fastdds/rtps/transport/UDPv4TransportDescriptor.hpp>
#include "gapprobePubSubTypes.hpp"

using namespace eprosima::fastdds::dds;

int main(int argc, char **argv)
{
    int domain_id = (argc > 1) ? std::atoi(argv[1]) : 0;
    int seconds = (argc > 2) ? std::atoi(argv[2]) : 20;

    DomainParticipantQos pqos;
    pqos.transport().use_builtin_transports = false;
    auto udp = std::make_shared<eprosima::fastdds::rtps::UDPv4TransportDescriptor>();
    udp->interfaceWhiteList.push_back("127.0.0.1");
    pqos.transport().user_transports.push_back(udp);

    DomainParticipant *participant =
        DomainParticipantFactory::get_instance()->create_participant(domain_id, pqos);
    TypeSupport type(new GapProbePubSubType());
    type.register_type(participant);
    Topic *topic = participant->create_topic("GapProbeTopic", type.get_type_name(), TOPIC_QOS_DEFAULT);
    Subscriber *sub = participant->create_subscriber(SUBSCRIBER_QOS_DEFAULT);
    DataReaderQos rqos = DATAREADER_QOS_DEFAULT;
    rqos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    rqos.history().kind = KEEP_LAST_HISTORY_QOS;
    rqos.history().depth = 10;
    rqos.data_sharing().off();
    rqos.representation().m_value.clear();
    rqos.representation().m_value.push_back(XCDR_DATA_REPRESENTATION);
    rqos.representation().m_value.push_back(XCDR2_DATA_REPRESENTATION);
    DataReader *reader = sub->create_datareader(topic, rqos);

    int n = 0;
    for (int i = 0; i < seconds * 10; i++) {
        GapProbe s; SampleInfo info;
        while (reader->take_next_sample(&s, &info) == eprosima::fastdds::dds::RETCODE_OK)
            if (info.valid_data) n++;
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }
    std::cout << n << std::endl;
    return 0;
}
