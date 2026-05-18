// SPDX-License-Identifier: Apache-2.0
//
// Cyclone DDS C++ Roundtrip-Bench-App. Apples-to-apples mit FastDDS+ZeroDDS.
//
// IDL: roundtrip.idl  →  RoundtripBench::Roundtrip  (via idlc -l cxx)
//
// Pipeline-Schichten:
//   App-Code (C++)                     -- typed Roundtrip-Class
//   ↓
//   ddsidlc-Codegen + TopicTraits     -- CDR-Encode/Decode auto-generated
//   ↓
//   isocpp DDS-PSM-Cxx (dds/dds.hpp)   -- DataWriter<T>/DataReader<T>
//   ↓
//   libddscxx -> libddsc               -- C++ Wrapper -> C Core
//   ↓
//   DDSI-RTPS 2.5                      -- Wire-Protokoll
//   ↓
//   UDP-Loopback                       -- Transport
//
// Build:
//   g++ -std=c++17 -O2 -Wall \
//       -I ~/cyclone-cxx-install/include \
//       -I ~/cyclone-cxx-install/include/ddscxx \
//       -L ~/cyclone-cxx-install/lib -L /usr/lib/x86_64-linux-gnu \
//       -o cyclone-app cyclone_app.cpp roundtrip.cpp \
//       -lddscxx -lddsc -lpthread

#include "dds/dds.hpp"
#include "roundtrip.hpp"

#include <algorithm>
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

constexpr uint32_t kDomain = 200;
constexpr const char* kReqTopic  = "RoundtripBench_Request";
constexpr const char* kEchoTopic = "RoundtripBench_Echo";

uint64_t now_ns() {
    auto t = std::chrono::steady_clock::now().time_since_epoch();
    return std::chrono::duration_cast<std::chrono::nanoseconds>(t).count();
}

// --- Pong: receive request, echo immediately ---
//
// DataReaderListener (PSM-Cxx-Spec §7.5.6.7) feuert on_data_available
// im Recv-Thread. Wir take()-en die Samples, schreiben sie auf das
// Echo-Topic. Best-Effort.

class PongListener : public dds::sub::NoOpDataReaderListener<RoundtripBench::Roundtrip>
{
public:
    explicit PongListener(dds::pub::DataWriter<RoundtripBench::Roundtrip>& w) : w_(w) {}

    void on_data_available(dds::sub::DataReader<RoundtripBench::Roundtrip>& reader) override
    {
        auto samples = reader.take();
        for (const auto& s : samples) {
            if (s.info().valid()) {
                // Echo unverändert zurück — sequence_id + t_send_ns + payload.
                w_.write(s.data());
            }
        }
    }

private:
    dds::pub::DataWriter<RoundtripBench::Roundtrip>& w_;
};

int run_pong(uint64_t max_runtime_s) {
    using namespace dds;

    domain::DomainParticipant dp(kDomain);
    topic::Topic<RoundtripBench::Roundtrip> t_req(dp, kReqTopic);
    topic::Topic<RoundtripBench::Roundtrip> t_echo(dp, kEchoTopic);
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    pub::qos::DataWriterQos dw_qos = pub_.default_datawriter_qos();
    dw_qos << core::policy::Reliability::Reliable();
    dw_qos << core::policy::History::KeepLast(64);
    sub::qos::DataReaderQos dr_qos = sub_.default_datareader_qos();
    dr_qos << core::policy::Reliability::Reliable();
    dr_qos << core::policy::History::KeepLast(64);
    pub::DataWriter<RoundtripBench::Roundtrip> dw(pub_, t_echo, dw_qos);
    sub::DataReader<RoundtripBench::Roundtrip> dr(sub_, t_req, dr_qos);

    PongListener listener(dw);
    dr.listener(&listener, dds::core::status::StatusMask::data_available());

    std::cout << "pong: matched, listener registered\n";
    std::this_thread::sleep_for(std::chrono::seconds(max_runtime_s));
    dr.listener(nullptr, dds::core::status::StatusMask::none());
    return 0;
}

// --- Ping: send + listener captures RTT ---

struct PingState {
    std::mutex                  mu;
    std::condition_variable     cv;
    uint64_t                    received    = 0;
    uint64_t                    warmup      = 0;
    std::vector<uint64_t>       rtts;
};

class PingListener : public dds::sub::NoOpDataReaderListener<RoundtripBench::Roundtrip>
{
public:
    explicit PingListener(PingState& st) : st_(st) {}

    void on_data_available(dds::sub::DataReader<RoundtripBench::Roundtrip>& reader) override
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

    domain::DomainParticipant dp(kDomain);
    topic::Topic<RoundtripBench::Roundtrip> t_req(dp, kReqTopic);
    topic::Topic<RoundtripBench::Roundtrip> t_echo(dp, kEchoTopic);
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    pub::qos::DataWriterQos dw_qos = pub_.default_datawriter_qos();
    dw_qos << core::policy::Reliability::Reliable();
    dw_qos << core::policy::History::KeepLast(64);
    sub::qos::DataReaderQos dr_qos = sub_.default_datareader_qos();
    dr_qos << core::policy::Reliability::Reliable();
    dr_qos << core::policy::History::KeepLast(64);
    pub::DataWriter<RoundtripBench::Roundtrip> dw(pub_, t_req, dw_qos);
    sub::DataReader<RoundtripBench::Roundtrip> dr(sub_, t_echo, dr_qos);

    // Brief stabilization for matching.
    std::this_thread::sleep_for(std::chrono::milliseconds(500));

    PingState st;
    st.warmup = warmup;
    st.rtts.reserve(samples);
    PingListener listener(st);
    dr.listener(&listener, dds::core::status::StatusMask::data_available());

    RoundtripBench::Roundtrip msg;
    std::vector<uint8_t> payload(payload_size, 0xAB);
    msg.payload(payload);

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
            "  cyclone-app pong [max_runtime_s]\n"
            "  cyclone-app ping --payload N [--samples N] [--warmup N]\n";
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
