// SPDX-License-Identifier: Apache-2.0
//
// What every roundtrip app shares, whatever its DDS stack: the clock, the
// environment it reads, the RTT record of a listener-driven ping side, the
// quantile line matrix_report.py parses, the ping command line, and the
// throughput modes of the speed setup (tests/perf/speed): the QoS a run
// measures under, the process CPU clock, the writing loop and the reader's
// count. The calls into a stack stay in the apps: each drives its own API.
#pragma once

#include <algorithm>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <ctime>
#include <iostream>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

namespace bench {

inline uint64_t now_ns() {
    auto t = std::chrono::steady_clock::now().time_since_epoch();
    return std::chrono::duration_cast<std::chrono::nanoseconds>(t).count();
}

// CPU time of the whole process, every thread of the stack included
// (CLOCK_PROCESS_CPUTIME_ID). The speed setup divides its difference over a
// measurement window by the samples of that window: CPU per sample.
inline uint64_t cpu_ns() {
    timespec ts{};
    clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &ts);
    return static_cast<uint64_t>(ts.tv_sec) * 1000000000ull + static_cast<uint64_t>(ts.tv_nsec);
}

// The QoS both endpoints of a run use. Unset, the variables give the QoS
// the roundtrip matrix has always used: RELIABLE, KEEP_LAST(64) and the
// stack's own RESOURCE_LIMITS.
//   ZERODDS_BENCH_RELIABILITY = reliable | besteffort
//   ZERODDS_BENCH_HISTORY     = all | <depth>
//   ZERODDS_BENCH_MAX_SAMPLES = <n>  RESOURCE_LIMITS max_samples and
//                                    max_samples_per_instance (the topic is
//                                    one keyless instance); unset: the
//                                    stack's default.
struct QosSpec {
    bool    reliable    = true;
    bool    keep_all    = false;
    int32_t depth       = 64;
    int32_t max_samples = -1;  // -1: leave the stack's default

    bool has_limits() const { return max_samples > 0; }
};

inline QosSpec qos_from_env() {
    QosSpec q;
    if (const char* r = std::getenv("ZERODDS_BENCH_RELIABILITY")) {
        std::string v(r);
        if (v == "besteffort" || v == "best_effort" || v == "be") q.reliable = false;
        else if (v == "reliable") q.reliable = true;
        else { std::cerr << "ZERODDS_BENCH_RELIABILITY=" << v << " unknown\n"; std::exit(2); }
    }
    if (const char* h = std::getenv("ZERODDS_BENCH_HISTORY")) {
        std::string v(h);
        if (v == "all") {
            q.keep_all = true;
        } else {
            try { q.depth = static_cast<int32_t>(std::stol(v)); } catch (...) { q.depth = 0; }
            if (q.depth < 1) { std::cerr << "ZERODDS_BENCH_HISTORY=" << v << " unknown\n"; std::exit(2); }
        }
    }
    if (const char* m = std::getenv("ZERODDS_BENCH_MAX_SAMPLES")) {
        try { q.max_samples = static_cast<int32_t>(std::stol(m)); } catch (...) { q.max_samples = -1; }
    }
    return q;
}

