// SPDX-License-Identifier: Apache-2.0
//
// OpenSplice Community Edition 6.9 roundtrip bench app, classic C++ API
// (SACPP). Apples-to-apples with the Cyclone/Fast-DDS/OpenDDS/ZeroDDS apps:
// the same topics, the same QoS (RELIABLE, KEEP_LAST 64), the same
// event-driven pong (a DataReaderListener echoes from on_data_available)
// and the same ping measurement and output line.
//
// IDL: roundtrip_ospl.idl -> RoundtripBench::Roundtrip (idlpp -S -l cpp),
// roundtrip.idl without annotations. OpenSplice writes and reads XCDR1
// only, so the peers of this app run with ZERODDS_BENCH_REPR=XCDR1.
//
// The domain comes from the configuration file the wrapper
// opensplice-roundtrip (opensplice_roundtrip.sh.in) writes.

#include "ccpp_dds_dcps.h"
#include "ccpp_roundtrip_ospl.h"

#include <algorithm>
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

constexpr const char* kReqTopic  = "RoundtripBench_Request";
constexpr const char* kEchoTopic = "RoundtripBench_Echo";

uint64_t now_ns() {
    auto t = std::chrono::steady_clock::now().time_since_epoch();
    return std::chrono::duration_cast<std::chrono::nanoseconds>(t).count();
}

// A DataReaderListener with empty bodies for everything but data.
class DataListener : public virtual DDS::DataReaderListener {
public:
    void on_requested_deadline_missed(DDS::DataReader_ptr,
                                      const DDS::RequestedDeadlineMissedStatus&) override {}
    void on_requested_incompatible_qos(DDS::DataReader_ptr,
                                       const DDS::RequestedIncompatibleQosStatus&) override {}
    void on_sample_rejected(DDS::DataReader_ptr, const DDS::SampleRejectedStatus&) override {}
    void on_liveliness_changed(DDS::DataReader_ptr, const DDS::LivelinessChangedStatus&) override {}
    void on_subscription_matched(DDS::DataReader_ptr,
                                 const DDS::SubscriptionMatchedStatus&) override {}
    void on_sample_lost(DDS::DataReader_ptr, const DDS::SampleLostStatus&) override {}
};

struct Endpoints {
    DDS::DomainParticipantFactory_var dpf;
    DDS::DomainParticipant_var dp;
    DDS::Topic_var t_req, t_echo;
    DDS::Publisher_var pub;
    DDS::Subscriber_var sub;
    DDS::DataWriterQos dw_qos;
    DDS::DataReaderQos dr_qos;
};

bool setup(Endpoints& e) {
    e.dpf = DDS::DomainParticipantFactory::get_instance();
    e.dp = e.dpf->create_participant(DDS::DOMAIN_ID_DEFAULT, PARTICIPANT_QOS_DEFAULT, NULL,
                                     DDS::STATUS_MASK_NONE);
    if (!e.dp.in()) { std::cerr << "opensplice-app: no participant (OSPL_URI?)\n"; return false; }
    RoundtripBench::RoundtripTypeSupport_var ts = new RoundtripBench::RoundtripTypeSupport;
    CORBA::String_var type_name = ts->get_type_name();
    if (ts->register_type(e.dp.in(), type_name.in()) != DDS::RETCODE_OK) {
        std::cerr << "opensplice-app: register_type failed\n";
        return false;
    }
    e.t_req = e.dp->create_topic(kReqTopic, type_name.in(), TOPIC_QOS_DEFAULT, NULL,
                                 DDS::STATUS_MASK_NONE);
    e.t_echo = e.dp->create_topic(kEchoTopic, type_name.in(), TOPIC_QOS_DEFAULT, NULL,
                                  DDS::STATUS_MASK_NONE);
    e.pub = e.dp->create_publisher(PUBLISHER_QOS_DEFAULT, NULL, DDS::STATUS_MASK_NONE);
    e.sub = e.dp->create_subscriber(SUBSCRIBER_QOS_DEFAULT, NULL, DDS::STATUS_MASK_NONE);
    if (!e.t_req.in() || !e.t_echo.in() || !e.pub.in() || !e.sub.in()) {
        std::cerr << "opensplice-app: topic/publisher/subscriber creation failed\n";
        return false;
    }
    e.pub->get_default_datawriter_qos(e.dw_qos);
    e.dw_qos.reliability.kind = DDS::RELIABLE_RELIABILITY_QOS;
    e.dw_qos.history.kind = DDS::KEEP_LAST_HISTORY_QOS;
    e.dw_qos.history.depth = 64;
    e.sub->get_default_datareader_qos(e.dr_qos);
    e.dr_qos.reliability.kind = DDS::RELIABLE_RELIABILITY_QOS;
    e.dr_qos.history.kind = DDS::KEEP_LAST_HISTORY_QOS;
    e.dr_qos.history.depth = 64;
    return true;
}

