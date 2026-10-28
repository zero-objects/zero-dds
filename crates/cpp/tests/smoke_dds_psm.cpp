// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// smoke_dds_psm.cpp — DDS-PSM-Cxx 1.0 Smoke-Test.
//
// Compile: clang++ -std=c++17 -I crates/cpp/include -I crates/zerodds-c-api/include \
//          crates/cpp/tests/smoke_dds_psm.cpp -L target/debug -lzerodds -o smoke_dds_psm
//          (the same file compiles with -std=c++03; the parts of DDS-PSM-Cxx 1.0 §7.12 that are
//          C++11 only are behind `#if ZERODDS_CXX11`)
// Run: ./smoke_dds_psm

// The program is ISO C++03 and C++11 or later (DDS-PSM-Cxx 1.0 §3 [C++], §7.1: "relies on C++03
// features only"); the parts DDS-PSM-Cxx 1.0 §7.12 adds for C++11 are behind `#if ZERODDS_CXX11`.
#include <stddef.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>

#include <cassert>
#include <cctype>
#include <set>
#include <map>
#include <iterator>
#include <cstdio>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <string>
#include <vector>

#include "zerodds/psm/cxx.hpp"

#if ZERODDS_CXX11
// The threaded tests and std::atomic need C++11 (no thread library in C++03).
#include <atomic>
#include <chrono>
#include <thread>
#endif

#include "dds/dds.hpp"
#include "dds/core/refmacros.hpp"
#include "dds/core/WeakReference.hpp"
#include "dds/core/xtypes/DynamicData.hpp"
#include "dds/core/xtypes/EnumTypes.hpp"
#include "dds/core/xtypes/TypeProvider.hpp"
#include "dds/core/BuiltinTopicTypes.hpp"
#include "dds/domain/DomainId.hpp"

using namespace dds;

// ---- the domains of the tests -------------------------------------------------------------------
// The tests name their domains by position, `D(0)` ... `D(49)`. A run started by `cargo test` gets
// a few leased domains in `ZERODDS_TEST_DOMAINS` (a comma-separated list of domains no other
// process of the test host holds), so that the runs of other test binaries do not hear it; the
// position picks one of them in turn. The tests that use two domains take consecutive positions,
// so they get two different ones. Started by hand, without the variable, `D(n)` is `n`.
static uint32_t D(uint32_t position) {
    const char* list = ::getenv("ZERODDS_TEST_DOMAINS");
    if (list == 0 || *list == '\0') {
        return position;
    }
    std::vector<uint32_t> domains;
    const char* cursor = list;
    while (true) {
        const char* end = cursor;
        while (*end != '\0' && *end != ',') ++end;
        domains.push_back(static_cast<uint32_t>(::strtoul(std::string(cursor, end).c_str(), 0, 10)));
        if (*end == '\0') break;
        cursor = end + 1;
    }
    if (domains.size() < 2) {
        std::cerr << "FAIL: ZERODDS_TEST_DOMAINS holds fewer than the two domains a test can use\n";
        ::abort();
    }
    return domains[position % domains.size()];
}

// The QoS of the tests whose writer and reader on one participant must see
// every sample: RELIABLE and KEEP_ALL on both sides.
static pub::qos::DataWriterQos reliable_keep_all_writer_qos() {
    pub::qos::DataWriterQos wq;
    wq << core::policy::Reliability::Reliable();
    wq << core::policy::History::KeepAll();
    return wq;
}

static sub::qos::DataReaderQos reliable_keep_all_reader_qos() {
    sub::qos::DataReaderQos rq;
    rq << core::policy::Reliability::Reliable();
    rq << core::policy::History::KeepAll();
    return rq;
}

// ---- C++03 construction helpers (no brace initialisation, no initializer_list) ----------------

/// A ByteSeq of the given octets (at most eight; an argument of -1 is "absent").
static core::ByteSeq bytes(int a0, int a1 = -1, int a2 = -1, int a3 = -1, int a4 = -1, int a5 = -1,
                           int a6 = -1, int a7 = -1) {
    const int all[8] = {a0, a1, a2, a3, a4, a5, a6, a7};
    core::ByteSeq out;
    for (size_t i = 0; i < 8 && all[i] >= 0; ++i) {
        out.push_back(static_cast<uint8_t>(all[i]));
    }
    return out;
}

/// A vector of the given strings (at most eight; a null argument is "absent").
static std::vector<std::string> strs(const char* a0 = 0, const char* a1 = 0, const char* a2 = 0,
                                     const char* a3 = 0, const char* a4 = 0, const char* a5 = 0,
                                     const char* a6 = 0, const char* a7 = 0) {
    const char* const all[8] = {a0, a1, a2, a3, a4, a5, a6, a7};
    std::vector<std::string> out;
    for (size_t i = 0; i < 8 && all[i] != 0; ++i) {
        out.push_back(all[i]);
    }
    return out;
}

/// A DataRepresentationIdSeq of the given identifiers (one to nine): `ids(0, 2)`.
static core::policy::DataRepresentationIdSeq ids(int a0, int a1 = -1, int a2 = -1, int a3 = -1,
                                                  int a4 = -1, int a5 = -1, int a6 = -1,
                                                  int a7 = -1, int a8 = -1) {
    const int all[9] = {a0, a1, a2, a3, a4, a5, a6, a7, a8};
    core::policy::DataRepresentationIdSeq out;
    for (int i = 0; i < 9 && all[i] >= 0; ++i) {
        out.push_back(static_cast<core::policy::DataRepresentationId>(all[i]));
    }
    return out;
}

/// A vector of the given values (one to eight), the stand-in for a braced list: `vec(a, b, c)`.
template <typename T>
static std::vector<T> vec(const T& a0) {
    std::vector<T> out;
    out.push_back(a0);
    return out;
}
template <typename T>
static std::vector<T> vec(const T& a0, const T& a1) {
    std::vector<T> out = vec(a0);
    out.push_back(a1);
    return out;
}
template <typename T>
static std::vector<T> vec(const T& a0, const T& a1, const T& a2) {
    std::vector<T> out = vec(a0, a1);
    out.push_back(a2);
    return out;
}
template <typename T>
static std::vector<T> vec(const T& a0, const T& a1, const T& a2, const T& a3) {
    std::vector<T> out = vec(a0, a1, a2);
    out.push_back(a3);
    return out;
}
template <typename T>
static std::vector<T> vec(const T& a0, const T& a1, const T& a2, const T& a3, const T& a4) {
    std::vector<T> out = vec(a0, a1, a2, a3);
    out.push_back(a4);
    return out;
}
template <typename T>
static std::vector<T> vec(const T& a0, const T& a1, const T& a2, const T& a3, const T& a4, const T& a5) {
    std::vector<T> out = vec(a0, a1, a2, a3, a4);
    out.push_back(a5);
    return out;
}

/// A set of the given strings (at most four).
static std::set<std::string> strset(const char* a0, const char* a1 = 0, const char* a2 = 0, const char* a3 = 0) {
    const std::vector<std::string> v = strs(a0, a1, a2, a3);
    return std::set<std::string>(v.begin(), v.end());
}

/// Counts the calls of a listener function object (the lambdas of C++11 are not available).
struct CountCalls {
    int* counter;
    explicit CountCalls(int* c) : counter(c) {}
    void operator()() const { ++*counter; }
};
struct CountConditionCalls {
    int* counter;
    explicit CountConditionCalls(int* c) : counter(c) {}
    void operator()(const sub::cond::ReadCondition&) const { ++*counter; }
};

// A support with the `CompleteTypeObject` members `idlc cpp` emits, but no
// object (`nullptr`/0, as for a declaration XTypes has none for): it takes
// the typed branch of `ContentFilteredTopic<T>` and declares nothing.
struct Untyped {};
namespace dds {
namespace topic {
template <>
struct topic_type_support<Untyped> {
    static const char* type_name() { return "Untyped"; }
    static bool is_keyed() { return false; }
    static ::zerodds::psm::DataRepresentationKind extensibility() {
        return ::zerodds::psm::DataRepresentationKind::FINAL;
    }
    static const uint8_t* type_object() { return nullptr; }
    static size_t type_object_len() { return 0; }
    static size_t type_object_dependency_count() { return 0; }
    static const uint8_t* type_object_dependency(size_t) { return nullptr; }
    static size_t type_object_dependency_len(size_t) { return 0; }
    static std::vector<uint8_t> encode(const Untyped&) { return std::vector<uint8_t>(); }
    static Untyped decode(const uint8_t*, size_t) { return Untyped(); }
};
} // namespace topic
} // namespace dds

// A type whose decode refuses the payload, to exercise the exception path of
// DataReader<T>::take (DDS-PSM-Cxx 1.0 §7.3: loan-based read/take must be
// exception safe).
struct Throwing {};
namespace dds {
namespace topic {
template <>
struct topic_type_support<Throwing> {
    static const char* type_name() { return "Throwing"; }
    static std::vector<uint8_t> encode(const Throwing&) { return bytes(1, 2, 3); }
    static Throwing decode(const uint8_t*, size_t) {
        throw std::runtime_error("decode refused the sample");
    }
};
} // namespace topic
} // namespace dds

static int failures = 0;
#define EXPECT(cond, msg)                                              \
    do {                                                               \
        if (!(cond)) {                                                 \
            std::cerr << "FAIL: " << msg << " at " << __LINE__ << "\n"; \
            ++failures;                                                \
        }                                                              \
    } while (0)

// DDS 1.4 §2.2.3: RESOURCE_LIMITS is LENGTH_UNLIMITED (-1) in all three
// members for every entity, RELIABILITY is BEST_EFFORT for a Topic and a
// DataReader and RELIABLE for a DataWriter.
static void test_default_qos_matches_the_specification() {
    using core::policy::ReliabilityKind;
    namespace p = core::policy;
    const topic::qos::TopicQos topic;
    const pub::qos::DataWriterQos writer;
    const sub::qos::DataReaderQos reader;
    const p::ResourceLimits* const all_limits[3] = {&topic.policy<p::ResourceLimits>(),
                                                    &writer.policy<p::ResourceLimits>(),
                                                    &reader.policy<p::ResourceLimits>()};
    for (size_t i = 0; i < 3; ++i) {
        const p::ResourceLimits* limits = all_limits[i];
        EXPECT(limits->max_samples() == -1, "max_samples default");
        EXPECT(limits->max_instances() == -1, "max_instances default");
        EXPECT(limits->max_samples_per_instance() == -1, "max_samples_per_instance default");
    }
    EXPECT(topic.policy<p::Reliability>().kind() == ReliabilityKind::BEST_EFFORT,
           "topic reliability default");
    EXPECT(writer.policy<p::Reliability>().kind() == ReliabilityKind::RELIABLE,
           "writer reliability default");
    EXPECT(reader.policy<p::Reliability>().kind() == ReliabilityKind::BEST_EFFORT,
           "reader reliability default");
}

// DDS-PSM-Cxx 1.0 §7.6.1, §7.8, §7.13: policies compose with `<<` for every
// QoS kind, a kind has named enumerators and static helpers, the setters
// chain, and a QoS is built from another kind (`DataWriterQos dwqos(tqos)`).
static void test_qos_composition() {
    namespace p = core::policy;
    topic::qos::TopicQos tqos;
    tqos << p::Reliability::Reliable() << p::Durability::Transient() << p::History::KeepLast(10)
         << p::TransportPriority(14) << p::Ownership::Exclusive() << p::Lifespan(core::Duration(5, 0));
    EXPECT(tqos.policy<p::Reliability>().kind() == p::ReliabilityKind::RELIABLE, "Reliable()");
    EXPECT(tqos.policy<p::Durability>().kind() == p::DurabilityKind::TRANSIENT, "Transient()");
    EXPECT(tqos.policy<p::History>().kind() == p::HistoryKind::KEEP_LAST &&
               tqos.policy<p::History>().depth() == 10,
           "KeepLast(10)");
    EXPECT(tqos.policy<p::TransportPriority>().value() == 14, "TransportPriority(14)");
    EXPECT(tqos.policy<p::Ownership>().kind() == p::OwnershipKind::EXCLUSIVE, "Exclusive()");
    EXPECT(tqos.policy<p::Lifespan>().duration() == core::Duration(5, 0), "Lifespan");

    // The Topic QoS reaches a writer and a reader; their own policies stay.
    pub::qos::DataWriterQos dwqos(tqos);
    sub::qos::DataReaderQos drqos(tqos);
    EXPECT(dwqos.policy<p::History>().depth() == 10 && drqos.policy<p::History>().depth() == 10,
           "the history of the Topic QoS");
    EXPECT(dwqos.policy<p::Durability>().kind() == p::DurabilityKind::TRANSIENT, "durability");
    EXPECT(dwqos.policy<p::OwnershipStrength>().value() == 0, "a policy the Topic has not");

    // Publisher and Subscriber QoS.
    pub::qos::PublisherQos pqos;
    pqos << p::Partition("Tracks") << p::Presentation::GroupAccessScope(true, false)
         << p::EntityFactory::ManuallyEnable();
    EXPECT(pqos.policy<p::Partition>().name().size() == 1 &&
               pqos.policy<p::Partition>().name()[0] == "Tracks",
           "Partition(\"Tracks\")");
    EXPECT(pqos.policy<p::Presentation>().access_scope() == p::PresentationAccessScopeKind::GROUP &&
               pqos.policy<p::Presentation>().coherent_access(),
           "GroupAccessScope");
    EXPECT(!pqos.policy<p::EntityFactory>().autoenable_created_entities(), "ManuallyEnable");

    // Setters return the policy, getters have a reference form.
    p::History h = p::History::KeepAll();
    h.kind(p::HistoryKind::KEEP_LAST).depth(3);
    h.depth() += 1;
    EXPECT(h.kind() == p::HistoryKind::KEEP_LAST && h.depth() == 4, "chained setters, int32_t&");
    EXPECT(p::policy_id<p::Reliability>::value == 11, "policy_id");
    EXPECT(p::policy_name<p::Reliability>::name() == "Reliability", "policy_name");

    // The QoS the C API holds comes back through qos(), the octet sequences
    // of a QoS (TOPIC_DATA here) included.
    tqos << p::TopicData(bytes(1, 2, 3));
    domain::DomainParticipant dp(D(19));
    topic::Topic<core::ByteSeq> t(dp, "QosRoundtrip", tqos);
    const topic::qos::TopicQos& back = t.qos();
    EXPECT(back.policy<p::Durability>().kind() == p::DurabilityKind::TRANSIENT &&
               back.policy<p::History>().depth() == 10 &&
               back.policy<p::Reliability>().kind() == p::ReliabilityKind::RELIABLE,
           "Topic::qos() returns what the Topic was created with");
    EXPECT(back.policy<p::TopicData>().value() == bytes(1, 2, 3),
           "Topic::qos() returns the TOPIC_DATA");
}

// DDS 1.2 §9.3.2 + DDS-PSM-Cxx 1.0 §7.5.6: the infinite Duration sentinel is
// {0x7fffffff, 0x7fffffff}, from_millisecs refuses an overflow instead of
// wrapping, the nanosec mutator normalises into the seconds component, and
// Time and Duration have the conversions, comparison and arithmetic of the
// normative header.
static void test_duration_sentinel_conversion_and_overflow() {
    EXPECT(core::Duration::infinite() == core::Duration(0x7fffffff, 0x7fffffffu),
           "infinite() is {0x7fffffff, 0x7fffffff} per DDS 1.2");
    EXPECT(core::Duration::infinite().sec() == 0x7fffffff &&
               core::Duration::infinite().nanosec() == 0x7fffffffu,
           "infinite() is {0x7fffffff, 0x7fffffff} per DDS 1.2");
    EXPECT(core::Duration(0x7fffffff, 0xffffffffu) == core::Duration::infinite(),
           "the C API's {0x7fffffff, 0xffffffff} is accepted as infinite too");
    EXPECT(core::Duration(1000, 0) < core::Duration::infinite(), "infinite is the greatest");

    bool overflow = false;
    try {
        (void)core::Duration::from_millisecs(static_cast<int64_t>(3000000000000L));
    } catch (const std::overflow_error&) {
        overflow = true;
    }
    EXPECT(overflow, "from_millisecs refuses a value past int32 seconds");

    const core::Duration negative = core::Duration::from_millisecs(-1500);
    EXPECT(negative.sec() == -2 && negative.nanosec() == 500000000u,
           "from_millisecs(-1500) is -1.5 seconds");
    EXPECT(core::Duration(2, 500000000u).to_millisecs() == 2500, "to_millisecs rounds down");
    EXPECT(core::Duration::from_microsecs(2500000).to_microsecs() == 2500000, "microseconds");
    EXPECT(core::Duration::from_secs(1.5) == core::Duration(1, 500000000u), "from_secs(double)");
    EXPECT(core::Duration(3, 0).to_secs() == 3.0, "to_secs");

    // Arithmetic.
    const core::Duration sum = core::Duration(1, 700000000u) + core::Duration(2, 600000000u);
    EXPECT(sum == core::Duration(4, 300000000u), "Duration + Duration carries the nanoseconds");
    const core::Duration diff = core::Duration(4, 300000000u) - core::Duration(1, 700000000u);
    EXPECT(diff == core::Duration(2, 600000000u), "Duration - Duration borrows the nanoseconds");
    EXPECT(core::Duration(1, 500000000u) * 3u == core::Duration(4, 500000000u), "Duration * n");
    EXPECT(core::Duration(4, 500000000u) / 3u == core::Duration(1, 500000000u), "Duration / n");
    EXPECT(core::Duration::infinite() + core::Duration(1, 0) == core::Duration::infinite(),
           "an infinite operand makes the sum infinite");

    core::Time t = core::Time::from_millisecs(1500);
    t += core::Duration(1, 600000000u);
    EXPECT(t == core::Time(3, 100000000u), "Time += Duration");
    EXPECT(t - core::Duration(3, 200000000u) == core::Time(-1, 900000000u), "Time - Duration");
    EXPECT(core::Time(5, 0) > core::Time(4, 999999999u), "Time comparison");
    EXPECT(core::Time(2, 500000000u).to_millisecs() == 2500, "Time::to_millisecs");

    core::Duration normalized;
    normalized.nanosec(1500000000u);
    EXPECT(normalized.sec() == 1 && normalized.nanosec() == 500000000u,
           "nanosec mutator normalises a value of one second or more");
}

static void test_participant_lifecycle() {
    domain::DomainParticipant dp(D(0));
    EXPECT(dp.domain_id() == D(0), "domain_id");
}

static void test_topic_creation() {
    domain::DomainParticipant dp(D(1));
    topic::Topic<core::ByteSeq> t(dp, "ChatterTopic");
    EXPECT(t.name() == "ChatterTopic", "topic name");
    EXPECT(t.type_name() == "DDS::Bytes", "topic type_name");
}

static void test_publisher_subscriber_lifecycle() {
    domain::DomainParticipant dp(D(2));
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    (void)pub_;
    (void)sub_;
}

static void test_writer_reader_lifecycle() {
    domain::DomainParticipant dp(D(3));
    topic::Topic<core::ByteSeq> t(dp, "T");
    pub::Publisher pub_(dp);
    const pub::qos::DataWriterQos wq = reliable_keep_all_writer_qos();
    pub::DataWriter<core::ByteSeq> dw(pub_, t, wq);
    sub::Subscriber sub_(dp);
    const sub::qos::DataReaderQos rq = reliable_keep_all_reader_qos();
    sub::DataReader<core::ByteSeq> dr(sub_, t, rq);
    dw->wait_for_matched(1, core::Duration::from_secs(5));

    // The write is not allowed to fail: an exception from it is the test failing, not a result to
    // print. The writer and the reader of one participant match each other, so the sample arrives.
    core::ByteSeq msg = bytes('h', 'i');
    dw.write(msg);
    const uint64_t deadline = ::zerodds::cxx::monotonic_microseconds() + 5 * 1000000u;
    bool taken = false;
    while (!taken && ::zerodds::cxx::monotonic_microseconds() < deadline) {
        sub::LoanedSamples<core::ByteSeq> got = dr.take();
        if (got.length() >= 1) {
            taken = true;
            EXPECT(got.length() == 1, "one sample was written, one is taken");
            EXPECT(got.begin()->data() == msg, "the sample taken is the sample written");
        } else {
            ::zerodds::cxx::sleep_milliseconds(10);
        }
    }
    EXPECT(taken, "the written sample is taken");
}

static void test_take_on_empty_returns_empty() {
    domain::DomainParticipant dp(D(4));
    topic::Topic<core::ByteSeq> t(dp, "Empty");
    sub::Subscriber sub_(dp);
    sub::DataReader<core::ByteSeq> dr(sub_, t);
    sub::LoanedSamples<core::ByteSeq> loaned = dr.take();
    EXPECT(loaned.length() == 0, "empty take");
}

// DDS 1.2 §7.1.2.5.3.8/9: `read` sets sample_state to READ and leaves the
// sample in the DataReader; `take` removes it. A reliable writer/reader of one
// participant matches itself, so one written sample can be read twice and
// taken once.
static void test_read_keeps_samples_take_removes_them() {
    domain::DomainParticipant dp(D(15));
    topic::Topic<core::ByteSeq> t(dp, "ReadVsTake");
    pub::Publisher pub_(dp);
    const pub::qos::DataWriterQos wq = reliable_keep_all_writer_qos();
    pub::DataWriter<core::ByteSeq> dw(pub_, t, wq);
    sub::Subscriber sub_(dp);
    const sub::qos::DataReaderQos rq = reliable_keep_all_reader_qos();
    sub::DataReader<core::ByteSeq> dr(sub_, t, rq);
    dw->wait_for_matched(1, core::Duration::from_secs(5));
    dr->wait_for_matched(1, core::Duration::from_secs(5));

    core::ByteSeq msg = bytes('r');
    dw.write(msg);

    // Wait for the sample to arrive, without consuming it.
    const uint64_t deadline = ::zerodds::cxx::monotonic_microseconds() + 3 * 1000000u;
    bool arrived = false;
    while (!arrived && ::zerodds::cxx::monotonic_microseconds() < deadline) {
        sub::LoanedSamples<core::ByteSeq> loaned = dr.read();
        arrived = loaned.length() >= 1;
        if (!arrived) ::zerodds::cxx::sleep_milliseconds(10);
    }
    EXPECT(arrived, "the sample arrives on read");

    // read does not remove: a second read still returns it, now sample_state READ.
    sub::LoanedSamples<core::ByteSeq> second = dr.read();
    EXPECT(second.length() >= 1, "a second read still finds the sample");
    if (second.length() >= 1) {
        EXPECT(second.begin()->info().state().sample_state() ==
                   dds::sub::status::SampleState::read(),
               "read sets the sample_state to READ");
    }

    // take removes it.
    sub::LoanedSamples<core::ByteSeq> taken = dr.take();
    EXPECT(taken.length() >= 1, "take finds the sample");
    sub::LoanedSamples<core::ByteSeq> after = dr.take();
    EXPECT(after.length() == 0, "take removed the sample");
}

static void test_status_getters() {
    domain::DomainParticipant dp(D(5));
    topic::Topic<core::ByteSeq> t(dp, "S");
    pub::Publisher pub_(dp);
    pub::DataWriter<core::ByteSeq> dw(pub_, t);
    sub::Subscriber sub_(dp);
    sub::DataReader<core::ByteSeq> dr(sub_, t);

    const core::status::PublicationMatchedStatus pmst = dw.publication_matched_status();
    const core::status::SubscriptionMatchedStatus smst = dr.subscription_matched_status();
    const core::status::SampleLostStatus lost = dr.sample_lost_status();
    // Writer + Reader live in the SAME participant on the same topic/type, so
    // they match each other (intra-participant self-match, DDS §2.2.2.4 — now
    // correctly reported since the F-DCPS-latency-self-match fix). One match
    // each; no samples written, so nothing lost.
    EXPECT(pmst.total_count() == 1, "pmst one (self-match)");
    EXPECT(smst.total_count() == 1, "smst one (self-match)");
    EXPECT(lost.total_count() == 0, "lost zero");
}

static void test_guardcondition_and_waitset() {
    core::cond::WaitSet ws;
    core::cond::GuardCondition gc;
    EXPECT(!gc.trigger_value(), "guard initially false");
    gc.trigger_value(true);
    EXPECT(gc.trigger_value(), "guard set true");
    ws += gc;
    const core::cond::WaitSet::ConditionSeq active = ws.wait(core::Duration::from_millisecs(100));
    EXPECT(active.size() == 1, "waitset active count");
    // TWaitSet / TGuardCondition: conditions(), the handler, dispatch(), detach_condition() as bool.
    EXPECT(ws.conditions().size() == 1, "conditions() lists the attached condition");
    int handled = 0;
    gc.handler(CountCalls(&handled));
    ws.dispatch(core::Duration::from_millisecs(100));
    EXPECT(handled == 1, "dispatch() calls the handler of a condition that holds");
    gc.reset_handler();
    ws.dispatch(core::Duration::from_millisecs(100));
    EXPECT(handled == 1, "reset_handler() removes it");
    EXPECT(ws.detach_condition(gc), "detach_condition() is true for an attached condition");
    EXPECT(!ws.detach_condition(gc), "and false for one that is not attached");
}

static void test_readcondition_lifecycle() {
    domain::DomainParticipant dp(D(7));
    topic::Topic<core::ByteSeq> t(dp, "RC");
    sub::Subscriber sub_(dp);
    sub::DataReader<core::ByteSeq> dr(sub_, t);
    sub::cond::ReadCondition rc(dr, sub::status::DataState::any());
    EXPECT(rc.state_filter() == sub::status::DataState::any(), "state_filter is the state given");
    EXPECT(rc.data_reader()->topic_name() == "RC", "data_reader is the reader the condition is bound to");
    int dispatched = 0;
    sub::cond::ReadCondition with_functor(dr, sub::status::DataState::any(), CountConditionCalls(&dispatched));
    with_functor.dispatch();
    int plain = 0;
    sub::cond::ReadCondition no_arg(dr, sub::status::DataState::any(), CountCalls(&plain));
    no_arg.dispatch();
    EXPECT(dispatched <= 1 && plain <= 1, "dispatch calls the functor at most once, when the condition holds");
    sub::Query query(dr, "x > %0", strs("1"));
    sub::cond::QueryCondition qc(query, sub::status::DataState::any());
    EXPECT(qc.expression() == "x > %0" && qc.parameters_length() == 1 && *qc.begin() == "1",
           "QueryCondition keeps the expression and the parameters");
    qc.add_parameter("2");
    EXPECT(qc.parameters_length() == 2 && *(qc.end() - 1) == "2", "add_parameter appends");
    const std::vector<std::string> replaced = strs("7");
    qc.parameters(replaced.begin(), replaced.end());
    EXPECT(qc.parameters_length() == 1 && *qc.begin() == "7", "parameters replaces the parameters");
    qc.expression("x < %0");
    EXPECT(qc.expression() == "x < %0", "expression replaces the expression");
    EXPECT(qc.data_reader()->topic_name() == "RC", "the query condition names its reader");
    core::cond::StatusCondition sc(dr);
    EXPECT(sc.entity().instance_handle() == dr.instance_handle(), "StatusCondition::entity is the entity");
    // read/take next_instance together with a content filter.
    EXPECT(dr.select().next_instance(core::InstanceHandle()).content(query).read().length() == 0,
           "next_instance with a content filter reads nothing from an empty reader");
    // Trigger-value is `matched_count > 0` — initial state may be true
    // briefly if discovery has matched something. Just check the call
    // doesn't crash and returns a defined bool.
    bool t1 = rc.trigger_value();
    bool t2 = rc.trigger_value();
    EXPECT(t1 == t2, "read cond trigger stable across calls");
}

