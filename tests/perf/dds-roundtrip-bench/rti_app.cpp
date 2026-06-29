// SPDX-License-Identifier: Apache-2.0
//
// RTI Connext C++11 Roundtrip-Bench-App. Apples-to-apples mit
// Cyclone+FastDDS+ZeroDDS. RTI Connext Express (post-60-day-eval, free).
//
// IDL: roundtrip.idl  →  RoundtripBench::Roundtrip  (via rtiddsgen)
//
// Build:
//   source /opt/rti.com/rti_connext_dds-7.7.0/resource/scripts/rtisetenv_x64Linux4gcc8.5.0.bash
//   g++ -std=c++17 -O2 -Wall \
//       -I $NDDSHOME/include -I $NDDSHOME/include/ndds -I $NDDSHOME/include/ndds/hpp \
//       -L $NDDSHOME/lib/x64Linux4gcc8.5.0 \
//       -DRTI_UNIX -DRTI_LINUX -DRTI_64BIT \
//       -o rti-app rti_app.cpp roundtrip.cxx roundtripPlugin.cxx \
//       -lnddscpp2 -lnddscpp -lnddsc -lnddscore -lpthread -ldl -lm -lrt

#include "roundtrip.hpp"
#include <dds/dds.hpp>

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

// Domain via ZERODDS_BENCH_DOMAIN env-var ueberschreibbar (Default 200).
static int32_t resolve_domain() {
    const char* s = std::getenv("ZERODDS_BENCH_DOMAIN");
    if (s) { try { return static_cast<int32_t>(std::stoi(s)); } catch (...) {} }
    return 200;
}
constexpr const char* kReqTopic  = "RoundtripBench_Request";
constexpr const char* kEchoTopic = "RoundtripBench_Echo";

// Coherent-spec QoS — matches Cyclone+FastDDS+ZeroDDS.
constexpr int32_t kHistoryDepth = 64;
constexpr uint32_t kWarmup  = 200;
constexpr uint32_t kSamples = 5000;

uint64_t now_ns() {
    auto t = std::chrono::steady_clock::now().time_since_epoch();
    return std::chrono::duration_cast<std::chrono::nanoseconds>(t).count();
}

// Transport-Selection via env-var ZERODDS_BENCH_TRANSPORT:
//   UDPv4 (default), UDPv6, SHM.
// TCPv4 fuer RTI braucht eine TCP-Transport-Plugin-Konfiguration via
// PropertyQosPolicy (dds.transport.TCPv4.tcp1.*), nicht via builtin-mask
// -> separat zu implementieren wenn priorisiert.
dds::domain::qos::DomainParticipantQos make_dp_qos() {
    using namespace rti::core::policy;
    dds::domain::qos::DomainParticipantQos qos;
    const char* t = std::getenv("ZERODDS_BENCH_TRANSPORT");
    std::string ts(t ? t : "UDPv4");
    TransportBuiltinMask mask = TransportBuiltinMask::udpv4();
    if (ts == "UDPv6") mask = TransportBuiltinMask::udpv6();
    else if (ts == "SHM") mask = TransportBuiltinMask::shmem();
    qos << TransportBuiltin(mask);

    // DDS-Security 1.2 via Property-QoS (RTI `com.rti.serv.secure`-Plugin).
    // ZERODDS_BENCH_SECURITY=1 + ZERODDS_BENCH_SEC_NAME={ping,pong} aktiv.
    const char* sec = std::getenv("ZERODDS_BENCH_SECURITY");
    if (sec && std::string(sec) == "1") {
        const char* sec_dir = std::getenv("ZERODDS_BENCH_SEC_DIR");
        if (!sec_dir) sec_dir = "/tmp/dds-bench-security";
        const char* who = std::getenv("ZERODDS_BENCH_SEC_NAME");
        if (!who) who = "ping";
        std::string base(sec_dir);
        auto& props = qos.policy<rti::core::policy::Property>();
        auto add = [&](const std::string& k, const std::string& v) {
            props.set(rti::core::policy::Property::Entry(k, v));
        };
        add("com.rti.serv.load_plugin", "com.rti.serv.secure");
        add("com.rti.serv.secure.create_function",
            "RTI_Security_PluginSuite_create");
        add("com.rti.serv.secure.library", "nddssecurity");
        add("com.rti.serv.secure.authentication.ca_file",
            base + "/certs/identity_ca.pem");
        add("com.rti.serv.secure.authentication.certificate_file",
            base + "/certs/" + who + "_cert.pem");
        add("com.rti.serv.secure.authentication.private_key_file",
            base + "/certs/" + who + "_key.pem");
        add("com.rti.serv.secure.access_control.permissions_authority_file",
            base + "/certs/permissions_ca.pem");
        add("com.rti.serv.secure.access_control.governance_file",
            base + "/governance.p7s");
        add("com.rti.serv.secure.access_control.permissions_file",
            base + "/permissions_" + who + ".p7s");
    }
    return qos;
}

