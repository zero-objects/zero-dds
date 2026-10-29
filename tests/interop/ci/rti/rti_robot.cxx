// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// RTI Connext <-> ZeroDDS interop client for the #28 CI gate.
//
// The counterpart of `../fastdds/fastdds_robot.cpp`, in RTI's modern C++11
// API, on the *same* `Robot` type: `build.sh` runs `rtiddsgen` over
// `../fastdds/robot.idl` rather than a copy, so the two vendor cells cannot
// drift into measuring two different types. @final, so the wire form (XCDR1,
// no DHEADER) matches the ZeroDDS @final endpoints.
//
// Usage:
//   rti_robot pub [seconds] [domain]   write Robot samples on domain/topic
//   rti_robot sub [seconds] [domain]   count decoded samples, print result
//   rti_robot version                  print the Connext version
//
// Domain id is the 3rd argument (default 100), matching the runner's per-cell
// domain. Topic is "robot".
//
// The subscriber prints exactly one machine-readable line to stdout:
//   RTI_RESULT matched=<0|1> samples=<n>
//
// Two pieces of QoS are not the RTI defaults and both are load-bearing; see
// USER_QOS_PROFILES.xml, which this program loads as the default profile:
// the participant is restricted to UDPv4, because same-host RTI otherwise
// delivers over shared memory, which no foreign stack can read; and the
// reader announces both data representations, because RTI's default reader
// announces XCDR1 alone and drops an XCDR2 writer without a word.

#include <dds/dds.hpp>
#include "robot.hpp"
#include <ndds/ndds_version.h>

#include <chrono>
#include <cstdlib>
#include <iostream>
#include <string>
#include <thread>

namespace {

const char *kTopic = "robot";

int run_pub(double seconds, int domain)
{
    dds::domain::DomainParticipant dp(domain);
    dds::topic::Topic< ::Robot> topic(dp, kTopic);
    dds::pub::DataWriter< ::Robot> writer(dds::pub::Publisher(dp), topic);

    std::cerr << "[rti pub] domain=" << domain << " topic=" << kTopic << "\n";
    auto t0 = std::chrono::steady_clock::now();
    uint32_t c = 0;
    while (std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count() < seconds) {
        // rtiddsgen's C++11 mapping gives an aggregate public members, not
        // accessors; the Fast DDS mapping gives accessors. Same IDL, two
        // spellings.
        ::Robot sample;
        sample.id = 1;
        sample.label = c % 1000;
        writer.write(sample);
        ++c;
        std::this_thread::sleep_for(std::chrono::milliseconds(300));
    }
    return 0;
}

int run_sub(double seconds, int domain)
{
    dds::domain::DomainParticipant dp(domain);
    dds::topic::Topic< ::Robot> topic(dp, kTopic);

    dds::sub::Subscriber subscriber(dp);
    dds::sub::qos::DataReaderQos rqos = subscriber.default_datareader_qos();
    // The two representation ids of XTypes 1.3 §7.6.3.1.2 by value: RTI's
    // C++11 header exposes `DataRepresentationId` as the plain integer and
    // carries no XCDR/XCDR2 constants on the policy class.
    std::vector<dds::core::policy::DataRepresentationId> reps;
    reps.push_back(0);  // XCDR_DATA_REPRESENTATION
    reps.push_back(2);  // XCDR2_DATA_REPRESENTATION
    rqos << dds::core::policy::DataRepresentation(reps);
    dds::sub::DataReader< ::Robot> reader(subscriber, topic, rqos);

    std::cerr << "[rti sub] domain=" << domain << " topic=" << kTopic << "\n";
    int samples = 0;
    int matched = 0;
    auto t0 = std::chrono::steady_clock::now();
    while (std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count() < seconds) {
        for (const auto &s : reader.take()) {
            if (s.info().valid()) {
                ++samples;
            }
        }
        if (!dds::sub::matched_publications(reader).empty()) {
            matched = 1;
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }

    std::cout << "RTI_RESULT matched=" << matched << " samples=" << samples << std::endl;
    return 0;
}

// The runner passes the domain as the third argument. A value it cannot read
// is refused rather than silently replaced: a cell that quietly moves to
// domain 100 would measure whatever else is there and call it interop.
int resolve_domain(int argc, char **argv, int idx)
{
    if (argc <= idx) {
        return 100;  // documented manual default when the argument is omitted
    }
    char *end = nullptr;
    long v = std::strtol(argv[idx], &end, 10);
    if (end == argv[idx] || *end != '\0' || v < 0 || v > 232) {
        std::cerr << "invalid domain " << argv[idx]
                  << ": expected an integer in 0..=232\n";
        std::exit(2);
    }
    return static_cast<int>(v);
}

}  // namespace

int main(int argc, char **argv)
{
    std::string mode = argc > 1 ? argv[1] : "";
    double seconds = argc > 2 ? std::strtod(argv[2], nullptr) : 12.0;
    int domain = resolve_domain(argc, argv, 3);

    try {
        if (mode == "version") {
            std::cout << "rti " << RTI_DDS_VERSION_MAJOR << "."
                      << RTI_DDS_VERSION_MINOR << "."
                      << RTI_DDS_VERSION_RELEASE << "."
                      << RTI_DDS_VERSION_REVISION << std::endl;
            return 0;
        }
        if (mode == "pub") {
            return run_pub(seconds, domain);
        }
        if (mode == "sub") {
            return run_sub(seconds, domain);
        }
    } catch (const std::exception &e) {
        std::cerr << "rti_robot: " << e.what() << "\n";
        return 2;
    }
    std::cerr << "usage: rti_robot <pub|sub|version> [seconds] [domain]\n";
    return 2;
}
