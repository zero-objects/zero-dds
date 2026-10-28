// SPDX-License-Identifier: Apache-2.0
//
// FastDDS C++ roundtrip bench app. Apples-to-apples with Cyclone+RTI+ZeroDDS
// via custom IDL.
//
// IDL: roundtrip.idl  →  RoundtripBench::Roundtrip  (via fastddsgen)
// With BENCH_RICH defined (target fastdds-roundtrip-rich) the sample type is
// the codec-heavy roundtrip_rich.idl → RoundtripBench::RoundtripRich, on the
// topics RoundtripRichBench_* of the other rich builds.
//
// Pipeline-Schichten:
//   App-Code (C++)                  -- typed RoundtripBench::Roundtrip
//   ↓
//   fastddsgen-Codegen + PubSubType -- CDR-Encode/Decode auto-generated
//   ↓
//   FastDDS DDS-API                 -- DataWriter/DataReader (typed)
//   ↓
//   libfastrtps                     -- RTPS-Engine
//   ↓
//   UDP-Loopback                    -- Transport
//
// Build:
//   g++ -std=c++17 -O2 -Wall \
//       -o fastdds-app fastdds_app.cpp roundtrip.cxx roundtripPubSubTypes.cxx \
//       -lfastrtps -lfastcdr -lpthread

#ifdef BENCH_RICH
#include "roundtrip_rich.hpp"
#include "roundtrip_richPubSubTypes.hpp"
#else
#include "roundtrip.hpp"
#include "roundtripPubSubTypes.hpp"
#endif
#include "bench_common.hpp"

#include <fastdds/dds/domain/DomainParticipant.hpp>
#include <fastdds/dds/domain/DomainParticipantFactory.hpp>
#include <fastdds/dds/publisher/DataWriter.hpp>
#include <fastdds/dds/publisher/Publisher.hpp>
#include <fastdds/dds/publisher/qos/DataWriterQos.hpp>
#include <fastdds/dds/subscriber/DataReader.hpp>
#include <fastdds/dds/subscriber/DataReaderListener.hpp>
#include <fastdds/dds/subscriber/SampleInfo.hpp>
#include <fastdds/dds/subscriber/Subscriber.hpp>
#include <fastdds/dds/subscriber/qos/DataReaderQos.hpp>
#include <fastdds/dds/topic/TypeSupport.hpp>
#include <fastdds/rtps/transport/UDPv4TransportDescriptor.hpp>
#include <fastdds/rtps/transport/UDPv6TransportDescriptor.hpp>
#include <fastdds/rtps/transport/shared_mem/SharedMemTransportDescriptor.hpp>
#include <fastdds/rtps/transport/TCPv4TransportDescriptor.hpp>

#include <array>
#include <chrono>
#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

using namespace eprosima::fastdds::dds;