// --- Pong: receive request, echo immediately ---

class PongListener : public DataListener {
public:
    explicit PongListener(RoundtripBench::RoundtripDataWriter_ptr w) : w_(w) {}
    void on_data_available(DDS::DataReader_ptr reader) override {
        RoundtripBench::RoundtripDataReader_var r = RoundtripBench::RoundtripDataReader::_narrow(reader);
        RoundtripBench::RoundtripSeq samples;
        DDS::SampleInfoSeq infos;
        if (r->take(samples, infos, DDS::LENGTH_UNLIMITED, DDS::ANY_SAMPLE_STATE,
                    DDS::ANY_VIEW_STATE, DDS::ANY_INSTANCE_STATE) != DDS::RETCODE_OK) {
            return;
        }
        for (DDS::ULong i = 0; i < samples.length(); ++i) {
            // Echo back unchanged — sequence_id + t_send_ns + payload.
            if (infos[i].valid_data) w_->write(samples[i], DDS::HANDLE_NIL);
        }
        r->return_loan(samples, infos);
    }
private:
    RoundtripBench::RoundtripDataWriter_ptr w_;
};

int run_pong(uint64_t max_runtime_s) {
    Endpoints e;
    if (!setup(e)) return 1;
    DDS::DataWriter_var dw = e.pub->create_datawriter(e.t_echo.in(), e.dw_qos, NULL,
                                                      DDS::STATUS_MASK_NONE);
    RoundtripBench::RoundtripDataWriter_var w = RoundtripBench::RoundtripDataWriter::_narrow(dw.in());
    PongListener* listener = new PongListener(w.in());
    DDS::DataReaderListener_var listener_var = listener;
    DDS::DataReader_var dr = e.sub->create_datareader(e.t_req.in(), e.dr_qos, listener,
                                                      DDS::DATA_AVAILABLE_STATUS);
    if (!dw.in() || !dr.in()) { std::cerr << "opensplice-app: endpoint creation failed\n"; return 1; }
    std::cout << "pong: matched, listener registered" << std::endl;
    std::this_thread::sleep_for(std::chrono::seconds(max_runtime_s));
    dr->set_listener(NULL, DDS::STATUS_MASK_NONE);
    e.dp->delete_contained_entities();
    e.dpf->delete_participant(e.dp.in());
    return 0;
}

// --- Ping: send + listener captures RTT ---

struct PingState {
    std::mutex              mu;
    std::condition_variable cv;
    uint64_t                received = 0;
    uint64_t                warmup   = 0;
    std::vector<uint64_t>   rtts;
};

class PingListener : public DataListener {
public:
    explicit PingListener(PingState& st) : st_(st) {}
    void on_data_available(DDS::DataReader_ptr reader) override {
        RoundtripBench::RoundtripDataReader_var r = RoundtripBench::RoundtripDataReader::_narrow(reader);
        RoundtripBench::RoundtripSeq samples;
        DDS::SampleInfoSeq infos;
        if (r->take(samples, infos, DDS::LENGTH_UNLIMITED, DDS::ANY_SAMPLE_STATE,
                    DDS::ANY_VIEW_STATE, DDS::ANY_INSTANCE_STATE) != DDS::RETCODE_OK) {
            return;
        }
        for (DDS::ULong i = 0; i < samples.length(); ++i) {
            if (!infos[i].valid_data) continue;
            uint64_t now = now_ns();
            uint64_t t_send = samples[i].t_send_ns;
            uint64_t rtt = now > t_send ? now - t_send : 1;
            {
                std::lock_guard<std::mutex> lk(st_.mu);
                if (st_.received >= st_.warmup) st_.rtts.push_back(rtt);
                st_.received++;
            }
            st_.cv.notify_all();
        }
        r->return_loan(samples, infos);
    }
private:
    PingState& st_;
};

