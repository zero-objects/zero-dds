// SPDX-License-Identifier: Apache-2.0
//
// Cyclone DDS C++ Roundtrip-Bench-App — RICH-TYPED variant.
//
// IDL: roundtrip_rich.idl  →  RoundtripBench::RoundtripRich  (via idlc -l cxx)
//
// Mechanisch identisch zu cyclone_app.cpp, aber der Sample-Typ ist der
// codec-schwere RoundtripRich. Topic-Namen = RoundtripRichBench_* (match
// zerodds_app_rich.cpp) für cross-vendor rich-Interop.

#include "dds/dds.hpp"
#include "roundtrip_rich.hpp"

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
    m.name(std::string("cyclone-rich-roundtrip-bench"));
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

void apply_transport_env() {
    const char* user_set = std::getenv("CYCLONEDDS_URI");
    if (user_set && *user_set) return;
    const char* t = std::getenv("ZERODDS_BENCH_TRANSPORT");
    if (!t) t = "UDPv4";
    std::string xml;
    if (std::string(t) == "SHM") {
        xml = R"(<CycloneDDS><Domain><SharedMemory><Enable>true</Enable></SharedMemory></Domain></CycloneDDS>)";
    } else if (std::string(t) == "UDPv6") {
        xml = R"(<CycloneDDS><Domain><General><Transport>udp6</Transport></General></Domain></CycloneDDS>)";
    } else if (std::string(t) == "TCPv4") {
        xml = R"(<CycloneDDS><Domain><General><Transport>tcp</Transport></General></Domain></CycloneDDS>)";
    } else {
        xml = R"(<CycloneDDS><Domain><General><Transport>udp</Transport></General></Domain></CycloneDDS>)";
    }
    setenv("CYCLONEDDS_URI", xml.c_str(), 1);
}

class PongListener : public dds::sub::NoOpDataReaderListener<RoundtripRich>
{
public:
    explicit PongListener(dds::pub::DataWriter<RoundtripRich>& w) : w_(w) {}
    void on_data_available(dds::sub::DataReader<RoundtripRich>& reader) override
    {
        auto samples = reader.take();
        for (const auto& s : samples) {
            if (s.info().valid()) w_.write(s.data());
        }
    }
private:
    dds::pub::DataWriter<RoundtripRich>& w_;
};

int run_pong(uint64_t max_runtime_s) {
    using namespace dds;
    domain::DomainParticipant dp(resolve_domain());
    topic::Topic<RoundtripRich> t_req(dp, kReqTopic);
    topic::Topic<RoundtripRich> t_echo(dp, kEchoTopic);
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    pub::qos::DataWriterQos dw_qos = pub_.default_datawriter_qos();
    dw_qos << core::policy::Reliability::Reliable();
    dw_qos << core::policy::History::KeepLast(64);
    dw_qos << core::policy::DataRepresentation(
        core::policy::DataRepresentationIdSeq{
            core::policy::DataRepresentationId::XCDR2});
    sub::qos::DataReaderQos dr_qos = sub_.default_datareader_qos();
    dr_qos << core::policy::Reliability::Reliable();
    dr_qos << core::policy::History::KeepLast(64);
    dr_qos << core::policy::DataRepresentation(
        core::policy::DataRepresentationIdSeq{
            core::policy::DataRepresentationId::XCDR2});
    dr_qos << core::policy::TypeConsistencyEnforcement(
        core::policy::TypeConsistencyKind::ALLOW_TYPE_COERCION);
    pub::DataWriter<RoundtripRich> dw(pub_, t_echo, dw_qos);
    sub::DataReader<RoundtripRich> dr(sub_, t_req, dr_qos);
    PongListener listener(dw);
    dr.listener(&listener, dds::core::status::StatusMask::data_available());
    std::cout << "pong: matched, listener registered (rich)\n";
    std::this_thread::sleep_for(std::chrono::seconds(max_runtime_s));
    dr.listener(nullptr, dds::core::status::StatusMask::none());
    return 0;
}

struct PingState {
    std::mutex                  mu;
    std::condition_variable     cv;
    uint64_t                    received    = 0;
    uint64_t                    warmup      = 0;
    std::vector<uint64_t>       rtts;
};

class PingListener : public dds::sub::NoOpDataReaderListener<RoundtripRich>
{
public:
    explicit PingListener(PingState& st) : st_(st) {}
    void on_data_available(dds::sub::DataReader<RoundtripRich>& reader) override
    {
        auto samples = reader.take();
        for (const auto& s : samples) {
            if (!s.info().valid()) continue;
            uint64_t now = now_ns();
            uint64_t t_send = s.data().t_send_ns();
            uint64_t rtt = now > t_send ? now - t_send : 1;
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
    using namespace dds;
    domain::DomainParticipant dp(resolve_domain());
    topic::Topic<RoundtripRich> t_req(dp, kReqTopic);
    topic::Topic<RoundtripRich> t_echo(dp, kEchoTopic);
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    pub::qos::DataWriterQos dw_qos = pub_.default_datawriter_qos();
    dw_qos << core::policy::Reliability::Reliable();
    dw_qos << core::policy::History::KeepLast(64);
    dw_qos << core::policy::DataRepresentation(
        core::policy::DataRepresentationIdSeq{
            core::policy::DataRepresentationId::XCDR2});
    sub::qos::DataReaderQos dr_qos = sub_.default_datareader_qos();
    dr_qos << core::policy::Reliability::Reliable();
    dr_qos << core::policy::History::KeepLast(64);
    dr_qos << core::policy::DataRepresentation(
        core::policy::DataRepresentationIdSeq{
            core::policy::DataRepresentationId::XCDR2});
    dr_qos << core::policy::TypeConsistencyEnforcement(
        core::policy::TypeConsistencyKind::ALLOW_TYPE_COERCION);
    pub::DataWriter<RoundtripRich> dw(pub_, t_req, dw_qos);
    sub::DataReader<RoundtripRich> dr(sub_, t_echo, dr_qos);
    {
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        bool pub_matched = false, sub_matched = false;
        while (std::chrono::steady_clock::now() < deadline) {
            if (!pub_matched && dw.publication_matched_status().current_count() >= 1) pub_matched = true;
            if (!sub_matched && dr.subscription_matched_status().current_count() >= 1) sub_matched = true;
            if (pub_matched && sub_matched) break;
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
        if (!pub_matched || !sub_matched) {
            std::cerr << "ping: match timeout (pub=" << pub_matched << " sub=" << sub_matched << ")\n";
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }
    PingState st;
    st.warmup = warmup;
    st.rtts.reserve(samples);
    PingListener listener(st);
    dr.listener(&listener, dds::core::status::StatusMask::data_available());

    RoundtripRich msg;
    populate_rich(msg, payload_size);

    uint64_t total = warmup + samples;
    for (uint64_t seq = 0; seq < total; ++seq) {
        msg.sequence_id(static_cast<uint32_t>(seq));
        msg.t_send_ns(now_ns());
        dw.write(msg);
        std::unique_lock<std::mutex> lk(st.mu);
        st.cv.wait_for(lk, std::chrono::milliseconds(50),
                       [&] { return st.received > seq; });
    }
    dr.listener(nullptr, dds::core::status::StatusMask::none());
    print_quantiles(st.rtts, payload_size);
    return 0;
}

}  // namespace

int main(int argc, char** argv) {
    if (argc < 2) {
        std::cerr <<
            "Usage:\n"
            "  cyclone-app-rich pong [max_runtime_s]\n"
            "  cyclone-app-rich ping --payload N [--samples N] [--warmup N]\n";
        return 2;
    }
    apply_transport_env();
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