// One line naming the QoS, so no number in a log stands without it.
inline void print_qos(const char* role, const QosSpec& q) {
    std::cout << "qos[" << role << "]: reliability=" << (q.reliable ? "reliable" : "besteffort")
              << " history=" << (q.keep_all ? std::string("keep_all")
                                            : "keep_last(" + std::to_string(q.depth) + ")")
              << " max_samples=" << q.max_samples << "\n" << std::flush;
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

// `--max-seconds S` caps a ping run's wall time (0, the default: no cap).
// The ping waits 50 ms for each echo it does not get; a stack that loses
// echoes would take minutes for 10 000 of them. The loop stops sending at
// the cap and `ping-sent:` names how many it sent.
inline uint64_t& ping_max_seconds() {
    static uint64_t v = 0;
    return v;
}

inline uint64_t ping_deadline() {
    return ping_max_seconds() ? now_ns() + ping_max_seconds() * 1000000000ull : 0;
}

inline bool ping_past(uint64_t deadline) {
    return deadline != 0 && now_ns() >= deadline;
}

inline void print_ping_sent(uint64_t sent) {
    std::cout << "ping-sent: " << sent << "\n" << std::flush;
}

inline PingArgs parse_ping_args(int argc, char** argv, int first) {
    PingArgs a;
    for (int i = first; i + 1 < argc; i += 2) {
        std::string flag = argv[i];
        if (flag.rfind("--", 0) != 0) continue;
        uint64_t   v    = std::stoull(argv[i+1]);
        if      (flag == "--payload") a.payload = static_cast<size_t>(v);
        else if (flag == "--samples") a.samples = v;
        else if (flag == "--warmup")  a.warmup  = v;
        else if (flag == "--max-seconds") ping_max_seconds() = v;
    }
    return a;
}

// ---------------------------------------------------------------------
// Throughput: `tput-pub --payload N --seconds S` writes one sample over and
// over for S seconds, its sequence_id counted up per accepted write, as
// ddsperf writes; `tput-sub --seconds S` takes in its listener and counts.
// Both print one `tput-pub:` / `tput-sub:` line that tests/perf/speed reads.
// ---------------------------------------------------------------------

struct TputArgs {
    size_t   payload = 1024;
    uint64_t seconds = 5;
};

inline TputArgs parse_tput_args(int argc, char** argv, int first) {
    TputArgs a;
    for (int i = first; i + 1 < argc; i += 2) {
        std::string flag = argv[i];
        if (flag.rfind("--", 0) != 0) continue;
        uint64_t v = std::stoull(argv[i + 1]);
        if      (flag == "--payload") a.payload = static_cast<size_t>(v);
        else if (flag == "--seconds") a.seconds = v;
    }
    return a;
}

// Waits until `matched()` is true or 10 s pass, then 500 ms more: a writer
// that has matched its reader does not know whether the reader has matched
// it yet, and under BEST_EFFORT what is written before is lost to it.
template <class Matched>
inline bool tput_wait_matched(Matched&& matched) {
    auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(10);
    bool ok = false;
    while (std::chrono::steady_clock::now() < deadline) {
        if (matched()) { ok = true; break; }
        std::this_thread::sleep_for(std::chrono::milliseconds(10));
    }
    if (!ok) std::cerr << "tput-pub: no matched reader within 10 s\n";
    std::this_thread::sleep_for(std::chrono::milliseconds(500));
    return ok;
}

struct TputPubResult {
    uint64_t written   = 0;  // writes the stack accepted
    uint64_t failed    = 0;  // writes it refused (e.g. TIMEOUT under KEEP_ALL)
    uint64_t window_ns = 0;
    uint64_t cpu_ns    = 0;
};

// The writing loop. `write(seq)` writes the sample with that sequence_id and
// returns whether the stack accepted it; a refused write does not use up a
// sequence number, so the reader's gaps count lost samples only. The clock
// is read every 16 writes and after every refused one (a refused write has
// waited max_blocking_time already).
template <class Write>
inline TputPubResult tput_pub_run(uint64_t seconds, Write&& write) {
    TputPubResult r;
    uint32_t seq = 0;
    const uint64_t cpu0 = cpu_ns();
    const uint64_t t0 = now_ns();
    const uint64_t deadline = t0 + seconds * 1000000000ull;
    uint64_t t1 = t0;
    for (uint64_t i = 1;; ++i) {
        bool ok = write(seq);
        if (ok) { ++r.written; ++seq; } else { ++r.failed; }
        if (!ok || (i & 15) == 0) {
            t1 = now_ns();
            if (t1 >= deadline) break;
        }
    }
    r.cpu_ns = cpu_ns() - cpu0;
    r.window_ns = t1 - t0;
    return r;
}

inline void print_tput_pub(const TputPubResult& r, size_t payload, int acked, uint64_t ack_ns) {
    std::printf("tput-pub: payload=%zu written=%llu failed=%llu window_ns=%llu cpu_ns=%llu "
                "acked=%d ack_ns=%llu\n",
                payload, (unsigned long long)r.written, (unsigned long long)r.failed,
                (unsigned long long)r.window_ns, (unsigned long long)r.cpu_ns, acked,
                (unsigned long long)ack_ns);
    std::fflush(stdout);
}

// The writer stays 2 s after its result before it deletes its entities. A
// stack may drop what a deleted writer sent and its reader has not taken
// yet: OpenSplice's reader missed the last 921 of 1 056 863 RELIABLE
// samples after wait_for_acknowledgments had returned (the speed setup's
// first runs). The reader stops 2 s after its last sample.
inline void tput_linger() {
    std::this_thread::sleep_for(std::chrono::seconds(2));
}

// What the reader's listener saw. `on_sample` is called for every valid
// sample, from whatever thread the stack runs its listener in.
struct TputSub {
    std::mutex            mu;
    std::atomic<uint64_t> received{0};
    uint64_t              bytes       = 0;
    uint64_t              gaps        = 0;  // sequence numbers never seen
    uint64_t              reordered   = 0;  // a sequence number below the next expected
    uint32_t              first_seq   = 0;
    uint32_t              last_seq    = 0;
    uint64_t              t_first_ns  = 0;
    uint64_t              t_last_ns   = 0;
    bool                  any         = false;

    void on_sample(uint32_t seq, size_t len) {
        uint64_t now = now_ns();
        std::lock_guard<std::mutex> lk(mu);
        if (!any) {
            any = true;
            first_seq = seq;
            last_seq = seq;
            t_first_ns = now;
        } else {
            uint32_t expected = last_seq + 1;
            if (seq < expected) {
                ++reordered;
            } else {
                gaps += seq - expected;
                last_seq = seq;
            }
        }
        t_last_ns = now;
        bytes += len;
        received.fetch_add(1, std::memory_order_relaxed);
    }
};

// The reader's main thread: polls every 20 ms until samples have stopped
// for `idle_ms` or `max_s` passed, then prints. The process CPU is read at
// the first poll that saw samples and at the last poll that saw new ones;
// `cpu_samples` are the samples taken between those two polls, so the time
// before the stream and after it is not charged to a sample.
inline void tput_sub_wait_and_report(TputSub& st, uint64_t max_s, uint64_t idle_ms = 2000) {
    const uint64_t t0 = now_ns();
    const uint64_t limit = t0 + max_s * 1000000000ull;
    uint64_t seen = 0, cpu_a = 0, cnt_a = 0, cpu_b = 0, cnt_b = 0, last_change = t0;
    bool started = false;
    for (;;) {
        std::this_thread::sleep_for(std::chrono::milliseconds(20));
        uint64_t now = now_ns();
        uint64_t c = st.received.load(std::memory_order_relaxed);
        if (c != seen) {
            uint64_t cpu = cpu_ns();
            if (!started) { started = true; cpu_a = cpu; cnt_a = c; }
            cpu_b = cpu; cnt_b = c; seen = c; last_change = now;
        } else if (started && now - last_change > idle_ms * 1000000ull) {
            break;
        }
        if (now >= limit) break;
    }
    std::lock_guard<std::mutex> lk(st.mu);
    std::printf("tput-sub: received=%llu bytes=%llu first_seq=%u last_seq=%u gaps=%llu "
                "reordered=%llu rx_window_ns=%llu cpu_ns=%llu cpu_samples=%llu\n",
                (unsigned long long)st.received.load(), (unsigned long long)st.bytes,
                st.first_seq, st.last_seq, (unsigned long long)st.gaps,
                (unsigned long long)st.reordered,
                (unsigned long long)(st.t_last_ns - st.t_first_ns),
                (unsigned long long)(cpu_b - cpu_a), (unsigned long long)(cnt_b - cnt_a));
    std::fflush(stdout);
}

}  // namespace bench
