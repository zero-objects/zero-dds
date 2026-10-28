// SPDX-License-Identifier: Apache-2.0
//
// OpenDDS C++ roundtrip bench app. Apples-to-apples with
// ZeroDDS/Cyclone/FastDDS/RTI via gemeinsame roundtrip.idl.
//
// OpenDDS nutzt die klassische OMG-DDS-C++-API (DDS::DomainParticipant
// etc). Codegen: opendds_idl + tao_idl (via opendds_target_sources im
// CMake). Discovery MUSS RTPS sein (opendds_rtps.ini) — OpenDDS-Default
// is InfoRepo, which no other vendor speaks.
//
// Call needs `-DCPSConfigFile opendds_rtps.ini`:
//   ./opendds-roundtrip pong 10 -DCPSConfigFile ../opendds_rtps.ini
//   ./opendds-roundtrip ping --payload 64 -DCPSConfigFile ../opendds_rtps.ini
//
// With BENCH_RICH defined (target opendds-roundtrip-rich) the sample type is
// the codec-heavy roundtrip_rich.idl → RoundtripBench::RoundtripRich, on the
// topics RoundtripRichBench_* of the other rich builds.

#ifdef BENCH_RICH
#include "roundtrip_richTypeSupportImpl.h"
#else
#include "roundtripTypeSupportImpl.h"
#endif
#include "bench_common.hpp"

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

using bench::now_ns;
using bench::print_quantiles;

static DDS::DomainId_t resolve_domain() {
    return static_cast<DDS::DomainId_t>(bench::env_domain());
}
static bool bench_xcdr1() {
    return bench::xcdr1_requested();
}
static DDS::DataRepresentationId_t bench_repr() {
    return bench_xcdr1() ? DDS::XCDR_DATA_REPRESENTATION : DDS::XCDR2_DATA_REPRESENTATION;
}

#ifdef BENCH_RICH
using Sample                = RoundtripBench::RoundtripRich;
using SampleTypeSupport_var = RoundtripBench::RoundtripRichTypeSupport_var;
using SampleTypeSupportImpl = RoundtripBench::RoundtripRichTypeSupportImpl;
using SampleDataWriter      = RoundtripBench::RoundtripRichDataWriter;
using SampleDataWriter_ptr  = RoundtripBench::RoundtripRichDataWriter_ptr;
using SampleDataWriter_var  = RoundtripBench::RoundtripRichDataWriter_var;
using SampleDataReader      = RoundtripBench::RoundtripRichDataReader;
using SampleDataReader_var  = RoundtripBench::RoundtripRichDataReader_var;
constexpr const char* kProgram = "opendds-roundtrip-rich";
const char* kReqTopic  = "RoundtripRichBench_Request";
const char* kEchoTopic = "RoundtripRichBench_Echo";
constexpr size_t kNumWaypoints = 8;

void populate(Sample& m, size_t payload_size) {
    m.name = "opendds-rich-roundtrip-bench";
    for (size_t i = 0; i < 16; ++i) m.transform[i] = static_cast<double>(i) * 1.5 + 0.25;
    m.waypoints.length(static_cast<CORBA::ULong>(kNumWaypoints));
    for (CORBA::ULong i = 0; i < kNumWaypoints; ++i) {
        auto& w = m.waypoints[i];
        w.id = static_cast<CORBA::ULong>(i);
        w.position.x = 1.0 * i; w.position.y = 2.0 * i; w.position.z = 3.0 * i;
        w.velocity.x = 0.1 * i; w.velocity.y = 0.2 * i; w.velocity.z = 0.3 * i;
        std::string lbl = std::string("wp-") + std::to_string(i);
        w.label = lbl.c_str();
        w.weight = 0.5 * static_cast<double>(i) + 1.0;
    }
    m.payload.length(static_cast<CORBA::ULong>(payload_size));
    for (size_t i = 0; i < payload_size; ++i) m.payload[i] = 0xAB;
}
#else
using Sample                = RoundtripBench::Roundtrip;
using SampleTypeSupport_var = RoundtripBench::RoundtripTypeSupport_var;
using SampleTypeSupportImpl = RoundtripBench::RoundtripTypeSupportImpl;
using SampleDataWriter      = RoundtripBench::RoundtripDataWriter;
using SampleDataWriter_ptr  = RoundtripBench::RoundtripDataWriter_ptr;
using SampleDataWriter_var  = RoundtripBench::RoundtripDataWriter_var;
using SampleDataReader      = RoundtripBench::RoundtripDataReader;
using SampleDataReader_var  = RoundtripBench::RoundtripDataReader_var;
constexpr const char* kProgram = "opendds-roundtrip";
const char* kReqTopic  = "RoundtripBench_Request";
const char* kEchoTopic = "RoundtripBench_Echo";

