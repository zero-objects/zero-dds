// SPDX-License-Identifier: Apache-2.0
//
// FastDDS C++ Roundtrip-Bench-App — RICH-TYPED variant.
//
// IDL: roundtrip_rich.idl  →  RoundtripBench::RoundtripRich  (via fastddsgen)
//
// Mechanisch identisch zu fastdds_app.cpp, aber der codec-schwere
// RoundtripRich. Topics=RoundtripRichBench_* (match zerodds/cyclone rich).

#include "roundtrip_rich.hpp"
#include "roundtrip_richPubSubTypes.hpp"

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

#include <algorithm>
#include <array>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

using namespace eprosima::fastdds::dds;

namespace {

using RoundtripBench::RoundtripRich;
using RoundtripBench::Vec3;
using RoundtripBench::Waypoint;

constexpr size_t kNumWaypoints = 8;

static uint32_t resolve_domain() {
    const char* s = std::getenv("ZERODDS_BENCH_DOMAIN");
    if (s) { try { return static_cast<uint32_t>(std::stoul(s)); } catch (...) {} }
    return 200;
}
constexpr const char* kReqTopic  = "RoundtripRichBench_Request";
constexpr const char* kEchoTopic = "RoundtripRichBench_Echo";

uint64_t now_ns() {
    auto t = std::chrono::steady_clock::now().time_since_epoch();
    return std::chrono::duration_cast<std::chrono::nanoseconds>(t).count();
}

void populate_rich(RoundtripRich& m, size_t payload_size) {
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

void apply_transport(DomainParticipantQos& pqos) {
    pqos.transport().use_builtin_transports = false;
    auto t = std::make_shared<eprosima::fastdds::rtps::UDPv4TransportDescriptor>();
    pqos.transport().user_transports.push_back(t);
}

void apply_security(DomainParticipantQos& pqos) {
    const char* sec = std::getenv("ZERODDS_BENCH_SECURITY");
    if (!sec || std::string(sec) != "1") return;
    const char* sec_dir = std::getenv("ZERODDS_BENCH_SEC_DIR");
    if (!sec_dir) sec_dir = "/tmp/dds-bench-security";
    const char* who = std::getenv("ZERODDS_BENCH_SEC_NAME");
    if (!who) who = "ping";
    auto& props = pqos.properties().properties();
    auto add = [&](const std::string& k, const std::string& v) { props.emplace_back(k, v); };
    add("dds.sec.auth.plugin", "builtin.PKI-DH");
    add("dds.sec.auth.builtin.PKI-DH.identity_ca", std::string("file://") + sec_dir + "/certs/identity_ca.pem");
    add("dds.sec.auth.builtin.PKI-DH.identity_certificate", std::string("file://") + sec_dir + "/certs/" + who + "_cert.pem");
    add("dds.sec.auth.builtin.PKI-DH.private_key", std::string("file://") + sec_dir + "/certs/" + who + "_key.pem");
    add("dds.sec.access.plugin", "builtin.Access-Permissions");
    add("dds.sec.access.builtin.Access-Permissions.permissions_ca", std::string("file://") + sec_dir + "/certs/permissions_ca.pem");
    add("dds.sec.access.builtin.Access-Permissions.governance", std::string("file://") + sec_dir + "/governance.p7s");
    add("dds.sec.access.builtin.Access-Permissions.permissions", std::string("file://") + sec_dir + "/permissions_" + who + ".p7s");
    add("dds.sec.crypto.plugin", "builtin.AES-GCM-GMAC");
}

class PongListener : public DataReaderListener {
public:
    explicit PongListener(DataWriter* writer) : writer_(writer) {}
    void on_data_available(DataReader* reader) override {
        RoundtripRich msg;
        SampleInfo info;
        while (reader->take_next_sample(&msg, &info) == RETCODE_OK) {
            if (info.valid_data) writer_->write(&msg);
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
    TypeSupport type_support(new RoundtripBench::RoundtripRichPubSubType());
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
    dw_qos.representation().m_value.push_back(XCDR2_DATA_REPRESENTATION);
    auto* writer = publisher->create_datawriter(topic_echo, dw_qos);
    DataReaderQos dr_qos;
    dr_qos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    dr_qos.history().kind = KEEP_LAST_HISTORY_QOS;
    dr_qos.history().depth = 64;
    dr_qos.representation().m_value.clear();
    dr_qos.representation().m_value.push_back(XCDR2_DATA_REPRESENTATION);
    PongListener listener(writer);
    auto* reader = subscriber->create_datareader(topic_req, dr_qos, &listener);
    std::cout << "pong[fastdds]: matched, listener registered (rich)\n" << std::flush;
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

struct PingState {
    std::mutex                  mu;
    std::condition_variable     cv;
    uint64_t                    received    = 0;
    uint64_t                    warmup      = 0;
    std::vector<uint64_t>       rtts;
};

class PingListener : public DataReaderListener {
public:
    explicit PingListener(PingState& st) : st_(st) {}
    void on_data_available(DataReader* reader) override {
        RoundtripRich msg;
        SampleInfo info;
        while (reader->take_next_sample(&msg, &info) == RETCODE_OK) {
            if (!info.valid_data) continue;
            uint64_t now = now_ns();
            uint64_t rtt = now > msg.t_send_ns() ? now - msg.t_send_ns() : 1;
            {
                std::lock_guard<std::mutex> lk(st_.mu);
                if (st_.received >= st_.warmup) st_.rtts.push_back(rtt);
                st_.received++;
            }
            st_.cv.notify_all();
        }
    }
private:
    PingState& st_;
};

void print_quantiles(std::vector<uint64_t>& rtts, size_t payload_size) {
    if (rtts.empty()) { std::cout << "no samples\n"; return; }
    std::sort(rtts.begin(), rtts.end());
    auto pct = [&](double p) {
        size_t idx = std::min(rtts.size() - 1,
                              static_cast<size_t>(p * (double)(rtts.size() - 1)));
        return rtts[idx];
    };
    std::cout
        << "payload=" << payload_size
        << "  n=" << rtts.size()
        << "  min=" << rtts.front()/1000.0 << "us"
        << "  p50=" << pct(0.50)/1000.0 << "us"
        << "  p90=" << pct(0.90)/1000.0 << "us"
        << "  p99=" << pct(0.99)/1000.0 << "us"
        << "  p999=" << pct(0.999)/1000.0 << "us"
        << "  max=" << rtts.back()/1000.0 << "us\n";
}

int run_ping(size_t payload_size, uint64_t warmup, uint64_t samples) {
    DomainParticipantQos pqos;
    apply_transport(pqos);
    apply_security(pqos);
    auto* dp = DomainParticipantFactory::get_instance()->create_participant(resolve_domain(), pqos);
    if (!dp) { std::cerr << "dp create failed\n"; return 1; }
    TypeSupport type_support(new RoundtripBench::RoundtripRichPubSubType());
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
    dw_qos.representation().m_value.push_back(XCDR2_DATA_REPRESENTATION);
    auto* writer = publisher->create_datawriter(topic_req, dw_qos);
    DataReaderQos dr_qos;
    dr_qos.reliability().kind = RELIABLE_RELIABILITY_QOS;
    dr_qos.history().kind = KEEP_LAST_HISTORY_QOS;
    dr_qos.history().depth = 64;
    dr_qos.representation().m_value.clear();
    dr_qos.representation().m_value.push_back(XCDR2_DATA_REPRESENTATION);
    PingState st;
    st.warmup = warmup;
    st.rtts.reserve(samples);
    PingListener listener(st);
    auto* reader = subscriber->create_datareader(topic_echo, dr_qos, &listener);
    {
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        while (std::chrono::steady_clock::now() < deadline) {
            PublicationMatchedStatus pm;
            SubscriptionMatchedStatus sm;
            writer->get_publication_matched_status(pm);
            reader->get_subscription_matched_status(sm);
            if (pm.current_count >= 1 && sm.current_count >= 1) break;
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }
    RoundtripRich msg;
    populate_rich(msg, payload_size);
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
            "  fastdds-app-rich pong [max_runtime_s]\n"
            "  fastdds-app-rich ping --payload N [--samples N] [--warmup N]\n";
        return 2;
    }
    std::string mode = argv[1];
    if (mode == "pong") {
        uint64_t rt_s = (argc > 2) ? std::stoull(argv[2]) : 30;
        return run_pong(rt_s);
    } else if (mode == "ping") {
        size_t   payload = 64;
        uint64_t warmup  = 200;
        uint64_t samples = 5000;
        for (int i = 2; i + 1 < argc; i += 2) {
            std::string flag = argv[i];
            uint64_t   v    = std::stoull(argv[i+1]);
            if      (flag == "--payload") payload = static_cast<size_t>(v);
            else if (flag == "--samples") samples = v;
            else if (flag == "--warmup")  warmup  = v;
        }
        return run_ping(payload, warmup, samples);
    }
    std::cerr << "unknown mode: " << mode << "\n";
    return 2;
}
