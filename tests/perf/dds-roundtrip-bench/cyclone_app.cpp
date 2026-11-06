// SPDX-License-Identifier: Apache-2.0
//
// Cyclone DDS C++ roundtrip bench app. Apples-to-apples with FastDDS+ZeroDDS.
//
// IDL: roundtrip.idl  →  RoundtripBench::Roundtrip  (via idlc -l cxx)
// With BENCH_RICH defined (target cyclone-roundtrip-rich) the sample type is
// the codec-heavy roundtrip_rich.idl → RoundtripBench::RoundtripRich, on the
// topics RoundtripRichBench_* that zerodds_app.cpp's rich build uses too.
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
#ifdef BENCH_RICH
#include "roundtrip_rich.hpp"
#else
#include "roundtrip.hpp"
#endif
#include "bench_common.hpp"

#include <array>
#include <chrono>
#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <string>
#include <thread>
#include <vector>

namespace {

using bench::now_ns;
using bench::PingState;
using bench::print_quantiles;

static uint32_t resolve_domain() {
    return static_cast<uint32_t>(bench::env_domain());
}
static dds::core::policy::DataRepresentationId bench_repr() {
    return bench::xcdr1_requested() ? dds::core::policy::DataRepresentationId::XCDR1
                                    : dds::core::policy::DataRepresentationId::XCDR2;
}

#ifdef BENCH_RICH
using Sample = RoundtripBench::RoundtripRich;
constexpr const char* kProgram   = "cyclone-app-rich";
constexpr const char* kReqTopic  = "RoundtripRichBench_Request";
constexpr const char* kEchoTopic = "RoundtripRichBench_Echo";
constexpr size_t kNumWaypoints = 8;

void populate(Sample& m, size_t payload_size) {
    using RoundtripBench::Vec3;
    using RoundtripBench::Waypoint;
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
#else
using Sample = RoundtripBench::Roundtrip;
constexpr const char* kProgram   = "cyclone-app";
constexpr const char* kReqTopic  = "RoundtripBench_Request";
constexpr const char* kEchoTopic = "RoundtripBench_Echo";

void populate(Sample& m, size_t payload_size) {
    m.payload(std::vector<uint8_t>(payload_size, 0xAB));
}
#endif

// Transport-Selection via env-var ZERODDS_BENCH_TRANSPORT
// → CYCLONEDDS_URI inline XML. Cyclone does not control transports via
// the QoS API but globally per participant through the XML config.
//
// CYCLONEDDS_URI must be set BEFORE a participant is created.
// We set it here at app start (idempotent — if the user explicitly
// set it, we ignore the transport choice).
void apply_transport_env() {
    const char* user_set = std::getenv("CYCLONEDDS_URI");
    if (user_set && *user_set) {
        return; // user-override respektieren
    }
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

// Writer and reader QoS of both sides: from bench::qos_from_env() (unset:
// reliable, KEEP_LAST 64, Cyclone's RESOURCE_LIMITS), one data
// representation offered and accepted.
//
// One representation on the writer side: RTI is strict-matching and rejects
// writers that advertise "XCDR1, XCDR2".
//
// ALLOW_TYPE_COERCION — cyclonedds-cxx defaults the reader to DISALLOW,
// which demands a byte-identical MINIMAL TypeObject hash from the writer.
// RTI's rtiddsgen derives a diverging MINIMAL TypeObject from the same IDL
// (the COMPLETE TypeObject is byte-identical across all five stacks,
// however). With ALLOW the match falls back to TypeLookup + assignability
// against the COMPLETE type -> cross-vendor match with RTI works (XTypes 1.3
// §7.6.3).
struct Endpoints {
    dds::domain::DomainParticipant   dp;
    dds::topic::Topic<Sample>        t_req;
    dds::topic::Topic<Sample>        t_echo;
    dds::pub::Publisher              pub;
    dds::sub::Subscriber             sub;
    dds::pub::qos::DataWriterQos     dw_qos;
    dds::sub::qos::DataReaderQos     dr_qos;

    Endpoints()
        : dp(resolve_domain()),
          t_req(dp, kReqTopic),
          t_echo(dp, kEchoTopic),
          pub(dp),
          sub(dp),
          dw_qos(pub.default_datawriter_qos()),
          dr_qos(sub.default_datareader_qos()),
          q(bench::qos_from_env()) {
        using namespace dds::core::policy;
        Reliability rel = q.reliable ? Reliability::Reliable() : Reliability::BestEffort();
        History hist = q.keep_all ? History::KeepAll() : History::KeepLast(q.depth);
        dw_qos << rel << hist;
        dr_qos << rel << hist;
        if (q.has_limits()) {
            ResourceLimits rl(q.max_samples, 1, q.max_samples);
            dw_qos << rl;
            dr_qos << rl;
        }
        dw_qos << DataRepresentation(DataRepresentationIdSeq{bench_repr()});
        dr_qos << DataRepresentation(DataRepresentationIdSeq{bench_repr()});
        dr_qos << TypeConsistencyEnforcement(TypeConsistencyKind::ALLOW_TYPE_COERCION);
    }

    bench::QosSpec q;
};

// --- Pong: receive request, echo immediately ---
//
// DataReaderListener (PSM-Cxx-Spec §7.5.6.7) feuert on_data_available
// in the recv thread. We take() the samples, write them to the
// Echo-Topic.

class PongListener : public dds::sub::NoOpDataReaderListener<Sample>
{
public:
    explicit PongListener(dds::pub::DataWriter<Sample>& w) : w_(w) {}

    void on_data_available(dds::sub::DataReader<Sample>& reader) override
    {
        auto samples = reader.take();
        for (const auto& s : samples) {
            if (s.info().valid()) {
                // Echo back unchanged — sequence_id + t_send_ns + payload.
                // A write refused within max_blocking_time (KEEP_ALL with
                // unacknowledged samples) is a lost echo, not the end.
                try {
                    w_.write(s.data());
                } catch (const dds::core::Exception&) {
                }
            }
        }
    }

private:
    dds::pub::DataWriter<Sample>& w_;
};

int run_pong(uint64_t max_runtime_s) {
    Endpoints e;
    dds::pub::DataWriter<Sample> dw(e.pub, e.t_echo, e.dw_qos);
    dds::sub::DataReader<Sample> dr(e.sub, e.t_req, e.dr_qos);

    PongListener listener(dw);
    dr.listener(&listener, dds::core::status::StatusMask::data_available());

    std::cout << "pong: matched, listener registered\n";
    std::this_thread::sleep_for(std::chrono::seconds(max_runtime_s));
    dr.listener(nullptr, dds::core::status::StatusMask::none());
    return 0;
}

// --- Ping: send + listener captures RTT ---

class PingListener : public dds::sub::NoOpDataReaderListener<Sample>
{
public:
    explicit PingListener(PingState& st) : st_(st) {}

    void on_data_available(dds::sub::DataReader<Sample>& reader) override
    {
        auto samples = reader.take();
        for (const auto& s : samples) {
            if (!s.info().valid()) continue;
            st_.record(s.data().t_send_ns());
        }
    }

private:
    PingState& st_;
};

int run_ping(size_t payload_size, uint64_t warmup, uint64_t samples) {
    Endpoints e;
    dds::pub::DataWriter<Sample> dw(e.pub, e.t_req, e.dw_qos);
    dds::sub::DataReader<Sample> dr(e.sub, e.t_echo, e.dr_qos);

    // Auf echtes Endpoint-Matching warten, NICHT bloss sleep(500ms).
    // With volatile durability (default), samples written BEFORE the
    // match are never delivered — the roundtrip partner
    // (pong) then does not see seq 0..N (startup window loss).
    // wait_for loop on publication_matched + subscription_matched.
    {
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        bool pub_matched = false, sub_matched = false;
        while (std::chrono::steady_clock::now() < deadline) {
            if (!pub_matched &&
                dw.publication_matched_status().current_count() >= 1) {
                pub_matched = true;
            }
            if (!sub_matched &&
                dr.subscription_matched_status().current_count() >= 1) {
                sub_matched = true;
            }
            if (pub_matched && sub_matched) break;
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
        if (!pub_matched || !sub_matched) {
            std::cerr << "ping: match timeout (pub=" << pub_matched
                      << " sub=" << sub_matched << ")\n";
        }
        // Short buffer after match: the reliable handshake (first
        // ACKNACK roundtrip) still needs a few ms until the
        // Writer-History garantiert delivered.
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }

    PingState st;
    st.warmup = warmup;
    st.rtts.reserve(samples);
    PingListener listener(st);
    dr.listener(&listener, dds::core::status::StatusMask::data_available());

    Sample msg;
    populate(msg, payload_size);

    uint64_t total = warmup + samples;
    uint64_t sent = 0;
    const uint64_t ping_deadline = bench::ping_deadline();
    for (uint64_t seq = 0; seq < total && !bench::ping_past(ping_deadline); ++seq, ++sent) {
        msg.sequence_id(static_cast<uint32_t>(seq));
        msg.t_send_ns(now_ns());
        try {
            dw.write(msg);
        } catch (const dds::core::Exception&) {
        }

        std::unique_lock<std::mutex> lk(st.mu);
        st.cv.wait_for(lk, std::chrono::milliseconds(50),
                       [&] { return st.received > seq; });
    }

    dr.listener(nullptr, dds::core::status::StatusMask::none());
    bench::print_ping_sent(sent);
    print_quantiles(st.rtts, payload_size);
    return 0;
}

// --- Throughput (tests/perf/speed): one writer, one reader, own processes ---

class TputListener : public dds::sub::NoOpDataReaderListener<Sample>
{
public:
    explicit TputListener(bench::TputSub& st) : st_(st) {}

    void on_data_available(dds::sub::DataReader<Sample>& reader) override
    {
        for (;;) {
            auto samples = reader.take();
            if (samples.length() == 0) break;
            for (const auto& s : samples) {
                if (s.info().valid()) {
                    st_.on_sample(s.data().sequence_id(), s.data().payload().size());
                }
            }
        }
    }

private:
    bench::TputSub& st_;
};

int run_tput_sub(uint64_t max_s) {
    Endpoints e;
    bench::print_qos("tput-sub", e.q);
    dds::sub::DataReader<Sample> dr(e.sub, e.t_req, e.dr_qos);
    bench::TputSub st;
    TputListener listener(st);
    dr.listener(&listener, dds::core::status::StatusMask::data_available());
    bench::tput_sub_wait_and_report(st, max_s);
    dr.listener(nullptr, dds::core::status::StatusMask::none());
    return 0;
}

int run_tput_pub(size_t payload_size, uint64_t seconds) {
    Endpoints e;
    bench::print_qos("tput-pub", e.q);
    dds::pub::DataWriter<Sample> dw(e.pub, e.t_req, e.dw_qos);
    bench::tput_wait_matched([&] { return dw.publication_matched_status().current_count() >= 1; });

    Sample msg;
    populate(msg, payload_size);
    auto r = bench::tput_pub_run(seconds, [&](uint32_t seq) {
        msg.sequence_id(seq);
        try {
            dw.write(msg);
            return true;
        } catch (const dds::core::Exception&) {
            return false;
        }
    });
    int acked = -1;
    uint64_t ack_ns = 0;
    if (e.q.reliable) {
        uint64_t t = now_ns();
        try {
            dw.wait_for_acknowledgments(dds::core::Duration::from_secs(30));
            acked = 1;
        } catch (const dds::core::Exception& ex) {
            std::cerr << "tput-pub: wait_for_acknowledgments: " << ex.what() << "\n";
            acked = 0;
        }
        ack_ns = now_ns() - t;
    }
    bench::print_tput_pub(r, payload_size, acked, ack_ns);
    bench::tput_linger();
    return 0;
}

}  // namespace

int main(int argc, char** argv) {
    if (argc < 2) {
        std::cerr <<
            "Usage:\n"
            "  " << kProgram << " pong [max_runtime_s]\n"
            "  " << kProgram << " ping --payload N [--samples N] [--warmup N]\n"
            "  " << kProgram << " tput-sub --seconds MAX\n"
            "  " << kProgram << " tput-pub --payload N --seconds S\n";
        return 2;
    }
    apply_transport_env();
    std::string mode = argv[1];
    if (mode == "pong") {
        uint64_t rt_s = (argc > 2) ? std::stoull(argv[2]) : 30;
        return run_pong(rt_s);
    } else if (mode == "ping") {
        bench::PingArgs a = bench::parse_ping_args(argc, argv, 2);
        return run_ping(a.payload, a.warmup, a.samples);
    } else if (mode == "tput-sub") {
        return run_tput_sub(bench::parse_tput_args(argc, argv, 2).seconds);
    } else if (mode == "tput-pub") {
        bench::TputArgs a = bench::parse_tput_args(argc, argv, 2);
        return run_tput_pub(a.payload, a.seconds);
    }
    std::cerr << "unknown mode: " << mode << "\n";
    return 2;
}