static void test_contentfilteredtopic_lifecycle() {
    domain::DomainParticipant dp(D(8));
    topic::Topic<core::ByteSeq> t(dp, "CFT_Base");
    std::vector<std::string> params = strs("42");
    topic::ContentFilteredTopic<core::ByteSeq> cft(t, "CFT_Filt", topic::Filter("x > %0", params));
    EXPECT(cft->native_cft_handle() != nullptr, "cft has handle");
    // DDS 1.2 §7.1.2.3.1.3: name() is the name the CFT was created with, not
    // the related topic's.
    EXPECT(cft.name() == "CFT_Filt", "cft name is its own, not the related topic's");
    // `x` is a field and ByteSeq has no type to find it in: the reader is
    // refused instead of passing every sample unfiltered.
    sub::Subscriber sub_(dp);
    bool refused = false;
    try {
        sub::DataReader<core::ByteSeq> dr(sub_, cft);
    } catch (const ::dds::core::Error&) {
        refused = true;
    }
    EXPECT(refused, "a field filter without a type is refused");

    EXPECT((cft.filter_parameters() == strs("42")), "filter_parameters reads back");
    std::vector<std::string> next = strs("7");
    cft.filter_parameters(next.begin(), next.end());
    EXPECT((cft.filter_parameters() == strs("7")), "filter_parameters sets");
    bool rejected = false;
    std::vector<std::string> none;
    try {
        cft.filter_parameters(none.begin(), none.end());
    } catch (const ::dds::core::Error&) {
        rejected = true;
    }
    EXPECT(rejected, "a parameter list the expression does not fit is refused");

    topic::Topic<Untyped> ut(dp, "CFT_Untyped");
    topic::ContentFilteredTopic<Untyped> ucft(ut, "CFT_UntypedFilt", topic::Filter("x > %0", params));
    EXPECT(ucft->native_cft_handle() != nullptr, "a support without an object still makes a cft");
}

static void test_listener_attach_detach() {
    class MyListener : public pub::NoOpDataWriterListener<core::ByteSeq> {
    public:
        MyListener() : matched(0) {}
        int matched;
        void on_publication_matched(
            pub::DataWriter<core::ByteSeq>&,
            const core::status::PublicationMatchedStatus&) ZERODDS_OVERRIDE {
            ++matched;
        }
    };
    domain::DomainParticipant dp(D(9));
    topic::Topic<core::ByteSeq> t(dp, "LT");
    pub::Publisher pub_(dp);
    pub::DataWriter<core::ByteSeq> dw(pub_, t);
    MyListener listener;
    dw.listener(&listener, core::status::StatusMask::all());
    dw.listener(nullptr, core::status::StatusMask::none());
    // attach/detach must not crash; active wireup is via zerodds_poll_listeners.
    EXPECT(listener.matched == 0, "listener not yet fired (no poll-call)");
}

// Vendor spec `zerodds-listener-callbacks` §7.1: the C++ headers wrap the C
// vtable — "each method is an `extern \"C\"` shim that casts `user_data` back
// to the C++ listener and calls the virtual method".
//
// `test_listener_attach_detach` above only shows that attaching does not
// crash and that nothing fires without a poll. It asserts the callback count
// stays zero, so it cannot show that the shim dispatches at all. This test
// checks the dispatch itself: it reads back the vtable the bridge installed
// through the C FFI and invokes the entries exactly as the runtime's poll
// does — `fn(vt->user_data, handle)` — then asserts the virtual method on the
// derived C++ listener ran. That is deterministic and independent of
// discovery timing.
static void test_listener_bridge_dispatches_to_virtual() {
    class WriterListener : public pub::NoOpDataWriterListener<core::ByteSeq> {
    public:
        WriterListener() : matched(0), liveliness_lost(0), seen_writer(nullptr), seen_total_count(-1) {}
        int matched;
        int liveliness_lost;
        zerodds_ZeroDdsDataWriter* seen_writer;
        int seen_total_count;
        void on_publication_matched(
            pub::DataWriter<core::ByteSeq>& w,
            const core::status::PublicationMatchedStatus& s) ZERODDS_OVERRIDE {
            ++matched;
            seen_writer = w->native_handle();
            seen_total_count = s.total_count();
        }
        void on_liveliness_lost(
            pub::DataWriter<core::ByteSeq>&,
            const core::status::LivelinessLostStatus&) ZERODDS_OVERRIDE {
            ++liveliness_lost;
        }
    };
    class ReaderListener : public sub::NoOpDataReaderListener<core::ByteSeq> {
    public:
        ReaderListener() : data_available(0), seen_reader(nullptr) {}
        int data_available;
        zerodds_ZeroDdsDataReader* seen_reader;
        void on_data_available(sub::DataReader<core::ByteSeq>& r) ZERODDS_OVERRIDE {
            ++data_available;
            seen_reader = r->native_handle();
        }
    };

    domain::DomainParticipant dp(D(10));
    topic::Topic<core::ByteSeq> t(dp, "LT_dispatch");
    pub::Publisher pub_(dp);
    pub::DataWriter<core::ByteSeq> dw(pub_, t);
    sub::Subscriber sub_(dp);
    sub::DataReader<core::ByteSeq> dr(sub_, t);

    WriterListener wl;
    dw.listener(&wl, core::status::StatusMask::all());
    const zerodds_ZeroDdsDataWriterListener* wvt =
        zerodds_dw_get_listener(dw->native_handle());
    EXPECT(wvt != nullptr, "writer vtable registered by the bridge");
    if (wvt != nullptr) {
        EXPECT(wvt->user_data == static_cast<void*>(&wl),
               "writer user_data is the C++ listener subclass");
        EXPECT(wvt->on_publication_matched != nullptr,
               "on_publication_matched shim installed");
        // Invoke as the C FFI does.
        wvt->on_publication_matched(wvt->user_data, dw->native_handle());
        EXPECT(wl.matched == 1,
               "shim casts user_data back and calls the virtual method");
        EXPECT(wl.seen_writer == dw->native_handle(),
               "the callback hands the triggering writer, not a dummy");
        EXPECT(wl.seen_total_count >= 0,
               "the callback hands the writer's real status");
        wvt->on_liveliness_lost(wvt->user_data, dw->native_handle());
        EXPECT(wl.liveliness_lost == 1,
               "a second shim dispatches to its own virtual method");
        // A NULL user_data must be tolerated, not dereferenced.
        wvt->on_publication_matched(nullptr, dw->native_handle());
        EXPECT(wl.matched == 1, "NULL user_data fires nothing");
    }

    ReaderListener rl;
    dr.listener(&rl, core::status::StatusMask::all());
    const zerodds_ZeroDdsDataReaderListener* rvt =
        zerodds_dr_get_listener(dr->native_handle());
    EXPECT(rvt != nullptr, "reader vtable registered by the bridge");
    if (rvt != nullptr) {
        EXPECT(rvt->user_data == static_cast<void*>(&rl),
               "reader user_data is the C++ listener subclass");
        rvt->on_data_available(rvt->user_data, dr->native_handle());
        EXPECT(rl.data_available == 1,
               "reader shim casts user_data back and calls the virtual method");
        EXPECT(rl.seen_reader == dr->native_handle(),
               "the callback hands the triggering reader, not a dummy");
    }

    // §1.3/§9: clearing must unregister, so a later poll cannot reach a
    // listener whose C++ object is about to go out of scope.
    dw.listener(nullptr, core::status::StatusMask::none());
    dr.listener(nullptr, core::status::StatusMask::none());
    EXPECT(zerodds_dw_get_listener(dw->native_handle()) == nullptr,
           "writer listener cleared via NULL");
    EXPECT(zerodds_dr_get_listener(dr->native_handle()) == nullptr,
           "reader listener cleared via NULL");
}

// Each writer owns its own listener vtable: two writers of the same type with
// two listeners must not share one thread-local table, and destroying a writer
// with an attached listener must deregister it so a later poll cannot reach a
// freed writer or vtable.
static void test_listener_table_is_per_entity_and_deregisters_on_destroy() {
    class ListenerA : public pub::NoOpDataWriterListener<core::ByteSeq> {
    public:
        ListenerA() : matched(0) {}
        int matched;
        void on_publication_matched(
            pub::DataWriter<core::ByteSeq>&,
            const core::status::PublicationMatchedStatus&) ZERODDS_OVERRIDE { ++matched; }
    };
    class ListenerB : public pub::NoOpDataWriterListener<core::ByteSeq> {
    public:
        ListenerB() : matched(0) {}
        int matched;
        void on_publication_matched(
            pub::DataWriter<core::ByteSeq>&,
            const core::status::PublicationMatchedStatus&) ZERODDS_OVERRIDE { ++matched; }
    };

    domain::DomainParticipant dp(D(11));
    topic::Topic<core::ByteSeq> t(dp, "LT_per_entity");
    pub::Publisher pub_(dp);
    pub::DataWriter<core::ByteSeq> dw1(pub_, t);
    pub::DataWriter<core::ByteSeq> dw2(pub_, t);
    ListenerA la;
    ListenerB lb;
    dw1.listener(&la, core::status::StatusMask::all());
    dw2.listener(&lb, core::status::StatusMask::all());

    const zerodds_ZeroDdsDataWriterListener* vt1 =
        zerodds_dw_get_listener(dw1->native_handle());
    const zerodds_ZeroDdsDataWriterListener* vt2 =
        zerodds_dw_get_listener(dw2->native_handle());
    EXPECT(vt1 != nullptr && vt2 != nullptr, "both writers registered");
    if (vt1 != nullptr && vt2 != nullptr) {
        EXPECT(vt1 != vt2, "each writer has its own vtable, not one shared");
        vt1->on_publication_matched(vt1->user_data, dw1->native_handle());
        EXPECT(la.matched == 1 && lb.matched == 0,
               "firing one writer's table reaches only its own listener");
    }

    // Destroy a writer with an attached listener: the registry entry must be
    // gone. get_listener looks the raw pointer up by value only (it does not
    // dereference it), so the stale pointer is safe to use as a lookup key.
    zerodds_ZeroDdsDataWriter* raw2 = dw2->native_handle();
    dw2.listener(nullptr, core::status::StatusMask::none());
    EXPECT(zerodds_dw_get_listener(raw2) == nullptr,
           "detaching removes the registry entry");

    // DDS-PSM-Cxx 1.0 §7.5.1.2: an entity with a non-null listener is still in use, so dropping
    // the last reference does not destroy the writer; the application finds it again and removes
    // the listener, and the writer goes with the next release.
    zerodds_ZeroDdsDataWriter* raw1 = dw1->native_handle();
    dw1 = pub::DataWriter<core::ByteSeq>(core::null);
    EXPECT(zerodds_dw_get_listener(raw1) != nullptr,
           "a writer with a listener is still in use after its last reference");
    std::vector<pub::DataWriter<core::ByteSeq> > again;
    pub::find<pub::DataWriter<core::ByteSeq> >(pub_, "LT_per_entity", std::back_inserter(again));
    bool refound = false;
    for (std::vector<pub::DataWriter<core::ByteSeq> >::iterator wi = again.begin(); wi != again.end(); ++wi) {
        pub::DataWriter<core::ByteSeq>& w = *wi;
        if (w->native_handle() == raw1) {
            refound = true;
            w.listener(nullptr, core::status::StatusMask::none());
            EXPECT(zerodds_dw_get_listener(raw1) == nullptr, "removing the listener deregisters it");
        }
    }
    EXPECT(refound, "and can be found again");
    again.clear();
}

// DDS 1.4 §2.2.2.4.2 get_matched_subscriptions and
// get_matched_subscription_data, §2.2.2.5.3 get_matched_publications and
// get_matched_publication_data: a writer and a reader of one participant
// match each other, and each reads the other's data with the QoS the other
// was created with. A reader of the writer's own participant is not in the
// builtin DCPSSubscription reader; its handle was listed and its data was
// not found.
static void test_matched_data_names_the_other_endpoint() {
    domain::DomainParticipant dp(D(6));
    topic::Topic<core::ByteSeq> t(dp, "Matched");
    pub::Publisher pub_(dp);
    pub::qos::DataWriterQos wq;
    wq << core::policy::Deadline(core::Duration::from_millisecs(400));
    wq << core::policy::UserData(bytes(1, 2, 3));
    pub::DataWriter<core::ByteSeq> dw(pub_, t, wq);
    sub::Subscriber sub_(dp);
    sub::qos::DataReaderQos rq;
    rq << core::policy::TimeBasedFilter(core::Duration::from_millisecs(100));
    sub::DataReader<core::ByteSeq> dr(sub_, t, rq);

    const core::InstanceHandleSeq subscriptions = pub::matched_subscriptions(dw);
    const core::InstanceHandleSeq publications = sub::matched_publications(dr);
    EXPECT(subscriptions.size() == 1, "the writer is matched with one reader");
    EXPECT(publications.size() == 1, "the reader is matched with one writer");
    if (subscriptions.size() == 1) {
        const topic::SubscriptionBuiltinTopicData s = pub::matched_subscription_data(dw, subscriptions[0]);
        EXPECT(s.topic_name() == "Matched", "subscription topic name");
        EXPECT(s.type_name() == "DDS::Bytes", "subscription type name");
        EXPECT(s.time_based_filter().minimum_separation().sec() == 0 &&
                   s.time_based_filter().minimum_separation().nanosec() == 100000000u,
               "subscription TIME_BASED_FILTER");
    }
    if (publications.size() == 1) {
        const topic::PublicationBuiltinTopicData p = sub::matched_publication_data(dr, publications[0]);
        EXPECT(p.topic_name() == "Matched", "publication topic name");
        EXPECT(p.deadline().period().sec() == 0 &&
                   p.deadline().period().nanosec() == 400000000u,
               "publication DEADLINE");
        EXPECT(p.user_data().value() == bytes(1, 2, 3),
               "publication USER_DATA");
    }

    // A handle that names no matched endpoint is BAD_PARAMETER.
    bool refused = false;
    try {
        (void)pub::matched_subscription_data(dw, core::InstanceHandle(12345));
    } catch (const core::InvalidArgumentError&) {
        refused = true;
    }
    EXPECT(refused, "an unmatched handle is refused");
}

// DDS 1.4 §2.2.5.2 DCPSTopic: every member the specification lists reaches
// dds::topic::TopicBuiltinTopicData, each as its policy class — including
// HISTORY, RESOURCE_LIMITS and TRANSPORT_PRIORITY, which the C struct held
// no place for. The struct is filled the way the C API fills it; the class
// copies it, so nothing here is owned by the release that
// zerodds_topic_builtin_topic_data_free is for.
static void test_topic_data_carries_every_member() {
    const std::string name = "Chatter";
    const std::string type_name = "DDS::Chatter";
    const std::vector<uint8_t> topic_data = bytes(1, 2, 3);
    zerodds_ZeroDdsTopicBuiltinTopicData native;
    memset(&native, 0, sizeof(native));
    native.key[0] = 0xAB;
    native.name = const_cast<char*>(name.c_str());
    native.type_name = const_cast<char*>(type_name.c_str());
    native.durability.kind = 1;
    native.durability_service.history_kind = 1;
    native.durability_service.history_depth = -1;
    native.durability_service.max_samples_per_instance = 2;
    native.deadline.period.sec = 1;
    native.deadline.period.nanosec = 250000000u;
    native.latency_budget.duration.nanosec = 500000000u;
    native.liveliness.kind = 1;
    native.liveliness.lease_duration.sec = 2;
    native.reliability.kind = 2;
    native.reliability.max_blocking_time.nanosec = 75000000u;
    native.transport_priority.value = 7;
    native.lifespan.duration.sec = 3;
    native.destination_order.kind = 1;
    native.history.kind = 1;
    native.history.depth = -1;
    native.resource_limits.max_samples = 4096;
    native.resource_limits.max_instances = 32;
    native.resource_limits.max_samples_per_instance = 128;
    native.ownership.kind = 1;
    native.topic_data.value = &topic_data[0];
    native.topic_data.value_len = topic_data.size();

    const topic::TopicBuiltinTopicData d = topic::TopicBuiltinTopicData::from_native(native);
    EXPECT(d.key().value()[0] == 0xAB, "topic key");
    EXPECT(d.name() == "Chatter", "topic name");
    EXPECT(d.type_name() == "DDS::Chatter", "topic type name");
    EXPECT(d.durability().kind() == core::policy::DurabilityKind::TRANSIENT_LOCAL,
           "topic DURABILITY");
    EXPECT(d.durability_service().history_kind() == core::policy::HistoryKind::KEEP_ALL &&
               d.durability_service().history_depth() == -1 &&
               d.durability_service().max_samples_per_instance() == 2,
           "topic DURABILITY_SERVICE");
    EXPECT(d.deadline().period().sec() == 1 && d.deadline().period().nanosec() == 250000000u,
           "topic DEADLINE");
    EXPECT(d.latency_budget().duration().nanosec() == 500000000u, "topic LATENCY_BUDGET");
    EXPECT(d.liveliness().kind() == core::policy::LivelinessKind::MANUAL_BY_PARTICIPANT &&
               d.liveliness().lease_duration().sec() == 2,
           "topic LIVELINESS");
    EXPECT(d.reliability().kind() == core::policy::ReliabilityKind::RELIABLE &&
               d.reliability().max_blocking_time().nanosec() == 75000000u,
           "topic RELIABILITY");
    EXPECT(d.transport_priority().value() == 7, "topic TRANSPORT_PRIORITY");
    EXPECT(d.lifespan().duration().sec() == 3, "topic LIFESPAN");
    EXPECT(d.destination_order().kind() == core::policy::DestinationOrderKind::BY_SOURCE_TIMESTAMP,
           "topic DESTINATION_ORDER");
    EXPECT(d.history().kind() == core::policy::HistoryKind::KEEP_ALL && d.history().depth() == -1,
           "topic HISTORY");
    EXPECT(d.resource_limits().max_samples() == 4096 && d.resource_limits().max_instances() == 32 &&
               d.resource_limits().max_samples_per_instance() == 128,
           "topic RESOURCE_LIMITS");
    EXPECT(d.ownership().kind() == core::policy::OwnershipKind::EXCLUSIVE, "topic OWNERSHIP");
    EXPECT(d.topic_data().value() == topic_data, "topic TOPIC_DATA");
}

static void test_discovered_topics_empty_initially() {
    domain::DomainParticipant dp(D(11));
    const core::InstanceHandleSeq topics = dp->get_discovered_topics();
    EXPECT(topics.empty(), "no discovered topics initially");
}

static void test_discovered_topic_data_unknown_handle_throws() {
    domain::DomainParticipant dp(D(12));
    bool threw = false;
    try {
        (void)dp->get_discovered_topic_data(core::InstanceHandle(0xDEAD));
    } catch (const core::PreconditionNotMetError&) {
        // DDS 1.4 2.2.2.2.1.30: "otherwise the operation will fail and
        // return PRECONDITION_NOT_MET".
        threw = true;
    }
    EXPECT(threw, "unknown handle throws PreconditionNotMetError");
}

static void test_discovered_publications_count_zero() {
    domain::DomainParticipant dp(D(13));
    EXPECT(zerodds_dp_discovered_publications_count(dp->native_handle()) == 0, "discovered publications count zero");
}

static void test_discovered_subscriptions_count_zero() {
    domain::DomainParticipant dp(D(14));
    EXPECT(zerodds_dp_discovered_subscriptions_count(dp->native_handle()) == 0, "discovered subscriptions count zero");
}

// A reference type declared with the macros of dds/core/refmacros.hpp.
namespace {
struct MacroDelegate {
    MacroDelegate() : value(7) {}
    int value;
};
class MacroRef : public ::dds::core::Reference<MacroDelegate> {
public:
    OMG_DDS_REF_TYPE(MacroRef, ::dds::core::Reference, MacroDelegate)
    MacroRef() : ::dds::core::Reference<MacroDelegate>(::zerodds::cxx::make_shared<MacroDelegate>()) {}
};
} // namespace

// DDS-PSM-Cxx 1.0: the macros, optional, array, BuiltinTopicKey, the RAII
// guards and the find functions with their entity tracker.
static void test_layout_headers_and_find() {
    OMG_DDS_STATIC_ASSERT(OMG_DDS_STATE_BIT_COUNT == 16);
    {
        MacroRef r;
        MacroRef none((::dds::core::null));
        EXPECT(r->value == 7 && none == ::dds::core::null, "OMG_DDS_REF_TYPE declares a reference type");
        const domain::DomainId id(5);
        const uint32_t as_integer = id;
        EXPECT(id.value() == 5 && as_integer == 5 && domain::DomainId::default_domain().value() == 0,
               "DomainId");
    }
#ifndef OMG_DDS_CONTENT_SUBSCRIPTION_SUPPORT
#error "the content-subscription switch is not defined"
#endif

    core::array<int, 3> a = {{1, 2, 3}};
    int sum = 0;
    for (core::array<int, 3>::const_iterator v = a.begin(); v != a.end(); ++v) {
        sum += *v;
    }
    EXPECT(a.size() == 3 && a[2] == 3 && sum == 6 && a == a, "core::array");

    core::optional<int32_t> o;
    EXPECT(!o.is_set(), "optional starts unset");
    bool threw = false;
    try {
        o.get();
    } catch (const core::PreconditionNotMetError&) {
        threw = true;
    }
    EXPECT(threw, "optional::get on an unset value throws PreconditionNotMetError");
    o = core::optional<int32_t>(5);
    EXPECT(o.is_set() && o.get() == 5, "optional holds a value");
    o.reset();
    EXPECT(!o.is_set(), "optional::reset");

    topic::BuiltinTopicKey key;
    EXPECT(key.value().size() == 16 && key.value()[0] == 0, "BuiltinTopicKey default is all zero");

    domain::DomainParticipant dp(D(23));
    topic::Topic<core::ByteSeq> t(dp, "FindTopic");
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    pub::DataWriter<core::ByteSeq> w1(pub_, t);
    pub::DataWriter<core::ByteSeq> w2(pub_, t);
    sub::DataReader<core::ByteSeq> r1(sub_, t);

    std::vector<pub::DataWriter<core::ByteSeq> > ws;
    EXPECT(pub::find<pub::DataWriter<core::ByteSeq> >(pub_, "FindTopic", std::back_inserter(ws)) == 2,
           "pub::find returns both writers");
    EXPECT(ws.size() == 2 && ws[0] == w1 && ws[1] == w2, "pub::find returns the writers in creation order");
    std::vector<pub::AnyDataWriter> aws;
    EXPECT(pub::find<pub::AnyDataWriter>(pub_, "FindTopic", std::back_inserter(aws)) == 2,
           "pub::find as AnyDataWriter");
    EXPECT(aws[0]->topic_name() == "FindTopic", "the AnyDataWriter names the topic");
    pub::DataWriter<core::ByteSeq> one[1] = {pub::DataWriter<core::ByteSeq>(core::null)};
    EXPECT(pub::find<pub::DataWriter<core::ByteSeq> >(pub_, "FindTopic", one, 1) == 1 && one[0] == w1,
           "pub::find stops at max_size");
    EXPECT(pub::find<pub::DataWriter<core::ByteSeq> >(pub_, "Other", std::back_inserter(ws)) == 0,
           "pub::find finds nothing for another topic");

    std::vector<sub::DataReader<core::ByteSeq> > rs;
    EXPECT(sub::find<sub::DataReader<core::ByteSeq> >(sub_, "FindTopic", std::back_inserter(rs)) == 1 &&
               rs[0] == r1,
           "sub::find by topic name");
    rs.clear();
    EXPECT(sub::find<sub::DataReader<core::ByteSeq> >(sub_, t, std::back_inserter(rs)) == 1,
           "sub::find by topic description");
    std::vector<sub::AnyDataReader> ars;
    EXPECT(sub::find<sub::AnyDataReader>(sub_, "FindTopic", std::back_inserter(ars)) == 1,
           "sub::find as AnyDataReader");
    rs.clear();
    EXPECT(sub::find<sub::DataReader<core::ByteSeq> >(sub_, sub::status::DataState::new_data(),
                                                     std::back_inserter(rs)) <= 1,
           "sub::find by data state");

    {
        // A reader that no longer exists is not found.
        sub::DataReader<core::ByteSeq> gone(sub_, t);
    }
    rs.clear();
    EXPECT(sub::find<sub::DataReader<core::ByteSeq> >(sub_, "FindTopic", std::back_inserter(rs)) == 1,
           "sub::find skips a reader the application released");

    topic::Topic<core::ByteSeq> ft = topic::find<topic::Topic<core::ByteSeq> >(dp, "FindTopic");
    EXPECT(ft != core::null && ft.name() == "FindTopic", "topic::find");
    EXPECT(topic::find<topic::Topic<core::ByteSeq> >(dp, "NoSuchTopic") == core::null,
           "topic::find returns a null topic for an unknown name");
    EXPECT(topic::find<topic::AnyTopic>(dp, "FindTopic") != topic::AnyTopic(core::null), "topic::find as AnyTopic");

    // The participant-less discover forms and the iterator form of matched_*.
    {
        std::vector<topic::AnyTopic> any_topics;
        const int n_any = topic::discover<topic::AnyTopic>(std::back_inserter(any_topics));
        EXPECT(n_any >= 0 && static_cast<size_t>(n_any) == any_topics.size(), "discover<AnyTopic> counts what it copies");
        std::vector<topic::TopicBuiltinTopicData> data;
        const uint32_t n_data = topic::discover<topic::TopicBuiltinTopicData>(std::back_inserter(data));
        EXPECT(n_data == data.size(), "discover<TopicBuiltinTopicData> counts what it copies");
        topic::TopicBuiltinTopicData bounded[1];
        EXPECT(topic::discover<topic::TopicBuiltinTopicData>(bounded, 1) <= 1, "discover stops at max_size");
        std::vector<core::InstanceHandle> handles(4);
        const core::InstanceHandleSeq& matched = pub::matched_subscriptions(w1, handles.begin(), handles.end());
        bool same = true;
        for (size_t i = 0; i < matched.size() && i < handles.size(); ++i) {
            same = same && handles[i] == matched[i];
        }
        EXPECT(same, "matched_subscriptions copies the handles into the range");
    }

    threw = false;
    try {
        pub::CoherentSet cs(pub_);
        cs.end();
        pub::SuspendedPublication sp(pub_);
        sp.resume();
        sub::CoherentAccess ca(sub_);
        ca.end();
        pub::CoherentSet scoped(pub_);
    } catch (const core::Exception&) {
        threw = true;
    }
    EXPECT(!threw, "CoherentSet, SuspendedPublication and CoherentAccess begin and end");
}

// DDS-PSM-Cxx 1.0 §7.7: a DomainParticipantListener is the publisher,
// subscriber and topic listener in one; the participant keeps the pointer.
namespace {
struct CountingParticipantListener : dds::domain::NoOpDomainParticipantListener {
    CountingParticipantListener() : inconsistent_topics(0) {}
    int inconsistent_topics;
    void on_inconsistent_topic(dds::topic::AnyTopic&,
                               const dds::core::status::InconsistentTopicStatus&) ZERODDS_OVERRIDE {
        ++inconsistent_topics;
    }
};
} // namespace

static void test_domain_participant_listener() {
    domain::DomainParticipant dp(D(24));
    CountingParticipantListener l;
    dp.listener(&l, core::status::StatusMask::inconsistent_topic());
    EXPECT(dp.listener() == &l, "the participant returns the listener it was given");
    dp.listener(nullptr, core::status::StatusMask::none());
    EXPECT(dp.listener() == nullptr, "a null listener removes the listener");
}

// DDS-PSM-Cxx 1.0: dds::domain::find, the ignore functions and a
// StatusCondition built from an entity.
static void test_find_ignore_status_condition() {
    domain::DomainParticipant dp(D(12));
    domain::DomainParticipant found = domain::find(D(12));
    EXPECT(found == dp, "find returns the participant of the domain id");
    EXPECT(domain::find(D(11)) == core::null, "find returns a null participant for an unknown id");
    core::InstanceHandle h(0x1234);
    bool threw = false;
    try {
        domain::ignore(dp, h);
        const std::vector<core::InstanceHandle> hs(1, h);
        topic::ignore(dp, hs.begin(), hs.end());
        pub::ignore(dp, h);
        sub::ignore(dp, h);
    } catch (const core::Exception&) {
        threw = true;
    }
    EXPECT(!threw, "ignore functions accept a handle on an enabled participant");
    core::cond::StatusCondition sc(dp);
    EXPECT(sc != core::null, "StatusCondition from an entity");
}