// Cross-Vendor-Helper: DataRepresentation auf XCDR2 setzen.
//
// RTI default = XCDR1 (legacy). Alle anderen Vendoren (Cyclone,
// FastDDS, ZeroDDS, OpenDDS) defaulten auf XCDR2. Damit der Match
// trotz `@final`-Type funktioniert, RTI auf XCDR2-only setzen.
//
// XCDR1+XCDR2 simultan (was permissiver wäre) crasht RTI DW-create
// auf @final Structs — vermutlich braucht der dual-rep-buffer eine
// extra-extensibility (APPENDABLE/MUTABLE).
dds::core::policy::DataRepresentation xcdr2_only() {
    dds::core::policy::DataRepresentationIdSeq seq;
    seq.push_back(dds::core::policy::DataRepresentation::xcdr2());
    return dds::core::policy::DataRepresentation(seq);
}

dds::pub::qos::DataWriterQos make_dw_qos() {
    using namespace dds::core::policy;
    dds::pub::qos::DataWriterQos qos;
    qos << Reliability::Reliable();
    qos << History::KeepLast(kHistoryDepth);
    // Cross-Vendor: RTI default ist XCDR1; Cyclone/FastDDS/ZeroDDS
    // default ist XCDR2. Wire-Match braucht XCDR2 auf RTI-Seite.
    qos << xcdr2_only();
    return qos;
}

dds::sub::qos::DataReaderQos make_dr_qos() {
    using namespace dds::core::policy;
    dds::sub::qos::DataReaderQos qos;
    qos << Reliability::Reliable();
    qos << History::KeepLast(kHistoryDepth);
    // Reader-Seite: RTI muss XCDR2 anbieten (analog Writer) UND
    // ALLOW_TYPE_COERCION damit type-hash-Differenzen zwischen
    // Vendoren toleriert werden (RTI default = EXACT_TYPE,
    // andere Vendoren default = ALLOW_TYPE_COERCION).
    qos << xcdr2_only();
    qos << TypeConsistencyEnforcement(TypeConsistencyEnforcementKind::ALLOW_TYPE_COERCION);
    return qos;
}

// --- Pong: receive request, echo immediately ---

class PongListener : public dds::sub::NoOpDataReaderListener<RoundtripBench::Roundtrip>
{
public:
    explicit PongListener(dds::pub::DataWriter<RoundtripBench::Roundtrip>& w) : w_(w) {}

    void on_data_available(dds::sub::DataReader<RoundtripBench::Roundtrip>& reader) override
    {
        auto samples = reader.take();
        for (const auto& s : samples) {
            if (s.info().valid()) {
                w_.write(s.data());
            }
        }
    }

private:
    dds::pub::DataWriter<RoundtripBench::Roundtrip>& w_;
};

int run_pong(uint64_t max_runtime_s) {
    using namespace dds;

    domain::DomainParticipant dp(resolve_domain(), make_dp_qos());
    topic::Topic<RoundtripBench::Roundtrip> t_req(dp, kReqTopic);
    topic::Topic<RoundtripBench::Roundtrip> t_echo(dp, kEchoTopic);
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    pub::DataWriter<RoundtripBench::Roundtrip> dw(pub_, t_echo, make_dw_qos());
    sub::DataReader<RoundtripBench::Roundtrip> dr(sub_, t_req, make_dr_qos());

    PongListener listener(dw);
    dr.listener(&listener, dds::core::status::StatusMask::data_available());

    std::cout << "pong[rti]: matched, listener registered\n";
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
            uint64_t t_send = s.data().t_send_ns;
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

    domain::DomainParticipant dp(resolve_domain(), make_dp_qos());
    topic::Topic<RoundtripBench::Roundtrip> t_req(dp, kReqTopic);
    topic::Topic<RoundtripBench::Roundtrip> t_echo(dp, kEchoTopic);
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    pub::DataWriter<RoundtripBench::Roundtrip> dw(pub_, t_req, make_dw_qos());
    sub::DataReader<RoundtripBench::Roundtrip> dr(sub_, t_echo, make_dr_qos());

    PingState st;
    st.warmup = warmup;
    st.rtts.reserve(samples);
    PingListener listener(st);
    dr.listener(&listener, dds::core::status::StatusMask::data_available());

    // Discovery + Matching settle.
    std::this_thread::sleep_for(std::chrono::milliseconds(2000));

    RoundtripBench::Roundtrip msg;
    // RTI 7.7 C++11-Codegen: Member sind public POD-Felder, der
    // payload ist eine rti::core::bounded_sequence (container-like).
    msg.payload.resize(payload_size);
    std::fill(msg.payload.begin(), msg.payload.end(), 0xAB);

    uint64_t total = warmup + samples;
    for (uint64_t seq = 0; seq < total; ++seq) {
        msg.sequence_id = static_cast<uint32_t>(seq);
        msg.t_send_ns = now_ns();
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
            "  rti-app pong [max_runtime_s]\n"
            "  rti-app ping --payload N [--samples N] [--warmup N]\n";
        return 2;
    }
    // RTI-Logger auf WARNING — sonst schluckt die modern-C++-PSM den
    // echten Grund hinter dem generischen "Failed to create
    // DomainParticipant".
    rti::config::Logger::instance().verbosity(
        rti::config::Verbosity::WARNING);
    std::string mode = argv[1];
    if (mode == "pong") {
        uint64_t rt_s = (argc > 2) ? std::stoull(argv[2]) : 30;
        return run_pong(rt_s);
    } else if (mode == "ping") {
        size_t   payload = 64;
        uint64_t warmup  = kWarmup;
        uint64_t samples = kSamples;
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
