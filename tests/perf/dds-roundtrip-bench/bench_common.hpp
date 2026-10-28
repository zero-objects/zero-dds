// SPDX-License-Identifier: Apache-2.0
//
// What every roundtrip app shares, whatever its DDS stack: the clock, the
// environment it reads, the RTT record of a listener-driven ping side, the
// quantile line matrix_report.py parses, and the ping command line. The
// measurement loops stay in the apps: each drives its stack's own API.
#pragma once

#include <algorithm>
#include <chrono>
#include <condition_variable>
#include <cstddef>
#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <mutex>
#include <string>
#include <vector>

namespace bench {

inline uint64_t now_ns() {
    auto t = std::chrono::steady_clock::now().time_since_epoch();
    return std::chrono::duration_cast<std::chrono::nanoseconds>(t).count();
}

// The domain: ZERODDS_BENCH_DOMAIN, 200 by default.
inline long env_domain() {
    const char* s = std::getenv("ZERODDS_BENCH_DOMAIN");
    if (s) { try { return std::stol(s); } catch (...) {} }
    return 200;
}

// XCDR1 when matrix_sweep.sh sets ZERODDS_BENCH_REPR=XCDR1 — for the pairs
// with OpenSplice, which writes and reads XCDR1 only; XCDR2 otherwise, as
// for every pair since the matrix of 23.05.
inline bool xcdr1_requested() {
    const char* s = std::getenv("ZERODDS_BENCH_REPR");
    return s && std::string(s) == "XCDR1";
}

// The ping side of an app whose reader calls back: the echo's RTT is
// recorded once the warm-up has passed, and the sending loop is woken.
struct PingState {
    std::mutex                  mu;
    std::condition_variable     cv;
    uint64_t                    received    = 0;
    uint64_t                    warmup      = 0;
    std::vector<uint64_t>       rtts;

    void record(uint64_t t_send) {
        uint64_t now = now_ns();
        uint64_t rtt = now > t_send ? now - t_send : 1;
        {
            std::lock_guard<std::mutex> lk(mu);
            if (received >= warmup) rtts.push_back(rtt);
            received++;
        }
        cv.notify_all();
    }
};

inline void print_quantiles(std::vector<uint64_t>& rtts, size_t payload_size) {
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

// `ping --payload N --samples N --warmup N`, from argv[first] on. A pair
// whose flag does not start with `--` is passed over: OpenDDS leaves its
// own `-DCPS…` options in argv.
struct PingArgs {
    size_t   payload = 64;
    uint64_t warmup  = 200;
    uint64_t samples = 5000;
};

inline PingArgs parse_ping_args(int argc, char** argv, int first) {
    PingArgs a;
    for (int i = first; i + 1 < argc; i += 2) {
        std::string flag = argv[i];
        if (flag.rfind("--", 0) != 0) continue;
        uint64_t   v    = std::stoull(argv[i+1]);
        if      (flag == "--payload") a.payload = static_cast<size_t>(v);
        else if (flag == "--samples") a.samples = v;
        else if (flag == "--warmup")  a.warmup  = v;
    }
    return a;
}

}  // namespace bench