// DDS-PSM-Cxx 1.0 §7.3: a loan-based take is exception safe. A decode that
// rejects the sample throws out of take, and the loan it opened is returned
// before the exception propagates — the reader stays usable afterwards.
static void test_take_returns_the_loan_when_decode_throws() {
    domain::DomainParticipant dp(D(16));
    topic::Topic<Throwing> t(dp, "ThrowingTake");
    pub::Publisher pub_(dp);
    const pub::qos::DataWriterQos wq = reliable_keep_all_writer_qos();
    pub::DataWriter<Throwing> dw(pub_, t, wq);
    sub::Subscriber sub_(dp);
    const sub::qos::DataReaderQos rq = reliable_keep_all_reader_qos();
    sub::DataReader<Throwing> dr(sub_, t, rq);
    dw->wait_for_matched(1, core::Duration::from_secs(5));
    dr->wait_for_matched(1, core::Duration::from_secs(5));

    Throwing sample;
    dw.write(sample);

    // Wait for the sample to arrive, then let the decode throw.
    const uint64_t deadline = ::zerodds::cxx::monotonic_microseconds() + 3 * 1000000u;
    bool threw = false;
    while (!threw && ::zerodds::cxx::monotonic_microseconds() < deadline) {
        try {
            (void)dr.take();
        } catch (const std::runtime_error&) {
            threw = true;
        }
        if (!threw) ::zerodds::cxx::sleep_milliseconds(10);
    }
    EXPECT(threw, "a decode that refuses the sample throws out of take");

    // The loan is returned: the reader answers a later take without error
    // (NoData → empty), not a reader left with an open loan.
    sub::LoanedSamples<Throwing> after = dr.take();
    EXPECT(after.length() == 0, "the reader is usable after the thrown decode");
}

// DDS 1.4 §2.2.2.1.1.6 to §2.2.2.1.1.8: enable is idempotent, every entity has
// a handle the participant's contains_entity recognises, and
// get_status_changes reports the triggered statuses (PUBLICATION_MATCHED,
// SUBSCRIPTION_MATCHED, DATA_AVAILABLE) until they are read.
static void test_entity_operations() {
    domain::DomainParticipant dp(D(17));
    topic::Topic<core::ByteSeq> t(dp, "EntityOps");
    pub::Publisher pub_(dp);
    pub::DataWriter<core::ByteSeq> dw(pub_, t);
    sub::Subscriber sub_(dp);
    sub::DataReader<core::ByteSeq> dr(sub_, t);

    dp.enable();
    t.enable();
    pub_.enable();
    sub_.enable();
    dw.enable();
    dw.enable();
    dr.enable();

    const core::InstanceHandle handles[] = {dp.instance_handle(),  t.instance_handle(),
                                            pub_.instance_handle(), sub_.instance_handle(),
                                            dw.instance_handle(),  dr.instance_handle()};
    for (size_t i = 0; i < 6; ++i) {
        EXPECT(!handles[i].is_nil(), "an entity has a handle");
        EXPECT(dp.contains_entity(handles[i]), "contains_entity knows the entity's handle");
        for (size_t j = i + 1; j < 6; ++j) {
            EXPECT(handles[i] != handles[j], "two entities have two handles");
        }
    }
    EXPECT(dp.status_changes().to_ulong() == 0, "a participant has no status change");

    dw->wait_for_matched(1, core::Duration::from_secs(5));
    const uint32_t publication_matched = 1u << 13;
    const uint32_t subscription_matched = 1u << 14;
    const uint32_t data_available = 1u << 10;
    EXPECT((dw.status_changes().to_ulong() & publication_matched) != 0,
           "a match triggers PUBLICATION_MATCHED");
    (void)dw.publication_matched_status();
    EXPECT((dw.status_changes().to_ulong() & publication_matched) == 0,
           "reading the status untriggers it");
    EXPECT((dr.status_changes().to_ulong() & subscription_matched) != 0,
           "a match triggers SUBSCRIPTION_MATCHED");

    core::ByteSeq msg = bytes('x');
    dw.write(msg);
    const uint64_t deadline = ::zerodds::cxx::monotonic_microseconds() + 3 * 1000000u;
    while ((dr.status_changes().to_ulong() & data_available) == 0 &&
           ::zerodds::cxx::monotonic_microseconds() < deadline) {
        ::zerodds::cxx::sleep_milliseconds(10);
    }
    EXPECT((dr.status_changes().to_ulong() & data_available) != 0,
           "an unread sample triggers DATA_AVAILABLE");
}

// DDS-PSM-Cxx 1.0 §7.5.4, §7.12, §7.13: SampleInfo and DataState accessors,
// LoanedSamples as a move-only type with cbegin/cend and a namespace level
// move, SharedSamples, read and take through iterators, select(), and the
// streaming operators.
static void test_samples_api() {
    domain::DomainParticipant dp(D(18));
    topic::Topic<core::ByteSeq> t(dp, "SamplesApi");
    pub::Publisher pub_(dp);
    const pub::qos::DataWriterQos wq = reliable_keep_all_writer_qos();
    pub::DataWriter<core::ByteSeq> dw(pub_, t, wq);
    sub::Subscriber sub_(dp);
    const sub::qos::DataReaderQos rq = reliable_keep_all_reader_qos();
    sub::DataReader<core::ByteSeq> dr(sub_, t, rq);
    dw->wait_for_matched(1, core::Duration::from_secs(5));
    dr->wait_for_matched(1, core::Duration::from_secs(5));

    for (int i = 1; i <= 3; ++i) dw.write(bytes(i));
    const uint64_t deadline = ::zerodds::cxx::monotonic_microseconds() + 5 * 1000000u;
    while (dr.select().state(sub::status::DataState::any()).read().length() < 3 &&
           ::zerodds::cxx::monotonic_microseconds() < deadline) {
        ::zerodds::cxx::sleep_milliseconds(10);
    }
    // The loop above read them: all three are READ now.
    EXPECT(dr.select().state(sub::status::DataState::new_data()).read().length() == 0,
           "read samples are not new data");

    sub::LoanedSamples<core::ByteSeq> two = dr.select().max_samples(2).read();
    EXPECT(two.length() == 2, "max_samples limits a read");
    const sub::SampleInfo info = two.begin()->info();
    EXPECT(info.valid(), "a sample with data is valid");
    EXPECT(info.state().sample_state() == sub::status::SampleState::read(), "READ state");
    EXPECT(info.state().instance_state() == sub::status::InstanceState::alive(), "ALIVE");

    // Copy forms: forward and back-inserting iterators.
    std::vector<sub::Sample<core::ByteSeq> > fixed(3);
    EXPECT(dr.read(fixed.begin(), 3) == 3, "read into a forward iterator");
    EXPECT(fixed[2].data() == bytes(3), "the third sample is the third written");
    std::vector<sub::Sample<core::ByteSeq> > back;
    EXPECT(dr.select().max_samples(2).read(std::back_inserter(back)) == 2,
           "read through a back-inserter honours max_samples");
    EXPECT(back.size() == 2, "two samples were inserted");

    // LoanedSamples moves (in C++11 the moved-from object is empty).
    sub::LoanedSamples<core::ByteSeq> moved = sub::move(two);
    EXPECT(moved.length() == 2, "move gives the samples");
#if ZERODDS_CXX11
    // DDS-PSM-Cxx 1.0 §7.12 (LoanedSamples is move-only in C++11; C++03 copies): C++11 only.
    EXPECT(two.length() == 0, "the moved-from LoanedSamples is empty");
#endif
    size_t seen = 0;
    for (sub::LoanedSamples<core::ByteSeq>::const_iterator it = moved.cbegin(); it != moved.cend(); ++it) ++seen;
    EXPECT(seen == 2, "cbegin/cend walk the samples");

    // SharedSamples: copies share the samples, which outlive the original.
    sub::SharedSamples<core::ByteSeq> shared(ZERODDS_MOVE(moved));
    sub::SharedSamples<core::ByteSeq> copy = shared;
    EXPECT(copy.length() == 2 && shared.length() == 2, "SharedSamples copies share the samples");
    size_t range = 0;
    for (sub::SharedSamples<core::ByteSeq>::const_iterator s = copy.begin(); s != copy.end(); ++s) {
        ++range;
    }
    EXPECT(range == 2, "iteration over SharedSamples");
#if ZERODDS_CXX11
    // DDS-PSM-Cxx 1.0 §7.12 (range-based for loops are C++11): C++11 only.
    range = 0;
    for (const auto& s : copy) {
        (void)s;
        ++range;
    }
    EXPECT(range == 2, "range-based for over SharedSamples");
#endif

    // Streaming: `dr >> read >> ls` reads, `dr >> max_samples(1) >> take >> ls` takes one.
    sub::LoanedSamples<core::ByteSeq> streamed;
    dr >> sub::read >> streamed;
    EXPECT(streamed.length() == 3, "the streaming read returns everything");
    dr >> sub::max_samples(1) >> sub::take >> streamed;
    EXPECT(streamed.length() == 1, "the streaming take honours max_samples");
    EXPECT(dr.select().read().length() == 2, "the take removed one sample");
}

// DDS-PSM-Cxx 1.0 §7.5.3, §7.8: Topic with a type name and a listener,
// AnyTopic and AnyTopicDescription hold a topic of any type and give it back
// as the topic it is, the participant of a topic, the topic of a
// ContentFilteredTopic.
struct CountingTopicListener : topic::NoOpTopicListener<core::ByteSeq> {};

static void test_topic_any_topic_and_listener() {
    domain::DomainParticipant dp(D(20));
    topic::Topic<core::ByteSeq> t(dp, "AnyTopicProbe", "DDS::Bytes");
    EXPECT(t.type_name() == "DDS::Bytes", "a topic created under a type name");
    EXPECT(t.domain_participant().domain_id() == D(20), "the participant of a topic");

    CountingTopicListener listener;
    t.listener(&listener, core::status::StatusMask::all());
    EXPECT(t.listener() == &listener, "the installed listener");
    t.listener(nullptr, core::status::StatusMask::none());
    EXPECT(t.listener() == nullptr, "the listener is removed");

    topic::AnyTopic any(t);
    EXPECT(any->name() == "AnyTopicProbe", "AnyTopic names its topic");
    EXPECT(any.domain_participant().domain_id() == D(20), "AnyTopic knows the participant");
    EXPECT(any.inconsistent_topic_status().total_count() == 0, "no inconsistent topic");
    topic::Topic<core::ByteSeq> back = any.get<core::ByteSeq>();
    EXPECT(back.name() == "AnyTopicProbe", "get<T>() gives the Topic back");
    bool wrong_type = false;
    try {
        (void)any.get<std::string>();
    } catch (const core::InvalidDowncastError&) {
        wrong_type = true;
    }
    EXPECT(wrong_type, "get<T>() of another type is an InvalidDowncastError");

    topic::AnyTopic found = dp->find_topic("AnyTopicProbe");
    EXPECT(found->type_name() == "DDS::Bytes", "find_topic finds the topic");
    EXPECT(found.get<core::ByteSeq>().name() == "AnyTopicProbe", "a found topic gets its type");

    topic::AnyTopicDescription description(t);
    EXPECT(description.name() == "AnyTopicProbe", "AnyTopicDescription names the topic");

    topic::ContentFilteredTopic<core::ByteSeq> cft(t, "AnyTopicCft", topic::Filter("1 = 1"));
    EXPECT(cft.topic().name() == "AnyTopicProbe", "the topic a ContentFilteredTopic filters");
    EXPECT(cft.domain_participant().domain_id() == D(20), "the participant of a ContentFilteredTopic");
}

// DDS-PSM-Cxx 1.0 §7.9, §7.10: Publisher and Subscriber QoS, default QoS of
// their endpoints, listener, participant; the DataWriter and DataReader
// navigation and QoS; the write forms; AnyDataWriter and AnyDataReader.
struct PubListener : pub::NoOpPublisherListener {};
struct SubListener : sub::NoOpSubscriberListener {};

static void test_publisher_subscriber_datawriter_api() {
    namespace p = core::policy;
    domain::DomainParticipant dp(D(21));

    pub::qos::PublisherQos pq;
    pq << p::Partition("Tracks");
    pub::Publisher pub_(dp, pq);
    EXPECT(pub_.qos().policy<p::Partition>().name().size() == 1 &&
               pub_.qos().policy<p::Partition>().name()[0] == "Tracks",
           "the QoS the Publisher was made with");
    EXPECT(pub_.participant().domain_id() == D(21), "the participant of a Publisher");
    pub::qos::DataWriterQos dwdefault;
    dwdefault << p::History::KeepLast(7);
    pub_.default_writer_qos(dwdefault);
    EXPECT(pub_.default_writer_qos().policy<p::History>().depth() == 7,
           "the default DataWriterQos of a Publisher");
    PubListener pl;
    pub_.listener(&pl, core::status::StatusMask::all());
    EXPECT(pub_.listener() == &pl, "the Publisher's listener");
    pub_.listener(nullptr, core::status::StatusMask::none());
    EXPECT(pub_.listener() == nullptr, "the Publisher's listener is removed");

    sub::qos::SubscriberQos sq;
    sq << p::Partition("Tracks");
    sub::Subscriber sub_(dp, sq);
    EXPECT(sub_.qos().policy<p::Partition>().name()[0] == "Tracks", "Subscriber QoS");
    EXPECT(sub_.participant().domain_id() == D(21), "the participant of a Subscriber");
    sub::qos::DataReaderQos drdefault;
    drdefault << p::History::KeepLast(9);
    sub_.default_datareader_qos(drdefault);
    EXPECT(sub_.default_datareader_qos().policy<p::History>().depth() == 9,
           "the default DataReaderQos of a Subscriber");
    SubListener sl;
    sub_.listener(&sl, core::status::StatusMask::all());
    EXPECT(sub_.listener() == &sl, "the Subscriber's listener");
    sub_.listener(nullptr, core::status::StatusMask::none());
    sub_.notify_datareaders();

    topic::Topic<core::ByteSeq> t(dp, "PubSubApi");
    pub::qos::DataWriterQos wq;
    wq << p::Reliability::Reliable() << p::History::KeepAll();
    pub::DataWriter<core::ByteSeq> dw(pub_, t, wq);
    sub::qos::DataReaderQos rq;
    rq << p::Reliability::Reliable() << p::History::KeepAll();
    sub::DataReader<core::ByteSeq> dr(sub_, t, rq);
    EXPECT(dw.topic().name() == "PubSubApi", "the topic of a DataWriter");
    EXPECT(dw.publisher().participant().domain_id() == D(21), "the publisher of a DataWriter");
    EXPECT(dw.qos().policy<p::History>().kind() == p::HistoryKind::KEEP_ALL, "DataWriter QoS");
    EXPECT(dr.topic_description().name() == "PubSubApi", "the topic description of a DataReader");
    EXPECT(dr.subscriber().participant().domain_id() == D(21), "the subscriber of a DataReader");
    EXPECT(dr.qos().policy<p::History>().kind() == p::HistoryKind::KEEP_ALL, "DataReader QoS");
    dw->wait_for_matched(1, core::Duration::from_secs(5));
    dr->wait_for_matched(1, core::Duration::from_secs(5));
    EXPECT(dw.publication_matched_status().current_count() == 1, "PublicationMatchedStatus");
    EXPECT(dr.subscription_matched_status().current_count() == 1, "SubscriptionMatchedStatus");

    // The write forms: one sample, with a time stamp, the stream operator,
    // a range, a pair with a time stamp.
    dw.write(bytes(1));
    dw.write(bytes(2), core::Time(10, 0));
    dw << bytes(3);
    std::vector<core::ByteSeq> range;
    range.push_back(bytes(4));
    range.push_back(bytes(5));
    dw.write(range.begin(), range.end());
    dw << std::make_pair(bytes(6), core::Time(11, 0));
    const uint64_t deadline = ::zerodds::cxx::monotonic_microseconds() + 5 * 1000000u;
    while (dr.select().read().length() < 6 && ::zerodds::cxx::monotonic_microseconds() < deadline) {
        ::zerodds::cxx::sleep_milliseconds(10);
    }
    EXPECT(dr.select().read().length() == 6, "every form wrote its sample");

    // AnyDataWriter and AnyDataReader.
    pub::AnyDataWriter adw(dw);
    EXPECT(adw->topic_name() == "PubSubApi", "AnyDataWriter names its topic");
    EXPECT(adw.get<core::ByteSeq>()->native_handle() == dw->native_handle(),
           "get<T>() gives the DataWriter back");
    bool wrong_writer = false;
    try {
        (void)adw.get<std::string>();
    } catch (const core::InvalidDowncastError&) {
        wrong_writer = true;
    }
    EXPECT(wrong_writer, "AnyDataWriter::get of another type");
    sub::AnyDataReader adr(dr);
    EXPECT(adr->topic_name() == "PubSubApi", "AnyDataReader names its topic");
    EXPECT(adr.get<core::ByteSeq>()->native_handle() == dr->native_handle(),
           "get<T>() gives the DataReader back");
}

// DDS-PSM-Cxx 1.0 §7.6.2.1: a QosProvider loads a DDS-XML QoS document from a
// file URI and gives the QoS of each entity kind, with profile inheritance;
// another URI scheme is UNSUPPORTED.
static void test_qos_provider() {
    namespace p = core::policy;
    const std::string path = "/tmp/zerodds_smoke_qos_" + ::zerodds::cxx::to_string(static_cast<int>(::getpid())) + ".xml";
    {
        std::ofstream xml(path.c_str());
        xml << "<dds><qos_library name=\"L\">"
               "<qos_profile name=\"Base\">"
               "<topic_qos><topic_data><value>dG9waWNfZGF0YQ==</value></topic_data>"
               "<durability><kind>TRANSIENT_LOCAL_DURABILITY_QOS</kind></durability></topic_qos>"
               "<publisher_qos><partition><name><element>Tracks</element></name></partition></publisher_qos>"
               "<datawriter_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>"
               "<history><kind>KEEP_LAST_HISTORY_QOS</kind><depth>5</depth></history></datawriter_qos>"
               "</qos_profile>"
               "<qos_profile name=\"Child\" base_name=\"Base\">"
               "<datawriter_qos><history><kind>KEEP_ALL_HISTORY_QOS</kind></history></datawriter_qos>"
               "</qos_profile></qos_library></dds>";
    }
    core::QosProvider provider("file://" + path, "L::Child");
    pub::qos::DataWriterQos dwqos = provider.datawriter_qos();
    EXPECT(dwqos.policy<p::Reliability>().kind() == p::ReliabilityKind::RELIABLE,
           "the Reliability of the base profile");
    EXPECT(dwqos.policy<p::History>().kind() == p::HistoryKind::KEEP_ALL,
           "the History the child profile overrides");
    EXPECT(provider.datawriter_qos("L::Base").policy<p::History>().depth() == 5,
           "a QoS by profile id");
    topic::qos::TopicQos tqos = provider.topic_qos();
    EXPECT(tqos.policy<p::Durability>().kind() == p::DurabilityKind::TRANSIENT_LOCAL,
           "the topic QoS of the profile");
    static const char kTopicData[] = "topic_data";
    EXPECT(tqos.policy<p::TopicData>().value() == core::ByteSeq(kTopicData, kTopicData + 10),
           "the TOPIC_DATA of the profile");
    EXPECT(provider.publisher_qos().policy<p::Partition>().name()[0] == "Tracks",
           "the PARTITION of the publisher QoS");
    // The QoS builds an entity, as in the specification's example.
    domain::DomainParticipant dp(D(22));
    topic::Topic<core::ByteSeq> t(dp, "QosProviderTopic", tqos);
    pub::Publisher pub_(dp, provider.publisher_qos());
    pub::DataWriter<core::ByteSeq> dw(pub_, t, dwqos);
    EXPECT(dw.qos().policy<p::History>().kind() == p::HistoryKind::KEEP_ALL,
           "a writer made with the provider's QoS");

    bool unsupported = false;
    try {
        core::QosProvider gopher("gopher://example.org/qos.xml");
    } catch (const core::UnsupportedError&) {
        unsupported = true;
    }
    EXPECT(unsupported, "a scheme no specification defines is UnsupportedError");
    {
        // A data: URI (RFC 2397) names the document itself.
        core::QosProvider inline_doc(
            "data:text/xml,%3Cdds%3E%3Cqos_library%20name%3D%22L%22%3E%3Cqos_profile%20name%3D%22P%22%3E"
            "%3Cdatawriter_qos%3E%3Cwriter_data_lifecycle%3E%3Cautodispose_unregistered_instances%3Efalse"
            "%3C%2Fautodispose_unregistered_instances%3E%3C%2Fwriter_data_lifecycle%3E%3C%2Fdatawriter_qos%3E"
            "%3C%2Fqos_profile%3E%3C%2Fqos_library%3E%3C%2Fdds%3E",
            "L::P");
        EXPECT(!inline_doc.datawriter_qos().policy<core::policy::WriterDataLifecycle>().autodispose_unregistered_instances(),
               "a data: URI is read as the document");
    }
    bool bad_profile = false;
    try {
        core::QosProvider missing("file://" + path, "L::Nope");
    } catch (const core::InvalidArgumentError&) {
        bad_profile = true;
    }
    EXPECT(bad_profile, "a profile the document lacks is InvalidArgumentError");
    std::remove(path.c_str());
}









































// ---- tests added for the re-evaluation of the coverage (generated block) ----

// The enumeration of the safe_enum example (a local type is no template argument in C++03).
struct Color_def {
    enum type { GREEN, WHITE, RED };
};

