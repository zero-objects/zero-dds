#include <iostream>
#include <thread>
#include <chrono>
#include <cstdlib>
#include <dds/sub/ddssub.hpp>
#include <dds/core/QosProvider.hpp>
#include <rti/core/policy/CorePolicy.hpp>
#include "gapprobe.hpp"

int main(int argc, char **argv)
{
    int domain_id = (argc > 1) ? std::atoi(argv[1]) : 0;
    int seconds = (argc > 2) ? std::atoi(argv[2]) : 12;

    dds::domain::qos::DomainParticipantQos pqos =
        dds::core::QosProvider::Default().participant_qos();
    pqos->transport_builtin.mask(rti::core::policy::TransportBuiltinMask::udpv4());
    pqos->property.set({"dds.transport.UDPv4.builtin.parent.allow_interfaces_list", "lo"});
    pqos->discovery.initial_peers({"udpv4://127.0.0.1"});
    pqos->discovery.multicast_receive_addresses(std::vector<std::string>());

    dds::domain::DomainParticipant participant(domain_id, pqos);
    dds::topic::Topic< ::GapProbe> topic(participant, "GapProbeTopic");
    dds::sub::Subscriber subscriber(participant);
    dds::sub::qos::DataReaderQos rqos = subscriber.default_datareader_qos();
    rqos << dds::core::policy::Reliability::Reliable();
    rqos << dds::core::policy::History::KeepAll();
    std::vector<dds::core::policy::DataRepresentationId> reps;
    reps.push_back(dds::core::policy::DataRepresentation::xcdr());
    reps.push_back(dds::core::policy::DataRepresentation::xcdr2());
    rqos << dds::core::policy::DataRepresentation(reps);
    dds::sub::DataReader< ::GapProbe> reader(subscriber, topic, rqos);

    for (int i = 0; i < seconds * 10; i++) {
        auto samples = reader.take();
        for (const auto &s : samples)
            if (s.info().valid())
                std::cout << "read s.len=" << s.data().s.size()
                          << " b=" << s.data().b << std::endl;
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }
    return 0;
}
