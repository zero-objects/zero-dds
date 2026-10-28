// Alternating long/short samples through a KeepLast(1) writer, so the
// serialisation buffer is reused and a short sample`s fill positions land
// on a long sample`s octets.
#include <iostream>
#include <string>
#include <thread>
#include <chrono>
#include <cstdlib>
#include <dds/pub/ddspub.hpp>
#include <dds/core/QosProvider.hpp>
#include <rti/core/policy/CorePolicy.hpp>
#include "gapprobe.hpp"

int main(int argc, char **argv)
{
    int domain_id = (argc > 1) ? std::atoi(argv[1]) : 0;
    std::string rep = (argc > 2) ? argv[2] : "2";

    dds::domain::qos::DomainParticipantQos pqos =
        dds::core::QosProvider::Default().participant_qos();
    pqos->transport_builtin.mask(rti::core::policy::TransportBuiltinMask::udpv4());
    pqos->property.set({"dds.transport.UDPv4.builtin.parent.allow_interfaces_list", "lo"});
    pqos->discovery.initial_peers({"udpv4://127.0.0.1"});
    pqos->discovery.multicast_receive_addresses(std::vector<std::string>());

    dds::domain::DomainParticipant participant(domain_id, pqos);
    dds::topic::Topic< ::GapProbe> topic(participant, "GapProbeTopic");
    dds::pub::Publisher publisher(participant);

    dds::pub::qos::DataWriterQos wqos = publisher.default_datawriter_qos();
    wqos << dds::core::policy::Reliability::Reliable();
    wqos << dds::core::policy::History::KeepLast(1);
    std::vector<dds::core::policy::DataRepresentationId> reps;
    reps.push_back(rep == "1" ? dds::core::policy::DataRepresentation::xcdr()
                              : dds::core::policy::DataRepresentation::xcdr2());
    wqos << dds::core::policy::DataRepresentation(reps);
    dds::pub::DataWriter< ::GapProbe> writer(publisher, topic, wqos);

    for (int i = 0; i < 200 && writer.publication_matched_status().current_count() == 0; i++)
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
    if (writer.publication_matched_status().current_count() == 0) {
        auto st = writer.offered_incompatible_qos_status();
        std::cerr << "no reader matched; incompatible total=" << st.total_count()
                  << " last_policy=" << (unsigned)st.last_policy_id() << std::endl;
        return 1;
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(500));

    ::GapProbe a;
    a.s = std::string(60, 0x41); a.b = 0x11223344; a.c = 0x55;
    a.d = 1.5; a.e = 0x66; a.f = 0x7788;
    ::GapProbe b;
    b.s = std::string(4, 0x42); b.b = 0x01020304; b.c = 0x05;
    b.d = 2.5; b.e = 0x06; b.f = 0x0708;

    for (int i = 0; i < 15; i++) {
        writer.write(a);
        std::this_thread::sleep_for(std::chrono::milliseconds(250));
        writer.write(b);
        std::this_thread::sleep_for(std::chrono::milliseconds(250));
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(600));
    std::cout << "wrote 10 alternating samples, xcdr" << rep << std::endl;
    return 0;
}