namespace {

using bench::now_ns;
using bench::PingState;
using bench::print_quantiles;

static DataRepresentationId_t bench_repr() {
    return bench::xcdr1_requested() ? XCDR_DATA_REPRESENTATION : XCDR2_DATA_REPRESENTATION;
}
static uint32_t resolve_domain() {
    return static_cast<uint32_t>(bench::env_domain());
}

#ifdef BENCH_RICH
using Sample = RoundtripBench::RoundtripRich;
using SamplePubSubType = RoundtripBench::RoundtripRichPubSubType;
constexpr const char* kProgram   = "fastdds-app-rich";
constexpr const char* kReqTopic  = "RoundtripRichBench_Request";
constexpr const char* kEchoTopic = "RoundtripRichBench_Echo";
constexpr size_t kNumWaypoints = 8;

void populate(Sample& m, size_t payload_size) {
    using RoundtripBench::Vec3;
    using RoundtripBench::Waypoint;
    m.name(std::string("fastdds-rich-roundtrip-bench"));
    std::array<double, 16> tr{};
    for (size_t i = 0; i < 16; ++i) tr[i] = static_cast<double>(i) * 1.5 + 0.25;
    m.transform(tr);
    std::vector<Waypoint> wps;
    wps.reserve(kNumWaypoints);
    for (size_t i = 0; i < kNumWaypoints; ++i) {
        Waypoint w;
        w.id(static_cast<uint32_t>(i));
        Vec3 pos; pos.x(1.0 * i); pos.y(2.0 * i); pos.z(3.0 * i);
        Vec3 vel; vel.x(0.1 * i); vel.y(0.2 * i); vel.z(0.3 * i);
        w.position(pos);
        w.velocity(vel);
        w.label(std::string("wp-") + std::to_string(i));
        w.weight(0.5 * static_cast<double>(i) + 1.0);
        wps.push_back(w);
    }
    m.waypoints(wps);
    m.payload(std::vector<uint8_t>(payload_size, 0xAB));
}
#else
using Sample = RoundtripBench::Roundtrip;
using SamplePubSubType = RoundtripBench::RoundtripPubSubType;
constexpr const char* kProgram   = "fastdds-app";
constexpr const char* kReqTopic  = "RoundtripBench_Request";
constexpr const char* kEchoTopic = "RoundtripBench_Echo";

void populate(Sample& m, size_t payload_size) {
    m.payload(std::vector<uint8_t>(payload_size, 0xAB));
}
#endif

// Transport-Selection via env-var ZERODDS_BENCH_TRANSPORT:
//   UDPv4 (default), UDPv6, SHM, TCPv4.
// FastDDS uses SHM + UDP by default. We set exactly one
// transport instance per run so that the bench is transport-pure.
enum class BenchTransport { UDPv4, UDPv6, SHM, TCPv4 };
BenchTransport resolve_transport() {
    const char* s = std::getenv("ZERODDS_BENCH_TRANSPORT");
    if (!s) return BenchTransport::UDPv4;
    std::string v(s);
    if (v == "UDPv4") return BenchTransport::UDPv4;
    if (v == "UDPv6") return BenchTransport::UDPv6;
    if (v == "SHM")   return BenchTransport::SHM;
    if (v == "TCPv4") return BenchTransport::TCPv4;
    return BenchTransport::UDPv4;
}
void apply_transport(DomainParticipantQos& pqos) {
    pqos.transport().use_builtin_transports = false;
    switch (resolve_transport()) {
    case BenchTransport::UDPv4: {
        auto t = std::make_shared<
            eprosima::fastdds::rtps::UDPv4TransportDescriptor>();
        pqos.transport().user_transports.push_back(t);
        break;
    }
    case BenchTransport::UDPv6: {
        auto t = std::make_shared<
            eprosima::fastdds::rtps::UDPv6TransportDescriptor>();
        pqos.transport().user_transports.push_back(t);
        break;
    }
    case BenchTransport::SHM: {
        auto t = std::make_shared<
            eprosima::fastdds::rtps::SharedMemTransportDescriptor>();
        pqos.transport().user_transports.push_back(t);
        break;
    }
    case BenchTransport::TCPv4: {
        auto t = std::make_shared<
            eprosima::fastdds::rtps::TCPv4TransportDescriptor>();
        // TCPv4 server side: listening port; ping connects as client.
        // Default 0 = ephemeral; explicitly 5100 for deterministic tests.
        t->add_listener_port(0);
        pqos.transport().user_transports.push_back(t);
        break;
    }
    }
}

// DDS-Security 1.2 Property-QoS Setup via env-var ZERODDS_BENCH_SECURITY.
// If set: loads PKI-DH auth + AccessControl + AES-GCM crypto plugins
// with paths from ZERODDS_BENCH_SEC_DIR (default /tmp/dds-bench-security/).
// ZERODDS_BENCH_SEC_NAME = "ping" or "pong" → selects identity cert + permissions.
void apply_security(DomainParticipantQos& pqos) {
    const char* sec = std::getenv("ZERODDS_BENCH_SECURITY");
    if (!sec || std::string(sec) != "1") return;
    const char* sec_dir = std::getenv("ZERODDS_BENCH_SEC_DIR");
    if (!sec_dir) sec_dir = "/tmp/dds-bench-security";
    const char* who = std::getenv("ZERODDS_BENCH_SEC_NAME");
    if (!who) who = "ping";
    auto& props = pqos.properties().properties();
    auto add = [&](const std::string& k, const std::string& v) {
        props.emplace_back(k, v);
    };
    add("dds.sec.auth.plugin", "builtin.PKI-DH");
    add("dds.sec.auth.builtin.PKI-DH.identity_ca",
        std::string("file://") + sec_dir + "/certs/identity_ca.pem");
    add("dds.sec.auth.builtin.PKI-DH.identity_certificate",
        std::string("file://") + sec_dir + "/certs/" + who + "_cert.pem");
    add("dds.sec.auth.builtin.PKI-DH.private_key",
        std::string("file://") + sec_dir + "/certs/" + who + "_key.pem");
    add("dds.sec.access.plugin", "builtin.Access-Permissions");
    add("dds.sec.access.builtin.Access-Permissions.permissions_ca",
        std::string("file://") + sec_dir + "/certs/permissions_ca.pem");
    add("dds.sec.access.builtin.Access-Permissions.governance",
        std::string("file://") + sec_dir + "/governance.p7s");
    add("dds.sec.access.builtin.Access-Permissions.permissions",
        std::string("file://") + sec_dir + "/permissions_" + who + ".p7s");
    add("dds.sec.crypto.plugin", "builtin.AES-GCM-GMAC");
    // ZERODDS_BENCH_SEC_LOG=<file>: Fast DDS writes why it refuses a
    // handshake or a message only through its security logging plugin.
    const char* log_file = std::getenv("ZERODDS_BENCH_SEC_LOG");
    if (log_file && *log_file) {
        add("dds.sec.log.plugin", "builtin.DDS_LogTopic");
        add("dds.sec.log.builtin.DDS_LogTopic.logging_level", "DEBUG_LEVEL");
        add("dds.sec.log.builtin.DDS_LogTopic.log_file", log_file);
    }
}

// --- Pong: receive request, echo via listener-direct write ---

class PongListener : public DataReaderListener {
public:
    explicit PongListener(DataWriter* writer) : writer_(writer) {}

