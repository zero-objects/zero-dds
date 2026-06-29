// SPDX-License-Identifier: Apache-2.0
//
// OpenDDS C++ Roundtrip-Bench-App. Apples-to-apples mit
// ZeroDDS/Cyclone/FastDDS/RTI via gemeinsame roundtrip.idl.
//
// OpenDDS nutzt die klassische OMG-DDS-C++-API (DDS::DomainParticipant
// etc). Codegen: opendds_idl + tao_idl (via opendds_target_sources im
// CMake). Discovery MUSS RTPS sein (opendds_rtps.ini) — OpenDDS-Default
// ist InfoRepo, das spricht kein anderer Vendor.
//
// Aufruf braucht `-DCPSConfigFile opendds_rtps.ini`:
//   ./opendds-roundtrip pong 10 -DCPSConfigFile ../opendds_rtps.ini
//   ./opendds-roundtrip ping --payload 64 -DCPSConfigFile ../opendds_rtps.ini

#include "roundtripTypeSupportImpl.h"

#include <dds/DCPS/Service_Participant.h>
#include <dds/DCPS/Marked_Default_Qos.h>
#include <dds/DCPS/WaitSet.h>
#include <dds/DCPS/LocalObject.h>
#include <dds/DdsDcpsCoreC.h>

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cstdint>
#include <cstring>
#include <cstdlib>
#include <iostream>
#include <string>
#include <thread>
#include <vector>