void populate(Sample& m, size_t payload_size) {
    m.payload.length(static_cast<CORBA::ULong>(payload_size));
    for (size_t i = 0; i < payload_size; ++i) m.payload[i] = 0xAB;
}
#endif

DDS::DataWriterQos reliable_dw_qos(DDS::Publisher_var& pub) {
    DDS::DataWriterQos qos;
    pub->get_default_datawriter_qos(qos);
    qos.reliability.kind = DDS::RELIABLE_RELIABILITY_QOS;
    qos.history.kind = DDS::KEEP_LAST_HISTORY_QOS;
    qos.history.depth = 64;
    // Force XCDR2. RTI is matching-strict: if writer advertises
    // "XCDR, XCDR2" → RTI-reader (XCDR2-only) sagt "incompatible".
    // Force all apps to XCDR2-only so cross-vendor everyone with
    // jedem matched (XCDR2 ist verpflichtend in XTypes 1.3).
    qos.representation.value.length(1);
    qos.representation.value[0] = bench_repr();
    return qos;
}

DDS::DataReaderQos reliable_dr_qos(DDS::Subscriber_var& sub) {
    DDS::DataReaderQos qos;
    sub->get_default_datareader_qos(qos);
    qos.reliability.kind = DDS::RELIABLE_RELIABILITY_QOS;
    qos.history.kind = DDS::KEEP_LAST_HISTORY_QOS;
    qos.history.depth = 64;
    // ALLOW_TYPE_COERCION — the match falls back to the (cross-vendor byte-
    // identical) COMPLETE TypeObject when a vendor like RTI
    // emits a diverging MINIMAL TypeObject (XTypes 1.3 §7.6.3).
    qos.type_consistency.kind = DDS::ALLOW_TYPE_COERCION;
    // Only in the XCDR1 run: the reader's default list is left alone for the
    // XCDR2 pairs, as in the matrix of 23.05.
    if (bench_xcdr1()) {
        qos.representation.value.length(1);
        qos.representation.value[0] = bench_repr();
    }
    return qos;
}

// DDS-Security 1.2 (env-driven) — setzt die Standard-PropertyQosPolicy
// like Fast-DDS/RTI. OpenDDS additionally needs `DCPSSecurity=1`
// globally (via -DCPSConfigFile opendds_rtps_sec.ini or -DCPSSecurity 1).
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
    SampleTypeSupport_var ts =
        new SampleTypeSupportImpl;
    ts->register_type(dp, "");
}

CORBA::String_var type_name() {
    SampleTypeSupport_var ts =
        new SampleTypeSupportImpl;
    return ts->get_type_name();
}

// Event-driven Pong-Listener: `on_data_available` feuert im OpenDDS-
// Recv thread as soon as a request sample is present — no busy/sleep poll.
// Takes the sample and writes it 1:1 to the echo topic.
class PongListener : public virtual OpenDDS::DCPS::LocalObject<DDS::DataReaderListener> {
public:
    explicit PongListener(SampleDataWriter_ptr dw)
        : dw_(SampleDataWriter::_duplicate(dw)), echoed_(0) {}

    uint64_t echoed() const { return echoed_.load(); }

    void on_data_available(DDS::DataReader_ptr reader) override {
        SampleDataReader_var dr =
            SampleDataReader::_narrow(reader);
        if (!dr) return;
        Sample sample;
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
    SampleDataWriter_var dw_;
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
    SampleDataWriter_var dw =
        SampleDataWriter::_narrow(dw_base);

    // Listener BEFORE the reader is created — the reader gets it
    // right away, on_data_available fires from the first sample.
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
    SampleDataWriter_var dw =
        SampleDataWriter::_narrow(dw_base);
    SampleDataReader_var dr =
        SampleDataReader::_narrow(dr_base);

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
    Sample msg;
    populate(msg, payload_size);

    uint64_t total = warmup + samples;
    for (uint64_t seq = 0; seq < total; ++seq) {
        msg.sequence_id = static_cast<CORBA::ULong>(seq);
        msg.t_send_ns = now_ns();
        dw->write(msg, DDS::HANDLE_NIL);

        auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(50);
        bool got = false;
        while (!got && std::chrono::steady_clock::now() < deadline) {
            Sample echo;
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
        std::cerr << "Usage: " << kProgram << " pong|ping [opts] -DCPSConfigFile <ini>\n";
        return 2;
    }
    std::string mode = argv[1];
    int rc;
    if (mode == "pong") {
        uint64_t rt_s = (argc > 2 && argv[2][0] != '-') ? std::stoull(argv[2]) : 30;
        rc = run_pong(rt_s);
    } else if (mode == "ping") {
        bench::PingArgs a = bench::parse_ping_args(argc, argv, 2);
        rc = run_ping(a.payload, a.warmup, a.samples);
    } else {
        std::cerr << "unknown mode: " << mode << "\n";
        rc = 2;
    }
    TheServiceParticipant->shutdown();
    return rc;
}
