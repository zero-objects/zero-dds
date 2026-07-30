// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// AsyncWriter regression tests for the reliable-stream fixes:
//   1. window overflow: enqueue > SENDER_WINDOW samples before any ACKNACK, then
//      let ACKNACKs flow — every sample must arrive (no loss, no hang).
//   2. head-of-line: an oversized (> MAX_PAYLOAD) sample at the queue head must
//      be skipped, not block the samples behind it.
//   3. bounded teardown: with no ACKNACK responder the destructor must still
//      return within the drain deadline instead of waiting forever on the window.

#include "zerodds_reliable.hpp"

#include <chrono>
#include <cstdio>
#include <cstring>
#include <mutex>
#include <thread>
#include <vector>

using namespace zerodds::reliable;

static int g_fail = 0;
#define CHECK(cond)                                                             \
    do {                                                                        \
        if (!(cond)) {                                                          \
            std::fprintf(stderr, "FAIL %s:%d  %s\n", __FILE__, __LINE__, #cond); \
            ++g_fail;                                                           \
        }                                                                       \
    } while (0)

// In-process lossless loopback peer. All hooks run on the drain thread; the main
// thread reads counters + flips the gate, so shared state is mutex-guarded.
struct LoopPeer {
    std::mutex m;
    Receiver rcv;
    std::vector<std::uint8_t> delivered;
    std::vector<Bytes> acks;
    std::size_t write_data_seen = 0;
    bool gate;

    explicit LoopPeer(bool open) : gate(open) {}

    void on_frame(const Bytes& f) {
        auto u = unframe(f.data(), f.size());
        if (!u) return;  // ignore HEARTBEAT / non-WRITE_DATA
        std::uint16_t seq = u->first;
        std::size_t off = u->second.first, blen = u->second.second;
        std::lock_guard<std::mutex> lk(m);
        if (seq_lt(seq, rcv.expected())) return;  // retransmit of delivered → ignore
        Bytes body(f.begin() + static_cast<long>(off),
                   f.begin() + static_cast<long>(off + blen));
        rcv.recv_data(seq, body);
        for (auto& kv : rcv.drain_in_order()) {
            delivered.push_back(kv.second.empty() ? 0 : kv.second[0]);
        }
        AckNack a = rcv.pending_acknack(std::optional<std::uint16_t>(seq));
        std::uint16_t bm = a.bitmap;
        acks.push_back(acknack_frame(0x80, STREAM_RELIABLE, 0, a.first_unacked,
                                     static_cast<std::uint8_t>(bm & 0xFF),
                                     static_cast<std::uint8_t>((bm >> 8) & 0xFF), 0x80));
        ++write_data_seen;
    }

    int poll(std::uint8_t* buf, std::size_t cap) {
        std::lock_guard<std::mutex> lk(m);
        if (!gate || acks.empty()) return -1;
        Bytes f = acks.front();
        acks.erase(acks.begin());
        if (f.size() > cap) return -1;
        std::memcpy(buf, f.data(), f.size());
        return static_cast<int>(f.size());
    }

    std::size_t delivered_count() {
        std::lock_guard<std::mutex> lk(m);
        return delivered.size();
    }
    std::size_t seen() {
        std::lock_guard<std::mutex> lk(m);
        return write_data_seen;
    }
    void open_gate() {
        std::lock_guard<std::mutex> lk(m);
        gate = true;
    }
};

// 1) window overflow — fill the 16-sample window with the gate closed, then open
//    it and require every one of the 24 samples to be delivered.
static void test_window_overflow_no_loss() {
    const int n = 24;  // > SENDER_WINDOW (16)
    LoopPeer peer(false);
    std::size_t drops = 0;
    {
        AsyncWriter w(
            [&](const std::vector<Bytes>& b) { for (auto& f : b) peer.on_frame(f); },
            [&](const Bytes& f) { peer.on_frame(f); },
            [&](std::uint8_t* buf, std::size_t cap) { return peer.poll(buf, cap); },
            512);

        for (int i = 0; i < n; ++i) {
            std::uint8_t s = static_cast<std::uint8_t>(i);
            CHECK(w.enqueue(&s, 1));
        }
        // Wait until the window is full (16 seen) while the gate is closed, then
        // give the drain thread a moment before releasing ACKNACKs.
        for (int k = 0; k < 5000 && peer.seen() < SENDER_WINDOW; ++k) {
            std::this_thread::sleep_for(std::chrono::milliseconds(1));
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(50));
        peer.open_gate();
        for (int k = 0; k < 10000 && peer.delivered_count() < static_cast<std::size_t>(n); ++k) {
            std::this_thread::sleep_for(std::chrono::milliseconds(1));
        }
        drops = w.dropped();
    }  // destructor: finish() + bounded join

    std::lock_guard<std::mutex> lk(peer.m);
    CHECK(peer.delivered.size() == static_cast<std::size_t>(n));
    bool all = peer.delivered.size() == static_cast<std::size_t>(n);
    for (int i = 0; all && i < n; ++i) {
        all = peer.delivered[static_cast<std::size_t>(i)] == static_cast<std::uint8_t>(i);
    }
    CHECK(all);
    CHECK(drops == 0);
}

// 2) head-of-line — an oversized sample at the head must be skipped, not block
//    the samples behind it, and it must count as dropped.
static void test_oversized_no_head_of_line_block() {
    LoopPeer peer(true);  // acks flow immediately
    std::size_t drops = 0;
    {
        AsyncWriter w(
            [&](const std::vector<Bytes>& b) { for (auto& f : b) peer.on_frame(f); },
            [&](const Bytes& f) { peer.on_frame(f); },
            [&](std::uint8_t* buf, std::size_t cap) { return peer.poll(buf, cap); },
            512);

        std::vector<std::uint8_t> huge(MAX_PAYLOAD + 10, 0x7);
        CHECK(w.enqueue(huge.data(), huge.size()));  // ring accepts it; sender rejects
        for (int i = 0; i < 5; ++i) {
            std::uint8_t s = static_cast<std::uint8_t>(100 + i);
            CHECK(w.enqueue(&s, 1));
        }
        for (int k = 0; k < 5000 && peer.delivered_count() < 5; ++k) {
            std::this_thread::sleep_for(std::chrono::milliseconds(1));
        }
        drops = w.dropped();
    }

    std::lock_guard<std::mutex> lk(peer.m);
    CHECK(peer.delivered.size() == 5);  // the 5 normal samples got through
    CHECK(drops == 1);                  // the oversized one was skipped
}

// 3) bounded teardown — no responder, so the window can never drain; the
//    destructor must still return within the (short) drain deadline.
static void test_bounded_destructor_no_responder() {
    auto t0 = std::chrono::steady_clock::now();
    {
        AsyncWriter w([](const std::vector<Bytes>&) {}, [](const Bytes&) {},
                      [](std::uint8_t*, std::size_t) { return -1; }, 512,
                      std::chrono::milliseconds(300));
        for (int i = 0; i < 8; ++i) {
            std::uint8_t s = static_cast<std::uint8_t>(i);
            w.enqueue(&s, 1);
        }
        // Leave 8 samples unacked; destructor (finish + join) must not hang.
    }
    auto elapsed = std::chrono::steady_clock::now() - t0;
    auto ms = std::chrono::duration_cast<std::chrono::milliseconds>(elapsed).count();
    CHECK(ms < 2000);  // bounded by the 300ms deadline, not infinite
}

int main() {
    test_window_overflow_no_loss();
    test_oversized_no_head_of_line_block();
    test_bounded_destructor_no_responder();

    if (g_fail == 0) {
        std::printf("ALL OK\n");
        return 0;
    }
    std::fprintf(stderr, "%d checks failed\n", g_fail);
    return 1;
}
