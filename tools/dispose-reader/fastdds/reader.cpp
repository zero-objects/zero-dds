// A Fast DDS reader of KeyedPointA that reports instance states, for
// crates/zerodds-c-api/tests/writer_dispose_sample_vendors.rs.
//
//   reader DOMAIN TOPIC SECONDS SNAPSHOT_FILE STOP_FILE
//
// Prints READY once the reader exists, MATCHED once it has matched a
// writer and SEEN id=<id> for the first valid sample of each instance.
// When SNAPSHOT_FILE appears it prints `SNAP id=<id> state=<state>`
// per instance (the state of the last sample taken for it) and SNAPDONE;
// when STOP_FILE appears, or after SECONDS, the same as FINAL lines.
#include <chrono>
#include <cstdlib>
#include <iostream>
#include <map>
#include <string>
#include <sys/stat.h>
#include <thread>

#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/subscriber/DataReader.hpp>
#include <fastdds/dds/subscriber/SampleInfo.hpp>
#include <fastdds/dds/subscriber/Subscriber.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include <fastdds/rtps/transport/UDPv4TransportDescriptor.hpp>

#include "KeyedPointA.hpp"
#include "KeyedPointAPubSubTypes.hpp"

using namespace eprosima::fastdds::dds;

static bool exists(const std::string& path)
{
    struct stat st;
    return stat(path.c_str(), &st) == 0;
}

static const char* state_name(InstanceStateKind s)
{
    switch (s)
    {
        case ALIVE_INSTANCE_STATE: return "ALIVE";
        case NOT_ALIVE_DISPOSED_INSTANCE_STATE: return "NOT_ALIVE_DISPOSED";
        case NOT_ALIVE_NO_WRITERS_INSTANCE_STATE: return "NOT_ALIVE_NO_WRITERS";
        default: return "UNKNOWN";
    }
}

int main(int argc, char** argv)
{
    if (argc != 6)
    {
        std::cerr << "usage: reader DOMAIN TOPIC SECONDS SNAPSHOT_FILE STOP_FILE\n";
        return 2;
    }
    const int domain = std::atoi(argv[1]);
    const std::string snapshot = argv[4], stop = argv[5];
    // UDP only: a ZeroDDS peer has no Fast DDS shared-memory segment.
    DomainParticipantQos pqos = PARTICIPANT_QOS_DEFAULT;
    pqos.transport().use_builtin_transports = false;
    pqos.transport().user_transports.push_back(
        std::make_shared<eprosima::fastdds::rtps::UDPv4TransportDescriptor>());
    DomainParticipant* dp =
        DomainParticipantFactory::get_instance()->create_participant(domain, pqos);
    TypeSupport ts(new KeyedPointAPubSubType());
    ts.register_type(dp);
    Topic* topic = dp->create_topic(argv[2], ts.get_type_name(), TOPIC_QOS_DEFAULT);
    Subscriber* sub = dp->create_subscriber(SUBSCRIBER_QOS_DEFAULT);
    DataReaderQos rq = DATAREADER_QOS_DEFAULT;
    rq.reliability().kind = RELIABLE_RELIABILITY_QOS;
    rq.history().kind = KEEP_ALL_HISTORY_QOS;
    rq.data_sharing().off();
    // Without it the reader accepts XCDR1 only (no PID_DATA_REPRESENTATION,
    // measured) and does not match a writer whose first representation is
    // XCDR2 (XTypes 1.3 §7.6.3.1.1), as a ZeroDDS typed writer's is.
    rq.representation().m_value.push_back(XCDR2_DATA_REPRESENTATION);
    rq.representation().m_value.push_back(XCDR_DATA_REPRESENTATION);
    DataReader* rd = sub->create_datareader(topic, rq);
    if (rd == nullptr)
    {
        std::cerr << "no reader\n";
        return 1;
    }
    std::cout << "READY" << std::endl;

    std::map<std::string, int32_t> ids;
    std::map<std::string, std::string> states;
    bool matched = false, snapped = false;
    const auto end = std::chrono::steady_clock::now() +
            std::chrono::milliseconds(static_cast<long>(std::atof(argv[3]) * 1000));
    while (std::chrono::steady_clock::now() < end && !exists(stop))
    {
        SubscriptionMatchedStatus st;
        rd->get_subscription_matched_status(st);
        if (!matched && st.current_count >= 1)
        {
            std::cout << "MATCHED" << std::endl;
            matched = true;
        }
        KeyedPointA s;
        SampleInfo info;
        while (rd->take_next_sample(&s, &info) == RETCODE_OK)
        {
            std::string h;
            for (int i = 0; i < 16; ++i)
            {
                h.push_back(static_cast<char>(info.instance_handle.value[i]));
            }
            if (info.valid_data)
            {
                if (ids.find(h) == ids.end())
                {
                    std::cout << "SEEN id=" << s.id() << std::endl;
                }
                ids[h] = s.id();
            }
            states[h] = state_name(info.instance_state);
        }
        if (!snapped && exists(snapshot))
        {
            for (const auto& [h, state] : states)
            {
                auto id = ids.find(h);
                std::cout << "SNAP id=" << (id == ids.end() ? std::string("?") : std::to_string(id->second))
                          << " state=" << state << std::endl;
            }
            std::cout << "SNAPDONE" << std::endl;
            snapped = true;
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(20));
    }
    for (const auto& [h, state] : states)
    {
        auto id = ids.find(h);
        std::cout << "FINAL id=" << (id == ids.end() ? std::string("?") : std::to_string(id->second))
                  << " state=" << state << std::endl;
    }
    dp->delete_contained_entities();
    DomainParticipantFactory::get_instance()->delete_participant(dp);
    return 0;
}