// DDS-PSM-Cxx 1.0 §2.1, §7.5.1, §7.5.4, §7.5.6, §7.6.1: operations and accessors of the value and
// reference classes that no other test in this file calls.
static void test_value_and_reference_classes_without_other_coverage() {
    namespace p = core::policy;
    namespace st = core::status;

    // safe_enum with the example of DDS-PSM-Cxx 1.0 §7.4.3.
    typedef core::safe_enum<Color_def> Color;
    const Color green(Color::GREEN), red(Color::RED);
    EXPECT(green.underlying() == Color::GREEN && red.underlying() == Color::RED, "safe_enum::underlying");
    EXPECT(green == Color(Color::GREEN) && green != red, "safe_enum == and !=");
    EXPECT(green < red && green <= red && red > green && red >= green && green <= green && green >= green &&
               !(red < green) && !(green > red),
           "safe_enum <, <=, >, >=");
    ZERODDS_STATIC_ASSERT((!::zerodds::cxx::is_convertible<int, Color>::value), "an integer does not convert to a safe_enum");
    ZERODDS_STATIC_ASSERT((!::zerodds::cxx::is_convertible<Color, int>::value), "a safe_enum does not convert to an integer");

    // Time and Duration: ordering, the invalid time, the infinite duration.
    EXPECT(core::Time::invalid().sec() == -1, "the invalid time has seconds -1");
    EXPECT(core::Time(5, 0).compare(core::Time(5, 1)) < 0, "Time::compare orders by nanoseconds");
    EXPECT(core::Time(6, 0).compare(core::Time(5, 9)) > 0, "Time::compare orders by seconds");
    EXPECT(core::Time(5, 7).compare(core::Time(5, 7)) == 0, "Time::compare of equal times");
    EXPECT(core::Duration(1, 0).compare(core::Duration::infinite()) < 0, "a duration is below infinite");
    EXPECT(core::Duration::infinite().compare(core::Duration::infinite()) == 0, "infinite equals infinite");

    // InstanceHandle: nil.
    EXPECT(core::InstanceHandle::nil().is_nil() && core::InstanceHandle().is_nil(),
           "InstanceHandle::nil() is nil");
    EXPECT(!core::InstanceHandle(7).is_nil() && core::InstanceHandle(7).value() == 7,
           "a handle with a value is not nil");

    // WeakReference: lock() gives the entity while it lives, expired() after the last reference.
    {
        domain::DomainParticipant dp(D(25));
        core::WeakReference<domain::DomainParticipant> weak(dp);
        EXPECT(!weak.expired(), "a weak reference to a live participant");
        domain::DomainParticipant locked = weak.lock();
        EXPECT(locked.domain_id() == D(25), "lock() gives the participant");
    }

    // The status masks of the specification and get_status<>.
    EXPECT(st::StatusMask::data_on_readers().to_ulong() == (1u << 9), "DATA_ON_READERS");
    EXPECT(st::StatusMask::liveliness_changed().to_ulong() == (1u << 12), "LIVELINESS_CHANGED");
    EXPECT(st::StatusMask::offered_deadline_missed().to_ulong() == (1u << 1), "OFFERED_DEADLINE_MISSED");
    EXPECT(st::StatusMask::offered_incompatible_qos().to_ulong() == (1u << 5),
           "OFFERED_INCOMPATIBLE_QOS");
    EXPECT(st::StatusMask::requested_deadline_missed().to_ulong() == (1u << 2),
           "REQUESTED_DEADLINE_MISSED");
    EXPECT(st::StatusMask::requested_incompatible_qos().to_ulong() == (1u << 6),
           "REQUESTED_INCOMPATIBLE_QOS");
    EXPECT(st::StatusMask::sample_lost().to_ulong() == (1u << 7), "SAMPLE_LOST");
    EXPECT(st::StatusMask::sample_rejected().to_ulong() == (1u << 8), "SAMPLE_REJECTED");
    EXPECT(st::get_status<st::SampleLostStatus>() == st::StatusMask::sample_lost(),
           "get_status<SampleLostStatus>");
    EXPECT(st::get_status<st::PublicationMatchedStatus>() == st::StatusMask::publication_matched(),
           "get_status<PublicationMatchedStatus>");
    EXPECT(st::get_status<st::RequestedDeadlineMissedStatus>() ==
               st::StatusMask::requested_deadline_missed(),
           "get_status<RequestedDeadlineMissedStatus>");

    // The reasons of a rejected sample.
    EXPECT(st::SampleRejectedState::not_rejected() != st::SampleRejectedState::rejected_by_samples_limit(),
           "two rejection reasons differ");
    EXPECT(st::SampleRejectedState::rejected_by_instances_limit() !=
               st::SampleRejectedState::rejected_by_samples_per_instance_limit(),
           "two more rejection reasons differ");

    // Status classes: the fields the constructors set.
    st::RequestedIncompatibleQosStatus riq;
    EXPECT(riq.total_count() == 0 && riq.total_count_change() == 0, "an incompatible-QoS status starts at zero");
    EXPECT(riq.policies().empty(), "and lists no policy");
    st::LivelinessChangedStatus lc;
    EXPECT(lc.alive_count() == 0 && lc.alive_count_change() == 0 && lc.not_alive_count() == 0 &&
               lc.not_alive_count_change() == 0,
           "a liveliness-changed status starts at zero");
    EXPECT(lc.last_publication_handle().is_nil(), "with no publication");
    st::SubscriptionMatchedStatus sm;
    EXPECT(sm.current_count_change() == 0 && sm.last_publication_handle().is_nil(),
           "a subscription-matched status starts at zero");
    st::PublicationMatchedStatus pm;
    EXPECT(pm.last_subscription_handle().is_nil(), "a publication-matched status has no subscription");
    st::SampleRejectedStatus sr;
    EXPECT(sr.last_reason() == st::SampleRejectedState::not_rejected() && sr.last_instance_handle().is_nil(),
           "a sample-rejected status starts not rejected");
    st::OfferedDeadlineMissedStatus odm;
    EXPECT(odm.total_count_change() == 0 && odm.last_instance_handle().is_nil(), "no missed deadline");

    // Every status class built from values: constructors, getters and setters.
    st::InconsistentTopicStatus inconsistent(4, 1);
    EXPECT(inconsistent.total_count() == 4 && inconsistent.total_count_change() == 1, "InconsistentTopicStatus");
    st::SampleLostStatus lost(3, 2);
    EXPECT(lost.total_count() == 3 && lost.total_count_change() == 2, "SampleLostStatus built from values");
    lost.total_count(9);
    lost.total_count() += 1;
    EXPECT(lost.total_count() == 10, "SampleLostStatus: the setter and the reference accessor");
    st::LivelinessLostStatus liveliness_lost(2, 1);
    EXPECT(liveliness_lost.total_count() == 2, "LivelinessLostStatus");
    st::RequestedDeadlineMissedStatus requested_deadline(5, 1, core::InstanceHandle(11));
    EXPECT(requested_deadline.total_count() == 5 && requested_deadline.last_instance_handle().value() == 11,
           "RequestedDeadlineMissedStatus");
    st::OfferedDeadlineMissedStatus offered_deadline(6, 2, core::InstanceHandle(12));
    EXPECT(offered_deadline.total_count_change() == 2 && offered_deadline.last_instance_handle().value() == 12,
           "OfferedDeadlineMissedStatus");
    st::PublicationMatchedStatus publication(7, 1, 2, 1, core::InstanceHandle(13));
    EXPECT(publication.total_count() == 7 && publication.current_count() == 2 &&
               publication.current_count_change() == 1 && publication.last_subscription_handle().value() == 13,
           "PublicationMatchedStatus");
    st::SubscriptionMatchedStatus subscription(8, 1, 3, 1, core::InstanceHandle(14));
    EXPECT(subscription.current_count() == 3 && subscription.last_publication_handle().value() == 14,
           "SubscriptionMatchedStatus");
    st::LivelinessChangedStatus changed(1, 2, 3, 4, core::InstanceHandle(15));
    EXPECT(changed.alive_count() == 1 && changed.not_alive_count() == 2 && changed.alive_count_change() == 3 &&
               changed.not_alive_count_change() == 4 && changed.last_publication_handle().value() == 15,
           "LivelinessChangedStatus");
    st::SampleRejectedStatus rejected(2, 1, st::SampleRejectedState::rejected_by_samples_limit(),
                                      core::InstanceHandle(16));
    EXPECT(rejected.total_count() == 2 &&
               rejected.last_reason() == st::SampleRejectedState::rejected_by_samples_limit() &&
               rejected.last_instance_handle().value() == 16,
           "SampleRejectedStatus");
    p::QosPolicyCountSeq counted;
    counted.push_back(p::QosPolicyCount(3, 2));
    st::OfferedIncompatibleQosStatus offered_incompatible(1, 1, 3, counted);
    EXPECT(offered_incompatible.last_policy_id() == 3 && offered_incompatible.policies().size() == 1 &&
               offered_incompatible.policies()[0].count() == 2,
           "OfferedIncompatibleQosStatus lists the policy counts");
    st::RequestedIncompatibleQosStatus requested_incompatible(2, 1, 4, counted);
    EXPECT(requested_incompatible.total_count() == 2 && requested_incompatible.policies().size() == 1,
           "RequestedIncompatibleQosStatus lists the policy counts");

#if ZERODDS_CXX11
    // DDS-PSM-Cxx 1.0 §7.12 (move constructors and move assignment of the value types are C++11): C++11 only.
    // Value types move: the data go over, the source is left empty.
    p::UserData move_source(core::ByteSeq(1000, 1));
    p::UserData move_target(std::move(move_source));
    EXPECT(move_target.value().size() == 1000 && move_source.value().empty(), "a policy moves");
    pub::qos::DataWriterQos qos_source;
    qos_source << p::UserData(core::ByteSeq(10, 2));
    pub::qos::DataWriterQos qos_target(std::move(qos_source));
    EXPECT(qos_target.policy<p::UserData>().value().size() == 10, "a QoS moves");
    pub::qos::DataWriterQos qos_assigned;
    qos_assigned = std::move(qos_target);
    EXPECT(qos_assigned.policy<p::UserData>().value().size() == 10, "and moves by assignment");
    ZERODDS_STATIC_ASSERT((std::is_move_constructible<st::OfferedIncompatibleQosStatus>::value &&
                      std::is_move_assignable<st::OfferedIncompatibleQosStatus>::value &&
                      std::is_move_constructible<sub::SampleInfo>::value &&
                      std::is_move_constructible<core::InstanceHandle>::value &&
                      std::is_move_constructible<p::History>::value),
                  "the value types are move constructible and assignable");

    // LoanedSamples is move-only (C++11 only) and offers begin/end at namespace level (§7.12).
    ZERODDS_STATIC_ASSERT((!std::is_copy_constructible<sub::LoanedSamples<core::ByteSeq> >::value &&
                      std::is_move_constructible<sub::LoanedSamples<core::ByteSeq> >::value),
                  "LoanedSamples is a move-only type");
#endif
    sub::LoanedSamples<core::ByteSeq> no_samples;
    EXPECT(core::begin(no_samples) == core::end(no_samples) && sub::cbegin(no_samples) == sub::cend(no_samples),
           "begin and end at namespace level");

    // Value types copy deeply: a change of the copy does not reach the original.
    pub::qos::DataWriterQos original_qos;
    pub::qos::DataWriterQos copied_qos = original_qos;
    copied_qos << p::History::KeepLast(33) << p::UserData(bytes(5));
    EXPECT(original_qos.policy<p::History>().depth() != 33 && original_qos.policy<p::UserData>().value().empty(),
           "a copy of a QoS is independent of the original");
    pub::qos::DataWriterQos assigned_qos;
    assigned_qos = copied_qos;
    copied_qos << p::History::KeepLast(34);
    EXPECT(assigned_qos.policy<p::History>().depth() == 33, "so is the target of an assignment");

    // The static helpers and accessors of the policies.
    EXPECT(p::Liveliness::Automatic().kind() == p::LivelinessKind::AUTOMATIC, "Liveliness::Automatic");
    EXPECT(p::Liveliness::ManualByParticipant(core::Duration(1, 0)).kind() ==
               p::LivelinessKind::MANUAL_BY_PARTICIPANT,
           "Liveliness::ManualByParticipant");
    EXPECT(p::Liveliness::ManualByTopic(core::Duration(1, 0)).kind() == p::LivelinessKind::MANUAL_BY_TOPIC,
           "Liveliness::ManualByTopic");
    EXPECT(p::Reliability::BestEffort().kind() == p::ReliabilityKind::BEST_EFFORT, "Reliability::BestEffort");
    EXPECT(p::Durability::Volatile().kind() == p::DurabilityKind::VOLATILE, "Durability::Volatile");
    EXPECT(p::Durability::TransientLocal().kind() == p::DurabilityKind::TRANSIENT_LOCAL,
           "Durability::TransientLocal");
    EXPECT(p::Durability::Persistent().kind() == p::DurabilityKind::PERSISTENT, "Durability::Persistent");
    EXPECT(p::Ownership::Shared().kind() == p::OwnershipKind::SHARED, "Ownership::Shared");
    EXPECT(p::DestinationOrder::SourceTimestamp().kind() == p::DestinationOrderKind::BY_SOURCE_TIMESTAMP,
           "DestinationOrder::SourceTimestamp");
    EXPECT(p::DestinationOrder::ReceptionTimestamp().kind() ==
               p::DestinationOrderKind::BY_RECEPTION_TIMESTAMP,
           "DestinationOrder::ReceptionTimestamp");
    EXPECT(p::Presentation::InstanceAccessScope(true, true).access_scope() ==
               p::PresentationAccessScopeKind::INSTANCE,
           "Presentation::InstanceAccessScope");
    EXPECT(p::Presentation::TopicAccessScope(true, false).access_scope() ==
                   p::PresentationAccessScopeKind::TOPIC &&
               p::Presentation::TopicAccessScope(true, false).coherent_access() &&
               !p::Presentation::TopicAccessScope(true, false).ordered_access(),
           "Presentation::TopicAccessScope");
    EXPECT(p::EntityFactory::AutoEnable().autoenable_created_entities(), "EntityFactory::AutoEnable");
    EXPECT(p::WriterDataLifecycle::AutoDisposeUnregisteredInstances().autodispose_unregistered_instances(),
           "WriterDataLifecycle::AutoDisposeUnregisteredInstances");
    EXPECT(!p::WriterDataLifecycle::ManuallyDisposeUnregisteredInstances().autodispose_unregistered_instances(),
           "WriterDataLifecycle::ManuallyDisposeUnregisteredInstances");
    EXPECT(p::ReaderDataLifecycle::NoAutoPurgeDisposedSamples().autopurge_disposed_samples_delay() ==
               core::Duration::infinite(),
           "ReaderDataLifecycle::NoAutoPurgeDisposedSamples");
    EXPECT(p::ReaderDataLifecycle::AutoPurgeDisposedSamples(core::Duration(3, 0))
                   .autopurge_disposed_samples_delay() == core::Duration(3, 0),
           "ReaderDataLifecycle::AutoPurgeDisposedSamples");
    p::ReaderDataLifecycle rdl;
    rdl.autopurge_nowriter_samples_delay(core::Duration(4, 0));
    EXPECT(rdl.autopurge_nowriter_samples_delay() == core::Duration(4, 0), "autopurge_nowriter_samples_delay");
    p::DurabilityService ds;
    ds.service_cleanup_delay(core::Duration(2, 0));
    EXPECT(ds.service_cleanup_delay() == core::Duration(2, 0), "service_cleanup_delay");

    // The policies and kinds of DDS-XTypes, and the traits of the policies.
    p::DataRepresentationIdSeq representation_ids(1, static_cast<p::DataRepresentationIdSeq::value_type>(2));
    p::DataRepresentation representation(representation_ids);
    EXPECT(representation.value().size() == 1 && representation.value()[0] == 2, "DataRepresentation value");
    p::TypeConsistencyEnforcement consistency(
        p::TypeConsistencyEnforcementKind::EXACT_NAME_TYPE_CONSISTENCY);
    EXPECT(consistency.kind() == p::TypeConsistencyEnforcementKind::EXACT_NAME_TYPE_CONSISTENCY,
           "TypeConsistencyEnforcement kind");
    EXPECT(p::TypeConsistencyEnforcement().kind() == p::TypeConsistencyEnforcementKind::ASSIGNABLE_TYPE_CONSISTENCY,
           "the default of TypeConsistencyEnforcement is ASSIGNABLE_TYPE_CONSISTENCY");
    EXPECT(p::policy_id<p::DataRepresentation>::value == 23 &&
               p::policy_name<p::TypeConsistencyEnforcement>::name() == "TypeConsistencyEnforcement",
           "policy_id and policy_name of the DDS-XTypes policies");

    // The type traits of dds/topic/TopicTraits.hpp.
    EXPECT(topic::is_topic_type<core::ByteSeq>::value == 1 && topic::is_topic_type<Untyped>::value == 0,
           "is_topic_type");
    EXPECT(topic::topic_type_name<core::ByteSeq>::value() == "DDS::Bytes" &&
               topic::topic_type_name<Untyped>::value() == "Untyped",
           "topic_type_name takes the name of the topic_type_support");
    EXPECT(topic::topic_type_name<int>::value() == "Undefined", "topic_type_name of a type without support");

    // dds::core::array: get<>, fill, front, operator<.
    core::array<int, 3> a = {{1, 2, 3}};
    EXPECT(core::get<1>(a) == 2, "array get<I>");
    a.fill(9);
    EXPECT(a.front() == 9, "array fill and front");
    core::array<int, 2> b = {{1, 2}};
    core::array<int, 2> c = {{1, 3}};
    EXPECT(b < c && !(c < b), "array operator<");
#if ZERODDS_CXX11
    // DDS-PSM-Cxx 1.0 §7.12 (dds::core::array is std::array in C++11: tuple interface, reverse
    // iterators, swap): C++11 only.
    EXPECT((std::tuple_size<core::array<int, 3> >::value == 3), "array tuple_size");
    EXPECT(*a.crbegin() == 9 && a.rbegin() != a.rend() && a.crend() - a.crbegin() == 3,
           "array reverse iterators");
    b.swap(c);
    EXPECT(b[1] == 3 && c[1] == 2, "array swap");
#endif

    // The type of the participant data of the built-in topics under its second name.
    topic::ParticipantTopicData participant_data;
    EXPECT(participant_data.user_data().value().empty(), "ParticipantTopicData is ParticipantBuiltinTopicData");
}

// DDS-PSM-Cxx 1.0 §7.9: the instance operations of a keyed writer with and without a time stamp, on
// a built-in topic type with a key.
static void test_instance_operations_with_a_time_stamp() {
    domain::DomainParticipant dp(D(36));
    topic::Topic<core::KeyedStringTopicType> t(dp, "InstanceOperations");
    pub::qos::DataWriterQos wq;
    wq << core::policy::Reliability::Reliable() << core::policy::History::KeepAll();
    sub::qos::DataReaderQos rq;
    rq << core::policy::Reliability::Reliable() << core::policy::History::KeepAll();
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    pub::DataWriter<core::KeyedStringTopicType> dw(pub_, t, wq);
    sub::DataReader<core::KeyedStringTopicType> dr(sub_, t, rq);
    dw->wait_for_matched(1, core::Duration::from_secs(5));
    dr->wait_for_matched(1, core::Duration::from_secs(5));

    const core::KeyedStringTopicType a("a", "1");
    const core::InstanceHandle h = dw.register_instance(a, core::Time(5, 0));
    EXPECT(!h.is_nil(), "register_instance with a time stamp gives a handle");
    EXPECT(dw.lookup_instance(a) == h, "lookup_instance finds it");
    EXPECT(dw.register_instance(a) == h, "register_instance of a registered key gives the same handle");
    core::KeyedStringTopicType key_out;
    dw.key_value(key_out, h);
    EXPECT(key_out.key() == "a", "key_value gives the key of the instance");
    topic::TopicInstance<core::KeyedStringTopicType> instance(h, key_out);
    dw.key_value(instance, h);
    EXPECT(instance.handle() == h && instance.sample().key() == "a", "key_value into a TopicInstance");

    dw.write(a, h);
    dw.write(a, h, core::Time(6, 0));
    dw.write(topic::TopicInstance<core::KeyedStringTopicType>(h, a));
    dw.write(topic::TopicInstance<core::KeyedStringTopicType>(h, a), core::Time(6, 1));
    std::vector<core::KeyedStringTopicType> samples;
    samples.push_back(core::KeyedStringTopicType("a", "2"));
    samples.push_back(core::KeyedStringTopicType("a", "3"));
    const std::vector<core::InstanceHandle> handles(2, h);
    dw.write(samples.begin(), samples.end(), handles.begin(), handles.end());
    dw.write(samples.begin(), samples.end(), core::Time(6, 2));

    // The reader knows the instance: its handle, its key, the samples of that instance.
    core::InstanceHandle reader_handle;
    const uint64_t arrival = ::zerodds::cxx::monotonic_microseconds() + 5 * 1000000u;
    while (reader_handle.is_nil() && ::zerodds::cxx::monotonic_microseconds() < arrival) {
        // The reader knows an instance from the first sample it hands out (read leaves it).
        (void)dr.read();
        reader_handle = dr.lookup_instance(a);
        ::zerodds::cxx::sleep_milliseconds(20);
    }
    EXPECT(!reader_handle.is_nil(), "the reader looks up the instance the writer registered");
    core::KeyedStringTopicType reader_key;
    dr.key_value(reader_key, reader_handle);
    EXPECT(reader_key.key() == "a", "the reader's key_value gives the key");
    EXPECT(dr.key_value(reader_handle).sample().key() == "a", "and as a TopicInstance");
    EXPECT(dr.select().instance(reader_handle).read().length() >= 1, "read of one instance");
    EXPECT(dr.select().next_instance(core::InstanceHandle()).read().length() >= 1,
           "read of the next instance after the nil handle");
    EXPECT(dr.select().instance(reader_handle).take().length() >= 1, "take of one instance");

    dw.dispose_instance(h, core::Time(7, 0));
    dw.unregister_instance(h, core::Time(8, 0));
    const core::InstanceHandle b = dw.register_instance(core::KeyedStringTopicType("b", "1"));
    dw.dispose_instance(b);
    dw.unregister_instance(b);
    EXPECT(b != h, "another key is another instance");

    bool unknown = false;
    try {
        dw.key_value(key_out, core::InstanceHandle(424242));
    } catch (const core::Exception&) {
        unknown = true;
    }
    EXPECT(unknown, "key_value of a handle that names no instance of the writer is an exception");
}

#if ZERODDS_CXX11
// DDS-PSM-Cxx 1.0 §7.12 (std::thread, std::atomic and lambdas are the C++11 additions; C++03 has no
// thread library): the three tests with threads are C++11 only.

// DDS-PSM-Cxx 1.0 §7.3: the operations of a DataWriter, a DataReader, a Publisher, a Subscriber and
// a DomainParticipant can be called from several threads at once.
static void test_operations_are_called_from_several_threads() {
    domain::DomainParticipant dp(D(37));
    topic::Topic<core::ByteSeq> t(dp, "Reentrant");
    pub::qos::DataWriterQos wq;
    wq << core::policy::Reliability::Reliable() << core::policy::History::KeepAll();
    sub::qos::DataReaderQos rq;
    rq << core::policy::Reliability::Reliable() << core::policy::History::KeepAll();
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    pub::DataWriter<core::ByteSeq> dw(pub_, t, wq);
    sub::DataReader<core::ByteSeq> dr(sub_, t, rq);
    dw->wait_for_matched(1, core::Duration::from_secs(5));
    dr->wait_for_matched(1, core::Duration::from_secs(5));

    constexpr int writers = 4;
    constexpr int per_writer = 50;
    std::atomic<bool> stop{false};
    std::atomic<int> failures_in_threads{0};
    std::vector<std::thread> threads;
    for (int w = 0; w < writers; ++w) {
        threads.emplace_back([&, w]() {
            try {
                for (int i = 0; i < per_writer; ++i) {
                    dw.write(bytes(static_cast<uint8_t>(w), static_cast<uint8_t>(i)));
                }
            } catch (...) {
                ++failures_in_threads;
            }
        });
    }
    // Operations of the other classes while the writers write.
    threads.emplace_back([&]() {
        try {
            while (!stop.load()) {
                (void)dw.publication_matched_status();
                (void)dr.subscription_matched_status();
                (void)pub_.qos();
                (void)sub_.qos();
                (void)dp.current_time();
                (void)dp.contains_entity(dw.instance_handle());
                (void)t.qos();
            }
        } catch (...) {
            ++failures_in_threads;
        }
    });
    size_t taken = 0;
    const uint64_t until = ::zerodds::cxx::monotonic_microseconds() + 10 * 1000000u;
    while (taken < static_cast<size_t>(writers * per_writer) && ::zerodds::cxx::monotonic_microseconds() < until) {
        taken += dr.select().take().length();
        ::zerodds::cxx::sleep_milliseconds(5);
    }
    stop.store(true);
    for (auto& th : threads) {
        th.join();
    }
    EXPECT(failures_in_threads.load() == 0, "no operation threw when called from several threads");
    EXPECT(taken == static_cast<size_t>(writers * per_writer),
           "every sample of the concurrent writers arrived");
}

// DDS-PSM-Cxx 1.0 §7.3: the operations of a WaitSet and of a Condition can be called from several
// threads at once, and so can the operations that create entities.
static void test_conditions_are_used_from_several_threads() {
    core::cond::WaitSet ws;
    core::cond::GuardCondition guards[4] = {core::cond::GuardCondition(), core::cond::GuardCondition(),
                                            core::cond::GuardCondition(), core::cond::GuardCondition()};
    for (auto& g : guards) {
        ws += g;
    }
    core::cond::GuardCondition spare;
    std::atomic<bool> stop{false};
    std::atomic<int> failures_in_threads{0};
    std::atomic<int> woken{0};
    std::vector<std::thread> threads;
    for (int i = 0; i < 4; ++i) {
        threads.emplace_back([&, i]() {
            try {
                for (int n = 0; n < 200; ++n) {
                    guards[i].trigger_value(true);
                    (void)guards[i].trigger_value();
                    guards[i].trigger_value(false);
                }
            } catch (...) {
                ++failures_in_threads;
            }
        });
    }
    // One thread waits, one attaches and detaches a condition, one reads the attached ones.
    threads.emplace_back([&]() {
        try {
            while (!stop.load()) {
                try {
                    woken += static_cast<int>(ws.wait(core::Duration::from_millisecs(5)).size());
                } catch (const core::TimeoutError&) {
                }
            }
        } catch (...) {
            ++failures_in_threads;
        }
    });
    threads.emplace_back([&]() {
        try {
            while (!stop.load()) {
                ws += spare;
                (void)ws.conditions();
                (void)ws.detach_condition(spare);
            }
        } catch (...) {
            ++failures_in_threads;
        }
    });
    for (std::size_t i = 0; i < 4; ++i) {
        threads[i].join();
    }
    stop.store(true);
    for (std::size_t i = 4; i < threads.size(); ++i) {
        threads[i].join();
    }
    EXPECT(failures_in_threads.load() == 0, "no condition or WaitSet operation threw");
}

static void test_entities_are_created_from_several_threads() {
    std::atomic<int> failures_in_threads{0};
    domain::DomainParticipant dp(D(38));
    std::atomic<int> created{0};
    std::vector<std::thread> makers;
    for (int i = 0; i < 4; ++i) {
        makers.emplace_back([&, i]() {
            try {
                for (int n = 0; n < 10; ++n) {
                    topic::Topic<core::ByteSeq> t(dp, "Threaded" + std::to_string(i) + "_" + std::to_string(n));
                    pub::Publisher publisher(dp);
                    sub::Subscriber subscriber(dp);
                    pub::DataWriter<core::ByteSeq> writer(publisher, t);
                    sub::DataReader<core::ByteSeq> reader(subscriber, t);
                    (void)writer.publication_matched_status();
                    ++created;
                }
            } catch (...) {
                ++failures_in_threads;
            }
        });
    }
    for (auto& th : makers) {
        th.join();
    }
    EXPECT(created.load() == 40 && failures_in_threads.load() == 0,
           "entities are created and closed from several threads");
}
#endif  // ZERODDS_CXX11

// DDS-PSM-Cxx 1.0 §2: where the PDF and the normative headers differ, the headers prevail. The
// signatures below are the ones `tools/psm-signature-diff.py` found different from the headers
// of the OMG: the getters of constructed attributes return by value (`const Duration period()
// const`), the writer of `on_offered_incompatible_qos` is taken by value, and GroupData,
// TopicData and Partition also copy their value to a destination argument.
/// Accepts only a const getter that returns exactly `Expected` (a value, no reference): any other
/// return type finds no matching overload and does not compile. The result is always true.
template <typename Expected, typename C>
static bool returns_by_value(Expected (C::*)() const) {
    return true;
}

static void test_signatures_follow_the_normative_headers() {
    namespace p = core::policy;
#if ZERODDS_CXX11
    // DDS-PSM-Cxx 1.0 §7.12 (decltype and std::declval are C++11): the static form is C++11 only,
    // the member function pointer form below runs in both modes.
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_same<decltype(std::declval<const p::Deadline&>().period()), core::Duration>::value),
                  "Deadline::period() const returns by value");
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_same<decltype(std::declval<const p::LatencyBudget&>().duration()), core::Duration>::value),
                  "LatencyBudget::duration() const returns by value");
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_same<decltype(std::declval<const p::Reliability&>().max_blocking_time()), core::Duration>::value),
                  "Reliability::max_blocking_time() const returns by value");
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_same<decltype(std::declval<const p::UserData&>().value()), core::ByteSeq>::value),
                  "UserData::value() const returns by value");
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_same<decltype(std::declval<const p::Partition&>().name()), core::StringSeq>::value),
                  "Partition::name() const returns by value");
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_same<decltype(std::declval<const core::status::SampleRejectedStatus&>().last_instance_handle()),
                               core::InstanceHandle>::value),
                  "last_instance_handle() const returns by value");
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_same<decltype(std::declval<const core::status::OfferedIncompatibleQosStatus&>().policies()),
                               p::QosPolicyCountSeq>::value),
                  "policies() const returns by value");
#endif
    EXPECT(returns_by_value<core::Duration>(&p::Deadline::period), "Deadline::period() const returns by value");
    EXPECT(returns_by_value<core::Duration>(&p::LatencyBudget::duration),
           "LatencyBudget::duration() const returns by value");
    EXPECT(returns_by_value<core::Duration>(&p::Reliability::max_blocking_time),
           "Reliability::max_blocking_time() const returns by value");
    EXPECT((returns_by_value<core::ByteSeq, p::UserData>(&p::UserData::value)), "UserData::value() const returns by value");
    EXPECT(returns_by_value<core::StringSeq>(&p::Partition::name), "Partition::name() const returns by value");
    EXPECT(returns_by_value<core::InstanceHandle>(&core::status::SampleRejectedStatus::last_instance_handle),
           "last_instance_handle() const returns by value");
    EXPECT(returns_by_value<p::QosPolicyCountSeq>(&core::status::OfferedIncompatibleQosStatus::policies),
           "policies() const returns by value");
    // The value of a temporary policy outlives the policy.
    const core::Duration period = p::Deadline(core::Duration::from_secs(3)).period();
    EXPECT(period.sec() == 3, "the period of a temporary Deadline is a value");
    // The destination forms.
    // On a constant object, as with the normative headers: on a non-constant one the setter of
    // the same name is as good a match.
    const p::GroupData group_data(bytes(1, 2));
    core::ByteSeq group;
    EXPECT(&group_data.value(group) == &group && group == bytes(1, 2),
           "GroupData::value(dst) const copies and returns dst");
    const p::TopicData topic_data(bytes(3));
    core::ByteSeq topic;
    topic_data.value(topic);
    EXPECT(topic == bytes(3), "TopicData::value(dst) const");
    const p::Partition partition("a");
    core::StringSeq names;
    EXPECT(&partition.name(names) == &names && names.size() == 1 && names[0] == "a",
           "Partition::name(dst) const copies and returns dst");
}

// DDS-PSM-Cxx 1.0 §7.5.5, Table 7.3: every return code of the C API maps to its exception, which
// derives from dds::core::Exception and from the standard exception of the table, and exceptions
// copy by value.
template <typename Expected>
static bool code_throws(int code) {
    try {
        ::zerodds::psm::check_status(code, "table 7.3");
    } catch (const Expected&) {
        return true;
    } catch (...) {
        return false;
    }
    return false;
}

static void test_return_codes_map_to_exceptions() {
    namespace c = core;
    EXPECT(code_throws<c::Error>(-1), "RETCODE_ERROR is Error");
    EXPECT(code_throws<c::InvalidArgumentError>(-6) && code_throws<c::InvalidArgumentError>(-3),
           "RETCODE_BAD_PARAMETER is InvalidArgumentError");
    EXPECT(code_throws<c::TimeoutError>(-4), "RETCODE_TIMEOUT is TimeoutError");
    EXPECT(code_throws<c::UnsupportedError>(-13), "RETCODE_UNSUPPORTED is UnsupportedError");
    EXPECT(code_throws<c::AlreadyClosedError>(-12) && code_throws<c::AlreadyClosedError>(-2),
           "RETCODE_ALREADY_DELETED is AlreadyClosedError");
    EXPECT(code_throws<c::IllegalOperationError>(-14), "RETCODE_ILLEGAL_OPERATION is IllegalOperationError");
    EXPECT(code_throws<c::NotEnabledError>(-9), "RETCODE_NOT_ENABLED is NotEnabledError");
    EXPECT(code_throws<c::PreconditionNotMetError>(-5), "RETCODE_PRECONDITION_NOT_MET is PreconditionNotMetError");
    EXPECT(code_throws<c::ImmutablePolicyError>(-10), "RETCODE_IMMUTABLE_POLICY is ImmutablePolicyError");
    EXPECT(code_throws<c::InconsistentPolicyError>(-11), "RETCODE_INCONSISTENT_POLICY is InconsistentPolicyError");
    EXPECT(code_throws<c::OutOfResourcesError>(-8), "RETCODE_OUT_OF_RESOURCES is OutOfResourcesError");
    bool no_exception = true;
    try {
        ::zerodds::psm::check_status(0, "ok");
        ::zerodds::psm::check_status(-7, "no data");
    } catch (...) {
        no_exception = false;
    }
    EXPECT(no_exception, "RETCODE_OK and RETCODE_NO_DATA throw nothing");

    // The parents of Table 7.3.
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_base_of<std::logic_error, c::AlreadyClosedError>::value &&
                      ::zerodds::cxx::is_base_of<std::logic_error, c::IllegalOperationError>::value &&
                      ::zerodds::cxx::is_base_of<std::logic_error, c::ImmutablePolicyError>::value &&
                      ::zerodds::cxx::is_base_of<std::logic_error, c::InconsistentPolicyError>::value &&
                      ::zerodds::cxx::is_base_of<std::logic_error, c::NotEnabledError>::value &&
                      ::zerodds::cxx::is_base_of<std::logic_error, c::PreconditionNotMetError>::value &&
                      ::zerodds::cxx::is_base_of<std::logic_error, c::UnsupportedError>::value),
                  "the logic_error exceptions");
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_base_of<std::invalid_argument, c::InvalidArgumentError>::value),
                  "InvalidArgumentError is an invalid_argument");
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_base_of<std::runtime_error, c::TimeoutError>::value &&
                      ::zerodds::cxx::is_base_of<std::runtime_error, c::OutOfResourcesError>::value),
                  "the runtime_error exceptions");
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_base_of<std::exception, c::Error>::value), "Error is a std::exception");
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_base_of<c::Exception, c::Error>::value &&
                      ::zerodds::cxx::is_base_of<c::Exception, c::TimeoutError>::value &&
                      ::zerodds::cxx::is_base_of<c::Exception, c::InvalidArgumentError>::value &&
                      ::zerodds::cxx::is_base_of<c::Exception, c::UnsupportedError>::value),
                  "every exception derives from dds::core::Exception");

    // Value semantics: a copy keeps the message and outlives the original.
    ::zerodds::cxx::shared_ptr<c::TimeoutError> copy;
    {
        c::TimeoutError original("the message");
        copy = ::zerodds::cxx::shared_ptr<c::TimeoutError>(new c::TimeoutError(original));
        c::PreconditionNotMetError assigned("first");
        assigned = c::PreconditionNotMetError("second");
        EXPECT(std::string(assigned.what()) == "second", "assignment of an exception");
    }
    EXPECT(std::string(copy->what()) == "the message", "a copied exception keeps its message");
}

