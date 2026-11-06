// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// test_support.hpp — what the C++ test programs CTest runs share: the DDS
// domain a program takes, and waiting on a reader's samples through a WaitSet
// instead of sleeping.
//
// The domain: the first argument, else the first domain of
// `ZERODDS_TEST_DOMAINS` (the leased domains tests/integration/with_domains.py
// hands to `ctest`). A program without either refuses to run: domain 0 would
// be heard by every other DDS process of the host.

#ifndef ZERODDS_CPP_TESTS_TEST_SUPPORT_HPP
#define ZERODDS_CPP_TESTS_TEST_SUPPORT_HPP

#include "dds/dds.hpp"

#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <string>

inline uint32_t test_domain(int argc, char** argv) {
    if (argc > 1) {
        return static_cast<uint32_t>(std::strtoul(argv[1], nullptr, 10));
    }
    const char* list = std::getenv("ZERODDS_TEST_DOMAINS");
    if (list == nullptr || *list == '\0') {
        std::fprintf(stderr, "FAIL: no domain: give one as the first argument or in ZERODDS_TEST_DOMAINS\n");
        std::exit(2);
    }
    return static_cast<uint32_t>(std::strtoul(std::string(list).substr(0, std::string(list).find(',')).c_str(),
                                              nullptr, 10));
}

/// Waits until `reader` holds a sample in `state` (DDS 1.4 §2.2.2.5.8 ReadCondition, attached to a
/// WaitSet, §2.2.2.1.6) or `deadline` has passed; true when the condition triggered.
template <typename T>
bool wait_for_samples(const dds::sub::DataReader<T>& reader, const dds::sub::status::SampleState& state,
                      std::chrono::steady_clock::time_point deadline) {
    const auto now = std::chrono::steady_clock::now();
    if (now >= deadline) {
        return false;
    }
    const auto left = std::chrono::duration_cast<std::chrono::microseconds>(deadline - now).count();
    dds::sub::cond::ReadCondition condition(
        reader, dds::sub::status::DataState(state, dds::sub::status::ViewState::any(),
                                            dds::sub::status::InstanceState::any()));
    dds::core::cond::WaitSet waitset;
    waitset += condition;
    try {
        waitset.wait(dds::core::Duration(static_cast<int32_t>(left / 1000000),
                                         static_cast<uint32_t>((left % 1000000) * 1000)));
        return true;
    } catch (const dds::core::TimeoutError&) {
        return false;
    }
}

/// A sample in any state: what a `take()` loop waits for.
template <typename T>
bool wait_for_samples(const dds::sub::DataReader<T>& reader, std::chrono::steady_clock::time_point deadline) {
    return wait_for_samples(reader, dds::sub::status::SampleState::any(), deadline);
}

#endif // ZERODDS_CPP_TESTS_TEST_SUPPORT_HPP
