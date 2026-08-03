// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Fast DDS <-> ZeroDDS interop client for the #28 CI gate.
//
// Typed `Robot` pub/sub over live DDS/RTPS. The `Robot` type + PubSubType are
// generated from robot.idl by Fast-DDS-Gen at build time (see CMakeLists.txt) —
// same structure as the ZeroDDS/Cyclone `Robot`, @final so the wire form
// (XCDR1, no DHEADER) matches the ZeroDDS @final endpoints.
//
// Usage:
//   fastdds_robot pub [seconds]     write Robot samples on domain/topic
//   fastdds_robot sub [seconds]     count decoded samples, print result
//   fastdds_robot version           print the Fast DDS version
//
// Domain id comes from ZERODDS_DOMAIN (default 100), matching the runner's
// per-cell domain. Topic is "robot".
//
// The subscriber prints exactly one machine-readable line to stdout:
//   FASTDDS_RESULT matched=<0|1> samples=<n>

#include "RobotPubSubTypes.hpp"

#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/publisher/DataWriter.hpp>
#include <fastdds/dds/publisher/DataWriterListener.hpp>
#include <fastdds/dds/publisher/Publisher.hpp>
#include <fastdds/dds/publisher/qos/DataWriterQos.hpp>
#include <fastdds/dds/subscriber/DataReader.hpp>
#include <fastdds/dds/subscriber/DataReaderListener.hpp>
#include <fastdds/dds/subscriber/SampleInfo.hpp>
#include <fastdds/dds/subscriber/Subscriber.hpp>
#include <fastdds/dds/subscriber/qos/DataReaderQos.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>

#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <string>
#include <thread>

using namespace eprosima::fastdds::dds;

namespace {

constexpr const char* kTopic = "robot";

int resolve_domain()
{
    const char* env = std::getenv("ZERODDS_DOMAIN");
    if (env != nullptr)
    {
        char* end = nullptr;
        long v = std::strtol(env, &end, 10);
        if (end != env && v >= 0 && v < 233)
        {
            return static_cast<int>(v);
        }
    }
    return 100;
}

class MatchWriterListener : public DataWriterListener
{
public:
    std::atomic<int> matched{0};
    void on_publication_matched(DataWriter*, const PublicationMatchedStatus& s) override
    {
        matched.store(s.current_count);
    }
};

class CountReaderListener : public DataReaderListener
{
public:
    std::atomic<int> matched{0};
    std::atomic<int> samples{0};

    void on_subscription_matched(DataReader*, const SubscriptionMatchedStatus& s) override
    {
        matched.store(s.current_count);
    }

    void on_data_available(DataReader* reader) override
    {
        Robot sample;
        SampleInfo info;
        while (reader->take_next_sample(&sample, &info) == eprosima::fastdds::dds::RETCODE_OK)
        {
            if (info.valid_data)
            {
                samples.fetch_add(1);
            }
        }
    }
};

DomainParticipant* make_participant()
{
    return DomainParticipantFactory::get_instance()->create_participant(
        resolve_domain(), PARTICIPANT_QOS_DEFAULT);
}

int run_pub(double seconds)
{
    DomainParticipant* dp = make_participant();
    if (dp == nullptr) { std::cerr << "fastdds: create_participant failed\n"; return 2; }

    TypeSupport type_support(new RobotPubSubType());
    type_support.register_type(dp);

    Topic* topic = dp->create_topic(kTopic, type_support.get_type_name(), TOPIC_QOS_DEFAULT);
    Publisher* pub = dp->create_publisher(PUBLISHER_QOS_DEFAULT);
    MatchWriterListener wl;
    DataWriter* writer = pub->create_datawriter(topic, DATAWRITER_QOS_DEFAULT, &wl);
    if (writer == nullptr) { std::cerr << "fastdds: create_datawriter failed\n"; return 2; }

    std::cerr << "[fastdds pub] domain=" << resolve_domain() << " topic=" << kTopic << "\n";
    auto t0 = std::chrono::steady_clock::now();
    std::uint32_t c = 0;
    while (std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count() < seconds)
    {
        Robot sample;
        sample.id(1);
        sample.label(c % 1000);
        writer->write(&sample);
        ++c;
        std::this_thread::sleep_for(std::chrono::milliseconds(300));
    }

    DomainParticipantFactory::get_instance()->delete_participant(dp);
    return 0;
}

int run_sub(double seconds)
{
    DomainParticipant* dp = make_participant();
    if (dp == nullptr) { std::cerr << "fastdds: create_participant failed\n"; return 2; }

    TypeSupport type_support(new RobotPubSubType());
    type_support.register_type(dp);

    Topic* topic = dp->create_topic(kTopic, type_support.get_type_name(), TOPIC_QOS_DEFAULT);
    Subscriber* sub = dp->create_subscriber(SUBSCRIBER_QOS_DEFAULT);
    CountReaderListener rl;
    DataReader* reader = sub->create_datareader(topic, DATAREADER_QOS_DEFAULT, &rl);
    if (reader == nullptr) { std::cerr << "fastdds: create_datareader failed\n"; return 2; }

    std::cerr << "[fastdds sub] domain=" << resolve_domain() << " topic=" << kTopic << "\n";
    auto t0 = std::chrono::steady_clock::now();
    while (std::chrono::duration<double>(std::chrono::steady_clock::now() - t0).count() < seconds)
    {
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }

    int matched = rl.matched.load() > 0 ? 1 : 0;
    std::cout << "FASTDDS_RESULT matched=" << matched << " samples=" << rl.samples.load()
              << std::endl;

    DomainParticipantFactory::get_instance()->delete_participant(dp);
    return 0;
}

}  // namespace

int main(int argc, char** argv)
{
    std::string mode = argc > 1 ? argv[1] : "";
    double seconds = argc > 2 ? std::strtod(argv[2], nullptr) : 12.0;

    if (mode == "version")
    {
        std::cout << "fastdds " <<
#ifdef FASTDDS_VERSION_STR
            FASTDDS_VERSION_STR
#else
            "unknown"
#endif
            << std::endl;
        return 0;
    }
    if (mode == "pub")
    {
        return run_pub(seconds);
    }
    if (mode == "sub")
    {
        return run_sub(seconds);
    }
    std::cerr << "usage: fastdds_robot <pub|sub|version> [seconds]\n";
    return 2;
}