// DDS-PSM-Cxx 1.0 §7.5.1.2: an object the application references, the creator of an object in use
// and an entity with a listener are still in use; they are not closed behind the application's back.
static void test_objects_in_use_stay_open() {
    pub::qos::DataWriterQos wq;
    wq << core::policy::Reliability::Reliable() << core::policy::History::KeepAll();
    sub::qos::DataReaderQos rq;
    rq << core::policy::Reliability::Reliable() << core::policy::History::KeepAll();
    pub::DataWriter<core::ByteSeq> dw(core::null);
    sub::DataReader<core::ByteSeq> dr(core::null);
    {
        domain::DomainParticipant dp(D(38));
        topic::Topic<core::ByteSeq> t(dp, "InUse");
        pub::Publisher publisher(dp);
        sub::Subscriber subscriber(dp);
        dw = pub::DataWriter<core::ByteSeq>(publisher, t, wq);
        dr = sub::DataReader<core::ByteSeq>(subscriber, t, rq);
    }
    // The participant, the topic, the publisher and the subscriber went out of scope; the writer
    // and the reader are referenced, so their creators are still in use.
    EXPECT(dw.publisher().participant().domain_id() == D(38), "the creators of a writer are in use");
    dw->wait_for_matched(1, core::Duration::from_secs(5));
    dr->wait_for_matched(1, core::Duration::from_secs(5));
    dw.write(bytes(7));
    size_t taken = 0;
    core::ByteSeq value;
    const uint64_t until = ::zerodds::cxx::monotonic_microseconds() + 5 * 1000000u;
    while (taken == 0 && ::zerodds::cxx::monotonic_microseconds() < until) {
        sub::LoanedSamples<core::ByteSeq> got = dr.select().take();
        for (sub::LoanedSamples<core::ByteSeq>::const_iterator s = got.begin(); s != got.end(); ++s) {
            if (s->info().valid()) {
                ++taken;
                value = s->data();
            }
        }
        ::zerodds::cxx::sleep_milliseconds(10);
    }
    EXPECT(taken == 1 && value == bytes(7), "a sample passes between objects whose creators are gone");

    // An entity with a non-null listener is still in use.
    struct Listener : domain::NoOpDomainParticipantListener {} listener;
    {
        domain::DomainParticipant dp(D(39));
        dp.listener(&listener, core::status::StatusMask::all());
    }
    EXPECT(domain::find(D(39)) != core::null, "a participant with a listener outlives its last reference");
    {
        domain::DomainParticipant again = domain::find(D(39));
        again.listener(nullptr, core::status::StatusMask::none());
    }
    EXPECT(domain::find(D(39)) == core::null, "and goes once the listener is removed");
}

// DDS-PSM-Cxx 1.0 §7.5.1: the types of Table 7.2 are reference types — they derive from
// Reference<DELEGATE>, assign shallowly, are null or valid, and take dds::core::null.
template <typename X>
struct derives_from_reference {
    static const bool value = ::zerodds::cxx::is_base_of<core::Reference<typename X::DELEGATE_T>, X>::value;
};


// The reference-type macros of the OMG's refmacros.hpp (DDS-PSM-Cxx 1.0 §7.2) declare a type
// that derives from `Reference`, shares its delegate and cannot be default-constructed.
namespace {
struct OmgMacroDelegate {
    OmgMacroDelegate() : value(7) {}
    int value;
};
template <typename Q>
struct OmgMacroTDelegate {
    OmgMacroTDelegate() : value() {}
    Q value;
};

class OmgMacroRef : public ::dds::core::Reference<OmgMacroDelegate> {
public:
    OMG_DDS_REF_TYPE_DELEGATE(OmgMacroRef, ::dds::core::Reference, OmgMacroDelegate)
    OMG_DDS_REF_TYPE_BASE(OmgMacroRef, ::dds::core::Reference, OmgMacroDelegate)
    explicit OmgMacroRef(const ::zerodds::cxx::shared_ptr<OmgMacroDelegate>& ref)
        : ::dds::core::Reference<OmgMacroDelegate>(ref) {}
    int value() const { return (*this)->value; }
};

template <typename Q, template <typename> class D>
class OmgMacroTBase : public ::dds::core::Reference<D<Q> > {
public:
    explicit OmgMacroTBase(D<Q>* p) : ::dds::core::Reference<D<Q> >(p) {}
    explicit OmgMacroTBase(const ::zerodds::cxx::shared_ptr<D<Q> >& ref) : ::dds::core::Reference<D<Q> >(ref) {}
    template <typename H>
    OmgMacroTBase(const H& h) : ::dds::core::Reference<D<Q> >(h) {}
};

class OmgMacroTRef : public OmgMacroTBase<int, OmgMacroTDelegate> {
public:
    OMG_DDS_REF_TYPE_DELEGATE_T(OmgMacroTRef, OmgMacroTBase, int, OmgMacroTDelegate)
    OMG_DDS_REF_TYPE_BASE_T(OmgMacroTRef, OmgMacroTBase, int, OmgMacroTDelegate)
    explicit OmgMacroTRef(const ::zerodds::cxx::shared_ptr<OmgMacroTDelegate<int> >& ref)
        : OmgMacroTBase<int, OmgMacroTDelegate>(ref) {}
};

class OmgMacroPtrRef : public ::dds::core::Reference<OmgMacroDelegate> {
public:
    DDS_PTR_CTOR_REF_TYPE(OmgMacroPtrRef, ::dds::core::Reference, OmgMacroDelegate)
};
} // namespace

static void test_reference_type_macros() {
#if ZERODDS_CXX11
    // DDS-PSM-Cxx 1.0 §7.12 (std::is_default_constructible is a C++11 type trait): C++11 only.
    static_assert(!std::is_default_constructible<OmgMacroRef>::value, "no default constructor");
    static_assert(!std::is_default_constructible<OmgMacroTRef>::value, "no default constructor");
    static_assert(!std::is_default_constructible<OmgMacroPtrRef>::value, "no default constructor");
#endif
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_same<OmgMacroRef::DELEGATE_T, OmgMacroDelegate>::value), "DELEGATE_T");
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_same<OmgMacroTRef::DELEGATE_T, OmgMacroTDelegate<int> >::value), "DELEGATE_T of the _T form");
    OmgMacroRef a(::zerodds::cxx::make_shared<OmgMacroDelegate>());
    OmgMacroRef b(a);
    EXPECT(a == b && b.value() == 7, "a handle built from another shares the delegate");
    OmgMacroPtrRef c(new OmgMacroDelegate());
    EXPECT(c->value == 7, "DDS_PTR_CTOR_REF_TYPE builds from a pointer to the delegate");
    OmgMacroPtrRef d(c);
    EXPECT(c == d, "and from another handle");
    OmgMacroTRef e(::zerodds::cxx::make_shared<OmgMacroTDelegate<int> >());
    EXPECT(e->value == 0, "the _T form");
}

static void test_reference_types() {
    // DDS-PSM-Cxx 1.0 §7.5.1.2: a reference type has no public default constructor; the null
    // reference is made from `dds::core::null`. A WaitSet and a GuardCondition create a new
    // object when default-constructed.
#if ZERODDS_CXX11
    // DDS-PSM-Cxx 1.0 §7.12 (std::is_default_constructible and std::is_constructible are C++11
    // type traits): C++11 only.
    static_assert(!std::is_default_constructible<domain::DomainParticipant>::value, "DomainParticipant");
    static_assert(!std::is_default_constructible<topic::Topic<core::ByteSeq> >::value, "Topic");
    static_assert(!std::is_default_constructible<topic::AnyTopic>::value, "AnyTopic");
    static_assert(!std::is_default_constructible<topic::ContentFilteredTopic<core::ByteSeq> >::value,
                  "ContentFilteredTopic");
    static_assert(!std::is_default_constructible<topic::MultiTopic<core::ByteSeq> >::value, "MultiTopic");
    static_assert(!std::is_default_constructible<pub::Publisher>::value, "Publisher");
    static_assert(!std::is_default_constructible<pub::DataWriter<core::ByteSeq> >::value, "DataWriter");
    static_assert(!std::is_default_constructible<sub::Subscriber>::value, "Subscriber");
    static_assert(!std::is_default_constructible<sub::DataReader<core::ByteSeq> >::value, "DataReader");
    static_assert(std::is_constructible<domain::DomainParticipant, const core::null_type&>::value,
                  "DomainParticipant from null");
    static_assert(std::is_constructible<pub::DataWriter<core::ByteSeq>, const core::null_type&>::value,
                  "DataWriter from null");
#endif
    // The second template parameter names the delegate; the default is ZeroDDS's.
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_same<pub::DataWriter<core::ByteSeq>,
                               pub::DataWriter<core::ByteSeq, pub::detail::DataWriterDelegate> >::value),
                  "DataWriter default delegate");
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_same<pub::DataWriter<core::ByteSeq>::DELEGATE_T,
                               pub::detail::DataWriterDelegate<core::ByteSeq> >::value),
                  "DataWriter DELEGATE_T");
    ZERODDS_STATIC_ASSERT((::zerodds::cxx::is_same<sub::DataReader<core::ByteSeq>,
                               sub::DataReader<core::ByteSeq, sub::detail::DataReaderDelegate> >::value),
                  "DataReader default delegate");
#if ZERODDS_CXX11
    // DDS-PSM-Cxx 1.0 §7.12 (std::is_default_constructible is a C++11 type trait): C++11 only.
    static_assert(std::is_default_constructible<core::cond::WaitSet>::value, "WaitSet");
    static_assert(std::is_default_constructible<core::cond::GuardCondition>::value, "GuardCondition");
#endif
    ZERODDS_STATIC_ASSERT((derives_from_reference<core::Entity>::value), "Entity");
    ZERODDS_STATIC_ASSERT((derives_from_reference<core::cond::Condition>::value), "Condition");
    ZERODDS_STATIC_ASSERT((derives_from_reference<core::cond::GuardCondition>::value), "GuardCondition");
    ZERODDS_STATIC_ASSERT((derives_from_reference<core::cond::WaitSet>::value), "WaitSet");
    ZERODDS_STATIC_ASSERT((derives_from_reference<core::cond::StatusCondition>::value), "StatusCondition");
    ZERODDS_STATIC_ASSERT((derives_from_reference<sub::cond::ReadCondition>::value), "ReadCondition");
    ZERODDS_STATIC_ASSERT((derives_from_reference<sub::cond::QueryCondition>::value), "QueryCondition");
    ZERODDS_STATIC_ASSERT((derives_from_reference<domain::DomainParticipant>::value), "DomainParticipant");
    ZERODDS_STATIC_ASSERT((derives_from_reference<pub::AnyDataWriter>::value), "AnyDataWriter");
    ZERODDS_STATIC_ASSERT((derives_from_reference<pub::Publisher>::value), "Publisher");
    ZERODDS_STATIC_ASSERT((derives_from_reference<pub::DataWriter<core::ByteSeq> >::value), "DataWriter");
    ZERODDS_STATIC_ASSERT((derives_from_reference<sub::AnyDataReader>::value), "AnyDataReader");
    ZERODDS_STATIC_ASSERT((derives_from_reference<sub::Subscriber>::value), "Subscriber");
    ZERODDS_STATIC_ASSERT((derives_from_reference<sub::DataReader<core::ByteSeq> >::value), "DataReader");
    ZERODDS_STATIC_ASSERT((derives_from_reference<sub::SharedSamples<core::ByteSeq> >::value), "SharedSamples");
    ZERODDS_STATIC_ASSERT((derives_from_reference<topic::AnyTopic>::value), "AnyTopic");
    ZERODDS_STATIC_ASSERT((derives_from_reference<topic::Topic<core::ByteSeq> >::value), "Topic");

    // Null: constructed from, assigned from and compared with dds::core::null.
    domain::DomainParticipant none(core::null);
    EXPECT(none == core::null && none.is_nil(), "a participant made from dds::core::null is null");
    topic::AnyTopic any_none = core::null;
    EXPECT(any_none == core::null, "AnyTopic from dds::core::null");
    pub::AnyDataWriter any_writer_none(core::null);
    EXPECT(any_writer_none == core::null, "AnyDataWriter from dds::core::null");
    bool null_use_throws = false;
    try {
        (void)none.domain_id();
    } catch (const core::Exception&) {
        null_use_throws = true;
    } catch (const std::exception&) {
        null_use_throws = true;
    }
    EXPECT(null_use_throws, "a call through a null reference is an exception, not undefined behaviour");

    // Shallow assignment: the copy is the same object, assigning null drops only the reference.
    domain::DomainParticipant dp(D(40));
    domain::DomainParticipant alias = dp;
    EXPECT(alias == dp && alias.domain_id() == D(40), "assignment copies the reference");
    pub::Publisher pub_(dp);
    pub::Publisher second = pub_;
    EXPECT(second == pub_, "a copied Publisher is the same Publisher");
    second = core::null;
    EXPECT(second == core::null && pub_ != core::null, "assigning null leaves the other reference");
    topic::Topic<core::ByteSeq> t(dp, "ReferenceTypes");
    topic::AnyTopic any_t(t);
    topic::AnyTopic any_copy = any_t;
    EXPECT(any_copy == any_t, "a copied AnyTopic is the same AnyTopic");

    // close() halts the communication of an entity: its peer sees it go, and later calls fail.
    pub::qos::DataWriterQos wq;
    wq << core::policy::Reliability::Reliable();
    sub::qos::DataReaderQos rq;
    rq << core::policy::Reliability::Reliable();
    sub::Subscriber sub_(dp);
    pub::DataWriter<core::ByteSeq> dw(pub_, t, wq);
    sub::DataReader<core::ByteSeq> dr(sub_, t, rq);
    dw->wait_for_matched(1, core::Duration::from_secs(5));
    dr->wait_for_matched(1, core::Duration::from_secs(5));
    EXPECT(dr.subscription_matched_status().current_count() == 1, "the reader has its writer");
    dw.close();
    bool closed_error = false;
    try {
        dw.assert_liveliness();
    } catch (const core::AlreadyClosedError&) {
        closed_error = true;
    }
    EXPECT(closed_error, "a call on a closed writer is AlreadyClosedError");
    int32_t remaining = 1;
    const uint64_t until = ::zerodds::cxx::monotonic_microseconds() + 5 * 1000000u;
    while (remaining != 0 && ::zerodds::cxx::monotonic_microseconds() < until) {
        remaining = dr.subscription_matched_status().current_count();
        ::zerodds::cxx::sleep_milliseconds(20);
    }
    EXPECT(remaining == 0, "the reader sees the closed writer go");
}

// DDS-PSM-Cxx 1.0 §7.6.1, DDS 1.2 §7.1.3: every policy has the default value of the table of
// DDS 1.2 and the attributes it lists, which a constructor sets and an accessor reads.
static void test_every_policy_has_its_default_and_its_attributes() {
    namespace p = core::policy;
    const core::Duration inf = core::Duration::infinite();

    EXPECT(p::UserData().value().empty() && p::TopicData().value().empty() && p::GroupData().value().empty(),
           "USER_DATA, TOPIC_DATA and GROUP_DATA default to an empty sequence");
    EXPECT(p::UserData(bytes(1)).value() == bytes(1) &&
               p::TopicData(bytes(2)).value() == bytes(2) &&
               p::GroupData(bytes(3)).value() == bytes(3),
           "USER_DATA, TOPIC_DATA and GROUP_DATA hold the value they are made with");
    EXPECT(p::Durability().kind() == p::DurabilityKind::VOLATILE, "DURABILITY defaults to VOLATILE");
    const p::DurabilityService service;
    EXPECT(service.service_cleanup_delay() == core::Duration::zero() &&
               service.history_kind() == p::HistoryKind::KEEP_LAST && service.history_depth() == 1 &&
               service.max_samples() == -1 && service.max_instances() == -1 &&
               service.max_samples_per_instance() == -1,
           "DURABILITY_SERVICE defaults");
    const p::DurabilityService service_set(core::Duration(1, 0), p::HistoryKind::KEEP_ALL, 5, 6, 7, 8);
    EXPECT(service_set.history_kind() == p::HistoryKind::KEEP_ALL && service_set.history_depth() == 5 &&
               service_set.max_samples() == 6 && service_set.max_instances() == 7 &&
               service_set.max_samples_per_instance() == 8,
           "DURABILITY_SERVICE attributes");
    const p::Presentation presentation;
    EXPECT(presentation.access_scope() == p::PresentationAccessScopeKind::INSTANCE &&
               !presentation.coherent_access() && !presentation.ordered_access(),
           "PRESENTATION defaults");
    EXPECT(p::Presentation(p::PresentationAccessScopeKind::GROUP, true, true).access_scope() ==
               p::PresentationAccessScopeKind::GROUP,
           "PRESENTATION access scope GROUP");
    EXPECT(p::Deadline().period() == inf && p::Deadline(core::Duration(2, 0)).period() == core::Duration(2, 0),
           "DEADLINE");
    EXPECT(p::LatencyBudget().duration() == core::Duration::zero() &&
               p::LatencyBudget(core::Duration(3, 0)).duration() == core::Duration(3, 0),
           "LATENCY_BUDGET");
    EXPECT(p::Ownership().kind() == p::OwnershipKind::SHARED &&
               p::Ownership::Exclusive().kind() == p::OwnershipKind::EXCLUSIVE,
           "OWNERSHIP");
    EXPECT(p::OwnershipStrength().value() == 0 && p::OwnershipStrength(7).value() == 7, "OWNERSHIP_STRENGTH");
    EXPECT(p::Liveliness().kind() == p::LivelinessKind::AUTOMATIC && p::Liveliness().lease_duration() == inf,
           "LIVELINESS defaults");
    EXPECT(p::TimeBasedFilter().minimum_separation() == core::Duration::zero() &&
               p::TimeBasedFilter(core::Duration(1, 0)).minimum_separation() == core::Duration(1, 0),
           "TIME_BASED_FILTER");
    EXPECT(p::Partition().name().empty() && p::Partition("a").name().size() == 1 &&
               p::Partition(strs("a", "b")).name()[1] == "b",
           "PARTITION");
    EXPECT(p::Reliability().kind() == p::ReliabilityKind::BEST_EFFORT &&
               p::Reliability().max_blocking_time() == core::Duration::from_millisecs(100) &&
               p::Reliability::Reliable().kind() == p::ReliabilityKind::RELIABLE,
           "RELIABILITY");
    EXPECT(p::TransportPriority().value() == 0 && p::TransportPriority(9).value() == 9, "TRANSPORT_PRIORITY");
    EXPECT(p::Lifespan().duration() == inf && p::Lifespan(core::Duration(4, 0)).duration() == core::Duration(4, 0),
           "LIFESPAN");
    EXPECT(p::DestinationOrder().kind() == p::DestinationOrderKind::BY_RECEPTION_TIMESTAMP, "DESTINATION_ORDER");
    EXPECT(p::History().kind() == p::HistoryKind::KEEP_LAST && p::History().depth() == 1 &&
               p::History::KeepAll().kind() == p::HistoryKind::KEEP_ALL,
           "HISTORY");
    const p::ResourceLimits limits;
    EXPECT(limits.max_samples() == core::LENGTH_UNLIMITED && limits.max_instances() == core::LENGTH_UNLIMITED &&
               limits.max_samples_per_instance() == core::LENGTH_UNLIMITED,
           "RESOURCE_LIMITS default to LENGTH_UNLIMITED");
    const p::ResourceLimits limits_set(10, 11, 12);
    EXPECT(limits_set.max_samples() == 10 && limits_set.max_instances() == 11 &&
               limits_set.max_samples_per_instance() == 12,
           "RESOURCE_LIMITS attributes");
    EXPECT(p::EntityFactory().autoenable_created_entities() &&
               !p::EntityFactory::ManuallyEnable().autoenable_created_entities(),
           "ENTITY_FACTORY");
    EXPECT(p::WriterDataLifecycle().autodispose_unregistered_instances(), "WRITER_DATA_LIFECYCLE defaults to TRUE");
    EXPECT(p::ReaderDataLifecycle().autopurge_nowriter_samples_delay() == inf &&
               p::ReaderDataLifecycle().autopurge_disposed_samples_delay() == inf,
           "READER_DATA_LIFECYCLE defaults to infinite");

    // The ids and names of the traits (DDS 1.2 QosPolicyId_t).
    EXPECT(p::policy_id<p::UserData>::value == 1 && p::policy_name<p::UserData>::name() == "UserData", "the traits of UserData");
    EXPECT(p::policy_id<p::Durability>::value == 2 && p::policy_name<p::Durability>::name() == "Durability", "the traits of Durability");
    EXPECT(p::policy_id<p::Presentation>::value == 3 && p::policy_name<p::Presentation>::name() == "Presentation", "the traits of Presentation");
    EXPECT(p::policy_id<p::Deadline>::value == 4 && p::policy_name<p::Deadline>::name() == "Deadline", "the traits of Deadline");
    EXPECT(p::policy_id<p::LatencyBudget>::value == 5 && p::policy_name<p::LatencyBudget>::name() == "LatencyBudget", "the traits of LatencyBudget");
    EXPECT(p::policy_id<p::Ownership>::value == 6 && p::policy_name<p::Ownership>::name() == "Ownership", "the traits of Ownership");
    EXPECT(p::policy_id<p::OwnershipStrength>::value == 7 && p::policy_name<p::OwnershipStrength>::name() == "OwnershipStrength", "the traits of OwnershipStrength");
    EXPECT(p::policy_id<p::Liveliness>::value == 8 && p::policy_name<p::Liveliness>::name() == "Liveliness", "the traits of Liveliness");
    EXPECT(p::policy_id<p::TimeBasedFilter>::value == 9 && p::policy_name<p::TimeBasedFilter>::name() == "TimeBasedFilter", "the traits of TimeBasedFilter");
    EXPECT(p::policy_id<p::Partition>::value == 10 && p::policy_name<p::Partition>::name() == "Partition", "the traits of Partition");
    EXPECT(p::policy_id<p::Reliability>::value == 11 && p::policy_name<p::Reliability>::name() == "Reliability", "the traits of Reliability");
    EXPECT(p::policy_id<p::DestinationOrder>::value == 12 && p::policy_name<p::DestinationOrder>::name() == "DestinationOrder", "the traits of DestinationOrder");
    EXPECT(p::policy_id<p::History>::value == 13 && p::policy_name<p::History>::name() == "History", "the traits of History");
    EXPECT(p::policy_id<p::ResourceLimits>::value == 14 && p::policy_name<p::ResourceLimits>::name() == "ResourceLimits", "the traits of ResourceLimits");
    EXPECT(p::policy_id<p::EntityFactory>::value == 15 && p::policy_name<p::EntityFactory>::name() == "EntityFactory", "the traits of EntityFactory");
    EXPECT(p::policy_id<p::WriterDataLifecycle>::value == 16 && p::policy_name<p::WriterDataLifecycle>::name() == "WriterDataLifecycle", "the traits of WriterDataLifecycle");
    EXPECT(p::policy_id<p::ReaderDataLifecycle>::value == 17 && p::policy_name<p::ReaderDataLifecycle>::name() == "ReaderDataLifecycle", "the traits of ReaderDataLifecycle");
    EXPECT(p::policy_id<p::TopicData>::value == 18 && p::policy_name<p::TopicData>::name() == "TopicData", "the traits of TopicData");
    EXPECT(p::policy_id<p::GroupData>::value == 19 && p::policy_name<p::GroupData>::name() == "GroupData", "the traits of GroupData");
    EXPECT(p::policy_id<p::TransportPriority>::value == 20 && p::policy_name<p::TransportPriority>::name() == "TransportPriority", "the traits of TransportPriority");
    EXPECT(p::policy_id<p::Lifespan>::value == 21 && p::policy_name<p::Lifespan>::name() == "Lifespan", "the traits of Lifespan");
    EXPECT(p::policy_id<p::DurabilityService>::value == 22 && p::policy_name<p::DurabilityService>::name() == "DurabilityService", "the traits of DurabilityService");

    // The delegate of a policy is reachable through operator-> and delegate() (§7.6).
    p::History history;
    EXPECT(history->depth() == 1 && history.delegate().depth() == 1, "operator-> and delegate() reach the delegate");
}

// DDS-PSM-Cxx 1.0 §2.1 (DDS 1.4 §2.2.2.2.1.14-17): ignore_participant, ignore_topic,
// ignore_publication and ignore_subscription end the local match of a discovered entity. Each case
// has two participants in one domain, a writer in the first and a reader in the second.
namespace {
struct IgnorePair {
    domain::DomainParticipant writer_side;
    domain::DomainParticipant reader_side;
    topic::Topic<core::ByteSeq> writer_topic;
    topic::Topic<core::ByteSeq> reader_topic;
    pub::Publisher publisher;
    sub::Subscriber subscriber;
    pub::DataWriter<core::ByteSeq> writer;
    sub::DataReader<core::ByteSeq> reader;

    IgnorePair(uint32_t domain_id, const std::string& name)
        : writer_side(domain_id),
          reader_side(domain_id),
          writer_topic(writer_side, name),
          reader_topic(reader_side, name),
          publisher(writer_side),
          subscriber(reader_side),
          writer(publisher, writer_topic, qos<pub::qos::DataWriterQos>()),
          reader(subscriber, reader_topic, qos<sub::qos::DataReaderQos>()) {
        writer->wait_for_matched(1, core::Duration::from_secs(5));
        reader->wait_for_matched(1, core::Duration::from_secs(5));
    }

    template <typename Q>
    static Q qos() {
        Q q;
        q << core::policy::Reliability::Reliable() << core::policy::History::KeepAll();
        return q;
    }
};

template <typename Predicate>
bool eventually_true(Predicate p) {
    const uint64_t until = ::zerodds::cxx::monotonic_microseconds() + 5 * 1000000u;
    while (::zerodds::cxx::monotonic_microseconds() < until) {
        if (p()) {
            return true;
        }
        ::zerodds::cxx::sleep_milliseconds(20);
    }
    return p();
}

/// True when a sample of the reader names the topic `name` (`topic_name()` of a publication or a
/// subscription); the samples are read, not taken.
template <typename T>
struct SampleNamesTopic {
    sub::DataReader<T> reader;
    std::string name;
    SampleNamesTopic(const sub::DataReader<T>& r, const char* n) : reader(r), name(n) {}
    bool operator()() {
        sub::LoanedSamples<T> samples = reader.read();
        for (typename sub::LoanedSamples<T>::const_iterator s = samples.begin(); s != samples.end(); ++s) {
            if (s->info().valid() && s->data().topic_name() == name) {
                return true;
            }
        }
        return false;
    }
};

/// The same for the samples of the DCPSTopic reader, whose `name()` is the topic name.
struct TopicSampleHasName {
    sub::DataReader< ::dds::topic::TopicBuiltinTopicData> reader;
    std::string name;
    TopicSampleHasName(const sub::DataReader< ::dds::topic::TopicBuiltinTopicData>& r, const char* n)
        : reader(r), name(n) {}
    bool operator()() {
        sub::LoanedSamples< ::dds::topic::TopicBuiltinTopicData> samples = reader.read();
        for (sub::LoanedSamples< ::dds::topic::TopicBuiltinTopicData>::const_iterator s = samples.begin();
             s != samples.end(); ++s) {
            if (s->info().valid() && s->data().name() == name) {
                return true;
            }
        }
        return false;
    }
};

/// True when the reader holds a sample.
template <typename T>
struct ReaderHoldsSample {
    sub::DataReader<T> reader;
    explicit ReaderHoldsSample(const sub::DataReader<T>& r) : reader(r) {}
    bool operator()() { return reader.read().length() >= 1; }
};

/// Like SampleNamesTopic, and keeps the first sample of that topic in `*data`.
template <typename T>
struct CaptureSampleOfTopic {
    sub::DataReader<T> reader;
    std::string name;
    T* data;
    bool* have;
    CaptureSampleOfTopic(const sub::DataReader<T>& r, const char* n, T* d, bool* h)
        : reader(r), name(n), data(d), have(h) {}
    bool operator()() {
        sub::LoanedSamples<T> samples = reader.read();
        for (typename sub::LoanedSamples<T>::const_iterator s = samples.begin(); s != samples.end(); ++s) {
            if (s->info().valid() && s->data().topic_name() == name) {
                *data = s->data();
                *have = true;
                return true;
            }
        }
        return false;
    }
};

struct HasDiscoveredParticipants {
    domain::DomainParticipant participant;
    explicit HasDiscoveredParticipants(const domain::DomainParticipant& p) : participant(p) {}
    bool operator()() { return !participant->get_discovered_participants().empty(); }
};

struct NoMatchedPublications {
    sub::DataReader<core::ByteSeq> reader;
    explicit NoMatchedPublications(const sub::DataReader<core::ByteSeq>& r) : reader(r) {}
    bool operator()() { return sub::matched_publications(reader).empty(); }
};

struct NoMatchedSubscriptions {
    pub::DataWriter<core::ByteSeq> writer;
    explicit NoMatchedSubscriptions(const pub::DataWriter<core::ByteSeq>& w) : writer(w) {}
    bool operator()() { return pub::matched_subscriptions(writer).empty(); }
};

/// Looks for a discovered topic of the name `name` and keeps its handle in `*found`.
struct FindsDiscoveredTopic {
    domain::DomainParticipant participant;
    std::string name;
    core::InstanceHandle* found;
    FindsDiscoveredTopic(const domain::DomainParticipant& p, const char* n, core::InstanceHandle* f)
        : participant(p), name(n), found(f) {}
    bool operator()() {
        const core::InstanceHandleSeq handles = participant->get_discovered_topics();
        for (core::InstanceHandleSeq::const_iterator h = handles.begin(); h != handles.end(); ++h) {
            if (participant->get_discovered_topic_data(*h).name() == name) {
                *found = *h;
                return true;
            }
        }
        return false;
    }
};
} // namespace

