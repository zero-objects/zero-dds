// tests/interop/fastdds_pub.cpp
//
// Minimaler Fast-DDS-Participant fuer den Multi-Vendor-Interop-Test.
// Erstellt einen Domain-Participant in Domain 0 und laesst ihn 30s
// SPDP-Beacons senden. Der ZeroDDS-spdp_demo soll diesen Participant
// via SPDP entdecken.
//
// Build:
//   g++ -std=c++17 fastdds_pub.cpp -o fastdds_pub \
//       -lfastrtps -lfastcdr -lpthread
//
// Voraussetzungen (Debian 12):
//   sudo apt-get install -y libfastrtps-dev libfastcdr-dev g++

#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/domain/qos/DomainParticipantQos.hpp>

#include <chrono>
#include <iostream>
#include <thread>

int main() {
    using namespace eprosima::fastdds::dds;
    DomainParticipantQos qos;
    qos.name("zerodds_interop_fastdds_pub");
    DomainParticipant *p =
        DomainParticipantFactory::get_instance()->create_participant(0, qos);
    if (!p) {
        std::cerr << "create_participant failed\n";
        return 1;
    }
    std::cout << "Fast-DDS Participant created in domain 0; "
                 "sending SPDP beacons for 30s.\n";
    std::this_thread::sleep_for(std::chrono::seconds(30));
    DomainParticipantFactory::get_instance()->delete_participant(p);
    return 0;
}
