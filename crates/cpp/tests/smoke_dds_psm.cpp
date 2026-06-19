// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// smoke_dds_psm.cpp — DDS-PSM-Cxx 1.0 Smoke-Test.
//
// Compile: clang++ -std=c++17 -I crates/cpp/include -I crates/zerodds-c-api/include \
//          crates/cpp/tests/smoke_dds_psm.cpp -L target/debug -lzerodds -o smoke_dds_psm
// Run: ./smoke_dds_psm

#include <cassert>
#include <cstdio>
#include <iostream>
#include <vector>

#include "dds/dds.hpp"

using namespace dds;

static int failures = 0;
#define EXPECT(cond, msg)                                              \
    do {                                                               \
        if (!(cond)) {                                                 \
            std::cerr << "FAIL: " << msg << " at " << __LINE__ << "\n"; \
            ++failures;                                                \
        }                                                              \
    } while (0)

static void test_participant_lifecycle() {
    domain::DomainParticipant dp(0);
    EXPECT(dp.domain_id() == 0, "domain_id");
}

static void test_topic_creation() {
    domain::DomainParticipant dp(1);
    topic::Topic<core::ByteSeq> t(dp, "ChatterTopic");
    EXPECT(t.name() == "ChatterTopic", "topic name");
    EXPECT(t.type_name() == "DDS::Bytes", "topic type_name");
}

static void test_publisher_subscriber_lifecycle() {
    domain::DomainParticipant dp(2);
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    (void)pub_;
    (void)sub_;
}

static void test_writer_reader_lifecycle() {
    domain::DomainParticipant dp(3);
    topic::Topic<core::ByteSeq> t(dp, "T");
    pub::Publisher pub_(dp);
    pub::DataWriter<core::ByteSeq> dw(pub_, t);
    sub::Subscriber sub_(dp);
    sub::DataReader<core::ByteSeq> dr(sub_, t);

    core::ByteSeq msg = {'h', 'i'};
    try {
        dw.write(msg);
    } catch (const core::Exception& e) {
        // Write may fail if no peer matched yet — both OK.
        std::cerr << "write returned: " << e.what() << "\n";
    }
}

static void test_take_on_empty_returns_empty() {
    domain::DomainParticipant dp(4);
    topic::Topic<core::ByteSeq> t(dp, "Empty");
    sub::Subscriber sub_(dp);
    sub::DataReader<core::ByteSeq> dr(sub_, t);
    auto loaned = dr.take();
    EXPECT(loaned.length() == 0, "empty take");
}

static void test_status_getters() {
    domain::DomainParticipant dp(5);
    topic::Topic<core::ByteSeq> t(dp, "S");
    pub::Publisher pub_(dp);
    pub::DataWriter<core::ByteSeq> dw(pub_, t);
    sub::Subscriber sub_(dp);
    sub::DataReader<core::ByteSeq> dr(sub_, t);

    auto pmst = dw.publication_matched_status();
    auto smst = dr.subscription_matched_status();
    auto lost = dr.sample_lost_status();
    // Writer + Reader live in the SAME participant on the same topic/type, so
    // they match each other (intra-participant self-match, DDS §2.2.2.4 — now
    // correctly reported since the F-DCPS-latency-self-match fix). One match
    // each; no samples written, so nothing lost.
    EXPECT(pmst.total_count == 1, "pmst one (self-match)");
    EXPECT(smst.total_count == 1, "smst one (self-match)");
    EXPECT(lost.total_count == 0, "lost zero");
}

static void test_guardcondition_and_waitset() {
    core::cond::WaitSet ws;
    core::cond::GuardCondition gc;
    EXPECT(!gc.trigger_value(), "guard initially false");
    gc.trigger_value(true);
    EXPECT(gc.trigger_value(), "guard set true");
    ws += gc;
    auto active = ws.wait(core::Duration::from_millis(100));
    EXPECT(active.size() == 1, "waitset active count");
}

static void test_readcondition_lifecycle() {
    domain::DomainParticipant dp(7);
    topic::Topic<core::ByteSeq> t(dp, "RC");
    sub::Subscriber sub_(dp);
    sub::DataReader<core::ByteSeq> dr(sub_, t);
    sub::cond::ReadCondition<core::ByteSeq> rc(dr, 0xFF, 0xFF, 0xFF);
    // Trigger-value is `matched_count > 0` — initial state may be true
    // briefly if discovery has matched something. Just check the call
    // doesn't crash and returns a defined bool.
    bool t1 = rc.trigger_value();
    bool t2 = rc.trigger_value();
    EXPECT(t1 == t2, "read cond trigger stable across calls");
}

static void test_contentfilteredtopic_lifecycle() {
    domain::DomainParticipant dp(8);
    topic::Topic<core::ByteSeq> t(dp, "CFT_Base");
    std::vector<std::string> params = {"42"};
    topic::ContentFilteredTopic<core::ByteSeq> cft(dp, "CFT_Filt", t, "x > %0", params);
    EXPECT(cft.native_handle() != nullptr, "cft has handle");
}

static void test_listener_attach_detach() {
    class MyListener : public pub::DataWriterListener<core::ByteSeq> {
    public:
        int matched = 0;
        void on_publication_matched(
            pub::DataWriter<core::ByteSeq>&,
            const core::status::PublicationMatchedStatus&) override {
            ++matched;
        }
    };
    domain::DomainParticipant dp(9);
    topic::Topic<core::ByteSeq> t(dp, "LT");
    pub::Publisher pub_(dp);
    pub::DataWriter<core::ByteSeq> dw(pub_, t);
    MyListener listener;
    pub::_DataWriterListenerBridge<core::ByteSeq>::attach(dw, &listener, 0xFFFFFFFF);
    pub::_DataWriterListenerBridge<core::ByteSeq>::attach(dw, nullptr, 0);
    // attach/detach must not crash; active wireup is via zerodds_poll_listeners.
    EXPECT(listener.matched == 0, "listener not yet fired (no poll-call)");
}

int main() {
#define RUN(fn) do { std::cerr << "[smoke] " #fn " start\n" << std::flush; fn(); std::cerr << "[smoke] " #fn " done\n" << std::flush; } while (0)
    RUN(test_participant_lifecycle);
    RUN(test_topic_creation);
    RUN(test_publisher_subscriber_lifecycle);
    RUN(test_writer_reader_lifecycle);
    RUN(test_take_on_empty_returns_empty);
    RUN(test_status_getters);
    RUN(test_guardcondition_and_waitset);
    RUN(test_readcondition_lifecycle);
    RUN(test_contentfilteredtopic_lifecycle);
    RUN(test_listener_attach_detach);
#undef RUN

    if (failures == 0) {
        std::cout << "OK — all DDS-PSM-Cxx smoke tests passed.\n";
    } else {
        std::cerr << "FAIL — " << failures << " smoke tests failed.\n";
    }
    return failures;
}