// DDS 1.4 §2.2.5: the readers of the built-in subscriber hold what discovery found about the
// other participant, its topic, its writer and this side's reader.
static void test_builtin_topic_readers() {
    namespace tp = ::dds::topic;
    domain::DomainParticipant reader_side(D(47));
    domain::DomainParticipant writer_side(D(47));
    tp::Topic<core::ByteSeq> writer_topic(writer_side, "BuiltinReaders");
    tp::Topic<core::ByteSeq> reader_topic(reader_side, "BuiltinReaders");
    pub::Publisher publisher(writer_side);
    pub::DataWriter<core::ByteSeq> writer(publisher, writer_topic);
    sub::Subscriber subscriber(reader_side);
    sub::DataReader<core::ByteSeq> reader(subscriber, reader_topic);

    sub::Subscriber builtin = sub::builtin_subscriber(reader_side);
    // The writer's side sees the reader as a subscription.
    sub::Subscriber builtin_of_writer = sub::builtin_subscriber(writer_side);
    {
        // The built-in subscriber is a dds::sub::Subscriber (DDS-PSM-Cxx 1.0, dds/sub/Subscriber.hpp):
        // it has the readers of the built-in topics and no reader of an application topic.
        std::vector<sub::DataReader<core::ByteSeq> > application_readers;
        EXPECT(sub::find<sub::DataReader<core::ByteSeq> >(builtin, "BuiltinReaders",
                                                           std::back_inserter(application_readers)) == 0,
               "the built-in subscriber has no reader of an application topic");
        EXPECT(builtin.participant().instance_handle() == reader_side.instance_handle(),
               "the built-in subscriber belongs to the participant it was asked from");
    }
    std::vector<sub::DataReader<tp::ParticipantBuiltinTopicData> > participants;
    std::vector<sub::DataReader<tp::TopicBuiltinTopicData> > topics;
    std::vector<sub::DataReader<tp::PublicationBuiltinTopicData> > publications;
    std::vector<sub::DataReader<tp::SubscriptionBuiltinTopicData> > subscriptions;
    EXPECT(sub::find<sub::DataReader<tp::ParticipantBuiltinTopicData> >(
               builtin, "DCPSParticipant", std::back_inserter(participants)) == 1,
           "the built-in subscriber has the DCPSParticipant reader");
    EXPECT(sub::find<sub::DataReader<tp::TopicBuiltinTopicData> >(
               builtin, "DCPSTopic", std::back_inserter(topics)) == 1,
           "and the DCPSTopic reader");
    EXPECT(sub::find<sub::DataReader<tp::PublicationBuiltinTopicData> >(
               builtin, "DCPSPublication", std::back_inserter(publications)) == 1,
           "and the DCPSPublication reader");
    EXPECT(sub::find<sub::DataReader<tp::SubscriptionBuiltinTopicData> >(
               builtin_of_writer, "DCPSSubscription", std::back_inserter(subscriptions)) == 1,
           "and the DCPSSubscription reader");
    EXPECT(sub::find<sub::DataReader<tp::TopicBuiltinTopicData> >(
               builtin, "DCPSParticipant", std::back_inserter(topics)) == 0,
           "a reader of another built-in type is not the reader of that topic");
    if (participants.size() != 1 || topics.size() != 1 || publications.size() != 1 ||
        subscriptions.size() != 1) {
        return;
    }

    EXPECT(eventually_true(SampleNamesTopic<tp::PublicationBuiltinTopicData>(publications[0], "BuiltinReaders")),
           "the publication of the other participant's writer");
    EXPECT(eventually_true(SampleNamesTopic<tp::SubscriptionBuiltinTopicData>(subscriptions[0], "BuiltinReaders")),
           "the subscription of the other participant's reader");
    EXPECT(eventually_true(TopicSampleHasName(topics[0], "BuiltinReaders")), "the topic");
    EXPECT(eventually_true(ReaderHoldsSample<tp::ParticipantBuiltinTopicData>(participants[0])), "a participant");

    // `take` removes what it returned.
    EXPECT(participants[0].take().length() >= 1, "take returns the participants");
    EXPECT(participants[0].read().length() <= 1, "and removes them");
}

// DDS 1.4 §2.2.5.3/4: the values of the members of the built-in publication and subscription
// samples are the ones the other participant's writer and reader were created with.
static void test_builtin_topic_values_arrive() {
    namespace p = core::policy;
    namespace tp = ::dds::topic;
    domain::DomainParticipant writer_side(D(48));
    domain::DomainParticipant reader_side(D(48));
    topic::qos::TopicQos tq;
    tq << p::TopicData(bytes(9, 9));
    topic::Topic<core::ByteSeq> wt(writer_side, "BuiltinValues", tq);
    topic::qos::TopicQos rtq;
    rtq << p::TopicData(bytes(8));
    topic::Topic<core::ByteSeq> rt(reader_side, "BuiltinValues", rtq);
    pub::qos::PublisherQos pq;
    pq << p::Partition("part1") << p::GroupData(bytes(4, 5)) << p::Presentation::GroupAccessScope(true, false);
    pub::Publisher publisher(writer_side, pq);
    pub::qos::DataWriterQos wq;
    wq << p::Durability::TransientLocal() << p::Deadline(core::Duration::from_millisecs(400))
       << p::LatencyBudget(core::Duration::from_millisecs(250))
       << p::Liveliness::ManualByTopic(core::Duration::from_secs(2)) << p::Reliability::Reliable()
       << p::DestinationOrder::SourceTimestamp() << p::UserData(bytes(1, 2, 3)) << p::Ownership::Exclusive()
       << p::OwnershipStrength(7) << p::Lifespan(core::Duration::from_secs(30))
       << p::DurabilityService(core::Duration::from_secs(5), p::HistoryKind::KEEP_LAST, 6, 100, 10, 50);
    pub::DataWriter<core::ByteSeq> writer(publisher, wt, wq);
    sub::qos::SubscriberQos sq;
    sq << p::Partition("part1");
    sub::Subscriber subscriber(reader_side, sq);
    sub::qos::DataReaderQos rq;
    rq << p::Durability::TransientLocal() << p::Deadline(core::Duration::from_millisecs(500))
       << p::LatencyBudget(core::Duration::from_millisecs(100)) << p::Reliability::Reliable()
       << p::Ownership::Exclusive() << p::UserData(bytes(6)) << p::TimeBasedFilter(core::Duration::from_millisecs(100));
    sub::DataReader<core::ByteSeq> reader(subscriber, rt, rq);

    std::vector<sub::DataReader<tp::PublicationBuiltinTopicData> > publications;
    sub::find<sub::DataReader<tp::PublicationBuiltinTopicData> >(
        sub::builtin_subscriber(reader_side), "DCPSPublication", std::back_inserter(publications));
    std::vector<sub::DataReader<tp::SubscriptionBuiltinTopicData> > subscriptions;
    sub::find<sub::DataReader<tp::SubscriptionBuiltinTopicData> >(
        sub::builtin_subscriber(writer_side), "DCPSSubscription", std::back_inserter(subscriptions));
    if (publications.size() != 1 || subscriptions.size() != 1) {
        EXPECT(false, "the built-in readers exist");
        return;
    }
    tp::PublicationBuiltinTopicData pub_data;
    bool have_pub = false;
    EXPECT(eventually_true(CaptureSampleOfTopic<tp::PublicationBuiltinTopicData>(publications[0], "BuiltinValues",
                                                                                 &pub_data, &have_pub)),
           "the publication arrives");
    if (have_pub) {
        EXPECT(pub_data.type_name() == "DDS::Bytes", "publication type name");
        EXPECT(pub_data.durability().kind() == core::policy::DurabilityKind::TRANSIENT_LOCAL,
               "publication DURABILITY");
        EXPECT(pub_data.deadline().period().nanosec() == 400000000u, "publication DEADLINE");
        EXPECT(pub_data.latency_budget().duration().nanosec() == 250000000u, "publication LATENCY_BUDGET");
        EXPECT(pub_data.liveliness().kind() == core::policy::LivelinessKind::MANUAL_BY_TOPIC &&
                   pub_data.liveliness().lease_duration().sec() == 2,
               "publication LIVELINESS");
        EXPECT(pub_data.reliability().kind() == core::policy::ReliabilityKind::RELIABLE,
               "publication RELIABILITY");
        EXPECT(pub_data.destination_order().kind() == core::policy::DestinationOrderKind::BY_SOURCE_TIMESTAMP,
               "publication DESTINATION_ORDER");
        EXPECT(pub_data.user_data().value() == bytes(1, 2, 3), "publication USER_DATA");
        EXPECT(pub_data.ownership().kind() == core::policy::OwnershipKind::EXCLUSIVE &&
                   pub_data.ownership_strength().value() == 7,
               "publication OWNERSHIP and OWNERSHIP_STRENGTH");
        EXPECT(pub_data.lifespan().duration().sec() == 30, "publication LIFESPAN");
        EXPECT(pub_data.partition().name().size() == 1 && pub_data.partition().name()[0] == "part1",
               "publication PARTITION");
        EXPECT(pub_data.group_data().value() == bytes(4, 5), "publication GROUP_DATA");
        EXPECT(pub_data.topic_data().value() == bytes(9, 9), "publication TOPIC_DATA");
        EXPECT(pub_data.presentation().access_scope() == core::policy::PresentationAccessScopeKind::GROUP &&
                   pub_data.presentation().coherent_access() && !pub_data.presentation().ordered_access(),
               "publication PRESENTATION");
        const core::policy::DurabilityService ds = pub_data.durability_service();
        EXPECT(ds.service_cleanup_delay().sec() == 5 && ds.history_kind() == core::policy::HistoryKind::KEEP_LAST &&
                   ds.history_depth() == 6 && ds.max_samples() == 100 && ds.max_instances() == 10 &&
                   ds.max_samples_per_instance() == 50,
               "publication DURABILITY_SERVICE");
        const tp::BuiltinTopicKey::VALUE_T zero = tp::BuiltinTopicKey::VALUE_T();
        EXPECT(pub_data.key().value() != zero && pub_data.participant_key().value() != zero &&
                   pub_data.key().value() != pub_data.participant_key().value(),
               "publication key and participant key");
    }
    tp::SubscriptionBuiltinTopicData sub_data;
    bool have_sub = false;
    EXPECT(eventually_true(CaptureSampleOfTopic<tp::SubscriptionBuiltinTopicData>(subscriptions[0], "BuiltinValues",
                                                                                  &sub_data, &have_sub)),
           "the subscription arrives");
    if (have_sub) {
        EXPECT(sub_data.durability().kind() == core::policy::DurabilityKind::TRANSIENT_LOCAL,
               "subscription DURABILITY");
        EXPECT(sub_data.deadline().period().nanosec() == 500000000u, "subscription DEADLINE");
        EXPECT(sub_data.latency_budget().duration().nanosec() == 100000000u, "subscription LATENCY_BUDGET");
        EXPECT(sub_data.reliability().kind() == core::policy::ReliabilityKind::RELIABLE,
               "subscription RELIABILITY");
        EXPECT(sub_data.ownership().kind() == core::policy::OwnershipKind::EXCLUSIVE, "subscription OWNERSHIP");
        EXPECT(sub_data.user_data().value() == bytes(6), "subscription USER_DATA");
        EXPECT(sub_data.partition().name().size() == 1 && sub_data.partition().name()[0] == "part1",
               "subscription PARTITION");
        EXPECT(sub_data.time_based_filter().minimum_separation().nanosec() == 100000000u,
               "subscription TIME_BASED_FILTER");
        EXPECT(sub_data.topic_data().value() == bytes(8), "subscription TOPIC_DATA");
    }
}

static void test_ignore_ends_a_match() {
    {
        IgnorePair pair(43, "IgnoreParticipant");
        EXPECT(eventually_true(HasDiscoveredParticipants(pair.reader_side)),
               "the reader's participant discovers the writer's participant");
        const core::InstanceHandleSeq discovered = pair.reader_side->get_discovered_participants();
        if (!discovered.empty()) {
            (void)pair.reader_side->get_discovered_participant_data(discovered[0]);
            for (core::InstanceHandleSeq::const_iterator h = discovered.begin(); h != discovered.end(); ++h) {
                domain::ignore(pair.reader_side, *h);
            }
            EXPECT(eventually_true(NoMatchedPublications(pair.reader)),
                   "ignore_participant ends the match of the ignored participant's writer");
        }
    }
    {
        IgnorePair pair(44, "IgnorePublication");
        const core::InstanceHandleSeq publications = sub::matched_publications(pair.reader);
        EXPECT(publications.size() == 1, "the reader has its writer");
        if (publications.size() == 1) {
            // The reader's participant ignores the remote writer: a publication.
            pub::ignore(pair.reader_side, publications[0]);
            EXPECT(eventually_true(NoMatchedPublications(pair.reader)),
                   "ignore_publication ends the match");
        }
    }
    {
        IgnorePair pair(45, "IgnoreSubscription");
        const core::InstanceHandleSeq subscriptions = pub::matched_subscriptions(pair.writer);
        EXPECT(subscriptions.size() == 1, "the writer has its reader");
        if (subscriptions.size() == 1) {
            // The writer's participant ignores the remote reader: a subscription.
            sub::ignore(pair.writer_side, subscriptions[0]);
            EXPECT(eventually_true(NoMatchedSubscriptions(pair.writer)),
                   "ignore_subscription ends the match");
        }
    }
    {
        IgnorePair pair(46, "IgnoreTopic");
        core::InstanceHandle topic_handle;
        EXPECT(eventually_true(FindsDiscoveredTopic(pair.reader_side, "IgnoreTopic", &topic_handle)),
               "the reader's participant discovers the writer's topic");
        if (!topic_handle.is_nil()) {
            const std::vector<core::InstanceHandle> topics_to_ignore(1, topic_handle);
            topic::ignore(pair.reader_side, topics_to_ignore.begin(), topics_to_ignore.end());
            EXPECT(eventually_true(NoMatchedPublications(pair.reader)),
                   "ignore_topic ends the match of the endpoints of the topic");
        }
    }
}

// DDS-PSM-Cxx 1.0 §2.1, §7.5.1.2, §7.6: retain and close of an entity, the default QoS of a
// participant, the acknowledgments of a publisher and a writer, and the status operations that no
// other test calls.
static void test_entities_and_statuses_without_other_coverage() {
    namespace p = core::policy;

    // retain(): an entity nobody references stays in use, until close().
    {
        domain::DomainParticipant dp(D(27));
        dp.retain();
    }
    EXPECT(domain::find(D(27)) != core::null, "a retained participant outlives its last reference");
    {
        domain::DomainParticipant again = domain::find(D(27));
        again.close();
    }
    EXPECT(domain::find(D(27)) == core::null, "close() ends the retention");
    {
        domain::DomainParticipant plain(D(27));
    }
    EXPECT(domain::find(D(27)) == core::null, "a participant nobody retained goes with its last reference");

    domain::DomainParticipant dp(D(28));
    dp.assert_liveliness();
    EXPECT(dp.current_time().sec() > 1000000000, "the current time of a participant is wall-clock time");

    // The default QoS of the factory and of the participant, set and read back.
    const domain::qos::DomainParticipantQos factory_before = dp.default_participant_qos();
    domain::qos::DomainParticipantQos manual = factory_before;
    manual << p::EntityFactory::ManuallyEnable();
    dp.default_participant_qos(manual);
    EXPECT(!domain::DomainParticipant::default_participant_qos()
                .policy<p::EntityFactory>()
                .autoenable_created_entities(),
           "the default participant QoS is the one set");
    dp.default_participant_qos(factory_before);
    EXPECT(dp.default_participant_qos().policy<p::EntityFactory>().autoenable_created_entities(),
           "and restored");

    topic::qos::TopicQos tq;
    tq << p::History::KeepLast(7);
    dp.default_topic_qos(tq);
    EXPECT(dp.default_topic_qos().policy<p::History>().depth() == 7, "the default topic QoS");
    pub::qos::PublisherQos pq;
    pq << p::Partition("p1");
    dp.default_publisher_qos(pq);
    EXPECT(dp.default_publisher_qos().policy<p::Partition>().name().size() == 1 &&
               dp.default_publisher_qos().policy<p::Partition>().name()[0] == "p1",
           "the default publisher QoS");
    sub::qos::SubscriberQos sq;
    sq << p::Partition("s1");
    dp.default_subscriber_qos(sq);
    EXPECT(dp.default_subscriber_qos().policy<p::Partition>().name()[0] == "s1",
           "the default subscriber QoS");

    // The partitions set above would keep the writer and the reader apart (a publisher takes
    // the default publisher QoS, DDS 1.4 §2.2.2.4.1.2): back to the defaults.
    dp.default_publisher_qos(pub::qos::PublisherQos());
    dp.default_subscriber_qos(sub::qos::SubscriberQos());
    dp.default_topic_qos(topic::qos::TopicQos());

    // Writer, reader, acknowledgments, status getters.
    topic::Topic<core::ByteSeq> t(dp, "EntitiesWithoutCoverage");
    pub::Publisher pub_(dp);
    pub::DataWriter<core::ByteSeq> dw(pub_, t);
    sub::Subscriber sub_(dp);
    sub::DataReader<core::ByteSeq> dr(sub_, t);
    dw->wait_for_matched(1, core::Duration::from_secs(5));
    dr->wait_for_matched(1, core::Duration::from_secs(5));

    dw.assert_liveliness();
    EXPECT(dw.liveliness_lost_status().total_count() == 0, "a writer has lost no liveliness");
    EXPECT(dw.offered_deadline_missed_status().total_count() == 0, "a writer has missed no deadline");
    EXPECT(dw.offered_deadlined_missed_status().total_count() == 0,
           "offered_deadlined_missed_status is the same status under the name of the normative header");
    EXPECT(dw.offered_incompatible_qos_status().total_count() == 0, "a writer has no incompatible reader");
    EXPECT(dw.offered_incompatible_qos_staus().total_count() == 0,
           "offered_incompatible_qos_staus is the same status under the name of the normative header");
    EXPECT(dr.sample_rejected_status().total_count() == 0, "a reader has rejected no sample");
    EXPECT(dr.liveliness_changed_status().alive_count() >= 0, "the liveliness-changed status of a reader");
    EXPECT(dr.requested_deadline_missed_status().total_count() == 0, "a reader has missed no deadline");
    EXPECT(dr.requested_incompatible_qos_status().total_count() == 0, "a reader has no incompatible writer");
    dr.wait_for_historical_data(core::Duration::from_secs(1));

    dw.write(bytes(1));
    pub_.wait_for_acknowledgments(core::Duration::from_secs(5));
    dw.wait_for_acknowledgments(core::Duration::from_secs(5));
    pub::AnyDataWriter any_writer(dw);
    any_writer.wait_for_acknowledgments(core::Duration::from_secs(5));

    // default_filter_state() is the state every select() starts from.
    const uint32_t data_available = 1u << 10;
    const uint64_t deadline = ::zerodds::cxx::monotonic_microseconds() + 5 * 1000000u;
    while ((dr.status_changes().to_ulong() & data_available) == 0 &&
           ::zerodds::cxx::monotonic_microseconds() < deadline) {
        ::zerodds::cxx::sleep_milliseconds(10);
    }
    EXPECT(dr.default_filter_state() == sub::status::DataState::any(), "the default filter state is any");
    dr.default_filter_state(sub::status::DataState(sub::status::SampleState::not_read()));
    EXPECT(dr.select().read().length() == 1, "select() starts from the default filter state");
    EXPECT(dr.select().read().length() == 0, "the sample just read is not NOT_READ any more");

    // The streaming selector has a read mode.
    sub::DataReader<core::ByteSeq>::ManipulatorSelector manipulator(dr);
    EXPECT(!manipulator.read_mode(), "the streaming selector takes by default");
    manipulator.read_mode(true);
    EXPECT(manipulator.read_mode(), "and reads once read_mode is set");

    // The status condition of a participant, with its enabled statuses.
    core::cond::StatusCondition status_condition(dp);
    status_condition.enabled_statuses(core::status::StatusMask::data_on_readers());
    EXPECT(status_condition.enabled_statuses() == core::status::StatusMask::data_on_readers(),
           "enabled_statuses of a status condition");
    core::cond::WaitSet waitset;
    waitset.attach_condition(status_condition);
    EXPECT(waitset.conditions().size() == 1, "attach_condition attaches the condition");
    EXPECT(waitset.detach_condition(status_condition), "and detach_condition detaches it");

    // wait() ends with TimeoutError when no attached condition holds.
    core::cond::GuardCondition quiet;
    core::cond::WaitSet idle;
    idle += quiet;
    bool timed_out = false;
    try {
        (void)idle.wait(core::Duration::from_millisecs(50));
    } catch (const core::TimeoutError&) {
        timed_out = true;
    }
    EXPECT(timed_out, "wait on a condition that does not hold is TimeoutError");
    quiet.trigger_value(true);
    core::cond::WaitSet::ConditionSeq active;
    idle.wait(active, core::Duration::from_millisecs(500));
    EXPECT(active.size() == 1, "wait(ConditionSeq&, timeout) returns the conditions that hold");
}

// DDS-PSM-Cxx 1.0 §2.1, §7.3: every callback of the writer, reader and participant listeners is
// installed in the vtable of the C API and reaches its virtual method with the triggering entity
// (the vtables are invoked as the runtime's poll does).
namespace {
struct AllWriterCallbacks : pub::NoOpDataWriterListener<core::ByteSeq> {
    AllWriterCallbacks() : deadline(0), incompatible(0), seen(nullptr) {}
    int deadline, incompatible;
    zerodds_ZeroDdsDataWriter* seen;
    void on_offered_deadline_missed(pub::DataWriter<core::ByteSeq>& w,
                                    const core::status::OfferedDeadlineMissedStatus&) ZERODDS_OVERRIDE {
        ++deadline;
        seen = w->native_handle();
    }
    void on_offered_incompatible_qos(pub::DataWriter<core::ByteSeq> w,
                                     const core::status::OfferedIncompatibleQosStatus&) ZERODDS_OVERRIDE {
        ++incompatible;
        seen = w->native_handle();
    }
};
struct AllReaderCallbacks : sub::NoOpDataReaderListener<core::ByteSeq> {
    AllReaderCallbacks() : rejected(0), liveliness(0), deadline(0), incompatible(0), matched(0), lost(0), seen(nullptr) {}
    int rejected, liveliness, deadline, incompatible, matched, lost;
    zerodds_ZeroDdsDataReader* seen;
    void on_sample_rejected(sub::DataReader<core::ByteSeq>& r,
                            const core::status::SampleRejectedStatus&) ZERODDS_OVERRIDE {
        ++rejected;
        seen = r->native_handle();
    }
    void on_liveliness_changed(sub::DataReader<core::ByteSeq>& r,
                               const core::status::LivelinessChangedStatus&) ZERODDS_OVERRIDE {
        ++liveliness;
        seen = r->native_handle();
    }
    void on_requested_deadline_missed(sub::DataReader<core::ByteSeq>& r,
                                      const core::status::RequestedDeadlineMissedStatus&) ZERODDS_OVERRIDE {
        ++deadline;
        seen = r->native_handle();
    }
    void on_requested_incompatible_qos(
        sub::DataReader<core::ByteSeq>& r,
        const core::status::RequestedIncompatibleQosStatus&) ZERODDS_OVERRIDE {
        ++incompatible;
        seen = r->native_handle();
    }
    void on_subscription_matched(sub::DataReader<core::ByteSeq>& r,
                                 const core::status::SubscriptionMatchedStatus&) ZERODDS_OVERRIDE {
        ++matched;
        seen = r->native_handle();
    }
    void on_sample_lost(sub::DataReader<core::ByteSeq>& r,
                        const core::status::SampleLostStatus&) ZERODDS_OVERRIDE {
        ++lost;
        seen = r->native_handle();
    }
};
struct AllParticipantCallbacks : domain::NoOpDomainParticipantListener {
    AllParticipantCallbacks() : inconsistent(0), on_readers(0), subscriber_seen(nullptr) {}
    int inconsistent, on_readers;
    zerodds_ZeroDdsSubscriber* subscriber_seen;
    std::string topic_seen;
    void on_inconsistent_topic(topic::AnyTopic& t,
                               const core::status::InconsistentTopicStatus&) ZERODDS_OVERRIDE {
        ++inconsistent;
        topic_seen = t->name();
    }
    void on_data_on_readers(sub::Subscriber& s) ZERODDS_OVERRIDE {
        ++on_readers;
        subscriber_seen = s->native_handle();
    }
};
struct AllPublisherCallbacks : pub::NoOpPublisherListener {
    AllPublisherCallbacks() : deadline(0), lost(0), incompatible(0), matched(0) {}
    int deadline, lost, incompatible, matched;
    std::string topic_seen;
    void on_offered_deadline_missed(pub::AnyDataWriter& w, const core::status::OfferedDeadlineMissedStatus&) ZERODDS_OVERRIDE {
        ++deadline;
        topic_seen = w->topic_name();
    }
    void on_liveliness_lost(pub::AnyDataWriter&, const core::status::LivelinessLostStatus&) ZERODDS_OVERRIDE { ++lost; }
    void on_offered_incompatible_qos(pub::AnyDataWriter&, const core::status::OfferedIncompatibleQosStatus&) ZERODDS_OVERRIDE {
        ++incompatible;
    }
    void on_publication_matched(pub::AnyDataWriter&, const core::status::PublicationMatchedStatus&) ZERODDS_OVERRIDE {
        ++matched;
    }
};
struct AllSubscriberCallbacks : sub::NoOpSubscriberListener {
    AllSubscriberCallbacks()
        : on_readers(0), available(0), rejected(0), lost(0), liveliness(0), matched(0), deadline(0), incompatible(0) {}
    int on_readers, available, rejected, lost, liveliness, matched, deadline, incompatible;
    std::string topic_seen;
    void on_data_on_readers(sub::Subscriber&) ZERODDS_OVERRIDE { ++on_readers; }
    void on_data_available(sub::AnyDataReader& r) ZERODDS_OVERRIDE {
        ++available;
        topic_seen = r->topic_name();
    }
    void on_sample_rejected(sub::AnyDataReader&, const core::status::SampleRejectedStatus&) ZERODDS_OVERRIDE { ++rejected; }
    void on_sample_lost(sub::AnyDataReader&, const core::status::SampleLostStatus&) ZERODDS_OVERRIDE { ++lost; }
    void on_liveliness_changed(sub::AnyDataReader&, const core::status::LivelinessChangedStatus&) ZERODDS_OVERRIDE {
        ++liveliness;
    }
    void on_subscription_matched(sub::AnyDataReader&, const core::status::SubscriptionMatchedStatus&) ZERODDS_OVERRIDE {
        ++matched;
    }
    void on_requested_deadline_missed(sub::AnyDataReader&, const core::status::RequestedDeadlineMissedStatus&) ZERODDS_OVERRIDE {
        ++deadline;
    }
    void on_requested_incompatible_qos(sub::AnyDataReader&,
                                       const core::status::RequestedIncompatibleQosStatus&) ZERODDS_OVERRIDE {
        ++incompatible;
    }
};
struct AllTopicCallbacks : topic::NoOpTopicListener<core::ByteSeq> {
    AllTopicCallbacks() : inconsistent(0) {}
    int inconsistent;
    std::string name_seen;
    void on_inconsistent_topic(topic::Topic<core::ByteSeq>& t, const core::status::InconsistentTopicStatus&) ZERODDS_OVERRIDE {
        ++inconsistent;
        name_seen = t.name();
    }
};
} // namespace