    void on_data_available(DataReader* reader) override {
        Sample msg;
        SampleInfo info;
        while (reader->take_next_sample(&msg, &info) == RETCODE_OK) {
            if (info.valid_data) {
                writer_->write(&msg);
            }
        }
    }

private:
    DataWriter* writer_;
};

int run_pong(uint64_t max_runtime_s) {
    DomainParticipantQos pqos;
    apply_transport(pqos);
    apply_security(pqos);
    auto* dp = DomainParticipantFactory::get_instance()->create_participant(resolve_domain(), pqos);
    if (!dp) { std::cerr << "dp create failed\n"; return 1; }

    TypeSupport type_support(new SamplePubSubType());
    type_support.register_type(dp);

    auto* topic_req  = dp->create_topic(kReqTopic,  type_support.get_type_name(), TOPIC_QOS_DEFAULT);
    auto* topic_echo = dp->create_topic(kEchoTopic, type_support.get_type_name(), TOPIC_QOS_DEFAULT);

    auto* publisher  = dp->create_publisher(PUBLISHER_QOS_DEFAULT);
    auto* subscriber = dp->create_subscriber(SUBSCRIBER_QOS_DEFAULT);

    DataWriterQos dw_qos;
    dw_qos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    dw_qos.history().kind = KEEP_LAST_HISTORY_QOS;
    dw_qos.history().depth = 64;
    // XCDR2-only: RTI is strict, accepts no writer that
    // advertises "XCDR1, XCDR2". Force all apps to XCDR2-only
    // so that RTI <-> all other vendors match.
    dw_qos.representation().m_value.clear();
    dw_qos.representation().m_value.push_back(bench_repr());
    auto* writer = publisher->create_datawriter(topic_echo, dw_qos);

    DataReaderQos dr_qos;
    dr_qos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    dr_qos.history().kind = KEEP_LAST_HISTORY_QOS;
    dr_qos.history().depth = 64;
    dr_qos.representation().m_value.clear();
    dr_qos.representation().m_value.push_back(bench_repr());
    PongListener listener(writer);
    auto* reader = subscriber->create_datareader(topic_req, dr_qos, &listener);

    std::cout << "pong[fastdds]: matched, listener registered\n" << std::flush;
    std::this_thread::sleep_for(std::chrono::seconds(max_runtime_s));

    subscriber->delete_datareader(reader);
    publisher->delete_datawriter(writer);
    dp->delete_subscriber(subscriber);
    dp->delete_publisher(publisher);
    dp->delete_topic(topic_req);
    dp->delete_topic(topic_echo);
    DomainParticipantFactory::get_instance()->delete_participant(dp);
    return 0;
}

// --- Ping: send + listener captures RTT ---

class PingListener : public DataReaderListener {
public:
    explicit PingListener(PingState& st) : st_(st) {}