void print_quantiles(std::vector<uint64_t>& rtts, size_t payload_size) {
    if (rtts.empty()) { std::cout << "no samples\n"; return; }
    std::sort(rtts.begin(), rtts.end());
    auto pct = [&](double p) {
        size_t idx = std::min(rtts.size() - 1,
                              static_cast<size_t>(p * static_cast<double>(rtts.size() - 1)));
        return rtts[idx];
    };
    std::cout
        << "payload=" << payload_size
        << "  n=" << rtts.size()
        << "  min=" << rtts.front() / 1000.0 << "us"
        << "  p50=" << pct(0.50) / 1000.0 << "us"
        << "  p90=" << pct(0.90) / 1000.0 << "us"
        << "  p99=" << pct(0.99) / 1000.0 << "us"
        << "  p999=" << pct(0.999) / 1000.0 << "us"
        << "  max=" << rtts.back() / 1000.0 << "us\n";
}

int run_ping(size_t payload_size, uint64_t warmup, uint64_t samples) {
    Endpoints e;
    if (!setup(e)) return 1;
    PingState st;
    st.warmup = warmup;
    st.rtts.reserve(samples);
    DDS::DataWriter_var dw = e.pub->create_datawriter(e.t_req.in(), e.dw_qos, NULL,
                                                      DDS::STATUS_MASK_NONE);
    RoundtripBench::RoundtripDataWriter_var w = RoundtripBench::RoundtripDataWriter::_narrow(dw.in());
    // The listener is attached from the start and counts nothing before the
    // first write: the pong echoes only what it was sent.
    PingListener* listener = new PingListener(st);
    DDS::DataReaderListener_var listener_var = listener;
    DDS::DataReader_var dr = e.sub->create_datareader(e.t_echo.in(), e.dr_qos, listener,
                                                      DDS::DATA_AVAILABLE_STATUS);
    if (!dw.in() || !dr.in()) { std::cerr << "opensplice-app: endpoint creation failed\n"; return 1; }

    // Wait for real endpoint matching, not a fixed sleep: with volatile
    // durability, samples written before the match are never delivered.
    {
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        bool pub_matched = false, sub_matched = false;
        while (std::chrono::steady_clock::now() < deadline) {
            DDS::PublicationMatchedStatus ps;
            DDS::SubscriptionMatchedStatus ss;
            if (!pub_matched && dw->get_publication_matched_status(ps) == DDS::RETCODE_OK &&
                ps.current_count >= 1) pub_matched = true;
            if (!sub_matched && dr->get_subscription_matched_status(ss) == DDS::RETCODE_OK &&
                ss.current_count >= 1) sub_matched = true;
            if (pub_matched && sub_matched) break;
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
        if (!pub_matched || !sub_matched) {
            // No peer to echo: every sample would wait its 50 ms for nothing.
            std::cerr << "ping: match timeout (pub=" << pub_matched << " sub=" << sub_matched << ")\n";
            std::cout << "no samples\n";
            e.dp->delete_contained_entities();
            e.dpf->delete_participant(e.dp.in());
            return 1;
        }
        // The same short buffer after the match as the other apps: the
        // reliable handshake's first ACKNACK round trip.
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }

    RoundtripBench::Roundtrip msg;
    msg.payload.length(static_cast<DDS::ULong>(payload_size));
    for (size_t i = 0; i < payload_size; ++i) msg.payload[static_cast<DDS::ULong>(i)] = 0xAB;

    uint64_t total = warmup + samples;
    for (uint64_t seq = 0; seq < total; ++seq) {
        msg.sequence_id = static_cast<DDS::ULong>(seq);
        msg.t_send_ns = now_ns();
        w->write(msg, DDS::HANDLE_NIL);
        std::unique_lock<std::mutex> lk(st.mu);
        st.cv.wait_for(lk, std::chrono::milliseconds(50), [&] { return st.received > seq; });
    }

    dr->set_listener(NULL, DDS::STATUS_MASK_NONE);
    {
        std::lock_guard<std::mutex> lk(st.mu);
        print_quantiles(st.rtts, payload_size);
    }
    e.dp->delete_contained_entities();
    e.dpf->delete_participant(e.dp.in());
    return 0;
}

}  // namespace

int main(int argc, char** argv) {
    if (argc < 2) {
        std::cerr <<
            "Usage:\n"
            "  opensplice-roundtrip pong [max_runtime_s]\n"
            "  opensplice-roundtrip ping --payload N [--samples N] [--warmup N]\n";
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
            uint64_t v = std::stoull(argv[i + 1]);
            if      (flag == "--payload") payload = static_cast<size_t>(v);
            else if (flag == "--samples") samples = v;
            else if (flag == "--warmup")  warmup  = v;
        }
        return run_ping(payload, warmup, samples);
    }
    std::cerr << "unknown mode: " << mode << "\n";
    return 2;
}