static void test_every_listener_callback_reaches_its_virtual_method() {
    domain::DomainParticipant dp(D(29));
    topic::Topic<core::ByteSeq> t(dp, "EveryCallback");
    pub::Publisher pub_(dp);
    pub::DataWriter<core::ByteSeq> dw(pub_, t);
    sub::Subscriber sub_(dp);
    sub::DataReader<core::ByteSeq> dr(sub_, t);

    AllWriterCallbacks wl;
    dw.listener(&wl, core::status::StatusMask::all());
    const zerodds_ZeroDdsDataWriterListener* wvt = zerodds_dw_get_listener(dw->native_handle());
    EXPECT(wvt != nullptr && wvt->on_offered_deadline_missed != nullptr &&
               wvt->on_offered_incompatible_qos != nullptr,
           "the writer vtable has the two remaining shims");
    if (wvt != nullptr && wvt->on_offered_deadline_missed != nullptr &&
        wvt->on_offered_incompatible_qos != nullptr) {
        wvt->on_offered_deadline_missed(wvt->user_data, dw->native_handle());
        wvt->on_offered_incompatible_qos(wvt->user_data, dw->native_handle());
        EXPECT(wl.deadline == 1 && wl.incompatible == 1, "each shim calls its own virtual method");
        EXPECT(wl.seen == dw->native_handle(), "with the writer that triggered it");
    }

    AllReaderCallbacks rl;
    dr.listener(&rl, core::status::StatusMask::all());
    const zerodds_ZeroDdsDataReaderListener* rvt = zerodds_dr_get_listener(dr->native_handle());
    EXPECT(rvt != nullptr, "the reader vtable is registered");
    if (rvt != nullptr) {
        rvt->on_sample_rejected(rvt->user_data, dr->native_handle());
        rvt->on_liveliness_changed(rvt->user_data, dr->native_handle());
        rvt->on_requested_deadline_missed(rvt->user_data, dr->native_handle());
        rvt->on_requested_incompatible_qos(rvt->user_data, dr->native_handle());
        rvt->on_subscription_matched(rvt->user_data, dr->native_handle());
        rvt->on_sample_lost(rvt->user_data, dr->native_handle());
        EXPECT(rl.rejected == 1 && rl.liveliness == 1 && rl.deadline == 1 && rl.incompatible == 1 &&
                   rl.matched == 1 && rl.lost == 1,
               "each of the six remaining shims calls its own virtual method");
        EXPECT(rl.seen == dr->native_handle(), "with the reader that triggered it");
    }

    AllParticipantCallbacks pl;
    dp.listener(&pl, core::status::StatusMask::all());
    const zerodds_ZeroDdsDomainParticipantListener* pvt = zerodds_dp_get_listener(dp->native_handle());
    EXPECT(pvt != nullptr && pvt->on_inconsistent_topic != nullptr && pvt->on_data_on_readers != nullptr,
           "the participant vtable has both shims");
    if (pvt != nullptr && pvt->on_inconsistent_topic != nullptr && pvt->on_data_on_readers != nullptr) {
        pvt->on_inconsistent_topic(pvt->user_data, t->native_handle());
        pvt->on_data_on_readers(pvt->user_data, sub_->native_handle());
        EXPECT(pl.inconsistent == 1 && pl.topic_seen == "EveryCallback",
               "on_inconsistent_topic gets the topic");
        EXPECT(pl.on_readers == 1 && pl.subscriber_seen == sub_->native_handle(),
               "on_data_on_readers gets the subscriber");
    }
    dw.listener(nullptr, core::status::StatusMask::none());
    dr.listener(nullptr, core::status::StatusMask::none());
    dp.listener(nullptr, core::status::StatusMask::none());
    // The tables of publisher, subscriber and topic.
    AllPublisherCallbacks publisher_listener;
    pub_.listener(&publisher_listener, core::status::StatusMask::all());
    const zerodds_ZeroDdsPublisherListener* pubvt = zerodds_pub_get_listener(pub_->native_handle());
    EXPECT(pubvt != nullptr, "the publisher vtable is registered");
    if (pubvt != nullptr) {
        pubvt->on_offered_deadline_missed(pubvt->user_data, dw->native_handle());
        pubvt->on_liveliness_lost(pubvt->user_data, dw->native_handle());
        pubvt->on_offered_incompatible_qos(pubvt->user_data, dw->native_handle());
        pubvt->on_publication_matched(pubvt->user_data, dw->native_handle());
        EXPECT(publisher_listener.deadline == 1 && publisher_listener.lost == 1 &&
                   publisher_listener.incompatible == 1 && publisher_listener.matched == 1,
               "each callback of the publisher table reaches its virtual method");
        EXPECT(publisher_listener.topic_seen == "EveryCallback", "with the writer that triggered it");
    }
    AllSubscriberCallbacks subscriber_listener;
    sub_.listener(&subscriber_listener, core::status::StatusMask::all());
    const zerodds_ZeroDdsSubscriberListener* subvt = zerodds_sub_get_listener(sub_->native_handle());
    EXPECT(subvt != nullptr, "the subscriber vtable is registered");
    if (subvt != nullptr) {
        subvt->on_data_on_readers(subvt->user_data, sub_->native_handle());
        subvt->on_data_available(subvt->user_data, dr->native_handle());
        subvt->on_sample_rejected(subvt->user_data, dr->native_handle());
        subvt->on_sample_lost(subvt->user_data, dr->native_handle());
        subvt->on_liveliness_changed(subvt->user_data, dr->native_handle());
        subvt->on_subscription_matched(subvt->user_data, dr->native_handle());
        subvt->on_requested_deadline_missed(subvt->user_data, dr->native_handle());
        subvt->on_requested_incompatible_qos(subvt->user_data, dr->native_handle());
        EXPECT(subscriber_listener.on_readers == 1 && subscriber_listener.available == 1 &&
                   subscriber_listener.rejected == 1 && subscriber_listener.lost == 1 &&
                   subscriber_listener.liveliness == 1 && subscriber_listener.matched == 1 &&
                   subscriber_listener.deadline == 1 && subscriber_listener.incompatible == 1,
               "each callback of the subscriber table reaches its virtual method");
        EXPECT(subscriber_listener.topic_seen == "EveryCallback", "with the reader that triggered it");
    }
    AllTopicCallbacks topic_listener;
    t.listener(&topic_listener, core::status::StatusMask::all());
    const zerodds_ZeroDdsTopicListener* topicvt = zerodds_topic_get_listener(t->native_handle());
    EXPECT(topicvt != nullptr, "the topic vtable is registered");
    if (topicvt != nullptr) {
        topicvt->on_inconsistent_topic(topicvt->user_data, t->native_handle());
        EXPECT(topic_listener.inconsistent == 1 && topic_listener.name_seen == "EveryCallback",
               "the callback of the topic table reaches its virtual method with the topic");
    }
    pub_.listener(nullptr, core::status::StatusMask::none());
    sub_.listener(nullptr, core::status::StatusMask::none());
    t.listener(nullptr, core::status::StatusMask::none());
}

// DDS-PSM-Cxx 1.0 §7.5.4: SampleInfo and its parts, from the values they are built with; §7.6.2.1:
// the QoS kinds of a QosProvider that the other test does not read; §2.1: the expression of a
// ContentFilteredTopic.
static void test_sample_info_parts_qos_provider_kinds_and_filter_expression() {
    namespace p = core::policy;

    const sub::SampleInfo info(core::Time(5, 6),
                               sub::status::DataState(sub::status::SampleState::not_read(),
                                                      sub::status::ViewState::new_view(),
                                                      sub::status::InstanceState::not_alive_disposed()),
                               sub::GenerationCount(1, 2), sub::Rank(3, 4, 5), true,
                               core::InstanceHandle(7), core::InstanceHandle(8));
    EXPECT(info.timestamp() == core::Time(5, 6), "SampleInfo::timestamp");
    EXPECT(info.state().sample_state() == sub::status::SampleState::not_read(), "SampleInfo sample_state");
    EXPECT(info.state().view_state() == sub::status::ViewState::new_view(), "SampleInfo view_state");
    EXPECT(info.state().instance_state() == sub::status::InstanceState::not_alive_disposed(),
           "SampleInfo instance_state");
    EXPECT(info.generation_count().disposed() == 1 && info.generation_count().no_writers() == 2,
           "SampleInfo generation_count: disposed and no-writers generation count");
    EXPECT(info.rank().sample() == 3 && info.rank().absolute_generation() == 4 &&
               info.rank().generation() == 5,
           "SampleInfo rank: sample, absolute generation and generation rank");
    EXPECT(info.valid() && info.instance_handle().value() == 7 && info.publication_handle().value() == 8,
           "SampleInfo valid, instance_handle and publication_handle");
    EXPECT(sub::status::InstanceState::not_alive_mask() ==
               sub::status::InstanceState(sub::status::InstanceState::not_alive_disposed().to_ulong() |
                                          sub::status::InstanceState::not_alive_no_writers().to_ulong()),
           "NOT_ALIVE_MASK is both not-alive states");
    EXPECT(sub::status::ViewState::not_new_view() != sub::status::ViewState::new_view() &&
               sub::status::SampleState::read() != sub::status::SampleState::not_read(),
           "the view and sample states differ");

    // A QosProvider reads the participant, subscriber and data reader QoS of a profile.
    const std::string path = "/tmp/zerodds_smoke_qos_kinds_" + ::zerodds::cxx::to_string(static_cast<int>(::getpid())) + ".xml";
    {
        std::ofstream xml(path.c_str());
        xml << "<dds><qos_library name=\"K\"><qos_profile name=\"P\">"
               "<domain_participant_qos><user_data><value>cmF3</value></user_data>"
               "</domain_participant_qos>"
               "<subscriber_qos><partition><name><element>S1</element></name></partition>"
               "</subscriber_qos>"
               "<datareader_qos><reliability><kind>RELIABLE_RELIABILITY_QOS</kind></reliability>"
               "<history><kind>KEEP_ALL_HISTORY_QOS</kind></history></datareader_qos>"
               "</qos_profile></qos_library></dds>";
    }
    core::QosProvider provider("file://" + path, "K::P");
    EXPECT(provider.participant_qos().policy<p::UserData>().value() == bytes('r', 'a', 'w'),
           "QosProvider::participant_qos");
    EXPECT(provider.participant_qos("K::P").policy<p::UserData>().value().size() == 3,
           "QosProvider::participant_qos by profile id");
    EXPECT(provider.subscriber_qos().policy<p::Partition>().name()[0] == "S1",
           "QosProvider::subscriber_qos");
    EXPECT(provider.subscriber_qos("K::P").policy<p::Partition>().name().size() == 1,
           "QosProvider::subscriber_qos by profile id");
    EXPECT(provider.datareader_qos().policy<p::Reliability>().kind() == p::ReliabilityKind::RELIABLE &&
               provider.datareader_qos().policy<p::History>().kind() == p::HistoryKind::KEEP_ALL,
           "QosProvider::datareader_qos");
    EXPECT(provider.datareader_qos("K::P").policy<p::History>().kind() == p::HistoryKind::KEEP_ALL,
           "QosProvider::datareader_qos by profile id");
    std::remove(path.c_str());

    // The expression of a ContentFilteredTopic, with its parameters.
    domain::DomainParticipant dp(D(30));
    topic::Topic<core::ByteSeq> t(dp, "FilterExpressionTopic");
    topic::ContentFilteredTopic<core::ByteSeq> cft(t, "FilterExpression", topic::Filter("x > %0", strs("3")));
    EXPECT(cft.filter_expression() == "x > %0", "ContentFilteredTopic::filter_expression");
    EXPECT(cft.topic().name() == "FilterExpressionTopic", "ContentFilteredTopic::topic is the related topic");
    EXPECT(cft.filter_parameters().size() == 1 && cft.filter_parameters()[0] == "3",
           "and its parameters");
}

// DDS-PSM-Cxx 1.0 §7.11, DDS-XTypes 1.3 §7.5: a DynamicType built in C++ has the members, keys and
// annotations it was given, a DynamicData of it round-trips through the wire and through JSON, the
// TypeProvider loads types from a URI, and a Topic<DynamicData> carries samples between a writer
// and a reader, with a query on a field.
// XTypes kinds beyond the primitive ones: a bit mask, a union with a default branch and a union
// that switches on an enumeration. Built from classes, written to the C API as IDL, encoded and
// decoded, and loaded back from IDL text by the TypeProvider.
static std::string percent_encoded(const std::string& text) {
    static const char* hex = "0123456789ABCDEF";
    std::string out;
    for (size_t k = 0; k < text.size(); ++k) {
        const unsigned char c = static_cast<unsigned char>(text[k]);
        if (std::isalnum(c) != 0) {
            out.push_back(static_cast<char>(c));
        } else {
            out.push_back('%');
            out.push_back(hex[c >> 4]);
            out.push_back(hex[c & 0xF]);
        }
    }
    return out;
}

static void test_bit_masks_union_defaults_and_enum_discriminators() {
    using namespace dds::core::xtypes;

    BitMaskType perm("m::Perm", 16, vec(BitFlag("READ", 0), BitFlag("WRITE", 3), BitFlag("EXEC", 4)));
    EXPECT(perm.kind() == TypeKind::BITMASK_TYPE && perm.bit_bound() == 16 && perm.flags().size() == 3 &&
               perm.flags()[1].position == 3,
           "BitMaskType: kind, bit bound and flags");
    bool refused = false;
    try {
        BitMaskType bad("m::Bad", 4, vec(BitFlag("HIGH", 7)));
    } catch (const core::InvalidArgumentError&) {
        refused = true;
    }
    EXPECT(refused, "a flag beyond the bit bound is refused");
    StructType file("m::File", StructType(),
                    vec(MemberType("id", PrimitiveType<int32_t>()), MemberType("perm", perm)));
    DynamicData f(file);
    f.value("id", int32_t(1));
    f.value("perm", strset("READ", "EXEC"));
    DynamicData f_back(file);
    f_back.decode(f.encode());
    EXPECT(f_back == f, "a bit mask member round-trips through the codec");
    EXPECT((f_back.value<std::set<std::string> >("perm") == strset("READ", "EXEC")),
           "and comes back as the set of its flags");

    // A union with a default branch and one member under two labels.
    const MemberType a("a", PrimitiveType<int32_t>());
    const MemberType b("b", StringType<char>());
    const MemberType c("c", PrimitiveType<double>());
    UnionType<int32_t> wide("m::Wide", PrimitiveType<int32_t>(),
                            vec(UnionCase<int32_t>(1, a), UnionCase<int32_t>(2, b),
                                UnionCase<int32_t>(5, b), UnionCase<int32_t>(c)));
    EXPECT(wide.members().size() == 4 && wide.members()[3].is_default() && !wide.members()[0].is_default() &&
               wide.member("c").name() == "c",
           "UnionCase has a default branch, found by name");
    EXPECT(wide.discriminator_type().kind() == TypeKind::INT_32_TYPE, "the discriminator type");
    StructType holder("m::Holder", StructType(), vec(MemberType("w", wide)));
    DynamicData h(holder);
    h.from_json_text("{\"w\":{\"_d\":7,\"c\":1.5}}");
    DynamicData h_back(holder);
    h_back.decode(h.encode());
    EXPECT(h_back == h && h_back.to_json_text().find("\"_d\":7") != std::string::npos,
           "the value of the default branch round-trips");
    EXPECT(wide.add_member(UnionCase<int32_t>(9, a)).members().size() == 5, "add_member keeps the default");
    EXPECT(wide.remove_member(UnionCase<int32_t>(c)).members().size() == 3, "remove_member of the default");

    // A union that switches on an enumeration.
    EnumerationType<uint32_t> level("m::Level", vec(Enumerator("LOW"), Enumerator("HIGH", 5)));
    UnionType<uint32_t> by_level("m::ByLevel", level, vec(UnionCase<uint32_t>(0, a), UnionCase<uint32_t>(5, b)));
    EXPECT(by_level.discriminator_type().kind() == TypeKind::ENUMERATION_TYPE,
           "the discriminator of a union may be an enumeration");
    StructType holder2("m::Holder2", StructType(), vec(MemberType("u", by_level)));
    DynamicData h2(holder2);
    h2.from_json_text("{\"u\":{\"_d\":\"HIGH\",\"b\":\"hi\"}}");
    DynamicData h2_back(holder2);
    h2_back.decode(h2.encode());
    EXPECT(h2_back == h2 && h2_back.to_json_text().find("\"HIGH\"") != std::string::npos,
           "a union on an enumeration round-trips with the enumerator as its discriminator");

    // The TypeProvider builds the same kinds from IDL text.
    const std::string idl =
        "enum E { X, Y, Z }; bitmask Flags { A, B, C }; "
        "union W switch (long) { case 1: long a; case 2: case 5: string b; default: double c; }; "
        "union Q switch (E) { case X: long a; case Y: string b; };";
    const std::vector<DynamicType> loaded = TypeProvider::load_types("data:text/plain," + percent_encoded(idl));
    std::map<std::string, DynamicType> by_name;
    for (std::vector<DynamicType>::const_iterator t = loaded.begin(); t != loaded.end(); ++t) {
        by_name.insert(std::make_pair(t->name(), *t));
    }
    EXPECT(by_name.count("Flags") == 1 && by_name.at("Flags").kind() == TypeKind::BITMASK_TYPE,
           "TypeProvider loads a bit mask");
    EXPECT(by_name.count("W") == 1 && by_name.at("W").kind() == TypeKind::UNION_TYPE,
           "TypeProvider loads a union with a default branch");
    EXPECT(by_name.count("Q") == 1 && by_name.at("Q").kind() == TypeKind::UNION_TYPE,
           "TypeProvider loads a union on an enumeration");
    // `fixed` has no kind in the XTypes type system (DDSXTY14-28 asks for it, §7.3.4.2 has none):
    // a structure with a `fixed` member is no DynamicType, and the loader says so.
    bool fixed_refused = false;
    try {
        (void)TypeProvider::load_type("data:text/plain," + percent_encoded("struct F { fixed<5,2> v; };"));
    } catch (const core::UnsupportedError&) {
        fixed_refused = true;
    } catch (const core::Exception&) {
        fixed_refused = true;
    }
    EXPECT(fixed_refused, "a structure with a fixed member is not loaded as a DynamicType");
    if (by_name.count("W") == 1) {
        const UnionType<int32_t> w(::zerodds::cxx::static_pointer_cast<detail::UnionType<int32_t> >(by_name.at("W").delegate()));
        EXPECT(w.members().size() == 4 && w.members()[3].is_default() && w.members()[2].discriminator() == 5,
               "the loaded union has its labels and its default branch");
    }
}

static void test_dynamic_types_dynamic_data_and_type_provider() {
    using namespace dds::core::xtypes;
    EnumerationType<uint32_t> color("m::Color", vec(Enumerator("RED"), Enumerator("GREEN", 5)));
    AliasType label("m::Label", StringType<char>(20));
    BitSetType flags("m::Flags", vec(BitField("low", 3), BitField("high", 5)));
    StructType s("m::Track", StructType(),
                 vec(MemberType("id", PrimitiveType<int32_t>(), annotation::Key()),
                     MemberType("label", label), MemberType("n", PrimitiveType<int64_t>()),
                     MemberType("color", color), MemberType("flags", flags)));
    EXPECT(s.name() == "m::Track" && s.kind() == TypeKind::STRUCTURE_TYPE, "StructType name and kind");
    EXPECT(s.members().size() == 5, "StructType members");
    EXPECT(isKey(s.member("id")) && !isKey(s.member("n")), "isKey(MemberType)");
    EXPECT(s.member("n").type().kind() == TypeKind::INT_64_TYPE, "the type of a member");
    EXPECT(!isPrimitiveType(s) && isConstructedType(s) && isAggregationType(s) && !isCollectionType(s),
           "the kind predicates of a structure");
    EXPECT(isPrimitiveType(PrimitiveType<int32_t>()), "the kind predicates of a primitive type");
    EXPECT(isCollectionType(StringType<char>(20)), "the kind predicates of a string type");

    DynamicData d(s);
    d.value("id", int32_t(7));
    d.value("label", std::string("x"));
    d.value("n", int64_t(5));
    d.value("color", std::string("GREEN"));
    std::map<std::string, uint64_t> flag_values;
    flag_values["low"] = 3;
    flag_values["high"] = 9;
    d.value("flags", flag_values);
    const std::vector<uint8_t> wire = d.encode();
    DynamicData e(s);
    e.decode(wire);
    EXPECT(e == d, "a DynamicData decodes to what was encoded");
    EXPECT(e.value<int32_t>("id") == 7 && e.value<std::string>("color") == "GREEN" &&
               e.value<int64_t>("n") == 5,
           "the members of the decoded value");
    EXPECT((e.value<std::map<std::string, uint64_t> >("flags").at("high") == 9), "a bit set member");
    EXPECT(e.member_id("n") == 2, "the id of a member");
    EXPECT(e.member_type("n").name() == "n", "the type of a member of a DynamicData");
    DynamicData from_text(s);
    from_text.from_json_text(d.to_json_text());
    EXPECT(from_text == d, "a DynamicData round-trips through JSON");
    const std::vector<uint8_t> xcdr1 = d.encode(false, true);
    DynamicData old(s);
    old.decode(xcdr1, false, true);
    EXPECT(old == d, "and through XCDR1 big-endian");

    // An array member: written as IDL `long cells[2][3]`, round-tripped through the codec.
    ArrayType cells(PrimitiveType<int32_t>(), vec(static_cast<uint32_t>(2), static_cast<uint32_t>(3)));
    EXPECT(cells.kind() == TypeKind::ARRAY_TYPE && cells.dimensions().size() == 2 &&
               cells.dimensions()[1] == 3 && cells.element_type().kind() == TypeKind::INT_32_TYPE,
           "ArrayType kind, extents and element type");
    StructType grid("m::Grid", StructType(), vec(MemberType("id", PrimitiveType<int32_t>()), MemberType("cells", cells)));
    DynamicData grid_value(grid);
    grid_value.from_json_text("{\"id\":1,\"cells\":[[1,2,3],[4,5,6]]}");
    DynamicData grid_back(grid);
    grid_back.decode(grid_value.encode());
    EXPECT(grid_back == grid_value, "a structure with an array member round-trips through the codec");
    EXPECT(grid_back.to_json_text().find("[[1,2,3],[4,5,6]]") != std::string::npos,
           "and the array comes back with its extents");

    // The members of a structure change by building a new one.
    StructType bigger = s.add_member(MemberType("extra", PrimitiveType<uint8_t>()));
    EXPECT(bigger.members().size() == 6 && s.members().size() == 5, "add_member builds a new structure");
    EXPECT(bigger.remove_member(bigger.member("extra")).members().size() == 5, "remove_member");
    StructType annotated = s.add_annotation(annotation::Mutable());
    EXPECT(isMutable(annotated) && !isMutable(s), "add_annotation builds a new structure");
    EXPECT(!isMutable(annotated.remove_annotation(annotation::Mutable())), "remove_annotation");
    EXPECT(isExtensible(StructType("m::Base", StructType(), vec(MemberType("a", PrimitiveType<int32_t>())))) &&
               !isFinal(s) && !isNested(s),
           "a structure is extensible by default, neither final nor nested");
    StructType child("m::Child", s, vec(MemberType("c", PrimitiveType<int32_t>())));
    EXPECT(child.parent().name() == "m::Track", "the parent of a structure");

    // The collection types.
    SequenceType seq(PrimitiveType<double>(), 8);
    EXPECT(seq.bounds() == 8 && seq.key_type().kind() == TypeKind::FLOAT_64_TYPE, "SequenceType");
    MapType map(PrimitiveType<int32_t>(), StringType<char>(), 4);
    EXPECT(map.bounds() == 4 && map.key_type().kind() == TypeKind::INT_32_TYPE &&
               map.value_type().kind() == TypeKind::STRING_TYPE,
           "MapType");

    // Annotations.
    EXPECT(annotation::Key().kind() == AnnotationKind::KEY_ANNOTATION_TYPE, "Key annotation");
    EXPECT(annotation::Id(9).kind() == AnnotationKind::ID_ANNOTATION_TYPE, "Id annotation");
    EXPECT(annotation::Final().kind() == AnnotationKind::EXTENSIBILITY_ANNOTATION_TYPE &&
               annotation::Extensible().kind() == AnnotationKind::EXTENSIBILITY_ANNOTATION_TYPE &&
               annotation::Mutable().kind() == AnnotationKind::EXTENSIBILITY_ANNOTATION_TYPE,
           "Extensibility annotations");
    EXPECT(annotation::Optional().kind() == AnnotationKind::OPTIONAL_ANNOTATION_TYPE &&
               annotation::Shared().kind() == AnnotationKind::SHARED_ANNOTATION_TYPE &&
               annotation::MustUnderstand().kind() == AnnotationKind::MUST_UNDERSTAND_ANNOTATION_TYPE &&
               annotation::Nested().kind() == AnnotationKind::NESTED_ANNOTATION_TYPE,
           "Optional, Shared, MustUnderstand and Nested annotations");
    const MemberType flagged("m", PrimitiveType<int32_t>(),
                             vec<Annotation>(annotation::Optional(), annotation::MustUnderstand(),
                                             annotation::Shared(), annotation::Id(9)));
    EXPECT(isOptional(flagged) && isMustUnderstand(flagged) && isShared(flagged) && hasId(flagged) &&
               getId(flagged) == 9 && !isKey(flagged),
           "isOptional, isMustUnderstand, isShared, hasId and getId of a member");

    EXPECT(annotation::Verbatim("text").kind() == AnnotationKind::VERBATIM_ANNOTATION_TYPE &&
               annotation::Verbatim("text").verbatim_text() == "text",
           "Verbatim annotation");
    EXPECT(annotation::Bitset().kind() == AnnotationKind::BITSET_ANNOTATION_TYPE, "Bitset annotation");
    EXPECT(annotation::BitBound(8).kind() == AnnotationKind::BIT_BOUND_ANNOTATION_TYPE, "BitBound annotation");
    EXPECT(annotation::Extensibility(ExtensibilityKind::MUTABLE).extensibility_kind() == ExtensibilityKind::MUTABLE &&
               annotation::Final().extensibility_kind() == ExtensibilityKind::FINAL &&
               annotation::Extensible().extensibility_kind() == ExtensibilityKind::EXTENSIBLE,
           "the extensibility annotations name their kind");

    // The TypeProvider: types from a URI become DynamicTypes.
    const DynamicType first = TypeProvider::load_type(
        "data:text/plain,struct%20T%20%7B%20long%20x%3B%20string%20name%3B%20%7D%3B");
    EXPECT(first.name() == "T" && first.kind() == TypeKind::STRUCTURE_TYPE,
           "TypeProvider::load_type names the structure");
    const std::vector<DynamicType> all = TypeProvider::load_types(
        "data:text/plain,struct%20A%20%7B%20long%20x%3B%20%7D%3B%20struct%20B%20%7B%20double%20y%3B%20%7D%3B");
    EXPECT(all.size() == 2, "TypeProvider::load_types gives every type of the document");
    const DynamicType with_array = TypeProvider::load_type("data:text/plain,struct%20U%20%7B%20long%20x%5B3%5D%3B%20%7D%3B");
    EXPECT(with_array.name() == "U" && StructType(::zerodds::cxx::static_pointer_cast<detail::StructType>(with_array.delegate()))
                   .member("x")
                   .type()
                   .kind() == TypeKind::ARRAY_TYPE,
           "TypeProvider loads a structure with an array member");
    // Topic<DynamicData>: a writer and a reader of a type that only exists at run time.
    StructType sensor("m::Sensor", StructType(),
                      vec(MemberType("id", PrimitiveType<int32_t>(), annotation::Key()),
                          MemberType("value", PrimitiveType<double>())));
    domain::DomainParticipant dp(D(31));
    topic::Topic<DynamicData> sensors(dp, "DynamicSensors", sensor);
    pub::qos::DataWriterQos wq;
    wq << core::policy::Reliability::Reliable() << core::policy::History::KeepAll();
    sub::qos::DataReaderQos rq;
    rq << core::policy::Reliability::Reliable() << core::policy::History::KeepAll();
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    pub::DataWriter<DynamicData> writer(pub_, sensors, wq);
    sub::DataReader<DynamicData> reader(sub_, sensors, rq);
    writer->wait_for_matched(1, core::Duration::from_secs(5));
    reader->wait_for_matched(1, core::Duration::from_secs(5));
    for (int i = 0; i < 2; ++i) {
        DynamicData sample(sensor);
        sample.value("id", int32_t(i));
        sample.value("value", i == 0 ? 1.5 : 21.5);
        writer.write(sample);
    }
    sub::Query query(reader, "value > %0", strs("20"));
    double seen = 0.0;
    uint32_t matching = 0;
    const uint64_t until = ::zerodds::cxx::monotonic_microseconds() + 5 * 1000000u;
    while (matching == 0 && ::zerodds::cxx::monotonic_microseconds() < until) {
        sub::LoanedSamples<DynamicData> got = reader.select().content(query).read();
        for (sub::LoanedSamples<DynamicData>::const_iterator smp = got.begin(); smp != got.end(); ++smp) {
            if (smp->info().valid()) {
                ++matching;
                seen = smp->data().value<double>("value");
            }
        }
        ::zerodds::cxx::sleep_milliseconds(20);
    }
    EXPECT(matching == 1 && seen == 21.5, "a query on a field of a DynamicData selects one sample");
    EXPECT(reader.select().read().length() == 2, "and the reader holds both");
}