    void on_data_available(DataReader* reader) override {
        Sample msg;
        SampleInfo info;
        while (reader->take_next_sample(&msg, &info) == RETCODE_OK) {
            if (!info.valid_data) continue;
            st_.record(msg.t_send_ns());
        }
    }

private:
    PingState& st_;
};

int run_ping(size_t payload_size, uint64_t warmup, uint64_t samples) {
    DomainParticipantQos pqos;
    apply_transport(pqos);
    apply_security(pqos);
    auto* dp = DomainParticipantFactory::get_instance()->create_participant(resolve_domain(), pqos);
    if (!dp) { std::cerr << "dp create failed\n"; return 1; }

    TypeSupport type_support(new SamplePubSubType());
    type_support.register_type(dp);

    auto* topic_req  = dp->create_topic(kReqTopic,  type_support.get_type_name(), TOPIC_QOS_DEFAULT);
    auto* topic_echo = dp->create_topic(kEchoTopic, type_support.get_type_name(), TOPIC_QOS_DEFAULT);

    auto* publisher  = dp->create_publisher(PUBLISHER_QOS_DEFAULT);
    auto* subscriber = dp->create_subscriber(SUBSCRIBER_QOS_DEFAULT);

    DataWriterQos dw_qos;
    dw_qos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    dw_qos.history().kind = KEEP_LAST_HISTORY_QOS;
    dw_qos.history().depth = 64;
    dw_qos.representation().m_value.clear();
    dw_qos.representation().m_value.push_back(bench_repr());
    auto* writer = publisher->create_datawriter(topic_req, dw_qos);

    DataReaderQos dr_qos;
    dr_qos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    dr_qos.history().kind = KEEP_LAST_HISTORY_QOS;
    dr_qos.history().depth = 64;
    dr_qos.representation().m_value.clear();
    dr_qos.representation().m_value.push_back(bench_repr());
    PingState st;
    st.warmup = warmup;
    st.rtts.reserve(samples);
    PingListener listener(st);
    auto* reader = subscriber->create_datareader(topic_echo, dr_qos, &listener);

    // Auf echtes Endpoint-Matching warten statt sleep(500ms). Mit
    // With volatile durability (default), samples before the match
    // are lost → the roundtrip partner does not see seq 0..N.
    // get_publication_matched_status / get_subscription_matched_status
    // liefern current_count.
    {
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        while (std::chrono::steady_clock::now() < deadline) {
            eprosima::fastdds::dds::PublicationMatchedStatus pm;
            eprosima::fastdds::dds::SubscriptionMatchedStatus sm;
            writer->get_publication_matched_status(pm);
            reader->get_subscription_matched_status(sm);
            if (pm.current_count >= 1 && sm.current_count >= 1) break;
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }

    Sample msg;
    populate(msg, payload_size);

    uint64_t total = warmup + samples;
    for (uint64_t seq = 0; seq < total; ++seq) {
        msg.sequence_id(static_cast<uint32_t>(seq));
        msg.t_send_ns(now_ns());
        writer->write(&msg);

        std::unique_lock<std::mutex> lk(st.mu);
        st.cv.wait_for(lk, std::chrono::milliseconds(50),
                       [&] { return st.received > seq; });
    }

    print_quantiles(st.rtts, payload_size);

    subscriber->delete_datareader(reader);
    publisher->delete_datawriter(writer);
    dp->delete_subscriber(subscriber);
    dp->delete_publisher(publisher);
    dp->delete_topic(topic_req);
    dp->delete_topic(topic_echo);
    DomainParticipantFactory::get_instance()->delete_participant(dp);
    return 0;
}

}  // namespace

int main(int argc, char** argv) {
    if (argc < 2) {
        std::cerr <<
            "Usage:\n"
            "  " << kProgram << " pong [max_runtime_s]\n"
            "  " << kProgram << " ping --payload N [--samples N] [--warmup N]\n";
        return 2;
    }
    std::string mode = argv[1];
    if (mode == "pong") {
        uint64_t rt_s = (argc > 2) ? std::stoull(argv[2]) : 30;
        return run_pong(rt_s);
    } else if (mode == "ping") {
        bench::PingArgs a = bench::parse_ping_args(argc, argv, 2);
        return run_ping(a.payload, a.warmup, a.samples);
    }
    std::cerr << "unknown mode: " << mode << "\n";
    return 2;
}