namespace {

// Domain via ZERODDS_BENCH_DOMAIN env-var ueberschreibbar (Default 200).
static DDS::DomainId_t resolve_domain() {
    const char* s = std::getenv("ZERODDS_BENCH_DOMAIN");
    if (s) { try { return static_cast<DDS::DomainId_t>(std::stoi(s)); } catch (...) {} }
    return 200;
}
const char* kReqTopic  = "RoundtripBench_Request";
const char* kEchoTopic = "RoundtripBench_Echo";

uint64_t now_ns() {
    auto t = std::chrono::steady_clock::now().time_since_epoch();
    return std::chrono::duration_cast<std::chrono::nanoseconds>(t).count();
}

DDS::DataWriterQos reliable_dw_qos(DDS::Publisher_var& pub) {
    DDS::DataWriterQos qos;
    pub->get_default_datawriter_qos(qos);
    qos.reliability.kind = DDS::RELIABLE_RELIABILITY_QOS;
    qos.history.kind = DDS::KEEP_LAST_HISTORY_QOS;
    qos.history.depth = 64;
    // XCDR2 erzwingen. RTI ist matching-strict: wenn writer advertise
    // "XCDR, XCDR2" → RTI-reader (XCDR2-only) sagt "incompatible".
    // Alle apps auf XCDR2-only zwingen damit cross-vendor jeder mit
    // jedem matched (XCDR2 ist verpflichtend in XTypes 1.3).
    qos.representation.value.length(1);
    qos.representation.value[0] = DDS::XCDR2_DATA_REPRESENTATION;
    return qos;
}

DDS::DataReaderQos reliable_dr_qos(DDS::Subscriber_var& sub) {
    DDS::DataReaderQos qos;
    sub->get_default_datareader_qos(qos);
    qos.reliability.kind = DDS::RELIABLE_RELIABILITY_QOS;
    qos.history.kind = DDS::KEEP_LAST_HISTORY_QOS;
    qos.history.depth = 64;
    // ALLOW_TYPE_COERCION — der Match faellt auf den (cross-vendor byte-
    // identischen) COMPLETE-TypeObject zurueck, wenn ein Vendor wie RTI
    // einen abweichenden MINIMAL-TypeObject emittiert (XTypes 1.3 §7.6.3).
    qos.type_consistency.kind = DDS::ALLOW_TYPE_COERCION;
    return qos;
}

// DDS-Security 1.2 (env-driven) — setzt die Standard-PropertyQosPolicy
// analog Fast-DDS/RTI. OpenDDS braucht zusaetzlich `DCPSSecurity=1`
// global (via -DCPSConfigFile opendds_rtps_sec.ini oder -DCPSSecurity 1).
//   ZERODDS_BENCH_SECURITY=1, ZERODDS_BENCH_SEC_NAME, ZERODDS_BENCH_SEC_DIR
void apply_security(DDS::DomainParticipantQos& qos) {
    const char* sec = std::getenv("ZERODDS_BENCH_SECURITY");
    if (!sec || std::string(sec) != "1") return;
    const char* dir = std::getenv("ZERODDS_BENCH_SEC_DIR");
    if (!dir) dir = "/tmp/dds-bench-security";
    const char* who = std::getenv("ZERODDS_BENCH_SEC_NAME");
    if (!who) who = "ping";
    const std::string d(dir), w(who);

    DDS::PropertySeq& props = qos.property.value;
    auto add = [&](const char* name, const std::string& val) {
        const CORBA::ULong n = props.length();
        props.length(n + 1);
        props[n].name = name;
        props[n].value = val.c_str();
        props[n].propagate = false;
    };
    add("dds.sec.auth.identity_ca", "file:" + d + "/certs/identity_ca.pem");
    add("dds.sec.auth.identity_certificate", "file:" + d + "/certs/" + w + "_cert.pem");
    add("dds.sec.auth.private_key", "file:" + d + "/certs/" + w + "_key.pem");
    add("dds.sec.access.permissions_ca", "file:" + d + "/certs/permissions_ca.pem");
    add("dds.sec.access.governance", "file:" + d + "/governance.p7s");
    add("dds.sec.access.permissions", "file:" + d + "/permissions_" + w + ".p7s");
}

// Setup: participant + типed topic-pair. Gibt participant zurueck.
DDS::DomainParticipant_var make_participant(DDS::DomainParticipantFactory_var& dpf) {
    DDS::DomainParticipantQos qos;
    dpf->get_default_participant_qos(qos);
    apply_security(qos);
    DDS::DomainParticipant_var dp = dpf->create_participant(
        resolve_domain(), qos, 0, OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    return dp;
}

void register_type(DDS::DomainParticipant_var& dp) {
    RoundtripBench::RoundtripTypeSupport_var ts =
        new RoundtripBench::RoundtripTypeSupportImpl;
    ts->register_type(dp, "");
}

CORBA::String_var type_name() {
    RoundtripBench::RoundtripTypeSupport_var ts =
        new RoundtripBench::RoundtripTypeSupportImpl;
    return ts->get_type_name();
}

// Event-driven Pong-Listener: `on_data_available` feuert im OpenDDS-
// Recv-Thread, sobald ein Request-Sample da ist — kein Busy-/Sleep-Poll.
// Nimmt das Sample und schreibt es 1:1 auf das Echo-Topic.
class PongListener : public virtual OpenDDS::DCPS::LocalObject<DDS::DataReaderListener> {
public:
    explicit PongListener(RoundtripBench::RoundtripDataWriter_ptr dw)
        : dw_(RoundtripBench::RoundtripDataWriter::_duplicate(dw)), echoed_(0) {}

    uint64_t echoed() const { return echoed_.load(); }

    void on_data_available(DDS::DataReader_ptr reader) override {
        RoundtripBench::RoundtripDataReader_var dr =
            RoundtripBench::RoundtripDataReader::_narrow(reader);
        if (!dr) return;
        RoundtripBench::Roundtrip sample;
        DDS::SampleInfo info;
        while (dr->take_next_sample(sample, info) == DDS::RETCODE_OK) {
            if (info.valid_data) {
                dw_->write(sample, DDS::HANDLE_NIL);
                echoed_.fetch_add(1, std::memory_order_relaxed);
            }
        }
    }

    void on_requested_deadline_missed(
        DDS::DataReader_ptr, const DDS::RequestedDeadlineMissedStatus&) override {}
    void on_requested_incompatible_qos(
        DDS::DataReader_ptr, const DDS::RequestedIncompatibleQosStatus&) override {}
    void on_sample_rejected(
        DDS::DataReader_ptr, const DDS::SampleRejectedStatus&) override {}
    void on_liveliness_changed(
        DDS::DataReader_ptr, const DDS::LivelinessChangedStatus&) override {}
    void on_subscription_matched(
        DDS::DataReader_ptr, const DDS::SubscriptionMatchedStatus&) override {}
    void on_sample_lost(
        DDS::DataReader_ptr, const DDS::SampleLostStatus&) override {}

private:
    RoundtripBench::RoundtripDataWriter_var dw_;
    std::atomic<uint64_t>                   echoed_;
};

// --- Pong: receive request, echo immediately (event-driven) ---
int run_pong(uint64_t max_runtime_s) {
    DDS::DomainParticipantFactory_var dpf = TheServiceParticipant->get_domain_participant_factory();
    DDS::DomainParticipant_var dp = make_participant(dpf);
    if (!dp) { std::cerr << "pong: participant create failed\n"; return 1; }
    register_type(dp);
    CORBA::String_var tn = type_name();

    DDS::Topic_var t_req = dp->create_topic(kReqTopic, tn, TOPIC_QOS_DEFAULT, 0,
        OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    DDS::Topic_var t_echo = dp->create_topic(kEchoTopic, tn, TOPIC_QOS_DEFAULT, 0,
        OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    DDS::Publisher_var pub = dp->create_publisher(PUBLISHER_QOS_DEFAULT, 0,
        OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    DDS::Subscriber_var sub = dp->create_subscriber(SUBSCRIBER_QOS_DEFAULT, 0,
        OpenDDS::DCPS::DEFAULT_STATUS_MASK);

    DDS::DataWriter_var dw_base = pub->create_datawriter(t_echo, reliable_dw_qos(pub), 0,
        OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    RoundtripBench::RoundtripDataWriter_var dw =
        RoundtripBench::RoundtripDataWriter::_narrow(dw_base);

    // Listener BEVOR der Reader erstellt wird — der Reader bekommt ihn
    // direkt mit, on_data_available feuert ab dem ersten Sample.
    PongListener* pong_impl = new PongListener(dw.in());
    DDS::DataReaderListener_var listener = pong_impl;
    DDS::DataReader_var dr_base = sub->create_datareader(t_req, reliable_dr_qos(sub),
        listener.in(), OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    if (!dr_base) { std::cerr << "pong: datareader create failed\n"; return 1; }

    std::cout << "pong[opendds]: started (event-driven)\n" << std::flush;
    std::this_thread::sleep_for(std::chrono::seconds(max_runtime_s));

    std::cout << "pong[opendds]: echoed " << pong_impl->echoed() << " samples\n";
    dp->delete_contained_entities();
    dpf->delete_participant(dp);
    return 0;
}

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
    DDS::DomainParticipantFactory_var dpf = TheServiceParticipant->get_domain_participant_factory();
    DDS::DomainParticipant_var dp = make_participant(dpf);
    if (!dp) { std::cerr << "ping: participant create failed\n"; return 1; }
    register_type(dp);
    CORBA::String_var tn = type_name();

    DDS::Topic_var t_req = dp->create_topic(kReqTopic, tn, TOPIC_QOS_DEFAULT, 0,
        OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    DDS::Topic_var t_echo = dp->create_topic(kEchoTopic, tn, TOPIC_QOS_DEFAULT, 0,
        OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    DDS::Publisher_var pub = dp->create_publisher(PUBLISHER_QOS_DEFAULT, 0,
        OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    DDS::Subscriber_var sub = dp->create_subscriber(SUBSCRIBER_QOS_DEFAULT, 0,
        OpenDDS::DCPS::DEFAULT_STATUS_MASK);

    DDS::DataWriter_var dw_base = pub->create_datawriter(t_req, reliable_dw_qos(pub), 0,
        OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    DDS::DataReader_var dr_base = sub->create_datareader(t_echo, reliable_dr_qos(sub), 0,
        OpenDDS::DCPS::DEFAULT_STATUS_MASK);
    RoundtripBench::RoundtripDataWriter_var dw =
        RoundtripBench::RoundtripDataWriter::_narrow(dw_base);
    RoundtripBench::RoundtripDataReader_var dr =
        RoundtripBench::RoundtripDataReader::_narrow(dr_base);

    // Auf Match warten — publication_matched + subscription_matched.
    {
        auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
        while (std::chrono::steady_clock::now() < deadline) {
            DDS::PublicationMatchedStatus pm;
            DDS::SubscriptionMatchedStatus sm;
            dw->get_publication_matched_status(pm);
            dr->get_subscription_matched_status(sm);
            if (pm.current_count >= 1 && sm.current_count >= 1) break;
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(100));
    }

    std::vector<uint64_t> rtts;
    rtts.reserve(samples);
    RoundtripBench::Roundtrip msg;
    msg.payload.length(static_cast<CORBA::ULong>(payload_size));
    for (size_t i = 0; i < payload_size; ++i) msg.payload[i] = 0xAB;

    uint64_t total = warmup + samples;
    for (uint64_t seq = 0; seq < total; ++seq) {
        msg.sequence_id = static_cast<CORBA::ULong>(seq);
        msg.t_send_ns = now_ns();
        dw->write(msg, DDS::HANDLE_NIL);

        auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(50);
        bool got = false;
        while (!got && std::chrono::steady_clock::now() < deadline) {
            RoundtripBench::Roundtrip echo;
            DDS::SampleInfo info;
            if (dr->take_next_sample(echo, info) == DDS::RETCODE_OK && info.valid_data) {
                uint64_t now = now_ns();
                uint64_t rtt = now > echo.t_send_ns ? now - echo.t_send_ns : 1;
                if (seq >= warmup) rtts.push_back(rtt);
                got = true;
            } else {
                std::this_thread::yield();
            }
        }
    }
    print_quantiles(rtts, payload_size);
    dp->delete_contained_entities();
    dpf->delete_participant(dp);
    return 0;
}

}  // namespace

int main(int argc, char** argv) {
    // TheParticipantFactoryWithArgs verarbeitet -DCPSConfigFile etc.
    DDS::DomainParticipantFactory_var dpf =
        TheParticipantFactoryWithArgs(argc, argv);
    if (argc < 2) {
        std::cerr << "Usage: opendds-roundtrip pong|ping [opts] -DCPSConfigFile <ini>\n";
        return 2;
    }
    std::string mode = argv[1];
    int rc;
    if (mode == "pong") {
        uint64_t rt_s = (argc > 2 && argv[2][0] != '-') ? std::stoull(argv[2]) : 30;
        rc = run_pong(rt_s);
    } else if (mode == "ping") {
        size_t   payload = 64;
        uint64_t warmup  = 200;
        uint64_t samples = 5000;
        for (int i = 2; i + 1 < argc; i += 2) {
            std::string flag = argv[i];
            if (flag.rfind("--", 0) != 0) continue;
            uint64_t v = std::stoull(argv[i+1]);
            if      (flag == "--payload") payload = static_cast<size_t>(v);
            else if (flag == "--samples") samples = v;
            else if (flag == "--warmup")  warmup  = v;
        }
        rc = run_ping(payload, warmup, samples);
    } else {
        std::cerr << "unknown mode: " << mode << "\n";
        rc = 2;
    }
    TheServiceParticipant->shutdown();
    return rc;
}