// DDS-PSM-Cxx 1.0 (dds/core/BuiltinTopicTypes.hpp, DDS-XTypes 1.3 §7.6.8): the built-in topic types
// carry a value from a writer to a reader.
static void test_builtin_topic_types_round_trip() {
    domain::DomainParticipant dp(D(32));
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    pub::qos::DataWriterQos wq;
    wq << core::policy::Reliability::Reliable() << core::policy::History::KeepAll();
    sub::qos::DataReaderQos rq;
    rq << core::policy::Reliability::Reliable() << core::policy::History::KeepAll();

    topic::Topic<core::KeyedStringTopicType> keyed(dp, "BuiltinKeyedString");
    pub::DataWriter<core::KeyedStringTopicType> kw(pub_, keyed, wq);
    sub::DataReader<core::KeyedStringTopicType> kr(sub_, keyed, rq);
    kw->wait_for_matched(1, core::Duration::from_secs(5));
    kr->wait_for_matched(1, core::Duration::from_secs(5));
    kw.write(core::KeyedStringTopicType("k", "v"));

    topic::Topic<core::StringTopicType> text(dp, "BuiltinString");
    pub::DataWriter<core::StringTopicType> tw(pub_, text, wq);
    sub::DataReader<core::StringTopicType> tr(sub_, text, rq);
    tw->wait_for_matched(1, core::Duration::from_secs(5));
    tr->wait_for_matched(1, core::Duration::from_secs(5));
    tw.write(core::StringTopicType("hello"));

    topic::Topic<core::KeyedBytesTopicType> keyed_bytes(dp, "BuiltinKeyedBytes");
    pub::DataWriter<core::KeyedBytesTopicType> bw(pub_, keyed_bytes, wq);
    sub::DataReader<core::KeyedBytesTopicType> br(sub_, keyed_bytes, rq);
    bw->wait_for_matched(1, core::Duration::from_secs(5));
    br->wait_for_matched(1, core::Duration::from_secs(5));
    bw.write(core::KeyedBytesTopicType("b", bytes(1, 2, 3)));

    topic::Topic<core::BytesTopicType> bytes_topic(dp, "BuiltinBytes");
    pub::DataWriter<core::BytesTopicType> yw(pub_, bytes_topic, wq);
    sub::DataReader<core::BytesTopicType> yr(sub_, bytes_topic, rq);
    yw->wait_for_matched(1, core::Duration::from_secs(5));
    yr->wait_for_matched(1, core::Duration::from_secs(5));
    yw.write(core::BytesTopicType(bytes(9, 8)));

    const uint64_t until = ::zerodds::cxx::monotonic_microseconds() + 5 * 1000000u;
    struct Arrived {
        sub::DataReader<core::KeyedStringTopicType>& kr;
        sub::DataReader<core::StringTopicType>& tr;
        sub::DataReader<core::KeyedBytesTopicType>& br;
        sub::DataReader<core::BytesTopicType>& yr;
        Arrived(sub::DataReader<core::KeyedStringTopicType>& k, sub::DataReader<core::StringTopicType>& t,
                sub::DataReader<core::KeyedBytesTopicType>& b, sub::DataReader<core::BytesTopicType>& y)
            : kr(k), tr(t), br(b), yr(y) {}
        bool operator()() const {
            return kr.select().state(sub::status::DataState::any()).read().length() >= 1 &&
                   tr.select().state(sub::status::DataState::any()).read().length() >= 1 &&
                   br.select().state(sub::status::DataState::any()).read().length() >= 1 &&
                   yr.select().state(sub::status::DataState::any()).read().length() >= 1;
        }
    } arrived(kr, tr, br, yr);
    while (!arrived() && ::zerodds::cxx::monotonic_microseconds() < until) {
        ::zerodds::cxx::sleep_milliseconds(20);
    }
    sub::LoanedSamples<core::KeyedStringTopicType> ks = kr.select().state(sub::status::DataState::any()).read();
    sub::LoanedSamples<core::StringTopicType> ts = tr.select().state(sub::status::DataState::any()).read();
    sub::LoanedSamples<core::KeyedBytesTopicType> bs = br.select().state(sub::status::DataState::any()).read();
    sub::LoanedSamples<core::BytesTopicType> ys = yr.select().state(sub::status::DataState::any()).read();
    EXPECT(ks.length() == 1 && ks.begin()->data().key() == "k" && ks.begin()->data().value() == "v",
           "KeyedStringTopicType");
    EXPECT(ts.length() == 1 && ts.begin()->data().data() == "hello", "StringTopicType");
    EXPECT(bs.length() == 1 && bs.begin()->data().key() == "b" &&
               bs.begin()->data().value() == bytes(1, 2, 3),
           "KeyedBytesTopicType");
    EXPECT(ys.length() == 1 && ys.begin()->data().data() == bytes(9, 8), "BytesTopicType");
}

// DDS-PSM-Cxx 1.0 §2.1: the participant operations that no other test calls: lookup of a topic
// description, the built-in subscriber, discovered participants, delete_contained_entities.
static void test_participant_operations_without_other_coverage() {
    domain::DomainParticipant dp(D(33));
    topic::Topic<core::ByteSeq> t(dp, "LookupTopicDescription");
    EXPECT(t.inconsistent_topic_status().total_count() == 0, "a topic has no inconsistent topic");
    const topic::AnyTopic found = dp->lookup_topicdescription("LookupTopicDescription");
    EXPECT(found->name() == "LookupTopicDescription", "lookup_topicdescription finds a topic by its name");
    EXPECT(dp->lookup_topicdescription("NoSuchTopicDescription") == topic::AnyTopic(core::null),
           "and gives a null topic for a name nobody created");
    EXPECT(dp->find_topic("NoSuchTopic") == topic::AnyTopic(core::null), "find_topic gives a null topic for an unknown name");

    // DDS-XTypes 1.3 §7.6.4.1 and §7.6.4.2: several topics of one name for several types, iterated
    // by the index of lookup_topicdescription.
    topic::Topic<core::StringTopicType> second(dp, "LookupTopicDescription");
    EXPECT(dp->lookup_topicdescription("LookupTopicDescription", 0)->type_name() != "",
           "index 0 is the first topic of the name");
    const topic::AnyTopic first_of_name = dp->lookup_topicdescription("LookupTopicDescription", 0);
    const topic::AnyTopic second_of_name = dp->lookup_topicdescription("LookupTopicDescription", 1);
    EXPECT(first_of_name->type_name() == t->type_name(), "index 0 is the topic created first");
    EXPECT(second_of_name->type_name() == second->type_name() &&
               second_of_name->type_name() != first_of_name->type_name(),
           "index 1 is the topic of the second type");
    EXPECT(dp->lookup_topicdescription("LookupTopicDescription", 2) == topic::AnyTopic(core::null),
           "an index past the last topic of the name is a null topic");
    EXPECT(dp->lookup_topicdescription("NoSuchTopicDescription", 0) == topic::AnyTopic(core::null),
           "and so is any index of a name nobody created");
    // The topics of one name are one network topic: a second one with other policies is refused
    // and the first stays.
    topic::qos::TopicQos other_qos;
    other_qos << core::policy::TopicData(bytes(1, 2, 3));
    bool refused = false;
    try {
        topic::Topic<core::KeyedStringTopicType> third(dp, "LookupTopicDescription", other_qos);
    } catch (const core::Error&) {
        refused = true;
    }
    EXPECT(refused, "a topic of the name with other policies is refused");
    EXPECT(dp->lookup_topicdescription("LookupTopicDescription", 2) == topic::AnyTopic(core::null),
           "and is not one of the topics of the name");

    EXPECT(sub::builtin_subscriber(dp)->builtin_handle() != nullptr, "the built-in subscriber of a participant");
    (void)dp->get_discovered_participants();
    bool unknown_participant = false;
    try {
        (void)dp->get_discovered_participant_data(core::InstanceHandle(12345));
    } catch (const core::PreconditionNotMetError&) {
        unknown_participant = true;
    }
    EXPECT(unknown_participant, "the data of a participant nobody discovered is PreconditionNotMetError");

    pub::Publisher pub_(dp);
    pub::DataWriter<core::ByteSeq> dw(pub_, t);
    sub::Subscriber sub_(dp);
    sub::DataReader<core::ByteSeq> dr(sub_, t);
    const core::InstanceHandle writer_handle = dw.instance_handle();
    const core::InstanceHandle topic_handle = t.instance_handle();
    EXPECT(dp.contains_entity(writer_handle) && dp.contains_entity(topic_handle),
           "the participant contains its writer and its topic");
    dp->delete_contained_entities();
    EXPECT(!dp.contains_entity(writer_handle) && !dp.contains_entity(topic_handle),
           "and no longer once delete_contained_entities has run");
    // The entities the application still holds are closed: destroying them frees nothing twice.
}

// DDS-PSM-Cxx 1.0 §2.1, §7.6: the QoS of a running entity is read and set, with `qos()` and with the
// stream operators; a changeable policy changes, an immutable one is refused.
static void test_qos_of_a_running_entity_is_set_and_read() {
    namespace p = core::policy;
    domain::DomainParticipant dp(D(34));
    domain::qos::DomainParticipantQos dq;
    dq << p::UserData(bytes(7, 8));
    dp.qos(dq);
    EXPECT(dp.qos().policy<p::UserData>().value() == bytes(7, 8), "DomainParticipant::qos(qos)");

    topic::Topic<core::ByteSeq> t(dp, "QosChange");
    topic::qos::TopicQos tq;
    tq << p::TopicData(bytes(1));
    t.qos(tq);
    EXPECT(t.qos().policy<p::TopicData>().value() == bytes(1), "Topic::qos(qos)");

    pub::Publisher pub_(dp);
    pub::qos::PublisherQos pq;
    pq << p::Partition("first");
    pub_.qos(pq);
    EXPECT(pub_.qos().policy<p::Partition>().name()[0] == "first", "Publisher::qos(qos)");
    pub::qos::PublisherQos pq2;
    pq2 << p::Partition("second");
    pub_ << pq2;
    pub::qos::PublisherQos pq_out;
    pub_ >> pq_out;
    EXPECT(pq_out.policy<p::Partition>().name()[0] == "second", "Publisher << qos and > > qos");

    sub::Subscriber sub_(dp);
    sub::qos::SubscriberQos sq;
    sq << p::Partition("first");
    sub_.qos(sq);
    EXPECT(sub_.qos().policy<p::Partition>().name()[0] == "first", "Subscriber::qos(qos)");
    sub::qos::SubscriberQos sq2;
    sq2 << p::Partition("second");
    sub_ << sq2;
    sub::qos::SubscriberQos sq_out;
    sub_ >> sq_out;
    EXPECT(sq_out.policy<p::Partition>().name()[0] == "second", "Subscriber << qos and > > qos");

    pub::DataWriter<core::ByteSeq> dw(pub_, t);
    pub::qos::DataWriterQos wq = dw.qos();
    wq << p::Deadline(core::Duration::from_millisecs(300)) << p::TransportPriority(5);
    dw.qos(wq);
    EXPECT(dw.qos().policy<p::Deadline>().period() == core::Duration::from_millisecs(300) &&
               dw.qos().policy<p::TransportPriority>().value() == 5,
           "DataWriter::qos(qos) changes a changeable policy");
    pub::qos::DataWriterQos immutable = dw.qos();
    immutable << p::Durability::TransientLocal();
    bool refused = false;
    try {
        dw.qos(immutable);
    } catch (const core::ImmutablePolicyError&) {
        refused = true;
    }
    EXPECT(refused, "an immutable policy of a running writer is ImmutablePolicyError");
    pub::qos::DataWriterQos wq_out;
    dw >> wq_out;
    EXPECT(wq_out.policy<p::TransportPriority>().value() == 5, "DataWriter >> qos");
    dw << wq;

    sub::DataReader<core::ByteSeq> dr(sub_, t);
    sub::qos::DataReaderQos rq = dr.qos();
    rq << p::UserData(bytes(9));
    dr.qos(rq);
    EXPECT(dr.qos().policy<p::UserData>().value() == bytes(9), "DataReader::qos(qos)");
    sub::qos::DataReaderQos rq_out;
    dr >> rq_out;
    EXPECT(rq_out.policy<p::UserData>().value() == bytes(9), "DataReader >> qos");
    dr << rq;
}

// DDS-XTypes 1.3 Annex D: the QoS sets of Topic, DataWriter and DataReader carry the
// DataRepresentationQosPolicy and the DataReader the TypeConsistencyEnforcementQosPolicy with its
// five flags; both reach the entity and come back from it.
static void test_data_representation_and_type_consistency_in_the_qos_sets() {
    namespace p = core::policy;
    domain::DomainParticipant dp(D(61));

    // Not given: the policies are not set and the flags have the defaults of §7.6.3.4.1.
    EXPECT(!topic::qos::TopicQos().policy<p::DataRepresentation>().is_set() &&
               !pub::qos::DataWriterQos().policy<p::DataRepresentation>().is_set() &&
               !sub::qos::DataReaderQos().policy<p::DataRepresentation>().is_set(),
           "a QoS set has the DataRepresentation policy, not given");
    const p::TypeConsistencyEnforcement defaults =
        sub::qos::DataReaderQos().policy<p::TypeConsistencyEnforcement>();
    EXPECT(!defaults.is_set() && defaults.ignore_sequence_bounds() && defaults.ignore_string_bounds() &&
               !defaults.ignore_member_names() && !defaults.prevent_type_widening() &&
               !defaults.force_type_validation() &&
               defaults.kind() == p::TypeConsistencyEnforcementKind::ASSIGNABLE_TYPE_CONSISTENCY,
           "the DataReaderQos has the TypeConsistencyEnforcement policy with the defaults of 7.6.3.4.1");

    // A topic that names a representation hands it back.
    topic::qos::TopicQos tq;
    tq << p::DataRepresentation(ids(2));
    topic::Topic<core::ByteSeq> t(dp, "RepresentationInQos", tq);
    const p::DataRepresentation of_topic = t.qos().policy<p::DataRepresentation>();
    EXPECT(of_topic.is_set() && of_topic.value() == ids(2),
           "the DataRepresentation of a topic comes back from it");

    // A writer and a reader that name one.
    pub::Publisher pub_(dp);
    sub::Subscriber sub_(dp);
    pub::qos::DataWriterQos wq;
    wq << p::DataRepresentation(ids(2));
    pub::DataWriter<core::ByteSeq> dw(pub_, t, wq);
    EXPECT(dw.qos().policy<p::DataRepresentation>().value() == ids(2),
           "the DataRepresentation of a writer comes back from it");
    pub::DataWriter<core::ByteSeq> plain_writer(pub_, t);
    EXPECT(!plain_writer.qos().policy<p::DataRepresentation>().is_set(),
           "a writer created without the policy does not have it set");

    sub::qos::DataReaderQos rq;
    rq << p::DataRepresentation(ids(0, 2));
    p::TypeConsistencyEnforcement wanted(p::TypeConsistencyEnforcementKind::ASSIGNABLE_TYPE_CONSISTENCY);
    wanted.ignore_sequence_bounds(false)
        .ignore_string_bounds(false)
        .ignore_member_names(true)
        .prevent_type_widening(true)
        .force_type_validation(true);
    rq << wanted;
    sub::DataReader<core::ByteSeq> dr(sub_, t, rq);
    EXPECT(dr.qos().policy<p::DataRepresentation>().value() == ids(0, 2),
           "the DataRepresentation list of a reader comes back from it, in order");
    const p::TypeConsistencyEnforcement got = dr.qos().policy<p::TypeConsistencyEnforcement>();
    EXPECT(got.kind() == p::TypeConsistencyEnforcementKind::ASSIGNABLE_TYPE_CONSISTENCY &&
               !got.ignore_sequence_bounds() && !got.ignore_string_bounds() && got.ignore_member_names() &&
               got.prevent_type_widening() && got.force_type_validation(),
           "the five flags of the TypeConsistencyEnforcement of a reader come back from it");

    // The kind that forbids coercion reads back as a kind that forbids it.
    sub::qos::DataReaderQos strict;
    strict << p::TypeConsistencyEnforcement(p::TypeConsistencyEnforcementKind::DECLARED_TYPE_CONSISTENCY);
    sub::DataReader<core::ByteSeq> strict_reader(sub_, t, strict);
    EXPECT(strict_reader.qos().policy<p::TypeConsistencyEnforcement>().kind() !=
               p::TypeConsistencyEnforcementKind::ASSIGNABLE_TYPE_CONSISTENCY,
           "DISALLOW_TYPE_COERCION is not the assignable kind when read back");

    // The list is not changeable once the writer is enabled, and a list that does not fit the
    // C struct is refused rather than cut.
    pub::qos::DataWriterQos changed = dw.qos();
    changed << p::DataRepresentation(ids(0));
    bool immutable = false;
    try {
        dw.qos(changed);
    } catch (const core::ImmutablePolicyError&) {
        immutable = true;
    }
    EXPECT(immutable, "the DataRepresentation of an enabled writer is not changeable");
    pub::qos::DataWriterQos too_long;
    too_long << p::DataRepresentation(ids(0, 1, 2, 0, 1, 2, 0, 1, 2));
    bool refused = false;
    try {
        pub::DataWriter<core::ByteSeq> never(pub_, t, too_long);
    } catch (const core::InvalidArgumentError&) {
        refused = true;
    }
    EXPECT(refused, "a DataRepresentation list of more than 8 identifiers is refused");

    // The QoS sets convert to one another by the policies they share.
    pub::qos::DataWriterQos from_topic(tq);
    EXPECT(from_topic.policy<p::DataRepresentation>().value() == ids(2),
           "a DataWriterQos built from a TopicQos takes its DataRepresentation");
}

// DDS-PSM-Cxx 1.0 §2.1: every entity can be created with its QoS, a listener and a status mask.
static void test_entities_are_created_with_qos_listener_and_mask() {
    namespace p = core::policy;
    struct ParticipantListener : domain::NoOpDomainParticipantListener {};
    struct TopicProbeListener : topic::NoOpTopicListener<core::ByteSeq> {};
    struct WriterProbeListener : pub::NoOpDataWriterListener<core::ByteSeq> {};
    struct ReaderProbeListener : sub::NoOpDataReaderListener<core::ByteSeq> {};

    domain::qos::DomainParticipantQos dq;
    dq << p::UserData(bytes(1, 2));
    ParticipantListener participant_listener;
    domain::DomainParticipant dp(D(35), dq, &participant_listener, core::status::StatusMask::all());
    EXPECT(dp.qos().policy<p::UserData>().value() == bytes(1, 2), "a participant made with QoS");
    EXPECT(dp.listener() == &participant_listener, "and with a listener");

    topic::qos::TopicQos tq;
    tq << p::TopicData(bytes(3));
    TopicProbeListener topic_listener;
    topic::Topic<core::ByteSeq> t(dp, "CreatedWithListener", tq, &topic_listener,
                                  core::status::StatusMask::all());
    EXPECT(t.qos().policy<p::TopicData>().value() == bytes(3) && t.listener() == &topic_listener,
           "a topic made with QoS and a listener");

    PubListener publisher_listener;
    pub::qos::PublisherQos pq;
    pq << p::Partition("created");
    pub::Publisher pub_(dp, pq, &publisher_listener, core::status::StatusMask::all());
    EXPECT(pub_.listener() == &publisher_listener && pub_.qos().policy<p::Partition>().name()[0] == "created",
           "a publisher made with QoS and a listener");

    SubListener subscriber_listener;
    sub::qos::SubscriberQos sq;
    sq << p::Partition("created");
    sub::Subscriber sub_(dp, sq, &subscriber_listener, core::status::StatusMask::all());
    EXPECT(sub_.listener() == &subscriber_listener && sub_.qos().policy<p::Partition>().name()[0] == "created",
           "a subscriber made with QoS and a listener");

    WriterProbeListener writer_listener;
    pub::qos::DataWriterQos wq;
    wq << p::TransportPriority(3);
    pub::DataWriter<core::ByteSeq> dw(pub_, t, wq, &writer_listener, core::status::StatusMask::all());
    EXPECT(dw.listener() == &writer_listener && dw.qos().policy<p::TransportPriority>().value() == 3,
           "a writer made with QoS and a listener");

    ReaderProbeListener reader_listener;
    sub::qos::DataReaderQos rq;
    rq << p::UserData(bytes(4));
    sub::DataReader<core::ByteSeq> dr(sub_, t, rq, &reader_listener, core::status::StatusMask::all());
    EXPECT(dr.listener() == &reader_listener && dr.qos().policy<p::UserData>().value() == bytes(4),
           "a reader made with QoS and a listener");

    // The listeners go before the entities they listen to.
    dr.listener(nullptr, core::status::StatusMask::none());
    dw.listener(nullptr, core::status::StatusMask::none());
    sub_.listener(nullptr, core::status::StatusMask::none());
    pub_.listener(nullptr, core::status::StatusMask::none());
    t.listener(nullptr, core::status::StatusMask::none());
    dp.listener(nullptr, core::status::StatusMask::none());
}
// ---- end of the generated block ----

int main(int argc, char** argv) {
#define RUN(fn) do { std::cerr << "[smoke] " #fn " start\n" << std::flush; fn(); std::cerr << "[smoke] " #fn " done\n" << std::flush; } while (0)
#if !ZERODDS_CXX11
    // The modes with threads need std::thread and std::atomic (DDS-PSM-Cxx 1.0 §7.12, C++11).
    if (argc > 1 && (std::string(argv[1]) == "threads" || std::string(argv[1]) == "threads-writers" ||
                     std::string(argv[1]) == "threads-conditions" || std::string(argv[1]) == "threads-entities")) {
        std::cout << "SKIP — the mode " << argv[1] << " needs C++11 (std::thread, std::atomic).\n";
        return 0;
    }
#else
    if (argc > 1 && std::string(argv[1]) == "threads-conditions") {
        for (int round = 0; round < 25; ++round) {
            test_conditions_are_used_from_several_threads();
        }
        if (failures == 0) {
            std::cout << "OK\n";
        }
        return failures;
    }
    if (argc > 1 && std::string(argv[1]) == "threads-entities") {
        RUN(test_entities_are_created_from_several_threads);
        if (failures == 0) {
            std::cout << "OK\n";
        }
        return failures;
    }
    if (argc > 1 && std::string(argv[1]) == "threads-writers") {
        RUN(test_operations_are_called_from_several_threads);
        if (failures == 0) {
            std::cout << "OK\n";
        }
        return failures;
    }
    if (argc > 1 && std::string(argv[1]) == "threads") {
        // Run under ThreadSanitizer by `dds_psm_cxx_smoke_under_thread_sanitizer`: the tests in
        // which threads call the operations of the entities at once.
        RUN(test_operations_are_called_from_several_threads);
        RUN(test_conditions_are_used_from_several_threads);
    RUN(test_entities_are_created_from_several_threads);
    RUN(test_signatures_follow_the_normative_headers);
        if (failures == 0) {
            std::cout << "OK — the threaded smoke tests passed.\n";
        }
        return failures;
    }
#endif  // ZERODDS_CXX11
    RUN(test_default_qos_matches_the_specification);
    RUN(test_qos_composition);
    RUN(test_duration_sentinel_conversion_and_overflow);
    RUN(test_participant_lifecycle);
    RUN(test_topic_creation);
    RUN(test_publisher_subscriber_lifecycle);
    RUN(test_writer_reader_lifecycle);
    RUN(test_take_on_empty_returns_empty);
    RUN(test_read_keeps_samples_take_removes_them);
    RUN(test_status_getters);
    RUN(test_guardcondition_and_waitset);
    RUN(test_readcondition_lifecycle);
    RUN(test_contentfilteredtopic_lifecycle);
    RUN(test_listener_attach_detach);
    RUN(test_listener_bridge_dispatches_to_virtual);
    RUN(test_listener_table_is_per_entity_and_deregisters_on_destroy);
    RUN(test_matched_data_names_the_other_endpoint);
    RUN(test_topic_data_carries_every_member);
    RUN(test_discovered_topics_empty_initially);
    RUN(test_discovered_topic_data_unknown_handle_throws);
    RUN(test_layout_headers_and_find);
    RUN(test_domain_participant_listener);
    RUN(test_find_ignore_status_condition);
    RUN(test_discovered_publications_count_zero);
    RUN(test_discovered_subscriptions_count_zero);
    RUN(test_take_returns_the_loan_when_decode_throws);
    RUN(test_entity_operations);
    RUN(test_samples_api);
    RUN(test_topic_any_topic_and_listener);
    RUN(test_publisher_subscriber_datawriter_api);
    RUN(test_qos_provider);
    RUN(test_value_and_reference_classes_without_other_coverage);
    RUN(test_entities_and_statuses_without_other_coverage);
    RUN(test_every_listener_callback_reaches_its_virtual_method);
    RUN(test_sample_info_parts_qos_provider_kinds_and_filter_expression);
    RUN(test_dynamic_types_dynamic_data_and_type_provider);
    RUN(test_bit_masks_union_defaults_and_enum_discriminators);
    RUN(test_builtin_topic_types_round_trip);
    RUN(test_participant_operations_without_other_coverage);
    RUN(test_qos_of_a_running_entity_is_set_and_read);
    RUN(test_data_representation_and_type_consistency_in_the_qos_sets);
    RUN(test_entities_are_created_with_qos_listener_and_mask);
    RUN(test_instance_operations_with_a_time_stamp);
#if ZERODDS_CXX11
    RUN(test_operations_are_called_from_several_threads);
    RUN(test_conditions_are_used_from_several_threads);
    RUN(test_entities_are_created_from_several_threads);
#endif
    RUN(test_return_codes_map_to_exceptions);
    RUN(test_signatures_follow_the_normative_headers);
    RUN(test_objects_in_use_stay_open);
    RUN(test_reference_types);
    RUN(test_reference_type_macros);
    RUN(test_every_policy_has_its_default_and_its_attributes);
    RUN(test_ignore_ends_a_match);
    RUN(test_builtin_topic_readers);
    RUN(test_builtin_topic_values_arrive);
#undef RUN

    if (failures == 0) {
        std::cout << "OK — all DDS-PSM-Cxx smoke tests passed.\n";
    } else {
        std::cerr << "FAIL — " << failures << " smoke tests failed.\n";
    }
    return failures;
}
